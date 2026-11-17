//! The read operations the latency harness measures (PERF-2) and the query-plan
//! gate runs (PERF-3), with the payloads the UI sends
//! (`apps/desktop/src/app/screens/*`): one place, so that the gate plans the
//! statements of exactly the reads that are measured.

use crate::client::Client;
use crate::meta::MetaOptions;
use crate::output::CliError;
use crate::perf::fixture::{self, PROGRAM_START, ZONE};
use crate::seed;
use g9_domain::{G9Error, UtcMillis};
use serde_json::{json, Value};

/// One IPC call.
#[derive(Debug, Clone, PartialEq)]
pub struct Call {
    /// Operation.
    pub op: &'static str,
    /// Short label of the payload variant (for reports).
    pub variant: &'static str,
    /// Payload.
    pub payload: Value,
}

fn call(op: &'static str, variant: &'static str, payload: Value) -> Call {
    Call {
        op,
        variant,
        payload,
    }
}

/// Monday 00:00 local of program week 1 (the busiest calendar week of the
/// fixture: every block has its event, plus the appointments) and the date
/// with the most sessions (Monday of week 1, six blocks).
fn busiest() -> Result<(UtcMillis, String), G9Error> {
    let zone =
        g9_domain::IanaZone::parse(ZONE).map_err(|_| G9Error::internal_redacted("perf.zone"))?;
    let date = seed::parse_date(PROGRAM_START)?;
    Ok((
        seed::local_instant(date, 0, &zone)?,
        seed::format_date(date),
    ))
}

/// The budget's indexed reads (`docs/ui/performance-budgets.md`): `session.list`
/// for a day (Daily Classroom, day rail, Performance Dashboard) and for every
/// session (Session Tracker, Performance Dashboard), `calendar.list` for one
/// week (the Calendar screen sends `from`/`to` only; the zone given here is
/// the context's own zone, the one the backend falls back to),
/// `retention.due` at `now`, `approvals.list` (Approvals and Audit) and
/// `audit.recent` with limit 50.
pub fn budget_reads(now: UtcMillis) -> Result<Vec<Call>, G9Error> {
    let (week_start, day) = busiest()?;
    let week_ms = 7 * 86_400_000;
    Ok(vec![
        call("session.list", "date", json!({ "date": day })),
        call("session.list", "all", json!({})),
        call(
            "calendar.list",
            "week",
            json!({ "from": week_start.as_millis(), "to": week_start.as_millis() + week_ms, "zone": ZONE }),
        ),
        call(
            "retention.due",
            "as_of",
            json!({ "as_of": now.as_millis() }),
        ),
        call("approvals.list", "all", json!({})),
        call("audit.recent", "limit 50", json!({ "limit": 50 })),
    ])
}

/// The other reads the screens issue (not in the budget list): planned by the
/// gate so that their statements are covered too.
pub fn other_reads() -> Result<Vec<Call>, G9Error> {
    let (_, day) = busiest()?;
    Ok(vec![
        call("approvals.list", "pending", json!({ "state": "PENDING" })),
        call("system.status", "-", json!({})),
        call("curriculum.program", "-", json!({})),
        call("curriculum.daily_plan", "date", json!({ "date": day })),
        call("competency.projection", "-", json!({})),
        call("notes.list", "-", json!({})),
        call("assessment.list", "-", json!({})),
        call("evidence.list", "-", json!({})),
        call("backup.list", "-", json!({})),
        call("interventions.list", "-", json!({})),
    ])
}

/// Dispatch every read of [`budget_reads`] and [`other_reads`] once through
/// `client` (the query-plan gate's read workload). Any refusal is an error.
pub fn run_reads(client: &mut Client) -> Result<usize, CliError> {
    let now = after_fixture()?;
    let mut calls = budget_reads(now)?;
    calls.extend(other_reads()?);
    for c in &calls {
        client.call(c.op, c.payload.clone())?;
    }
    Ok(calls.len())
}

/// The writes the fixture does not make, once each, through `client` (the
/// query-plan gate's second workload, for coverage): a note read back, turned
/// into evidence, given an attachment and erased (`deletion.request`,
/// approval round trip); a calendar appointment moved and deleted (approval
/// round trip); a review-block proposal applied (approval round trip); the
/// competency packs that ship with the application imported; a backup
/// created, verified and restore-tested; an export of the learning domain
/// (approval round trip). Returns the number of operations dispatched
/// (an approval round trip counts as one).
///
/// Not reached: `session.correct_time` (it accepts only an `UNCERTAIN`
/// interval, which only crash recovery produces), `curriculum.promote`,
/// `skills.invoke` (no skills are loaded).
pub fn run_other_writes(client: &mut Client) -> Result<usize, CliError> {
    let mut n = 0;
    let mut call = |client: &mut Client, op: &str, payload: Value| {
        n += 1;
        client.call(op, payload)
    };
    let note = call(
        client,
        "notes.create",
        json!({ "title": "Gate note", "body_markdown": "Gate note body.", "classification": "INTERNAL", "domain": "LEARNING" }),
    )?;
    let note_id = note["result"]["note_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    call(client, "notes.get", json!({ "note_id": note_id }))?;
    call(client, "evidence.from_note", json!({ "note_id": note_id }))?;
    call(
        client,
        "evidence.attach_file",
        // "Gate attachment.\n"
        json!({ "note_id": note_id, "filename": "gate.txt", "media_type": "text/plain", "content_base64": "R2F0ZSBhdHRhY2htZW50Lgo=" }),
    )?;
    let now = client.now().as_millis();
    let event = call(
        client,
        "calendar.create_event",
        json!({ "title": "Gate appointment", "kind": "APPOINTMENT", "start": now + 3_600_000, "end": now + 7_200_000 }),
    )?;
    let event_id = event["result"]["event_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    call(
        client,
        "calendar.update_event",
        json!({ "event_id": event_id, "start": now + 10_800_000, "end": now + 14_400_000 }),
    )?;
    let proposal = call(
        client,
        "calendar.propose",
        json!({ "from": now + 86_400_000, "to": now + 2 * 86_400_000, "intent": "ADD_REVIEWS" }),
    )?;
    let proposal_id = proposal["result"]["proposal_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    call(client, "competency.import_pack", json!({}))?;
    let backup = call(client, "backup.create", json!({ "label": "gate" }))?;
    let backup_id = backup["result"]["backup_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    call(client, "backup.verify", json!({ "backup_id": backup_id }))?;
    call(
        client,
        "backup.restore_test",
        json!({ "backup_id": backup_id }),
    )?;
    let meta = MetaOptions::default();
    client.run_approved(
        "calendar.apply",
        json!({ "proposal_id": proposal_id }),
        &meta,
    )?;
    client.run_approved(
        "calendar.delete_event",
        json!({ "event_id": event_id }),
        &meta,
    )?;
    client.run_approved(
        "deletion.request",
        json!({ "resource": { "kind": "NOTE", "id": note_id }, "reason": "query-plan gate workload" }),
        &meta,
    )?;
    client.run_approved(
        "export.request",
        json!({ "domains": ["LEARNING"], "format": "JSON", "include_private": false }),
        &meta,
    )?;
    Ok(n + 4)
}

/// The instant a client of a finished fixture continues from: 07:00 local on
/// the day after the timeline.
pub fn after_fixture() -> Result<UtcMillis, G9Error> {
    let zone =
        g9_domain::IanaZone::parse(ZONE).map_err(|_| G9Error::internal_redacted("perf.zone"))?;
    let start = seed::parse_date(PROGRAM_START)?;
    let day = start
        .add_days(fixture::TIMELINE_DAYS)
        .map_err(|_| G9Error::internal_redacted("perf.date"))?;
    seed::local_instant(day, fixture::CLOCK_DAY_HOUR, &zone)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_budget_reads_validate_against_their_request_schemas() {
        let now = after_fixture().unwrap();
        let reads = budget_reads(now).unwrap();
        assert_eq!(reads.len(), 6);
        for c in reads.iter().chain(other_reads().unwrap().iter()) {
            g9_app::schemas::validate_request(c.op, &c.payload)
                .unwrap_or_else(|e| panic!("{} {}: {e:?}", c.op, c.variant));
        }
        // 2027-01-04 07:00 CET.
        assert_eq!(now, UtcMillis::from_millis(1_799_042_400_000));
    }
}
