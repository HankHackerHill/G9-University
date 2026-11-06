//! Acceptance-gate tests for M03/M04 through the public API only.
//!
//! A02 — recurrence and DST fixtures preserve wall-time intent; conflict
//! detection and protected rest; infeasible schedules are reported.
//! A03 — timer survives suspend/crash/reboot and reports uncertain time
//! instead of double-counting; credit uses monotonic deltas.
//!
//! Expected instants are computed with `chrono::Utc` (independent of the
//! zone-resolution code under test). Transition facts (IANA 2026):
//! Europe/Berlin 29 Mar 02:00→03:00, 25 Oct 03:00→02:00;
//! America/New_York 8 Mar 02:00→03:00, 1 Nov 02:00→01:00;
//! Australia/Sydney 5 Apr 03:00→02:00, 4 Oct 02:00→03:00.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use chrono::{TimeZone, Utc};
use g9_calendar::civil::DAY_MS;
use g9_calendar::*;
use g9_domain::state::SessionEvent;
use g9_domain::{
    Actor, ActorId, CalendarEventId, CivilDateTime, DataDomain, DurationMs, IanaZone, SessionId,
    SessionState, UtcMillis,
};
use pretty_assertions::assert_eq;

fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> UtcMillis {
    UtcMillis::from_millis(
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s)
            .single()
            .unwrap()
            .timestamp_millis(),
    )
}

fn civil(y: i32, mo: u8, d: u8, h: u8, mi: u8, s: u8) -> CivilDateTime {
    CivilDateTime::new(y, mo, d, h, mi, s).unwrap()
}

fn zone(name: &str) -> IanaZone {
    IanaZone::parse(name).unwrap()
}

fn event(
    id: &str,
    kind: EventKind,
    start: CivilDateTime,
    minutes: u32,
    zone: &IanaZone,
) -> CalendarEvent {
    CalendarEvent {
        id: CalendarEventId::parse(id).unwrap(),
        title: id.to_string(),
        kind,
        immutable: false,
        start,
        duration: DurationMs::from_minutes_u32(minutes),
        zone: zone.clone(),
        all_day: false,
        rule: None,
        data_domain: DataDomain::Learning,
    }
}

fn operator() -> Actor {
    Actor::operator(ActorId::parse("operator").unwrap())
}

fn system() -> Actor {
    Actor::system(ActorId::parse("system").unwrap())
}

// ---------------------------------------------------------------- A02 civil

#[test]
fn a02_dst_spring_forward_gap_berlin() {
    let berlin = zone("Europe/Berlin");
    let requested = civil(2026, 3, 29, 2, 30, 0);
    let r = resolve_local(&requested, &berlin).unwrap();
    // 03:30 CEST == 01:30Z
    assert_eq!(r.instant, utc(2026, 3, 29, 1, 30, 0));
    assert_eq!(r.offset_minutes, 120);
    assert_eq!(
        r.adjustment,
        Adjustment::Gap {
            requested,
            shifted_to: civil(2026, 3, 29, 3, 30, 0),
        }
    );
    assert_eq!(
        to_civil(r.instant, &berlin).unwrap(),
        civil(2026, 3, 29, 3, 30, 0)
    );
    // Just outside the gap there is no adjustment.
    assert_eq!(
        resolve_local(&civil(2026, 3, 29, 1, 59, 59), &berlin)
            .unwrap()
            .adjustment,
        Adjustment::None
    );
    assert_eq!(
        resolve_local(&civil(2026, 3, 29, 3, 0, 0), &berlin)
            .unwrap()
            .adjustment,
        Adjustment::None
    );
}

#[test]
fn a02_dst_fall_back_overlap_berlin() {
    let berlin = zone("Europe/Berlin");
    let r = resolve_local(&civil(2026, 10, 25, 2, 30, 0), &berlin).unwrap();
    // Earlier instant = CEST (+02:00) → 00:30Z; alternative = CET → 01:30Z.
    assert_eq!(r.instant, utc(2026, 10, 25, 0, 30, 0));
    assert_eq!(
        r.adjustment,
        Adjustment::Overlap {
            chosen_offset_minutes: 120,
            alternative: utc(2026, 10, 25, 1, 30, 0),
        }
    );
    assert_eq!(
        to_civil(r.instant, &berlin).unwrap(),
        civil(2026, 10, 25, 2, 30, 0)
    );
    assert_eq!(
        to_civil(utc(2026, 10, 25, 1, 30, 0), &berlin).unwrap(),
        civil(2026, 10, 25, 2, 30, 0)
    );
}

#[test]
fn a02_new_york_and_sydney_transitions() {
    let ny = zone("America/New_York");
    // Spring forward 8 Mar 2026: 02:30 does not exist → 03:30 EDT == 07:30Z.
    let r = resolve_local(&civil(2026, 3, 8, 2, 30, 0), &ny).unwrap();
    assert_eq!(r.instant, utc(2026, 3, 8, 7, 30, 0));
    assert!(
        matches!(r.adjustment, Adjustment::Gap { shifted_to, .. } if shifted_to == civil(2026, 3, 8, 3, 30, 0))
    );
    // Fall back 1 Nov 2026: 01:30 twice → EDT (-04:00) == 05:30Z, alt 06:30Z.
    let r = resolve_local(&civil(2026, 11, 1, 1, 30, 0), &ny).unwrap();
    assert_eq!(r.instant, utc(2026, 11, 1, 5, 30, 0));
    assert_eq!(
        r.adjustment,
        Adjustment::Overlap {
            chosen_offset_minutes: -240,
            alternative: utc(2026, 11, 1, 6, 30, 0),
        }
    );

    let sydney = zone("Australia/Sydney");
    // Southern hemisphere: DST starts 4 Oct 2026 (02:00→03:00 AEDT, +11).
    let r = resolve_local(&civil(2026, 10, 4, 2, 30, 0), &sydney).unwrap();
    assert_eq!(r.instant, utc(2026, 10, 3, 16, 30, 0));
    assert!(
        matches!(r.adjustment, Adjustment::Gap { shifted_to, .. } if shifted_to == civil(2026, 10, 4, 3, 30, 0))
    );
    assert_eq!(r.offset_minutes, 660);
    // DST ends 5 Apr 2026 (03:00→02:00): 02:30 twice → AEDT first == 15:30Z.
    let r = resolve_local(&civil(2026, 4, 5, 2, 30, 0), &sydney).unwrap();
    assert_eq!(r.instant, utc(2026, 4, 4, 15, 30, 0));
    assert_eq!(
        r.adjustment,
        Adjustment::Overlap {
            chosen_offset_minutes: 660,
            alternative: utc(2026, 4, 4, 16, 30, 0),
        }
    );

    // UTC fixture: same wall times, never adjusted.
    for c in [civil(2026, 3, 8, 2, 30, 0), civil(2026, 11, 1, 1, 30, 0)] {
        let r = resolve_local(&c, &IanaZone::utc()).unwrap();
        assert_eq!(r.adjustment, Adjustment::None);
        assert_eq!(to_civil(r.instant, &IanaZone::utc()).unwrap(), c);
    }
}

// ------------------------------------------------------------ A02 recurrence

#[test]
fn a02_weekly_byday_recurrence_preserves_wall_time_across_dst() {
    let ny = zone("America/New_York");
    let rule = parse_rrule("FREQ=WEEKLY;BYDAY=MO,WE,FR").unwrap();
    let start = civil(2026, 3, 2, 9, 0, 0); // Monday
    let range = (utc(2026, 3, 1, 0, 0, 0), utc(2026, 3, 15, 0, 0, 0));
    let occ = occurrences(&rule, &start, &ny, range).unwrap();
    assert_eq!(occ.len(), 6);
    let expected = [
        (2, 14), // Mon 2 Mar EST → 14:00Z
        (4, 14),
        (6, 14),
        (9, 13), // Mon 9 Mar EDT → 13:00Z
        (11, 13),
        (13, 13),
    ];
    for (o, (day, utc_hour)) in occ.iter().zip(expected) {
        assert_eq!(o.civil, civil(2026, 3, day, 9, 0, 0));
        assert_eq!(
            o.resolved.instant,
            utc(2026, 3, u32::from(day), utc_hour, 0, 0)
        );
        assert_eq!(o.resolved.adjustment, Adjustment::None);
        assert_eq!(
            to_civil(o.resolved.instant, &ny).unwrap().hour,
            9,
            "wall time intent preserved"
        );
    }
    assert_eq!(
        occ.iter().map(|o| o.seq).collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4, 5]
    );

    // Same rule as a calendar event: expand returns the same six occurrences.
    let mut ev = event("standup", EventKind::StudyBlock, start, 60, &ny);
    ev.rule = Some(rule);
    let resolved = expand(&[ev], range).unwrap();
    assert_eq!(resolved.len(), 6);
    assert_eq!(resolved[3].start, utc(2026, 3, 9, 13, 0, 0));
    assert_eq!(resolved[3].end, utc(2026, 3, 9, 14, 0, 0));
}

#[test]
fn a02_daily_recurrence_through_gap_reports_adjustment_once() {
    let berlin = zone("Europe/Berlin");
    let rule = parse_rrule("FREQ=DAILY").unwrap();
    let occ = occurrences(
        &rule,
        &civil(2026, 3, 28, 2, 30, 0),
        &berlin,
        (utc(2026, 3, 27, 0, 0, 0), utc(2026, 3, 31, 0, 0, 0)),
    )
    .unwrap();
    assert_eq!(occ.len(), 3);
    assert_eq!(occ[0].resolved.adjustment, Adjustment::None);
    assert!(matches!(occ[1].resolved.adjustment, Adjustment::Gap { .. }));
    assert_eq!(occ[1].resolved.instant, utc(2026, 3, 29, 1, 30, 0));
    assert_eq!(occ[2].resolved.adjustment, Adjustment::None);
    assert_eq!(occ[2].resolved.instant, utc(2026, 3, 30, 0, 30, 0));
}

#[test]
fn a02_monthly_31st_skips_short_months() {
    let rule = parse_rrule("FREQ=MONTHLY").unwrap();
    let occ = occurrences(
        &rule,
        &civil(2026, 1, 31, 10, 0, 0),
        &IanaZone::utc(),
        (utc(2026, 1, 1, 0, 0, 0), utc(2027, 1, 1, 0, 0, 0)),
    )
    .unwrap();
    let months: Vec<u8> = occ.iter().map(|o| o.civil.month).collect();
    assert_eq!(months, vec![1, 3, 5, 7, 8, 10, 12]);
    assert!(occ.iter().all(|o| o.civil.day == 31 && o.civil.hour == 10));
    assert_eq!(
        occ.iter().map(|o| o.seq).collect::<Vec<_>>(),
        (0..7).collect::<Vec<_>>()
    );
    // COUNT counts real occurrences only.
    let counted = occurrences(
        &parse_rrule("FREQ=MONTHLY;COUNT=3").unwrap(),
        &civil(2026, 1, 31, 10, 0, 0),
        &IanaZone::utc(),
        (utc(2026, 1, 1, 0, 0, 0), utc(2027, 1, 1, 0, 0, 0)),
    )
    .unwrap();
    assert_eq!(
        counted.iter().map(|o| o.civil.month).collect::<Vec<_>>(),
        vec![1, 3, 5]
    );
    // 30th skips February only.
    let occ = occurrences(
        &rule,
        &civil(2026, 1, 30, 10, 0, 0),
        &IanaZone::utc(),
        (utc(2026, 1, 1, 0, 0, 0), utc(2027, 1, 1, 0, 0, 0)),
    )
    .unwrap();
    assert_eq!(occ.len(), 11);
}

#[test]
fn a02_feb29_only_in_leap_years() {
    let rule = parse_rrule("FREQ=MONTHLY;INTERVAL=12").unwrap();
    let occ = occurrences(
        &rule,
        &civil(2028, 2, 29, 9, 0, 0),
        &IanaZone::utc(),
        (utc(2028, 1, 1, 0, 0, 0), utc(2041, 1, 1, 0, 0, 0)),
    )
    .unwrap();
    let years: Vec<i32> = occ.iter().map(|o| o.civil.year).collect();
    assert_eq!(years, vec![2028, 2032, 2036, 2040]);
    assert!(occ.iter().all(|o| o.civil.month == 2 && o.civil.day == 29));
    assert_eq!(
        occ[1].seq, 1,
        "non-leap years do not consume a sequence number"
    );
    // Across 2100 (not a leap year).
    let occ = occurrences(
        &rule,
        &civil(2096, 2, 29, 9, 0, 0),
        &IanaZone::utc(),
        (utc(2096, 1, 1, 0, 0, 0), utc(2105, 1, 1, 0, 0, 0)),
    )
    .unwrap();
    assert_eq!(
        occ.iter().map(|o| o.civil.year).collect::<Vec<_>>(),
        vec![2096, 2104]
    );
}

#[test]
fn rrule_parse_roundtrip() {
    for text in [
        "FREQ=DAILY",
        "FREQ=DAILY;INTERVAL=2;COUNT=5",
        "FREQ=WEEKLY;BYDAY=MO,WE,FR",
        "FREQ=WEEKLY;INTERVAL=2;BYDAY=TU,TH;UNTIL=20261231",
        "FREQ=MONTHLY;INTERVAL=12",
    ] {
        let rule = parse_rrule(text).unwrap();
        assert_eq!(to_rrule(&rule), text);
        assert_eq!(parse_rrule(&to_rrule(&rule)).unwrap(), rule);
    }
    assert_eq!(
        parse_rrule("RRULE:freq=weekly;byday=FR,mo")
            .unwrap()
            .to_rrule(),
        "FREQ=WEEKLY;BYDAY=MO,FR"
    );
    assert!(parse_rrule("FREQ=YEARLY").is_err());
    assert!(parse_rrule("FREQ=DAILY;UNTIL=20261231;COUNT=2").is_err());
}

// ------------------------------------------------------------- A02 conflicts

#[test]
fn a02_conflicts_detected_and_protected_rest_respected() {
    let berlin = zone("Europe/Berlin");
    // Nightly protected rest 22:00 → 07:00 as a recurring hard event.
    let mut rest = event(
        "rest",
        EventKind::ProtectedRest,
        civil(2026, 3, 1, 22, 0, 0),
        540,
        &berlin,
    );
    rest.rule = Some(parse_rrule("FREQ=DAILY").unwrap());
    let mut dentist = event(
        "dentist",
        EventKind::Appointment,
        civil(2026, 3, 3, 10, 0, 0),
        60,
        &berlin,
    );
    dentist.immutable = true;
    // A study block that collides with the dentist, and one that runs into rest.
    let study_a = event(
        "study-a",
        EventKind::StudyBlock,
        civil(2026, 3, 3, 9, 30, 0),
        90,
        &berlin,
    );
    let study_b = event(
        "study-b",
        EventKind::StudyBlock,
        civil(2026, 3, 3, 21, 0, 0),
        120,
        &berlin,
    );
    let fine = event(
        "fine",
        EventKind::StudyBlock,
        civil(2026, 3, 3, 13, 0, 0),
        60,
        &berlin,
    );
    let range = (utc(2026, 3, 2, 0, 0, 0), utc(2026, 3, 5, 0, 0, 0));
    let resolved = expand(&[rest, dentist, study_a, study_b, fine], range).unwrap();
    let conflicts = find_conflicts(&resolved);
    let pairs: Vec<(String, String, i64, bool)> = conflicts
        .iter()
        .map(|c| {
            (
                c.a.event_id.to_string(),
                c.b.event_id.to_string(),
                c.overlap_ms.as_whole_minutes(),
                c.hard,
            )
        })
        .collect();
    assert_eq!(
        pairs,
        vec![
            ("study-a".into(), "dentist".into(), 60, true),
            ("study-b".into(), "rest".into(), 60, true),
        ]
    );

    // Constraint evaluation names the violations explicitly.
    let constraints = vec![
        Constraint::ImmutableAppointment,
        Constraint::ProtectedRest {
            daily_window: DailyWindow {
                start_minute: 22 * 60,
                end_minute: 7 * 60,
            },
            zone: berlin.clone(),
        },
    ];
    let violations = check_constraints(&resolved, &constraints).unwrap();
    let kinds: Vec<(ConstraintKind, String)> = violations
        .iter()
        .map(|v| (v.constraint, v.subject.clone()))
        .collect();
    assert_eq!(
        kinds,
        vec![
            (
                ConstraintKind::ImmutableAppointment,
                "study-a#0 and dentist#0".into()
            ),
            (ConstraintKind::ProtectedRest, "study-b#0".into()),
        ]
    );

    // The scheduler never places work inside protected rest or over the dentist.
    let fixed: Vec<ResolvedEvent> = resolved
        .iter()
        .filter(|r| r.event_id.as_str() == "rest" || r.event_id.as_str() == "dentist")
        .cloned()
        .collect();
    let tasks = vec![
        ScheduleTask {
            id: "night-owl".into(),
            title: "late block".into(),
            minutes: 120,
            priority_class: PriorityClass::PreferredWorkload,
            earliest: Some(utc(2026, 3, 3, 20, 30, 0)), // 21:30 CET
            deadline: None,
            prerequisites: vec![],
        },
        ScheduleTask {
            id: "morning".into(),
            title: "morning block".into(),
            minutes: 60,
            priority_class: PriorityClass::DueReview,
            earliest: Some(utc(2026, 3, 3, 8, 30, 0)), // 09:30 CET
            deadline: None,
            prerequisites: vec![],
        },
    ];
    // Availability: 3 Mar 06:00 → 4 Mar 09:00 CET (crosses the rest window).
    let windows = vec![(utc(2026, 3, 3, 5, 0, 0), utc(2026, 3, 4, 8, 0, 0))];
    let proposal = propose_schedule(&tasks, &windows, &fixed, &constraints).unwrap();
    assert_eq!(proposal.feasibility, Feasibility::Feasible);
    let by_id = |id: &str| {
        proposal
            .placements
            .iter()
            .find(|p| p.task_id == id)
            .unwrap()
            .clone()
    };
    // morning: 09:30 would hit the dentist at 10:00 → placed after it, 11:00 CET.
    assert_eq!(by_id("morning").start, utc(2026, 3, 3, 10, 0, 0));
    // night-owl: 21:30 CET only leaves 30 min before rest → placed after rest, 07:00 CET next day.
    assert_eq!(by_id("night-owl").start, utc(2026, 3, 4, 6, 0, 0));
    for p in &proposal.placements {
        for f in &fixed {
            assert!(p.end <= f.start || p.start >= f.end, "{p:?} overlaps {f:?}");
        }
    }
}

#[test]
fn a02_infeasible_schedule_reports_violations_not_silent_drop() {
    let berlin = zone("Europe/Berlin");
    let tasks = vec![
        ScheduleTask {
            id: "exam-prep".into(),
            title: "exam prep".into(),
            minutes: 180,
            priority_class: PriorityClass::SafetyOrAssessmentDeadline,
            earliest: None,
            deadline: Some(utc(2026, 3, 3, 10, 0, 0)),
            prerequisites: vec![],
        },
        ScheduleTask {
            id: "review".into(),
            title: "review".into(),
            minutes: 60,
            priority_class: PriorityClass::DueReview,
            earliest: None,
            deadline: None,
            prerequisites: vec![],
        },
        ScheduleTask {
            id: "lab".into(),
            title: "lab".into(),
            minutes: 240,
            priority_class: PriorityClass::PreferredWorkload,
            earliest: None,
            deadline: None,
            prerequisites: vec!["exam-prep".into()],
        },
        ScheduleTask {
            id: "capstone".into(),
            title: "capstone".into(),
            minutes: 600,
            priority_class: PriorityClass::OperatorPriority,
            earliest: None,
            deadline: None,
            prerequisites: vec![],
        },
    ];
    // Four hours available (09:00–13:00 CET) and a 7-hour daily cap. exam-prep
    // would fit, but not before its 11:00 CET deadline.
    let windows = vec![(utc(2026, 3, 3, 8, 0, 0), utc(2026, 3, 3, 12, 0, 0))];
    let constraints = vec![Constraint::DailyCapacity {
        minutes: 420,
        zone: berlin,
    }];
    let p = propose_schedule(&tasks, &windows, &[], &constraints).unwrap();

    // Control: without the deadline the same task is placed, so the deadline is
    // the real blocker and the reported reason is accurate.
    let mut relaxed = tasks.clone();
    relaxed[0].deadline = None;
    let control = propose_schedule(&relaxed, &windows, &[], &constraints).unwrap();
    assert!(control
        .placements
        .iter()
        .any(|pl| pl.task_id == "exam-prep"));

    assert_eq!(
        p.placements.len() + p.unplaced.len(),
        tasks.len(),
        "nothing dropped"
    );
    assert_eq!(p.placements.len(), 1);
    assert_eq!(p.placements[0].task_id, "review");
    let reasons: Vec<(String, UnplacedReason)> = p
        .unplaced
        .iter()
        .map(|u| (u.task_id.clone(), u.reason.clone()))
        .collect();
    assert_eq!(
        reasons,
        vec![
            (
                "exam-prep".into(),
                UnplacedReason::DeadlineUnreachable {
                    deadline: utc(2026, 3, 3, 10, 0, 0)
                }
            ),
            ("capstone".into(), UnplacedReason::NoFittingWindow),
            (
                "lab".into(),
                UnplacedReason::PrerequisiteUnplaced {
                    requires: "exam-prep".into()
                }
            ),
        ]
    );
    match &p.feasibility {
        Feasibility::Infeasible { violations } => {
            assert_eq!(violations.len(), 3);
            let kinds: Vec<ConstraintKind> = violations.iter().map(|v| v.constraint).collect();
            assert_eq!(
                kinds,
                vec![
                    ConstraintKind::AssessmentDeadline,
                    ConstraintKind::Availability,
                    ConstraintKind::PrerequisiteReady,
                ]
            );
            for v in violations {
                assert!(!v.detail.is_empty());
                assert!(
                    !v.detail.contains("exam prep"),
                    "titles never leak into violations"
                );
            }
        }
        Feasibility::Feasible => panic!("must be infeasible"),
    }
    assert!(p
        .constraint_trace
        .iter()
        .any(|l| l.contains("exam-prep") && l.contains("unplaced")));
    let json = serde_json::to_value(&p).unwrap();
    assert_eq!(json["feasibility"]["kind"], "INFEASIBLE");
    assert_eq!(
        json["unplaced"][0]["reason"]["kind"],
        "DEADLINE_UNREACHABLE"
    );
}

#[test]
fn a02_all_day_events_are_date_only() {
    let berlin = zone("Europe/Berlin");
    // All-day on the spring-forward day: local midnight to local midnight is 23 h.
    let mut day = event(
        "offsite",
        EventKind::Appointment,
        civil(2026, 3, 29, 0, 0, 0),
        0,
        &berlin,
    );
    day.all_day = true;
    day.duration = DurationMs::new(DAY_MS).unwrap();
    day.immutable = true;
    let resolved = expand(
        &[day.clone()],
        (utc(2026, 3, 1, 0, 0, 0), utc(2026, 4, 1, 0, 0, 0)),
    )
    .unwrap();
    assert_eq!(resolved.len(), 1);
    assert!(resolved[0].all_day);
    assert_eq!(resolved[0].start, utc(2026, 3, 28, 23, 0, 0));
    assert_eq!(resolved[0].end, utc(2026, 3, 29, 22, 0, 0));
    assert_eq!(resolved[0].span(), DurationMs::new(23 * 3_600_000).unwrap());

    // Fall-back day is 25 h.
    let mut long = day.clone();
    long.start = civil(2026, 10, 25, 0, 0, 0);
    let resolved = expand(
        &[long],
        (utc(2026, 10, 1, 0, 0, 0), utc(2026, 11, 1, 0, 0, 0)),
    )
    .unwrap();
    assert_eq!(resolved[0].span(), DurationMs::new(25 * 3_600_000).unwrap());

    // Two-day all-day event spans two local dates regardless of DST.
    let mut two = day.clone();
    two.duration = DurationMs::new(2 * DAY_MS).unwrap();
    let resolved = expand(&[two], (utc(2026, 3, 1, 0, 0, 0), utc(2026, 4, 1, 0, 0, 0))).unwrap();
    assert_eq!(resolved[0].end, utc(2026, 3, 30, 22, 0, 0));

    // Date-only means no time part: 10:30 is refused.
    let mut timed = day.clone();
    timed.start = civil(2026, 3, 29, 10, 30, 0);
    assert!(matches!(
        timed.validate().unwrap_err(),
        CalendarError::InvalidEvent { .. }
    ));
    // ... and the duration must be whole days.
    let mut partial = day.clone();
    partial.duration = DurationMs::from_minutes_u32(90);
    assert!(partial.validate().is_err());

    // Recurring weekly all-day event keeps whole-day spans across the transition.
    let mut weekly = day;
    weekly.start = civil(2026, 3, 22, 0, 0, 0);
    weekly.rule = Some(parse_rrule("FREQ=WEEKLY;COUNT=3").unwrap());
    let resolved = expand(
        &[weekly],
        (utc(2026, 3, 1, 0, 0, 0), utc(2026, 5, 1, 0, 0, 0)),
    )
    .unwrap();
    let spans: Vec<i64> = resolved
        .iter()
        .map(|r| r.span().as_millis() / 3_600_000)
        .collect();
    assert_eq!(spans, vec![24, 23, 24]);
    assert_eq!(resolved[2].start, utc(2026, 4, 4, 22, 0, 0)); // 5 Apr 00:00 CEST
}

// ------------------------------------------------------------------ A03 timer

#[test]
fn a03_restart_marks_open_interval_uncertain() {
    let mut ledger = Ledger::new();
    let sid = SessionId::parse("session-1").unwrap();
    let mut session = SessionRecord::new(sid.clone());
    let t0 = utc(2026, 3, 3, 9, 0, 0);
    session
        .transition(SessionEvent::Prepare, &operator(), t0)
        .unwrap();
    session
        .transition(SessionEvent::Start, &operator(), t0)
        .unwrap();
    ledger.start(sid.clone(), t0, 1_000).unwrap();

    // ... suspend / crash / reboot; the process comes back 3 hours later.
    let restart = utc(2026, 3, 3, 12, 0, 0);
    let affected = ledger.on_process_restart(restart);
    assert_eq!(affected, vec![sid.clone()]);
    assert_eq!(ledger.process_epoch, 1);
    let iv = &ledger.intervals[0];
    assert_eq!(iv.continuity, Continuity::Uncertain);
    assert_eq!(iv.end_wall, Some(restart));
    assert_eq!(iv.end_mono_ms, None);
    assert!(!iv.is_open());

    // The caller transitions each affected session with the system actor.
    for s in affected {
        assert_eq!(s, session.id);
        session
            .transition(SessionEvent::Interrupt, &system(), restart)
            .unwrap();
    }
    assert_eq!(session.state, SessionState::Interrupted);
    assert_eq!(session.events.last().unwrap().actor, system());

    // A second restart with nothing open affects nobody and is idempotent.
    assert!(ledger.on_process_restart(restart).is_empty());
    assert_eq!(ledger.intervals.len(), 1);
}

#[test]
fn a03_uncertain_time_not_credited_without_correction() {
    let mut ledger = Ledger::new();
    let sid = SessionId::parse("session-1").unwrap();
    let mut session = SessionRecord::new(sid.clone());
    let t0 = utc(2026, 3, 3, 9, 0, 0);
    session
        .transition(SessionEvent::Prepare, &operator(), t0)
        .unwrap();
    session
        .transition(SessionEvent::Start, &operator(), t0)
        .unwrap();

    // A verified 25-minute block first.
    ledger.start(sid.clone(), t0, 0).unwrap();
    ledger
        .stop(&sid, utc(2026, 3, 3, 9, 25, 0), 25 * 60_000)
        .unwrap();
    // Then an interval that is interrupted by a reboot after "3 hours" of wall time.
    ledger
        .start(sid.clone(), utc(2026, 3, 3, 9, 30, 0), 30 * 60_000)
        .unwrap();
    let restart = utc(2026, 3, 3, 12, 30, 0);
    ledger.on_process_restart(restart);
    session
        .transition(SessionEvent::Interrupt, &system(), restart)
        .unwrap();

    let credit = ledger.credited(&sid);
    assert_eq!(credit.verified_ms, DurationMs::from_minutes_u32(25));
    assert_eq!(
        credit.uncertain_ms,
        DurationMs::from_minutes_u32(180),
        "reported, not credited"
    );
    assert_eq!(credit.corrected_ms, DurationMs::ZERO);
    assert_eq!(credit.total_credited_ms, DurationMs::from_minutes_u32(25));

    // Recovery is refused until the operator decides what the uncertain time was worth.
    assert_eq!(
        session.recover(&operator(), restart, &ledger).unwrap_err(),
        SessionError::UncorrectedUncertainTime {
            session_id: sid.clone()
        }
    );
    assert_eq!(session.state, SessionState::Interrupted);

    // A model actor cannot correct; the operator credits 40 minutes with a reason.
    let correction = ManualCorrection {
        id: "corr-1".into(),
        interval_index: 1,
        credited_ms: DurationMs::from_minutes_u32(40),
        reason: "laptop suspended after ~40 min of focused work".into(),
        actor: Actor::model(ActorId::parse("model").unwrap()),
        recorded_at: restart,
    };
    assert_eq!(
        ledger.correct(correction.clone()).unwrap_err(),
        TimerError::ActorNotOperator
    );
    ledger
        .correct(ManualCorrection {
            actor: operator(),
            ..correction
        })
        .unwrap();

    let credit = ledger.credited(&sid);
    assert_eq!(credit.verified_ms, DurationMs::from_minutes_u32(25));
    assert_eq!(credit.uncertain_ms, DurationMs::ZERO);
    assert_eq!(credit.corrected_ms, DurationMs::from_minutes_u32(40));
    assert_eq!(credit.total_credited_ms, DurationMs::from_minutes_u32(65));
    // The original interval row is unchanged (corrections never overwrite).
    assert_eq!(ledger.intervals[1].continuity, Continuity::Uncertain);
    assert_eq!(ledger.intervals[1].end_mono_ms, None);
    assert_eq!(ledger.corrections.len(), 1);

    // Now recovery proceeds (Interrupted → Paused) and work can resume.
    session.recover(&operator(), restart, &ledger).unwrap();
    assert_eq!(session.state, SessionState::Paused);
    session
        .transition(SessionEvent::Resume, &operator(), restart)
        .unwrap();
    assert_eq!(session.state, SessionState::Running);
    ledger.start(sid.clone(), restart, 5_000).unwrap();
    ledger
        .stop(&sid, utc(2026, 3, 3, 12, 40, 0), 5_000 + 10 * 60_000)
        .unwrap();
    assert_eq!(
        ledger.credited(&sid).total_credited_ms,
        DurationMs::from_minutes_u32(75)
    );
}

#[test]
fn a03_overlapping_intervals_rejected() {
    let mut ledger = Ledger::new();
    let a = SessionId::parse("session-a").unwrap();
    let b = SessionId::parse("session-b").unwrap();
    let t = utc(2026, 3, 3, 9, 0, 0);
    ledger.start(a.clone(), t, 1_000).unwrap();
    // Same session twice.
    assert_eq!(
        ledger.start(a.clone(), t, 2_000).unwrap_err(),
        TimerError::AlreadyRunning {
            session_id: a.clone()
        }
    );
    // Another session while one is running: would double-count focused time.
    assert_eq!(
        ledger.start(b.clone(), t, 2_000).unwrap_err(),
        TimerError::AlreadyRunning {
            session_id: a.clone()
        }
    );
    // Stopping the wrong session does not close the open one.
    assert_eq!(
        ledger.stop(&b, t, 3_000).unwrap_err(),
        TimerError::NoOpenInterval {
            session_id: b.clone()
        }
    );
    // An end before the start on the monotonic clock is impossible → rejected.
    assert!(matches!(
        ledger.stop(&a, t, 500).unwrap_err(),
        TimerError::MonotonicRegression {
            reading_ms: 500,
            reference_ms: 1_000
        }
    ));
    ledger.stop(&a, t, 61_000).unwrap();
    // A new interval that starts before the previous one ended (same process) is rejected.
    assert!(matches!(
        ledger.start(b.clone(), t, 60_000).unwrap_err(),
        TimerError::MonotonicRegression {
            reading_ms: 60_000,
            reference_ms: 61_000
        }
    ));
    ledger.start(b.clone(), t, 61_000).unwrap();
    ledger.stop(&b, t, 62_000).unwrap();
    assert_eq!(ledger.intervals.len(), 2);
    assert_eq!(
        ledger.credited(&a).total_credited_ms,
        DurationMs::new(60_000).unwrap()
    );
    assert_eq!(
        ledger.credited(&b).total_credited_ms,
        DurationMs::new(1_000).unwrap()
    );
}

#[test]
fn a03_credit_uses_monotonic_delta_not_wall_clock() {
    let mut ledger = Ledger::new();
    let sid = SessionId::parse("session-1").unwrap();
    let start_wall = utc(2026, 3, 3, 10, 0, 0);
    ledger.start(sid.clone(), start_wall, 100_000).unwrap();
    // The wall clock jumps back two hours (NTP step / manual change) during the session,
    // but 50 minutes really elapsed on the monotonic clock.
    let end_wall = utc(2026, 3, 3, 8, 50, 0);
    ledger.stop(&sid, end_wall, 100_000 + 50 * 60_000).unwrap();
    let credit = ledger.credited(&sid);
    assert_eq!(credit.verified_ms, DurationMs::from_minutes_u32(50));
    assert_eq!(credit.total_credited_ms, DurationMs::from_minutes_u32(50));
    assert_eq!(credit.uncertain_ms, DurationMs::ZERO);
    let iv = &ledger.intervals[0];
    assert_eq!(iv.continuity, Continuity::Verified);
    assert_eq!(
        iv.wall_estimate(),
        DurationMs::ZERO,
        "wall delta is display-only and clamped"
    );
    assert_eq!(iv.mono_elapsed(), Some(DurationMs::from_minutes_u32(50)));
    assert_eq!((iv.start_wall, iv.end_wall), (start_wall, Some(end_wall)));

    // Wall clock jumping forward does not inflate credit either.
    ledger
        .start(sid.clone(), utc(2026, 3, 3, 11, 0, 0), 4_000_000)
        .unwrap();
    ledger
        .stop(&sid, utc(2026, 3, 4, 11, 0, 0), 4_000_000 + 5 * 60_000)
        .unwrap();
    assert_eq!(
        ledger.credited(&sid).total_credited_ms,
        DurationMs::from_minutes_u32(55)
    );
}

#[test]
fn a03_ledger_survives_serialization_roundtrip_after_restart() {
    let mut ledger = Ledger::new();
    let sid = SessionId::parse("session-1").unwrap();
    ledger
        .start(sid.clone(), utc(2026, 3, 3, 9, 0, 0), 1)
        .unwrap();
    ledger.on_process_restart(utc(2026, 3, 3, 9, 5, 0));
    let json = serde_json::to_string(&ledger).unwrap();
    let back: Ledger = serde_json::from_str(&json).unwrap();
    assert_eq!(back, ledger);
    assert!(back.has_uncorrected_uncertain(&sid));
    assert_eq!(back.credited(&sid).total_credited_ms, DurationMs::ZERO);
}
