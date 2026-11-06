# Desktop UI performance budgets (provisional)

Status: **PROPOSED targets, NOT RUN.** The figures come from the v6.0 handoff §4 ("Nonfunctional targets are provisional"). They become gates only after profiling on the target host (Ubuntu 26.04.1 LTS, Wayland, Tauri 2.12.5) with the measurement method below; until then they are not guarantees and no build claims them.

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

## Known non-budgeted costs in the current scaffold

| Item | Note |
|---|---|
| Lazy routes | Each screen is a separate chunk (`router.tsx` `lazy`), so first navigation to a screen includes a chunk load; measure both first and repeat navigations. |
| Curriculum Navigator | Issues six `curriculum.daily_plan` calls per week view; a `curriculum.week` read model (PROPOSED) would reduce this to one. |
| Calendar grid | Renders 7 × (hours + 1) cells; at the default 07:00–21:00 range that is 105 cells with per-cell event filtering. Fine for one week; a month view would need virtualization. |
| Markdown preview | Re-renders on every keystroke through marked + DOMPurify; debounce if measured above budget. |
