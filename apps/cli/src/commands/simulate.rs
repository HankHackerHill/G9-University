//! `g9 simulate-week`: the Stage 03 exit criterion ("full simulated week and
//! crash recovery") as a deterministic scripted scenario over the real backend.
//!
//! Sequence (every row of the step table is one of these):
//!
//! | # | Step | Path |
//! |---|---|---|
//! | 0 | open the data directory; recovery runs | `AppContext::builder(..).open()` |
//! | 1 | probe `curriculum.daily_plan`; when no version exists: `curriculum.validate`, `curriculum.import` (`APPROVAL_REQUIRED` expected), `approvals.decide`, retry | `dispatch` |
//! | 2 | materialize one `PLANNED` session per study block of the six days, and the Saturday blind practical (competency, definition, version with the seed-declared difficulty 3, baseline rubric) | `g9-storage` repositories (`SEEDED`; `FILL_CURRICULUM` and catalogue import are PROPOSED) |
//! | 3 | per day Monday..Saturday: `curriculum.daily_plan`; per block `session.prepare`, `session.start`, clock advance (one `session.pause`/`session.resume` on the third block), `session.complete` (credited time must equal the block minutes); `notes.create` (learning journal) | `dispatch` |
//! | 4 | Saturday: `assessment.begin`, `assessment.submit`, `assessment.review_evidence`, `assessment.grade_provisional`, `assessment.approve_final` (`APPROVAL_REQUIRED` expected), `approvals.decide`, retry → `APPROVED_FINAL` | `dispatch` |
//! | 5 | `retention.due`, `competency.projection`; checks: the projection is non-empty and its `evidence_refs` include every observation derived from the finalized attempt, and the self-adjudicated grade never validates (`provisional`, no `VALIDATED`) | `dispatch` + `competency_repo::observations_for_attempt` |
//! | 6 | `backup.create`, `backup.verify`, `backup.restore_test` → `RETAINED` | `dispatch` |
//! | 7 | `audit.recent`; the hash chain verifies and has more rows than at open | `dispatch` + `AuditChain::verify_chain` |
//!
//! The clock is a stepping clock: it starts at `--fixed-clock` (or 08:00 local
//! on the start date) and advances only by the block minutes, the pause and
//! the day jumps. With `--fixed-clock` the request/idempotency/trace ids are
//! sequential as well, so the step table is byte-for-byte reproducible
//! (backend-generated ids live in `artifacts` only).
//!
//! Verdict (`result`): `PASS` only when every step ran and every check passed;
//! `INCOMPLETE` when nothing failed but at least one step answered
//! `PRECONDITION_FAILED <area>.not_implemented` (recorded `NOT RUN (PROPOSED)`,
//! listed in `not_run`); `FAIL` otherwise. Only `PASS` exits 0: a week with
//! steps that did not run is not the Stage 03 exit criterion, so `INCOMPLETE`
//! and `FAIL` both exit 2 with the report on stdout.
//!
//! A mutating step whose outcome comes back `replayed = true` is a failure
//! (`FAIL: idempotency.replayed`), never a pass: the backend returned the
//! stored outcome of an earlier command with the same `(actor, idempotency
//! key)` and executed nothing. Within one run every key is fresh, so a replay
//! only happens when the same week is run again in the same data directory
//! with the same `--fixed-clock` (identical sequential keys); without the check
//! such a re-run would "pass" every credited-time check on stored results.

use crate::bootstrap::{self, BootOptions};
use crate::cli::{Cli, SimulateArgs};
use crate::client::Client;
use crate::clock::SimClock;
use crate::commands::import_seed;
use crate::meta::{IdSource, MetaOptions};
use crate::output::{CategoryStr, CliError};
use crate::seed::{self, SeedReport, DAY_START_HOUR};
use g9_curriculum::CivilDate;
use g9_domain::{AttemptId, ErrorCategory, G9Error, IanaZone, UtcMillis};
use g9_storage::repos::competency as competency_repo;
use g9_storage::AuditChain;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::path::PathBuf;

/// Zone used when `--zone` is absent.
pub const DEFAULT_ZONE: &str = "Europe/Berlin";
/// Break length of the one pause per day.
pub const PAUSE_MINUTES: u32 = 5;
/// Anchored level given to every criterion of the Saturday practical.
pub const GRADE_LEVEL: u8 = 3;
/// Milliseconds per day (fixed-clock day jumps).
const DAY_MS: i64 = 86_400_000;
/// Added to `failures` after a replayed outcome.
pub const REPLAY_HINT: &str = "hint: the backend replayed the stored outcome of an earlier command with the \
     same idempotency key and executed nothing; this week was already simulated in this data \
     directory with the same --fixed-clock (sequential ids); use a fresh --data-dir or another --week";

/// Resolved options of one run.
#[derive(Debug, Clone)]
pub struct SimulateOptions {
    /// Context options (zone, program start and stepping clock are set here).
    pub boot: BootOptions,
    /// Monday of the simulated week.
    pub start_date: CivilDate,
    /// Program week of that Monday.
    pub week: u32,
    /// Monday of program week 1.
    pub program_start: CivilDate,
    /// Clock origin when fixed.
    pub fixed_clock: Option<i64>,
    /// Seed manifest imported when no curriculum version exists.
    pub manifest: PathBuf,
    /// Operator zone.
    pub zone: IanaZone,
}

impl SimulateOptions {
    /// From the parsed command line. The program start is derived from
    /// `--start-date` and `--week`; an explicit `--program-start` must agree
    /// with it (it is never silently overridden).
    pub fn from_cli(cli: &Cli, args: &SimulateArgs) -> Result<Self, CliError> {
        let boot = super::boot_options(cli)?;
        let explicit_program_start = boot.program_start;
        let zone = match &cli.zone {
            Some(z) => super::parse_zone(z)?,
            None => IanaZone::parse(DEFAULT_ZONE)
                .map_err(|_| CliError::Usage("default zone unavailable".to_string()))?,
        };
        let start_date = seed::parse_date(&args.start_date).map_err(|_| {
            CliError::Usage(format!(
                "--start-date {:?}: dates are YYYY-MM-DD",
                args.start_date
            ))
        })?;
        let options = Self::new(
            boot,
            start_date,
            args.week,
            args.fixed_clock,
            args.manifest
                .clone()
                .unwrap_or_else(bootstrap::default_seed_manifest),
            zone,
        )?;
        if let Some(explicit) = explicit_program_start {
            if explicit != options.program_start {
                return Err(CliError::Usage(format!(
                    "--program-start {} contradicts --start-date {} --week {} (program week {} starts on {})",
                    seed::format_date(explicit),
                    seed::format_date(start_date),
                    args.week,
                    args.week,
                    seed::format_date(options.program_start),
                )));
            }
        }
        Ok(options)
    }

    /// Validate and derive program start and clock origin.
    pub fn new(
        mut boot: BootOptions,
        start_date: CivilDate,
        week: u32,
        fixed_clock: Option<i64>,
        manifest: PathBuf,
        zone: IanaZone,
    ) -> Result<Self, CliError> {
        if start_date.iso_weekday() != 1 {
            return Err(CliError::Usage(format!(
                "--start-date {} is not a Monday",
                seed::format_date(start_date)
            )));
        }
        if !(1..=12).contains(&week) {
            return Err(CliError::Usage("--week must be 1..=12".to_string()));
        }
        let program_start = CivilDate::from_days(start_date.to_days() - 7 * i64::from(week - 1))
            .map_err(|_| CliError::Usage("start date out of range".to_string()))?;
        let origin = match fixed_clock {
            Some(ms) => UtcMillis::from_millis(ms),
            None => seed::local_instant(start_date, DAY_START_HOUR, &zone)?,
        };
        boot.zone = Some(zone.clone());
        boot.program_start = Some(program_start);
        boot.clock = Some(SimClock::new(origin));
        Ok(Self {
            boot,
            start_date,
            week,
            program_start,
            fixed_clock,
            manifest,
            zone,
        })
    }
}

/// One row of the step table.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Step {
    /// 1-based step number.
    pub step: u32,
    /// Operation name, or a `boot`/`probe:`/`seed:`/`check:` label.
    pub op: String,
    /// `OK`, the closed error category, `REPOSITORY`, `CHECK`.
    pub category: String,
    /// `OK`, `<CATEGORY> (expected)`, `SEEDED`, `PASS`, `FAIL: <code>` (including
    /// `FAIL: idempotency.replayed`), `NOT RUN (PROPOSED)`.
    pub decision: String,
    /// Metadata-only state after the step (never note bodies or key material).
    pub state_after: Value,
}

/// One evaluated check.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Check {
    /// What was checked.
    pub name: String,
    /// Result.
    pub passed: bool,
    /// Observed values.
    pub detail: Value,
}

/// Early exit marker: the failure is already in the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Halt;

/// What the backend may answer for a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Expect {
    Ok,
    Category(&'static str),
}

/// Outcome of a recorded dispatch.
#[derive(Debug)]
enum Outcome {
    Ok(Value),
    Expected(G9Error),
    NotRun,
}

struct Sim {
    client: Client,
    clock: SimClock,
    zone: IanaZone,
    fixed: bool,
    origin: UtcMillis,
    steps: Vec<Step>,
    checks: Vec<Check>,
    failures: Vec<String>,
    not_run: Vec<String>,
    artifacts: Map<String, Value>,
}

impl Sim {
    fn row(&mut self, op: &str, category: &str, decision: &str, state_after: Value) -> u32 {
        let step = u32::try_from(self.steps.len() + 1).unwrap_or(u32::MAX);
        self.steps.push(Step {
            step,
            op: op.to_string(),
            category: category.to_string(),
            decision: decision.to_string(),
            state_after,
        });
        step
    }

    fn fail(&mut self, op: &str, category: &str, code: &str, state_after: Value) -> Halt {
        let step = self.row(op, category, &format!("FAIL: {code}"), state_after);
        self.failures
            .push(format!("step {step}: {op} -> {category} {code}"));
        Halt
    }

    fn dispatch(
        &mut self,
        op: &str,
        payload: Value,
        meta: &MetaOptions,
        expect: Expect,
    ) -> Result<Outcome, Halt> {
        let result = self.client.call_with(op, payload, meta);
        let state_after = summarize(op, &result);
        match (expect, result) {
            (Expect::Ok, Ok(out)) if out["replayed"] == json!(true) => {
                // The command did not execute: a stored outcome was returned.
                let halt = self.fail(op, "OK", "idempotency.replayed", state_after);
                self.failures.push(REPLAY_HINT.to_string());
                Err(halt)
            }
            (Expect::Ok, Ok(out)) => {
                self.row(op, "OK", "OK", state_after);
                Ok(Outcome::Ok(out))
            }
            (Expect::Category(c), Err(e)) if e.category_str() == c => {
                self.row(op, c, &format!("{c} (expected)"), state_after);
                Ok(Outcome::Expected(e))
            }
            (_, Err(e))
                if e.category == ErrorCategory::PreconditionFailed
                    && e.code.ends_with(".not_implemented") =>
            {
                let category = e.category_str();
                let step = self.row(op, &category, "NOT RUN (PROPOSED)", state_after);
                self.not_run
                    .push(format!("step {step}: {op} -> {}", e.code));
                Ok(Outcome::NotRun)
            }
            (Expect::Ok, Err(e)) => Err(self.fail(op, &e.category_str(), &e.code, state_after)),
            (Expect::Category(c), Ok(_)) => Err(self.fail(
                op,
                "OK",
                &format!("expected {c}, the command succeeded"),
                state_after,
            )),
            (Expect::Category(c), Err(e)) => Err(self.fail(
                op,
                &e.category_str(),
                &format!("expected {c}, got {}", e.code),
                state_after,
            )),
        }
    }

    /// Dispatch that must succeed; `None` when the operation is PROPOSED.
    fn ok(&mut self, op: &str, payload: Value, meta: &MetaOptions) -> Result<Option<Value>, Halt> {
        match self.dispatch(op, payload, meta, Expect::Ok)? {
            Outcome::Ok(v) => Ok(Some(v)),
            Outcome::NotRun => Ok(None),
            Outcome::Expected(_) => Err(Halt),
        }
    }

    /// Dispatch that must be refused with `category`; `None` when PROPOSED.
    fn refused(
        &mut self,
        op: &str,
        payload: Value,
        meta: &MetaOptions,
        category: &'static str,
    ) -> Result<Option<G9Error>, Halt> {
        match self.dispatch(op, payload, meta, Expect::Category(category))? {
            Outcome::Expected(e) => Ok(Some(e)),
            Outcome::NotRun => Ok(None),
            Outcome::Ok(_) => Err(Halt),
        }
    }

    fn check(&mut self, name: &str, passed: bool, detail: Value) {
        self.row(
            &format!("check:{name}"),
            "CHECK",
            if passed { "PASS" } else { "FAIL" },
            detail.clone(),
        );
        if !passed {
            self.failures.push(format!("check failed: {name} {detail}"));
        }
        self.checks.push(Check {
            name: name.to_string(),
            passed,
            detail,
        });
    }

    fn seeded(
        &mut self,
        label: &str,
        outcome: Result<SeedReport, G9Error>,
    ) -> Result<SeedReport, Halt> {
        match outcome {
            Ok(report) => {
                self.row(
                    label,
                    "REPOSITORY",
                    "SEEDED",
                    json!({ "created": report.created, "existing": report.existing }),
                );
                Ok(report)
            }
            Err(e) => Err(self.fail(
                label,
                &e.category_str(),
                &e.code,
                json!({ "category": e.category_str(), "code": e.code }),
            )),
        }
    }

    fn day_start(&self, k: usize, date: CivilDate) -> Result<UtcMillis, G9Error> {
        if self.fixed {
            let offset = i64::try_from(k).unwrap_or(0) * DAY_MS;
            Ok(UtcMillis::from_millis(self.origin.as_millis() + offset))
        } else {
            seed::local_instant(date, DAY_START_HOUR, &self.zone)
        }
    }
}

fn tail(id: &str) -> &str {
    id.rsplit_once(':').map_or(id, |(_, t)| t)
}

fn pick(result: &Value, keys: &[&str]) -> Value {
    let mut m = Map::new();
    for k in keys {
        if let Some(v) = result.get(k) {
            m.insert((*k).to_string(), v.clone());
        }
    }
    Value::Object(m)
}

fn len_of(v: &Value) -> usize {
    v.as_array().map_or(0, Vec::len)
}

/// Metadata-only summary of a step's answer.
fn summarize(op: &str, result: &Result<Value, G9Error>) -> Value {
    match result {
        Err(e) => {
            let mut v = json!({ "category": e.category_str(), "code": e.code });
            if e.approval_id.is_some() {
                v["approval_requested"] = json!(true);
            }
            v
        }
        Ok(out) => {
            let r = &out["result"];
            let area = op.split('.').next().unwrap_or(op);
            match (area, op) {
                ("session", _) => pick(
                    r,
                    &[
                        "state",
                        "credited_ms",
                        "uncertain_ms",
                        "requires_correction",
                    ],
                ),
                ("assessment", _) => {
                    let mut v = pick(r, &["state", "variant"]);
                    if let Some(grade) = r.get("grade").filter(|g| !g.is_null()) {
                        v["grade_total"] = grade["total"].clone();
                        v["grade_pass"] = grade["pass"].clone();
                        v["grade_final"] = grade["final"].clone();
                        v["reliability_class"] = grade["reliability_class"].clone();
                    }
                    v
                }
                (_, "curriculum.daily_plan") => json!({
                    "day_id": r["day_id"],
                    "week_number": r["week_number"],
                    "is_assessment_day": r["is_assessment_day"],
                    "unscheduled": r["unscheduled"],
                    "blocks": len_of(&r["blocks"]),
                    "sessions_linked": r["blocks"].as_array().map_or(0, |b| b.iter().filter(|x| !x["session_id"].is_null()).count()),
                }),
                (_, "curriculum.import") => pick(
                    r,
                    &[
                        "curriculum_version_id",
                        "provenance",
                        "weeks",
                        "days",
                        "objectives",
                    ],
                ),
                (_, "curriculum.validate") => pick(r, &["valid", "checksum_sha256"]),
                ("approvals", _) => pick(r, &["state", "operation_name"]),
                ("notes", _) => pick(r, &["classification", "domain", "version_count"]),
                ("backup", _) => pick(r, &["state", "integrity"]),
                ("retention", _) => json!({
                    "due": len_of(&r["due"]),
                    "overdue": len_of(&r["overdue"]),
                    "overflow": len_of(&r["overflow"]),
                }),
                ("competency", _) => {
                    let projections = r["projections"].as_array().map_or(&[][..], Vec::as_slice);
                    json!({
                        "projections": projections.len(),
                        "evidence_refs": projections.iter().map(|p| len_of(&p["evidence_refs"])).sum::<usize>(),
                        "provisional": projections.iter().filter(|p| p["provisional"] == json!(true)).count(),
                    })
                }
                ("audit", _) => {
                    json!({ "entries": len_of(&r["entries"]), "chain_ok": r["chain_ok"] })
                }
                ("system", _) => {
                    json!({ "storage": r["storage"]["state"], "model_available": r["model"]["available"] })
                }
                _ => json!({ "ok": true }),
            }
        }
    }
}

fn ensure_curriculum(sim: &mut Sim, o: &SimulateOptions) -> Result<(), Halt> {
    let date = seed::format_date(o.start_date);
    let probe = sim
        .client
        .call("curriculum.daily_plan", json!({ "date": date }));
    match probe {
        Ok(out) => {
            let state = summarize("curriculum.daily_plan", &Ok(out));
            sim.row("probe:curriculum.daily_plan", "OK", "PRESENT", state);
            return Ok(());
        }
        Err(e) if e.code == "curriculum.not_imported" => {
            sim.row(
                "probe:curriculum.daily_plan",
                &e.category_str(),
                "ABSENT (import follows)",
                json!({ "code": e.code }),
            );
        }
        Err(e) => {
            let state = summarize("curriculum.daily_plan", &Err(e.clone()));
            return Err(sim.fail(
                "probe:curriculum.daily_plan",
                &e.category_str(),
                &e.code,
                state,
            ));
        }
    }
    let manifest = match import_seed::load_manifest(&o.manifest) {
        Ok(m) => m,
        Err(e) => {
            return Err(sim.fail(
                "load:manifest",
                "IO",
                &e.to_string(),
                json!({ "manifest": o.manifest }),
            ))
        }
    };
    let mut observed: Vec<(String, Result<Value, G9Error>)> = Vec::new();
    let imported = {
        let mut observe = |op: &str, res: &Result<Value, G9Error>| {
            observed.push((op.to_string(), res.clone()));
        };
        import_seed::import(&mut sim.client, &manifest, &mut observe)
    };
    for (op, res) in observed {
        let state = summarize(&op, &res);
        match &res {
            Ok(out) if out["replayed"] == json!(true) => {
                sim.fail(&op, "OK", "idempotency.replayed", state);
                sim.failures.push(REPLAY_HINT.to_string());
            }
            Ok(_) => {
                sim.row(&op, "OK", "OK", state);
            }
            Err(e)
                if op == "curriculum.import" && e.category == ErrorCategory::ApprovalRequired =>
            {
                sim.row(
                    &op,
                    "APPROVAL_REQUIRED",
                    "APPROVAL_REQUIRED (expected)",
                    state,
                );
            }
            Err(e) => {
                sim.fail(&op, &e.category_str(), &e.code, state);
            }
        }
    }
    match imported {
        Ok(v) => {
            sim.artifacts.insert(
                "curriculum_version_id".into(),
                v["curriculum_version_id"].clone(),
            );
            sim.artifacts
                .insert("import_approval_id".into(), v["approval_id"].clone());
            Ok(())
        }
        Err(CliError::Backend(e)) => {
            if sim.failures.is_empty() {
                let state = json!({ "category": e.category_str(), "code": e.code });
                sim.fail("import-seed", &e.category_str(), &e.code, state);
            }
            Err(Halt)
        }
        Err(other) => Err(sim.fail("import-seed", "IO", &other.to_string(), json!({}))),
    }
}

struct DayPlan {
    date: CivilDate,
    day: g9_storage::repos::curriculum::TrainingDayRow,
}

fn resolve_days(sim: &mut Sim, o: &SimulateOptions) -> Result<Vec<DayPlan>, Halt> {
    let version = match seed::active_version(sim.client.ctx.database()) {
        Ok(Some(v)) => v,
        Ok(None) => {
            return Err(sim.fail(
                "resolve:curriculum",
                "NOT_FOUND",
                "curriculum.not_imported",
                json!({}),
            ))
        }
        Err(e) => {
            return Err(sim.fail("resolve:curriculum", &e.category_str(), &e.code, json!({})))
        }
    };
    sim.artifacts
        .entry("curriculum_version_id")
        .or_insert_with(|| json!(version.id));
    let mut days = Vec::new();
    for k in 0..6u32 {
        let date = match o.start_date.add_days(k) {
            Ok(d) => d,
            Err(_) => {
                return Err(sim.fail("resolve:days", "INVALID_INPUT", "date.overflow", json!({})))
            }
        };
        match seed::training_day(sim.client.ctx.database(), &version, o.program_start, date) {
            Ok(Some(day)) => days.push(DayPlan { date, day }),
            Ok(None) => {
                return Err(sim.fail(
                    "resolve:days",
                    "NOT_FOUND",
                    "curriculum.day_not_found",
                    json!({ "date": seed::format_date(date), "week": o.week }),
                ))
            }
            Err(e) => return Err(sim.fail("resolve:days", &e.category_str(), &e.code, json!({}))),
        }
    }
    sim.row(
        "resolve:days",
        "OK",
        "RESOLVED",
        json!({
            "version_provenance": version.provenance,
            "days": days.iter().map(|d| tail(d.day.day_code.as_str()).to_string()).collect::<Vec<_>>(),
        }),
    );
    Ok(days)
}

fn seed_week(sim: &mut Sim, o: &SimulateOptions, days: &[DayPlan]) -> Result<(), Halt> {
    let now = sim.client.now();
    let mut total = SeedReport::default();
    let mut failure: Option<G9Error> = None;
    for d in days {
        match seed::materialize_sessions(
            sim.client.ctx.database(),
            o.week,
            &d.day,
            d.date,
            &o.zone,
            now,
        ) {
            Ok(r) => {
                total.created += r.created;
                total.existing += r.existing;
                total.ids.extend(r.ids);
            }
            Err(e) => {
                failure = Some(e);
                break;
            }
        }
    }
    let sessions = sim.seeded(
        "seed:sessions",
        match failure {
            Some(e) => Err(e),
            None => Ok(total),
        },
    )?;
    sim.artifacts
        .insert("session_ids".into(), json!(sessions.ids));
    let Some(saturday) = days.iter().find(|d| d.day.day_of_week.is_saturday()) else {
        return Err(sim.fail(
            "seed:assessment",
            "NOT_FOUND",
            "curriculum.saturday_missing",
            json!({}),
        ));
    };
    let operator = sim.client.ctx.operator().id.clone();
    let assessment = sim.seeded(
        "seed:assessment",
        seed::materialize_assessment(
            sim.client.ctx.database(),
            o.week,
            &saturday.day,
            &operator,
            now,
        ),
    )?;
    sim.artifacts.insert(
        "assessment_id".into(),
        json!(seed::assessment_id_for(o.week)),
    );
    sim.artifacts
        .insert("seeded_assessment_ids".into(), json!(assessment.ids));
    Ok(())
}

fn session_payload(id: &str) -> Value {
    json!({ "session_id": id })
}

fn run_block(
    sim: &mut Sim,
    block: &Value,
    pause_here: bool,
    re_run_hint: &str,
) -> Result<(), Halt> {
    let Some(session_id) = block["session_id"].as_str().map(str::to_string) else {
        return Err(sim.fail(
            "session.prepare",
            "NOT_FOUND",
            "session.block_without_session",
            json!({ "block": block["block"], "seq": block["seq"] }),
        ));
    };
    let minutes = u32::try_from(block["minutes"].as_u64().unwrap_or(0)).unwrap_or(0);
    let meta = MetaOptions::default();
    let prepared = sim.dispatch(
        "session.prepare",
        session_payload(&session_id),
        &meta,
        Expect::Ok,
    );
    match prepared {
        Ok(_) => {}
        Err(halt) => {
            if sim
                .failures
                .last()
                .is_some_and(|f| f.contains("session.illegal_transition"))
            {
                sim.failures.push(re_run_hint.to_string());
            }
            return Err(halt);
        }
    }
    if sim
        .ok("session.start", session_payload(&session_id), &meta)?
        .is_none()
    {
        return Ok(());
    }
    if pause_here {
        let half = minutes / 2;
        sim.clock.advance_minutes(half);
        sim.ok("session.pause", session_payload(&session_id), &meta)?;
        sim.clock.advance_minutes(PAUSE_MINUTES);
        sim.ok("session.resume", session_payload(&session_id), &meta)?;
        sim.clock.advance_minutes(minutes - half);
    } else {
        sim.clock.advance_minutes(minutes);
    }
    if let Some(done) = sim.ok("session.complete", session_payload(&session_id), &meta)? {
        let planned_ms = i64::from(minutes) * 60_000;
        let credited = done["result"]["credited_ms"].as_i64().unwrap_or(-1);
        let state = done["result"]["state"].clone();
        sim.check(
            &format!("{}:{} credited equals planned", tail(&session_id), block["block"].as_str().unwrap_or("?")),
            state == json!("COMPLETED") && credited == planned_ms,
            json!({ "state": state, "credited_ms": credited, "planned_ms": planned_ms, "paused": pause_here }),
        );
    }
    sim.clock.advance_minutes(1);
    Ok(())
}

fn assessment_flow(sim: &mut Sim, o: &SimulateOptions) -> Result<(), Halt> {
    let assessment_id = seed::assessment_id_for(o.week);
    let Some(begun) = sim.ok(
        "assessment.begin",
        json!({ "assessment_id": assessment_id }),
        &MetaOptions::default(),
    )?
    else {
        return Ok(());
    };
    let Some(attempt_id) = begun["result"]["attempt_id"].as_str().map(str::to_string) else {
        return Err(sim.fail(
            "assessment.begin",
            "INTERNAL",
            "attempt_id_missing",
            json!({}),
        ));
    };
    sim.artifacts.insert("attempt_id".into(), json!(attempt_id));
    sim.clock.advance_minutes(60);
    sim.ok(
        "assessment.submit",
        json!({
            "attempt_id": attempt_id,
            "evidence_refs": [],
            "notes": "simulated submission (synthetic; no evidence items)",
        }),
        &MetaOptions::default(),
    )?;
    sim.ok(
        "assessment.review_evidence",
        json!({ "attempt_id": attempt_id }),
        &MetaOptions::default(),
    )?;
    let criteria: Vec<Value> = seed::baseline_criteria()
        .into_iter()
        .map(|code| {
            json!({
                "criterion_code": code,
                "score": GRADE_LEVEL,
                "source": "HUMAN_REVIEWER",
                "evidence_refs": [],
            })
        })
        .collect();
    let Some(graded) = sim.ok(
        "assessment.grade_provisional",
        json!({ "attempt_id": attempt_id, "criteria": criteria }),
        &MetaOptions::default(),
    )?
    else {
        return Ok(());
    };
    let revision = graded["revision"].as_u64();
    let first = MetaOptions {
        expected_revision: revision,
        ..MetaOptions::default()
    };
    let Some(refusal) = sim.refused(
        "assessment.approve_final",
        json!({ "attempt_id": attempt_id }),
        &first,
        "APPROVAL_REQUIRED",
    )?
    else {
        return Ok(());
    };
    let Some(approval_id) = refusal.approval_id.map(|a| a.as_str().to_string()) else {
        return Err(sim.fail(
            "assessment.approve_final",
            "APPROVAL_REQUIRED",
            "approval_id_missing",
            json!({}),
        ));
    };
    sim.artifacts
        .insert("finalize_approval_id".into(), json!(approval_id));
    sim.ok(
        "approvals.decide",
        json!({ "approval_id": approval_id, "decision": "APPROVE" }),
        &MetaOptions::default(),
    )?;
    let retry = MetaOptions {
        approval_id: Some(approval_id),
        expected_revision: revision,
        ..MetaOptions::default()
    };
    if let Some(finalized) = sim.ok(
        "assessment.approve_final",
        json!({ "attempt_id": attempt_id }),
        &retry,
    )? {
        let r = &finalized["result"];
        sim.check(
            "attempt APPROVED_FINAL with a final grade",
            r["state"] == json!("APPROVED_FINAL") && r["grade"]["final"] == json!(true),
            json!({
                "state": r["state"],
                "final": r["grade"]["final"],
                "total": r["grade"]["total"],
                "pass": r["grade"]["pass"],
                "reliability_class": r["grade"]["reliability_class"],
            }),
        );
    }
    Ok(())
}

fn run_day(sim: &mut Sim, o: &SimulateOptions, k: usize, plan_day: &DayPlan) -> Result<(), Halt> {
    let start = match sim.day_start(k, plan_day.date) {
        Ok(s) => s,
        Err(e) => return Err(sim.fail("clock:day_start", &e.category_str(), &e.code, json!({}))),
    };
    sim.clock.jump_to(start);
    let date = seed::format_date(plan_day.date);
    let Some(plan) = sim.ok(
        "curriculum.daily_plan",
        json!({ "date": date }),
        &MetaOptions::default(),
    )?
    else {
        return Ok(());
    };
    let r = &plan["result"];
    let expected_day = tail(plan_day.day.day_code.as_str());
    sim.check(
        &format!("{date} plans {expected_day}"),
        r["unscheduled"] == json!(false) && r["day_id"] == json!(expected_day),
        json!({ "day_id": r["day_id"], "unscheduled": r["unscheduled"], "week_number": r["week_number"] }),
    );
    let blocks = r["blocks"].as_array().cloned().unwrap_or_default();
    if blocks.is_empty() {
        return Err(sim.fail(
            "curriculum.daily_plan",
            "NOT_FOUND",
            "day.without_blocks",
            json!({ "date": date }),
        ));
    }
    let pause_index = if blocks.len() > 2 { 2 } else { 0 };
    let re_run_hint = format!(
        "hint: week {} was already simulated in this data directory (sessions are not PLANNED); \
         use a fresh --data-dir or another --week",
        o.week
    );
    for (i, block) in blocks.iter().enumerate() {
        run_block(sim, block, i == pause_index, &re_run_hint)?;
    }
    let objective = r["objectives"]
        .as_array()
        .and_then(|objs| objs.first())
        .and_then(|first| first["objective_id"].as_str())
        .map(str::to_string);
    let mut note = json!({
        "title": format!("Learning journal {date}"),
        "body_markdown": format!(
            "## Learning journal {date}\n\nSimulated day {expected_day}: {} blocks completed, one pause/resume.\n",
            blocks.len()
        ),
        "classification": "INTERNAL",
        "domain": "LEARNING",
    });
    if let Some(objective) = objective {
        note["objective_id"] = json!(objective);
    }
    if let Some(created) = sim.ok("notes.create", note, &MetaOptions::default())? {
        if let Some(id) = created["result"]["note_id"].as_str() {
            let entry = sim.artifacts.entry("note_ids").or_insert_with(|| json!([]));
            if let Some(list) = entry.as_array_mut() {
                list.push(json!(id));
            }
        }
    }
    if r["is_assessment_day"] == json!(true) {
        assessment_flow(sim, o)?;
    }
    Ok(())
}

fn backups(sim: &mut Sim, o: &SimulateOptions) -> Result<(), Halt> {
    let Some(created) = sim.ok(
        "backup.create",
        json!({ "label": format!("simulate-week-{}", o.week) }),
        &MetaOptions::default(),
    )?
    else {
        return Ok(());
    };
    let Some(backup_id) = created["result"]["backup_id"].as_str().map(str::to_string) else {
        return Err(sim.fail("backup.create", "INTERNAL", "backup_id_missing", json!({})));
    };
    sim.artifacts.insert("backup_id".into(), json!(backup_id));
    sim.ok(
        "backup.verify",
        json!({ "backup_id": backup_id }),
        &MetaOptions::default(),
    )?;
    if let Some(tested) = sim.ok(
        "backup.restore_test",
        json!({ "backup_id": backup_id }),
        &MetaOptions::default(),
    )? {
        let r = &tested["result"];
        sim.check(
            "backup restore-tested and retained",
            r["state"] == json!("RETAINED") && r["integrity"] == json!("OK"),
            json!({ "state": r["state"], "integrity": r["integrity"], "restore_tested": !r["restore_tested_at"].is_null() }),
        );
    }
    Ok(())
}

/// The Saturday grade reached the competency model: the projection is
/// non-empty and its `evidence_refs` include every observation that
/// `assessment.approve_final` derived from the finalized attempt. The operator
/// is the learner, so the grade is `SELF_ADJUDICATED` (ADR-0012): it must not
/// validate, and the projection says so (`provisional`).
fn projection_checks(sim: &mut Sim, projected: &Value) {
    let Some(attempt) = sim
        .artifacts
        .get("attempt_id")
        .and_then(Value::as_str)
        .and_then(|a| a.parse::<AttemptId>().ok())
    else {
        return;
    };
    let derived: Vec<String> =
        competency_repo::observations_for_attempt(sim.client.ctx.database().conn(), &attempt)
            .map(|rows| {
                rows.into_iter()
                    .map(|o| o.id.as_str().to_string())
                    .collect()
            })
            .unwrap_or_default();
    let projections = projected["result"]["projections"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let refs: Vec<&str> = projections
        .iter()
        .flat_map(|p| p["evidence_refs"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .collect();
    let covered = !derived.is_empty() && derived.iter().all(|id| refs.contains(&id.as_str()));
    sim.check(
        "competency.projection includes the observations of the finalized attempt",
        !projections.is_empty() && covered,
        json!({
            "projections": projections.len(),
            "attempt_observations": derived.len(),
            "evidence_refs": refs.len(),
            "attempt_observations_in_evidence_refs": covered,
        }),
    );
    let validated = projections.iter().any(|p| p["state"] == json!("VALIDATED"));
    let provisional = projections.iter().any(|p| p["provisional"] == json!(true));
    sim.check(
        "self-adjudicated grade does not validate (provisional, no VALIDATED)",
        !validated && provisional,
        json!({ "validated": validated, "provisional": provisional }),
    );
}

fn script(sim: &mut Sim, o: &SimulateOptions) -> Result<(), Halt> {
    ensure_curriculum(sim, o)?;
    let days = resolve_days(sim, o)?;
    seed_week(sim, o, &days)?;
    for (k, day) in days.iter().enumerate() {
        run_day(sim, o, k, day)?;
    }
    sim.ok("retention.due", json!({}), &MetaOptions::default())?;
    if let Some(projected) = sim.ok("competency.projection", json!({}), &MetaOptions::default())? {
        projection_checks(sim, &projected);
    }
    backups(sim, o)?;
    Ok(())
}

fn audit_growth(sim: &mut Sim, rows_at_open: u64) {
    let recent = sim.dispatch(
        "audit.recent",
        json!({ "limit": 500 }),
        &MetaOptions::default(),
        Expect::Ok,
    );
    let listed_ok = match &recent {
        Ok(Outcome::Ok(v)) => v["result"]["chain_ok"] == json!(true),
        _ => false,
    };
    let entries = match &recent {
        Ok(Outcome::Ok(v)) => len_of(&v["result"]["entries"]),
        _ => 0,
    };
    let chain = AuditChain::verify_chain(sim.client.ctx.database().conn()).map_err(G9Error::from);
    let (rows_after, chain_ok) = match &chain {
        Ok(report) => (report.rows, report.ok),
        Err(_) => (0, false),
    };
    sim.check(
        "audit chain verifies and grew",
        listed_ok && chain_ok && rows_after > rows_at_open,
        json!({
            "rows_at_open": rows_at_open,
            "rows_after": rows_after,
            "chain_ok": chain_ok,
            "entries_listed_this_process": entries,
        }),
    );
}

/// Scenario verdict from the failure and NOT RUN lists: `PASS` needs both empty.
pub fn verdict(failures: &[String], not_run: &[String]) -> &'static str {
    match (failures.is_empty(), not_run.is_empty()) {
        (true, true) => "PASS",
        (true, false) => "INCOMPLETE",
        (false, _) => "FAIL",
    }
}

/// Run the scenario. `Ok(report)` on `PASS`; `Err(CliError::Scenario(report))`
/// on `INCOMPLETE` or `FAIL` (exit 2; the report says which).
pub fn run(o: SimulateOptions) -> Result<Value, CliError> {
    let clock = o
        .boot
        .clock
        .clone()
        .ok_or_else(|| CliError::Usage("simulate-week needs a stepping clock".to_string()))?;
    let origin = clock.now();
    let booted = bootstrap::open(&o.boot)?;
    let recovery = booted.ctx.recovery_report().clone();
    let ids = if o.fixed_clock.is_some() {
        IdSource::sequential(format!("sim-w{:02}", o.week))
    } else {
        IdSource::Uuid
    };
    let mut sim = Sim {
        client: Client::with_ids(booted.ctx, ids),
        clock,
        zone: o.zone.clone(),
        fixed: o.fixed_clock.is_some(),
        origin,
        steps: Vec::new(),
        checks: Vec::new(),
        failures: Vec::new(),
        not_run: Vec::new(),
        artifacts: Map::new(),
    };
    sim.row(
        "boot",
        "OK",
        "RECOVERED",
        json!({
            "audit_chain_ok": recovery.audit_chain_ok,
            "audit_rows": recovery.audit_rows,
            "interrupted_sessions": recovery.interrupted_sessions.len(),
            "closed_intervals": recovery.closed_intervals.len(),
            "expired_approvals": recovery.expired_approvals.len(),
            "pending_migrations": recovery.pending_migrations.len(),
            "skills_packages": booted.skills.packages,
        }),
    );
    if let Some(reason) = recovery.write_block_reason() {
        sim.fail("boot", "READ_ONLY", reason, json!({}));
    } else {
        let _ = script(&mut sim, &o);
    }
    audit_growth(&mut sim, recovery.audit_rows);
    let result = verdict(&sim.failures, &sim.not_run);
    let passed = result == "PASS";
    let report = json!({
        "scenario": "simulate-week",
        "result": result,
        "passed": passed,
        "complete": sim.not_run.is_empty(),
        "start_date": seed::format_date(o.start_date),
        "week": o.week,
        "program_start": seed::format_date(o.program_start),
        "zone": o.zone.as_str(),
        "operator_id": sim.client.ctx.operator().id,
        "clock": {
            "mode": if sim.fixed { "fixed" } else { "derived" },
            "origin_ms": origin.as_millis(),
            "end_ms": sim.clock.now().as_millis(),
            "ids": if sim.fixed { "sequential" } else { "uuid-v7" },
        },
        "data_dir": booted.layout.root,
        "steps": sim.steps,
        "checks": sim.checks,
        "not_run": sim.not_run,
        "failures": sim.failures,
        "artifacts": sim.artifacts,
        "warnings": booted.warnings,
    });
    drop(sim);
    if passed {
        Ok(report)
    } else {
        Err(CliError::Scenario(report))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boot() -> BootOptions {
        BootOptions {
            data_dir: PathBuf::from("/nonexistent"),
            skills_dir: None,
            zone: None,
            program_start: None,
            operator_id: "op".into(),
            clock: None,
        }
    }

    #[test]
    fn options_require_a_monday_and_derive_program_start() {
        let zone = IanaZone::parse("Europe/Berlin").unwrap();
        let monday = seed::parse_date("2026-11-16").unwrap();
        let o = SimulateOptions::new(
            boot(),
            monday,
            3,
            None,
            PathBuf::from("m.json"),
            zone.clone(),
        )
        .unwrap();
        assert_eq!(seed::format_date(o.program_start), "2026-11-02");
        assert_eq!(o.boot.program_start, Some(o.program_start));
        assert_eq!(
            o.boot.clock.as_ref().unwrap().now(),
            UtcMillis::from_millis(1_794_812_400_000),
            "08:00 CET on the start date"
        );
        let fixed = SimulateOptions::new(
            boot(),
            monday,
            1,
            Some(5),
            PathBuf::from("m.json"),
            zone.clone(),
        )
        .unwrap();
        assert_eq!(
            fixed.boot.clock.as_ref().unwrap().now(),
            UtcMillis::from_millis(5)
        );
        let tuesday = seed::parse_date("2026-11-17").unwrap();
        assert!(matches!(
            SimulateOptions::new(
                boot(),
                tuesday,
                1,
                None,
                PathBuf::from("m.json"),
                zone.clone()
            ),
            Err(CliError::Usage(_))
        ));
        assert!(matches!(
            SimulateOptions::new(boot(), monday, 13, None, PathBuf::from("m.json"), zone),
            Err(CliError::Usage(_))
        ));
    }

    #[test]
    fn verdict_is_pass_only_when_everything_ran_and_passed() {
        let none: Vec<String> = Vec::new();
        let some = vec!["step 7: x.y -> x.not_implemented".to_string()];
        assert_eq!(verdict(&none, &none), "PASS");
        assert_eq!(verdict(&none, &some), "INCOMPLETE");
        assert_eq!(verdict(&some, &none), "FAIL");
        assert_eq!(verdict(&some, &some), "FAIL");
    }

    #[test]
    fn explicit_program_start_must_agree_with_the_derived_one() {
        use crate::cli::Cli;
        use clap::Parser;
        let base = [
            "g9",
            "--data-dir",
            "/nonexistent",
            "--no-skills",
            "simulate-week",
            "--start-date",
            "2026-11-16",
            "--week",
            "3",
        ];
        let agreeing = Cli::try_parse_from(
            base.iter()
                .copied()
                .chain(["--program-start", "2026-11-02"]),
        )
        .unwrap();
        let crate::cli::Command::SimulateWeek(args) = &agreeing.command else {
            panic!("not simulate-week");
        };
        let o = SimulateOptions::from_cli(&agreeing, args).unwrap();
        assert_eq!(seed::format_date(o.program_start), "2026-11-02");
        let contradicting = Cli::try_parse_from(
            base.iter()
                .copied()
                .chain(["--program-start", "2026-11-09"]),
        )
        .unwrap();
        let crate::cli::Command::SimulateWeek(args) = &contradicting.command else {
            panic!("not simulate-week");
        };
        assert!(matches!(
            SimulateOptions::from_cli(&contradicting, args),
            Err(CliError::Usage(msg)) if msg.contains("contradicts")
        ));
    }

    #[test]
    fn summaries_carry_metadata_only() {
        let note = Ok(
            json!({ "result": { "note_id": "n", "body_markdown": "secret text", "classification": "INTERNAL", "domain": "LEARNING", "version_count": 1 }, "replayed": false }),
        );
        let s = summarize("notes.create", &note);
        assert!(s.get("body_markdown").is_none());
        assert_eq!(s["version_count"], 1);
        let err = Err(G9Error::precondition_failed("x.not_implemented", "later"));
        assert_eq!(summarize("x.y", &err)["code"], "x.not_implemented");
        let plan = Ok(
            json!({ "result": { "day_id": "D01", "unscheduled": false, "is_assessment_day": false, "week_number": 1, "blocks": [ { "session_id": "a" }, { "session_id": null } ] }, "replayed": false }),
        );
        let s = summarize("curriculum.daily_plan", &plan);
        assert_eq!(s["blocks"], 2);
        assert_eq!(s["sessions_linked"], 1);
    }
}
