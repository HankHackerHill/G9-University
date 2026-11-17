//! `g9 perf-fixture` (PERF-1): twelve months of use at the sizes the measurement
//! method of `docs/ui/performance-budgets.md` requires, built into a scratch
//! data directory.
//!
//! ## Sizes
//!
//! | Record | Budget document | How it is produced |
//! |---|---|---|
//! | training days / objectives | 72 / 216 | `curriculum.validate`, `curriculum.import` (approval round trip) of `data/curriculum/g9-ct9-480.draft.v1.json`; not scaled |
//! | sessions | ~1,700 | `session.plan_day` on every training day of the program (408 sessions), `session.create` on the other days |
//! | time intervals | 5,000 | every session: `session.prepare`, `session.start`, one or two `session.pause`/`session.resume` pairs, `session.complete` (one interval per start or resume) |
//! | notes / revisions | 2,000 with 3 revisions | `notes.create` then two `notes.revise`: three stored versions, revision 3 (the budget's "3 revisions" read as the note's revision counter; a reading with 4 versions is one more `notes.revise` per note) |
//! | attempts | 500 | `assessment.begin`, `submit`, `review_evidence`, `grade_provisional`, `approve_final` (approval round trip) on definitions made with `competency.create` and `assessment.define` |
//! | competency observations | 20,000 | 3 per finalized attempt derived by `assessment.approve_final`; the rest **SEEDED**: one learning episode per session and observations on those episodes, through `g9_storage::repos` (no operation creates either: the same precedent as `simulate-week`, only behind `--scratch`) |
//! | audit rows | 50,000 | one per mutating command; the commands above give 23,792 at the budget sizes, the other 26,208 are `retention.record_review` commands spread over the year (the budget fixes the row count, not the command mix) |
//!
//! Not in the budget document, chosen here so that every measured read has
//! data: 20 competencies, 100 assessment definitions, 1,000 review items,
//! 104 calendar appointments (two a week).
//!
//! `--divisor N` divides every scaled count by `N` (rounded up) and keeps every
//! `N`-th day of the year, so a reduced fixture keeps the mix of program days
//! and later days (the tests and the query-plan gate use one).
//!
//! ## Determinism
//!
//! Fixed curriculum, program start ([`PROGRAM_START`]), zone ([`ZONE`]) and
//! inputs; a stepping clock ([`SimClock`]) that moves only by the scripted
//! minutes; sequential request, idempotency and trace ids. Ids the backend
//! mints (uuid v7) and the binding key (OS random) are not reproducible and
//! production id generation is not changed for the fixture: the checksum
//! ([`crate::perf::canonical`]) projects them out. Two builds into two fresh
//! directories give the same checksum (`tests/perf_fixture.rs`).

use crate::bootstrap::{self, BootOptions};
use crate::client::Client;
use crate::clock::SimClock;
use crate::commands::import_seed;
use crate::meta::{IdSource, MetaOptions};
use crate::output::{CategoryStr, CliError};
use crate::perf::canonical;
use crate::seed;
use g9_competency::{CompetencyObservation, ObservationSource};
use g9_curriculum::CivilDate;
use g9_domain::{
    ActorId, AssistanceLevel, Classification, CompetencyId, Confidence, DataDomain, Difficulty,
    DurationMs, EpisodeId, G9Error, IanaZone, InteractionMode, LearningDimension, ObservationId,
    Provenance, Revision, SessionId, SourceKind, TrustClass, UtcMillis,
};
use g9_storage::diagnostics;
use g9_storage::repos::competency as competency_repo;
use g9_storage::repos::retention::{self as retention_repo, EpisodeKind, EpisodeRow};
use g9_storage::Database;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Monday of program week 1.
pub const PROGRAM_START: &str = "2026-01-05";
/// Operator zone.
pub const ZONE: &str = "Europe/Berlin";
/// Days of the timeline (52 weeks).
pub const TIMELINE_DAYS: u32 = 364;
/// Days of the program (12 weeks).
pub const PROGRAM_DAYS: u32 = 84;
/// Local hour the clock jumps to at the start of each day.
pub const CLOCK_DAY_HOUR: u8 = 7;
/// Program day start set through `curriculum.set_program_start`.
pub const DAY_START: &str = "08:00";
/// Local hour of the first session of a day without a program day.
pub const FIRST_SESSION_HOUR: u8 = 8;
/// Length of every pause.
pub const PAUSE_MINUTES: u32 = 5;
/// Local hour of a calendar appointment.
pub const APPOINTMENT_HOUR: u8 = 18;
/// Prefix of the sequential request, idempotency and trace ids.
pub const ID_PREFIX: &str = "perf";
/// Label of rows written through the repositories.
pub const SEEDED: &str = "SEEDED";
/// Provenance reference of the seeded observations.
pub const SEED_SOURCE_REF: &str = "g9-perf-fixture:SEEDED";
/// Grade level per criterion: 2, 3 or 4 by attempt and criterion.
const GRADE_LEVELS: [u8; 3] = [2, 3, 4];
/// Planned minutes of `session.create` sessions, in rotation.
const STANDALONE_MINUTES: [u32; 4] = [30, 45, 60, 90];
/// Modes of `session.create` sessions, in rotation.
const STANDALONE_MODES: [&str; 3] = ["STUDY", "LAB", "REVIEW"];
/// Mutating commands per attempt: begin, submit, review_evidence,
/// grade_provisional, approve_final (refused), approvals.decide, approve_final.
const COMMANDS_PER_ATTEMPT: u64 = 7;
/// Mutating commands of the curriculum import (refused import, decide, retry)
/// plus `curriculum.set_program_start`.
const CURRICULUM_COMMANDS: u64 = 4;
/// Dimensions an attempt's final grade yields per competency (baseline rubric).
pub const DERIVED_PER_ATTEMPT: u32 = 3;

/// Record counts of a fixture. The first seven come from the budget document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Targets {
    /// Study sessions.
    pub sessions: u32,
    /// Time intervals.
    pub intervals: u32,
    /// Notes.
    pub notes: u32,
    /// Stored versions per note.
    pub versions_per_note: u32,
    /// Assessment attempts.
    pub attempts: u32,
    /// Competency observations.
    pub observations: u32,
    /// Audit rows.
    pub audit_rows: u32,
    /// Competencies (not in the budget document).
    pub competencies: u32,
    /// Assessment definitions (not in the budget document).
    pub assessments: u32,
    /// Review items (not in the budget document).
    pub review_items: u32,
    /// Calendar appointments (not in the budget document).
    pub appointments: u32,
}

impl Targets {
    /// The sizes of `docs/ui/performance-budgets.md` (and the chosen extras).
    pub const BUDGET: Self = Self {
        sessions: 1_700,
        intervals: 5_000,
        notes: 2_000,
        versions_per_note: 3,
        attempts: 500,
        observations: 20_000,
        audit_rows: 50_000,
        competencies: 20,
        assessments: 100,
        review_items: 1_000,
        appointments: 104,
    };

    /// Every count divided by `divisor`, rounded up (versions per note kept;
    /// at least two competencies and one definition).
    pub fn divided(divisor: u32) -> Self {
        let d = divisor.max(1);
        let b = Self::BUDGET;
        let div = |n: u32| n.div_ceil(d);
        Self {
            sessions: div(b.sessions),
            intervals: div(b.intervals),
            notes: div(b.notes),
            versions_per_note: b.versions_per_note,
            attempts: div(b.attempts),
            observations: div(b.observations),
            audit_rows: div(b.audit_rows),
            competencies: div(b.competencies).max(2),
            assessments: div(b.assessments).max(1),
            review_items: div(b.review_items),
            appointments: div(b.appointments),
        }
    }
}

/// Options of one build.
#[derive(Debug, Clone)]
pub struct FixtureOptions {
    /// Scratch data directory (created when missing).
    pub data_dir: PathBuf,
    /// Scale divisor (1 = the budget sizes).
    pub divisor: u32,
    /// Curriculum manifest.
    pub manifest: PathBuf,
}

impl FixtureOptions {
    /// Options for `data_dir` at `divisor`, with the curriculum that ships with
    /// the window.
    pub fn new(data_dir: PathBuf, divisor: u32) -> Result<Self, CliError> {
        if divisor == 0 {
            return Err(CliError::Usage("--divisor must be at least 1".to_string()));
        }
        Ok(Self {
            data_dir,
            divisor,
            manifest: default_manifest(),
        })
    }

    /// Targets of this build.
    pub fn targets(&self) -> Targets {
        Targets::divided(self.divisor)
    }
}

/// `data/curriculum/g9-ct9-480.draft.v1.json` (the curriculum that ships with the window).
pub fn default_manifest() -> PathBuf {
    bootstrap::repo_root()
        .join("data")
        .join("curriculum")
        .join("g9-ct9-480.draft.v1.json")
}

/// `share(total, n, i)`: the `i`-th of `n` near-equal parts of `total` (the
/// parts sum to `total` exactly).
pub fn share(total: u64, n: u64, i: u64) -> u64 {
    if n == 0 {
        return 0;
    }
    total * (i + 1) / n - total * i / n
}

/// One day of the timeline that the build visits.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Slot {
    day: u32,
    date: CivilDate,
    /// Program week and day of week (1 = Monday) when the day is a training day.
    training: Option<(u32, u8)>,
    /// Study blocks of the training day (0 otherwise).
    blocks: u64,
}

fn parse_fixed_date(text: &str) -> Result<CivilDate, CliError> {
    seed::parse_date(text).map_err(|_| CliError::Usage(format!("fixture date {text}")))
}

/// Blocks per `(week, day of week)` from the manifest.
fn manifest_blocks(manifest: &Value) -> BTreeMap<(u32, u8), u64> {
    let mut out = BTreeMap::new();
    for week in manifest["weeks"].as_array().into_iter().flatten() {
        let Some(number) = week["week_number"].as_u64() else {
            continue;
        };
        for day in week["days"].as_array().into_iter().flatten() {
            let (Some(dow), Some(blocks)) = (day["day_of_week"].as_u64(), day["blocks"].as_array())
            else {
                continue;
            };
            if let (Ok(w), Ok(d)) = (u32::try_from(number), u8::try_from(dow)) {
                out.insert((w, d), u64::try_from(blocks.len()).unwrap_or(0));
            }
        }
    }
    out
}

fn timeline(
    divisor: u32,
    start: CivilDate,
    blocks: &BTreeMap<(u32, u8), u64>,
) -> Result<Vec<Slot>, CliError> {
    let mut slots = Vec::new();
    let mut day = 0;
    while day < TIMELINE_DAYS {
        let date = start
            .add_days(day)
            .map_err(|_| CliError::Usage("fixture timeline out of range".to_string()))?;
        let training = if day < PROGRAM_DAYS && day % 7 < 6 {
            let week = day / 7 + 1;
            let dow = u8::try_from(day % 7 + 1).unwrap_or(7);
            blocks.get(&(week, dow)).map(|_| (week, dow))
        } else {
            None
        };
        let n = training
            .and_then(|key| blocks.get(&key).copied())
            .unwrap_or(0);
        slots.push(Slot {
            day,
            date,
            training,
            blocks: n,
        });
        day += divisor.max(1);
    }
    Ok(slots)
}

/// What the build did, metadata only (no note text, no key material).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FixtureReport {
    /// `PASS` when every target count was met, else `FAIL`.
    pub result: String,
    /// Scale divisor.
    pub divisor: u32,
    /// Target counts.
    pub targets: Targets,
    /// Budget record counts as stored.
    pub counts: BTreeMap<String, u64>,
    /// Counts that differ from their target (`name: stored != target`).
    pub mismatches: Vec<String>,
    /// Commands dispatched, per operation (refusals that the script expects
    /// are counted under `<op> (APPROVAL_REQUIRED)`).
    pub dispatched: BTreeMap<String, u64>,
    /// Rows written through the repositories, per table (`SEEDED`).
    pub seeded: BTreeMap<String, u64>,
    /// `retention.record_review` commands added after the timeline because
    /// the audit-row plan fell short (0 when the plan was exact).
    pub audit_top_up: u64,
    /// Clock origin and end (epoch ms).
    pub clock: BTreeMap<String, i64>,
    /// The checksum ([`canonical`]).
    pub checksum: canonical::Checksum,
}

impl FixtureReport {
    /// JSON form (the `g9 perf-fixture` output).
    pub fn to_value(&self, data_dir: &std::path::Path) -> Value {
        let mut v = serde_json::to_value(self).unwrap_or(Value::Null);
        v["scenario"] = json!("perf-fixture");
        v["data_dir"] = json!(data_dir);
        v["passed"] = json!(self.result == "PASS");
        v
    }
}

/// A run of one session: id and planned minutes.
#[derive(Debug, Clone)]
struct PlannedSession {
    id: String,
    minutes: u32,
}

/// Session that was completed (for the seeded learning record).
#[derive(Debug, Clone)]
struct DoneSession {
    id: String,
    started: UtcMillis,
    ended: UtcMillis,
}

struct Builder {
    client: Client,
    clock: SimClock,
    zone: IanaZone,
    targets: Targets,
    dispatched: BTreeMap<String, u64>,
    seeded: BTreeMap<String, u64>,
    competencies: Vec<String>,
    assessments: Vec<String>,
    review_items: Vec<String>,
    done: Vec<DoneSession>,
    session_ordinal: u64,
    note_ordinal: u64,
    attempt_ordinal: u64,
    review_ordinal: u64,
    item_ordinal: u64,
    appointment_ordinal: u64,
    pauses_total: u64,
}

/// The `n`-th entry of `list` in rotation; an empty list is an error (the
/// build creates at least one of each before using them).
fn rotate(list: &[String], n: u64, what: &str) -> Result<String, CliError> {
    let len = u64::try_from(list.len()).unwrap_or(0);
    n.checked_rem(len)
        .and_then(|i| usize::try_from(i).ok())
        .and_then(|i| list.get(i))
        .cloned()
        .ok_or_else(|| CliError::Usage(format!("perf-fixture: no {what} to use")))
}

fn failure(step: &str, op: &str, e: &G9Error) -> CliError {
    CliError::Scenario(json!({
        "scenario": "perf-fixture",
        "result": "FAIL",
        "step": step,
        "operation": op,
        "category": e.category_str(),
        "code": e.code,
    }))
}

impl Builder {
    fn count(&mut self, key: &str) {
        *self.dispatched.entry(key.to_string()).or_insert(0) += 1;
    }

    fn call(&mut self, step: &str, op: &str, payload: Value) -> Result<Value, CliError> {
        self.call_with(step, op, payload, &MetaOptions::default())
    }

    fn call_with(
        &mut self,
        step: &str,
        op: &str,
        payload: Value,
        meta: &MetaOptions,
    ) -> Result<Value, CliError> {
        let out = self.client.call_with(op, payload, meta);
        self.count(op);
        match out {
            Ok(v) if v["replayed"] == json!(true) => Err(failure(
                step,
                op,
                &G9Error::conflict("idempotency.replayed", "a stored outcome was replayed"),
            )),
            Ok(v) => Ok(v),
            Err(e) => Err(failure(step, op, &e)),
        }
    }

    fn approved(
        &mut self,
        step: &str,
        op: &str,
        payload: Value,
        meta: &MetaOptions,
    ) -> Result<Value, CliError> {
        let out = self.client.run_approved(op, payload, meta);
        self.count(&format!("{op} (APPROVAL_REQUIRED)"));
        self.count("approvals.decide");
        self.count(op);
        out.map(|(_, v)| v).map_err(|e| failure(step, op, &e))
    }

    fn now(&self) -> UtcMillis {
        self.clock.now()
    }

    fn local(&self, date: CivilDate, hour: u8) -> Result<UtcMillis, CliError> {
        Ok(seed::local_instant(date, hour, &self.zone)?)
    }

    fn audit_rows(&self) -> Result<u64, CliError> {
        Ok(
            diagnostics::row_count(self.client.ctx.database().conn(), "audit_metadata")
                .map_err(G9Error::from)?,
        )
    }

    fn curriculum(&mut self, manifest: &Value) -> Result<(), CliError> {
        let mut seen: Vec<(String, bool)> = Vec::new();
        let imported = {
            let mut observe = |op: &str, res: &Result<Value, G9Error>| {
                let refused = matches!(res, Err(e) if e.category == g9_domain::ErrorCategory::ApprovalRequired);
                seen.push((op.to_string(), refused));
            };
            import_seed::import(&mut self.client, manifest, &mut observe)
        };
        for (op, refused) in seen {
            if refused {
                self.count(&format!("{op} (APPROVAL_REQUIRED)"));
            } else {
                self.count(&op);
            }
        }
        imported?;
        self.call(
            "program",
            "curriculum.set_program_start",
            json!({ "date": PROGRAM_START, "day_start": DAY_START }),
        )?;
        Ok(())
    }

    fn authoring(&mut self) -> Result<(), CliError> {
        for c in 0..self.targets.competencies {
            let out = self.call(
                "authoring",
                "competency.create",
                json!({
                    "code": format!("PERF-C{:02}", c + 1),
                    "title": format!("Fixture competency {}", c + 1),
                    "description": "Synthetic competency of the performance fixture.",
                    "validity_days": 180,
                }),
            )?;
            let id = out["result"]["competency_id"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            self.competencies.push(id);
        }
        for a in 0..self.targets.assessments {
            let competency = rotate(&self.competencies, u64::from(a), "competency")?;
            let out = self.call(
                "authoring",
                "assessment.define",
                json!({
                    "code": format!("PERF-A{:03}", a + 1),
                    "title": format!("Fixture practical {}", a + 1),
                    "kind": "BLIND_PRACTICAL",
                    "competency_id": competency,
                    "task_statement": format!(
                        "Fixture task {}: complete the lab on the unseen environment and document the evidence (synthetic).",
                        a + 1
                    ),
                    "difficulty": a % 5 + 1,
                }),
            )?;
            let id = out["result"]["assessment_id"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            self.assessments.push(id);
        }
        Ok(())
    }

    /// Prepare, start, pause/resume `pauses` times, complete.
    fn run_session(&mut self, step: &str, s: &PlannedSession, pauses: u64) -> Result<(), CliError> {
        let p = json!({ "session_id": s.id });
        self.call(step, "session.prepare", p.clone())?;
        self.call(step, "session.start", p.clone())?;
        let started = self.now();
        let segments = u32::try_from(pauses + 1).unwrap_or(1);
        let segment = (s.minutes / segments).max(1);
        for _ in 0..pauses {
            self.clock.advance_minutes(segment);
            self.call(step, "session.pause", p.clone())?;
            self.clock.advance_minutes(PAUSE_MINUTES);
            self.call(step, "session.resume", p.clone())?;
        }
        let used = segment.saturating_mul(segments.saturating_sub(1));
        self.clock
            .advance_minutes(s.minutes.saturating_sub(used).max(1));
        let done = self.call(step, "session.complete", p)?;
        if done["result"]["state"] != json!("COMPLETED") {
            return Err(failure(
                step,
                "session.complete",
                &G9Error::internal_redacted("perf.session_not_completed"),
            ));
        }
        self.done.push(DoneSession {
            id: s.id.clone(),
            started,
            ended: self.now(),
        });
        self.clock.advance_minutes(1);
        Ok(())
    }

    fn next_pauses(&mut self) -> u64 {
        let k = share(
            self.pauses_total,
            u64::from(self.targets.sessions),
            self.session_ordinal,
        );
        self.session_ordinal += 1;
        k
    }

    fn program_day(&mut self, slot: &Slot, label: &str) -> Result<Option<String>, CliError> {
        let date = seed::format_date(slot.date);
        let plan = self.call(label, "curriculum.daily_plan", json!({ "date": date }))?;
        let objective = plan["result"]["objectives"]
            .as_array()
            .and_then(|o| o.first())
            .and_then(|o| o["objective_id"].as_str())
            .map(str::to_string);
        let planned = self.call(label, "session.plan_day", json!({ "date": date }))?;
        let sessions: Vec<PlannedSession> = planned["result"]["created"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|s| {
                Some(PlannedSession {
                    id: s["session_id"].as_str()?.to_string(),
                    minutes: u32::try_from(s["planned_ms"].as_i64()? / 60_000).ok()?,
                })
            })
            .collect();
        for s in sessions {
            let pauses = self.next_pauses();
            self.run_session(label, &s, pauses)?;
        }
        Ok(objective)
    }

    fn free_day(&mut self, slot: &Slot, label: &str, count: u64) -> Result<(), CliError> {
        let mut cursor = self.local(slot.date, FIRST_SESSION_HOUR)?;
        for _ in 0..count {
            let n = usize::try_from(self.session_ordinal).unwrap_or(0);
            let minutes = STANDALONE_MINUTES[n % STANDALONE_MINUTES.len()];
            let mode = STANDALONE_MODES[n % STANDALONE_MODES.len()];
            let created = self.call(
                label,
                "session.create",
                json!({
                    "planned_minutes": minutes,
                    "planned_start": cursor.as_millis(),
                    "mode": mode,
                }),
            )?;
            let id = created["result"]["session_id"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            self.clock.jump_to(cursor);
            let pauses = self.next_pauses();
            self.run_session(label, &PlannedSession { id, minutes }, pauses)?;
            cursor = self
                .now()
                .checked_add(DurationMs::from_minutes_u32(10))
                .map_err(|_| CliError::Usage("fixture clock overflow".to_string()))?;
        }
        Ok(())
    }

    fn notes(
        &mut self,
        label: &str,
        date: &str,
        count: u64,
        objective: Option<&str>,
    ) -> Result<(), CliError> {
        for _ in 0..count {
            self.note_ordinal += 1;
            let n = self.note_ordinal;
            let mut payload = json!({
                "title": format!("Journal {date} #{n}"),
                "body_markdown": note_body(n, 1),
                "classification": "INTERNAL",
                "domain": "LEARNING",
            });
            if let Some(objective) = objective {
                payload["objective_id"] = json!(objective);
            }
            let created = self.call(label, "notes.create", payload)?;
            let id = created["result"]["note_id"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            self.clock.advance_minutes(2);
            for version in 2..=self.targets.versions_per_note {
                self.call(
                    label,
                    "notes.revise",
                    json!({ "note_id": id, "body_markdown": note_body(n, version) }),
                )?;
                self.clock.advance_minutes(1);
            }
        }
        Ok(())
    }

    fn attempts(&mut self, label: &str, count: u64) -> Result<(), CliError> {
        let criteria_codes = seed::baseline_criteria();
        for _ in 0..count {
            let a = self.attempt_ordinal;
            self.attempt_ordinal += 1;
            let definition = rotate(&self.assessments, a, "assessment definition")?;
            let begun = self.call(
                label,
                "assessment.begin",
                json!({ "assessment_id": definition }),
            )?;
            let attempt = begun["result"]["attempt_id"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            self.clock.advance_minutes(45);
            self.call(
                label,
                "assessment.submit",
                // No `notes`: submission notes are stored as notes, and the
                // budget's 2,000 notes are the journal (3 versions each).
                json!({ "attempt_id": attempt, "evidence_refs": [] }),
            )?;
            self.call(
                label,
                "assessment.review_evidence",
                json!({ "attempt_id": attempt }),
            )?;
            let criteria: Vec<Value> = criteria_codes
                .iter()
                .enumerate()
                .map(|(c, code)| {
                    let level = GRADE_LEVELS
                        [(usize::try_from(a).unwrap_or(0) + c) % GRADE_LEVELS.len()];
                    json!({ "criterion_code": code, "score": level, "source": "HUMAN_REVIEWER", "evidence_refs": [] })
                })
                .collect();
            let graded = self.call(
                label,
                "assessment.grade_provisional",
                json!({ "attempt_id": attempt, "criteria": criteria }),
            )?;
            let meta = MetaOptions {
                expected_revision: graded["revision"].as_u64(),
                ..MetaOptions::default()
            };
            self.approved(
                label,
                "assessment.approve_final",
                json!({ "attempt_id": attempt }),
                &meta,
            )?;
            self.clock.advance_minutes(5);
        }
        Ok(())
    }

    fn review_items(&mut self, label: &str, count: u64) -> Result<(), CliError> {
        for _ in 0..count {
            let i = self.item_ordinal;
            self.item_ordinal += 1;
            let competency = rotate(&self.competencies, i, "competency")?;
            let out = self.call(
                label,
                "retention.create_item",
                // No `answer`: an answer is stored as a note (see `attempts`).
                json!({
                    "prompt": format!("Fixture recall prompt {}", i + 1),
                    "kind": if i % 4 == 3 { "PROCEDURAL" } else { "DECLARATIVE" },
                    "competency_id": competency,
                }),
            )?;
            let id = out["result"]["review_item_id"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            self.review_items.push(id);
        }
        Ok(())
    }

    fn reviews(&mut self, label: &str, count: u64) -> Result<(), CliError> {
        for _ in 0..count {
            if self.review_items.is_empty() {
                return Ok(());
            }
            let r = self.review_ordinal;
            self.review_ordinal += 1;
            let item = rotate(&self.review_items, r, "review item")?;
            self.call(
                label,
                "retention.record_review",
                json!({
                    "review_item_id": item,
                    "outcome": if r % 5 == 4 { "FAILURE" } else { "SUCCESS" },
                    "response_ms": 4_000 + (r % 7) * 1_000,
                }),
            )?;
            self.clock
                .advance(DurationMs::new(30_000).unwrap_or(DurationMs::ZERO));
        }
        Ok(())
    }

    fn appointments(&mut self, label: &str, date: CivilDate, count: u64) -> Result<(), CliError> {
        let start = self.local(date, APPOINTMENT_HOUR)?;
        for _ in 0..count {
            self.appointment_ordinal += 1;
            let end = start
                .checked_add(DurationMs::from_minutes_u32(60))
                .map_err(|_| CliError::Usage("fixture clock overflow".to_string()))?;
            self.call(
                label,
                "calendar.create_event",
                json!({
                    "title": format!("Appointment {}", self.appointment_ordinal),
                    "kind": "APPOINTMENT",
                    "start": start.as_millis(),
                    "end": end.as_millis(),
                }),
            )?;
        }
        Ok(())
    }

    /// One learning episode per completed session and the observations that
    /// the budget needs beyond the derived ones, through the repositories.
    fn seed_learning_record(&mut self, observations: u64) -> Result<(), CliError> {
        let db: &Database = self.client.ctx.database();
        let conn = db.conn();
        let operator: ActorId = self.client.ctx.operator().id.clone();
        let episodes = self.done.clone();
        let competencies: Vec<CompetencyId> = self
            .competencies
            .iter()
            .filter_map(|c| CompetencyId::parse(c).ok())
            .collect();
        if competencies.is_empty() {
            return Ok(());
        }
        let kinds = [
            EpisodeKind::GuidedLab,
            EpisodeKind::IndependentLab,
            EpisodeKind::Theory,
            EpisodeKind::Recall,
        ];
        let mut episode_rows = 0u64;
        let mut observation_rows = 0u64;
        let n_episodes = u64::try_from(episodes.len()).unwrap_or(0);
        for (e, done) in episodes.iter().enumerate() {
            let e64 = u64::try_from(e).unwrap_or(0);
            let competency = competencies[e % competencies.len()].clone();
            let id = EpisodeId::parse(&format!("perf-ep-{:06}", e + 1))
                .map_err(|_| G9Error::internal_redacted("perf.episode_id"))?;
            let difficulty = Difficulty::new(u8::try_from(e % 5 + 1).unwrap_or(1))
                .map_err(|_| G9Error::internal_redacted("perf.difficulty"))?;
            retention_repo::insert_episode(
                conn,
                &EpisodeRow {
                    id: id.clone(),
                    session_id: SessionId::parse(&done.id).ok(),
                    objective_id: None,
                    competency_id: Some(competency.clone()),
                    kind: kinds[e % kinds.len()],
                    started_at: done.started,
                    ended_at: Some(done.ended),
                    assistance_level: AssistanceLevel::Unaided,
                    difficulty: Some(difficulty),
                    outcome: json!({ "source": SEED_SOURCE_REF }),
                    data_domain: DataDomain::Learning,
                    classification: Classification::Private,
                    revision: Revision::INITIAL,
                    created_at: done.ended,
                    updated_at: done.ended,
                },
            )
            .map_err(G9Error::from)?;
            episode_rows += 1;
            for k in 0..share(observations, n_episodes, e64) {
                observation_rows += 1;
                let o = observation_rows;
                let dimension = LearningDimension::ALL
                    [usize::try_from(o).unwrap_or(0) % LearningDimension::ALL.len()];
                let obs = CompetencyObservation {
                    id: ObservationId::parse(&format!("perf-obs-{o:06}"))
                        .map_err(|_| G9Error::internal_redacted("perf.observation_id"))?,
                    competency_id: competency.clone(),
                    dimension,
                    source: ObservationSource::Episode(id.clone()),
                    mode: InteractionMode::Study,
                    difficulty,
                    assistance: AssistanceLevel::Unaided,
                    score: u8::try_from(40 + (o * 7 + k) % 61).unwrap_or(50),
                    environment: "perf-fixture".to_string(),
                    reviewer: None,
                    provenance: Provenance {
                        source_kind: SourceKind::SystemGenerated,
                        source_ref: SEED_SOURCE_REF.to_string(),
                        trust: TrustClass::SystemDeterministic,
                        content_sha256: None,
                        recorded_at: done.ended,
                        recorded_by: operator.clone(),
                        version: None,
                    },
                    uncertainty: Confidence::new(0.5)
                        .map_err(|_| G9Error::internal_redacted("perf.uncertainty"))?,
                    observed_at: done.ended,
                    supersedes: None,
                };
                competency_repo::append_observation(conn, &obs).map_err(G9Error::from)?;
            }
        }
        self.seeded
            .insert("learning_episodes".to_string(), episode_rows);
        self.seeded
            .insert("competency_observations".to_string(), observation_rows);
        Ok(())
    }
}

/// Deterministic markdown of note `n`, version `version`: about 400 bytes,
/// and about 100 more per revision (the budget document fixes no note size).
fn note_body(n: u64, version: u32) -> String {
    let mut body = format!(
        "## Journal entry {n}\n\nWhat I did: worked through the planned blocks, ran the lab, \
         recorded the commands and their output in the lab log, and checked the result against \
         the objective.\n\nWhat went wrong: the first attempt at the exercise missed one \
         prerequisite; the fix was to re-read the reference and repeat the step.\n\n\
         Next: repeat the exercise without notes, then explain the mechanism in my own words.\n"
    );
    for v in 2..=version {
        body.push_str(&format!(
            "\n### Revision {v}\n\nAdded the missing evidence reference and a clearer statement \
             of the result (entry {n}).\n"
        ));
    }
    body
}

/// Counts of the budget's records, as stored.
fn budget_counts(db: &Database) -> Result<BTreeMap<String, u64>, CliError> {
    let mut counts = BTreeMap::new();
    for table in [
        "training_days",
        "daily_objectives",
        "study_sessions",
        "time_intervals",
        "notes",
        "note_versions",
        "attempts",
        "competency_observations",
        "audit_metadata",
    ] {
        counts.insert(
            table.to_string(),
            diagnostics::row_count(db.conn(), table).map_err(G9Error::from)?,
        );
    }
    Ok(counts)
}

/// Build the fixture into `options.data_dir`. `before_context` runs on the
/// opened database before the context exists (the query-plan gate attaches
/// its statement capture there). Returns the report and the still-open
/// client, so that a caller can keep dispatching against the fixture.
pub fn build(
    options: &FixtureOptions,
    before_context: &mut dyn FnMut(&mut Database) -> Result<(), CliError>,
) -> Result<(FixtureReport, Client), CliError> {
    if options.data_dir.join(bootstrap::DATABASE_FILE).exists() {
        return Err(CliError::Usage(
            "perf-fixture builds into a fresh data directory, and this one already holds a database"
                .to_string(),
        ));
    }
    let targets = options.targets();
    let zone = IanaZone::parse(ZONE).map_err(|_| CliError::Usage("fixture zone".to_string()))?;
    let start = parse_fixed_date(PROGRAM_START)?;
    let origin = seed::local_instant(start, CLOCK_DAY_HOUR, &zone)?;
    let clock = SimClock::new(origin);
    let mut boot = BootOptions::new(options.data_dir.clone());
    boot.skills_dir = None;
    boot.zone = Some(zone.clone());
    boot.program_start = Some(start);
    boot.clock = Some(clock.clone());
    boot.native_confirmation = false;
    boot.event_log = false;
    let manifest = import_seed::load_manifest(&options.manifest)?;
    let blocks = manifest_blocks(&manifest);
    let slots = timeline(options.divisor, start, &blocks)?;

    let booted = bootstrap::open_with(&boot, before_context)?;
    let mut b = Builder {
        client: Client::with_ids(booted.ctx, IdSource::sequential(ID_PREFIX)),
        clock,
        zone,
        targets,
        dispatched: BTreeMap::new(),
        seeded: BTreeMap::new(),
        competencies: Vec::new(),
        assessments: Vec::new(),
        review_items: Vec::new(),
        done: Vec::new(),
        session_ordinal: 0,
        note_ordinal: 0,
        attempt_ordinal: 0,
        review_ordinal: 0,
        item_ordinal: 0,
        appointment_ordinal: 0,
        pauses_total: u64::from(targets.intervals.saturating_sub(targets.sessions)),
    };
    let audit_at_open = b.audit_rows()?;
    b.curriculum(&manifest)?;
    b.authoring()?;

    // The plan: every mutating command writes one audit row; the reviews fill
    // what the other commands leave of the audit-row target.
    let n = u64::try_from(slots.len()).unwrap_or(1);
    let training: Vec<&Slot> = slots.iter().filter(|s| s.training.is_some()).collect();
    let free = n - u64::try_from(training.len()).unwrap_or(0);
    let program_sessions: u64 = training.iter().map(|s| s.blocks).sum();
    let standalone = u64::from(targets.sessions).saturating_sub(program_sessions);
    let sessions = program_sessions + standalone;
    let t = targets;
    let planned_other = CURRICULUM_COMMANDS
        + u64::from(t.competencies)
        + u64::from(t.assessments)
        + u64::try_from(training.len()).unwrap_or(0)
        + standalone
        + 3 * sessions
        + 2 * b.pauses_total
        + u64::from(t.notes) * u64::from(t.versions_per_note)
        + COMMANDS_PER_ATTEMPT * u64::from(t.attempts)
        + u64::from(t.review_items)
        + u64::from(t.appointments);
    let reviews_total = u64::from(t.audit_rows)
        .saturating_sub(audit_at_open)
        .saturating_sub(planned_other);

    let mut free_index = 0u64;
    for (i, slot) in slots.iter().enumerate() {
        let i = u64::try_from(i).unwrap_or(0);
        let date = seed::format_date(slot.date);
        let label = format!("day {} ({date})", slot.day);
        let day_start = b.local(slot.date, CLOCK_DAY_HOUR)?;
        b.clock.jump_to(day_start);
        let objective = if slot.training.is_some() {
            b.program_day(slot, &label)?
        } else {
            let count = share(standalone, free, free_index);
            free_index += 1;
            b.free_day(slot, &label, count)?;
            None
        };
        b.notes(
            &label,
            &date,
            share(u64::from(t.notes), n, i),
            objective.as_deref(),
        )?;
        b.attempts(&label, share(u64::from(t.attempts), n, i))?;
        b.review_items(&label, share(u64::from(t.review_items), n, i))?;
        b.reviews(&label, share(reviews_total, n, i))?;
        b.appointments(&label, slot.date, share(u64::from(t.appointments), n, i))?;
    }

    // Exact audit-row count: top up when the plan fell short (expected 0).
    let mut top_up = 0u64;
    while b.audit_rows()? < u64::from(t.audit_rows) && !b.review_items.is_empty() {
        b.reviews("top-up", 1)?;
        top_up += 1;
    }

    let derived = diagnostics::row_count(b.client.ctx.database().conn(), "competency_observations")
        .map_err(G9Error::from)?;
    b.seed_learning_record(u64::from(t.observations).saturating_sub(derived))?;

    let db = b.client.ctx.database();
    let counts = budget_counts(db)?;
    let expected: [(&str, u64); 7] = [
        ("study_sessions", u64::from(t.sessions)),
        ("time_intervals", u64::from(t.intervals)),
        ("notes", u64::from(t.notes)),
        (
            "note_versions",
            u64::from(t.notes) * u64::from(t.versions_per_note),
        ),
        ("attempts", u64::from(t.attempts)),
        ("competency_observations", u64::from(t.observations)),
        ("audit_metadata", u64::from(t.audit_rows)),
    ];
    let mut mismatches = Vec::new();
    for (table, want) in expected {
        let got = counts.get(table).copied().unwrap_or(0);
        if got != want {
            mismatches.push(format!("{table}: {got} != {want}"));
        }
    }
    for (table, want) in [("training_days", 72u64), ("daily_objectives", 216)] {
        let got = counts.get(table).copied().unwrap_or(0);
        if got != want {
            mismatches.push(format!("{table}: {got} != {want}"));
        }
    }
    let checksum = canonical::checksum(db)?;
    let mut clock = BTreeMap::new();
    clock.insert("origin_ms".to_string(), origin.as_millis());
    clock.insert("end_ms".to_string(), b.now().as_millis());
    let report = FixtureReport {
        result: if mismatches.is_empty() {
            "PASS"
        } else {
            "FAIL"
        }
        .to_string(),
        divisor: options.divisor,
        targets,
        counts,
        mismatches,
        dispatched: b.dispatched,
        seeded: b.seeded,
        audit_top_up: top_up,
        clock,
        checksum,
    };
    Ok((report, b.client))
}

/// `g9 perf-fixture`: build and report. `PASS` exits 0; `FAIL` (a count that
/// missed its target) exits 2 with the report.
pub fn run(options: &FixtureOptions) -> Result<Value, CliError> {
    let (report, client) = build(options, &mut |_| Ok(()))?;
    drop(client);
    let value = report.to_value(&options.data_dir);
    if report.result == "PASS" {
        Ok(value)
    } else {
        Err(CliError::Scenario(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shares_sum_to_the_total_and_differ_by_at_most_one() {
        for (total, n) in [(3_300u64, 1_700u64), (1_292, 292), (7, 3), (0, 5), (5, 1)] {
            let parts: Vec<u64> = (0..n).map(|i| share(total, n, i)).collect();
            assert_eq!(parts.iter().sum::<u64>(), total);
            let (lo, hi) = (parts.iter().min().unwrap(), parts.iter().max().unwrap());
            assert!(hi - lo <= 1, "{total}/{n}");
        }
        assert_eq!(share(5, 0, 0), 0);
    }

    #[test]
    fn budget_targets_and_division() {
        let b = Targets::BUDGET;
        assert_eq!(
            (
                b.sessions,
                b.intervals,
                b.notes,
                b.versions_per_note,
                b.attempts,
                b.observations,
                b.audit_rows
            ),
            (1_700, 5_000, 2_000, 3, 500, 20_000, 50_000)
        );
        assert_eq!(Targets::divided(1), b);
        let d = Targets::divided(50);
        assert_eq!(
            (d.sessions, d.intervals, d.notes, d.attempts),
            (34, 100, 40, 10)
        );
        assert_eq!(
            (d.competencies, d.assessments, d.versions_per_note),
            (2, 2, 3)
        );
        assert!(FixtureOptions::new(PathBuf::from("/x"), 0).is_err());
    }

    #[test]
    fn the_timeline_covers_the_year_and_the_program_days() {
        let manifest: Value =
            serde_json::from_str(&std::fs::read_to_string(default_manifest()).unwrap()).unwrap();
        let blocks = manifest_blocks(&manifest);
        assert_eq!(blocks.len(), 72);
        assert_eq!(blocks.values().sum::<u64>(), 408);
        let start = parse_fixed_date(PROGRAM_START).unwrap();
        assert_eq!(start.iso_weekday(), 1);
        let full = timeline(1, start, &blocks).unwrap();
        assert_eq!(full.len(), 364);
        assert_eq!(full.iter().filter(|s| s.training.is_some()).count(), 72);
        assert_eq!(full.iter().map(|s| s.blocks).sum::<u64>(), 408);
        let reduced = timeline(20, start, &blocks).unwrap();
        assert_eq!(reduced.len(), 19);
        assert!(reduced.iter().any(|s| s.training.is_some()));
        assert!(reduced.iter().any(|s| s.training.is_none()));
    }

    #[test]
    fn rotation_cycles_and_refuses_an_empty_list() {
        let list = vec!["a".to_string(), "b".to_string()];
        assert_eq!(rotate(&list, 0, "x").unwrap(), "a");
        assert_eq!(rotate(&list, 3, "x").unwrap(), "b");
        assert!(rotate(&[], 0, "x").is_err());
    }

    #[test]
    fn note_bodies_grow_with_each_version() {
        let first = note_body(1, 1).len();
        assert!((350..450).contains(&first), "{first}");
        assert!(note_body(1, 3).len() > first + 150);
        assert!(note_body(1, 3).contains("Revision 3"));
        assert_ne!(note_body(1, 1), note_body(2, 1));
    }
}
