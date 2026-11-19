//! Assessment commands (M07; ARCHITECTURE.md §8; gates A04, A07).
//!
//! * The attempt moves only along `AssessmentAttemptState` (pure transition
//!   table); `APPROVED_FINAL` has no outgoing edge, and the policy invariant
//!   `invariant.immutable_finalized` plus the storage triggers refuse any later
//!   update.
//! * `assessment.grade_provisional` records anchored levels 0–4 per criterion,
//!   converted with the rubric's explicit level scale (ADR-0012, LRN-10;
//!   standard 0/25/50/80/100) to the crate's 0–100 scale; a level above 4 is
//!   `INVALID_INPUT`. Results are append-only, a re-grade supersedes.
//! * `assessment.approve_final` requires `Capability::GradeFinalize` (a
//!   consumed operator approval), calls `g9_assessment::finalize_decision`
//!   with the backend-assigned operator as reviewer (`GradeSource::ModelAdvisory`
//!   can never finalize), appends the immutable `grade_decisions` row with the
//!   reliability class the crate classified, and finalizes the attempt in the
//!   same transaction. Attempts store no learner column; in the MVP the
//!   operator takes every attempt, so the operator is the learner and an
//!   operator finalization is `SELF_ADJUDICATED` (ADR-0012, LRN-11).
//! * `assessment.begin` with `retake_of` starts the retake on the newest
//!   other version whose task was never shown to the operator, else the
//!   newest version that differs from the original's with the warning
//!   `assessment.retake_exposed_variant` (LRN-13: a retake uses a different
//!   variant). When the assessment has no other version the retake
//!   reuses the original variant and the outcome carries the warning
//!   `assessment.retake_same_variant` (the item is no longer blind-eligible).
//!   A `begin` without `retake_of` on an assessment that already has a
//!   terminal attempt is recorded as a retake of the latest terminal attempt
//!   (warning `assessment.retake_inferred`; Addendum A.3: retakes are linked
//!   and reported separately, never counted as first attempts).
//! * Evidence references are checked against storage, not only parsed
//!   (app-6, app-8): `assessment.submit` links only evidence items that exist in
//!   the attempt's domain (anything else is `NOT_FOUND evidence.not_found`, as
//!   seen from the attempt's scope), and every `evidence_refs` entry of
//!   `assessment.grade_provisional` must be evidence linked to the attempt being
//!   graded (`attempt_evidence`) in its domain, else `INVALID_INPUT
//!   grade.evidence_unknown` and no criterion result is appended: a result is
//!   append-only and hashed into the final decision, so it never cites evidence
//!   that cannot be produced.
//! * `assessment.grade_provisional` records the backend-assigned operator as
//!   `scored_by`; the operator may not claim `DETERMINISTIC_CHECKER`
//!   provenance (`CriterionResult::new`: only a `System` checker writes it →
//!   `UNAUTHORIZED assessment.grade.source_actor_mismatch`). A relayed
//!   `MODEL_ADVISORY` score is accepted; it can never finalize on its own.
//!
//! * Observation derivation (ARCHITECTURE.md §8, Addendum A.2; ADR-0005/0012):
//!   inside the transaction that appends the final `grade_decisions` row and
//!   moves the attempt to `APPROVED_FINAL`, `assessment.approve_final` calls
//!   `g9_assessment::observations_from_decision` (Operator provenance: the
//!   decision is an operator-approved final decision) and appends one
//!   append-only `competency_observations` row per `(competency, dimension)`
//!   (source `Attempt`, reviewer = the deciding operator, environment
//!   `assessment:<definition id>@<version label>`, uncertainty 0 = the
//!   `g9-competency` "certain" value, as no measurement-error model exists) plus
//!   one `competency_observation_evidence` row carrying the decision's
//!   reliability class, `blind` and `validation_eligible`. `competency.projection`
//!   passes the `validation_eligible = 0` ids to
//!   `g9_competency::derive_state_excluding_validation`, so a `SELF_ADJUDICATED`
//!   (every operator finalization in the MVP), failed or non-blind decision
//!   never validates. The difficulty is the one the version's author declared
//!   (`assessment_versions.difficulty`, nullable, never defaulted): a version
//!   that declares none derives nothing and the outcome carries the warning
//!   `competency.observations_not_derived` naming that reason; so does an
//!   assessment linked to no competency. Blindness is computed from the
//!   attempt's recorded assistance level and its `exposure_log` rows, and an
//!   item that was consumed before the attempt began is never blind (LRN-13: the
//!   task of the same version was shown to the operator for an earlier attempt,
//!   or its key was shown up to the submission with no attempt attached or for
//!   another attempt; see `consumed_before_attempt`).
//!   A replay of the command (same idempotency key) returns the stored outcome
//!   and appends nothing. No operation produces a second final decision for an
//!   attempt (`APPROVED_FINAL` has no outgoing edge and the repository has no
//!   adjudication command), so no derived observation supersedes another here;
//!   an adjudication command is PROPOSED (it would append observations whose
//!   `supersedes` names the earlier decision's observation for the same
//!   competency and dimension).
//!
//! PROPOSED (not wired, reported): gate failures (ADR-0012) cannot be declared
//! through IPC (`assessment.approve_final` carries only `attempt_id`) and
//! `grade_decisions` has no gate-failure column, so finalization here records
//! none. Only the operator can finalize and the operator is the learner, so
//! every decision this crate records is `SELF_ADJUDICATED` and none of its
//! observations is validation-eligible; an external reviewer or a deterministic
//! checker deciding an attempt is PROPOSED (the derivation path below already
//! records `EXTERNAL_REVIEW` / `DETERMINISTIC` decisions correctly; it is
//! tested at crate level with a distinct learner).

use super::{creation_id, decode, new_id, parse_id, rc, scope_of, tail, to_value};
use crate::context::AppContext;
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation};
use g9_assessment::{
    finalize_decision, observations_from_decision, score_breakdown, Attempt, CriterionResult,
    DecisionInput, ExposureKind, ExposureRecord, GradeDecision, ReliabilityClass as DecisionClass,
    Rubric,
};
use g9_competency::{CompetencyObservation, ObservationSource};
use g9_domain::hashing::{sha256_hex, sha256_json};
use g9_domain::state::AssessmentEvent;
use g9_domain::{
    Actor, ActorId, AssessmentAttemptState, AssessmentId, AttemptId, Classification, CommandMeta,
    CommandOutcome, Confidence, Difficulty, EvidenceId, G9Error, GradeDecisionId, GradeSource,
    NoteId, ObservationId, Provenance, ResourceKind, ResourceRef, Revision, SourceKind, TrustClass,
    UtcMillis,
};
use g9_evidence::{EvidenceItem, EvidenceKind, MAX_NOTE_BYTES};
use g9_storage::repos::assessment::{
    self as repo, AssessmentDefinitionRow, AssessmentVersionRow, AttemptEvidenceRole,
    AttemptEvidenceRow, AttemptPatch, AttemptRow, CriterionResultRow, ExposedTo,
    ExposureContentKind, ExposureLogRow, GradeDecisionRow, ReliabilityClass, RetakeLinkRow,
    RubricCriterionRow,
};
use g9_storage::repos::competency::{self as competency_repo, ObservationEvidenceRow};
use g9_storage::repos::evidence::{self as evidence_repo, EvidenceItemRow, NoteKind, NoteRow};
use g9_storage::repos::ReadScope;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{json, Value};

struct Loaded {
    attempt: AttemptRow,
    version: AssessmentVersionRow,
    definition: AssessmentDefinitionRow,
    criteria: Vec<RubricCriterionRow>,
}

fn illegal(from: AssessmentAttemptState, event: AssessmentEvent) -> G9Error {
    G9Error::precondition_failed(
        "assessment.illegal_transition",
        format!("assessment_attempt: event {event:?} is not allowed in state {from}"),
    )
    .with_details(json!({ "machine": "assessment_attempt", "from": from, "event": event }))
}

fn load_attempt(
    conn: &Connection,
    scope: &ReadScope,
    id: &AttemptId,
) -> Result<AttemptRow, G9Error> {
    repo::get_attempt(conn, scope, id)?.ok_or_else(|| {
        G9Error::not_found("attempt.not_found", "attempt not found")
            .with_details(json!({ "attempt_id": id }))
    })
}

fn load(conn: &Connection, scope: &ReadScope, id: &AttemptId) -> Result<Loaded, G9Error> {
    let attempt = load_attempt(conn, scope, id)?;
    let version = repo::get_version(conn, &attempt.assessment_version_id)?.ok_or_else(|| {
        G9Error::data_corruption(
            "assessment.version_missing",
            "attempt references a missing version",
        )
    })?;
    let definition = repo::get_definition(
        conn,
        &ReadScope::cross_domain(attempt.data_domain),
        &version.definition_id,
    )?
    .ok_or_else(|| {
        G9Error::data_corruption(
            "assessment.definition_missing",
            "version references a missing definition",
        )
    })?;
    let (_, criteria) = repo::rubric_for_version(conn, &version.id)?.ok_or_else(|| {
        G9Error::precondition_failed(
            "assessment.rubric_missing",
            "assessment version has no rubric",
        )
    })?;
    Ok(Loaded {
        attempt,
        version,
        definition,
        criteria,
    })
}

fn rubric(criteria: &[RubricCriterionRow]) -> Result<Rubric, G9Error> {
    Ok(repo::rubric_from_rows(
        criteria,
        Rubric::baseline_dimension,
    )?)
}

fn results(conn: &Connection, attempt: &AttemptId) -> Result<Vec<CriterionResult>, G9Error> {
    repo::effective_criterion_results(conn, attempt)?
        .iter()
        .map(|r| r.to_domain().map_err(G9Error::from))
        .collect()
}

/// Storage form of the crate's reliability class.
const fn stored_class(class: DecisionClass) -> ReliabilityClass {
    match class {
        DecisionClass::Deterministic => ReliabilityClass::Deterministic,
        DecisionClass::ExternalReview => ReliabilityClass::ExternalReview,
        DecisionClass::SelfAdjudicated => ReliabilityClass::SelfAdjudicated,
    }
}

/// Finalize with the crate rules. `operator` is both the learner (MVP: the
/// operator takes every attempt; attempts store no learner) and the reviewer.
/// No gate failure can be declared through IPC yet (PROPOSED).
fn decide_final(
    conn: &Connection,
    loaded: &Loaded,
    attempt: &Attempt,
    operator: &Actor,
    now: UtcMillis,
) -> Result<(GradeDecision, Vec<CriterionResult>), G9Error> {
    let rubric = rubric(&loaded.criteria)?;
    let results = results(conn, &loaded.attempt.id)?;
    let decision = finalize_decision(
        GradeDecisionId::new_v7(),
        DecisionInput {
            attempt,
            rubric: &rubric,
            results: &results,
            gate_failures: &[],
            reviewer: operator,
            now,
        },
    )?;
    Ok((decision, results))
}

fn grade_view(conn: &Connection, loaded: &Loaded, operator: &ActorId) -> Result<Value, G9Error> {
    let stored = repo::effective_criterion_results(conn, &loaded.attempt.id)?;
    if stored.is_empty() {
        return Ok(Value::Null);
    }
    let rubric = rubric(&loaded.criteria)?;
    let domain: Vec<CriterionResult> = stored
        .iter()
        .map(|r| r.to_domain().map_err(G9Error::from))
        .collect::<Result<_, _>>()?;
    let breakdown = score_breakdown(&rubric, &domain);
    let decision = repo::effective_final_decision(conn, &loaded.attempt.id)?;
    let criteria: Vec<Value> = stored
        .iter()
        .map(|r| {
            let weight = loaded
                .criteria
                .iter()
                .find(|c| c.code == r.criterion_code)
                .map_or(0, |c| c.weight);
            json!({
                "criterion_code": r.criterion_code,
                "weight": weight,
                "score": rubric.level_for_score(r.row.score).get(),
                "source": r.row.source,
                "evidence_refs": r.row.evidence_refs,
            })
        })
        .collect();
    let gate_failures: Vec<String> = breakdown
        .per_criterion
        .iter()
        .filter(|c| c.critical_gate && c.gate_passed == Some(false))
        .map(|c| c.code.clone())
        .collect();
    let (total, pass, class, is_final) = match &decision {
        Some(d) => (
            f64::from(d.total_score),
            d.passed,
            d.reliability_class,
            true,
        ),
        // The class an operator finalization would record now: the operator
        // is the learner (MVP), so it is SELF_ADJUDICATED (LRN-11).
        None => (
            breakdown
                .weighted_total
                .map_or(0.0, |t| (t * 100.0).round() / 100.0),
            breakdown.pass_recommendation.unwrap_or(false),
            stored_class(DecisionClass::classify(
                operator, operator, &rubric, &domain,
            )),
            false,
        ),
    };
    to_value(&json!({
        "total": total,
        "pass": pass,
        "gate_failures": gate_failures,
        "criteria": criteria,
        "reliability_class": class,
        "final": is_final,
    }))
}

fn attempt_view(conn: &Connection, loaded: &Loaded, operator: &ActorId) -> Result<Value, G9Error> {
    let a = &loaded.attempt;
    to_value(&json!({
        "attempt_id": a.id,
        "assessment_id": loaded.definition.id,
        "title": loaded.definition.title,
        "state": a.state,
        "mode": a.mode,
        "variant": loaded.version.version_label,
        "retake_of": a.retake_of,
        "started_at": a.started_at,
        "submitted_at": a.submitted_at,
        "finalized_at": a.finalized_at,
        "grade": grade_view(conn, loaded, operator)?,
        "revision": a.revision,
    }))
}

fn reload_view(conn: &Connection, loaded: &Loaded, operator: &ActorId) -> Result<Value, G9Error> {
    let fresh = load(
        conn,
        &ReadScope::cross_domain(loaded.attempt.data_domain),
        &loaded.attempt.id,
    )?;
    attempt_view(conn, &fresh, operator)
}

/// `assessment.list` (read-only).
pub fn list(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let resource = ResourceRef::new(ResourceKind::Attempt, "attempts");
    let scope = scope_of(meta);
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |_, _| {
            Ok(Facts::new_record(rc(
                meta.scope.domain,
                Classification::Private,
                false,
            )))
        },
        |env| {
            let definitions: Vec<Value> = repo::list_definitions(env.conn, &scope)?
                .iter()
                .map(|d| {
                    json!({
                        "assessment_id": d.id,
                        "title": d.title,
                        "day_id": d.training_day_id.as_deref().map_or(d.code.as_str(), tail),
                        "mode": d.mode,
                    })
                })
                .collect();
            let mut attempts: Vec<(Option<UtcMillis>, String, Value)> = Vec::new();
            for state in AssessmentAttemptState::ALL {
                for row in repo::list_attempts_by_state(env.conn, &scope, *state)? {
                    let loaded = load(env.conn, &scope, &row.id)?;
                    attempts.push((
                        row.started_at,
                        row.id.as_str().to_string(),
                        attempt_view(env.conn, &loaded, &env.meta.actor.id)?,
                    ));
                }
            }
            attempts.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            let attempts: Vec<Value> = attempts.into_iter().map(|a| a.2).collect();
            Ok(Mutation::new(
                json!({ "definitions": definitions, "attempts": attempts }),
                None,
            ))
        },
    )
}

#[derive(Debug, Deserialize)]
struct BeginRequest {
    assessment_id: String,
    retake_of: Option<String>,
}

/// `assessment.begin`: a new `STARTED` attempt on the latest version; a retake
/// links to a terminal original and never modifies it.
pub fn begin(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: BeginRequest = decode(payload)?;
    let def_id: AssessmentId = parse_id(&req.assessment_id, "assessment_id")?;
    let retake_of: Option<AttemptId> = req
        .retake_of
        .as_deref()
        .map(|r| parse_id(r, "retake_of"))
        .transpose()?;
    let attempt_id: AttemptId = parse_id(
        &creation_id(ctx, spec, meta, payload, || {
            AttemptId::new_v7().as_str().to_string()
        })?,
        "attempt_id",
    )?;
    let resource = ResourceRef::new(ResourceKind::Attempt, attempt_id.as_str());
    let scope = scope_of(meta);
    let facts_def = def_id.clone();
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        move |conn, _| {
            let def = repo::get_definition(conn, &scope, &facts_def)?.ok_or_else(|| {
                G9Error::not_found("assessment.not_found", "assessment definition not found")
            })?;
            Ok(Facts::new_record(rc(
                def.data_domain,
                Classification::Private,
                false,
            )))
        },
        move |env| {
            let def = repo::get_definition(env.conn, &scope, &def_id)?.ok_or_else(|| {
                G9Error::not_found("assessment.not_found", "assessment definition not found")
            })?;
            let mut versions = repo::list_versions(env.conn, &def.id)?;
            versions.sort_by_key(|v| v.version_number().unwrap_or(0));
            let mut version = versions.last().cloned().ok_or_else(|| {
                G9Error::precondition_failed("assessment.no_version", "assessment has no version")
            })?;
            let mut same_variant = false;
            let mut exposed_variant = false;
            let mut latest_terminal: Option<AttemptRow> = None;
            for v in &versions {
                for a in repo::list_attempts_for_version(
                    env.conn,
                    &ReadScope::cross_domain(def.data_domain),
                    &v.id,
                )? {
                    if !a.state.is_terminal() {
                        return Err(G9Error::conflict(
                            "attempt.already_open",
                            "an attempt for this assessment is already in progress",
                        )
                        .with_details(json!({ "attempt_id": a.id })));
                    }
                    let key =
                        |r: &AttemptRow| (r.started_at, r.created_at, r.id.as_str().to_string());
                    if latest_terminal.as_ref().is_none_or(|l| key(&a) > key(l)) {
                        latest_terminal = Some(a);
                    }
                }
            }
            // Addendum A.3 / LRN-13: a second attempt on the same assessment is
            // a retake, whether or not the client said so.
            let inferred = retake_of.is_none() && latest_terminal.is_some();
            let retake_of = retake_of.or_else(|| latest_terminal.map(|a| a.id));
            if let Some(original) = &retake_of {
                let orig = load_attempt(env.conn, &scope, original)?;
                if !orig.state.is_terminal() {
                    return Err(G9Error::precondition_failed(
                        "assessment.retake_of_open_attempt",
                        "a retake requires a finalized or voided original",
                    ));
                }
                let orig_version = repo::get_version(env.conn, &orig.assessment_version_id)?;
                if orig_version.map(|v| v.definition_id) != Some(def.id.clone()) {
                    return Err(G9Error::invalid_input(
                        "assessment.retake_definition_mismatch",
                        "retake_of belongs to another assessment",
                    ));
                }
                // LRN-13: a retake uses a different variant — the newest
                // other version whose task was never shown, else the newest
                // other version (flagged), else the only one (flagged).
                let shown_until = env
                    .now
                    .checked_add(
                        g9_domain::DurationMs::new(1)
                            .map_err(|_| G9Error::internal_redacted("assessment.duration"))?,
                    )
                    .unwrap_or(env.now);
                let mut unexposed = None;
                for v in versions
                    .iter()
                    .rev()
                    .filter(|v| v.id != orig.assessment_version_id)
                {
                    if !repo::exposed_before(
                        env.conn,
                        &v.id,
                        ExposureContentKind::Task,
                        ExposedTo::Operator,
                        shown_until,
                    )? {
                        unexposed = Some(v.clone());
                        break;
                    }
                }
                let other = versions
                    .iter()
                    .rev()
                    .find(|v| v.id != orig.assessment_version_id)
                    .cloned();
                match (unexposed, other) {
                    (Some(v), _) => version = v,
                    (None, Some(v)) => {
                        version = v;
                        exposed_variant = true;
                    }
                    // Only one version exists: `version` is the original's.
                    (None, None) => same_variant = true,
                }
            }
            let state = AssessmentAttemptState::Available
                .transition(AssessmentEvent::Begin)
                .map_err(|_| illegal(AssessmentAttemptState::Available, AssessmentEvent::Begin))?;
            let row = AttemptRow {
                id: attempt_id.clone(),
                assessment_version_id: version.id.clone(),
                session_id: None,
                retake_of: retake_of.clone(),
                state,
                mode: def.mode,
                assistance_level: g9_domain::AssistanceLevel::Unaided,
                hint_count: 0,
                started_at: Some(env.now),
                submitted_at: None,
                finalized_at: None,
                submission_sha256: None,
                data_domain: def.data_domain,
                classification: Classification::Private,
                revision: Revision::INITIAL,
                created_at: env.now,
                updated_at: env.now,
            };
            let revision = repo::insert_attempt(env.conn, &row)?;
            if let Some(original) = &retake_of {
                repo::insert_retake_link(
                    env.conn,
                    &RetakeLinkRow {
                        original_attempt_id: original.clone(),
                        retake_attempt_id: attempt_id.clone(),
                        reason: if inferred {
                            "operator retake (inferred: a terminal attempt already existed)"
                                .to_string()
                        } else {
                            "operator retake".to_string()
                        },
                        created_by: env.meta.actor.id.clone(),
                        created_at: env.now,
                    },
                )?;
            }
            // Displaying the task to the learner is an exposure (LRN-13).
            repo::append_exposure(
                env.conn,
                &ExposureLogRow {
                    id: new_id("ex"),
                    assessment_version_id: version.id.clone(),
                    attempt_id: Some(attempt_id.clone()),
                    exposed_to: ExposedTo::Operator,
                    actor_id: env.meta.actor.id.clone(),
                    mode: def.mode,
                    content_kind: ExposureContentKind::Task,
                    content_sha256: version.task_sha256.clone(),
                    reason: Some("attempt begun".to_string()),
                    exposed_at: env.now,
                    created_at: env.now,
                },
            )?;
            let loaded = load(
                env.conn,
                &ReadScope::cross_domain(def.data_domain),
                &attempt_id,
            )?;
            let mutation = Mutation::new(
                attempt_view(env.conn, &loaded, &env.meta.actor.id)?,
                Some(revision),
            )
            .with_event(EventSpec::learning(
                "assessment.attempt.started",
                json!({
                    "attempt_id": attempt_id,
                    "assessment_id": def.id,
                    "retake_of": retake_of,
                    "variant": version.version_label,
                    "retake_same_variant": same_variant,
                    "retake_exposed_variant": exposed_variant,
                    "retake_inferred": inferred,
                }),
            ));
            let mutation = if inferred {
                mutation.with_warning(
                    "assessment.retake_inferred: this assessment already has a terminal attempt; the new attempt is recorded as a retake of the latest one (Addendum A.3, LRN-13)",
                )
            } else {
                mutation
            };
            Ok(if same_variant {
                mutation.with_warning(
                    "assessment.retake_same_variant: the assessment has no other variant; this retake reuses the original variant, which is no longer blind-eligible (LRN-13)",
                )
            } else if exposed_variant {
                mutation.with_warning(
                    "assessment.retake_exposed_variant: every other variant was already shown; this retake uses one of them and is not blind-eligible (LRN-13)",
                )
            } else {
                mutation
            })
        },
    )
}

/// Shared shape of the simple state-transition commands.
#[allow(clippy::too_many_arguments)]
fn transition(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
    attempt_id: AttemptId,
    resource_kind: ResourceKind,
    event: AssessmentEvent,
    extra: impl FnOnce(&mut pipeline::Env<'_>, &Loaded) -> Result<Option<String>, G9Error>,
) -> Result<CommandOutcome<Value>, G9Error> {
    let resource = ResourceRef::new(resource_kind, attempt_id.as_str());
    let scope = scope_of(meta);
    let facts_id = attempt_id.clone();
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        move |conn, _| {
            let a = load_attempt(conn, &scope, &facts_id)?;
            Ok(Facts::existing(
                rc(
                    a.data_domain,
                    a.classification,
                    a.state == AssessmentAttemptState::ApprovedFinal,
                ),
                a.revision,
            ))
        },
        move |env| {
            let loaded = load(env.conn, &scope, &attempt_id)?;
            let a = &loaded.attempt;
            let next = a
                .state
                .transition(event)
                .map_err(|_| illegal(a.state, event))?;
            let submission = extra(env, &loaded)?;
            let mut patch = a.patch();
            patch.state = next;
            if event == AssessmentEvent::Submit {
                patch.submitted_at = Some(env.now);
                patch.submission_sha256 = submission;
            }
            let revision =
                repo::update_attempt(env.conn, &attempt_id, a.revision, &patch, env.now)?;
            let view = reload_view(env.conn, &loaded, &env.meta.actor.id)?;
            Ok(Mutation::new(view, Some(revision)).with_event(EventSpec::learning(
                "assessment.attempt.transitioned",
                json!({ "attempt_id": attempt_id, "from": a.state, "to": next, "event": event }),
            )))
        },
    )
}

#[derive(Debug, Deserialize)]
struct SubmitRequest {
    attempt_id: String,
    evidence_refs: Vec<String>,
    notes: Option<String>,
}

/// `assessment.submit` (STARTED → SUBMITTED; links evidence, stores only a submission hash).
pub fn submit(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: SubmitRequest = decode(payload)?;
    let id: AttemptId = parse_id(&req.attempt_id, "attempt_id")?;
    let evidence: Vec<EvidenceId> = req
        .evidence_refs
        .iter()
        .map(|e| parse_id(e, "evidence_refs[]"))
        .collect::<Result<_, _>>()?;
    // app-5: the text was hashed into `submission_sha256` and otherwise lost. It is now stored as an
    // erasable note, so the attempt (an evidentiary row that is never erased) commits to the evidence
    // and to whether notes were handed in, never to their text: a digest of erasable text is the
    // text for anyone who can guess it (ADR-0017).
    let notes = req.notes.clone().filter(|n| !n.trim().is_empty());
    let digest =
        sha256_json(&json!({ "evidence_refs": req.evidence_refs, "has_notes": notes.is_some() }));
    if let Some(text) = &notes {
        if text.len() as u64 > MAX_NOTE_BYTES {
            return Err(G9Error::invalid_input(
                "assessment.submit.notes_too_large",
                format!("notes exceed {MAX_NOTE_BYTES} bytes"),
            ));
        }
    }
    transition(
        ctx,
        spec,
        meta,
        payload,
        id,
        ResourceKind::Attempt,
        AssessmentEvent::Submit,
        move |env, loaded| {
            for e in &evidence {
                require_domain_evidence(env.conn, &loaded.attempt, e)?;
            }
            for e in &evidence {
                repo::link_attempt_evidence(
                    env.conn,
                    &AttemptEvidenceRow {
                        attempt_id: loaded.attempt.id.clone(),
                        evidence_item_id: e.clone(),
                        role: AttemptEvidenceRole::Submission,
                        added_at: env.now,
                    },
                )?;
            }
            if let Some(text) = &notes {
                store_submission_notes(
                    env.conn,
                    &loaded.attempt,
                    text,
                    &env.meta.actor.id,
                    env.now,
                )?;
            }
            Ok(Some(digest))
        },
    )
}

/// The evidence item `id` as seen from the attempt: it exists and lives in the
/// attempt's domain. Anything else is `NOT_FOUND evidence.not_found`, so an item
/// of another domain is indistinguishable from a missing one (app-6: the
/// foreign key used to refuse a missing id with SQLite's text, and nothing
/// refused another domain's item).
fn require_domain_evidence(
    conn: &Connection,
    attempt: &AttemptRow,
    id: &EvidenceId,
) -> Result<EvidenceItemRow, G9Error> {
    domain_evidence(conn, attempt, id)?.ok_or_else(|| {
        G9Error::not_found("evidence.not_found", "evidence item not found")
            .with_details(json!({ "evidence_id": id }))
    })
}

/// The evidence item `id` when it exists in the attempt's domain.
fn domain_evidence(
    conn: &Connection,
    attempt: &AttemptRow,
    id: &EvidenceId,
) -> Result<Option<EvidenceItemRow>, G9Error> {
    Ok(
        evidence_repo::get_evidence_item(conn, &ReadScope::cross_domain(attempt.data_domain), id)?
            .filter(|item| item.data_domain == attempt.data_domain),
    )
}

/// Store the operator's submission notes as an erasable note, wrapped in an
/// evidence item and linked to the attempt as SUBMISSION evidence (app-5).
/// Runs inside the submitting transaction.
fn store_submission_notes(
    conn: &Connection,
    attempt: &AttemptRow,
    text: &str,
    actor: &ActorId,
    now: UtcMillis,
) -> Result<(), G9Error> {
    const TITLE: &str = "Submission notes";
    let note_id = NoteId::new_v7();
    let note = NoteRow {
        id: note_id.clone(),
        title: TITLE.to_string(),
        body_markdown: text.to_string(),
        body_sha256: String::new(),
        kind: NoteKind::Report,
        session_id: attempt.session_id.clone(),
        objective_id: None,
        source_kind: SourceKind::AssessmentSubmission,
        trust: TrustClass::Operator,
        data_domain: attempt.data_domain,
        classification: attempt.classification,
        erased: false,
        erased_at: None,
        erasure_request_id: None,
        revision: Revision::INITIAL,
        created_at: now,
        updated_at: now,
    };
    evidence_repo::insert_note(conn, &note, actor, &new_id("nv"))?;
    let content_sha256 = sha256_hex(text.as_bytes());
    let provenance = Provenance {
        source_kind: SourceKind::AssessmentSubmission,
        source_ref: format!("attempt:{}", attempt.id.as_str()),
        trust: TrustClass::Operator,
        content_sha256: Some(content_sha256.clone()),
        recorded_at: now,
        recorded_by: actor.clone(),
        version: None,
    };
    let item = EvidenceItem::new(
        EvidenceId::new_v7(),
        EvidenceKind::Note,
        note_id.as_str(),
        attempt.data_domain,
        attempt.classification,
        provenance,
        TITLE,
        now,
    )
    .map_err(|_| G9Error::internal_redacted("assessment.submit.notes_evidence"))?;
    let row = EvidenceItemRow::from_domain(&item, &content_sha256)?;
    evidence_repo::insert_evidence_item(conn, &row)?;
    repo::link_attempt_evidence(
        conn,
        &AttemptEvidenceRow {
            attempt_id: attempt.id.clone(),
            evidence_item_id: item.id.clone(),
            role: AttemptEvidenceRole::Submission,
            added_at: now,
        },
    )?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct AttemptRequest {
    attempt_id: String,
}

/// `assessment.review_evidence` (SUBMITTED/GRADED_PROVISIONAL → EVIDENCE_REVIEW).
pub fn review_evidence(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: AttemptRequest = decode(payload)?;
    let id: AttemptId = parse_id(&req.attempt_id, "attempt_id")?;
    transition(
        ctx,
        spec,
        meta,
        payload,
        id,
        ResourceKind::Attempt,
        AssessmentEvent::ReviewEvidence,
        |_, _| Ok(None),
    )
}

#[derive(Debug, Deserialize)]
struct CriterionInput {
    criterion_code: String,
    score: u8,
    source: GradeSource,
    #[serde(default)]
    evidence_refs: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GradeRequest {
    attempt_id: String,
    criteria: Vec<CriterionInput>,
}

/// `assessment.grade_provisional` (EVIDENCE_REVIEW → GRADED_PROVISIONAL).
pub fn grade_provisional(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: GradeRequest = decode(payload)?;
    let id: AttemptId = parse_id(&req.attempt_id, "attempt_id")?;
    transition(
        ctx,
        spec,
        meta,
        payload,
        id,
        ResourceKind::GradeDecision,
        AssessmentEvent::GradeProvisional,
        move |env, loaded| {
            let (rubric_row, _) = repo::rubric_for_version(env.conn, &loaded.version.id)?
                .ok_or_else(|| {
                    G9Error::precondition_failed(
                        "assessment.rubric_missing",
                        "assessment version has no rubric",
                    )
                })?;
            // Anchored level -> score through the rubric's explicit scale (ADR-0012).
            let scale = rubric(&loaded.criteria)?;
            let linked = repo::list_attempt_evidence(env.conn, &loaded.attempt.id)?;
            for input in &req.criteria {
                for reference in &input.evidence_refs {
                    let id: EvidenceId = parse_id(reference, "evidence_refs[]")?;
                    // app-8: only evidence submitted for this attempt, in its domain.
                    let submitted = linked.iter().any(|l| l.evidence_item_id == id)
                        && domain_evidence(env.conn, &loaded.attempt, &id)?.is_some();
                    if !submitted {
                        return Err(G9Error::invalid_input(
                            "grade.evidence_unknown",
                            "a criterion cites evidence that was not submitted for this attempt",
                        )
                        .with_details(json!({
                            "criterion_code": input.criterion_code,
                            "evidence_id": id,
                        })));
                    }
                }
                if !loaded
                    .criteria
                    .iter()
                    .any(|c| c.code == input.criterion_code)
                {
                    return Err(G9Error::invalid_input(
                        "grade.criterion_unknown",
                        "criterion is not part of this rubric",
                    )
                    .with_details(json!({ "criterion_code": input.criterion_code })));
                }
                if scale.score_for_level(input.score).is_err() {
                    return Err(G9Error::invalid_input(
                        "grade.level_out_of_range",
                        "criterion score is an anchored level 0..=4",
                    )
                    .with_details(json!({
                        "criterion_code": input.criterion_code,
                        "level": input.score,
                    })));
                }
            }
            let existing = repo::effective_criterion_results(env.conn, &loaded.attempt.id)?;
            for c in &loaded.criteria {
                let input = req
                    .criteria
                    .iter()
                    .find(|i| i.criterion_code == c.code)
                    .ok_or_else(|| {
                        G9Error::invalid_input(
                            "grade.criterion_missing",
                            "every rubric criterion needs a result",
                        )
                        .with_details(json!({ "criterion_code": c.code }))
                    })?;
                let evidence: Vec<EvidenceId> = input
                    .evidence_refs
                    .iter()
                    .map(|e| parse_id(e, "evidence_refs[]"))
                    .collect::<Result<_, _>>()?;
                let criterion_id = repo::criterion_id(env.conn, &rubric_row.id, &c.code)?
                    .ok_or_else(|| {
                        G9Error::data_corruption(
                            "assessment.criterion_missing",
                            "rubric criterion row missing",
                        )
                    })?;
                let supersedes = existing
                    .iter()
                    .find(|r| r.criterion_code == c.code && r.row.source == input.source)
                    .map(|r| r.row.id.clone());
                let score = scale.score_for_level(input.score)?;
                if input.source != GradeSource::ModelAdvisory {
                    // Provenance is checked by the domain constructor: the
                    // operator records HUMAN_REVIEWER results; a
                    // DETERMINISTIC_CHECKER result can only come from a System
                    // checker (→ UNAUTHORIZED, nothing is stored).
                    CriterionResult::new(
                        loaded.attempt.id.clone(),
                        c.code.clone(),
                        g9_assessment::Score::new(score)?,
                        input.source,
                        &env.meta.actor,
                        env.now,
                    )?;
                }
                repo::append_criterion_result(
                    env.conn,
                    &CriterionResultRow {
                        id: new_id("cr"),
                        attempt_id: loaded.attempt.id.clone(),
                        criterion_id,
                        score,
                        source: input.source,
                        evidence_refs: evidence,
                        rationale: None,
                        scored_by: env.meta.actor.id.clone(),
                        scored_at: env.now,
                        supersedes,
                        created_at: env.now,
                    },
                )?;
            }
            Ok(None)
        },
    )
}

/// The rehydrated domain attempt and whether its difficulty is the one the
/// version's author declared.
struct DomainAttempt {
    attempt: Attempt,
    /// `false` = the version declares no difficulty and `attempt` carries a
    /// placeholder that `finalize_decision` never reads. Such an attempt must
    /// never be used to derive an observation ([`derive_observations`] refuses).
    difficulty_declared: bool,
}

/// Exposure records of the attempt from the append-only `exposure_log`
/// (`Task` stays blind; a `Key` row is the answer key or reference solution and
/// compromises blindness). The log stores no rubric kind in the assessment
/// crate's vocabulary: `Rubric` rows are not mapped (ASSUMED: a displayed
/// rubric does not compromise a blind attempt; no command logs one today).
/// `ExposureRecord::actor` is not read by `Attempt::is_blind`; it is built from
/// the logged actor id with the kind implied by the audience (`Skill` and
/// `Export` audiences, which the log stores without a skill id, use `System`).
fn exposure_records(
    conn: &Connection,
    attempt: &AttemptId,
) -> Result<Vec<ExposureRecord>, G9Error> {
    let mut out = Vec::new();
    for row in repo::list_exposures_for_attempt(conn, attempt)? {
        let what = match row.content_kind {
            ExposureContentKind::Task => ExposureKind::Task,
            ExposureContentKind::Key => ExposureKind::AnswerKey,
            ExposureContentKind::Rubric => continue,
        };
        let actor = match row.exposed_to {
            ExposedTo::Operator => Actor::operator(row.actor_id),
            ExposedTo::Model => Actor::model(row.actor_id),
            ExposedTo::Skill | ExposedTo::Export => Actor::system(row.actor_id),
        };
        out.push(ExposureRecord {
            what,
            at: row.exposed_at,
            actor,
        });
    }
    Ok(out)
}

/// Whether the item was consumed before this attempt (LRN-13: an exam item is
/// consumed on its first exposure). [`exposure_records`] reads what the attempt's
/// own log rows say; this reads the exposure of the same version that no row of
/// this attempt carries:
///
/// * the task was shown to the operator at or before this attempt began, for
///   another attempt (a retake on a variant that was already shown; the same rule
///   `assessment.begin` applies when it flags `assessment.retake_same_variant` /
///   `assessment.retake_exposed_variant`);
/// * the answer key or a reference solution was shown (any audience) up to the
///   submission without an attempt attached, or for another attempt (a study
///   session, the review of an earlier attempt).
///
/// A rubric row consumes nothing (as in [`exposure_records`]). The result decides
/// the immutable `blind` flag of the evidence rows, so it errs towards `false`.
fn consumed_before_attempt(conn: &Connection, loaded: &Loaded) -> Result<bool, G9Error> {
    let attempt = &loaded.attempt;
    let began = attempt.started_at.unwrap_or(attempt.created_at);
    for row in repo::list_exposures(conn, &loaded.version.id)? {
        if row.attempt_id.as_ref() == Some(&attempt.id) {
            continue;
        }
        let consumed = match row.content_kind {
            ExposureContentKind::Task => {
                row.exposed_to == ExposedTo::Operator && row.exposed_at <= began
            }
            ExposureContentKind::Key => attempt.submitted_at.is_none_or(|s| row.exposed_at <= s),
            ExposureContentKind::Rubric => false,
        };
        if consumed {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Rehydrate the domain attempt for `learner` with the version's declared
/// difficulty and the attempt's recorded exposure. A version that declares none
/// gets a placeholder (the pass/fail rule never reads it) and
/// `difficulty_declared = false`; the difficulty is never guessed.
fn domain_attempt(
    conn: &Connection,
    loaded: &Loaded,
    learner: &Actor,
) -> Result<DomainAttempt, G9Error> {
    let (difficulty, difficulty_declared) = match loaded.version.difficulty {
        Some(declared) => (declared, true),
        None => (
            Difficulty::new(1).map_err(|_| G9Error::internal_redacted("assessment.difficulty"))?,
            false,
        ),
    };
    let attempt = loaded.attempt.to_attempt(
        &loaded.definition.id,
        loaded.version.version_number()?,
        &learner.id,
        loaded
            .definition
            .competency_id
            .clone()
            .into_iter()
            .collect(),
        difficulty,
        exposure_records(conn, &loaded.attempt.id)?,
    )?;
    Ok(DomainAttempt {
        attempt,
        difficulty_declared,
    })
}

/// What `approve_final` recorded for the competency model.
enum Derivation {
    /// This many observations (and as many evidence rows) were appended.
    Derived(usize),
    /// Nothing was derived; the reason is reported as a warning, never guessed.
    NotDerived(&'static str),
}

/// Derive the competency observations of the final `decision` and append them
/// with their evidence rows (same transaction as the decision). Authority: the
/// decision is an operator-approved final decision, so the provenance is
/// `TrustClass::Operator`; model output never reaches this path.
fn derive_observations(
    conn: &Connection,
    loaded: &Loaded,
    attempt: &DomainAttempt,
    decision: &GradeDecision,
    results_sha256: &str,
    reviewer: &Actor,
    now: UtcMillis,
) -> Result<Derivation, G9Error> {
    if !attempt.difficulty_declared {
        return Ok(Derivation::NotDerived(
            "the assessment version declares no difficulty",
        ));
    }
    if attempt.attempt.competency_ids().is_empty() {
        return Ok(Derivation::NotDerived(
            "the assessment is linked to no competency",
        ));
    }
    let provenance = g9_domain::Provenance {
        source_kind: g9_domain::SourceKind::AssessmentSubmission,
        source_ref: format!("grade_decision:{}", decision.id),
        trust: g9_domain::TrustClass::Operator,
        content_sha256: Some(results_sha256.to_string()),
        recorded_at: now,
        recorded_by: reviewer.id.clone(),
        version: Some(loaded.version.version_label.clone()),
    };
    let mut drafts = observations_from_decision(
        &attempt.attempt,
        &rubric(&loaded.criteria)?,
        decision,
        provenance,
    )?;
    if drafts.is_empty() {
        return Ok(Derivation::NotDerived(
            "the decision maps to no competency dimension",
        ));
    }
    // The draft's blindness knows only the attempt's own exposure rows; an item
    // that was consumed before the attempt began is not blind (LRN-13), and its
    // observations are therefore never validation evidence.
    if consumed_before_attempt(conn, loaded)? {
        for draft in &mut drafts {
            draft.blind = false;
            draft.validation_eligible = false;
        }
    }
    let environment = format!(
        "assessment:{}@{}",
        loaded.definition.id, loaded.version.version_label
    );
    for draft in &drafts {
        let id = ObservationId::parse(&new_id("obs"))
            .map_err(|_| G9Error::internal_redacted("assessment.observation_id"))?;
        competency_repo::append_observation(
            conn,
            &CompetencyObservation {
                id: id.clone(),
                competency_id: draft.competency_id.clone(),
                dimension: draft.dimension,
                source: ObservationSource::Attempt(draft.source_attempt.clone()),
                mode: draft.mode,
                difficulty: draft.difficulty,
                assistance: draft.assistance,
                score: draft.score.get(),
                environment: environment.clone(),
                reviewer: Some(draft.reviewer.clone()),
                provenance: draft.provenance.clone(),
                // g9-competency: 0 = certain, 1 = no information. No
                // measurement-error model exists for a rubric grade, so the
                // documented "certain" value is recorded; it is not a claim
                // that the grade is infallible (PROPOSED: an uncertainty model).
                uncertainty: Confidence::new(0.0)
                    .map_err(|_| G9Error::internal_redacted("assessment.uncertainty"))?,
                observed_at: draft.observed_at,
                supersedes: None,
            },
        )?;
        competency_repo::append_observation_evidence(
            conn,
            &ObservationEvidenceRow {
                observation_id: id,
                decision_id: draft.decision_id.clone(),
                reliability_class: stored_class(draft.reliability_class),
                validation_eligible: draft.validation_eligible,
                blind: draft.blind,
                created_at: now,
            },
        )?;
    }
    Ok(Derivation::Derived(drafts.len()))
}

/// What [`finalize_attempt`] wrote.
struct Finalized {
    decision: GradeDecision,
    row: GradeDecisionRow,
    revision: Revision,
    derivation: Derivation,
}

/// Everything `assessment.approve_final` writes, in the caller's transaction:
/// the immutable final `grade_decisions` row (with the class the crate
/// classified), the attempt moved to `APPROVED_FINAL`, and the competency
/// observations with their evidence rows ([`derive_observations`]). `learner`
/// and `reviewer` are the same operator in the MVP (attempts store no learner
/// column), so the class is `SELF_ADJUDICATED`; the parameters are separate
/// because the class is a function of both (ADR-0012) and a distinct reviewer
/// is exercised in this module's tests.
fn finalize_attempt(
    conn: &Connection,
    loaded: &Loaded,
    learner: &Actor,
    reviewer: &Actor,
    approval_id: Option<g9_domain::ApprovalId>,
    now: UtcMillis,
) -> Result<Finalized, G9Error> {
    let mut domain = domain_attempt(conn, loaded, learner)?;
    let (decision, results) = decide_final(conn, loaded, &domain.attempt, reviewer, now)?;
    domain.attempt.approve_final(reviewer, now)?;
    let results_sha = sha256_json(
        &serde_json::to_value(&results)
            .map_err(|_| G9Error::internal_redacted("assessment.results"))?,
    );
    let row = GradeDecisionRow::from_domain(&decision, approval_id, Some(results_sha.clone()))?
        .with_reliability_class(stored_class(decision.reliability_class()));
    repo::append_grade_decision(conn, &row)?;
    let a = &loaded.attempt;
    let patch = AttemptPatch {
        state: AssessmentAttemptState::ApprovedFinal,
        finalized_at: Some(now),
        ..a.patch()
    };
    let revision = repo::update_attempt(conn, &a.id, a.revision, &patch, now)?;
    // Same transaction: the competency model sees the grade or none of it.
    let derivation = derive_observations(
        conn,
        loaded,
        &domain,
        &decision,
        &results_sha,
        reviewer,
        now,
    )?;
    Ok(Finalized {
        decision,
        row,
        revision,
        derivation,
    })
}

/// What `assessment.approve_final` would make final for `attempt_id`, in words, for the native
/// confirmation of the approval (audit 2026-11-19, P2A-01). Read-only: the same derivation as the
/// command's own precondition check, nothing is stored. A provisional grade is not a stored
/// decision, so the storage layer cannot say it; this function says it from the criterion results.
pub(crate) fn final_grade_preview(
    conn: &Connection,
    scope: &ReadScope,
    attempt_id: &str,
    operator: &Actor,
    now: UtcMillis,
) -> String {
    let preview = || -> Option<String> {
        let id: AttemptId = attempt_id.parse().ok()?;
        let loaded = load(conn, scope, &id).ok()?;
        if loaded.attempt.state == AssessmentAttemptState::ApprovedFinal {
            return Some("this attempt already has a final grade".to_string());
        }
        let attempt = domain_attempt(conn, &loaded, operator).ok()?;
        let (decision, _) = decide_final(conn, &loaded, &attempt.attempt, operator, now).ok()?;
        let total = decision
            .breakdown
            .weighted_total
            .map_or_else(|| "no total".to_string(), |t| format!("{t:.1} of 100"));
        Some(format!(
            "the grade that would become final: {total}, {}",
            if decision.pass { "pass" } else { "fail" }
        ))
    };
    preview()
        .unwrap_or_else(|| "the grade that would become final could not be computed".to_string())
}

/// `assessment.approve_final` (approval-gated: `GradeFinalize`).
pub fn approve_final(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: AttemptRequest = decode(payload)?;
    let attempt_id: AttemptId = parse_id(&req.attempt_id, "attempt_id")?;
    let resource = ResourceRef::new(ResourceKind::GradeDecision, attempt_id.as_str());
    let scope = scope_of(meta);
    let facts_id = attempt_id.clone();
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        move |conn, svc| {
            let loaded = load(conn, &scope, &facts_id)?;
            let a = &loaded.attempt;
            let finalized = a.state == AssessmentAttemptState::ApprovedFinal;
            if !finalized {
                // Preconditions are checked before policy so that an impossible
                // finalization never produces an approval request.
                if a.state != AssessmentAttemptState::GradedProvisional {
                    return Err(illegal(a.state, AssessmentEvent::ApproveFinal));
                }
                let attempt = domain_attempt(conn, &loaded, &svc.operator)?;
                decide_final(conn, &loaded, &attempt.attempt, &svc.operator, svc.now())?;
            }
            Ok(Facts::existing(
                rc(a.data_domain, a.classification, finalized),
                a.revision,
            ))
        },
        move |env| {
            let loaded = load(env.conn, &scope, &attempt_id)?;
            let consumed = env.consumed.clone().ok_or_else(|| {
                G9Error::approval_required(
                    "assessment.finalize_requires_approval",
                    "finalization requires a consumed operator approval",
                )
            })?;
            // The backend-assigned operator is the reviewer and, in the MVP, the
            // learner: the crate classifies the decision SELF_ADJUDICATED.
            let reviewer = env.meta.actor.clone();
            let Finalized {
                decision,
                row,
                revision,
                derivation,
            } = finalize_attempt(
                env.conn,
                &loaded,
                &reviewer,
                &reviewer,
                Some(consumed.approval_id.clone()),
                env.now,
            )?;
            let view = reload_view(env.conn, &loaded, &reviewer.id)?;
            let gate_failures: Vec<&str> =
                decision.gate_failures().iter().map(|g| g.code()).collect();
            let observations_derived = match &derivation {
                Derivation::Derived(n) => *n,
                Derivation::NotDerived(_) => 0,
            };
            let mutation = Mutation::new(view, Some(revision)).with_event(EventSpec::learning(
                "assessment.attempt.finalized",
                json!({
                    "attempt_id": attempt_id,
                    "grade_decision_id": decision.id,
                    "passed": decision.pass,
                    "reliability_class": row.reliability_class,
                    "gate_failures": gate_failures,
                    "approval_id": consumed.approval_id,
                    "observations_derived": observations_derived,
                }),
            ));
            Ok(match derivation {
                Derivation::Derived(_) => mutation,
                Derivation::NotDerived(reason) => mutation.with_warning(format!(
                    "competency.observations_not_derived: {reason}; no competency observation was recorded from this decision and the difficulty is never guessed"
                )),
            })
        },
    )
}

#[cfg(test)]
mod tests;
