# Changelog

All notable changes are recorded here. Labels follow `GOVERNANCE.md`.

## [0.1.0] — unreleased (2026-11-14 update)

### Added (2026-11-14, the 12-week curriculum built with the owner; ADR-0022)
- **The curriculum** (`docs/curriculum/`): 12 week files, 72 day sheets (objectives with success criteria, recall, theory, guided and independent labs, defence and verification, evidence and report, what to watch for, resources) and 12 Saturday assessments. They are the single source; `scripts/build_curriculum.py` checks them (the Appendix A wording starts every objective and hand-in; at least 25 words in a block; each of the 64 competencies built by at least two objectives and never before its prerequisites; assessment difficulty inside the competency's band; resource versions and licence notes inside the manifest limits; no hidden character) and generates `data/curriculum/g9-ct9-480.draft.v1.json` (`1.0.0-draft1`, `PROPOSED_SEED`; its canonical checksum is printed by `scripts/check_curriculum_seed.py`), 12 `assessment.define` payloads, the lesson sheets and `docs/curriculum/coverage.md`. It is proposed content: no external resource was opened (138 distinct references: 15 checked by search result only, 104 ASSUMED, 19 numbered publications cited from knowledge), and the owner's items (the source of each blind practical, the resources, the promotion) are listed in `docs/curriculum/design.md`.
- **The window ships the draft and shows its lessons.** The import on Mission Control and Settings brings in the draft (the Appendix A seed stays in the repository as the original it is reconciled against). The Daily Classroom shows the day's lesson sheet, one folded section per block with the first unfinished block open, and what to watch for and the sources in the margin. A sheet is shown only when the plan's objective texts equal the ones it was written for; otherwise the panel says so and shows none of it. Rendering goes through the sanitizer: no link, no image. Schema `schemas/curriculum/lesson-sheets.schema.json`.
- **`g9 reconcile`**: compares two manifest files (both validated), prints the diff (days grouped by the fields that changed) and the checksums; with explicit `--candidate-version`, `--approved-by` and `--approved-at` it writes the `RECONCILED_CANDIDATE` manifest and the payloads of `curriculum.import` and `curriculum.promote`. It reads files, opens no data directory, never overwrites a file and approves nothing. The route to `APPROVED_CANONICAL` is `docs/curriculum/promotion.md`; the window has no promote action (PROPOSED).

### Security (2026-11-14, found while testing the promotion of the built curriculum)
- **A reconciliation block binds the content it reviewed.** `diff_checksum` hashed which fields differ in which days, not what they say, so an objective reworded after the review still matched its block. It now also hashes the candidate's content checksum (the manifest without its provenance fields, `ReconciliationReport::candidate_content_checksum`): relabelling a candidate keeps it, any edit changes it. Fail-first test `a01_reviewed_diff_is_bound_to_the_candidate_content`. No block had been written for a real manifest, so nothing in use is invalidated (ADR-0022).

### Added (owner request 2026-11-14: the window usable on its own, with the full curriculum imported)
- **First run in the window** (`docs/runbooks/first-run.md`, ADR-0018). Mission Control, and Settings under "Program", walk through three steps: import the 12-week G9/CT9 curriculum that ships with the application (12 weeks, 72 days, 216 objectives, 480 hours, PROPOSED_SEED; approval and the native confirmation), choose the Monday week 1 starts and the time a day starts, and load the competency packs. Until then "Program not set up" is shown as setup, never as an error, in the header, the Classroom and the Navigator.
- Schema version 15: `program_plan` (migration 0015, digest pinned). `curriculum.program` reports what is imported, the stored start and how many of the 64 shipped competencies are loaded; `curriculum.set_program_start` stores the Monday (refused when it is not a Monday, `program.start_not_a_monday`), the `HH:MM` start of a day and a revision. A start given to the process (`--program-start`) still wins over the stored one.
- `session.plan_day`: one PLANNED session per block of a program day, linked to the day, objective and block, in the block's mode (the blind practical in EXAM), laid out from the day start in the operator's zone (DST by the calendar crate), each with its calendar event (source SCHEDULER, not editable by the operator, the blind practical immutable). Repeatable: it plans what is missing. "Plan today's blocks" on Mission Control and the Daily Classroom, "Plan this day's blocks" in the Curriculum Navigator, which now browses every week (Earlier week, Later week, This week, Week 1) and names the curriculum version.
- `notes.list` and `notes.get`: the Journal lists every stored note (newest change first, no bodies in the list), also after a restart, and opens one; **Erase this note…** goes through `deletion.request` (reason, approval, native confirmation, files removed: ADR-0017).
- `competency.import_pack`: the eight packs are embedded in `g9-app`; loading creates 64 competencies and 67 prerequisite edges, is idempotent and completes the edges of packs loaded earlier. NICE references (`K-UNPINNED`, ASSUMED) are not stored. Offered on the Competency Graph and in Settings.
- `backup.list`: Settings lists the backups recorded in the database, also after a restart; a recorded path outside the approved roots is not followed.
- Real-backend end-to-end tooling: `apps/cli/examples/bridge_stdio.rs` (the shell's backend over JSON lines), `scripts/e2e/harness.cjs`, `tour.cjs`, `personal.cjs` (22 checks); `?mock=fresh` starts the browser mock as a new install and `measure.cjs --fresh` measures those states.

### Changed (2026-11-14)
- `calendar.update_event` and `calendar.delete_event` refuse an event the operator cannot change (`calendar.event_not_editable`) before policy, so no approval is asked for a deletion that cannot happen.
- `curriculum.daily_plan` reports `curriculum.not_imported` before `curriculum.program_start_not_configured`.
- Before week 1 the screens say "Week 1 starts Monday ..." (Mission Control) and "Outside the program weeks" instead of "Rest day"; a Sunday inside the program is still a rest day.
- A plan from a version that is not canonical yet is a quiet note with the backend's sentence, not an alarm (`QUIET_WARNING_CODES`); the Proposed badges stay.
- Settings: the last backup is "the last verified and restore-tested backup" (the status reports the newest RETAINED one, so "no backup on record" next to a listed backup was misleading). The Calendar's empty week says where events come from (the old text promised curriculum blocks "by themselves", which `FILL_CURRICULUM` never provided).
- `parseIsoDate` refuses a day the month does not have (2026-02-30), as the backend does.
- Operation count 46 to 53 (`g9 ops`, smoke test, CLI tests).

### Security (2026-11-14, the LOW and INFO findings still open from the audits of 2026-11-12 and 2026-11-13)
- **Audit tail truncation is detected.** `AuditChain::verify_chain` also compares the `AUTOINCREMENT` counter with the newest row and checks every backup anchor; deleting the newest rows makes the database read-only at open (`audit_chain_broken`) and makes a snapshot fail verification. Limit stated in the ADR-0004 addendum: the counter and the anchors live in the same file, so a same-user writer who rewrites them too is not detected (an off-file anchor is PROPOSED). Runbook: `docs/runbooks/backup-restore.md`, "If the window opens read-only because the audit chain is broken".
- **The backend derives the interaction mode for every mutating operation** in `dispatch` (EXAM while an EXAM attempt is STARTED); the backup operations are exempt by a backend flag; MAINTENANCE stays declared (ADR-0019). `skills.invoke` and `session.correct_time` no longer derive on their own.
- **A skill package is read once.** Parse and hash use the same bytes; only regular files are opened (`O_NOFOLLOW`, `O_NONBLOCK`); a symlink, FIFO, socket or device is refused instead of skipped (a FIFO used to block the load for good); bounds of 1 MiB a file, 8 MiB, 256 files and depth 6; `Registry::load_pinned` compares each package with its pin before parsing it; a symlinked package directory is refused. New direct dependency of `g9-skills` on Unix: `libc =0.2.191` (platform constants only; the version `Cargo.lock` already held, one line added to the lock file by cargo).
- **Revision 0 is refused at the boundary** (`expected_revision` is 1 to i64::MAX in the command meta schema and in the mock); a constraint violation carries no SQLite text; a refusal whose approval request cannot be stored is still recorded (`approval.request_not_stored`).
- INFO items: the orchestrator's check detail is cut on a character boundary (it panicked on one); a declined native confirmation leaves one metadata-only audit row; a production bundle without the Tauri runtime no longer falls back to the mock (`RESOURCE_UNAVAILABLE ipc.tauri_runtime_missing`; `VITE_G9_TRANSPORT=mock npm run build` makes a review build); `session.create` `planned_start` is at most 2100-01-01T00:00:00Z.
- `scripts/skill_hashes.py` refuses non-regular entries as the loader does; the live packages are checked against `skill-pins.json` by `crates/g9-skills/tests/live_registry.rs` as well as by the script.

### Verified (2026-11-14, security remediation; no CI was started)
Rust 951 passed, 0 failed (916 before; 35 new tests); fmt, clippy (also the shell crate, type-check only), eslint, tsc, vite build, content gates, `ds_audit` clean; vitest 463 in 38 files (460 before); smoke test passes; layout 32/32 clean; `personal.cjs` 22/22 on the real backend. NOT RUN: the Tauri shell, the native dialog, WebKitGTK and a restore drill (they need the host), `tour.cjs`, `cargo deny` (not installed here), a concurrent writer racing the package loader (the single read is tested by construction). Details: `docs/release/stage-report.md`, "Update 2026-11-14 (later)".

### Verified (2026-11-14; no CI was started)
Rust 916 passed, 0 failed; vitest 460 in 38 files; smoke 100 assertions; fmt, clippy (also the shell crate, type-check only), eslint, tsc, vite build, content gates, `ds_audit` clean; layout 32/32 clean twice; `personal.cjs` 22/22 and `tour.cjs` 16 screens without a console, page or CSP problem, on the real backend. NOT RUN: the Tauri window, the native dialog, WebKitGTK, a restore drill on the host.

## [0.1.0] — unreleased (2026-11-13 update)

### Added (owner requests, 2026-11-13)
- Study sessions: `session.create` plans a session (1 to 600 minutes; STUDY, LAB or REVIEW; optional start and objective) that runs like any other (prepare, start, pause, complete). EXAM is refused at the boundary.
- Calendar: `calendar.create_event`, `calendar.update_event`, `calendar.delete_event` for events the operator adds (study block, appointment, review; at most 7 days long). Events from the scheduler or the curriculum cannot be changed (`calendar.event_not_editable`); deleting needs an approval (`operator.delete.approval`).
- Review results: `retention.create_item` writes a review item (the answer is stored as an erasable note, first due after a day); `retention.record_review` records SUCCESS, FAILURE or SKIPPED, appends the attempt and moves the item along the baseline ladder (1, 3, 7, 21, 60 days; a failure resets it to 1 day; a skip moves nothing).
- Authoring: `competency.create` and `assessment.define` (blind practical, capstone or transfer check; difficulty 1 to 5; the baseline rubric). An authored assessment is taken, graded, finalized and feeds its competency like a seeded one. Custom rubric criteria and the quiz/report kinds are PROPOSED (no storage for them yet).
- Evidence: `evidence.attach_file` stores a file the operator picked (bytes sent by the UI, base64, at most 16 MiB; the backend opens no host path) in a content-addressed store (`<data>/attachments/ab/cd/<sha256>`), written durably and never outside the store, identical bytes once. Executables, archives and packet captures are refused by signature; a file whose bytes do not match its declared type is refused (quarantine has no storage yet: PROPOSED). The attachment takes its note's domain and classification, so a file on a security-sensitive note needs an approval. `evidence.from_note` makes a typed note evidence; `evidence.list` lists a note's evidence or the newest items; `assessment.submit` takes them as `evidence_refs`. New dependency: `base64 = "=0.23.1"` in `g9-app` (already in the lock file through the shell's tree; default features off).
- Export: an approved `export.request` now writes `<data>/exports/<export_id>/` (`data.json` or one CSV per table, the attachment bytes hash-checked, `manifest.json` with every file's SHA-256 written last) and returns `READY` with the folder. Every schema table is assigned to the export exactly once (a test fails on an unassigned table); SECRET rows never leave (privacy model) and PRIVATE rows only with `include_private`; erased rows and replay bookkeeping never leave. The manifest lists every file with its SHA-256 but is not signed (signing with the local key is PROPOSED). CSV cells that would start a spreadsheet formula are prefixed with `'`. The export directory is checked after the approval step; a failure writes nothing, rolls the command back and leaves the approval unspent (TESTED).
- Desktop UI for every operation above (screens Session Tracker, Calendar, Retention Center, Competency Graph, Assessments, Learning Journal, Settings/Recovery), with the mock transport mirroring the backend's checks, error codes and approval flow; times in forms are entered in the backend's time zone; a cancelled native confirmation reads "Nothing was approved". vitest: 302 tests in 30 files.
- Backups carry attachment bytes: `backup.create` copies every referenced attachment into `<backups>/attachments` (hash-checked); `g9 copy-backup` takes them to the other drive; `g9 restore` puts them back into the live store (default source: the `attachments` folder beside the snapshot; `--attachments <dir>` to choose). A missing file is reported, never silently dropped.

### Changed (desktop UI, 2026-11-13)
- All 16 screens rebuilt on "Ledger 2" (`docs/ui/design-system.md`, `docs/ui/handoff.md`, `docs/ui/design-direction.md`): records are ruled panels, instruments are raised, one primary action per screen in the page head opens a composer; the operator's words come first (`src/lib/copy.ts`, `docs/ui/ux-copy.md`) and the backend's exact name stays in `title` and `code`; "No data" is said in words, never as a zero; every ordinal chart ramp is contrast-tested in all 22 palettes. Layout measured at the default 1280×800 and the minimum 960×640 window (32 measurements: no overflow, no target under 24 px, one h1, named controls, no duplicate ids). Research notes: `docs/research/`. Skill for future UI work: `.claude/skills/g9-ui-design` (its three evals are NOT RUN).

### Security (triple audit, 2026-11-13; report `docs/audit/security-audit-2026-11-13.md`; each fix has a test that failed first where one could)
- MEDIUM: erasure kept the SHA-256 of what it erased, in tombstones, in the redacted rows and in events, and exports carried it; the digest of a short text is the text (a four-digit SECRET note body was recovered from an export of the SYSTEM area). Erasure now replaces every stored digest of the erased content with an identity digest (migration 0014, schema version 14), no append-only table receives a digest of erasable content, the approved erasure removes the files of the erased attachments before it commits, and exports withhold tombstone digests, deletion reports and reasons, event and provenance digests, approval MACs, and every hash of a whole request and the audit chain's check values (a hash of `{operation, payload}` confirmed a guessed four-digit body: found by re-running the probe, fixed second); the attempt's submission digest no longer covers the notes text (ADR-0017; this resolves the purge item of ADR-0016). Hashes of whole requests remain in the immutable ledgers and in backup snapshots (PROPOSED: keyed hashes).
- MEDIUM: the native approval confirmation named an operation and an opaque id. It now says what the record is (a note's title and number of files, an event's time, an attempt's latest grade, an export's areas), what policy granted, the request's own context and a fingerprint of the exact values, all written by the backend from storage and sanitised; "Approve" is inert for 1.5 s (ADR-0014 addendum 2).
- MEDIUM: `g9 restore` printed a paste-ready command built from unquoted paths, so a folder named `x $(command)` ran when pasted. It prints the exact arguments and a line quoted for a POSIX shell, `null` when a path holds a control character; no `g9` output carries a raw control character any more (also closes the 2026-11-12 `--human` LOW).
- MEDIUM (plausible, NOT RUN on a display): the window could be navigated to another origin by a link in a note. Rendered Markdown now has no link, the shell refuses every navigation but the application's, and the CSP has `form-action 'none'`.
- LOW: `evidence.attach_file` dropped a stricter classification when a `note_id` was given; backup snapshots and `g9 copy-backup` copies were `0644` (now `0600`); titles, file names and labels that are blank or hold control, invisible or bidirectional characters are refused (`input.name_not_printable`); `--scratch` needs `--data-dir`; `g9 ops` lists `calendar.delete_event` as approval-gated.
- `crates/g9-domain/tests/source_text.rs` fails when a source file holds a hidden or direction-changing character (found in two files written during the audit).
- Deviation: nothing in ARCHITECTURE.md was changed; `core:default` is still the capability (PROPOSED reduction in `apps/desktop/README.md`).

### Security (independent review of the new features, 2026-11-13; each finding reproduced by a test before its fix)
- MEDIUM: an approved erasure left the note title and the file name in `evidence_items.summary`, so `evidence.list` and exports still showed them; the erasure now redacts the summary with the item (`g9_storage::deletion`).
- MEDIUM: `evidence.from_note` and `evidence.attach_file` stored their results (title, file name) in `idempotency_records`, so a replay after erasure returned them; both now store ids only and rebuild a replay from current rows (`idempotency.replay_rebuilt`).
- Events are never classified below the record they are about (pipeline floor from the policy facts); an event about SECRET content is stored PRIVATE without its content hash. Before, a finalized PRIVATE attempt's pass/fail and the body hash of a SECRET note rode INTERNAL events into exports without private rows (found while fixing the review's attachment-event finding; the pass/fail case was not in the review).
- Exports: rows tied to an excluded attempt, episode, intervention or document chunk stay out with it (competency observations carried PRIVATE attempt scores); assessment task statements are withheld (an export would expose blind variants without an exposure record), listed under `withheld` in the manifest.
- The export approval names the privacy ceiling and the format (`export:LEARNING:PRIVATE:CSV`), so the native confirmation shows what leaves.
- `already_stored` answers only for attachments in the caller's scope (it disclosed identical bytes held in other domains).
- `g9 copy-backup` copies attachments before placing the snapshot (an attachment error left a snapshot without its manifest and refused the retry) and takes a damaged backup-store copy from the live store; a file already intact at a destination counts as present without a source.
- `evidence.from_note` turns control characters in a title into spaces (an INTERNAL error before).
- An export over several domains could not be approved: its approval resource id joined the domains with `+`, which the approvals schemas refuse, so `approvals.list` failed for as long as the request was pending. The id now joins with `.` (`export:LEARNING.SYSTEM:INTERNAL:JSON`). Found by the desktop UI work; older than this change.

### Security
- Backup verification, restore and open-time recovery now verify the schema itself: every table, index, trigger and view must equal what the embedded migrations produce at the recorded version. A tampered snapshot or live file with a dropped or planted trigger is refused at verify and restore, and blocks every mutation at open (`schema_drift`). Before, a copy of a genuine database with planted triggers passed every check (security audit 2026-11-12, MEDIUM). The SQL text is compared exactly: the first version collapsed whitespace, and since a newline ends a `--` comment, moving one newline could remove a CHECK constraint unnoticed (found while verifying the fix).
- Approvals bind to the stored revision of the resource, and a request whose `expected_revision` differs from it is refused `STALE_REVISION`. Before, an approval could be bound to a future revision and consumed after a regrade the operator never saw (MEDIUM).
- The restore plan no longer prints a mismatching snapshot hash next to the confirmation command; table names read from a foreign file are quoted.
- `src-tauri/build.rs` refuses a release build in Tauri development mode (which would load its UI from `http://localhost:5173`); the README builds with `npm run tauri:build` (LOW).
- Report: `docs/audit/security-audit-2026-11-12.md`.
- Approvals are granted only through a native confirmation the desktop shell raises from Rust (ADR-0014, SEC-03 now PARTIAL): `g9_app::dispatch_confirming` shows what the stored request grants and dispatches only on the operator's Approve; a grant sent from the webview alone is refused (`approval.native_confirmation_required`), a cancelled dialog answers `approval.not_confirmed`. The shell uses a GTK dialog already in the dependency tree (no new crate); other platforms fail closed; the CLI, a terminal surface, opts out. The Approvals screen describes the control and handles a cancelled confirmation.
- Backups: after the snapshot is renamed into place the backup directory is synced (and the file explicitly before the rename), so a power cut cannot leave a snapshot without its final name; verified with a syscall trace (`fsync(partial)` → `rename` → `fsync(dir)`).
- `g9 copy-backup --backup-id <id> --to <dir>` copies a RETAINED backup to another drive (USB, NAS): written durably, never overwriting, re-hashed against the manifest, with `<id>.manifest.json` beside it so the copy restores with `g9 restore --snapshot … --expect-sha256 …` when this disk is gone.

### Fixed (2026-11-10 review leads, each reproduced by a test that failed before the fix)
- `assessment.submit` stores the operator's notes as an erasable note linked to the attempt as SUBMISSION evidence; they were hashed and discarded (app-5).
- An approval-gated create (a security-sensitive note, an attempt) completes on the approved retry: the retry reuses the approved resource id; the whole binding is still verified (app-1).
- A failed or interrupted migration rehearsal no longer blocks every later migration: its copy is removed on failure and a stale copy is replaced; a symlink or other non-file at the copy path is still refused (storage-core-3).
- A restore test fails a backup only for a content failure; a leftover scratch copy is replaced and an environmental failure (scratch directory, disk, I/O) leaves the backup CHECKED (storage-core-2).
- One process at a time holds the data: an exclusive OS lock on `<database>.lock`, taken before recovery by the desktop bridge and every CLI command, and by `g9 restore` around the file swap; a second process is refused with `app.data_in_use` instead of interrupting a live session (app-4).
- Pause and complete no longer fail after the wall clock steps backward: the interval's wall end is clamped to its start, as recovery does (storage-repos-1).

### Changed
- Owner decisions (ADR-0015): the application database runs with `synchronous = FULL` (`g9_app::database_options`, used by the desktop bridge and the CLI), so a power cut loses no acknowledged command; approvals expire after 10 minutes (was 15); encryption at rest is left to the host's full-disk encryption; no AI model for now (a Claude API runtime is planned and needs an architecture amendment first); `VALIDATED` stays unreachable while every grade is self-adjudicated. ARCHITECTURE.md lines 103 and 149 still state the old values (reported deviation).
- CI runs only when started by hand (`workflow_dispatch`), by the owner's decision: the push and pull_request triggers are removed, so no push starts a run. All five jobs are unchanged. The same checks run locally with `scripts/ci_preflight.sh`. GitHub shows the "Run workflow" button only for workflows on the default branch, which does not contain `.github/workflows/ci.yml` yet.

## [0.1.0] — unreleased (2026-11-12 update)

### Added
- Desktop UI design system "The Ledger" (`docs/ui/design-system.md`): OKLCH tokens, a grammar of standing (posted, pencilled, in doubt, advice, owed, missing), the Recursive variable font (OFL-1.1, bundled, licence shipped) in three voices, cascade layers.
- New shell: grouped table of contents, day rail drawn to scale from backend values, status lamps with the backend clock, command palette (`Ctrl K`), ledger key (`?`).
- 21 themes (Ledger plus 20, including Hacker, Amber Terminal, Synthwave, Cyberpunk, Classic Desktop, Pocket Console, Clean Crisp, Modern Tech, Professional), each palette contrast-tested on every run; theme picker in the shell, preview gallery in Settings, palette commands.
- Every screen has a page head stating its purpose; Mission Control and Session Tracker reworked (per-block meters and totals, a timesheet summary); measured columns right-aligned in the machine voice.
- `scripts/ci_preflight.sh`: every CI job replicated on a clean `git archive` export of a revision; steps that cannot run locally are reported as SKIPPED, never as passed (testing guide §1.7).

### Changed
- CI cost guards: a newer push cancels the superseded run of the same ref (`concurrency`), every job has `timeout-minutes` (the default is 360), and the workflow can be started by hand (`workflow_dispatch`). No check was removed and the triggers are unchanged. Standard runners are free while the repository is public; a private repository draws 9 to 14 billed minutes per run (measured, runs 27 and 28) from the plan allowance.

## [0.1.0] — unreleased (2026-11-09 update)

### Fixed
- `g9-skills`: an absent `references/` directory in a skill package is now `PackageError::MissingReferences` (it was `Io`, `layout.io`); other I/O errors are unchanged. GitHub CI run 27 failed `package_rejects_each_contract_violation` because its fixture was an empty directory, which git does not store, so only a fresh checkout failed; local runs passed throughout. The test now builds the absent, empty and non-markdown cases at runtime. After the fix, GitHub CI run 28 passed all five jobs (rust incl. cargo-deny, frontend, sbom, content-gates, desktop-shell); the Tauri shell compiles and its unit tests pass on the CI runner for the first time.
- Docs: removed the claims that the shell was never compiled and that no CI result had been read; corrected the stage-report statement that the clean-export replication covered the test step (it did not).

## [0.1.0] — unreleased (2026-11-08 update)

### Fixed
- Desktop and CLI: directory approved-roots check (`ApprovedRoots::contains_directory`); backups, the pre-migration dry run and the attachment-manifest check were inert in the desktop configuration.
- `g9-competency`: validation evidence must trace to a graded attempt; an EXAM-labelled episode or an unrooted correction no longer reaches VALIDATED.
- `g9-retention`: reliability-bin edges placed probabilities one bin low.
- Desktop UI: 12 verified and 11 fresh review findings (warnings rendering, rubric codes, time-correction flow, error states, KPI definitions, calendar hours); SYNTHETIC DATA banner for the mock transport.
- Skill registry hash column recomputed under the runtime definition (`scripts/skill_hashes.py`); runtime pins unchanged.

### Added (learning loop, backups and restore, 2026-11-08)
- Finalized grades produce competency observations in the finalizing transaction with persisted validation eligibility (migration 0013); `competency.projection` honours it. Regress counts occasions; eligibility is fail-closed.
- Backups usable across restarts (database-backed manifests, `system.status.last_backup`), verified offline restore `g9 restore` (`g9_storage::restore`), `--scratch` required for `simulate-week` and `crash-drill`. Smoke test: 100 assertions.

### Added
- CLI `g9`, `scripts/smoke_test.sh` (67 assertions), testing guide; layering gate covers the CLI.
- Reference source register (48 unverified candidates, 0 approved) with schema, validator and an offline-safe verifier (`scripts/verify_source_register.py`, dry run by default).
- Defect register status re-assessment (section E), `docs/release/stage-report.md`, `docs/release/gate-evidence.*`.
- CI: `--locked` everywhere, shell tests, pinned SBOM tools, hash and register checks; `.github/CODEOWNERS`; `.gitignore` for SQLite sidecars and key files.

## [0.1.0] — unreleased (2026-11-05)

### Stage 00 — Design
- Triple audit of the v5.1 plan and the v6.0 handoff; defect register, improvement backlog and ADRs under `docs/audit/` and `docs/adr/`.
- `ARCHITECTURE.md` v6.1: 17 modules mapped to crates, dependency law, command pipeline, storage rules, time/DST policy, assessment/competency/memory model, retention scope, AI gateway and skill orchestration, authorization, UI, gates.
- Requirements traceability matrix, threat model, privacy model, evaluation protocols, UI screen flows.

### Stage 01 — Scaffold
- Cargo workspace (13 crates + Tauri shell excluded from default members), pinned dependencies, `deny.toml`, CI workflow, `g9-domain` vocabulary with state machines and invariants.
- Desktop UI scaffold with typed IPC client, mock transport, 16 screens, sanitized Markdown, accessible calendar grid.
- 37 skill packages and 8 knowledge packs with validators.

### Stage 02 — Data & importer
- SQLite schema and checksummed migrations, immutability triggers, hash-chained audit metadata, idempotency store.
- Curriculum manifest v1.0.0, seed v0 (Appendix A, `PROPOSED_SEED`), invariant validator, reconciliation diff, provenance gate.
- Backup via Online Backup API / `VACUUM INTO` with integrity and restore verification.
