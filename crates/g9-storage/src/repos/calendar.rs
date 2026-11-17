//! Calendar repository (M03; migration `0004_calendar`, tables `calendar_events`,
//! `recurrence_rules`, `schedule_proposals`, `schedule_decisions`).
//!
//! * Events are mutable rows with a revision check; the wall-clock intent is
//!   stored as civil start/end plus IANA zone and the resolved occurrence as
//!   `start_utc`/`end_utc` (resolution is `g9_calendar::resolve_local`, done by
//!   the caller so DST policy stays in one crate).
//! * A recurrence rule is one row per event (`FREQ=DAILY|WEEKLY`, `INTERVAL`,
//!   `BYDAY`, `UNTIL`/`COUNT`).
//! * Proposals are appended by the scheduler and only ever change state
//!   (`PROPOSED → APPLIED | REJECTED | SUPERSEDED`); decisions are immutable rows.
//!
//! DDL ↔ wave-1 gaps (reported, adapters refuse rather than guess):
//! `calendar_events.kind` lacks `INTERVENTION` and `COMMITMENT` and has `OTHER`
//! ([`CalendarEventKind`]); `recurrence_rules.freq` lacks `MONTHLY`.

use super::{
    bool_i64, finish_update, from_json, from_ms, i64_bool, ms, opt_parse_id, query_all, query_opt,
    required, rev_from_i64, rev_i64, to_json, u32_from_i64, ReadScope, RepoError, RepoResult,
};
use crate::error::{StorageError, StorageResult};
use crate::sql::{enum_from_text, enum_to_text, parse_id};
use g9_calendar::{
    Adjustment, CalendarEvent, CivilDate, ConstraintViolation, EventKind, Feasibility, Frequency,
    RecurrenceRule, ScheduleProposal, Weekday,
};
use g9_domain::{
    ActorId, ApprovalId, CalendarEventId, CivilDateTime, Classification, DataDomain, DurationMs,
    IanaZone, Provenance, Revision, ScheduleProposalId, TrustClass, UtcMillis,
};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

/// Storage-level event kind (mirrors the `calendar_events.kind` CHECK list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CalendarEventKind {
    /// Curriculum study block.
    StudyBlock,
    /// External appointment.
    Appointment,
    /// Protected rest (hard constraint).
    ProtectedRest,
    /// Assessment window or deadline.
    Assessment,
    /// Retention review slot.
    Review,
    /// Anything else the DDL cannot name.
    Other,
}

impl CalendarEventKind {
    /// Map the calendar crate's kind; `INTERVENTION`/`COMMITMENT` have no column
    /// value yet (gap) and are refused instead of being folded into `OTHER`.
    pub fn from_domain(kind: EventKind) -> StorageResult<Self> {
        Ok(match kind {
            EventKind::StudyBlock => Self::StudyBlock,
            EventKind::Appointment => Self::Appointment,
            EventKind::ProtectedRest => Self::ProtectedRest,
            EventKind::Assessment => Self::Assessment,
            EventKind::Review => Self::Review,
            EventKind::Intervention | EventKind::Commitment => {
                return Err(StorageError::NotImplemented("calendar.event_kind"))
            }
        })
    }

    /// The calendar crate's kind, if representable (`OTHER` is not).
    pub const fn to_domain(self) -> Option<EventKind> {
        Some(match self {
            Self::StudyBlock => EventKind::StudyBlock,
            Self::Appointment => EventKind::Appointment,
            Self::ProtectedRest => EventKind::ProtectedRest,
            Self::Assessment => EventKind::Assessment,
            Self::Review => EventKind::Review,
            Self::Other => return None,
        })
    }
}

/// Who created an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventSource {
    /// Entered by the operator.
    Operator,
    /// Placed by the deterministic scheduler (applied proposal).
    Scheduler,
    /// Imported.
    Import,
}

/// A `calendar_events` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalendarEventRow {
    /// Event id.
    pub id: CalendarEventId,
    /// Operator-facing title (private content; never placed in errors).
    pub title: String,
    /// Kind.
    pub kind: CalendarEventKind,
    /// Civil (wall-clock) start.
    pub civil_start: CivilDateTime,
    /// Civil end.
    pub civil_end: CivilDateTime,
    /// Zone of the civil fields.
    pub zone: IanaZone,
    /// Resolved start instant.
    pub start_utc: UtcMillis,
    /// Resolved end instant.
    pub end_utc: UtcMillis,
    /// Date-only event.
    pub all_day: bool,
    /// Hard constraint for the scheduler.
    pub immutable: bool,
    /// Protected (rest) flag.
    pub protected: bool,
    /// Origin.
    pub source: EventSource,
    /// Linked training-day row.
    pub training_day_id: Option<String>,
    /// Linked objective row.
    pub objective_id: Option<String>,
    /// Linked curriculum block (`study_blocks.id`, migration `0010`; IPC `block`).
    pub study_block_id: Option<String>,
    /// DST adjustment recorded when the civil start was resolved (IPC `adjusted`).
    pub dst_adjustment: Option<DstAdjustment>,
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

/// Render a civil date-time as `YYYY-MM-DDTHH:MM:SS` (zone stored separately).
pub fn format_civil(c: &CivilDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        c.year, c.month, c.day, c.hour, c.minute, c.second
    )
}

/// Parse the `format_civil` rendering.
pub fn parse_civil(text: &str) -> StorageResult<CivilDateTime> {
    let bad = || StorageError::RowDecode("civil date-time column".to_string());
    let (date, time) = text.split_once('T').ok_or_else(bad)?;
    let mut d = date.split('-');
    let mut t = time.split(':');
    let year: i32 = d.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let month: u8 = d.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let day: u8 = d.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let hour: u8 = t.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let minute: u8 = t.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let second: u8 = t.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    if d.next().is_some() || t.next().is_some() {
        return Err(bad());
    }
    CivilDateTime::new(year, month, day, hour, minute, second).map_err(|_| bad())
}

fn format_civil_date(d: &CivilDate) -> String {
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

fn parse_civil_date(text: &str) -> StorageResult<CivilDate> {
    let bad = || StorageError::RowDecode("civil date column".to_string());
    let mut parts = text.split('-');
    let year: i32 = parts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let month: u8 = parts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let day: u8 = parts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    if parts.next().is_some() {
        return Err(bad());
    }
    CivilDate::new(year, month, day).map_err(|_| bad())
}

/// Civil end = civil start + duration (naive wall-clock arithmetic, no zone).
fn civil_end_of(start: &CivilDateTime, duration: DurationMs) -> StorageResult<CivilDateTime> {
    let naive = start
        .to_naive()
        .ok_or_else(|| StorageError::InvalidInput("invalid civil start".to_string()))?;
    let delta = chrono_delta(duration.as_millis())?;
    naive
        .checked_add_signed(delta)
        .map(CivilDateTime::from_naive)
        .ok_or_else(|| StorageError::InvalidInput("civil end out of range".to_string()))
}

fn chrono_delta(ms: i64) -> StorageResult<chrono::TimeDelta> {
    chrono::TimeDelta::try_milliseconds(ms)
        .ok_or_else(|| StorageError::InvalidInput("duration out of range".to_string()))
}

/// DST adjustment flag stored in `calendar_events.dst_adjustment` (ARCHITECTURE §7):
/// the tag of `g9_calendar::Adjustment` without its payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DstAdjustment {
    /// Spring-forward gap: the requested wall time did not exist.
    Gap,
    /// Fall-back overlap: the earlier instant was chosen.
    Overlap,
}

impl DstAdjustment {
    /// Flag for a resolution result (`None` when no adjustment took place).
    pub fn from_adjustment(adjustment: &Adjustment) -> Option<Self> {
        match adjustment {
            Adjustment::None => None,
            Adjustment::Gap { .. } => Some(DstAdjustment::Gap),
            Adjustment::Overlap { .. } => Some(DstAdjustment::Overlap),
        }
    }
}

impl CalendarEventRow {
    /// Build a row from a calendar-crate event plus the resolved occurrence
    /// instants (`g9_calendar::resolve_local` on start and end) and the storage
    /// attributes the domain type does not carry.
    #[allow(clippy::too_many_arguments)]
    pub fn from_domain(
        event: &CalendarEvent,
        start_utc: UtcMillis,
        end_utc: UtcMillis,
        source: EventSource,
        protected: bool,
        classification: Classification,
        now: UtcMillis,
    ) -> StorageResult<Self> {
        Ok(Self {
            id: event.id.clone(),
            title: event.title.clone(),
            kind: CalendarEventKind::from_domain(event.kind)?,
            civil_start: event.start,
            civil_end: civil_end_of(&event.start, event.duration)?,
            zone: event.zone.clone(),
            start_utc,
            end_utc,
            all_day: event.all_day,
            immutable: event.immutable,
            protected,
            source,
            training_day_id: None,
            objective_id: None,
            study_block_id: None,
            dst_adjustment: None,
            data_domain: event.data_domain,
            classification,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        })
    }

    /// The calendar-crate view of the row (`rule` supplied by the caller from
    /// [`get_rule`]). Refused for `OTHER` kinds, which the crate cannot express.
    pub fn to_domain(&self, rule: Option<RecurrenceRule>) -> StorageResult<CalendarEvent> {
        let kind = self
            .kind
            .to_domain()
            .ok_or(StorageError::NotImplemented("calendar.event_kind.other"))?;
        let start = self
            .civil_start
            .to_naive()
            .ok_or_else(|| StorageError::RowDecode("civil_start".to_string()))?;
        let end = self
            .civil_end
            .to_naive()
            .ok_or_else(|| StorageError::RowDecode("civil_end".to_string()))?;
        let duration = DurationMs::new(end.signed_duration_since(start).num_milliseconds())
            .map_err(|_| StorageError::RowDecode("civil_end before civil_start".to_string()))?;
        Ok(CalendarEvent {
            id: self.id.clone(),
            title: self.title.clone(),
            kind,
            immutable: self.immutable,
            start: self.civil_start,
            duration,
            zone: self.zone.clone(),
            all_day: self.all_day,
            rule,
            data_domain: self.data_domain,
        })
    }
}

const EVENT_COLUMNS: &str = "id, title, kind, civil_start, civil_end, zone, start_utc, end_utc, all_day, immutable, protected,
    source, training_day_id, objective_id, data_domain, classification, revision, created_at, updated_at, study_block_id,
    dst_adjustment";

fn map_event(r: &Row<'_>) -> StorageResult<CalendarEventRow> {
    Ok(CalendarEventRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        title: r.get("title")?,
        kind: enum_from_text(&r.get::<_, String>("kind")?)?,
        civil_start: parse_civil(&r.get::<_, String>("civil_start")?)?,
        civil_end: parse_civil(&r.get::<_, String>("civil_end")?)?,
        zone: IanaZone::parse(&r.get::<_, String>("zone")?)
            .map_err(|e| StorageError::RowDecode(e.to_string()))?,
        start_utc: from_ms(r.get("start_utc")?),
        end_utc: from_ms(r.get("end_utc")?),
        all_day: i64_bool(r.get("all_day")?)?,
        immutable: i64_bool(r.get("immutable")?)?,
        protected: i64_bool(r.get("protected")?)?,
        source: enum_from_text(&r.get::<_, String>("source")?)?,
        training_day_id: r.get("training_day_id")?,
        objective_id: r.get("objective_id")?,
        study_block_id: r.get("study_block_id")?,
        dst_adjustment: r
            .get::<_, Option<String>>("dst_adjustment")?
            .as_deref()
            .map(enum_from_text)
            .transpose()?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert an event (revision starts at 1).
pub fn insert_event(conn: &Connection, row: &CalendarEventRow) -> RepoResult<Revision> {
    if row.end_utc < row.start_utc {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "end_utc before start_utc".to_string(),
        )));
    }
    conn.execute(
        &format!(
            "INSERT INTO calendar_events ({EVENT_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, 1, ?17, ?17, ?18, ?19)"
        ),
        params![
            row.id.as_str(),
            row.title,
            enum_to_text(&row.kind)?,
            format_civil(&row.civil_start),
            format_civil(&row.civil_end),
            row.zone.as_str(),
            ms(row.start_utc),
            ms(row.end_utc),
            bool_i64(row.all_day),
            bool_i64(row.immutable),
            bool_i64(row.protected),
            enum_to_text(&row.source)?,
            row.training_day_id,
            row.objective_id,
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
            row.study_block_id,
            row.dst_adjustment.as_ref().map(enum_to_text).transpose()?,
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One event, scope-checked.
pub fn get_event(
    conn: &Connection,
    scope: &ReadScope,
    id: &CalendarEventId,
) -> RepoResult<Option<CalendarEventRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {EVENT_COLUMNS} FROM calendar_events WHERE id = ?1"),
        &[&id.as_str()],
        map_event,
    )?;
    if let Some(r) = &row {
        scope.check("calendar_events", r.data_domain)?;
    }
    Ok(row)
}

/// Events overlapping `[range_start, range_end)`, domain-filtered in SQL, ordered by start.
pub fn list_events_between(
    conn: &Connection,
    scope: &ReadScope,
    range_start: UtcMillis,
    range_end: UtcMillis,
) -> RepoResult<Vec<CalendarEventRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {EVENT_COLUMNS} FROM calendar_events
             WHERE start_utc < ?2 AND end_utc > ?1 AND (?3 = 1 OR data_domain = ?4)
             ORDER BY start_utc, id"
        ),
        &[
            &ms(range_start),
            &ms(range_end),
            &scope.sql_flag(),
            &scope.sql_domain()?,
        ],
        map_event,
    )
}

/// Replace every mutable column of an event (everything except `id`, `source`,
/// `data_domain`, `created_at`) under a revision check.
pub fn update_event(
    conn: &Connection,
    row: &CalendarEventRow,
    expected_revision: Revision,
    now: UtcMillis,
) -> RepoResult<Revision> {
    if row.end_utc < row.start_utc {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "end_utc before start_utc".to_string(),
        )));
    }
    let changed = conn.execute(
        "UPDATE calendar_events
         SET title = ?3, kind = ?4, civil_start = ?5, civil_end = ?6, zone = ?7, start_utc = ?8, end_utc = ?9,
             all_day = ?10, immutable = ?11, protected = ?12, training_day_id = ?13, objective_id = ?14,
             classification = ?15, revision = revision + 1, updated_at = ?16, study_block_id = ?17, dst_adjustment = ?18
         WHERE id = ?1 AND revision = ?2",
        params![
            row.id.as_str(),
            rev_i64(expected_revision),
            row.title,
            enum_to_text(&row.kind)?,
            format_civil(&row.civil_start),
            format_civil(&row.civil_end),
            row.zone.as_str(),
            ms(row.start_utc),
            ms(row.end_utc),
            bool_i64(row.all_day),
            bool_i64(row.immutable),
            bool_i64(row.protected),
            row.training_day_id,
            row.objective_id,
            enum_to_text(&row.classification)?,
            ms(now),
            row.study_block_id,
            row.dst_adjustment.as_ref().map(enum_to_text).transpose()?,
        ],
    )?;
    finish_update(
        conn,
        "calendar_events",
        "id",
        row.id.as_str(),
        expected_revision,
        changed,
    )
}

/// Delete an event and its recurrence rule under a revision check. Rows that
/// sessions or commitments reference keep working (`SET NULL`).
pub fn delete_event(
    conn: &Connection,
    id: &CalendarEventId,
    expected_revision: Revision,
) -> RepoResult<()> {
    super::atomic(conn, || {
        delete_event_statements(conn, id, expected_revision)
    })
}

/// The statements of [`delete_event`], which runs them as one unit (`super::atomic`).
fn delete_event_statements(
    conn: &Connection,
    id: &CalendarEventId,
    expected_revision: Revision,
) -> RepoResult<()> {
    let current = required(
        "calendar_events",
        super::current_revision(conn, "calendar_events", "id", id.as_str())?,
    )?;
    if current != expected_revision {
        return Err(RepoError::Storage(StorageError::StaleRevision {
            expected: Some(expected_revision),
            actual: current,
        }));
    }
    conn.execute(
        "DELETE FROM recurrence_rules WHERE calendar_event_id = ?1",
        params![id.as_str()],
    )?;
    let changed = conn.execute(
        "DELETE FROM calendar_events WHERE id = ?1 AND revision = ?2",
        params![id.as_str(), rev_i64(expected_revision)],
    )?;
    finish_update(
        conn,
        "calendar_events",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
    .map(|_| ())
}

/// A `recurrence_rules` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurrenceRuleRow {
    /// Row id.
    pub id: String,
    /// Owning event (one rule per event).
    pub calendar_event_id: CalendarEventId,
    /// The rule.
    pub rule: RecurrenceRule,
    /// Zone the rule is evaluated in.
    pub zone: IanaZone,
    /// Revision.
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

fn rule_columns(
    rule: &RecurrenceRule,
) -> StorageResult<(String, String, Option<String>, Option<i64>)> {
    rule.validate()
        .map_err(|e| StorageError::InvalidInput(format!("recurrence rule: {e}")))?;
    let freq = match rule.freq {
        Frequency::Daily => "DAILY",
        Frequency::Weekly => "WEEKLY",
        Frequency::Monthly => {
            return Err(StorageError::NotImplemented("calendar.recurrence.monthly"))
        }
    };
    let by_day: Vec<&str> = rule.by_day.iter().map(|d| d.rrule_code()).collect();
    Ok((
        freq.to_string(),
        to_json(&by_day)?,
        rule.until.as_ref().map(format_civil_date),
        rule.count.map(i64::from),
    ))
}

fn map_rule(r: &Row<'_>) -> StorageResult<RecurrenceRuleRow> {
    let freq = match r.get::<_, String>("freq")?.as_str() {
        "DAILY" => Frequency::Daily,
        "WEEKLY" => Frequency::Weekly,
        other => return Err(StorageError::RowDecode(format!("freq {other:?}"))),
    };
    let by_day_codes: Vec<String> = from_json(&r.get::<_, String>("by_day")?, "by_day")?;
    let mut by_day = Vec::with_capacity(by_day_codes.len());
    for code in &by_day_codes {
        by_day.push(
            Weekday::from_rrule_code(code)
                .ok_or_else(|| StorageError::RowDecode(format!("by_day code {code:?}")))?,
        );
    }
    let until = r
        .get::<_, Option<String>>("until_civil")?
        .as_deref()
        .map(parse_civil_date)
        .transpose()?;
    let count = r
        .get::<_, Option<i64>>("count")?
        .map(|c| u32_from_i64(c, "count"))
        .transpose()?;
    Ok(RecurrenceRuleRow {
        id: r.get("id")?,
        calendar_event_id: parse_id(&r.get::<_, String>("calendar_event_id")?)?,
        rule: RecurrenceRule {
            freq,
            interval: u32_from_i64(r.get("interval")?, "interval")?,
            by_day,
            until,
            count,
        },
        zone: IanaZone::parse(&r.get::<_, String>("zone")?)
            .map_err(|e| StorageError::RowDecode(e.to_string()))?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Attach a recurrence rule to an event (one per event; `CONFLICT` if present).
pub fn insert_rule(conn: &Connection, row: &RecurrenceRuleRow) -> RepoResult<Revision> {
    let (freq, by_day, until, count) = rule_columns(&row.rule)?;
    conn.execute(
        "INSERT INTO recurrence_rules (id, calendar_event_id, freq, interval, by_day, until_civil, count, zone, revision,
            created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9)",
        params![
            row.id,
            row.calendar_event_id.as_str(),
            freq,
            i64::from(row.rule.interval),
            by_day,
            until,
            count,
            row.zone.as_str(),
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// The rule of an event, if any.
pub fn get_rule(
    conn: &Connection,
    event_id: &CalendarEventId,
) -> RepoResult<Option<RecurrenceRuleRow>> {
    query_opt(
        conn,
        "SELECT id, calendar_event_id, freq, interval, by_day, until_civil, count, zone, revision, created_at, updated_at
         FROM recurrence_rules WHERE calendar_event_id = ?1",
        &[&event_id.as_str()],
        map_rule,
    )
}

/// Replace the rule of an event under a revision check.
pub fn update_rule(
    conn: &Connection,
    event_id: &CalendarEventId,
    expected_revision: Revision,
    rule: &RecurrenceRule,
    zone: &IanaZone,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let (freq, by_day, until, count) = rule_columns(rule)?;
    let changed = conn.execute(
        "UPDATE recurrence_rules SET freq = ?3, interval = ?4, by_day = ?5, until_civil = ?6, count = ?7, zone = ?8,
             revision = revision + 1, updated_at = ?9
         WHERE calendar_event_id = ?1 AND revision = ?2",
        params![
            event_id.as_str(),
            rev_i64(expected_revision),
            freq,
            i64::from(rule.interval),
            by_day,
            until,
            count,
            zone.as_str(),
            ms(now)
        ],
    )?;
    finish_update(
        conn,
        "recurrence_rules",
        "calendar_event_id",
        event_id.as_str(),
        expected_revision,
        changed,
    )
}

/// Remove the rule of an event under a revision check (the event becomes single).
pub fn delete_rule(
    conn: &Connection,
    event_id: &CalendarEventId,
    expected_revision: Revision,
) -> RepoResult<()> {
    let changed = conn.execute(
        "DELETE FROM recurrence_rules WHERE calendar_event_id = ?1 AND revision = ?2",
        params![event_id.as_str(), rev_i64(expected_revision)],
    )?;
    finish_update(
        conn,
        "recurrence_rules",
        "calendar_event_id",
        event_id.as_str(),
        expected_revision,
        changed,
    )
    .map(|_| ())
}

/// Proposal lifecycle (mirrors the `schedule_proposals.state` CHECK list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProposalState {
    /// Awaiting an operator decision.
    Proposed,
    /// Applied to the calendar.
    Applied,
    /// Rejected by the operator.
    Rejected,
    /// Replaced by a newer proposal.
    Superseded,
}

/// A `schedule_proposals` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleProposalRow {
    /// Proposal id.
    pub id: ScheduleProposalId,
    /// Proposing actor (scheduler system actor or a skill under the operator).
    pub proposed_by: ActorId,
    /// When proposed.
    pub proposed_at: UtcMillis,
    /// Planning range start.
    pub range_start_utc: UtcMillis,
    /// Planning range end.
    pub range_end_utc: UtcMillis,
    /// The deterministic proposal (placements, unplaced, feasibility, trace).
    pub proposal: ScheduleProposal,
    /// Lifecycle state.
    pub state: ProposalState,
    /// Trust class of the proposer.
    pub trust: TrustClass,
    /// Provenance.
    pub provenance: Provenance,
    /// Revision.
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

impl ScheduleProposalRow {
    /// Violations listed by the proposal (empty when feasible).
    pub fn violated_constraints(&self) -> Vec<ConstraintViolation> {
        match &self.proposal.feasibility {
            Feasibility::Feasible => Vec::new(),
            Feasibility::Infeasible { violations } => violations.clone(),
        }
    }

    /// Whether the proposal is feasible.
    pub fn feasible(&self) -> bool {
        matches!(self.proposal.feasibility, Feasibility::Feasible)
    }
}

const PROPOSAL_COLUMNS: &str = "id, proposed_by, proposed_at, range_start_utc, range_end_utc, proposal_json,
    violated_constraints_json, feasible, state, trust, provenance_json, revision, created_at, updated_at";

fn map_proposal(r: &Row<'_>) -> StorageResult<ScheduleProposalRow> {
    Ok(ScheduleProposalRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        proposed_by: parse_id(&r.get::<_, String>("proposed_by")?)?,
        proposed_at: from_ms(r.get("proposed_at")?),
        range_start_utc: from_ms(r.get("range_start_utc")?),
        range_end_utc: from_ms(r.get("range_end_utc")?),
        proposal: from_json(&r.get::<_, String>("proposal_json")?, "proposal_json")?,
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        trust: enum_from_text(&r.get::<_, String>("trust")?)?,
        provenance: from_json(&r.get::<_, String>("provenance_json")?, "provenance_json")?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Append a proposal (state must be `PROPOSED`; proposals never start applied).
pub fn insert_proposal(conn: &Connection, row: &ScheduleProposalRow) -> RepoResult<Revision> {
    if row.state != ProposalState::Proposed {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "a new schedule proposal must be PROPOSED".to_string(),
        )));
    }
    conn.execute(
        &format!(
            "INSERT INTO schedule_proposals ({PROPOSAL_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'PROPOSED', ?9, ?10, 1, ?11, ?11)"
        ),
        params![
            row.id.as_str(),
            row.proposed_by.as_str(),
            ms(row.proposed_at),
            ms(row.range_start_utc),
            ms(row.range_end_utc),
            to_json(&row.proposal)?,
            to_json(&row.violated_constraints())?,
            bool_i64(row.feasible()),
            enum_to_text(&row.trust)?,
            to_json(&row.provenance)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One proposal.
pub fn get_proposal(
    conn: &Connection,
    id: &ScheduleProposalId,
) -> RepoResult<Option<ScheduleProposalRow>> {
    query_opt(
        conn,
        &format!("SELECT {PROPOSAL_COLUMNS} FROM schedule_proposals WHERE id = ?1"),
        &[&id.as_str()],
        map_proposal,
    )
}

/// Proposals in a state, newest first.
pub fn list_proposals(
    conn: &Connection,
    state: ProposalState,
) -> RepoResult<Vec<ScheduleProposalRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {PROPOSAL_COLUMNS} FROM schedule_proposals WHERE state = ?1 ORDER BY proposed_at DESC, id"
        ),
        &[&enum_to_text(&state)?],
        map_proposal,
    )
}

/// Move a `PROPOSED` proposal to `APPLIED`, `REJECTED` or `SUPERSEDED` (the only
/// mutation proposals accept).
pub fn set_proposal_state(
    conn: &Connection,
    id: &ScheduleProposalId,
    expected_revision: Revision,
    state: ProposalState,
    now: UtcMillis,
) -> RepoResult<Revision> {
    if state == ProposalState::Proposed {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "a proposal cannot return to PROPOSED".to_string(),
        )));
    }
    let changed = conn.execute(
        "UPDATE schedule_proposals SET state = ?3, revision = revision + 1, updated_at = ?4
         WHERE id = ?1 AND revision = ?2 AND state = 'PROPOSED'",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            enum_to_text(&state)?,
            ms(now)
        ],
    )?;
    finish_update(
        conn,
        "schedule_proposals",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

/// Operator verdict on a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScheduleDecisionKind {
    /// Apply the proposal.
    Apply,
    /// Reject it.
    Reject,
}

/// A `schedule_decisions` row (immutable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleDecisionRow {
    /// Decision id.
    pub id: String,
    /// Proposal decided.
    pub proposal_id: ScheduleProposalId,
    /// Verdict.
    pub decision: ScheduleDecisionKind,
    /// Deciding operator.
    pub decided_by: ActorId,
    /// When.
    pub decided_at: UtcMillis,
    /// Approval token consumed for `APPLY` (`Capability::ScheduleApply`).
    pub approval_id: Option<ApprovalId>,
    /// Reason.
    pub reason: Option<String>,
    /// Proposal revision the decision applied to.
    pub applied_revision: Option<Revision>,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Append a decision (never updated or deleted; trigger-enforced).
pub fn insert_decision(conn: &Connection, row: &ScheduleDecisionRow) -> RepoResult<()> {
    conn.execute(
        "INSERT INTO schedule_decisions (id, proposal_id, decision, decided_by, decided_at, approval_id, reason,
            applied_revision, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            row.id,
            row.proposal_id.as_str(),
            enum_to_text(&row.decision)?,
            row.decided_by.as_str(),
            ms(row.decided_at),
            row.approval_id.as_ref().map(|a| a.as_str().to_string()),
            row.reason,
            row.applied_revision.map(rev_i64),
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

/// Decisions on a proposal in time order.
pub fn list_decisions(
    conn: &Connection,
    proposal_id: &ScheduleProposalId,
) -> RepoResult<Vec<ScheduleDecisionRow>> {
    query_all(
        conn,
        "SELECT id, proposal_id, decision, decided_by, decided_at, approval_id, reason, applied_revision, created_at
         FROM schedule_decisions WHERE proposal_id = ?1 ORDER BY decided_at, id",
        &[&proposal_id.as_str()],
        |r| {
            Ok(ScheduleDecisionRow {
                id: r.get("id")?,
                proposal_id: parse_id(&r.get::<_, String>("proposal_id")?)?,
                decision: enum_from_text(&r.get::<_, String>("decision")?)?,
                decided_by: parse_id(&r.get::<_, String>("decided_by")?)?,
                decided_at: from_ms(r.get("decided_at")?),
                approval_id: opt_parse_id(r.get("approval_id")?)?,
                reason: r.get("reason")?,
                applied_revision: r
                    .get::<_, Option<i64>>("applied_revision")?
                    .map(rev_from_i64),
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::testutil::{approval, assert_immutable, db, learning, operator_id, t};
    use g9_domain::SourceKind;

    fn zone() -> IanaZone {
        IanaZone::parse("Europe/Berlin").unwrap()
    }

    fn event(id: &str, domain: DataDomain) -> CalendarEventRow {
        CalendarEventRow {
            id: CalendarEventId::parse(id).unwrap(),
            title: "Day 1 study".into(),
            kind: CalendarEventKind::StudyBlock,
            civil_start: CivilDateTime::new(2025, 11, 6, 9, 0, 0).unwrap(),
            civil_end: CivilDateTime::new(2025, 11, 6, 16, 0, 0).unwrap(),
            zone: zone(),
            start_utc: t(8 * 3_600_000),
            end_utc: t(15 * 3_600_000),
            all_day: false,
            immutable: false,
            protected: false,
            source: EventSource::Operator,
            training_day_id: None,
            objective_id: None,
            study_block_id: None,
            dst_adjustment: None,
            data_domain: domain,
            classification: Classification::Private,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        }
    }

    fn provenance() -> Provenance {
        Provenance {
            source_kind: SourceKind::SystemGenerated,
            source_ref: "scheduler".into(),
            trust: TrustClass::SystemDeterministic,
            content_sha256: None,
            recorded_at: t(0),
            recorded_by: operator_id(),
            version: Some("1".into()),
        }
    }

    #[test]
    fn event_crud_roundtrip_with_revision_check() {
        let db = db();
        let conn = db.conn();
        let row = event("ev-1", DataDomain::Learning);
        assert_eq!(insert_event(conn, &row).unwrap(), Revision::INITIAL);
        let stored = get_event(conn, &learning(), &row.id).unwrap().unwrap();
        assert_eq!(stored, row);

        let mut changed = stored.clone();
        changed.title = "Day 1 study (moved)".into();
        changed.civil_start = CivilDateTime::new(2025, 11, 6, 10, 0, 0).unwrap();
        changed.start_utc = t(9 * 3_600_000);
        changed.dst_adjustment = Some(DstAdjustment::Overlap);
        assert!(matches!(
            update_event(conn, &changed, Revision::new(5), t(1)),
            Err(RepoError::Storage(StorageError::StaleRevision { actual, .. })) if actual == Revision::INITIAL
        ));
        let r2 = update_event(conn, &changed, Revision::INITIAL, t(1)).unwrap();
        assert_eq!(r2, Revision::new(2));
        let after = get_event(conn, &learning(), &row.id).unwrap().unwrap();
        assert_eq!(after.title, "Day 1 study (moved)");
        assert_eq!(after.revision, Revision::new(2));
        assert_eq!(after.updated_at, t(1));
        assert_eq!(after.civil_start.hour, 10);
        assert_eq!(after.dst_adjustment, Some(DstAdjustment::Overlap));
        assert_eq!(DstAdjustment::from_adjustment(&Adjustment::None), None);
        // the CHECK list mirrors the enum spelling
        let raw: StorageError = conn
            .execute(
                "UPDATE calendar_events SET dst_adjustment = 'SHIFTED' WHERE id = 'ev-1'",
                [],
            )
            .expect_err("CHECK must refuse")
            .into();
        assert!(matches!(raw, StorageError::CheckViolation(_)), "{raw:?}");

        let listed = list_events_between(conn, &learning(), t(0), t(24 * 3_600_000)).unwrap();
        assert_eq!(listed.len(), 1);
        assert!(list_events_between(conn, &learning(), t(0), t(3_600_000))
            .unwrap()
            .is_empty());

        assert!(matches!(
            delete_event(conn, &row.id, Revision::INITIAL),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        delete_event(conn, &row.id, r2).unwrap();
        assert!(get_event(conn, &learning(), &row.id).unwrap().is_none());
        assert!(matches!(
            delete_event(conn, &row.id, r2),
            Err(RepoError::Storage(StorageError::NotFound(_)))
        ));
    }

    #[test]
    fn cross_domain_filter_applies_to_get_and_list() {
        let db = db();
        let conn = db.conn();
        insert_event(conn, &event("ev-l", DataDomain::Learning)).unwrap();
        insert_event(conn, &event("ev-b", DataDomain::Business)).unwrap();
        let id_b = CalendarEventId::parse("ev-b").unwrap();
        let err = get_event(conn, &learning(), &id_b).unwrap_err();
        assert!(matches!(
            err,
            RepoError::CrossDomainRefused {
                table: "calendar_events",
                requested: DataDomain::Learning,
                actual: DataDomain::Business
            }
        ));
        assert!(
            get_event(conn, &ReadScope::cross_domain(DataDomain::Learning), &id_b)
                .unwrap()
                .is_some()
        );
        let only_learning = list_events_between(conn, &learning(), t(0), t(86_400_000)).unwrap();
        assert_eq!(only_learning.len(), 1);
        assert_eq!(only_learning[0].id.as_str(), "ev-l");
        let all = list_events_between(
            conn,
            &ReadScope::cross_domain(DataDomain::Learning),
            t(0),
            t(86_400_000),
        )
        .unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn domain_event_adapter_roundtrips_and_refuses_unsupported_kinds() {
        let db = db();
        let conn = db.conn();
        let domain_event = CalendarEvent {
            id: CalendarEventId::parse("ev-d").unwrap(),
            title: "Rest".into(),
            kind: EventKind::ProtectedRest,
            immutable: true,
            start: CivilDateTime::new(2025, 11, 6, 22, 0, 0).unwrap(),
            duration: DurationMs::from_minutes_u32(8 * 60),
            zone: zone(),
            all_day: false,
            rule: Some(RecurrenceRule::new(Frequency::Daily, 1).unwrap()),
            data_domain: DataDomain::Personal,
        };
        let row = CalendarEventRow::from_domain(
            &domain_event,
            t(21 * 3_600_000),
            t(29 * 3_600_000),
            EventSource::Operator,
            true,
            Classification::Private,
            t(0),
        )
        .unwrap();
        assert_eq!(
            row.civil_end,
            CivilDateTime::new(2025, 11, 7, 6, 0, 0).unwrap()
        );
        insert_event(conn, &row).unwrap();
        insert_rule(
            conn,
            &RecurrenceRuleRow {
                id: "rr-1".into(),
                calendar_event_id: row.id.clone(),
                rule: domain_event.rule.clone().unwrap(),
                zone: zone(),
                revision: Revision::INITIAL,
                created_at: t(0),
                updated_at: t(0),
            },
        )
        .unwrap();
        let personal = ReadScope::domain(DataDomain::Personal);
        let stored = get_event(conn, &personal, &row.id).unwrap().unwrap();
        let rule = get_rule(conn, &row.id).unwrap().map(|r| r.rule);
        assert_eq!(stored.to_domain(rule).unwrap(), domain_event);

        let mut unsupported = domain_event.clone();
        unsupported.kind = EventKind::Commitment;
        assert!(matches!(
            CalendarEventRow::from_domain(
                &unsupported,
                t(0),
                t(1),
                EventSource::Operator,
                false,
                Classification::Private,
                t(0)
            ),
            Err(StorageError::NotImplemented("calendar.event_kind"))
        ));
        let mut other = stored.clone();
        other.kind = CalendarEventKind::Other;
        assert!(other.to_domain(None).is_err());
    }

    #[test]
    fn recurrence_rule_roundtrip_update_delete_and_monthly_gap() {
        let db = db();
        let conn = db.conn();
        let ev = event("ev-1", DataDomain::Learning);
        insert_event(conn, &ev).unwrap();
        let rule = RecurrenceRule::new(Frequency::Weekly, 2)
            .unwrap()
            .with_by_day([Weekday::Mon, Weekday::Wed])
            .with_until(CivilDate::new(2026, 2, 28).unwrap());
        insert_rule(
            conn,
            &RecurrenceRuleRow {
                id: "rr-1".into(),
                calendar_event_id: ev.id.clone(),
                rule: rule.clone(),
                zone: zone(),
                revision: Revision::INITIAL,
                created_at: t(0),
                updated_at: t(0),
            },
        )
        .unwrap();
        let stored = get_rule(conn, &ev.id).unwrap().unwrap();
        assert_eq!(stored.rule, rule);
        // second rule on the same event conflicts (UNIQUE)
        assert!(matches!(
            insert_rule(
                conn,
                &RecurrenceRuleRow {
                    id: "rr-2".into(),
                    ..stored.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::UniqueViolation(_)))
        ));
        let counted = RecurrenceRule::new(Frequency::Daily, 1)
            .unwrap()
            .with_count(5);
        assert!(matches!(
            update_rule(conn, &ev.id, Revision::new(4), &counted, &zone(), t(1)),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        let r2 = update_rule(conn, &ev.id, Revision::INITIAL, &counted, &zone(), t(1)).unwrap();
        assert_eq!(get_rule(conn, &ev.id).unwrap().unwrap().rule, counted);
        let monthly = RecurrenceRule::new(Frequency::Monthly, 1).unwrap();
        assert!(matches!(
            update_rule(conn, &ev.id, r2, &monthly, &zone(), t(2)),
            Err(RepoError::Storage(StorageError::NotImplemented(
                "calendar.recurrence.monthly"
            )))
        ));
        delete_rule(conn, &ev.id, r2).unwrap();
        assert!(get_rule(conn, &ev.id).unwrap().is_none());
        // deleting the event removes a rule implicitly
        insert_rule(
            conn,
            &RecurrenceRuleRow {
                id: "rr-3".into(),
                ..stored
            },
        )
        .unwrap();
        delete_event(conn, &ev.id, Revision::INITIAL).unwrap();
        assert!(get_rule(conn, &ev.id).unwrap().is_none());
    }

    #[test]
    fn proposals_change_state_once_and_decisions_are_immutable() {
        let db = db();
        let conn = db.conn();
        let proposal = ScheduleProposal {
            placements: vec![],
            unplaced: vec![],
            feasibility: Feasibility::Feasible,
            constraint_trace: vec!["no tasks".into()],
        };
        let row = ScheduleProposalRow {
            id: ScheduleProposalId::parse("sp-1").unwrap(),
            proposed_by: ActorId::parse("system-scheduler").unwrap(),
            proposed_at: t(0),
            range_start_utc: t(0),
            range_end_utc: t(7 * 86_400_000),
            proposal,
            state: ProposalState::Proposed,
            trust: TrustClass::SystemDeterministic,
            provenance: provenance(),
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        };
        insert_proposal(conn, &row).unwrap();
        let stored = get_proposal(conn, &row.id).unwrap().unwrap();
        assert_eq!(stored, row);
        assert!(stored.feasible());
        assert_eq!(
            list_proposals(conn, ProposalState::Proposed).unwrap().len(),
            1
        );
        assert!(matches!(
            insert_proposal(
                conn,
                &ScheduleProposalRow {
                    id: ScheduleProposalId::parse("sp-x").unwrap(),
                    state: ProposalState::Applied,
                    ..row.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));

        let apr = approval(conn, "apr-1");
        let decision = ScheduleDecisionRow {
            id: "sd-1".into(),
            proposal_id: row.id.clone(),
            decision: ScheduleDecisionKind::Apply,
            decided_by: operator_id(),
            decided_at: t(5),
            approval_id: Some(apr),
            reason: None,
            applied_revision: Some(Revision::INITIAL),
            created_at: t(5),
        };
        insert_decision(conn, &decision).unwrap();
        let r2 = set_proposal_state(
            conn,
            &row.id,
            Revision::INITIAL,
            ProposalState::Applied,
            t(5),
        )
        .unwrap();
        assert_eq!(r2, Revision::new(2));
        assert_eq!(
            get_proposal(conn, &row.id).unwrap().unwrap().state,
            ProposalState::Applied
        );
        // a decided proposal cannot change again (state guard, not stale)
        assert!(matches!(
            set_proposal_state(conn, &row.id, r2, ProposalState::Rejected, t(6)),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        assert!(matches!(
            set_proposal_state(conn, &row.id, r2, ProposalState::Proposed, t(6)),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        assert_eq!(list_decisions(conn, &row.id).unwrap(), vec![decision]);
        assert_immutable(
            conn,
            "UPDATE schedule_decisions SET reason = 'x' WHERE id = 'sd-1'",
        );
        assert_immutable(conn, "DELETE FROM schedule_decisions WHERE id = 'sd-1'");
    }

    #[test]
    fn civil_text_roundtrip() {
        let c = CivilDateTime::new(2026, 3, 29, 2, 30, 5).unwrap();
        assert_eq!(format_civil(&c), "2026-03-29T02:30:05");
        assert_eq!(parse_civil(&format_civil(&c)).unwrap(), c);
        assert!(parse_civil("2026-03-29").is_err());
        assert!(parse_civil("2026-13-29T00:00:00").is_err());
        let d = CivilDate::new(2026, 2, 28).unwrap();
        assert_eq!(parse_civil_date(&format_civil_date(&d)).unwrap(), d);
    }
}
