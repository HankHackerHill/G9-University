//! Calendar commands (M03; ARCHITECTURE.md §7; gate A02).
//!
//! * `calendar.list` returns resolved occurrences as stored (`UtcMillis`), with
//!   the DST adjustment flag.
//! * `calendar.propose` runs the deterministic `g9_calendar::propose_schedule`
//!   and stores the proposal (`SystemDeterministic` provenance); `Infeasible`
//!   is a first-class result. Implemented intent: `ADD_REVIEWS` (one review
//!   block sized by the due queue). `FILL_CURRICULUM` and `RESCHEDULE` are
//!   PROPOSED and return `PRECONDITION_FAILED calendar.<intent>.not_implemented`.
//! * `calendar.apply` requires `Capability::ScheduleApply` (a consumed operator
//!   approval), refuses expired or infeasible proposals, creates the events and
//!   records the immutable schedule decision.

use super::{decode, new_id, parse_id, rc, scope_of, tail, to_value};
use crate::commands::retention::REVIEW_ITEM_ESTIMATE_MS;
use crate::context::AppContext;
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation};
use g9_calendar::{
    propose_schedule, Adjustment, EventKind, Feasibility, PriorityClass, ResolvedEvent,
    ScheduleTask,
};
use g9_domain::{
    CalendarEventId, Classification, CommandMeta, CommandOutcome, DataDomain, G9Error,
    ResourceKind, ResourceRef, Revision, ScheduleProposalId, SourceKind, TrustClass, UtcMillis,
};
use g9_policy::DEFAULT_APPROVAL_TTL;
use g9_storage::repos::calendar::{
    self as repo, CalendarEventKind, CalendarEventRow, DstAdjustment, EventSource, ProposalState,
    ScheduleDecisionKind, ScheduleDecisionRow, ScheduleProposalRow,
};
use g9_storage::repos::retention as retention_repo;
use serde::Deserialize;
use serde_json::{json, Value};

const REVIEW_TASK_PREFIX: &str = "review-queue";

fn kind_label(k: CalendarEventKind) -> &'static str {
    match k {
        CalendarEventKind::StudyBlock => "STUDY_BLOCK",
        CalendarEventKind::Assessment => "ASSESSMENT",
        CalendarEventKind::Review => "REVIEW",
        CalendarEventKind::ProtectedRest => "PROTECTED_REST",
        // `Appointment` and `Other` share the UI's APPOINTMENT label (UI enum has no OTHER).
        _ => "APPOINTMENT",
    }
}

fn event_view(e: &CalendarEventRow) -> Value {
    let provenance = match e.source {
        EventSource::Operator => TrustClass::Operator,
        EventSource::Scheduler => TrustClass::SystemDeterministic,
        EventSource::Import => TrustClass::ImportedDocument,
    };
    json!({
        "event_id": e.id,
        "title": e.title,
        "kind": kind_label(e.kind),
        "start": e.start_utc,
        "end": e.end_utc,
        "all_day": e.all_day,
        "immutable": e.immutable,
        "day_id": e.training_day_id.as_deref().map(tail),
        // Block codes are resolved on the daily plan, not on calendar rows.
        "block": Value::Null,
        "session_id": Value::Null,
        "adjusted": e.dst_adjustment.map(|a| match a { DstAdjustment::Gap => "GAP", DstAdjustment::Overlap => "OVERLAP" }),
        "provenance": provenance,
    })
}

#[derive(Debug, Deserialize)]
struct ListRequest {
    from: UtcMillis,
    to: UtcMillis,
    zone: Option<String>,
}

/// `calendar.list` (read-only).
pub fn list(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: ListRequest = decode(payload)?;
    let resource = ResourceRef::new(ResourceKind::CalendarEvent, "events");
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
            if req.to < req.from {
                return Err(G9Error::invalid_input(
                    "range.inverted",
                    "'to' must not precede 'from'",
                ));
            }
            let zone = match &req.zone {
                Some(z) => g9_domain::IanaZone::parse(z)
                    .map_err(|_| G9Error::invalid_input("zone.invalid", "unknown IANA zone"))?,
                None => env.svc.config.zone.clone(),
            };
            let events: Vec<Value> = repo::list_events_between(env.conn, &scope, req.from, req.to)?
                .iter()
                .map(event_view)
                .collect();
            Ok(Mutation::new(
                json!({ "zone": zone.as_str(), "events": events }),
                None,
            ))
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Intent {
    FillCurriculum,
    AddReviews,
    Reschedule,
}

#[derive(Debug, Deserialize)]
struct ProposeRequest {
    from: UtcMillis,
    to: UtcMillis,
    intent: Intent,
}

fn resolved_kind(k: CalendarEventKind) -> EventKind {
    match k {
        CalendarEventKind::StudyBlock => EventKind::StudyBlock,
        CalendarEventKind::Assessment => EventKind::Assessment,
        CalendarEventKind::Review => EventKind::Review,
        CalendarEventKind::ProtectedRest => EventKind::ProtectedRest,
        _ => EventKind::Appointment,
    }
}

fn proposal_view(row: &ScheduleProposalRow) -> Result<Value, G9Error> {
    let violated: Vec<Value> = match &row.proposal.feasibility {
        Feasibility::Feasible => Vec::new(),
        Feasibility::Infeasible { violations } => violations
            .iter()
            .map(|v| {
                json!({
                    "code": serde_json::to_value(v.constraint).ok().and_then(|c| c.as_str().map(str::to_string)).unwrap_or_else(|| "CONSTRAINT".into()),
                    "message": v.detail,
                    "event_id": CalendarEventId::parse(&v.subject).ok(),
                })
            })
            .collect(),
    };
    let mut violated = violated;
    for u in &row.proposal.unplaced {
        violated.push(json!({ "code": "UNPLACED", "message": format!("task {} could not be placed", u.task_id), "event_id": Value::Null }));
    }
    let changes: Vec<Value> = row
        .proposal
        .placements
        .iter()
        .map(|p| {
            json!({
                "op": "CREATE",
                "event": {
                    "event_id": format!("{}:{}", row.id.as_str(), p.task_id),
                    "title": "Review queue",
                    "kind": "REVIEW",
                    "start": p.start,
                    "end": p.end,
                    "all_day": false,
                    "immutable": false,
                    "day_id": Value::Null,
                    "block": Value::Null,
                    "session_id": Value::Null,
                    "adjusted": Value::Null,
                    "provenance": "SYSTEM_DETERMINISTIC",
                },
            })
        })
        .collect();
    to_value(&json!({
        "proposal_id": row.id,
        "feasible": violated.is_empty(),
        "violated_constraints": violated,
        "changes": changes,
        "parameters_sha256": row.provenance.content_sha256.clone().unwrap_or_default(),
        "created_at": row.proposed_at,
        "expires_at": row.proposed_at.checked_add(DEFAULT_APPROVAL_TTL).unwrap_or(row.proposed_at),
        "revision": row.revision,
    }))
}

/// `calendar.propose` (deterministic proposal; never applies state).
pub fn propose(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: ProposeRequest = decode(payload)?;
    let id = ScheduleProposalId::new_v7();
    let resource = ResourceRef::new(ResourceKind::ScheduleProposal, id.as_str());
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
            match req.intent {
                Intent::AddReviews => {}
                Intent::FillCurriculum => {
                    return Err(G9Error::precondition_failed(
                        "calendar.fill_curriculum.not_implemented",
                        "curriculum fill proposals are PROPOSED (stage 03)",
                    ))
                }
                Intent::Reschedule => {
                    return Err(G9Error::precondition_failed(
                        "calendar.reschedule.not_implemented",
                        "rescheduling proposals are PROPOSED (stage 03)",
                    ))
                }
            }
            if req.to <= req.from {
                return Err(G9Error::invalid_input(
                    "range.inverted",
                    "'to' must follow 'from'",
                ));
            }
            Ok(Facts::new_record(rc(
                DataDomain::Learning,
                Classification::Internal,
                false,
            )))
        },
        |env| {
            let due = retention_repo::list_due_items(env.conn, &scope, env.now, 1_000)?;
            let minutes = u32::try_from(
                (i64::try_from(due.len()).unwrap_or(i64::MAX) * REVIEW_ITEM_ESTIMATE_MS) / 60_000,
            )
            .unwrap_or(u32::MAX);
            let tasks = if minutes == 0 {
                Vec::new()
            } else {
                vec![ScheduleTask {
                    id: format!("{REVIEW_TASK_PREFIX}-{}", due.len()),
                    title: format!("Review queue ({} items)", due.len()),
                    minutes,
                    priority_class: PriorityClass::DueReview,
                    earliest: Some(req.from),
                    deadline: Some(req.to),
                    prerequisites: Vec::new(),
                }]
            };
            let fixed: Vec<ResolvedEvent> =
                repo::list_events_between(env.conn, &scope, req.from, req.to)?
                    .iter()
                    .map(|e| ResolvedEvent {
                        event_id: e.id.clone(),
                        occurrence_seq: 0,
                        start: e.start_utc,
                        end: e.end_utc,
                        kind: resolved_kind(e.kind),
                        immutable: e.immutable,
                        all_day: e.all_day,
                        adjustment: Adjustment::None,
                    })
                    .collect();
            let proposal = propose_schedule(&tasks, &[(req.from, req.to)], &fixed, &[])?;
            let parameters = g9_domain::hashing::sha256_json(&json!({
                "from": req.from, "to": req.to, "intent": "ADD_REVIEWS", "proposal": proposal,
            }));
            let row = ScheduleProposalRow {
                id: id.clone(),
                proposed_by: env.meta.actor.id.clone(),
                proposed_at: env.now,
                range_start_utc: req.from,
                range_end_utc: req.to,
                proposal,
                state: ProposalState::Proposed,
                trust: TrustClass::SystemDeterministic,
                provenance: g9_domain::Provenance {
                    source_kind: SourceKind::SystemGenerated,
                    source_ref: "g9_calendar::propose_schedule".to_string(),
                    trust: TrustClass::SystemDeterministic,
                    content_sha256: Some(parameters),
                    recorded_at: env.now,
                    recorded_by: env.meta.actor.id.clone(),
                    version: None,
                },
                revision: Revision::INITIAL,
                created_at: env.now,
                updated_at: env.now,
            };
            let revision = repo::insert_proposal(env.conn, &row)?;
            let view = proposal_view(&row)?;
            Ok(
                Mutation::new(view, Some(revision)).with_event(EventSpec::learning(
                    "calendar.proposed",
                    json!({ "proposal_id": id, "intent": "ADD_REVIEWS" }),
                )),
            )
        },
    )
}

#[derive(Debug, Deserialize)]
struct ApplyRequest {
    proposal_id: String,
}

/// `calendar.apply` (approval-gated: `ScheduleApply`).
pub fn apply(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: ApplyRequest = decode(payload)?;
    let id: ScheduleProposalId = parse_id(&req.proposal_id, "proposal_id")?;
    let resource = ResourceRef::new(ResourceKind::ScheduleProposal, id.as_str());
    let facts_id = id.clone();
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        move |conn, svc| {
            let row = repo::get_proposal(conn, &facts_id)?.ok_or_else(|| {
                G9Error::not_found("proposal.not_found", "schedule proposal not found")
            })?;
            let view = proposal_view(&row)?;
            if row.state != ProposalState::Proposed {
                // Checked before policy: a closed proposal never yields an approval request.
                return Err(G9Error::precondition_failed(
                    "proposal.not_open",
                    "proposal was already applied or rejected",
                ));
            }
            if !view["feasible"].as_bool().unwrap_or(false) {
                return Err(G9Error::precondition_failed(
                    "proposal.infeasible",
                    "an infeasible proposal cannot be applied",
                ));
            }
            let expires = row
                .proposed_at
                .checked_add(DEFAULT_APPROVAL_TTL)
                .unwrap_or(row.proposed_at);
            if svc.now() >= expires {
                return Err(G9Error::precondition_failed(
                    "proposal.expired",
                    "proposal expired; propose again",
                ));
            }
            Ok(Facts::existing(
                rc(DataDomain::Learning, Classification::Internal, false),
                row.revision,
            ))
        },
        move |env| {
            let row = repo::get_proposal(env.conn, &id)?.ok_or_else(|| {
                G9Error::not_found("proposal.not_found", "schedule proposal not found")
            })?;
            if row.state != ProposalState::Proposed {
                return Err(G9Error::precondition_failed(
                    "proposal.not_open",
                    "proposal was already applied or rejected",
                ));
            }
            let zone = env.svc.config.zone.clone();
            let mut events = Vec::new();
            for p in &row.proposal.placements {
                let civil_start = g9_calendar::to_civil(p.start, &zone)?;
                let civil_end = g9_calendar::to_civil(p.end, &zone)?;
                let event = CalendarEventRow {
                    id: CalendarEventId::parse(&new_id("ev"))
                        .map_err(|_| G9Error::internal_redacted("calendar.event_id"))?,
                    title: "Review queue".to_string(),
                    kind: CalendarEventKind::Review,
                    civil_start,
                    civil_end,
                    zone: zone.clone(),
                    start_utc: p.start,
                    end_utc: p.end,
                    all_day: false,
                    immutable: false,
                    protected: false,
                    source: EventSource::Scheduler,
                    training_day_id: None,
                    objective_id: None,
                    study_block_id: None,
                    dst_adjustment: None,
                    data_domain: DataDomain::Learning,
                    classification: Classification::Internal,
                    revision: Revision::INITIAL,
                    created_at: env.now,
                    updated_at: env.now,
                };
                repo::insert_event(env.conn, &event)?;
                events.push(event_view(&event));
            }
            let revision = repo::set_proposal_state(
                env.conn,
                &id,
                row.revision,
                ProposalState::Applied,
                env.now,
            )?;
            repo::insert_decision(
                env.conn,
                &ScheduleDecisionRow {
                    id: new_id("sd"),
                    proposal_id: id.clone(),
                    decision: ScheduleDecisionKind::Apply,
                    decided_by: env.meta.actor.id.clone(),
                    decided_at: env.now,
                    approval_id: env.consumed.as_ref().map(|c| c.approval_id.clone()),
                    reason: None,
                    applied_revision: Some(revision),
                    created_at: env.now,
                },
            )?;
            let applied = events.len();
            Ok(Mutation::new(
                json!({ "proposal_id": id, "applied": applied, "events": events }),
                Some(revision),
            )
            .with_event(EventSpec::learning(
                "calendar.applied",
                json!({ "proposal_id": id, "applied": applied }),
            )))
        },
    )
}
