//! Performance tooling (`docs/release/path-to-10.md` §5.21, PERF-1 to PERF-3).
//!
//! | Module | Step | Contract |
//! |---|---|---|
//! | [`fixture`] | PERF-1 | `g9 perf-fixture --scratch`: a 12-month data directory at the sizes of `docs/ui/performance-budgets.md`, built through `g9_app::dispatch` wherever an operation exists; the rest `SEEDED` through the `g9-storage` repositories (as `simulate-week` does) |
//! | [`canonical`] | PERF-1 | the fixture checksum: a canonical projection of every table that is independent of the ids and digests the backend generates |
//! | [`plans`] | PERF-3 | reading `EXPLAIN QUERY PLAN` output: which steps read a whole table of which table (aliases resolved, sub-queries and CTEs set apart), matched against an allow-list with reasons; the static list of SQL literals of `g9-storage` for the coverage report; the trigger bodies of the migrations |
//! | [`workload`] | PERF-2, PERF-3 | the read operations (with the UI's payloads) that the harness measures and the gate plans, and the writes the fixture does not make |
//!
//! The latency harness (PERF-2) is `examples/perf_ipc.rs`: it reads the wall
//! clock, which nothing in this library does. The query-plan gate (PERF-3) is
//! `tests/query_plans.rs`. No SQL is written here: every statement goes
//! through `g9_app::dispatch`, a `g9-storage` repository or
//! `g9_storage::diagnostics`.

pub mod canonical;
pub mod fixture;
pub mod plans;
pub mod workload;
