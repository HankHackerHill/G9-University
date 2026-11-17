//! PERF-2: latency at the IPC boundary on the PERF-1 fixture.
//!
//! Every sample is the wall time of one `g9_app::dispatch` call, measured with
//! `std::time::Instant` around the call (metadata and payload are built
//! before the clock starts): operation lookup, metadata and payload checks,
//! policy (and the persisted policy-decision row every command writes, reads
//! included), the handler, the response check, the commit
//! (`synchronous = FULL`) and the operations log line, exactly as the desktop
//! bridge runs them. Per operation: 10 warm-up calls (reported separately as
//! cold), then 200 samples; p50, p95 (nearest rank) and max in milliseconds,
//! as JSON on stdout.
//!
//! Reads: the budget list of `docs/ui/performance-budgets.md` with the UI's
//! payloads (`g9_cli::perf::workload::budget_reads`), measured first, on the
//! fixture as built. Writes: the most used writes of the fixture, ranked by
//! the fixture's own audit rows (one per mutating command): every operation
//! whose count reaches the fifth-highest count (ties included), plus
//! `session.pause` and `session.resume`; the ranking is printed. Then
//! `audit.recent` once more: it lists only the rows this process appended,
//! so on the untouched fixture it returns none and the second series (after
//! the writes) is the one that reads 50 entries back.
//!
//! The backend runs on a stepping clock that starts on the day after the
//! fixture's timeline (the harness never reads the backend's clock for a
//! measurement); everything else is the production composition
//! (`bootstrap::open`). The harness writes (notes, sessions, reviews) into the
//! directory it is given: point it at a copy of a fixture, never at operator
//! data (`--scratch` confirms that).
//!
//! It is a measurement tool, not a test and not part of the shipped `g9`
//! binary: numbers measured anywhere but the G9 host are not a verdict on the
//! budgets of `docs/ui/performance-budgets.md`.
//!
//! Usage (release build):
//! `cargo run --release -p g9-cli --example perf_ipc -- --data-dir <fixture copy> --scratch`

use g9_app::AppContext;
use g9_cli::bootstrap::{self, BootOptions};
use g9_cli::clock::SimClock;
use g9_cli::meta::{build_meta, IdSource, MetaOptions};
use g9_cli::perf::fixture::{Targets, PROGRAM_START, ZONE};
use g9_cli::perf::workload::{self, Call};
use g9_domain::{G9Error, IanaZone};
use g9_storage::diagnostics::{self, Cell};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Warm-up calls per operation (reported apart from the samples).
const WARMUP: usize = 10;
/// Samples per operation.
const SAMPLES: usize = 200;
/// Provisional p95 target of an indexed read (`docs/ui/performance-budgets.md`).
const READ_BUDGET_MS: u64 = 100;
/// How many of the most used writes are measured (ties at the last place included).
const TOP_WRITES: usize = 5;
/// Writes this harness can drive (each has a payload generator below).
const MEASURABLE_WRITES: &[&str] = &[
    "retention.record_review",
    "notes.revise",
    "notes.create",
    "session.prepare",
    "session.start",
    "session.pause",
    "session.resume",
    "session.complete",
];

fn argument(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn fail(message: &str) -> ! {
    eprintln!("perf_ipc: {message}");
    std::process::exit(2)
}

struct Harness {
    ctx: AppContext,
    ids: IdSource,
    clock: SimClock,
}

impl Harness {
    /// One timed dispatch; an error ends the run (a failed call is not a sample).
    fn timed(&mut self, op: &str, payload: Value) -> (Duration, Value) {
        let meta = build_meta(op, &MetaOptions::default(), &mut self.ids, self.clock.now());
        let start = Instant::now();
        let out = g9_app::dispatch(&mut self.ctx, op, meta, payload);
        let elapsed = start.elapsed();
        match out {
            Ok(v) => (elapsed, v),
            Err(e) => fail(&format!("{op} failed: {:?} {}", e.category, e.code)),
        }
    }

    /// One untimed dispatch (set-up of a measured write).
    fn call(&mut self, op: &str, payload: Value) -> Value {
        self.timed(op, payload).1
    }

    fn rows(&self, table: &str) -> u64 {
        diagnostics::row_count(self.ctx.database().conn(), table).unwrap_or(0)
    }

    /// Audit rows per operation (one per mutating command), most used first.
    fn audit_rows_per_operation(&self) -> Vec<(String, u64)> {
        let Ok(dump) = diagnostics::dump_table(self.ctx.database().conn(), "audit_metadata") else {
            fail("audit_metadata could not be read");
        };
        let Some(col) = dump.columns.iter().position(|c| c == "operation") else {
            fail("audit_metadata has no operation column");
        };
        let mut counts: BTreeMap<String, u64> = BTreeMap::new();
        for row in &dump.rows {
            if let Some(Cell::Text(op)) = row.get(col) {
                *counts.entry(op.clone()).or_insert(0) += 1;
            }
        }
        let mut ranked: Vec<(String, u64)> = counts.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        ranked
    }
}

fn ms(d: Duration) -> f64 {
    (d.as_secs_f64() * 1_000_000.0).round() / 1_000.0
}

/// Nearest-rank percentile of sorted samples.
fn percentile(sorted: &[Duration], p: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// Collects warm-up and samples of one operation in call order.
#[derive(Default)]
struct Series {
    warmup: Vec<Duration>,
    samples: Vec<Duration>,
}

impl Series {
    fn push(&mut self, d: Duration) {
        if self.warmup.len() < WARMUP {
            self.warmup.push(d);
        } else {
            self.samples.push(d);
        }
    }

    fn summary(&self, op: &str, variant: &str, kind: &str, extra: Value) -> Value {
        let mut sorted = self.samples.clone();
        sorted.sort();
        let mut v = json!({
            "op": op,
            "variant": variant,
            "kind": kind,
            "warmup_ms": self.warmup.iter().map(|d| ms(*d)).collect::<Vec<_>>(),
            "samples": self.samples.len(),
            "p50_ms": ms(percentile(&sorted, 50.0)),
            "p95_ms": ms(percentile(&sorted, 95.0)),
            "max_ms": ms(sorted.last().copied().unwrap_or(Duration::ZERO)),
        });
        if let (Some(obj), Value::Object(more)) = (v.as_object_mut(), extra) {
            obj.extend(more);
        }
        v
    }
}

/// Items in a read's result (sessions, events, due and overdue items,
/// approvals, entries).
fn result_size(op: &str, out: &Value) -> usize {
    let keys: &[&str] = match op {
        "session.list" => &["sessions"],
        "calendar.list" => &["events"],
        "retention.due" => &["due", "overdue"],
        "approvals.list" => &["approvals"],
        "audit.recent" => &["entries"],
        _ => &[],
    };
    keys.iter()
        .map(|k| out["result"][*k].as_array().map_or(0, Vec::len))
        .sum()
}

fn measure_read(h: &mut Harness, call: &Call, variant: &str) -> Value {
    let mut series = Series::default();
    let mut size = 0;
    for _ in 0..WARMUP + SAMPLES {
        let (d, out) = h.timed(call.op, call.payload.clone());
        series.push(d);
        size = result_size(call.op, &out);
        h.clock.advance_minutes(1);
    }
    series.summary(
        call.op,
        variant,
        "read",
        json!({ "result_items": size, "provisional_p95_budget_ms": READ_BUDGET_MS }),
    )
}

fn note_body(n: usize, revision: bool) -> String {
    let mut body = format!(
        "## Harness note {n}\n\nWhat I did, what went wrong, what comes next: a journal entry of \
         about the size the fixture writes, with a list:\n\n- step one\n- step two\n- step three\n"
    );
    if revision {
        body.push_str("\n### Revision\n\nAdded the missing evidence reference.\n");
    }
    body
}

fn id_of(out: &Value, key: &str) -> String {
    out["result"][key].as_str().unwrap_or_default().to_string()
}

/// CPU model and memory of a Linux host, when readable (nothing else is read).
fn host() -> Value {
    let read = |path: &str| std::fs::read_to_string(path).unwrap_or_default();
    let field = |text: &str, key: &str| {
        text.lines()
            .find(|l| l.starts_with(key))
            .and_then(|l| l.split(':').nth(1))
            .map(|v| v.trim().to_string())
    };
    let cpuinfo = read("/proc/cpuinfo");
    let meminfo = read("/proc/meminfo");
    json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "available_parallelism": std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0),
        "cpu_model": field(&cpuinfo, "model name"),
        "mem_total": field(&meminfo, "MemTotal"),
        "not_recorded": "CPU governor, storage device and filesystem, GPU state, power source and other load (docs/ui/performance-budgets.md 'Host state'): not available or not meaningful in a sandbox",
    })
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(data_dir) = argument(&args, "--data-dir") else {
        fail("--data-dir <fixture copy> is required");
    };
    if !args.iter().any(|a| a == "--scratch") {
        fail("the harness writes notes, sessions and reviews into the data directory; pass --scratch to confirm it is a disposable copy of a fixture");
    }
    let zone = IanaZone::parse(ZONE).unwrap_or_else(|_| fail("zone"));
    let start = workload::after_fixture().unwrap_or_else(|e: G9Error| fail(&e.code));
    let clock = SimClock::new(start);
    let mut boot = BootOptions::new(PathBuf::from(&data_dir));
    boot.skills_dir = None;
    boot.zone = Some(zone);
    boot.program_start = g9_cli::seed::parse_date(PROGRAM_START).ok();
    boot.clock = Some(clock.clone());
    boot.native_confirmation = false;
    boot.event_log = true;

    let opened = Instant::now();
    let booted = bootstrap::open(&boot).unwrap_or_else(|e| fail(&format!("open: {e}")));
    let open_ms = ms(opened.elapsed());
    if let Some(reason) = booted.ctx.recovery_report().write_block_reason() {
        fail(&format!("recovery refused writes: {reason}"));
    }
    let mut h = Harness {
        ctx: booted.ctx,
        ids: IdSource::sequential("perf-ipc"),
        clock,
    };

    // The fixture as measured (the budget document: smaller fixtures
    // invalidate the result).
    let budget = Targets::BUDGET;
    let mut fixture = BTreeMap::new();
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
        fixture.insert(table.to_string(), h.rows(table));
    }
    let at_budget_size = fixture["study_sessions"] >= u64::from(budget.sessions)
        && fixture["time_intervals"] >= u64::from(budget.intervals)
        && fixture["notes"] >= u64::from(budget.notes)
        && fixture["attempts"] >= u64::from(budget.attempts)
        && fixture["competency_observations"] >= u64::from(budget.observations)
        && fixture["audit_metadata"] >= u64::from(budget.audit_rows)
        && fixture["training_days"] >= 72
        && fixture["daily_objectives"] >= 216;

    // The writes to measure: ranked by the fixture's audit rows.
    let ranked = h.audit_rows_per_operation();
    let cut = ranked.get(TOP_WRITES - 1).map_or(0, |(_, n)| *n);
    let top: Vec<(String, u64)> = ranked.iter().filter(|(_, n)| *n >= cut).cloned().collect();
    for (op, _) in &top {
        if !MEASURABLE_WRITES.contains(&op.as_str()) {
            fail(&format!(
                "{op} is among the {TOP_WRITES} most used writes of this fixture and the harness has no payload for it"
            ));
        }
    }

    let mut results = Vec::new();
    let reads = workload::budget_reads(h.clock.now()).unwrap_or_else(|e| fail(&e.code));
    for call in &reads {
        results.push(measure_read(&mut h, call, call.variant));
    }

    let n = WARMUP + SAMPLES;
    let mut series: BTreeMap<&str, Series> = MEASURABLE_WRITES
        .iter()
        .map(|op| (*op, Series::default()))
        .collect();
    let record = |series: &mut BTreeMap<&str, Series>, op: &'static str, d: Duration| {
        if let Some(s) = series.get_mut(op) {
            s.push(d);
        }
    };

    // notes.create, then notes.revise on the notes just written.
    let mut notes = Vec::with_capacity(n);
    for i in 0..n {
        let (d, out) = h.timed(
            "notes.create",
            json!({ "title": format!("Harness note {i}"), "body_markdown": note_body(i, false), "classification": "INTERNAL", "domain": "LEARNING" }),
        );
        record(&mut series, "notes.create", d);
        notes.push(id_of(&out, "note_id"));
        h.clock.advance_minutes(1);
    }
    for (i, id) in notes.iter().enumerate() {
        let (d, _) = h.timed(
            "notes.revise",
            json!({ "note_id": id, "body_markdown": note_body(i, true) }),
        );
        record(&mut series, "notes.revise", d);
        h.clock.advance_minutes(1);
    }

    // Sessions: created untimed (`session.create` is not among the most
    // used writes), then prepare, start, pause, resume, complete timed, one
    // session at a time.
    for _ in 0..n {
        let created = h.call(
            "session.create",
            json!({ "planned_minutes": 45, "planned_start": h.clock.now().as_millis(), "mode": "STUDY" }),
        );
        let p = json!({ "session_id": id_of(&created, "session_id") });
        for (op, minutes) in [
            ("session.prepare", 1),
            ("session.start", 20),
            ("session.pause", 5),
            ("session.resume", 20),
            ("session.complete", 1),
        ] {
            let (d, _) = h.timed(op, p.clone());
            record(&mut series, op, d);
            h.clock.advance_minutes(minutes);
        }
    }

    // retention.record_review on one item made for the harness.
    let item = h.call(
        "retention.create_item",
        json!({ "prompt": "Harness recall prompt", "kind": "DECLARATIVE" }),
    );
    let item = id_of(&item, "review_item_id");
    for i in 0..n {
        let (d, _) = h.timed(
            "retention.record_review",
            json!({ "review_item_id": item, "outcome": if i % 5 == 4 { "FAILURE" } else { "SUCCESS" }, "response_ms": 5_000 }),
        );
        record(&mut series, "retention.record_review", d);
        h.clock.advance_minutes(1);
    }

    for op in MEASURABLE_WRITES {
        let rank = ranked.iter().position(|(o, _)| o == op).map(|r| r + 1);
        let fixture_count = ranked.iter().find(|(o, _)| o == op).map(|(_, c)| *c);
        let kind = if top.iter().any(|(o, _)| o == op) {
            "write (most used)"
        } else {
            "write (extra)"
        };
        if let Some(s) = series.get(op) {
            results.push(s.summary(
                op,
                "-",
                kind,
                json!({ "fixture_audit_rows": fixture_count, "fixture_rank": rank }),
            ));
        }
    }

    // audit.recent again, now that this process appended rows.
    if let Some(call) = reads.iter().find(|c| c.op == "audit.recent") {
        results.push(measure_read(&mut h, call, "limit 50, after the writes"));
    }

    let report = json!({
        "harness": "perf_ipc",
        "label": "SANDBOX NUMBERS: measured where the harness ran, not on the G9 host; not a verdict on docs/ui/performance-budgets.md",
        "build": if cfg!(debug_assertions) { "debug" } else { "release" },
        "host": host(),
        "fixture": {
            "rows_at_start": fixture,
            "at_budget_size": at_budget_size,
        },
        "write_ranking": {
            "source": "audit rows per operation in the fixture (one per mutating command)",
            "caveat": "the ranking follows the fixture's command mix, not observed use: docs/ui/performance-budgets.md fixes the audit-row total (50,000), not which commands write it, and the fixture fills most of it with retention.record_review",
            "top": top.iter().map(|(op, c)| json!({ "op": op, "audit_rows": c })).collect::<Vec<_>>(),
            "first_ten": ranked.iter().take(10).map(|(op, c)| json!({ "op": op, "audit_rows": c })).collect::<Vec<_>>(),
        },
        "method": {
            "clock": "std::time::Instant around g9_app::dispatch (metadata and payload built before the clock starts)",
            "warmup_per_op": WARMUP,
            "samples_per_op": SAMPLES,
            "percentile": "nearest rank",
            "backend_clock": "stepping clock from the day after the fixture timeline, one minute per read call",
            "composition": "bootstrap::open (production database options, synchronous FULL, operations log on), no skills, no model",
            "order": "reads on the fixture as built, then the writes, then audit.recent again",
        },
        "backend_open_ms": open_ms,
        "results": results,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).unwrap_or_default()
    );
}
