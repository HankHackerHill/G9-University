# Desktop UI performance budgets (provisional)

Status: **PROPOSED targets.** Measured once in the sandbox (2026-11-16, below), NOT RUN on the G9 host. The figures come from the v6.0 handoff §4 ("Nonfunctional targets are provisional"). They become gates only after profiling on the target host (Ubuntu 26.04.1 LTS, Wayland, Tauri 2.12.5) with the measurement method below; until then they are not guarantees and no build claims them.

## Targets

| Budget | Provisional p95 target | Scope | Source |
|---|---|---|---|
| UI interaction latency | < 150 ms | From input event (keydown/click) to the next committed paint for a representative action: switching screens, opening a session, moving the calendar grid cursor, toggling tabs. | handoff §4 |
| Indexed local query | < 100 ms | `g9-app` read-model commands over SQLite with indexed predicates (`session.list`, `calendar.list` for one week, `retention.due`, `approvals.list`, `audit.recent limit 50`). Measured end-to-end at the IPC boundary (invoke → outcome). | handoff §4 |
| Cold startup | < 3 s | Process start → first interactive frame of Mission Control with `system.status` rendered, cold OS cache, no model loaded. | handoff §4 |
| Model inference | separate | TTFT, prefill throughput, output tokens/s, p50/p95 by output size, VRAM/RAM, cancellation latency; measured per `docs/evaluations/model-benchmark-protocol.md`. Not a UI budget. | handoff §4, §13 |

## Measurement method (to be executed before any PASS claim)

| Element | Requirement |
|---|---|
| Percentile definition | p95 of at least 200 samples per scenario; report p50 and max alongside. Warm-up samples (first 10) excluded and reported separately as cold. |
| Fixture size | State the row counts: 72 training days, 216 objectives, 12 months of sessions (~1,700), 5,000 time intervals, 2,000 notes with 3 revisions each, 500 attempts, 20,000 competency observations, 50,000 audit rows. Smaller fixtures invalidate the result. |
| Warm vs cold | Cold: fresh process, OS page cache dropped where permitted. Warm: steady state after 60 s of use. Both reported. |
| Host state | Record CPU model and governor, RAM, storage device and filesystem, GPU activity (idle vs model loaded), battery/AC, other load. Tauri/webkit2gtk versions. |
| UI timing | `performance.mark` at the input handler and `performance.measure` in a `requestAnimationFrame` after React commit; exported as JSON via a `MAINTENANCE`-mode diagnostics command (PROPOSED). No telemetry leaves the machine. |
| IPC timing | Per-command elapsed from `issued_at` to outcome receipt plus the backend's own duration in `audit_metadata` details (metadata only). |
| Startup timing | OS process start time to the `first-interactive` mark emitted when Mission Control has rendered `system.status`. |
| Reproducibility | Scripted run (`scripts/` proposal), fixed fixture checksum, three repetitions; results committed under `docs/release/` with the stage report; unexecuted = NOT RUN. |

## Tooling and the first sandbox run (2026-11-16)

| Tool | What it does | Command |
|---|---|---|
| Fixture (PERF-1) | builds a data directory at the sizes above through the real pipeline where an operation exists (SEEDED rows through the storage repositories otherwise, as `g9 simulate-week` does); deterministic: the checksum is SHA-256 over every table in rowid order with uuids and 64-hex tokens replaced by their first-appearance ordinal | `g9 perf-fixture --scratch --data-dir <fresh dir> [--divisor N]` |
| Latency harness (PERF-2) | `std::time::Instant` around `g9_app::dispatch`; 10 warm-ups reported apart, 200 samples, nearest-rank percentiles, JSON | `cargo run --release -p g9-cli --example perf_ipc -- --data-dir <copy of a fixture> --scratch` |
| Query-plan gate (PERF-3) | `cargo test -p g9-cli --test query_plans`: every statement the connection runs during a representative workload (297) is planned with `EXPLAIN QUERY PLAN`; a scan, an automatic index or an unconstrained search of a table that is large in the fixture or expected to grow fails unless allow-listed with a reason | `cargo run -p g9-cli --example perf_plans -- --fixture <full fixture> --work <new dir> --scratch` compares the plans on the full fixture |

Checksum of the full fixture: `fce07b9ff41011e9dff44d0a57d546c18a466783f69bdd2981f84f3c375e78fe` (four builds, one checksum). All 297 statements plan identically on the 1/25 fixture, the full fixture and an empty schema (no planner statistics exist), which is why the gate can run at a reduced scale.

Sandbox numbers (`docs/release/perf-sandbox-2026-11-16.json`, release build, 4 vCPU, not the G9 host): NOT a verdict on the budgets above.

| Operation | p50 ms | p95 ms | Against the provisional target |
|---|---|---|---|
| `session.list` (one day) | 7.3 | 9.1 | within |
| `session.list` (all 1,700 sessions, no date) | 109.0 | 125.1 | above 100 ms: finding PERF-F1 |
| `calendar.list` (one week) | 0.7 | 1.0 | within |
| `retention.due` | 34.6 | 39.6 | within |
| `approvals.list` | 10.1 | 14.1 | within |
| `audit.recent` (limit 50, 50,000 audit rows) | 249.1 | 283.6 | above 100 ms: finding PERF-F3 (the whole audit chain is verified on every call) |
| eight writes (record_review, notes revise/create, session prepare/start/pause/resume/complete) | 1.0 to 1.2 | 1.5 to 4.1 | n/a (no write budget) |

The "most used" writes are ranked by the fixture's own command mix, not by observed use. Findings for PERF-6 (allow-listed in the gate as debt, each with its reason): PERF-F1 `session.list` without a date builds every session; PERF-F2 the note list has no `(updated_at, id)` index (needs a migration); PERF-F3 `audit.recent` verifies the whole chain; PERF-F4 competency eligibility sets are not filtered by competency; PERF-F5 an attachment erasure scans `attachments` by `storage_ref`. NOT RUN: cold-cache runs, repetitions and host-state recording on the G9 host (PERF-5).

## Known non-budgeted costs in the current scaffold

| Item | Note |
|---|---|
| Lazy routes | Each screen is a separate chunk (`router.tsx` `lazy`), so first navigation to a screen includes a chunk load; measure both first and repeat navigations. |
| Curriculum Navigator | Issues six `curriculum.daily_plan` calls per week view; a `curriculum.week` read model (PROPOSED) would reduce this to one. |
| Calendar grid | Renders 7 × (hours + 1) cells; at the default 07:00–21:00 range that is 105 cells with per-cell event filtering. Fine for one week; a month view would need virtualization. |
| Markdown preview | Re-renders on every keystroke through marked + DOMPurify; debounce if measured above budget. |
