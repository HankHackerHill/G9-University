//! PERF-3 support: are the plans of the query-plan gate's statements the same
//! on the full PERF-1 fixture as on the reduced fixture the gate runs?
//!
//! The gate (`tests/query_plans.rs`) captures the statements of a 1/25-scale
//! workload and plans them on that fixture, which is only valid if SQLite's
//! plans do not depend on row counts (no `sqlite_stat*` statistics). This tool
//! builds the same reduced workload into `--work`, then plans every captured
//! statement on it and on the full fixture given with `--fixture`, and prints
//! the comparison as JSON. Nothing is written to the full fixture (it is opened
//! without migrating and only read). The reduced fixture writes `SEEDED` rows
//! straight to storage, so, like `g9 perf-fixture` and `perf_ipc`, the tool
//! runs only with `--scratch` (`--work` must be a disposable directory; the
//! build also refuses a directory that already holds a database).
//!
//! Usage: `cargo run -p g9-cli --example perf_plans -- --fixture <full fixture dir> --work <new scratch dir> --scratch`

// `CliError` carries a `G9Error` by value (the closed IPC error contract).
#![allow(clippy::result_large_err)]

use g9_cli::perf::fixture::{self, FixtureOptions};
use g9_cli::perf::{plans, workload};
use g9_domain::G9Error;
use g9_storage::diagnostics::{self, StatementCapture};
use g9_storage::Database;
use serde_json::json;
use std::path::PathBuf;

/// Scale of the gate's workload (`tests/query_plans.rs`).
const DIVISOR: u32 = 25;

fn argument(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn fail(message: &str) -> ! {
    eprintln!("perf_plans: {message}");
    std::process::exit(2)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if !args.iter().any(|a| a == "--scratch") {
        fail("the reduced fixture writes SEEDED rows straight to storage, bypassing the command pipeline; pass --scratch to confirm that --work is a disposable directory (it is not inspected)");
    }
    let (Some(full), Some(work)) = (argument(&args, "--fixture"), argument(&args, "--work")) else {
        fail("--fixture <full fixture dir> and --work <new scratch dir> are required");
    };
    let options = FixtureOptions::new(PathBuf::from(&work).join("reduced"), DIVISOR)
        .unwrap_or_else(|e| fail(&e.to_string()));
    let mut capture = None;
    let (report, mut client) = fixture::build(&options, &mut |db| {
        capture = Some(StatementCapture::install(db.conn_mut()).map_err(G9Error::from)?);
        Ok(())
    })
    .unwrap_or_else(|e| fail(&format!("reduced fixture: {e}")));
    workload::run_reads(&mut client).unwrap_or_else(|e| fail(&format!("reads: {e}")));
    workload::run_other_writes(&mut client).unwrap_or_else(|e| fail(&format!("writes: {e}")));
    let Some(capture) = capture else {
        fail("no capture");
    };
    let statements = capture.statements();
    let reduced = client.ctx.database();
    let full_db = Database::open(
        PathBuf::from(&full).join(g9_cli::bootstrap::DATABASE_FILE),
        g9_app::database_options("perf-plans").no_auto_migrate(),
    )
    .unwrap_or_else(|e| fail(&format!("open the full fixture: {e}")));
    let on_reduced = plans::plan_all(reduced, &statements).unwrap_or_else(|e| fail(&e.code));
    let on_full = plans::plan_all(&full_db, &statements).unwrap_or_else(|e| fail(&e.code));
    let differing: Vec<_> = on_reduced
        .iter()
        .zip(&on_full)
        .filter(|(a, b)| a.details != b.details)
        .map(|(a, b)| json!({ "sql": a.sql, "reduced": a.details, "full": b.details }))
        .collect();
    let rows = |db: &Database, table: &str| diagnostics::row_count(db.conn(), table).unwrap_or(0);
    let out = json!({
        "tool": "perf_plans",
        "reduced_fixture": { "divisor": DIVISOR, "result": report.result, "audit_rows": rows(reduced, "audit_metadata") },
        "full_fixture": { "audit_rows": rows(&full_db, "audit_metadata"), "study_sessions": rows(&full_db, "study_sessions") },
        "planner_statistics": {
            "reduced": diagnostics::has_planner_statistics(reduced.conn()).unwrap_or(true),
            "full": diagnostics::has_planner_statistics(full_db.conn()).unwrap_or(true),
        },
        "statements_planned": on_reduced.len(),
        "identical_plans": on_reduced.len() - differing.len(),
        "differing_plans": differing,
    });
    println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
}
