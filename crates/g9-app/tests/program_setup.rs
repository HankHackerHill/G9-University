#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Program setup (owner request 2026-11-14): the application reads the program start the operator
//! set (`curriculum.set_program_start`), reports what is imported (`curriculum.program`) and plans
//! the blocks of a day as sessions linked to their curriculum blocks (`session.plan_day`).

mod common;

use common::{seed_manifest, Harness, Opts};
use g9_domain::{ErrorCategory, IanaZone};
use serde_json::{json, Value};

fn imported() -> Harness {
    let mut h = Harness::new();
    import(&mut h);
    h
}

fn import(h: &mut Harness) {
    h.approved(
        "curriculum.import",
        json!({ "manifest": seed_manifest(), "provenance": "PROPOSED_SEED" }),
        &Opts::default(),
    );
}

fn set_start(h: &mut Harness, date: &str) -> Value {
    h.call("curriculum.set_program_start", json!({ "date": date }))
        .unwrap()
}

fn program(h: &mut Harness) -> Value {
    h.call("curriculum.program", json!({})).unwrap()["result"].clone()
}

fn blocks_of(h: &mut Harness, date: &str) -> Vec<Value> {
    h.call("curriculum.daily_plan", json!({ "date": date }))
        .unwrap()["result"]["blocks"]
        .as_array()
        .unwrap()
        .clone()
}

fn session_count(h: &Harness) -> i64 {
    h.conn()
        .query_row("SELECT COUNT(*) FROM study_sessions", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn program_reports_nothing_before_an_import_and_the_whole_curriculum_after() {
    let mut h = Harness::new();
    let before = program(&mut h);
    assert_eq!(before["curriculum"], Value::Null);
    assert_eq!(before["program_start"], Value::Null);
    assert_eq!(
        before["competencies"],
        json!({ "loaded": 0, "available": 64 })
    );

    import(&mut h);
    let after = program(&mut h);
    let c = &after["curriculum"];
    assert_eq!(c["weeks"], 12);
    assert_eq!(c["days"], 72);
    assert_eq!(c["objectives"], 216);
    // 480 hours.
    assert_eq!(c["planned_minutes"], 28_800);
    assert_eq!(c["provenance"], "PROPOSED_SEED");
    assert!(c["name"].as_str().unwrap().len() > 3);
    assert_eq!(after["program_start"], Value::Null);
}

#[test]
fn the_start_must_be_a_monday_and_nothing_is_stored_when_it_is_not() {
    let mut h = imported();
    for date in ["2026-11-03", "2026-11-08", "2026-11-01"] {
        let e = h
            .call("curriculum.set_program_start", json!({ "date": date }))
            .unwrap_err();
        assert_eq!(e.category, ErrorCategory::InvalidInput, "{date}");
        assert_eq!(e.code, "program.start_not_a_monday", "{date}");
    }
    assert_eq!(program(&mut h)["program_start"], Value::Null);
    let rows: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM program_plan", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rows, 0);
}

#[test]
fn an_impossible_date_and_a_malformed_start_time_are_refused() {
    let mut h = imported();
    let e = h
        .call(
            "curriculum.set_program_start",
            json!({ "date": "2026-02-30" }),
        )
        .unwrap_err();
    assert_eq!(e.code, "date.invalid");
    for time in ["9:00", "24:00", "09:60", "ab:cd", "09:00:00", "0900", ""] {
        let e = h
            .call(
                "curriculum.set_program_start",
                json!({ "date": "2026-11-02", "day_start": time }),
            )
            .unwrap_err();
        // The request schema stops a malformed value before the handler; the handler
        // refuses the ones that match the shape but not the clock.
        assert_eq!(e.category, ErrorCategory::InvalidInput, "{time:?}");
    }
    assert_eq!(program(&mut h)["program_start"], Value::Null);
}

#[test]
fn the_program_start_needs_an_imported_curriculum() {
    let mut h = Harness::new();
    let e = h
        .call(
            "curriculum.set_program_start",
            json!({ "date": "2026-11-02" }),
        )
        .unwrap_err();
    assert_eq!(e.category, ErrorCategory::NotFound);
    assert_eq!(e.code, "curriculum.not_imported");
}

#[test]
fn the_daily_plan_follows_the_stored_start_and_moving_it_moves_the_plan() {
    let mut h = imported();
    let e = h
        .call("curriculum.daily_plan", json!({ "date": "2026-11-03" }))
        .unwrap_err();
    assert_eq!(e.code, "curriculum.program_start_not_configured");

    let set = set_start(&mut h, "2026-11-02");
    assert_eq!(set["result"]["monday"], "2026-11-02");
    assert_eq!(set["result"]["day_start"], "09:00");
    let tuesday = h
        .call("curriculum.daily_plan", json!({ "date": "2026-11-03" }))
        .unwrap();
    assert_eq!(tuesday["result"]["day_id"], "D02");
    assert_eq!(tuesday["result"]["week_number"], 1);
    let p = program(&mut h);
    assert_eq!(p["program_start"]["monday"], "2026-11-02");
    assert_eq!(p["program_start"]["day_start"], "09:00");
    assert_eq!(p["program_start"]["revision"], 1);

    // The operator moves the start a week later and chooses another hour.
    let moved = h
        .call(
            "curriculum.set_program_start",
            json!({ "date": "2026-11-09", "day_start": "08:30" }),
        )
        .unwrap();
    assert_eq!(moved["result"]["day_start"], "08:30");
    let p = program(&mut h);
    assert_eq!(p["program_start"]["monday"], "2026-11-09");
    assert_eq!(p["program_start"]["revision"], 2);
    let before_week_1 = h
        .call("curriculum.daily_plan", json!({ "date": "2026-11-03" }))
        .unwrap();
    assert_eq!(before_week_1["result"]["unscheduled"], true);
    let tuesday = h
        .call("curriculum.daily_plan", json!({ "date": "2026-11-10" }))
        .unwrap();
    assert_eq!(tuesday["result"]["day_id"], "D02");
}

#[test]
fn a_start_given_to_the_process_wins_over_the_stored_one() {
    let mut h = Harness::with(|s| {
        s.config.program_start = Some(g9_curriculum::CivilDate::new(2026, 11, 2).unwrap());
    });
    import(&mut h);
    set_start(&mut h, "2026-11-09");
    let day = h
        .call("curriculum.daily_plan", json!({ "date": "2026-11-03" }))
        .unwrap();
    assert_eq!(day["result"]["day_id"], "D02");
}

#[test]
fn planning_a_day_links_one_session_to_each_block_in_the_blocks_mode() {
    let mut h = imported();
    set_start(&mut h, "2026-11-02");
    let blocks = blocks_of(&mut h, "2026-11-03");
    assert_eq!(blocks.len(), 6);
    assert!(blocks.iter().all(|b| b["session_id"].is_null()));

    let planned = h
        .call("session.plan_day", json!({ "date": "2026-11-03" }))
        .unwrap();
    let r = &planned["result"];
    assert_eq!(r["day_id"], "D02");
    let created = r["created"].as_array().unwrap();
    assert_eq!(created.len(), 6);
    assert!(r["already_planned"].as_array().unwrap().is_empty());
    let codes: Vec<&str> = created
        .iter()
        .map(|s| s["block"].as_str().unwrap())
        .collect();
    assert_eq!(
        codes,
        [
            "recall",
            "theory",
            "guided_lab",
            "independent_lab",
            "defense_verification",
            "evidence_reporting"
        ]
    );
    for (i, s) in created.iter().enumerate() {
        assert_eq!(s["state"], "PLANNED");
        assert_eq!(s["day_id"], "D02");
        assert_eq!(s["block_seq"], i + 1);
        assert_eq!(
            s["planned_ms"],
            blocks[i]["minutes"].as_i64().unwrap() * 60_000
        );
        assert_eq!(s["credited_ms"], 0);
    }

    // The daily plan now says which session belongs to which block.
    let after = blocks_of(&mut h, "2026-11-03");
    for (block, session) in after.iter().zip(created) {
        assert_eq!(block["session_id"], session["session_id"]);
    }

    // Times: back to back from 09:00 UTC (the harness zone), the first block at the day start.
    let rows: Vec<(String, i64, i64, String)> = {
        let mut stmt = h
            .conn()
            .prepare(
                "SELECT b.block_code, s.planned_start_utc, s.planned_end_utc, s.mode
                   FROM study_sessions s JOIN study_blocks b ON b.id = s.study_block_id
                  ORDER BY b.ordinal",
            )
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(rows.len(), 6);
    let nine = 1_793_696_400_000_i64; // 2026-11-03T09:00:00Z
    assert_eq!(rows[0].1, nine);
    for pair in rows.windows(2) {
        assert_eq!(
            pair[0].2, pair[1].1,
            "blocks follow one another without a gap"
        );
    }
    assert_eq!(rows[5].2 - rows[0].1, 420 * 60_000);
    let modes: Vec<&str> = rows.iter().map(|r| r.3.as_str()).collect();
    assert_eq!(modes, ["REVIEW", "STUDY", "LAB", "LAB", "LAB", "STUDY"]);

    // The planner's sessions are found by date, with the others.
    let listed = h
        .call("session.list", json!({ "date": "2026-11-03" }))
        .unwrap();
    assert_eq!(listed["result"]["sessions"].as_array().unwrap().len(), 6);
}

#[test]
fn planning_a_day_also_places_its_blocks_on_the_calendar_where_the_operator_cannot_edit_them() {
    let mut h = imported();
    set_start(&mut h, "2026-11-02");
    let planned = h
        .call("session.plan_day", json!({ "date": "2026-11-03" }))
        .unwrap();
    let day_start = 1_793_664_000_000_i64; // 2026-11-03T00:00:00Z
    let listed = h
        .call(
            "calendar.list",
            json!({ "from": day_start, "to": day_start + 86_400_000 }),
        )
        .unwrap();
    let events = listed["result"]["events"].as_array().unwrap();
    assert_eq!(events.len(), 6);
    assert!(events
        .iter()
        .all(|e| e["kind"] == "STUDY_BLOCK" && e["day_id"] == "D02"));
    assert!(events
        .iter()
        .all(|e| e["provenance"] == "SYSTEM_DETERMINISTIC"));
    assert!(events.iter().all(|e| e["immutable"] == false));
    assert_eq!(events[0]["title"], "D02 recall");
    assert_eq!(events[2]["title"], "D02 guided lab");
    // Each event runs exactly when its session is planned to run, and the session names it.
    let rows: Vec<(String, i64, i64, Option<String>)> = {
        let mut stmt = h
            .conn()
            .prepare(
                "SELECT s.id, s.planned_start_utc, s.planned_end_utc, s.calendar_event_id
                   FROM study_sessions s JOIN study_blocks b ON b.id = s.study_block_id
                  ORDER BY b.ordinal",
            )
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(rows.len(), 6);
    for (i, (_, start, end, event)) in rows.iter().enumerate() {
        assert_eq!(events[i]["start"], *start);
        assert_eq!(events[i]["end"], *end);
        assert_eq!(event.as_deref(), events[i]["event_id"].as_str());
    }
    assert_eq!(planned["result"]["created"].as_array().unwrap().len(), 6);

    // The operator cannot move or delete a block the scheduler placed.
    let id = events[0]["event_id"].as_str().unwrap().to_string();
    let e = h
        .call(
            "calendar.update_event",
            json!({ "event_id": id, "title": "mine" }),
        )
        .unwrap_err();
    assert_eq!(e.code, "calendar.event_not_editable");
    let e = h
        .call("calendar.delete_event", json!({ "event_id": id }))
        .unwrap_err();
    // Refused before an approval is asked: the operator never approves a deletion that cannot happen.
    assert_ne!(e.category, ErrorCategory::ApprovalRequired);
    assert_eq!(e.code, "calendar.event_not_editable");

    // Planning again adds neither sessions nor events.
    h.call("session.plan_day", json!({ "date": "2026-11-03" }))
        .unwrap();
    let events_after: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM calendar_events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(events_after, 6);
}

#[test]
fn the_blind_practical_is_an_assessment_event_the_scheduler_cannot_move() {
    let mut h = imported();
    set_start(&mut h, "2026-11-02");
    h.call("session.plan_day", json!({ "date": "2026-11-07" }))
        .unwrap();
    let (kind, immutable): (String, i64) = h
        .conn()
        .query_row(
            "SELECT e.kind, e.immutable FROM calendar_events e
               JOIN study_blocks b ON b.id = e.study_block_id
              WHERE b.block_code = 'blind_practical'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((kind.as_str(), immutable), ("ASSESSMENT", 1));
}

#[test]
fn planning_a_day_twice_plans_only_what_is_missing() {
    let mut h = imported();
    set_start(&mut h, "2026-11-02");
    h.call("session.plan_day", json!({ "date": "2026-11-03" }))
        .unwrap();
    assert_eq!(session_count(&h), 6);
    let again = h
        .call("session.plan_day", json!({ "date": "2026-11-03" }))
        .unwrap();
    assert!(again["result"]["created"].as_array().unwrap().is_empty());
    assert_eq!(
        again["result"]["already_planned"].as_array().unwrap().len(),
        6
    );
    assert_eq!(session_count(&h), 6);

    // One block of the day loses its session (a deleted row stands for a block that was never
    // planned): the next call plans exactly that block.
    h.conn()
        .execute(
            "DELETE FROM study_sessions WHERE study_block_id IN
               (SELECT id FROM study_blocks WHERE block_code = 'theory')",
            [],
        )
        .unwrap();
    let mended = h
        .call("session.plan_day", json!({ "date": "2026-11-03" }))
        .unwrap();
    let created = mended["result"]["created"].as_array().unwrap();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0]["block"], "theory");
    assert_eq!(
        mended["result"]["already_planned"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    assert_eq!(session_count(&h), 6);
}

#[test]
fn the_same_request_sent_again_returns_the_stored_answer_and_plans_nothing_new() {
    let mut h = imported();
    set_start(&mut h, "2026-11-02");
    let opts = Opts::key("plan-d02");
    let first = h
        .call_with("session.plan_day", json!({ "date": "2026-11-03" }), &opts)
        .unwrap();
    let second = h
        .call_with("session.plan_day", json!({ "date": "2026-11-03" }), &opts)
        .unwrap();
    assert_eq!(first["result"], second["result"]);
    assert_eq!(session_count(&h), 6);
}

#[test]
fn the_saturday_blind_practical_is_planned_in_exam_mode() {
    let mut h = imported();
    set_start(&mut h, "2026-11-02");
    let planned = h
        .call("session.plan_day", json!({ "date": "2026-11-07" }))
        .unwrap();
    let created = planned["result"]["created"].as_array().unwrap();
    assert!(created.iter().any(|s| s["block"] == "blind_practical"));
    let mode: String = h
        .conn()
        .query_row(
            "SELECT s.mode FROM study_sessions s JOIN study_blocks b ON b.id = s.study_block_id
              WHERE b.block_code = 'blind_practical'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(mode, "EXAM");
    let minutes: i64 = created
        .iter()
        .map(|s| s["planned_ms"].as_i64().unwrap() / 60_000)
        .sum();
    assert_eq!(minutes, 300);
}

#[test]
fn a_day_without_blocks_in_the_program_is_refused_and_nothing_is_planned() {
    let mut h = imported();
    set_start(&mut h, "2026-11-02");
    for date in ["2026-11-08", "2026-10-27", "2027-03-01"] {
        let e = h
            .call("session.plan_day", json!({ "date": date }))
            .unwrap_err();
        assert_eq!(e.category, ErrorCategory::NotFound, "{date}");
        assert_eq!(e.code, "curriculum.no_training_day", "{date}");
    }
    let e = h
        .call("session.plan_day", json!({ "date": "2026-13-01" }))
        .unwrap_err();
    assert_eq!(e.code, "date.invalid");
    assert_eq!(session_count(&h), 0);
}

#[test]
fn planning_needs_the_start_and_the_curriculum() {
    let mut h = Harness::new();
    let e = h
        .call("session.plan_day", json!({ "date": "2026-11-03" }))
        .unwrap_err();
    assert_eq!(e.code, "curriculum.not_imported");
    import(&mut h);
    let e = h
        .call("session.plan_day", json!({ "date": "2026-11-03" }))
        .unwrap_err();
    assert_eq!(e.code, "curriculum.program_start_not_configured");
    assert_eq!(session_count(&h), 0);
}

#[test]
fn a_planned_day_starts_at_the_local_hour_in_the_operators_zone() {
    let mut h = Harness::with(|s| s.config.zone = IanaZone::parse("Europe/Berlin").unwrap());
    import(&mut h);
    // Summer time ends on Sunday 2026-10-25: week 1 (Monday 2026-10-19) is still CEST (UTC+2),
    // week 2 (Monday 2026-10-26) is CET (UTC+1).
    h.call(
        "curriculum.set_program_start",
        json!({ "date": "2026-10-19", "day_start": "07:15" }),
    )
    .unwrap();
    h.call("session.plan_day", json!({ "date": "2026-10-23" }))
        .unwrap();
    h.call("session.plan_day", json!({ "date": "2026-10-27" }))
        .unwrap();
    let firsts: Vec<i64> = {
        let mut stmt = h
            .conn()
            .prepare(
                "SELECT MIN(s.planned_start_utc) FROM study_sessions s
                   JOIN training_days d ON d.id = s.training_day_id
                  GROUP BY d.id ORDER BY MIN(s.planned_start_utc)",
            )
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(firsts.len(), 2);
    assert_eq!(firsts[0], 1_792_732_500_000); // 2026-10-23T05:15:00Z = 07:15 CEST
    assert_eq!(firsts[1], 1_793_081_700_000); // 2026-10-27T06:15:00Z = 07:15 CET
}

#[test]
fn planning_and_setting_the_start_leave_events_and_the_audit_chain_intact() {
    let mut h = imported();
    set_start(&mut h, "2026-11-02");
    h.call("session.plan_day", json!({ "date": "2026-11-03" }))
        .unwrap();
    let kinds: Vec<String> = {
        let mut stmt = h
            .conn()
            .prepare(
                "SELECT kind FROM domain_events
                  WHERE kind IN ('curriculum.program_start_set', 'session.day_planned')
                  ORDER BY seq",
            )
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(
        kinds,
        ["curriculum.program_start_set", "session.day_planned"]
    );
    let audit = h.call("audit.recent", json!({})).unwrap();
    assert_eq!(audit["result"]["chain_ok"], true);
}
