//! Session and timer repository (M04; migration `0004_calendar`, tables
//! `study_sessions`, `session_events`, `time_intervals`, `manual_time_corrections`).
//!
//! * `study_sessions` carries the `SessionState` and a revision; every state
//!   change goes through [`transition_session`], which verifies the recorded
//!   edge against `g9_domain::SessionState::transition` and appends the
//!   immutable `session_events` row in the same statement sequence.
//! * `time_intervals` is the append-only timer ledger (A03): [`open_interval`]
//!   refuses overlap in SQL (any open interval anywhere, or a closed interval of
//!   any session ending after the new start); [`close_interval`] closes exactly
//!   once; the trigger refuses re-timing a closed row; nothing is ever deleted.
//! * [`open_intervals`] is the restart-recovery query: `g9-app` closes every
//!   returned interval `UNCERTAIN` and raises `SessionEvent::Interrupt`.
//! * `manual_time_corrections` are appended, never updated.
//!
//! DDL ↔ wave-1 notes: `IntervalRecord::process_epoch` has no column and reads
//! back as `0`; an `UNCERTAIN` closure stores `end_mono_ms = start_mono_ms`
//! (the DDL requires both ends or neither) and the adapter maps it to `None`;
//! `session_events` cannot record a skill actor's skill id, and sessions are
//! operator/system driven, so skill actors are refused.

use super::{
    finish_update, from_json, from_ms, i64_from_u64, ms, opt_from_ms, opt_ms, opt_parse_id,
    query_all, query_opt, required, rev_from_i64, rev_i64, to_json, u32_from_i64, u64_from_i64,
    ReadScope, RepoError, RepoResult,
};
use crate::error::{StorageError, StorageResult};
use crate::sql::{actor_columns, actor_from_columns, enum_from_text, enum_to_text, parse_id};
use g9_calendar::{Continuity, IntervalRecord, SessionEventRow, SessionRecord};
use g9_domain::state::SessionEvent;
use g9_domain::{
    Actor, ActorId, ActorKind, ApprovalId, CalendarEventId, Classification, DataDomain, DurationMs,
    IanaZone, InteractionMode, Revision, SessionId, SessionState, UtcMillis,
};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

/// A `study_sessions` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudySessionRow {
    /// Session id.
    pub id: SessionId,
    /// Linked training-day row.
    pub training_day_id: Option<String>,
    /// Linked objective row.
    pub objective_id: Option<String>,
    /// Linked curriculum block (`study_blocks.id`, migration `0010`; IPC `block`/`block_seq`).
    pub study_block_id: Option<String>,
    /// Linked calendar event.
    pub calendar_event_id: Option<CalendarEventId>,
    /// Machine state.
    pub state: SessionState,
    /// Interaction mode.
    pub mode: InteractionMode,
    /// Operator zone at planning time.
    pub zone: IanaZone,
    /// Planned start.
    pub planned_start_utc: Option<UtcMillis>,
    /// Planned end.
    pub planned_end_utc: Option<UtcMillis>,
    /// Planned minutes.
    pub planned_minutes: u32,
    /// Actual start.
    pub started_at: Option<UtcMillis>,
    /// Actual end.
    pub ended_at: Option<UtcMillis>,
    /// Credited focused time (verified + corrected).
    pub credited_ms: DurationMs,
    /// Uncertain time awaiting correction.
    pub uncertain_ms: DurationMs,
    /// Privacy domain.
    pub data_domain: DataDomain,
    /// Classification.
    pub classification: Classification,
    /// Revision.
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

const SESSION_COLUMNS: &str = "id, training_day_id, objective_id, calendar_event_id, state, mode, zone, planned_start_utc,
    planned_end_utc, planned_minutes, started_at, ended_at, credited_ms, uncertain_ms, data_domain, classification,
    revision, created_at, updated_at, study_block_id";

fn duration(v: i64, column: &str) -> StorageResult<DurationMs> {
    DurationMs::new(v).map_err(|_| StorageError::RowDecode(format!("column {column} negative")))
}

fn map_session(r: &Row<'_>) -> StorageResult<StudySessionRow> {
    Ok(StudySessionRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        training_day_id: r.get("training_day_id")?,
        objective_id: r.get("objective_id")?,
        study_block_id: r.get("study_block_id")?,
        calendar_event_id: opt_parse_id(r.get("calendar_event_id")?)?,
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        mode: enum_from_text(&r.get::<_, String>("mode")?)?,
        zone: IanaZone::parse(&r.get::<_, String>("zone")?)
            .map_err(|e| StorageError::RowDecode(e.to_string()))?,
        planned_start_utc: opt_from_ms(r.get("planned_start_utc")?),
        planned_end_utc: opt_from_ms(r.get("planned_end_utc")?),
        planned_minutes: u32_from_i64(r.get("planned_minutes")?, "planned_minutes")?,
        started_at: opt_from_ms(r.get("started_at")?),
        ended_at: opt_from_ms(r.get("ended_at")?),
        credited_ms: duration(r.get("credited_ms")?, "credited_ms")?,
        uncertain_ms: duration(r.get("uncertain_ms")?, "uncertain_ms")?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert a session (revision 1). The initial state is whatever the caller's
/// `SessionRecord` holds (normally `PLANNED`).
pub fn insert_session(conn: &Connection, row: &StudySessionRow) -> RepoResult<Revision> {
    conn.execute(
        &format!(
            "INSERT INTO study_sessions ({SESSION_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, 1, ?17, ?17, ?18)"
        ),
        params![
            row.id.as_str(),
            row.training_day_id,
            row.objective_id,
            row.calendar_event_id.as_ref().map(|c| c.as_str().to_string()),
            enum_to_text(&row.state)?,
            enum_to_text(&row.mode)?,
            row.zone.as_str(),
            opt_ms(row.planned_start_utc),
            opt_ms(row.planned_end_utc),
            i64::from(row.planned_minutes),
            opt_ms(row.started_at),
            opt_ms(row.ended_at),
            row.credited_ms.as_millis(),
            row.uncertain_ms.as_millis(),
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
            row.study_block_id,
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One session, scope-checked.
pub fn get_session(
    conn: &Connection,
    scope: &ReadScope,
    id: &SessionId,
) -> RepoResult<Option<StudySessionRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {SESSION_COLUMNS} FROM study_sessions WHERE id = ?1"),
        &[&id.as_str()],
        map_session,
    )?;
    if let Some(r) = &row {
        scope.check("study_sessions", r.data_domain)?;
    }
    Ok(row)
}

/// Sessions in a state (or all), domain-filtered in SQL, planned start order.
pub fn list_sessions(
    conn: &Connection,
    scope: &ReadScope,
    state: Option<SessionState>,
) -> RepoResult<Vec<StudySessionRow>> {
    let state_text = state.as_ref().map(enum_to_text).transpose()?;
    query_all(
        conn,
        &format!(
            "SELECT {SESSION_COLUMNS} FROM study_sessions
             WHERE (?1 IS NULL OR state = ?1) AND (?2 = 1 OR data_domain = ?3)
             ORDER BY planned_start_utc, created_at, id"
        ),
        &[&state_text, &scope.sql_flag(), &scope.sql_domain()?],
        map_session,
    )
}

/// Sessions linked to a training-day row.
pub fn list_sessions_for_day(
    conn: &Connection,
    scope: &ReadScope,
    training_day_id: &str,
) -> RepoResult<Vec<StudySessionRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {SESSION_COLUMNS} FROM study_sessions
             WHERE training_day_id = ?1 AND (?2 = 1 OR data_domain = ?3)
             ORDER BY planned_start_utc, created_at, id"
        ),
        &[&training_day_id, &scope.sql_flag(), &scope.sql_domain()?],
        map_session,
    )
}

/// Planning fields that may change before a session is closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionPlanPatch {
    /// Linked training-day row.
    pub training_day_id: Option<String>,
    /// Linked objective row.
    pub objective_id: Option<String>,
    /// Linked curriculum block.
    pub study_block_id: Option<String>,
    /// Linked calendar event.
    pub calendar_event_id: Option<CalendarEventId>,
    /// Interaction mode.
    pub mode: InteractionMode,
    /// Zone.
    pub zone: IanaZone,
    /// Planned start.
    pub planned_start_utc: Option<UtcMillis>,
    /// Planned end.
    pub planned_end_utc: Option<UtcMillis>,
    /// Planned minutes.
    pub planned_minutes: u32,
}

/// Replace the planning fields of a non-terminal session.
pub fn update_session_plan(
    conn: &Connection,
    id: &SessionId,
    expected_revision: Revision,
    patch: &SessionPlanPatch,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let changed = conn.execute(
        "UPDATE study_sessions
         SET training_day_id = ?3, objective_id = ?4, calendar_event_id = ?5, mode = ?6, zone = ?7,
             planned_start_utc = ?8, planned_end_utc = ?9, planned_minutes = ?10, revision = revision + 1, updated_at = ?11,
             study_block_id = ?12
         WHERE id = ?1 AND revision = ?2 AND state NOT IN ('CLOSED', 'CANCELLED')",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            patch.training_day_id,
            patch.objective_id,
            patch.calendar_event_id.as_ref().map(|c| c.as_str().to_string()),
            enum_to_text(&patch.mode)?,
            patch.zone.as_str(),
            opt_ms(patch.planned_start_utc),
            opt_ms(patch.planned_end_utc),
            i64::from(patch.planned_minutes),
            ms(now),
            patch.study_block_id,
        ],
    )?;
    finish_update(
        conn,
        "study_sessions",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

/// Replace the credited/uncertain time of a session (computed by
/// `g9_calendar::Ledger::credited`).
pub fn update_session_credit(
    conn: &Connection,
    id: &SessionId,
    expected_revision: Revision,
    credited_ms: DurationMs,
    uncertain_ms: DurationMs,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let changed = conn.execute(
        "UPDATE study_sessions SET credited_ms = ?3, uncertain_ms = ?4, revision = revision + 1, updated_at = ?5
         WHERE id = ?1 AND revision = ?2",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            credited_ms.as_millis(),
            uncertain_ms.as_millis(),
            ms(now)
        ],
    )?;
    finish_update(
        conn,
        "study_sessions",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

/// A stored `session_events` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredSessionEvent {
    /// Row id.
    pub id: String,
    /// The transition as the calendar crate models it (`seq` is the position in
    /// the session's history).
    pub row: SessionEventRow,
    /// Structured details (ids and facts only).
    pub details: Option<serde_json::Value>,
}

fn actor_kind_for_session(actor: &Actor) -> StorageResult<(String, &'static str)> {
    if matches!(actor.kind, ActorKind::Skill { .. }) {
        return Err(StorageError::InvalidInput(
            "session_events cannot record a skill actor; sessions are operator/system driven"
                .to_string(),
        ));
    }
    let (id, kind, _) = actor_columns(actor);
    Ok((id, kind))
}

/// Apply one recorded transition: the row's `from` must equal the stored state,
/// `from + event` must yield `to` in the `SessionState` table, the session row
/// moves to `to` (with `started_at`/`ended_at` bookkeeping) and the event is
/// appended. Returns the new revision.
pub fn transition_session(
    conn: &Connection,
    event_row_id: &str,
    event: &SessionEventRow,
    details: Option<&serde_json::Value>,
    expected_revision: Revision,
) -> RepoResult<Revision> {
    super::atomic(conn, || {
        transition_session_statements(conn, event_row_id, event, details, expected_revision)
    })
}

/// The statements of [`transition_session`], which runs them as one unit (`super::atomic`).
fn transition_session_statements(
    conn: &Connection,
    event_row_id: &str,
    event: &SessionEventRow,
    details: Option<&serde_json::Value>,
    expected_revision: Revision,
) -> RepoResult<Revision> {
    let scope = ReadScope::cross_domain(DataDomain::Learning);
    let current = required(
        "study_sessions",
        get_session(conn, &scope, &event.session_id)?,
    )?;
    if current.revision != expected_revision {
        return Err(RepoError::Storage(StorageError::StaleRevision {
            expected: Some(expected_revision),
            actual: current.revision,
        }));
    }
    if current.state != event.from {
        return Err(RepoError::Storage(StorageError::Conflict(format!(
            "session state is {} but the transition starts from {}",
            current.state, event.from
        ))));
    }
    let next = event.from.transition(event.event)?;
    if next != event.to {
        return Err(RepoError::Storage(StorageError::InvalidInput(format!(
            "transition {} + {:?} yields {}, not {}",
            event.from, event.event, next, event.to
        ))));
    }
    let (actor_id, actor_kind) = actor_kind_for_session(&event.actor)?;
    let started_at = match (event.event, current.started_at) {
        (SessionEvent::Start, None) => Some(ms(event.occurred_at)),
        (_, existing) => opt_ms(existing),
    };
    let ended_at = match event.event {
        SessionEvent::Complete | SessionEvent::Cancel | SessionEvent::Interrupt => {
            Some(ms(event.occurred_at))
        }
        SessionEvent::Resume | SessionEvent::Recover => None,
        _ => opt_ms(current.ended_at),
    };
    let changed = conn.execute(
        "UPDATE study_sessions SET state = ?3, started_at = ?4, ended_at = ?5, revision = revision + 1, updated_at = ?6
         WHERE id = ?1 AND revision = ?2",
        params![
            event.session_id.as_str(),
            rev_i64(expected_revision),
            enum_to_text(&next)?,
            started_at,
            ended_at,
            ms(event.occurred_at),
        ],
    )?;
    let revision = finish_update(
        conn,
        "study_sessions",
        "id",
        event.session_id.as_str(),
        expected_revision,
        changed,
    )?;
    conn.execute(
        "INSERT INTO session_events (id, session_id, event, from_state, to_state, occurred_at, actor_id, actor_kind,
            details_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?6)",
        params![
            event_row_id,
            event.session_id.as_str(),
            enum_to_text(&event.event)?,
            enum_to_text(&event.from)?,
            enum_to_text(&event.to)?,
            ms(event.occurred_at),
            actor_id,
            actor_kind,
            details.map(to_json).transpose()?,
        ],
    )?;
    Ok(revision)
}

/// Transition history of a session in the order the transitions were written
/// (`seq` assigned by position). Insertion order is the rowid order: the table
/// is append-only (no UPDATE or DELETE), so SQLite assigns increasing rowids,
/// and VACUUM, VACUUM INTO and the backup API keep them. `occurred_at` is wall
/// time, which orders nothing: after the clock steps back, a later transition
/// carries an earlier stamp (storage-repos-11).
pub fn list_session_events(
    conn: &Connection,
    session_id: &SessionId,
) -> RepoResult<Vec<StoredSessionEvent>> {
    let mut seq: u32 = 0;
    query_all(
        conn,
        "SELECT id, session_id, event, from_state, to_state, occurred_at, actor_id, actor_kind, details_json
         FROM session_events WHERE session_id = ?1 ORDER BY rowid",
        &[&session_id.as_str()],
        |r| {
            let this_seq = seq;
            seq = seq.saturating_add(1);
            Ok(StoredSessionEvent {
                id: r.get("id")?,
                row: SessionEventRow {
                    seq: this_seq,
                    session_id: parse_id(&r.get::<_, String>("session_id")?)?,
                    event: enum_from_text(&r.get::<_, String>("event")?)?,
                    from: enum_from_text(&r.get::<_, String>("from_state")?)?,
                    to: enum_from_text(&r.get::<_, String>("to_state")?)?,
                    actor: actor_from_columns(
                        &r.get::<_, String>("actor_id")?,
                        &r.get::<_, String>("actor_kind")?,
                        None,
                    )?,
                    occurred_at: from_ms(r.get("occurred_at")?),
                },
                details: r
                    .get::<_, Option<String>>("details_json")?
                    .as_deref()
                    .map(|t| from_json(t, "details_json"))
                    .transpose()?,
            })
        },
    )
}

/// Rebuild the calendar crate's `SessionRecord` (state, revision, domain, history).
pub fn load_session_record(
    conn: &Connection,
    scope: &ReadScope,
    id: &SessionId,
) -> RepoResult<Option<SessionRecord>> {
    let Some(row) = get_session(conn, scope, id)? else {
        return Ok(None);
    };
    let events = list_session_events(conn, id)?
        .into_iter()
        .map(|e| e.row)
        .collect();
    Ok(Some(SessionRecord {
        id: row.id,
        state: row.state,
        revision: row.revision,
        data_domain: row.data_domain,
        events,
    }))
}

/// Who closed an interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClosedBy {
    /// Operator stop/pause.
    Operator,
    /// Deterministic system routine (session completion).
    System,
    /// Restart recovery (`UNCERTAIN`).
    Recovery,
}

/// A `time_intervals` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeIntervalRow {
    /// Row id.
    pub id: String,
    /// The ledger record (`process_epoch` is not persisted and reads as 0).
    pub record: IntervalRecord,
    /// Who closed it (`None` while open).
    pub closed_by: Option<ClosedBy>,
    /// Revision (bookkeeping only; closed rows cannot be re-timed).
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

const INTERVAL_COLUMNS: &str = "id, session_id, start_utc, end_utc, start_mono_ms, end_mono_ms, continuity, closed_by, revision,
    created_at, updated_at";

fn map_interval(r: &Row<'_>) -> StorageResult<TimeIntervalRow> {
    let continuity: Continuity = enum_from_text(&r.get::<_, String>("continuity")?)?;
    let end_utc = opt_from_ms(r.get("end_utc")?);
    let end_mono_ms = match (continuity, r.get::<_, Option<i64>>("end_mono_ms")?) {
        // UNCERTAIN rows carry a placeholder end_mono_ms (DDL requires both ends); the record has none
        (Continuity::Uncertain, _) => None,
        (Continuity::Verified, v) => v.map(|m| u64_from_i64(m, "end_mono_ms")).transpose()?,
    };
    Ok(TimeIntervalRow {
        id: r.get("id")?,
        record: IntervalRecord {
            session_id: parse_id(&r.get::<_, String>("session_id")?)?,
            start_wall: from_ms(r.get("start_utc")?),
            end_wall: end_utc,
            start_mono_ms: u64_from_i64(r.get("start_mono_ms")?, "start_mono_ms")?,
            end_mono_ms,
            continuity,
            process_epoch: 0,
        },
        closed_by: r
            .get::<_, Option<String>>("closed_by")?
            .as_deref()
            .map(enum_from_text)
            .transpose()?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Open a focused-time interval. Refused (`CONFLICT`) while any interval is
/// open in any session: there is one learner, and an interval can only start
/// after the previous one was closed, so focused time of two sessions can never
/// overlap and cannot be double counted (credit comes from the monotonic
/// readings, ADR-0011). A backdated start (`start_utc < now`) is a claim about
/// the past and is also refused (`CONFLICT`) when a recorded interval ends after it.
///
/// A start at the caller's current reading (`start_utc >= now`, the `g9-app`
/// path) is NOT compared with recorded wall instants, which are for display and
/// correlation only (`g9-calendar` timer contract): the wall clock can step
/// backwards (NTP, a hardware clock kept in local time, defect TEC-07) and
/// closed intervals are immutable, so such a rule would refuse every start
/// until real time passed the latest recorded end.
pub fn open_interval(
    conn: &Connection,
    id: &str,
    session_id: &SessionId,
    start_utc: UtcMillis,
    start_mono_ms: u64,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let open: Option<String> = query_opt(
        conn,
        "SELECT session_id FROM time_intervals WHERE end_utc IS NULL LIMIT 1",
        &[],
        |r| Ok(r.get("session_id")?),
    )?;
    if let Some(open_session) = open {
        return Err(RepoError::Storage(StorageError::Conflict(format!(
            "time_intervals.already_running: session {open_session} has an open interval"
        ))));
    }
    // `start_utc` and `now` come from the caller's clock; a live start has
    // start_utc == now, so a wall-clock step can never reach this check.
    if start_utc < now {
        let overlapping: i64 = conn.query_row(
            "SELECT COUNT(*) FROM time_intervals WHERE end_utc > ?1",
            params![ms(start_utc)],
            |r| r.get(0),
        )?;
        if overlapping > 0 {
            return Err(RepoError::Storage(StorageError::Conflict(
                "time_intervals.overlap: a recorded interval ends after the new start".to_string(),
            )));
        }
    }
    conn.execute(
        "INSERT INTO time_intervals (id, session_id, start_utc, end_utc, start_mono_ms, end_mono_ms, continuity, closed_by,
            revision, created_at, updated_at)
         VALUES (?1, ?2, ?3, NULL, ?4, NULL, 'VERIFIED', NULL, 1, ?5, ?5)",
        params![
            id,
            session_id.as_str(),
            ms(start_utc),
            i64_from_u64(start_mono_ms, "start_mono_ms")?,
            ms(now)
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// Close an open interval exactly once. `VERIFIED` needs the monotonic end
/// reading; `UNCERTAIN` (restart recovery) has none and stores the placeholder.
#[allow(clippy::too_many_arguments)]
pub fn close_interval(
    conn: &Connection,
    id: &str,
    expected_revision: Revision,
    end_utc: UtcMillis,
    end_mono_ms: Option<u64>,
    continuity: Continuity,
    closed_by: ClosedBy,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let current = required(
        "time_intervals",
        query_opt(
            conn,
            &format!("SELECT {INTERVAL_COLUMNS} FROM time_intervals WHERE id = ?1"),
            &[&id],
            map_interval,
        )?,
    )?;
    if current.revision != expected_revision {
        return Err(RepoError::Storage(StorageError::StaleRevision {
            expected: Some(expected_revision),
            actual: current.revision,
        }));
    }
    if !current.record.is_open() {
        return Err(RepoError::Storage(StorageError::Immutable(
            "time interval is already closed; add a manual_time_corrections row".to_string(),
        )));
    }
    if end_utc < current.record.start_wall {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "end_utc before start_utc".to_string(),
        )));
    }
    let end_mono = match (continuity, end_mono_ms) {
        (Continuity::Verified, Some(m)) if m >= current.record.start_mono_ms => m,
        (Continuity::Verified, Some(_)) => {
            return Err(RepoError::Storage(StorageError::InvalidInput(
                "monotonic end before monotonic start".to_string(),
            )))
        }
        (Continuity::Verified, None) => {
            return Err(RepoError::Storage(StorageError::InvalidInput(
                "a VERIFIED closure needs the monotonic end reading".to_string(),
            )))
        }
        (Continuity::Uncertain, _) => current.record.start_mono_ms,
    };
    let changed = conn.execute(
        "UPDATE time_intervals SET end_utc = ?3, end_mono_ms = ?4, continuity = ?5, closed_by = ?6,
             revision = revision + 1, updated_at = ?7
         WHERE id = ?1 AND revision = ?2 AND end_utc IS NULL",
        params![
            id,
            rev_i64(expected_revision),
            ms(end_utc),
            i64_from_u64(end_mono, "end_mono_ms")?,
            enum_to_text(&continuity)?,
            enum_to_text(&closed_by)?,
            ms(now),
        ],
    )?;
    finish_update(conn, "time_intervals", "id", id, expected_revision, changed)
}

/// Every open interval across all sessions (restart recovery; A03).
pub fn open_intervals(conn: &Connection) -> RepoResult<Vec<TimeIntervalRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {INTERVAL_COLUMNS} FROM time_intervals WHERE end_utc IS NULL ORDER BY start_utc, id"
        ),
        &[],
        map_interval,
    )
}

/// Intervals of a session in start order.
pub fn list_intervals(
    conn: &Connection,
    session_id: &SessionId,
) -> RepoResult<Vec<TimeIntervalRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {INTERVAL_COLUMNS} FROM time_intervals WHERE session_id = ?1 ORDER BY start_utc, id"
        ),
        &[&session_id.as_str()],
        map_interval,
    )
}

/// A `manual_time_corrections` row (immutable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualTimeCorrectionRow {
    /// Row id.
    pub id: String,
    /// Session corrected.
    pub session_id: SessionId,
    /// Interval corrected (`None` for a session-level adjustment).
    pub interval_id: Option<String>,
    /// Corrected start, if the operator restated it.
    pub corrected_start_utc: Option<UtcMillis>,
    /// Corrected end, if the operator restated it.
    pub corrected_end_utc: Option<UtcMillis>,
    /// Signed change to the credited time in milliseconds.
    pub delta_ms: i64,
    /// Operator's reason (required, non-empty).
    pub reason: String,
    /// Correcting operator.
    pub corrected_by: ActorId,
    /// When.
    pub corrected_at: UtcMillis,
    /// Approval token consumed (`Capability::ProtectedRecordWrite`).
    pub approval_id: Option<ApprovalId>,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Append a correction (never updated or deleted; trigger-enforced).
pub fn append_manual_correction(
    conn: &Connection,
    row: &ManualTimeCorrectionRow,
) -> RepoResult<()> {
    super::require_non_empty("reason", &row.reason)?;
    conn.execute(
        "INSERT INTO manual_time_corrections (id, session_id, interval_id, corrected_start_utc, corrected_end_utc, delta_ms,
            reason, corrected_by, corrected_at, approval_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            row.id,
            row.session_id.as_str(),
            row.interval_id,
            opt_ms(row.corrected_start_utc),
            opt_ms(row.corrected_end_utc),
            row.delta_ms,
            row.reason,
            row.corrected_by.as_str(),
            ms(row.corrected_at),
            row.approval_id.as_ref().map(|a| a.as_str().to_string()),
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

/// Corrections of a session in the order they were recorded, oldest first: the
/// last listed correction of an interval is the effective one. Ordered by rowid
/// (insertion order of this append-only table, kept by VACUUM, VACUUM INTO and
/// the backup API), never by `corrected_at`: a correction recorded after the
/// wall clock stepped back carries an earlier stamp than the one it
/// supersedes (storage-repos-11).
pub fn list_corrections(
    conn: &Connection,
    session_id: &SessionId,
) -> RepoResult<Vec<ManualTimeCorrectionRow>> {
    query_all(
        conn,
        "SELECT id, session_id, interval_id, corrected_start_utc, corrected_end_utc, delta_ms, reason, corrected_by,
                corrected_at, approval_id, created_at
         FROM manual_time_corrections WHERE session_id = ?1 ORDER BY rowid",
        &[&session_id.as_str()],
        |r| {
            Ok(ManualTimeCorrectionRow {
                id: r.get("id")?,
                session_id: parse_id(&r.get::<_, String>("session_id")?)?,
                interval_id: r.get("interval_id")?,
                corrected_start_utc: opt_from_ms(r.get("corrected_start_utc")?),
                corrected_end_utc: opt_from_ms(r.get("corrected_end_utc")?),
                delta_ms: r.get("delta_ms")?,
                reason: r.get("reason")?,
                corrected_by: parse_id(&r.get::<_, String>("corrected_by")?)?,
                corrected_at: from_ms(r.get("corrected_at")?),
                approval_id: opt_parse_id(r.get("approval_id")?)?,
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::testutil::{assert_immutable, db, learning, operator, operator_id, t};

    fn session(id: &str, state: SessionState, domain: DataDomain) -> StudySessionRow {
        StudySessionRow {
            id: SessionId::parse(id).unwrap(),
            training_day_id: None,
            objective_id: None,
            study_block_id: None,
            calendar_event_id: None,
            state,
            mode: InteractionMode::Study,
            zone: IanaZone::parse("Europe/Berlin").unwrap(),
            planned_start_utc: Some(t(3_600_000)),
            planned_end_utc: Some(t(8 * 3_600_000)),
            planned_minutes: 420,
            started_at: None,
            ended_at: None,
            credited_ms: DurationMs::ZERO,
            uncertain_ms: DurationMs::ZERO,
            data_domain: domain,
            classification: Classification::Private,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        }
    }

    fn edge(
        session: &str,
        event: SessionEvent,
        from: SessionState,
        to: SessionState,
        at: UtcMillis,
    ) -> SessionEventRow {
        SessionEventRow {
            seq: 0,
            session_id: SessionId::parse(session).unwrap(),
            event,
            from,
            to,
            actor: operator(),
            occurred_at: at,
        }
    }

    #[test]
    fn session_roundtrip_transitions_and_revision_conflict() {
        let db = db();
        let conn = db.conn();
        let row = session("sess-1", SessionState::Planned, DataDomain::Learning);
        insert_session(conn, &row).unwrap();
        assert_eq!(
            get_session(conn, &learning(), &row.id).unwrap().unwrap(),
            row
        );

        let r2 = transition_session(
            conn,
            "se-1",
            &edge(
                "sess-1",
                SessionEvent::Prepare,
                SessionState::Planned,
                SessionState::Ready,
                t(1),
            ),
            None,
            Revision::INITIAL,
        )
        .unwrap();
        assert_eq!(r2, Revision::new(2));
        // stale revision
        let stale = transition_session(
            conn,
            "se-x",
            &edge(
                "sess-1",
                SessionEvent::Start,
                SessionState::Ready,
                SessionState::Running,
                t(2),
            ),
            None,
            Revision::INITIAL,
        )
        .unwrap_err();
        assert!(matches!(
            stale,
            RepoError::Storage(StorageError::StaleRevision { actual, .. }) if actual == Revision::new(2)
        ));
        // wrong `from`
        assert!(matches!(
            transition_session(
                conn,
                "se-x",
                &edge(
                    "sess-1",
                    SessionEvent::Start,
                    SessionState::Planned,
                    SessionState::Running,
                    t(2)
                ),
                None,
                r2
            ),
            Err(RepoError::Storage(StorageError::Conflict(_)))
        ));
        // illegal edge
        assert!(matches!(
            transition_session(
                conn,
                "se-x",
                &edge(
                    "sess-1",
                    SessionEvent::Resume,
                    SessionState::Ready,
                    SessionState::Running,
                    t(2)
                ),
                None,
                r2
            ),
            Err(RepoError::Storage(StorageError::InvalidTransition(_)))
        ));
        // inconsistent `to`
        assert!(matches!(
            transition_session(
                conn,
                "se-x",
                &edge(
                    "sess-1",
                    SessionEvent::Start,
                    SessionState::Ready,
                    SessionState::Paused,
                    t(2)
                ),
                None,
                r2
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        let r3 = transition_session(
            conn,
            "se-2",
            &edge(
                "sess-1",
                SessionEvent::Start,
                SessionState::Ready,
                SessionState::Running,
                t(2),
            ),
            Some(&serde_json::json!({ "interval_id": "ti-1" })),
            r2,
        )
        .unwrap();
        let running = get_session(conn, &learning(), &row.id).unwrap().unwrap();
        assert_eq!(running.state, SessionState::Running);
        assert_eq!(running.started_at, Some(t(2)));
        assert_eq!(running.revision, r3);

        let events = list_session_events(conn, &row.id).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].row.seq, 0);
        assert_eq!(events[1].row.seq, 1);
        assert_eq!(events[1].row.event, SessionEvent::Start);
        assert_eq!(
            events[1].details,
            Some(serde_json::json!({ "interval_id": "ti-1" }))
        );
        let record = load_session_record(conn, &learning(), &row.id)
            .unwrap()
            .unwrap();
        assert_eq!(record.state, SessionState::Running);
        assert_eq!(record.revision, r3);
        assert_eq!(record.events.len(), 2);

        // the history is immutable
        assert_immutable(
            conn,
            "UPDATE session_events SET to_state = 'CLOSED' WHERE id = 'se-1'",
        );
        assert_immutable(conn, "DELETE FROM session_events WHERE id = 'se-1'");

        // skill actors are refused
        let mut skill_edge = edge(
            "sess-1",
            SessionEvent::Pause,
            SessionState::Running,
            SessionState::Paused,
            t(3),
        );
        skill_edge.actor = Actor::skill(
            ActorId::parse("skill-1").unwrap(),
            g9_domain::SkillId::parse("g9-session-director").unwrap(),
        );
        assert!(matches!(
            transition_session(conn, "se-3", &skill_edge, None, r3),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        assert_eq!(list_session_events(conn, &row.id).unwrap().len(), 2);

        let r4 = update_session_credit(
            conn,
            &row.id,
            r3,
            DurationMs::from_minutes_u32(10),
            DurationMs::ZERO,
            t(4),
        )
        .unwrap();
        assert_eq!(
            get_session(conn, &learning(), &row.id)
                .unwrap()
                .unwrap()
                .credited_ms,
            DurationMs::from_minutes_u32(10)
        );
        let patch = SessionPlanPatch {
            training_day_id: None,
            objective_id: None,
            study_block_id: None,
            calendar_event_id: None,
            mode: InteractionMode::Lab,
            zone: IanaZone::parse("UTC").unwrap(),
            planned_start_utc: None,
            planned_end_utc: None,
            planned_minutes: 60,
        };
        let r5 = update_session_plan(conn, &row.id, r4, &patch, t(5)).unwrap();
        let planned = get_session(conn, &learning(), &row.id).unwrap().unwrap();
        assert_eq!(planned.mode, InteractionMode::Lab);
        assert_eq!(planned.planned_minutes, 60);
        assert_eq!(planned.revision, r5);
        assert_eq!(
            list_sessions(conn, &learning(), Some(SessionState::Running))
                .unwrap()
                .len(),
            1
        );
        assert!(list_sessions(conn, &learning(), Some(SessionState::Closed))
            .unwrap()
            .is_empty());
        assert_eq!(list_sessions(conn, &learning(), None).unwrap().len(), 1);
    }

    #[test]
    fn cross_domain_session_reads_are_refused_without_capability() {
        let db = db();
        let conn = db.conn();
        insert_session(
            conn,
            &session("sess-b", SessionState::Planned, DataDomain::Business),
        )
        .unwrap();
        let id = SessionId::parse("sess-b").unwrap();
        assert!(matches!(
            get_session(conn, &learning(), &id),
            Err(RepoError::CrossDomainRefused {
                table: "study_sessions",
                ..
            })
        ));
        assert!(load_session_record(conn, &learning(), &id).is_err());
        assert!(list_sessions(conn, &learning(), None).unwrap().is_empty());
        assert_eq!(
            list_sessions(conn, &ReadScope::cross_domain(DataDomain::Learning), None)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn interval_ledger_appends_refuses_overlap_and_recovers_open_intervals() {
        let db = db();
        let conn = db.conn();
        insert_session(
            conn,
            &session("sess-1", SessionState::Running, DataDomain::Learning),
        )
        .unwrap();
        insert_session(
            conn,
            &session("sess-2", SessionState::Running, DataDomain::Learning),
        )
        .unwrap();
        let s1 = SessionId::parse("sess-1").unwrap();
        let s2 = SessionId::parse("sess-2").unwrap();

        open_interval(conn, "ti-1", &s1, t(0), 1_000, t(0)).unwrap();
        // one focused interval at a time, in any session
        assert!(matches!(
            open_interval(conn, "ti-2", &s2, t(10), 2_000, t(10)),
            Err(RepoError::Storage(StorageError::Conflict(_)))
        ));
        assert!(matches!(
            close_interval(
                conn,
                "ti-1",
                Revision::INITIAL,
                t(600_000),
                None,
                Continuity::Verified,
                ClosedBy::Operator,
                t(600_000)
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        assert!(matches!(
            close_interval(
                conn,
                "ti-1",
                Revision::new(3),
                t(600_000),
                Some(601_000),
                Continuity::Verified,
                ClosedBy::Operator,
                t(600_000)
            ),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        let r2 = close_interval(
            conn,
            "ti-1",
            Revision::INITIAL,
            t(600_000),
            Some(601_000),
            Continuity::Verified,
            ClosedBy::Operator,
            t(600_000),
        )
        .unwrap();
        assert_eq!(r2, Revision::new(2));
        // closing twice is refused
        assert!(matches!(
            close_interval(
                conn,
                "ti-1",
                r2,
                t(700_000),
                Some(701_000),
                Continuity::Verified,
                ClosedBy::Operator,
                t(700_000)
            ),
            Err(RepoError::Storage(StorageError::Immutable(_)))
        ));
        // a start backdated into the closed interval (starts before it ended) overlaps
        assert!(matches!(
            open_interval(conn, "ti-2", &s1, t(300_000), 700_000, t(700_000)),
            Err(RepoError::Storage(StorageError::Conflict(_)))
        ));
        let closed = list_intervals(conn, &s1).unwrap();
        assert_eq!(closed.len(), 1);
        assert_eq!(
            closed[0].record.mono_elapsed(),
            Some(DurationMs::new(600_000).unwrap())
        );
        assert_eq!(closed[0].closed_by, Some(ClosedBy::Operator));
        assert_eq!(closed[0].record.continuity, Continuity::Verified);

        // the trigger refuses re-timing and deletion of the ledger
        assert_immutable(
            conn,
            "UPDATE time_intervals SET start_utc = 5 WHERE id = 'ti-1'",
        );
        assert_immutable(conn, "DELETE FROM time_intervals WHERE id = 'ti-1'");

        // simulate a crash: an interval left open, then process restart
        open_interval(conn, "ti-3", &s2, t(900_000), 10_000, t(900_000)).unwrap();
        let open = open_intervals(conn).unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].id, "ti-3");
        assert!(open[0].record.is_open());
        assert_eq!(open[0].record.session_id, s2);
        let r = close_interval(
            conn,
            "ti-3",
            Revision::INITIAL,
            t(1_000_000),
            None,
            Continuity::Uncertain,
            ClosedBy::Recovery,
            t(1_000_000),
        )
        .unwrap();
        assert_eq!(r, Revision::new(2));
        assert!(open_intervals(conn).unwrap().is_empty());
        let recovered = &list_intervals(conn, &s2).unwrap()[0];
        assert_eq!(recovered.record.continuity, Continuity::Uncertain);
        assert_eq!(recovered.record.end_mono_ms, None);
        assert_eq!(recovered.record.end_wall, Some(t(1_000_000)));
        assert_eq!(recovered.closed_by, Some(ClosedBy::Recovery));

        // corrections are appended, never updated
        let correction = ManualTimeCorrectionRow {
            id: "mtc-1".into(),
            session_id: s2.clone(),
            interval_id: Some("ti-3".into()),
            corrected_start_utc: None,
            corrected_end_utc: None,
            delta_ms: 60_000,
            reason: "worked through the reboot".into(),
            corrected_by: operator_id(),
            corrected_at: t(1_100_000),
            approval_id: None,
            created_at: t(1_100_000),
        };
        assert!(matches!(
            append_manual_correction(
                conn,
                &ManualTimeCorrectionRow {
                    reason: "  ".into(),
                    ..correction.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        append_manual_correction(conn, &correction).unwrap();
        assert_eq!(list_corrections(conn, &s2).unwrap(), vec![correction]);
        assert_immutable(
            conn,
            "UPDATE manual_time_corrections SET delta_ms = 0 WHERE id = 'mtc-1'",
        );
        assert_immutable(
            conn,
            "DELETE FROM manual_time_corrections WHERE id = 'mtc-1'",
        );
    }

    #[test]
    fn restart_recovery_interrupts_the_running_session_and_keeps_the_ledger() {
        let db = db();
        let conn = db.conn();
        let row = session("sess-r", SessionState::Ready, DataDomain::Learning);
        insert_session(conn, &row).unwrap();
        let r2 = transition_session(
            conn,
            "se-1",
            &edge(
                "sess-r",
                SessionEvent::Start,
                SessionState::Ready,
                SessionState::Running,
                t(0),
            ),
            None,
            Revision::INITIAL,
        )
        .unwrap();
        open_interval(conn, "ti-1", &row.id, t(0), 5_000, t(0)).unwrap();

        // --- process restart: only the database survives ---
        let open = open_intervals(conn).unwrap();
        assert_eq!(open.len(), 1);
        let interval = &open[0];
        assert_eq!(interval.record.session_id, row.id);
        close_interval(
            conn,
            &interval.id,
            interval.revision,
            t(3_600_000),
            None,
            Continuity::Uncertain,
            ClosedBy::Recovery,
            t(3_600_000),
        )
        .unwrap();
        let system = Actor::system(ActorId::parse("system-recovery").unwrap());
        let mut interrupt = edge(
            "sess-r",
            SessionEvent::Interrupt,
            SessionState::Running,
            SessionState::Interrupted,
            t(3_600_000),
        );
        interrupt.actor = system.clone();
        let r3 = transition_session(
            conn,
            "se-2",
            &interrupt,
            Some(&serde_json::json!({ "interval_id": "ti-1" })),
            r2,
        )
        .unwrap();
        let r4 = update_session_credit(
            conn,
            &row.id,
            r3,
            DurationMs::ZERO,
            DurationMs::new(3_600_000).unwrap(),
            t(3_600_000),
        )
        .unwrap();

        // final backend state
        assert!(open_intervals(conn).unwrap().is_empty());
        let stored = get_session(conn, &learning(), &row.id).unwrap().unwrap();
        assert_eq!(stored.state, SessionState::Interrupted);
        assert_eq!(stored.ended_at, Some(t(3_600_000)));
        assert_eq!(stored.started_at, Some(t(0)));
        assert_eq!(stored.uncertain_ms, DurationMs::new(3_600_000).unwrap());
        assert_eq!(stored.credited_ms, DurationMs::ZERO);
        assert_eq!(stored.revision, r4);
        let events = list_session_events(conn, &row.id).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[1].row.actor, system);
        assert_eq!(events[1].row.to, SessionState::Interrupted);
        let ledger = list_intervals(conn, &row.id).unwrap();
        assert_eq!(ledger[0].record.continuity, Continuity::Uncertain);
        assert_eq!(ledger[0].closed_by, Some(ClosedBy::Recovery));

        // recovery resumes to PAUSED; ended_at is cleared
        let mut recover = edge(
            "sess-r",
            SessionEvent::Recover,
            SessionState::Interrupted,
            SessionState::Paused,
            t(3_700_000),
        );
        recover.actor = system;
        let r5 = transition_session(conn, "se-3", &recover, None, r4).unwrap();
        let resumed = get_session(conn, &learning(), &row.id).unwrap().unwrap();
        assert_eq!(
            (resumed.state, resumed.ended_at, resumed.revision),
            (SessionState::Paused, None, r5)
        );
        // the next interval may not start inside the recovered one
        assert!(matches!(
            open_interval(conn, "ti-2", &row.id, t(1_800_000), 6_000, t(3_700_000)),
            Err(RepoError::Storage(StorageError::Conflict(_)))
        ));
        open_interval(conn, "ti-2", &row.id, t(3_700_000), 6_000, t(3_700_000)).unwrap();
        assert_eq!(list_intervals(conn, &row.id).unwrap().len(), 2);
    }

    #[test]
    fn intervals_of_different_sessions_cannot_overlap_and_terminal_sessions_keep_their_plan() {
        let db = db();
        let conn = db.conn();
        let a = session("sess-a", SessionState::Running, DataDomain::Learning);
        let b = session("sess-b", SessionState::Running, DataDomain::Learning);
        insert_session(conn, &a).unwrap();
        insert_session(conn, &b).unwrap();
        open_interval(conn, "ti-a", &a.id, t(0), 0, t(0)).unwrap();
        close_interval(
            conn,
            "ti-a",
            Revision::INITIAL,
            t(600_000),
            Some(600_000),
            Continuity::Verified,
            ClosedBy::Operator,
            t(600_000),
        )
        .unwrap();
        // a backdated start in another session would double-count the learner's time
        assert!(matches!(
            open_interval(conn, "ti-b", &b.id, t(300_000), 700_000, t(700_000)),
            Err(RepoError::Storage(StorageError::Conflict(m))) if m.contains("overlap")
        ));
        open_interval(conn, "ti-b", &b.id, t(600_000), 700_000, t(700_000)).unwrap();
        assert_eq!(open_intervals(conn).unwrap().len(), 1);
        assert_eq!(count_rows(conn), 2);

        // study_block_id round-trips (IPC block/block_seq) and must name a real block
        let zeros = "0".repeat(64);
        conn.execute_batch(&format!(
            "INSERT INTO curricula (id, title, created_at, updated_at) VALUES ('cur', 'G9', 1, 1);
             INSERT INTO curriculum_versions (id, curriculum_id, version_label, provenance, manifest_sha256, source_ref,
                 invariants_json, imported_by, imported_at, created_at, updated_at)
               VALUES ('cv', 'cur', 'seed', 'PROPOSED_SEED', '{zeros}', 'seed', '{{}}', 'operator-1', 1, 1, 1);
             INSERT INTO weeks (id, curriculum_version_id, week_number, title, planned_minutes, created_at)
               VALUES ('w1', 'cv', 1, 'W1', 2400, 1);
             INSERT INTO training_days (id, week_id, curriculum_version_id, day_code, day_number, day_of_week,
                 planned_minutes, title, created_at)
               VALUES ('d01', 'w1', 'cv', 'D01', 1, 1, 420, 'D01', 1);
             INSERT INTO study_blocks (id, training_day_id, ordinal, block_code, minutes, mode, created_at)
               VALUES ('blk-2', 'd01', 2, 'theory', 60, 'STUDY', 1);"
        ))
        .unwrap();
        let mut d = session("sess-d", SessionState::Planned, DataDomain::Learning);
        d.training_day_id = Some("d01".into());
        d.study_block_id = Some("blk-2".into());
        insert_session(conn, &d).unwrap();
        assert_eq!(get_session(conn, &learning(), &d.id).unwrap().unwrap(), d);
        assert_eq!(
            list_sessions_for_day(conn, &learning(), "d01").unwrap(),
            vec![d.clone()]
        );
        assert!(
            list_sessions_for_day(conn, &ReadScope::domain(DataDomain::Business), "d01")
                .unwrap()
                .is_empty()
        );
        let block = crate::repos::curriculum::get_study_block(conn, "blk-2")
            .unwrap()
            .unwrap();
        assert_eq!((block.block_code.as_str(), block.ordinal), ("theory", 2));
        let mut dangling = session("sess-x", SessionState::Planned, DataDomain::Learning);
        dangling.study_block_id = Some("blk-missing".into());
        assert!(matches!(
            insert_session(conn, &dangling),
            Err(RepoError::Storage(StorageError::ForeignKey(_)))
        ));

        let c = session("sess-c", SessionState::Closed, DataDomain::Learning);
        insert_session(conn, &c).unwrap();
        let patch = SessionPlanPatch {
            training_day_id: None,
            objective_id: None,
            study_block_id: None,
            calendar_event_id: None,
            mode: InteractionMode::Lab,
            zone: IanaZone::parse("UTC").unwrap(),
            planned_start_utc: None,
            planned_end_utc: None,
            planned_minutes: 1,
        };
        // a CLOSED session's plan is history
        assert!(matches!(
            update_session_plan(conn, &c.id, Revision::INITIAL, &patch, t(1)),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        assert_eq!(
            get_session(conn, &learning(), &c.id)
                .unwrap()
                .unwrap()
                .planned_minutes,
            420
        );
        // revision check on credit updates
        assert!(matches!(
            update_session_credit(
                conn,
                &a.id,
                Revision::new(9),
                DurationMs::ZERO,
                DurationMs::ZERO,
                t(1)
            ),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        assert!(matches!(
            update_session_credit(
                conn,
                &SessionId::parse("missing").unwrap(),
                Revision::INITIAL,
                DurationMs::ZERO,
                DurationMs::ZERO,
                t(1)
            ),
            Err(RepoError::Storage(StorageError::NotFound("study_sessions")))
        ));
    }

    #[test]
    fn wall_clock_regression_never_locks_the_timer() {
        let db = db();
        let conn = db.conn();
        let a = session("sess-a", SessionState::Running, DataDomain::Learning);
        let b = session("sess-b", SessionState::Running, DataDomain::Learning);
        insert_session(conn, &a).unwrap();
        insert_session(conn, &b).unwrap();
        let year_ms: i64 = 365 * 24 * 3_600_000;

        // session A runs while the wall clock is wrongly one year ahead
        open_interval(conn, "ti-a", &a.id, t(year_ms), 1_000, t(year_ms)).unwrap();
        close_interval(
            conn,
            "ti-a",
            Revision::INITIAL,
            t(year_ms + 600_000),
            Some(601_000),
            Continuity::Verified,
            ClosedBy::Operator,
            t(year_ms + 600_000),
        )
        .unwrap();

        // the clock is corrected: another session's timer still starts and stops
        open_interval(conn, "ti-b1", &b.id, t(0), 700_000, t(0)).unwrap();
        // the single-open-interval rule still prevents any overlap of focused time
        assert!(matches!(
            open_interval(conn, "ti-a2", &a.id, t(10), 701_000, t(10)),
            Err(RepoError::Storage(StorageError::Conflict(m))) if m.contains("already_running")
        ));
        close_interval(
            conn,
            "ti-b1",
            Revision::INITIAL,
            t(60_000),
            Some(760_000),
            Continuity::Verified,
            ClosedBy::Operator,
            t(60_000),
        )
        .unwrap();

        // an NTP step backwards (30 s) between pause and resume is not a conflict
        open_interval(conn, "ti-b2", &b.id, t(30_000), 790_000, t(30_000)).unwrap();
        assert_eq!(open_intervals(conn).unwrap().len(), 1);
        assert_eq!(count_rows(conn), 3);
        // credit stays monotonic: the earlier intervals keep their elapsed time
        let ledger = list_intervals(conn, &b.id).unwrap();
        assert_eq!(
            ledger[0].record.mono_elapsed(),
            Some(DurationMs::new(60_000).unwrap())
        );
        assert_eq!(
            list_intervals(conn, &a.id).unwrap()[0]
                .record
                .mono_elapsed(),
            Some(DurationMs::new(600_000).unwrap())
        );
    }

    #[test]
    fn closed_intervals_cannot_be_moved_relabelled_or_reidentified() {
        let db = db();
        let conn = db.conn();
        let a = session("sess-a", SessionState::Running, DataDomain::Learning);
        let b = session("sess-b", SessionState::Running, DataDomain::Learning);
        insert_session(conn, &a).unwrap();
        insert_session(conn, &b).unwrap();
        open_interval(conn, "ti-1", &a.id, t(0), 0, t(0)).unwrap();
        // an open interval keeps its identity, session and start
        assert_immutable(
            conn,
            "UPDATE time_intervals SET session_id = 'sess-b' WHERE id = 'ti-1'",
        );
        assert_immutable(
            conn,
            "UPDATE time_intervals SET start_mono_ms = 5 WHERE id = 'ti-1'",
        );
        close_interval(
            conn,
            "ti-1",
            Revision::INITIAL,
            t(3_600_000),
            Some(3_600_000),
            Continuity::Verified,
            ClosedBy::Recovery,
            t(3_600_000),
        )
        .unwrap();
        // credited time cannot move to another session; the closer cannot be relabelled
        assert_immutable(
            conn,
            "UPDATE time_intervals SET session_id = 'sess-b', closed_by = 'SYSTEM' WHERE id = 'ti-1'",
        );
        assert_immutable(
            conn,
            "UPDATE time_intervals SET session_id = 'sess-b' WHERE id = 'ti-1'",
        );
        assert_immutable(
            conn,
            "UPDATE time_intervals SET closed_by = 'SYSTEM' WHERE id = 'ti-1'",
        );
        assert_immutable(
            conn,
            "UPDATE time_intervals SET created_at = 99 WHERE id = 'ti-1'",
        );
        assert_immutable(
            conn,
            "UPDATE time_intervals SET id = 'ti-x' WHERE id = 'ti-1'",
        );
        assert_immutable(
            conn,
            "UPDATE time_intervals SET continuity = 'UNCERTAIN' WHERE id = 'ti-1'",
        );
        let row = &list_intervals(conn, &a.id).unwrap()[0];
        assert_eq!(row.closed_by, Some(ClosedBy::Recovery));
        assert_eq!(row.record.session_id, a.id);
        assert!(list_intervals(conn, &b.id).unwrap().is_empty());
        // revision bookkeeping is the only change a closed row accepts
        conn.execute(
            "UPDATE time_intervals SET revision = revision + 1, updated_at = 4000000 WHERE id = 'ti-1'",
            [],
        )
        .unwrap();
    }

    fn count_rows(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM time_intervals", [], |r| r.get(0))
            .unwrap()
    }

    /// storage-repos-11: corrections and session events are append-only
    /// ledgers whose order is the order they were written. After the wall clock
    /// steps back, a later correction carries an earlier `corrected_at` than the
    /// one it supersedes; it must still be listed last (the timer ledger takes
    /// the last listed correction of an interval as the effective one). The
    /// order survives VACUUM and a VACUUM INTO copy (the backup format).
    #[test]
    fn append_only_ledgers_are_listed_in_insertion_order_not_wall_clock_order() {
        let db = db();
        let conn = db.conn();
        let row = session("sess-step", SessionState::Planned, DataDomain::Learning);
        let sid = row.id.clone();
        insert_session(conn, &row).unwrap();
        let r2 = transition_session(
            conn,
            "se-z-prepare",
            &edge(
                "sess-step",
                SessionEvent::Prepare,
                SessionState::Planned,
                SessionState::Ready,
                t(5_000_000),
            ),
            None,
            Revision::INITIAL,
        )
        .unwrap();
        // the wall clock stepped back an hour before the start
        transition_session(
            conn,
            "se-a-start",
            &edge(
                "sess-step",
                SessionEvent::Start,
                SessionState::Ready,
                SessionState::Running,
                t(5_000_000 - 3_600_000),
            ),
            None,
            r2,
        )
        .unwrap();
        open_interval(conn, "ti-step", &sid, t(0), 1_000, t(0)).unwrap();
        close_interval(
            conn,
            "ti-step",
            Revision::INITIAL,
            t(3_600_000),
            None,
            Continuity::Uncertain,
            ClosedBy::Recovery,
            t(3_600_000),
        )
        .unwrap();
        let correction = |id: &str, minutes: i64, at: i64| ManualTimeCorrectionRow {
            id: id.into(),
            session_id: sid.clone(),
            interval_id: Some("ti-step".into()),
            corrected_start_utc: None,
            corrected_end_utc: None,
            delta_ms: minutes * 60_000,
            reason: format!("{minutes} minutes"),
            corrected_by: operator_id(),
            corrected_at: t(at),
            approval_id: None,
            created_at: t(at),
        };
        let first = correction("mtc-z-first", 30, 10_000_000);
        // recorded later, stamped earlier: the clock was set back an hour
        let second = correction("mtc-a-second", 10, 10_000_000 - 3_600_000);
        append_manual_correction(conn, &first).unwrap();
        append_manual_correction(conn, &second).unwrap();

        let check = |conn: &Connection, what: &str| {
            let ids: Vec<String> = list_corrections(conn, &sid)
                .unwrap()
                .into_iter()
                .map(|c| c.id)
                .collect();
            assert_eq!(ids, ["mtc-z-first", "mtc-a-second"], "{what}");
            let events: Vec<(String, u32)> = list_session_events(conn, &sid)
                .unwrap()
                .into_iter()
                .map(|e| (e.id, e.row.seq))
                .collect();
            assert_eq!(
                events,
                [
                    ("se-z-prepare".to_string(), 0),
                    ("se-a-start".to_string(), 1)
                ],
                "{what}"
            );
        };
        check(conn, "as written");
        conn.execute_batch("VACUUM").unwrap();
        check(conn, "after VACUUM");
        let dir = tempfile::tempdir().unwrap();
        let copy = dir.path().join("copy.sqlite");
        conn.execute("VACUUM INTO ?1", rusqlite::params![copy.to_str().unwrap()])
            .unwrap();
        check(&Connection::open(&copy).unwrap(), "in a VACUUM INTO copy");
    }
}
