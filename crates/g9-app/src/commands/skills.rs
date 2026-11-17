//! `skills.invoke` (M11/M12; ARCHITECTURE.md §5 last paragraph, §10; gates A07, A09, A11).
//!
//! The command builds the invocation envelope with the backend-assigned
//! operator as actor, runs `g9_skills::Orchestrator` (preflight → policy →
//! runtime → schema and evidence validation) with the **real** policy engine
//! and the configured runtime through adapter structs implementing the
//! `g9-skills` boundary traits, and persists only invocation metadata
//! (`skill_invocations`: hashes, usage, state; never prompt or output text).
//!
//! The output is advisory: `recommended_actions` are returned to the UI and
//! never executed; each becomes a *new* command under operator authority with
//! its own approval. A non-completed invocation (policy denial, model down,
//! rejected output) commits its metadata row and returns the orchestrator's
//! redacted error.
//!
//! Interaction mode (TEC-25, SEC-10): the mode the pipeline policy and the
//! orchestrator preflight evaluate is derived from backend state — while an
//! EXAM-mode attempt is `STARTED` the invocation runs in `EXAM` whatever mode
//! the client declared, so teaching skills (hints, worked steps) are refused
//! during an open blind attempt. Recording a refused hint request on the
//! attempt (LRN-12 `hint_requested`) is PROPOSED.
//!
//! Model output is never kept in `idempotency_records`: the stored replay
//! record has `output: null` and no recommended actions, and a replay says so
//! with the warning `idempotency.content_not_retained`. A non-completed
//! invocation stores its redacted error there (the pipeline's committed
//! failure), so a retry with the same key answers that error and records no
//! second row. A key that already names a recorded invocation without such a
//! record (a database written before, or a purged record) is refused with
//! `CONFLICT skills.invocation_already_recorded` before anything runs.
//!
//! Version integrity (ARCHITECTURE.md §10; migration 0008): the
//! `skill_versions` row `<name>@<g9-version>` is immutable and records the
//! package, `SKILL.md` and schema hashes. A loaded package whose hashes differ
//! from the row registered under its version (its files changed and its
//! `g9-version` did not) is refused before the model runs
//! (`PRECONDITION_FAILED skills.version_hash_mismatch`), so no invocation is
//! recorded against hashes of files that did not run.

use super::{decode, rc};
use crate::context::{AppContext, Services};
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation};
use g9_ai_adapter::{
    run_inference, ContextSegment as AiSegment, FinishReason, InferenceError, InferenceRequest,
    InferenceRuntime, SegmentRole as AiRole,
};
use g9_domain::hashing::{sha256_hex, sha256_json};
use g9_domain::skill::PolicyContext;
use g9_domain::{
    Actor, Capability, Classification, CommandMeta, CommandOutcome, DataDomain, Decision, G9Error,
    InteractionMode, OperationDescriptor, ResourceKind, ResourceRef, Revision, SkillCode, SkillId,
    SkillInvocationState,
};
use g9_policy::{DigestDomain, Engine, ResourceContext};
use g9_skills::{
    build_invocation, BackendError, BackendFinish, BackendRequest, BackendResponse, BackendUsage,
    CancelToken, InferenceBackend, InvocationRequest, Orchestrator, PolicyPreflight, SegmentRole,
    SkillPackage,
};
use g9_storage::repos::knowledge::{
    self as repo, ActorKindTag, InvocationPatch, SkillInvocationRow, SkillRow, SkillState,
    SkillVersionRow, ValidationOutcome,
};
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;

/// `PolicyPreflight` over the shared engine (the orchestrator asks the same
/// deny-by-default table the pipeline uses).
#[derive(Debug, Clone)]
pub struct PolicyAdapter(pub Arc<Engine>);

impl PolicyPreflight for PolicyAdapter {
    fn preflight(
        &self,
        actor: &Actor,
        op: &OperationDescriptor,
        ctx: &ResourceContext,
    ) -> Decision {
        <Engine as PolicyPreflight>::preflight(&self.0, actor, op, ctx)
    }
}

/// `InferenceBackend` over the configured `InferenceRuntime`, through the
/// gateway entry point `g9_ai_adapter::run_inference` (budget check,
/// late-response discard, structured-output validation).
#[derive(Clone)]
pub struct RuntimeBackend(pub Arc<dyn InferenceRuntime>);

impl std::fmt::Debug for RuntimeBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("RuntimeBackend")
            .field(&self.0.describe().name)
            .finish()
    }
}

fn role(r: SegmentRole) -> AiRole {
    match r {
        SegmentRole::SystemPolicy => AiRole::SystemPolicy,
        SegmentRole::SkillInstructions => AiRole::SkillInstructions,
        SegmentRole::Task => AiRole::Task,
        SegmentRole::Evidence => AiRole::Evidence,
        _ => AiRole::LearnerInput,
    }
}

impl InferenceBackend for RuntimeBackend {
    fn name(&self) -> &str {
        "g9-ai-adapter"
    }

    fn generate(
        &self,
        req: BackendRequest,
        cancel: &CancelToken,
    ) -> Result<BackendResponse, BackendError> {
        if cancel.is_cancelled() {
            return Err(BackendError::Cancelled);
        }
        let request = InferenceRequest {
            invocation_id: req.invocation_id,
            segments: req
                .segments
                .into_iter()
                .map(|s| {
                    AiSegment::new(
                        role(s.role),
                        s.text,
                        s.trust,
                        s.classification,
                        s.source_ref,
                    )
                })
                .collect(),
            max_output_tokens: req.max_output_tokens,
            timeout: req.timeout,
            response_schema: Some(req.response_schema),
            temperature: 0.0,
            template_version: req.template_version,
        };
        // The two crates define separate cancel tokens; the skills token is
        // checked before and after the blocking call (late responses are discarded).
        let token = g9_ai_adapter::CancelToken::new();
        let response = run_inference(self.0.as_ref(), request, &token).map_err(|e| match e {
            InferenceError::ModelUnavailable => BackendError::Unavailable,
            InferenceError::Timeout => BackendError::Timeout,
            InferenceError::Cancelled => BackendError::Cancelled,
            InferenceError::Budget { reason } => BackendError::Budget(reason),
            InferenceError::Runtime(_) => BackendError::Runtime("runtime failure".to_string()),
            InferenceError::InvalidOutput(issues) => BackendError::Runtime(format!(
                "structured output invalid ({} issue(s))",
                issues.len()
            )),
        })?;
        if cancel.is_cancelled() {
            return Err(BackendError::Cancelled);
        }
        Ok(BackendResponse {
            text: response.text,
            structured: response.structured,
            usage: BackendUsage {
                prompt_tokens: response.usage.prompt_tokens,
                output_tokens: response.usage.output_tokens,
                total_ms: u64::try_from(response.usage.total_ms.as_millis()).unwrap_or(0),
            },
            finish: match response.finish {
                FinishReason::Stop => BackendFinish::Stop,
                FinishReason::Length => BackendFinish::Length,
                FinishReason::Cancelled => BackendFinish::Cancelled,
                FinishReason::Timeout => BackendFinish::Timeout,
                FinishReason::Error => BackendFinish::Error,
            },
            model_id: Some(response.model.model_id),
        })
    }
}

#[derive(Debug, Deserialize)]
struct InvokeRequest {
    skill_code: String,
    input: Value,
    mode: Option<InteractionMode>,
}

fn find_package<'a>(svc: &'a Services, code: &str) -> Result<&'a SkillPackage, G9Error> {
    let registry = svc.skills.as_ref().ok_or_else(|| {
        G9Error::resource_unavailable(
            "skills.registry_unavailable",
            "no validated skill registry is configured",
        )
    })?;
    let by_code = SkillCode::parse(code)
        .ok()
        .and_then(|c| registry.by_code(&c));
    let by_name = SkillId::parse(code).ok().and_then(|id| registry.get(&id));
    by_code
        .or(by_name)
        .ok_or_else(|| G9Error::not_found("skills.not_registered", "skill is not registered"))
}

/// Register the package's skill and version rows once (FK targets of `skill_invocations`), and
/// refuse a package whose hashes differ from the version row registered under its version.
///
/// Only a package from a pinned registry (`Registry::load_pinned`, as the desktop application
/// loads) is registered under the plain `<name>@<version>` id. A package from an unpinned
/// registry (the CLI's `--skills-dir`) is registered under `<name>@<version>+<first 12 hex of
/// its package hash>`: rows cannot be deleted, so an unpinned package registered first would
/// otherwise block the pinned package of the same version for good (final security review
/// authz-2), and two unpinned contents never share a row.
fn ensure_registered(
    conn: &Connection,
    package: &SkillPackage,
    pinned: bool,
    operator: &Actor,
    now: g9_domain::UtcMillis,
) -> Result<String, G9Error> {
    if repo::get_skill(conn, &package.name)?.is_none() {
        repo::insert_skill(
            conn,
            &SkillRow {
                skill_id: package.name.clone(),
                code: package.code.clone(),
                family: package.family,
                title: package.description.chars().take(200).collect(),
                package_dir: package
                    .dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(package.name.as_str())
                    .to_string(),
                current_version_label: Some(package.version.clone()),
                state: SkillState::Validated,
                revision: Revision::INITIAL,
                created_at: now,
                updated_at: now,
            },
        )?;
    }
    // An unpinned package's label carries its content as SemVer build metadata
    // (`0.1.0+<12 hex>`), so its row is unique per content (UNIQUE skill_id, version_label).
    let version_label = if pinned {
        package.version.clone()
    } else {
        let short = package
            .package_sha256
            .get(..12)
            .unwrap_or(&package.package_sha256);
        format!("{}+{short}", package.version)
    };
    let version_id = format!("{}@{version_label}", package.name.as_str());
    let loaded = SkillVersionRow {
        id: version_id.clone(),
        skill_id: package.name.clone(),
        version_label,
        package_sha256: package.package_sha256.clone(),
        skill_md_sha256: sha256_hex(package.body.as_bytes()),
        input_schema_sha256: sha256_json(&package.input_schema),
        output_schema_sha256: sha256_json(&package.output_schema),
        budget: package.budget,
        allowed_tool_ids: Vec::new(),
        registered_by: operator.id.clone(),
        registered_at: now,
        created_at: now,
    };
    match repo::get_skill_version(conn, &version_id)? {
        None => repo::insert_skill_version(conn, &loaded)?,
        Some(stored) => {
            let same = stored.package_sha256 == loaded.package_sha256
                && stored.skill_md_sha256 == loaded.skill_md_sha256
                && stored.input_schema_sha256 == loaded.input_schema_sha256
                && stored.output_schema_sha256 == loaded.output_schema_sha256;
            if !same {
                return Err(G9Error::precondition_failed(
                    "skills.version_hash_mismatch",
                    "the loaded skill package differs from the package registered under the same version; a changed package needs a new g9-version",
                )
                .with_details(json!({ "skill_version_id": version_id })));
            }
        }
    }
    Ok(version_id)
}

/// Persist the invocation metadata: insert `REQUESTED`, then replay the
/// trace's transitions through the storage state machine; the final transition
/// carries the outcome patch (hashes, usage, validation, error category).
fn persist_invocation(
    conn: &Connection,
    row: SkillInvocationRow,
    transitions: &[(SkillInvocationState, g9_domain::UtcMillis)],
    last_patch: InvocationPatch,
) -> Result<(), G9Error> {
    let mut revision = repo::insert_invocation(conn, &row)?;
    let mut previous = SkillInvocationState::Requested;
    let steps: Vec<&(SkillInvocationState, g9_domain::UtcMillis)> = transitions
        .iter()
        .skip_while(|(s, _)| *s == SkillInvocationState::Requested)
        .collect();
    for (i, (state, at)) in steps.iter().enumerate() {
        let event = SkillInvocationState::TRANSITIONS
            .iter()
            .find(|(from, _, to)| *from == previous && to == state)
            .map(|(_, event, _)| *event)
            .ok_or_else(|| G9Error::internal_redacted("skills.trace_transition"))?;
        let patch = if i + 1 == steps.len() {
            last_patch.clone()
        } else {
            InvocationPatch::default()
        };
        let (next, rev) = repo::transition_invocation(conn, &row.id, revision, event, &patch, *at)?;
        previous = next;
        revision = rev;
    }
    Ok(())
}

/// `skills.invoke`.
pub fn invoke(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: InvokeRequest = decode(payload)?;
    // `meta.scope.mode` is the mode the backend evaluates (derived in dispatch, TEC-25), so a
    // payload mode that disagrees with it is refused, an open exam included.
    if req.mode.is_some_and(|m| m != meta.scope.mode) {
        return Err(G9Error::invalid_input(
            "skills.mode_mismatch",
            "payload mode must equal the command scope mode",
        ));
    }
    let resource = ResourceRef::new(ResourceKind::Skill, req.skill_code.as_str());
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |_, svc| {
            find_package(svc, &req.skill_code)?;
            Ok(Facts::new_record(rc(
                meta.scope.domain,
                Classification::Internal,
                false,
            )))
        },
        |env| {
            let package = find_package(env.svc, &req.skill_code)?.clone();
            let registry = env.svc.skills.clone().ok_or_else(|| {
                G9Error::resource_unavailable("skills.registry_unavailable", "no skill registry")
            })?;
            // Before anything runs: the key must not name a recorded invocation, and the
            // package must be the one registered under its version (module docs).
            if let Some(recorded) = repo::find_invocation_by_idempotency(
                env.conn,
                &env.meta.actor.id,
                &env.meta.idempotency_key,
            )? {
                return Err(G9Error::conflict(
                    "skills.invocation_already_recorded",
                    "a skill invocation is already recorded under this idempotency key; send a new idempotency key to invoke the skill again",
                )
                .with_details(json!({
                    "invocation_id": recorded.id,
                    "state": recorded.state,
                })));
            }
            let pinned = env
                .svc
                .skills
                .as_ref()
                .is_some_and(g9_skills::Registry::is_pinned);
            let version_id =
                ensure_registered(env.conn, &package, pinned, &env.meta.actor, env.now)?;
            let granted: Vec<Capability> = [Capability::SkillInvoke, Capability::ModelInference]
                .into_iter()
                .filter(|c| package.requested_capabilities.contains(c))
                .collect();
            let envelope = build_invocation(InvocationRequest {
                package: &package,
                actor: env.meta.actor.clone(),
                intent: format!("operator invoked {}", package.code.as_str()),
                scope: env.meta.scope,
                classification: Classification::Private,
                input_payload: req.input.clone(),
                evidence_refs: Vec::new(),
                policy_context: PolicyContext {
                    mode: env.meta.scope.mode,
                    granted_capabilities: granted,
                    recursion_depth: 0,
                },
                idempotency_key: env.meta.idempotency_key.clone(),
                trace_id: env.meta.trace_id.clone(),
            })
            .map_err(|e| G9Error::invalid_input("skills.envelope_invalid", e.to_string()))?;
            let clock = env.svc.clock.clone();
            let orchestrator = Orchestrator::new(
                registry,
                Box::new(PolicyAdapter(env.svc.policy.clone())),
                Box::new(RuntimeBackend(env.svc.runtime.clone())),
                Box::new(move || clock()),
            );
            let outcome = orchestrator.invoke(envelope.clone(), &CancelToken::new());
            let trace = &outcome.record;
            let completed = outcome.state == SkillInvocationState::Completed;
            let output_json = match (&outcome.output, completed) {
                (Some(out), true) => Some(
                    serde_json::to_value(out)
                        .map_err(|_| G9Error::internal_redacted("skills.output"))?,
                ),
                _ => None,
            };
            // The ledger keeps keyed digests of a skill's input and output (ADR-0021): both can be
            // operator content, and `skill_invocations` cannot be erased.
            let input_digest = env
                .svc
                .binding_key
                .digest_json(DigestDomain::SkillInput, &req.input)
                .map_err(|_| G9Error::internal_redacted("digest.key_rejected"))?;
            let output_digest = output_json
                .as_ref()
                .map(|out| {
                    env.svc
                        .binding_key
                        .digest_json(DigestDomain::SkillOutput, out)
                })
                .transpose()
                .map_err(|_| G9Error::internal_redacted("digest.key_rejected"))?;
            let validation = if trace.validation.is_empty() {
                ValidationOutcome::NotRun
            } else if trace.validation.iter().all(|c| c.passed) {
                ValidationOutcome::Valid
            } else {
                ValidationOutcome::Invalid
            };
            persist_invocation(
                env.conn,
                SkillInvocationRow {
                    id: envelope.invocation_id.clone(),
                    skill_id: package.name.clone(),
                    skill_version_id: version_id,
                    actor_id: env.meta.actor.id.clone(),
                    actor_kind: ActorKindTag::of(&env.meta.actor),
                    intent: envelope.intent.clone(),
                    scope: env.meta.scope,
                    data_classification: envelope.data_classification,
                    state: SkillInvocationState::Requested,
                    model_id: None,
                    model_id_label: None,
                    template_version: Some(trace.template_version.clone()),
                    input_sha256: input_digest,
                    output_sha256: None,
                    output_status: None,
                    validation_outcome: ValidationOutcome::NotRun,
                    usage: Default::default(),
                    latency_ms: None,
                    policy_decision_id: None,
                    approval_id: None,
                    idempotency_key: env.meta.idempotency_key.clone(),
                    trace_id: env.meta.trace_id.clone(),
                    error_category: None,
                    requested_at: env.now,
                    finished_at: None,
                    revision: Revision::INITIAL,
                    created_at: env.now,
                    updated_at: env.now,
                },
                &trace.transitions,
                InvocationPatch {
                    model_id: None,
                    model_id_label: env
                        .svc
                        .runtime
                        .describe()
                        .model
                        .map(|m| m.model_id.as_str().to_string()),
                    template_version: Some(trace.template_version.clone()),
                    output_sha256: output_digest,
                    output_status: if completed {
                        outcome.output.as_ref().map(|o| o.status)
                    } else {
                        None
                    },
                    validation_outcome: Some(validation),
                    usage: trace.usage,
                    latency_ms: None,
                    policy_decision_id: None,
                    approval_id: None,
                    error_category: trace.error.as_ref().map(|e| e.category),
                },
            )?;
            let event = EventSpec {
                domain: DataDomain::System,
                ..EventSpec::system(
                    "skill.invoked",
                    json!({
                        "invocation_id": envelope.invocation_id,
                        "skill_id": package.name,
                        "state": outcome.state,
                        "policy_rule_id": trace.policy_rule_id,
                        "validation": validation,
                    }),
                )
            };
            if !completed {
                let error = trace
                    .error
                    .clone()
                    .unwrap_or_else(|| G9Error::internal_redacted("skills.invocation_failed"));
                let error = G9Error {
                    details: Some(json!({
                        "invocation_id": envelope.invocation_id,
                        "state": outcome.state,
                        "skill_code": package.code.as_str(),
                        "failed_checks": trace.validation.iter().filter(|c| !c.passed).map(|c| c.check.clone()).collect::<Vec<_>>(),
                    })),
                    ..error
                };
                return Ok(Mutation::failed(error).with_event(event));
            }
            let out = outcome
                .output
                .as_ref()
                .ok_or_else(|| G9Error::internal_redacted("skills.output_missing"))?;
            let actions: Vec<Value> = out
                .recommended_actions
                .iter()
                .map(|a| {
                    let action = serde_json::to_value(a.action).ok();
                    let kind = serde_json::to_value(a.resource.kind).ok();
                    json!({
                        "operation": format!(
                            "{} {} {}",
                            action.as_ref().and_then(Value::as_str).unwrap_or("?"),
                            kind.as_ref().and_then(Value::as_str).unwrap_or("?"),
                            a.resource.id
                        ),
                        "payload": {
                            "action": a.action,
                            "resource": a.resource,
                            "parameters": a.parameters,
                            "evidence_refs": a.evidence_refs,
                        },
                        "rationale": a.rationale,
                    })
                })
                .collect();
            let usage = trace.usage.unwrap_or_default();
            let result = json!({
                "invocation_id": envelope.invocation_id,
                "skill_code": package.code.as_str(),
                "state": outcome.state,
                "output": outcome.result,
                "recommended_actions": actions,
                "provenance": "MODEL_OUTPUT",
                "validation": validation,
                "budget": {
                    "tokens_used": usage.tokens,
                    "elapsed_ms": i64::from(usage.seconds) * 1_000,
                    "tool_calls": usage.tool_calls,
                },
            });
            // Model output is content: the idempotency record keeps the
            // envelope without it (see module docs).
            let mut replay = result.clone();
            replay["output"] = Value::Null;
            replay["recommended_actions"] = json!([]);
            Ok(Mutation::new(result, Some(Revision::INITIAL))
                .with_event(event)
                .with_warning("skills.output_advisory: recommended actions are proposals and were not executed")
                .replay_with(replay))
        },
    )
}

/// Replayed `skills.invoke` result: the stored envelope without model output
/// (see module docs).
pub(crate) fn replay_view(record: &Value) -> (Value, Vec<String>) {
    (
        record.clone(),
        vec![
            "idempotency.content_not_retained: model output and recommended actions are not retained for replay; invoke the skill again to see them".to_string(),
        ],
    )
}
