//! `retention.due` and `interventions.list` (read-only; M08, M12).
//!
//! The due queue is planned by `g9_retention::plan_reviews` under the daily
//! review budget (default 15 minutes, ARCHITECTURE.md §9); every due item is
//! read (no row limit) and items that do not fit are returned as `overflow`,
//! never dropped; an item whose scheduler state cannot be read is reported in
//! a warning. "Overdue" means due before local midnight (00:00:00) of `as_of`
//! in the operator zone. The per-item time estimate
//! is a scheduling parameter (ASSUMED 3 minutes, [`REVIEW_ITEM_ESTIMATE_MS`]),
//! not a measurement. Interventions are listed as stored; model-originated
//! proposals keep their `MODEL_OUTPUT` provenance.
//!
//! Reported gap: the v1.0 response vocabulary has no `PROCEDURAL` kind, so a
//! memory-scheduled `PROCEDURAL` review item is labelled `PRACTICAL_TRANSFER`
//! (it is NOT a `practical_transfer_checks` row); every such outcome carries
//! the warning `retention.due.procedural_label` until the response schema and
//! the UI add `PROCEDURAL` (PROPOSED, outside `g9-app`).

use super::{decode, rc, scope_of};
use crate::context::AppContext;
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, Facts, Mutation};
use g9_domain::{
    Classification, CommandMeta, CommandOutcome, DurationMs, G9Error, InterventionState,
    ResourceKind, ResourceRef, UtcMillis,
};
use g9_retention::plan_reviews;
use g9_storage::repos::retention::{self as repo, ReviewItemKind};
use serde::Deserialize;
use serde_json::{json, Value};

/// Default daily review budget (ARCHITECTURE.md §9: 15 minutes).
pub const DEFAULT_REVIEW_BUDGET_MS: i64 = 15 * 60_000;
/// Planning estimate per review item (ASSUMED scheduling parameter).
pub const REVIEW_ITEM_ESTIMATE_MS: i64 = 3 * 60_000;

#[derive(Debug, Deserialize)]
struct DueRequest {
    as_of: Option<UtcMillis>,
    budget_ms: Option<i64>,
}

/// `retention.due`.
pub fn due(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: DueRequest = decode(payload)?;
    let resource = ResourceRef::new(ResourceKind::ReviewItem, "due");
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
                Classification::Internal,
                false,
            )))
        },
        |env| {
            let as_of = req.as_of.unwrap_or(env.now);
            let budget = DurationMs::new(req.budget_ms.unwrap_or(DEFAULT_REVIEW_BUDGET_MS))
                .map_err(|_| {
                    G9Error::invalid_input(
                        "retention.budget_invalid",
                        "budget must be non-negative",
                    )
                })?;
            let estimate = DurationMs::new(REVIEW_ITEM_ESTIMATE_MS)
                .map_err(|_| G9Error::internal_redacted("retention.estimate"))?;
            // Every due item (no row limit): overflow is reported, never dropped.
            let items = repo::list_due_items(env.conn, &scope, as_of, u32::MAX)?;
            let mut states = Vec::new();
            let mut stateless = Vec::new();
            for item in &items {
                match repo::item_state(env.conn, &item.id)? {
                    Some(s) => states.push(s),
                    None => stateless.push(item.id.as_str().to_string()),
                }
            }
            let plan = plan_reviews(&states, budget, estimate, as_of)
                .map_err(|e| G9Error::invalid_input("retention.plan_invalid", e.to_string()))?;
            let start_of_day = g9_calendar::to_civil(as_of, &env.svc.config.zone)
                .ok()
                .and_then(|c| {
                    let midnight = g9_domain::CivilDateTime {
                        hour: 0,
                        minute: 0,
                        second: 0,
                        ..c
                    };
                    g9_calendar::resolve_local(&midnight, &env.svc.config.zone).ok()
                })
                .map_or(as_of, |r| r.instant);
            let view = |id: &g9_domain::ReviewItemId| -> Option<(bool, Value)> {
                let item = items.iter().find(|i| &i.id == id)?;
                Some((item.due_at < start_of_day, review_item_view(item)))
            };
            let mut due = Vec::new();
            let mut overdue = Vec::new();
            for id in &plan.scheduled {
                if let Some((late, v)) = view(id) {
                    if late {
                        overdue.push(v)
                    } else {
                        due.push(v)
                    }
                }
            }
            let overflow: Vec<Value> = plan
                .deferred_overflow
                .iter()
                .filter_map(|id| view(id).map(|v| v.1))
                .collect();
            let procedural = items
                .iter()
                .filter(|i| i.kind == ReviewItemKind::Procedural)
                .count();
            let mut mutation = Mutation::new(
                json!({ "as_of": as_of, "budget_ms": budget, "due": due, "overdue": overdue, "overflow": overflow }),
                None,
            );
            if procedural > 0 {
                mutation = mutation.with_warning(format!(
                    "retention.due.procedural_label: {procedural} PROCEDURAL review item(s) are memory-scheduled recall items labelled PRACTICAL_TRANSFER because the v1.0 response has no PROCEDURAL kind; they are not practical transfer checks"
                ));
            }
            if !stateless.is_empty() {
                mutation = mutation.with_warning(format!(
                    "retention.due.item_state_missing: no scheduler state for {}; not planned",
                    stateless.join(", ")
                ));
            }
            Ok(mutation)
        },
    )
}

#[derive(Debug, Deserialize)]
struct InterventionsRequest {
    state: Option<InterventionState>,
}

/// `interventions.list`.
pub fn interventions(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: InterventionsRequest = decode(payload)?;
    let resource = ResourceRef::new(ResourceKind::Intervention, "interventions");
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
                Classification::Internal,
                false,
            )))
        },
        |env| {
            let interventions: Vec<Value> = repo::list_interventions(env.conn, &scope, req.state)?
                .into_iter()
                .map(|i| {
                    json!({
                        "intervention_id": i.id,
                        "state": i.state,
                        "hypothesis_id": i.hypothesis_id,
                        "statement": i.description,
                        "steps": i.steps,
                        "time_budget_ms": i.time_budget,
                        "acceptance": i.acceptance,
                        "provenance": i.origin_trust,
                        "created_at": i.created_at,
                        "revision": i.revision,
                    })
                })
                .collect();
            Ok(Mutation::new(
                json!({ "interventions": interventions }),
                None,
            ))
        },
    )
}

/// The item view `retention.due` renders; `retention.create_item` answers it too.
fn review_item_view(item: &repo::ReviewItemRow) -> Value {
    let kind = match item.kind {
        ReviewItemKind::Declarative => "DECLARATIVE",
        // The UI vocabulary has no PROCEDURAL kind (reported gap).
        ReviewItemKind::Procedural => "PRACTICAL_TRANSFER",
    };
    json!({
        "review_item_id": item.id,
        "competency_id": item.competency_id,
        "objective_id": item.objective_id.as_deref().map(super::tail),
        "prompt": item.prompt,
        "kind": kind,
        "due_at": item.due_at,
        "last_success_at": item.last_success_at,
        "offset_days": item.interval_days,
        "estimated_ms": REVIEW_ITEM_ESTIMATE_MS,
    })
}

const DAY_MS: i64 = 86_400_000;

#[derive(Debug, Deserialize)]
struct CreateItemRequest {
    prompt: String,
    answer: Option<String>,
    kind: Option<String>,
    competency_id: Option<String>,
}

/// `retention.create_item`: a review item the operator writes (prompt and an
/// optional answer, kept as an erasable note). Baseline schedule: first review
/// one day later.
pub fn create_item(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: CreateItemRequest = decode(payload)?;
    let id: g9_domain::ReviewItemId = super::parse_id(
        &super::creation_id(ctx, spec, meta, || super::new_id("ri"))?,
        "review_item_id",
    )?;
    let resource = ResourceRef::new(ResourceKind::ReviewItem, id.as_str());
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
                g9_domain::DataDomain::Learning,
                Classification::Internal,
                false,
            )))
        },
        |env| {
            let kind = match req.kind.as_deref() {
                None | Some("DECLARATIVE") => ReviewItemKind::Declarative,
                Some("PROCEDURAL") => ReviewItemKind::Procedural,
                Some(_) => {
                    return Err(G9Error::invalid_input(
                        "retention.item_kind_invalid",
                        "a review item is DECLARATIVE or PROCEDURAL",
                    ))
                }
            };
            let competency_id = match &req.competency_id {
                None => None,
                Some(raw) => {
                    let cid: g9_domain::CompetencyId = super::parse_id(raw, "competency_id")?;
                    g9_storage::repos::competency::get_competency(
                        env.conn,
                        &g9_storage::repos::ReadScope::cross_domain(
                            g9_domain::DataDomain::Learning,
                        ),
                        &cid,
                    )?
                    .ok_or_else(|| {
                        G9Error::not_found("competency.not_found", "competency not found")
                    })?;
                    Some(cid)
                }
            };
            let answer_ref = match req.answer.as_deref().map(str::trim) {
                Some(answer) if !answer.is_empty() => {
                    let note_id = g9_domain::NoteId::new_v7();
                    let note = g9_storage::repos::evidence::NoteRow {
                        id: note_id.clone(),
                        title: "Review answer".to_string(),
                        body_markdown: answer.to_string(),
                        body_sha256: String::new(),
                        kind: g9_storage::repos::evidence::NoteKind::Other,
                        session_id: None,
                        objective_id: None,
                        source_kind: g9_domain::SourceKind::OperatorInput,
                        trust: g9_domain::TrustClass::Operator,
                        data_domain: g9_domain::DataDomain::Learning,
                        classification: Classification::Internal,
                        erased: false,
                        erased_at: None,
                        erasure_request_id: None,
                        revision: g9_domain::Revision::INITIAL,
                        created_at: env.now,
                        updated_at: env.now,
                    };
                    g9_storage::repos::evidence::insert_note(
                        env.conn,
                        &note,
                        &env.meta.actor.id,
                        &super::new_id("nv"),
                    )?;
                    Some(format!("note:{}", note_id.as_str()))
                }
                _ => None,
            };
            let row = repo::ReviewItemRow {
                id: id.clone(),
                kind,
                competency_id,
                objective_id: None,
                prompt: req.prompt.trim().to_string(),
                answer_ref,
                scheduler: repo::SchedulerKind::Baseline,
                due_at: UtcMillis::from_millis(env.now.as_millis() + DAY_MS),
                last_success_at: None,
                stage: 0,
                interval_days: 1,
                fsrs_state: None,
                state: repo::ReviewItemState::Active,
                data_domain: g9_domain::DataDomain::Learning,
                classification: Classification::Internal,
                revision: g9_domain::Revision::INITIAL,
                created_at: env.now,
                updated_at: env.now,
            };
            let revision = repo::insert_review_item(env.conn, &row)?;
            Ok(
                Mutation::new(review_item_view(&row), Some(revision)).with_event(
                    crate::pipeline::EventSpec::learning(
                        "retention.item_created",
                        json!({ "review_item_id": id }),
                    ),
                ),
            )
        },
    )
}

#[derive(Debug, Deserialize)]
struct RecordReviewRequest {
    review_item_id: String,
    outcome: String,
    response_ms: Option<i64>,
    rating: Option<u8>,
    session_id: Option<String>,
}

/// `retention.record_review`: append the attempt and advance the item on the
/// baseline schedule (success: next stage; failure: back to the first stage;
/// skipped: recorded, schedule unchanged), in one transaction.
pub fn record_review(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: RecordReviewRequest = decode(payload)?;
    let id: g9_domain::ReviewItemId = super::parse_id(&req.review_item_id, "review_item_id")?;
    let resource = ResourceRef::new(ResourceKind::ReviewItem, id.as_str());
    let facts_id = id.clone();
    let load = |conn: &rusqlite::Connection, id: &g9_domain::ReviewItemId| {
        repo::get_review_item(
            conn,
            &g9_storage::repos::ReadScope::cross_domain(g9_domain::DataDomain::Learning),
            id,
        )?
        .ok_or_else(|| G9Error::not_found("retention.item_not_found", "review item not found"))
    };
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        move |conn, _| {
            let row = load(conn, &facts_id)?;
            Ok(Facts::existing(
                rc(row.data_domain, row.classification, false),
                row.revision,
            ))
        },
        |env| {
            let row = load(env.conn, &id)?;
            if row.state != repo::ReviewItemState::Active {
                return Err(G9Error::precondition_failed(
                    "retention.item_not_active",
                    "the review item is suspended or retired",
                ));
            }
            if row.scheduler != repo::SchedulerKind::Baseline {
                return Err(G9Error::precondition_failed(
                    "retention.scheduler_not_wired",
                    "only baseline-scheduled items are recorded",
                ));
            }
            let outcome = match req.outcome.as_str() {
                "SUCCESS" => repo::ReviewAttemptOutcome::Success,
                "FAILURE" => repo::ReviewAttemptOutcome::Failure,
                "SKIPPED" => repo::ReviewAttemptOutcome::Skipped,
                _ => {
                    return Err(G9Error::invalid_input(
                        "retention.outcome_invalid",
                        "the outcome is SUCCESS, FAILURE or SKIPPED",
                    ))
                }
            };
            let session_id = req
                .session_id
                .as_deref()
                .map(|s| super::parse_id::<g9_domain::SessionId>(s, "session_id"))
                .transpose()?;
            let response_ms = req
                .response_ms
                .map(|ms| {
                    DurationMs::new(ms).map_err(|_| {
                        G9Error::invalid_input(
                            "retention.response_invalid",
                            "response_ms is negative",
                        )
                    })
                })
                .transpose()?;
            repo::append_review_attempt(
                env.conn,
                &repo::ReviewAttemptRow {
                    id: super::new_id("ra"),
                    review_item_id: id.clone(),
                    session_id,
                    attempted_at: env.now,
                    outcome,
                    response_ms,
                    confidence: None,
                    rating: req.rating,
                    created_at: env.now,
                },
            )?;
            let (due_at, stage, interval_days, revision) = match outcome {
                repo::ReviewAttemptOutcome::Skipped => {
                    (row.due_at, row.stage, row.interval_days, row.revision)
                }
                _ => {
                    let success = outcome == repo::ReviewAttemptOutcome::Success;
                    let state = g9_retention::ItemState {
                        item_id: id.clone(),
                        stage_index: row.stage as usize,
                        last_reviewed_at: row.last_success_at,
                        due_at: row.due_at,
                        lapses: 0,
                        successes: 0,
                        scheduler: g9_retention::BaselineSchedule::NAME.to_string(),
                        engine_state: None,
                    };
                    let next = g9_retention::BaselineSchedule::default()
                        .next_after(
                            &state,
                            if success {
                                g9_retention::ReviewOutcome::Success
                            } else {
                                g9_retention::ReviewOutcome::Fail
                            },
                            env.now,
                        )
                        .map_err(|e| {
                            G9Error::invalid_input("retention.schedule_invalid", e.to_string())
                        })?;
                    let interval_days =
                        u32::try_from((next.due_at.as_millis() - env.now.as_millis()) / DAY_MS)
                            .unwrap_or(0);
                    let stage = u32::try_from(next.stage_index).unwrap_or(row.stage);
                    let revision = repo::update_schedule(
                        env.conn,
                        &id,
                        row.revision,
                        &repo::SchedulePatch {
                            due_at: next.due_at,
                            last_success_at: if success {
                                Some(env.now)
                            } else {
                                row.last_success_at
                            },
                            stage,
                            interval_days,
                            fsrs_state: None,
                        },
                        env.now,
                    )?;
                    (next.due_at, stage, interval_days, revision)
                }
            };
            Ok(Mutation::new(
                json!({
                    "review_item_id": id,
                    "outcome": req.outcome,
                    "due_at": due_at,
                    "stage": stage,
                    "offset_days": interval_days,
                }),
                Some(revision),
            )
            .with_event(crate::pipeline::EventSpec::learning(
                "retention.review_recorded",
                json!({ "review_item_id": id, "outcome": req.outcome }),
            )))
        },
    )
}
