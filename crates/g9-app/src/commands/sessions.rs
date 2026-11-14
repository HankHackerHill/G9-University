//! Session and timer commands (M04; ARCHITECTURE.md §7 "Timer ledger"; gate A03).
//!
//! State changes go through `SessionState::transition` (pure) and
//! `g9_storage::repos::sessions::transition_session` (revision-checked,
//! appends the immutable `session_events` row). Focused time is measured by the
//! `g9_calendar::Ledger` from **monotonic** readings: `start`/`resume` open an
//! interval, `pause`/`complete` close it `VERIFIED`; credited time is always
//! recomputed from the ledger (verified intervals plus operator corrections;
//! `UNCERTAIN` time is never credited without a correction). Monotonic
//! readings are comparable only within one process lifetime: intervals opened
//! by this process belong to epoch 1, every stored interval from an earlier
//! process to epoch 0 (recovery closed those `UNCERTAIN` at open).
//!
//! Dual-clock check at stop (TEC-07, partial): when `pause`/`complete` closes
//! an interval, the wall-clock and monotonic elapsed times are compared; if
//! they differ by more than [`CLOCK_DIVERGENCE_TOLERANCE_MS`] (a suspend
//! during which the monotonic clock stood still, or a wall-clock jump) the
//! interval is closed `UNCERTAIN` (never credited without a correction), the
//! session moves to `INTERRUPTED` and the outcome carries the warning
//! `session.clock_discontinuity`. A suspend during which the monotonic source
//! keeps counting (both deltas grow together) cannot be seen at stop time; the
//! TEC-07 heartbeat ledger that would bound it needs a `session_heartbeats`
//! table in `g9-storage` and is PROPOSED.
//!
//! Recovery after a correction (A03): `session.correct_time` moves an
//! `INTERRUPTED` session to `PAUSED` only when no closed `UNCERTAIN` interval
//! of the session is left without a correction (the rule of
//! `g9_calendar::SessionRecord::recover`); otherwise the session stays
//! `INTERRUPTED` and the warning names the remaining interval ids.

use super::{decode, new_id, parse_id, rc, scope_of, tail, to_value};
use crate::context::{AppContext, Services};
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation};
use g9_calendar::{Continuity, Ledger, ManualCorrection, SessionEventRow, TimerError};
use g9_domain::state::SessionEvent;
use g9_domain::{
    Actor, CommandMeta, CommandOutcome, DurationMs, G9Error, ResourceKind, ResourceRef, SessionId,
    SessionState, UtcMillis,
};
use g9_storage::repos::calendar::{
    self as calendar_repo, CalendarEventKind, CalendarEventRow, DstAdjustment, EventSource,
};
use g9_storage::repos::curriculum as curriculum_repo;
use g9_storage::repos::sessions::{
    self as repo, ClosedBy, ManualTimeCorrectionRow, StudySessionRow,
};
use g9_storage::repos::ReadScope;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{json, Value};

/// Largest tolerated difference between wall-clock and monotonic elapsed time
/// of one interval before it is treated as a discontinuity (TEC-07: 5000 ms).
pub const CLOCK_DIVERGENCE_TOLERANCE_MS: i64 = 5_000;

/// Map a timer ledger refusal onto the closed category set.
pub(crate) fn timer_err(e: TimerError) -> G9Error {
    let code = e.code();
    match e {
        TimerError::AlreadyRunning { .. } => G9Error::conflict(code, e.to_string()),
        TimerError::IntervalNotFound(_) => G9Error::not_found(code, e.to_string()),
        TimerError::ActorNotOperator => G9Error::unauthorized(code, e.to_string()),
        TimerError::EmptyReason | TimerError::DuplicateCorrectionId => {
            G9Error::invalid_input(code, e.to_string())
        }
        _ => G9Error::precondition_failed(code, e.to_string()),
    }
}

fn illegal(from: SessionState, event: SessionEvent) -> G9Error {
    G9Error::precondition_failed(
        "session.illegal_transition",
        format!("session: event {event:?} is not allowed in state {from}"),
    )
    .with_details(json!({ "machine": "session", "from": from, "event": event }))
}

/// Rebuild the timer ledger of one session from storage (see module docs on epochs).
pub(crate) fn ledger_for(
    conn: &Connection,
    svc: &Services,
    session: &SessionId,
) -> Result<Ledger, G9Error> {
    let rows = repo::list_intervals(conn, session)?;
    let mut ledger = Ledger::new();
    ledger.process_epoch = 1;
    let mut index = std::collections::BTreeMap::new();
    for (i, row) in rows.iter().enumerate() {
        let mut record = row.record.clone();
        record.process_epoch = u64::from(svc.epoch_intervals.contains(&row.id));
        ledger.intervals.push(record);
        index.insert(row.id.clone(), i);
    }
    for c in repo::list_corrections(conn, session)? {
        let Some(interval_index) = c.interval_id.as_ref().and_then(|id| index.get(id)) else {
            continue;
        };
        ledger.corrections.push(ManualCorrection {
            id: c.id,
            interval_index: *interval_index,
            credited_ms: DurationMs::new(c.delta_ms.max(0)).unwrap_or(DurationMs::ZERO),
            reason: c.reason,
            actor: Actor::operator(c.corrected_by),
            recorded_at: c.corrected_at,
        });
    }
    Ok(ledger)
}

/// `SessionView` (operations.ts) from storage.
pub(crate) fn session_view(
    conn: &Connection,
    svc: &Services,
    row: &StudySessionRow,
) -> Result<Value, G9Error> {
    let ledger = ledger_for(conn, svc, &row.id)?;
    let credit = ledger.credited(&row.id);
    let block = match &row.study_block_id {
        Some(id) => curriculum_repo::get_study_block(conn, id)?,
        None => None,
    };
    let day_source = row
        .training_day_id
        .clone()
        .or_else(|| block.as_ref().map(|b| b.training_day_id.clone()));
    let day_id = day_source
        .as_deref()
        .map(tail)
        .unwrap_or("UNLINKED")
        .to_string();
    let (block_code, block_seq) = match &block {
        // A session not linked to a curriculum block (wave-1 rows created
        // outside the planner) is labelled as such; block_seq must be >= 1.
        Some(b) => (b.block_code.clone(), b.ordinal.max(1)),
        None => ("UNLINKED".to_string(), 1),
    };
    let rows = repo::list_intervals(conn, &row.id)?;
    let intervals: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "interval_id": r.id,
                "start_utc": r.record.start_wall,
                "end_utc": r.record.end_wall,
                "continuity": r.record.continuity,
            })
        })
        .collect();
    let corrections: Vec<Value> = repo::list_corrections(conn, &row.id)?
        .into_iter()
        .filter_map(|c| {
            let interval_id = c.interval_id?;
            Some(json!({
                "correction_id": c.id,
                "interval_id": interval_id,
                "credited_ms": c.delta_ms.max(0),
                "reason": c.reason,
                "corrected_at": c.corrected_at,
            }))
        })
        .collect();
    let open_start = rows
        .iter()
        .find(|r| r.record.is_open())
        .map(|r| r.record.start_wall);
    let requires_correction =
        row.state == SessionState::Interrupted || ledger.has_uncorrected_uncertain(&row.id);
    let view = json!({
        "session_id": row.id,
        "state": row.state,
        "day_id": day_id,
        "block": block_code,
        "block_seq": block_seq,
        "planned_ms": i64::from(row.planned_minutes) * 60_000,
        "credited_ms": credit.total_credited_ms,
        "uncertain_ms": credit.uncertain_ms,
        "started_at": row.started_at,
        "open_interval_started_at": open_start,
        "requires_correction": requires_correction,
        "intervals": intervals,
        "corrections": corrections,
        "revision": row.revision,
    });
    to_value(&view)
}

fn load(conn: &Connection, scope: &ReadScope, id: &SessionId) -> Result<StudySessionRow, G9Error> {
    repo::get_session(conn, scope, id)?.ok_or_else(|| {
        G9Error::not_found("session.not_found", "session not found")
            .with_details(json!({ "session_id": id }))
    })
}

fn civil_date(instant: UtcMillis, svc: &Services) -> Option<String> {
    let c = g9_calendar::to_civil(instant, &svc.config.zone).ok()?;
    Some(format!("{:04}-{:02}-{:02}", c.year, c.month, c.day))
}

#[derive(Debug, Deserialize)]
struct ListRequest {
    date: Option<String>,
}

/// `session.list`.
pub fn list(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: ListRequest = decode(payload)?;
    let resource = ResourceRef::new(ResourceKind::StudySession, "sessions");
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
                g9_domain::Classification::Internal,
                false,
            )))
        },
        |env| {
            let mut sessions = Vec::new();
            for row in repo::list_sessions(env.conn, &scope, None)? {
                if let Some(date) = &req.date {
                    let at = row.planned_start_utc.unwrap_or(row.created_at);
                    if civil_date(at, env.svc).as_deref() != Some(date.as_str()) {
                        continue;
                    }
                }
                sessions.push(session_view(env.conn, env.svc, &row)?);
            }
            Ok(Mutation::new(json!({ "sessions": sessions }), None))
        },
    )
}

#[derive(Debug, Deserialize)]
struct SessionRequest {
    session_id: String,
}

fn transition(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
    event: SessionEvent,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: SessionRequest = decode(payload)?;
    let id: SessionId = parse_id(&req.session_id, "session_id")?;
    let resource = ResourceRef::new(ResourceKind::StudySession, id.as_str());
    let scope = scope_of(meta);
    let facts_id = id.clone();
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        move |conn, _| {
            let row = load(conn, &scope, &facts_id)?;
            Ok(Facts::existing(
                rc(row.data_domain, row.classification, row.state.is_terminal()),
                row.revision,
            ))
        },
        move |env| {
            let row = load(env.conn, &scope, &id)?;
            let mut next = row
                .state
                .transition(event)
                .map_err(|_| illegal(row.state, event))?;
            let mut applied = event;
            let mut warning = None;
            let mut ledger = ledger_for(env.conn, env.svc, &id)?;
            let mono = env.svc.mono();
            let mut details = json!({ "from": row.state, "to": next });
            match event {
                SessionEvent::Start | SessionEvent::Resume => {
                    ledger.start(id.clone(), env.now, mono).map_err(timer_err)?;
                    let interval_id = new_id("ti");
                    repo::open_interval(env.conn, &interval_id, &id, env.now, mono, env.now)?;
                    env.svc.epoch_intervals.insert(interval_id.clone());
                    details["interval_id"] = json!(interval_id);
                }
                SessionEvent::Pause | SessionEvent::Complete => {
                    let open = repo::list_intervals(env.conn, &id)?
                        .into_iter()
                        .find(|r| r.record.is_open());
                    if let Some(open) = open {
                        // A wall clock stepped back before the interval's start
                        // (NTP correction, manual change) must not lock the timer
                        // (storage-repos-1): the wall end is clamped to the start,
                        // as open-time recovery does; the monotonic clock and the
                        // divergence check below decide what is credited.
                        let end_wall = if env.now < open.record.start_wall {
                            open.record.start_wall
                        } else {
                            env.now
                        };
                        let divergence = clock_divergence(&open.record, env.now, mono);
                        if env.svc.epoch_intervals.contains(&open.id)
                            && divergence > CLOCK_DIVERGENCE_TOLERANCE_MS
                        {
                            // TEC-07: the two clocks disagree (suspend with a
                            // stopped monotonic clock, or a wall-clock jump):
                            // the elapsed time is unknown, never VERIFIED.
                            repo::close_interval(
                                env.conn,
                                &open.id,
                                open.revision,
                                end_wall,
                                None,
                                Continuity::Uncertain,
                                ClosedBy::System,
                                env.now,
                            )?;
                            applied = SessionEvent::Interrupt;
                            next = row
                                .state
                                .transition(SessionEvent::Interrupt)
                                .map_err(|_| illegal(row.state, SessionEvent::Interrupt))?;
                            details = json!({
                                "from": row.state,
                                "to": next,
                                "requested": event,
                                "reason": "clock_discontinuity",
                                "divergence_ms": divergence,
                            });
                            warning = Some(format!(
                                "session.clock_discontinuity: wall-clock and monotonic elapsed time differ by {divergence} ms (more than {CLOCK_DIVERGENCE_TOLERANCE_MS} ms: suspend or clock change); the interval was closed UNCERTAIN, is not credited and the session is INTERRUPTED until the operator records a correction"
                            ));
                        } else {
                            ledger.stop(&id, end_wall, mono).map_err(timer_err)?;
                            repo::close_interval(
                                env.conn,
                                &open.id,
                                open.revision,
                                end_wall,
                                Some(mono),
                                Continuity::Verified,
                                if event == SessionEvent::Pause {
                                    ClosedBy::Operator
                                } else {
                                    ClosedBy::System
                                },
                                env.now,
                            )?;
                        }
                        details["interval_id"] = json!(open.id);
                    }
                }
                _ => {}
            }
            let seq =
                u32::try_from(repo::list_session_events(env.conn, &id)?.len()).unwrap_or(u32::MAX);
            let revision = repo::transition_session(
                env.conn,
                &new_id("se"),
                &SessionEventRow {
                    seq,
                    session_id: id.clone(),
                    event: applied,
                    from: row.state,
                    to: next,
                    actor: if applied == event {
                        env.meta.actor.clone()
                    } else {
                        // The interruption is the backend's finding, not the operator's act.
                        env.svc.system.clone()
                    },
                    occurred_at: env.now,
                },
                Some(&details),
                row.revision,
            )?;
            let credit = ledger_for(env.conn, env.svc, &id)?.credited(&id);
            let revision = repo::update_session_credit(
                env.conn,
                &id,
                revision,
                credit.total_credited_ms,
                credit.uncertain_ms,
                env.now,
            )?;
            let stored = load(env.conn, &ReadScope::cross_domain(row.data_domain), &id)?;
            let view = session_view(env.conn, env.svc, &stored)?;
            let kind = match applied {
                SessionEvent::Prepare => "session.prepared",
                SessionEvent::Start => "session.started",
                SessionEvent::Pause => "session.paused",
                SessionEvent::Resume => "session.resumed",
                SessionEvent::Complete => "session.completed",
                SessionEvent::Interrupt => "session.interrupted",
                _ => "session.transitioned",
            };
            let mutation = Mutation::new(view, Some(revision)).with_event(EventSpec::learning(
                kind,
                json!({
                    "session_id": id,
                    "from": row.state,
                    "to": next,
                    "credited_ms": credit.total_credited_ms,
                    "uncertain_ms": credit.uncertain_ms,
                }),
            ));
            Ok(match warning {
                Some(w) => mutation.with_warning(w),
                None => mutation,
            })
        },
    )
}

/// |wall-clock elapsed − monotonic elapsed| of an open interval at `now`/`mono`
/// (saturating; a clock that went backwards counts as divergence).
fn clock_divergence(record: &g9_calendar::IntervalRecord, now: UtcMillis, mono: u64) -> i64 {
    let wall = now
        .as_millis()
        .saturating_sub(record.start_wall.as_millis());
    let mono_now = i64::try_from(mono).unwrap_or(i64::MAX);
    let mono_start = i64::try_from(record.start_mono_ms).unwrap_or(i64::MAX);
    wall.saturating_sub(mono_now.saturating_sub(mono_start))
        .saturating_abs()
}

/// `session.prepare` (PLANNED → READY).
pub fn prepare(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    transition(ctx, spec, meta, payload, SessionEvent::Prepare)
}

/// `session.start` (READY → RUNNING; opens a monotonic interval).
pub fn start(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    transition(ctx, spec, meta, payload, SessionEvent::Start)
}

/// `session.pause` (RUNNING → PAUSED; closes the interval VERIFIED).
pub fn pause(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    transition(ctx, spec, meta, payload, SessionEvent::Pause)
}

/// `session.resume` (PAUSED → RUNNING; opens a new interval).
pub fn resume(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    transition(ctx, spec, meta, payload, SessionEvent::Resume)
}

/// `session.complete` (RUNNING/PAUSED/INTERRUPTED → COMPLETED; credit from the ledger).
pub fn complete(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    transition(ctx, spec, meta, payload, SessionEvent::Complete)
}

#[derive(Debug, Deserialize)]
struct CorrectRequest {
    session_id: String,
    interval_id: String,
    credited_ms: i64,
    reason: String,
}

/// `session.correct_time`: an operator correction is a new
/// `manual_time_corrections` row that supersedes the credit of one closed
/// `UNCERTAIN` interval; the interval itself is never edited. An
/// `INTERRUPTED` session recovers to `PAUSED` once no `UNCERTAIN` interval of
/// the session is left without a correction.
pub fn correct_time(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: CorrectRequest = decode(payload)?;
    let id: SessionId = parse_id(&req.session_id, "session_id")?;
    // Corrections are refused during an open exam (`operator.correct.allow`
    // excludes EXAM); the mode is backend-derived (TEC-25).
    let meta = &super::with_backend_mode(ctx, meta)?;
    let resource = ResourceRef::new(ResourceKind::TimeInterval, req.interval_id.as_str());
    let scope = scope_of(meta);
    let facts_id = id.clone();
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        move |conn, _| {
            let row = load(conn, &scope, &facts_id)?;
            Ok(Facts::existing(
                rc(row.data_domain, row.classification, row.state.is_terminal()),
                row.revision,
            ))
        },
        move |env| {
            let row = load(env.conn, &scope, &id)?;
            let mut ledger = ledger_for(env.conn, env.svc, &id)?;
            let intervals = repo::list_intervals(env.conn, &id)?;
            let (index, interval) = intervals
                .iter()
                .enumerate()
                .find(|(_, r)| r.id == req.interval_id)
                .ok_or_else(|| {
                    G9Error::not_found("interval.not_found", "time interval not found")
                        .with_details(json!({ "interval_id": req.interval_id }))
                })?;
            if interval.record.continuity != Continuity::Uncertain || interval.record.is_open() {
                return Err(G9Error::precondition_failed(
                    "interval.not_correctable",
                    "only closed UNCERTAIN intervals accept a correction",
                ));
            }
            let max = interval.record.wall_estimate();
            let credited = DurationMs::new(req.credited_ms).map_err(|_| {
                G9Error::invalid_input("correction.negative", "credited time must be non-negative")
            })?;
            if credited > max {
                return Err(G9Error::invalid_input(
                    "correction.exceeds_interval",
                    "credited time cannot exceed the uncertain interval length",
                )
                .with_details(json!({ "max_ms": max })));
            }
            let correction_id = new_id("tc");
            ledger
                .correct(ManualCorrection {
                    id: correction_id.clone(),
                    interval_index: index,
                    credited_ms: credited,
                    reason: req.reason.clone(),
                    actor: env.meta.actor.clone(),
                    recorded_at: env.now,
                })
                .map_err(timer_err)?;
            repo::append_manual_correction(
                env.conn,
                &ManualTimeCorrectionRow {
                    id: correction_id.clone(),
                    session_id: id.clone(),
                    interval_id: Some(req.interval_id.clone()),
                    corrected_start_utc: None,
                    corrected_end_utc: None,
                    delta_ms: credited.as_millis(),
                    reason: req.reason.clone(),
                    corrected_by: env.meta.actor.id.clone(),
                    corrected_at: env.now,
                    approval_id: env.consumed.as_ref().map(|c| c.approval_id.clone()),
                    created_at: env.now,
                },
            )?;
            // A03 / `SessionRecord::recover`: recover only when no closed
            // UNCERTAIN interval of this session is left uncorrected.
            let remaining: Vec<String> = intervals
                .iter()
                .enumerate()
                .filter(|(i, r)| {
                    r.record.continuity == Continuity::Uncertain
                        && !r.record.is_open()
                        && ledger.latest_correction(*i).is_none()
                })
                .map(|(_, r)| r.id.clone())
                .collect();
            let mut warning = None;
            let mut revision = row.revision;
            if row.state == SessionState::Interrupted && !remaining.is_empty() {
                warning = Some(format!(
                    "session.uncorrected_uncertain_time: the session stays INTERRUPTED until every UNCERTAIN interval has a correction; remaining: {}",
                    remaining.join(", ")
                ));
            } else if row.state == SessionState::Interrupted {
                let next = row
                    .state
                    .transition(SessionEvent::Recover)
                    .map_err(|_| illegal(row.state, SessionEvent::Recover))?;
                let seq = u32::try_from(repo::list_session_events(env.conn, &id)?.len())
                    .unwrap_or(u32::MAX);
                revision = repo::transition_session(
                    env.conn,
                    &new_id("se"),
                    &SessionEventRow {
                        seq,
                        session_id: id.clone(),
                        event: SessionEvent::Recover,
                        from: row.state,
                        to: next,
                        actor: env.meta.actor.clone(),
                        occurred_at: env.now,
                    },
                    Some(&json!({ "correction_id": correction_id })),
                    revision,
                )?;
            }
            let credit = ledger_for(env.conn, env.svc, &id)?.credited(&id);
            let revision = repo::update_session_credit(
                env.conn,
                &id,
                revision,
                credit.total_credited_ms,
                credit.uncertain_ms,
                env.now,
            )?;
            let stored = load(env.conn, &ReadScope::cross_domain(row.data_domain), &id)?;
            let view = session_view(env.conn, env.svc, &stored)?;
            let mutation = Mutation::new(view, Some(revision)).with_event(EventSpec::learning(
                "session.time_corrected",
                json!({
                    "session_id": id,
                    "interval_id": req.interval_id,
                    "correction_id": correction_id,
                    "credited_ms": credited,
                    "remaining_uncertain": remaining,
                }),
            ));
            Ok(match warning {
                Some(w) => mutation.with_warning(w),
                None => mutation,
            })
        },
    )
}

#[derive(Debug, Deserialize)]
struct CreateRequest {
    planned_minutes: u32,
    planned_start: Option<i64>,
    mode: Option<g9_domain::InteractionMode>,
    objective_id: Option<String>,
}

/// `session.create`: a PLANNED study session the operator schedules, optionally
/// linked to a curriculum objective by code. It then runs like any session
/// (prepare, start, pause, complete).
pub fn create(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: CreateRequest = decode(payload)?;
    let session_id: SessionId = parse_id(
        &super::creation_id(ctx, spec, meta, || new_id("session"))?,
        "session_id",
    )?;
    let resource = ResourceRef::new(ResourceKind::StudySession, session_id.as_str());
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
                g9_domain::Classification::Internal,
                false,
            )))
        },
        |env| {
            let objective_id = match &req.objective_id {
                None => None,
                Some(code) => {
                    let code: g9_domain::ObjectiveId = parse_id(code, "objective_id")?;
                    let version = crate::commands::curriculum::active_version(env.conn, env.svc)?
                        .ok_or_else(|| {
                        G9Error::not_found(
                            "curriculum.not_imported",
                            "no curriculum version is imported",
                        )
                    })?;
                    let row = curriculum_repo::objective_by_code(env.conn, &version.id, &code)?
                        .ok_or_else(|| {
                            G9Error::not_found("objective.not_found", "objective not found")
                        })?;
                    Some(row.id)
                }
            };
            let mode = req.mode.unwrap_or(g9_domain::InteractionMode::Study);
            if !matches!(
                mode,
                g9_domain::InteractionMode::Study
                    | g9_domain::InteractionMode::Lab
                    | g9_domain::InteractionMode::Review
            ) {
                return Err(G9Error::invalid_input(
                    "session.mode_invalid",
                    "a study session runs in STUDY, LAB or REVIEW mode",
                ));
            }
            let start = req.planned_start.map_or(env.now, UtcMillis::from_millis);
            let end = start
                .checked_add(DurationMs::from_minutes_u32(req.planned_minutes))
                .map_err(|_| {
                    G9Error::invalid_input(
                        "session.planned_end_invalid",
                        "the planned end overflows",
                    )
                })?;
            let row = StudySessionRow {
                id: session_id.clone(),
                training_day_id: None,
                objective_id,
                study_block_id: None,
                calendar_event_id: None,
                state: SessionState::Planned,
                mode,
                zone: env.svc.config.zone.clone(),
                planned_start_utc: Some(start),
                planned_end_utc: Some(end),
                planned_minutes: req.planned_minutes,
                started_at: None,
                ended_at: None,
                credited_ms: DurationMs::ZERO,
                uncertain_ms: DurationMs::ZERO,
                data_domain: g9_domain::DataDomain::Learning,
                classification: g9_domain::Classification::Internal,
                revision: g9_domain::Revision::INITIAL,
                created_at: env.now,
                updated_at: env.now,
            };
            let revision = repo::insert_session(env.conn, &row)?;
            Ok(
                Mutation::new(session_view(env.conn, env.svc, &row)?, Some(revision)).with_event(
                    EventSpec::learning("session.created", json!({ "session_id": session_id })),
                ),
            )
        },
    )
}

#[derive(Debug, Deserialize)]
struct PlanDayRequest {
    date: String,
}

/// `session.plan_day`: one PLANNED session for each block of the program day that falls on
/// `date`, linked to its training day, objective and block, in the mode the curriculum gives the
/// block, laid out back to back from the time the operator set for a day to start (09:00 by
/// default) in the configured zone. Each session gets its calendar event (a study block, or an
/// assessment for the blind practical; source SCHEDULER, so the operator cannot edit it and the
/// scheduler plans reviews around it). A block that already has a session is left as it is, so
/// the call can be repeated: it plans what is still missing.
pub fn plan_day(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: PlanDayRequest = decode(payload)?;
    let date = super::curriculum::parse_date(&req.date)?;
    let resource = ResourceRef::new(ResourceKind::StudySession, format!("day-{}", req.date));
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
                g9_domain::Classification::Internal,
                false,
            )))
        },
        |env| {
            let (_, _, day) = super::curriculum::training_day_on(env.conn, env.svc, date)?
                .ok_or_else(|| {
                    G9Error::not_found(
                        "curriculum.no_training_day",
                        "the program has no training day on this date",
                    )
                })?;
            let blocks = curriculum_repo::study_blocks(env.conn, &day.id)?;
            if blocks.is_empty() {
                return Err(G9Error::not_found(
                    "curriculum.no_blocks",
                    "the training day on this date has no study blocks",
                ));
            }
            let scope = ReadScope::domain(g9_domain::DataDomain::Learning);
            let existing = repo::list_sessions_for_day(env.conn, &scope, &day.id)?;
            let day_start_minute =
                curriculum_repo::get_program_plan(env.conn, &env.svc.config.curriculum_id)?
                    .map_or(super::curriculum::DEFAULT_DAY_START_MINUTE, |p| {
                        p.day_start_minute
                    });
            let hour = u8::try_from(day_start_minute / 60).unwrap_or(0);
            let minute = u8::try_from(day_start_minute % 60).unwrap_or(0);
            let civil =
                g9_domain::CivilDateTime::new(date.year, date.month, date.day, hour, minute, 0)
                    .map_err(|_| {
                        G9Error::invalid_input("date.invalid", "date must be 'YYYY-MM-DD'")
                    })?;
            let resolved = g9_calendar::resolve_local(&civil, &env.svc.config.zone)?;
            let day_adjustment = DstAdjustment::from_adjustment(&resolved.adjustment);
            let mut cursor = resolved.instant;
            let mut created = Vec::new();
            let mut events = 0u32;
            let mut skipped = Vec::new();
            for block in &blocks {
                let start = cursor;
                let end = start
                    .checked_add(DurationMs::from_minutes_u32(block.minutes))
                    .map_err(|_| {
                        G9Error::invalid_input(
                            "session.planned_end_invalid",
                            "the planned end overflows",
                        )
                    })?;
                cursor = end;
                if let Some(linked) = existing
                    .iter()
                    .find(|s| s.study_block_id.as_deref() == Some(block.id.as_str()))
                {
                    skipped.push(json!({
                        "block": block.block_code,
                        "seq": block.ordinal,
                        "session_id": linked.id,
                    }));
                    continue;
                }
                let id: SessionId = parse_id(&new_id("session"), "session_id")?;
                let event_id: g9_domain::CalendarEventId = parse_id(&new_id("ev"), "event_id")?;
                let zone = env.svc.config.zone.clone();
                calendar_repo::insert_event(
                    env.conn,
                    &CalendarEventRow {
                        id: event_id.clone(),
                        title: format!(
                            "{} {}",
                            tail(day.day_code.as_str()),
                            block.block_code.replace('_', " ")
                        ),
                        kind: if block.mode == g9_domain::InteractionMode::Exam {
                            CalendarEventKind::Assessment
                        } else {
                            CalendarEventKind::StudyBlock
                        },
                        civil_start: g9_calendar::to_civil(start, &zone)?,
                        civil_end: g9_calendar::to_civil(end, &zone)?,
                        zone,
                        start_utc: start,
                        end_utc: end,
                        all_day: false,
                        immutable: block.mode == g9_domain::InteractionMode::Exam,
                        protected: false,
                        source: EventSource::Scheduler,
                        training_day_id: Some(day.id.clone()),
                        objective_id: block.objective_id.clone(),
                        study_block_id: Some(block.id.clone()),
                        dst_adjustment: if created.is_empty() && skipped.is_empty() {
                            day_adjustment
                        } else {
                            None
                        },
                        data_domain: g9_domain::DataDomain::Learning,
                        classification: g9_domain::Classification::Internal,
                        revision: g9_domain::Revision::INITIAL,
                        created_at: env.now,
                        updated_at: env.now,
                    },
                )?;
                events += 1;
                let row = StudySessionRow {
                    id: id.clone(),
                    training_day_id: Some(day.id.clone()),
                    objective_id: block.objective_id.clone(),
                    study_block_id: Some(block.id.clone()),
                    calendar_event_id: Some(event_id),
                    state: SessionState::Planned,
                    mode: block.mode,
                    zone: env.svc.config.zone.clone(),
                    planned_start_utc: Some(start),
                    planned_end_utc: Some(end),
                    planned_minutes: block.minutes,
                    started_at: None,
                    ended_at: None,
                    credited_ms: DurationMs::ZERO,
                    uncertain_ms: DurationMs::ZERO,
                    data_domain: g9_domain::DataDomain::Learning,
                    classification: g9_domain::Classification::Internal,
                    revision: g9_domain::Revision::INITIAL,
                    created_at: env.now,
                    updated_at: env.now,
                };
                repo::insert_session(env.conn, &row)?;
                created.push(session_view(env.conn, env.svc, &row)?);
            }
            let ids: Vec<Value> = created
                .iter()
                .filter_map(|v| v.get("session_id").cloned())
                .collect();
            Ok(Mutation::new(
                json!({
                    "date": req.date,
                    "day_id": tail(day.day_code.as_str()),
                    "created": created,
                    "already_planned": skipped,
                }),
                Some(g9_domain::Revision::INITIAL),
            )
            .with_event(EventSpec::learning(
                "session.day_planned",
                json!({ "date": req.date, "day": tail(day.day_code.as_str()), "sessions": ids, "events": events }),
            )))
        },
    )
}
