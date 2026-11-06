//! Assessment commands (M07; ARCHITECTURE.md §8; gates A04, A07).
//!
//! * The attempt moves only along `AssessmentAttemptState` (pure transition
//!   table); `APPROVED_FINAL` has no outgoing edge, and the policy invariant
//!   `invariant.immutable_finalized` plus the storage triggers refuse any later
//!   update.
//! * `assessment.grade_provisional` records anchored levels 0–4 per criterion
//!   (stored as `level × 25` on the crate's 0–100 scale); results are
//!   append-only, a re-grade supersedes.
//! * `assessment.approve_final` requires `Capability::GradeFinalize` (a
//!   consumed operator approval), calls `g9_assessment::finalize` with the
//!   backend-assigned operator as reviewer (`GradeSource::ModelAdvisory` can
//!   never finalize), appends the immutable `grade_decisions` row with a
//!   reliability class, and finalizes the attempt in the same transaction.
//!
//! Observation derivation from a final decision (`observations_from_decision`)
//! needs the version difficulty, which `assessment_versions` does not persist
//! (wave-1 gap): it is PROPOSED and reported as a warning, never guessed.

use super::{decode, new_id, parse_id, rc, scope_of, tail, to_value};
use crate::context::AppContext;
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation};
use g9_assessment::{finalize, score_breakdown, Attempt, CriterionResult, Rubric};
use g9_domain::hashing::sha256_json;
use g9_domain::state::AssessmentEvent;
use g9_domain::{
    Actor, AssessmentAttemptState, AssessmentId, AttemptId, Classification, CommandMeta,
    CommandOutcome, Difficulty, EvidenceId, G9Error, GradeDecisionId, GradeSource, ResourceKind,
    ResourceRef, Revision, UtcMillis,
};
use g9_storage::repos::assessment::{
    self as repo, AssessmentDefinitionRow, AssessmentVersionRow, AttemptEvidenceRole,
    AttemptEvidenceRow, AttemptPatch, AttemptRow, CriterionResultRow, ExposedTo,
    ExposureContentKind, ExposureLogRow, GradeDecisionRow, ReliabilityClass, RetakeLinkRow,
    RubricCriterionRow,
};
use g9_storage::repos::ReadScope;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{json, Value};

/// Anchored level 0–4 ↔ crate score 0–100 (ADR-0012 anchored levels).
const LEVEL_STEP: u8 = 25;

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

fn reliability(results: &[CriterionResult]) -> ReliabilityClass {
    if !results.is_empty()
        && results
            .iter()
            .filter(|r| r.source != GradeSource::ModelAdvisory)
            .all(|r| r.source == GradeSource::DeterministicChecker)
    {
        ReliabilityClass::Deterministic
    } else {
        // The operator is the learner: their own grading is self-adjudicated (LRN-11).
        ReliabilityClass::SelfAdjudicated
    }
}

fn grade_view(conn: &Connection, loaded: &Loaded) -> Result<Value, G9Error> {
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
                "score": r.row.score / LEVEL_STEP,
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
        None => (
            breakdown
                .weighted_total
                .map_or(0.0, |t| (t * 100.0).round() / 100.0),
            breakdown.pass_recommendation.unwrap_or(false),
            reliability(&domain),
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

fn attempt_view(conn: &Connection, loaded: &Loaded) -> Result<Value, G9Error> {
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
        "grade": grade_view(conn, loaded)?,
        "revision": a.revision,
    }))
}

fn reload_view(conn: &Connection, loaded: &Loaded) -> Result<Value, G9Error> {
    let fresh = load(
        conn,
        &ReadScope::cross_domain(loaded.attempt.data_domain),
        &loaded.attempt.id,
    )?;
    attempt_view(conn, &fresh)
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
                        attempt_view(env.conn, &loaded)?,
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
    let attempt_id = AttemptId::new_v7();
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
            let version = versions.last().cloned().ok_or_else(|| {
                G9Error::precondition_failed("assessment.no_version", "assessment has no version")
            })?;
            for v in &versions {
                if let Some(open) = repo::list_attempts_for_version(
                    env.conn,
                    &ReadScope::cross_domain(def.data_domain),
                    &v.id,
                )?
                .into_iter()
                .find(|a| !a.state.is_terminal())
                {
                    return Err(G9Error::conflict(
                        "attempt.already_open",
                        "an attempt for this assessment is already in progress",
                    )
                    .with_details(json!({ "attempt_id": open.id })));
                }
            }
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
                        reason: "operator retake".to_string(),
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
            Ok(Mutation::new(attempt_view(env.conn, &loaded)?, Some(revision))
                .with_event(EventSpec::learning("assessment.attempt.started", json!({ "attempt_id": attempt_id, "assessment_id": def.id, "retake_of": retake_of }))))
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
            let view = reload_view(env.conn, &loaded)?;
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
    let digest = sha256_json(&json!({ "evidence_refs": req.evidence_refs, "notes": req.notes }));
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
            Ok(Some(digest))
        },
    )
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
            for input in &req.criteria {
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
                repo::append_criterion_result(
                    env.conn,
                    &CriterionResultRow {
                        id: new_id("cr"),
                        attempt_id: loaded.attempt.id.clone(),
                        criterion_id,
                        score: input.score.saturating_mul(LEVEL_STEP),
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

/// Rehydrate the domain attempt. `difficulty` is not persisted (wave-1 gap);
/// the placeholder is never read by `finalize` and no observation is derived.
fn domain_attempt(loaded: &Loaded, learner: &Actor) -> Result<Attempt, G9Error> {
    let placeholder =
        Difficulty::new(1).map_err(|_| G9Error::internal_redacted("assessment.difficulty"))?;
    Ok(loaded.attempt.to_attempt(
        &loaded.definition.id,
        loaded.version.version_number()?,
        &learner.id,
        loaded
            .definition
            .competency_id
            .clone()
            .into_iter()
            .collect(),
        placeholder,
        Vec::new(),
    )?)
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
                let attempt = domain_attempt(&loaded, &svc.operator)?;
                finalize(
                    GradeDecisionId::new_v7(),
                    &attempt,
                    &rubric(&loaded.criteria)?,
                    &results(conn, &a.id)?,
                    &svc.operator,
                    svc.now(),
                )?;
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
            let reviewer = env.meta.actor.clone();
            let mut attempt = domain_attempt(&loaded, &reviewer)?;
            let rubric = rubric(&loaded.criteria)?;
            let results = results(env.conn, &attempt_id)?;
            let decision = finalize(
                GradeDecisionId::new_v7(),
                &attempt,
                &rubric,
                &results,
                &reviewer,
                env.now,
            )?;
            attempt.approve_final(&reviewer, env.now)?;
            let results_sha = sha256_json(
                &serde_json::to_value(&results)
                    .map_err(|_| G9Error::internal_redacted("assessment.results"))?,
            );
            let row = GradeDecisionRow::from_domain(
                &decision,
                Some(consumed.approval_id.clone()),
                Some(results_sha),
            )?
            .with_reliability_class(reliability(&results));
            repo::append_grade_decision(env.conn, &row)?;
            let a = &loaded.attempt;
            let patch = AttemptPatch {
                state: AssessmentAttemptState::ApprovedFinal,
                finalized_at: Some(env.now),
                ..a.patch()
            };
            let revision =
                repo::update_attempt(env.conn, &attempt_id, a.revision, &patch, env.now)?;
            let view = reload_view(env.conn, &loaded)?;
            Ok(Mutation::new(view, Some(revision))
                .with_event(EventSpec::learning(
                    "assessment.attempt.finalized",
                    json!({
                        "attempt_id": attempt_id,
                        "grade_decision_id": decision.id,
                        "passed": decision.pass,
                        "reliability_class": row.reliability_class,
                        "approval_id": consumed.approval_id,
                    }),
                ))
                .with_warning("competency.observations_not_derived: assessment versions store no difficulty (wave-1 gap); observation derivation is PROPOSED"))
        },
    )
}
