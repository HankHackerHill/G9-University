# ADR-0018: The program is set up in the window and a day is planned in one command

**Status:** Accepted (2026-11-14), owner request: "the GUI version fully complete and ready to use for personal use, with the fully cybersecurity learning curriculum imported in". Extends ADR-0015 (owner decisions) and ADR-0017 (erasure). Changes no controlling rule of `ARCHITECTURE.md`.

## Context

Running the real interface against the real backend (`scripts/e2e/tour.cjs`, 2026-11-14) showed that the desktop window could not run the curriculum it was built for:

| Finding | Cause |
|---|---|
| `curriculum.daily_plan` failed on every screen | the Monday of week 1 existed only as the CLI flag `--program-start`; the shell never set it |
| nothing in the window imported the curriculum | the import was reachable only with `g9 import-seed` |
| a block could not be started from the window | a session linked to a curriculum block was written only by `g9 simulate-week`, which writes seed rows directly to storage |
| notes written before the last restart were unreachable | the Journal listed notes saved since the window opened (`notes.list` was PROPOSED) |
| an erasure could be requested only with `g9 call deletion.request` | no screen offered it |
| the 64 competencies of the 8 packs could not be loaded | no operation read a pack |
| the backups list was empty after a restart | `backup.list` was PROPOSED |
| the Calendar said curriculum blocks appear by themselves | they did not: `FILL_CURRICULUM` is not implemented |

## Decision

1. **A stored program plan.** Migration 0015 adds `program_plan` (schema version 15, digest pinned): one row per curriculum with the Monday of week 1 (`YYYY-MM-DD`), the minute of the day a planned day starts (default 540 = 09:00) and a revision. It is a table of its own, not an `app_settings` row, because the policy table requires a MAINTENANCE change set and an approval for any setting (`operator.write.system.allow` is MAINTENANCE only; policy and settings changes need `PolicyModify`). A start date changes nothing the operator would approve; it is a LEARNING record like a competency. A start given to the process (`AppConfig::program_start`, the CLI's `--program-start`, the bridge's flag) wins over the stored one, so scripted runs stay reproducible. The table is exported with the curriculum it belongs to.
2. **`curriculum.program`** (read) reports the active version (name, version label, provenance, weeks, days, objectives, planned minutes), the stored start and how many of the shipped competencies are loaded. **`curriculum.set_program_start`** refuses a date that is not a Monday (`program.start_not_a_monday`), refuses a time that is not `HH:MM` (`program.day_start_invalid`) and needs an imported curriculum (`curriculum.not_imported`); it is revision-checked and records the event `curriculum.program_start_set`. `curriculum.daily_plan` and `session.plan_day` report `curriculum.not_imported` before `curriculum.program_start_not_configured`.
3. **`session.plan_day`** creates, for each block of the program day on a date, one PLANNED session linked to the training day, the objective and the block, in the mode the curriculum gives the block (recall and performance review REVIEW; theory, reporting, remediation STUDY; labs and defence verification LAB; the blind practical EXAM). Blocks follow one another from the day start in the configured zone (`g9_calendar::resolve_local`, so DST is the calendar crate's, not the window's). It also places each block on the calendar as an event of source SCHEDULER (a study block, or an immutable assessment for the blind practical) and links the session to it. A block that already has a session is skipped, so the call plans exactly what is missing. A date with no training day answers `curriculum.no_training_day`.
4. **Events the operator cannot change are refused before policy.** `calendar.update_event` and `calendar.delete_event` check editability first, so the operator is never asked to approve a deletion that cannot happen (a scheduler or immutable event answers `calendar.event_not_editable` without an approval request).
5. **Reading notes back.** `notes.list` returns summaries (no body, size in bytes, version count, classification, area) of the scope's area, newest change first, erased notes excluded; `notes.get` returns one note as `notes.create` does and answers `note.erased` for an erased one. Bodies are loaded only for the note that is opened. The Journal erases through the existing `deletion.request` (approval, native confirmation, files removed: ADR-0017).
6. **Competency packs.** `competency.import_pack` loads the packs that ship with the application. The eight `pack.json` files are embedded in `g9-app` at build time: the request names a pack (`pack_ids`, default all) and carries no content, so the application reads no file at run time. A competency becomes a `competencies` row with its pack id as `domain_area` and its own id as `code`; each prerequisite becomes a PREREQUISITE edge from the prerequisite to the competency, completed on every run over all packs so a pack loaded later adds the edges of packs loaded before. Idempotent. The NICE references of the packs (`K-UNPINNED`, confidence ASSUMED) are not stored.
7. **Backups.** `backup.list` lists `backup_manifests` newest first with the operator label of backups this process made. A recorded path outside the approved roots is not followed and is counted (`unlisted`, warning `backup.manifest_path_outside_approved_roots`).
8. **The window.** The curriculum seed ships in the interface bundle as a lazy chunk and is read only when the operator presses **Import the curriculum**; the import is the existing approval-gated `curriculum.import` (PROPOSED_SEED). A shared `ProgramSetup` component (Mission Control while the program is not set up, Settings always), `usePlanDay`, `FlowOutcome`, week navigation in the Curriculum Navigator, the stored notes list and erase form in the Journal, a pack prompt on the Competency Graph and the stored backups list in Settings. "Program not set up" is shown as setup, never as an error. The standing fact that a plan comes from a version that is not canonical is shown as a quiet note, not as an alarm (it stays visible and keeps the backend's sentence). `?mock=fresh` starts the mock as a new install (a review aid of the browser build; the shell never reads it).

## Alternatives considered

| Option | Why not |
|---|---|
| Store the start in `app_settings` | needs MAINTENANCE and an approval for a value that is not a policy; the invariant that only an operator MAINTENANCE change set writes settings stays untouched |
| Let the window pass the start with every request | the daily plan, the Classroom, the day rail and the calendar would each need it; one stored value keeps them consistent |
| Create sessions on the first `session.start` | a session would then carry no planned window, and the day rail and the calendar could not show the day before it began |
| Let the window send the manifest or a pack file from disk | a chosen file path is a filesystem capability the mentor runtime does not have; the seed and the packs are shipped data |
| Implement `FILL_CURRICULUM` as an approval-gated proposal | it needs a placement policy over twelve weeks (holidays, conflicts) that has no owner decision yet; planning one day at a time does not |
| Map objectives to competencies from the pack texts | neither source holds the mapping; a derived mapping would be an invented number |

## Consequences

- The window is usable from an empty install with no command line. The operator's three decisions stay explicit: an approval for the import, a Monday, an optional pack load.
- Planning writes sessions and calendar events in one transaction; planning twice writes nothing new (TESTED).
- A start moved later does not move sessions already planned: they keep their own times (stated in the window).
- Export: `program_plan` is assigned to the export as a child of `curricula`; a test fails for an unassigned table.
- `notes.list` shows titles of notes in the scope's area, including SECRET ones to the operator; the policy already lets the operator read every record, and a skill or model still cannot read SECRET content (`sm.read.secret.deny`).
- Open: assessments for the twelve Saturdays, the objective-to-competency mapping, NICE pins, promotion to APPROVED_CANONICAL and `FILL_CURRICULUM` are PROPOSED (`docs/runbooks/first-run.md`, "What the program does not include").
- Reversal: drop the window's setup panel; the CLI path (`--program-start`, `g9 import-seed`) is unchanged.

## Verification

| Claim | Test |
|---|---|
| Migration 0015 is pinned; the export assigns `program_plan` | `crates/g9-storage/src/migrations.rs::the_schema_at_version_15_is_pinned_too`; `export::tests::every_table_of_the_schema_is_assigned_exactly_once` |
| The start must be a Monday, a clock time, and an imported curriculum; it is stored with a revision; the plan follows it; a process start wins | `crates/g9-app/tests/program_setup.rs` (16 tests; three were mutation-checked: no Monday check, stored plan ignored, already planned not skipped) |
| One session per block with the block's mode, times back to back, DST correct (Europe/Berlin, 2026-10-23 and 2026-10-27), events placed and not editable, repeat and mend, idempotent replay, Sunday and out-of-range refused | `program_setup.rs` |
| Notes: order, no bodies, limit, area scope, erased excluded and not openable | `crates/g9-app/tests/notes_read_back.rs` (5) |
| Packs: 64 competencies and every edge (direction, count), idempotent, later pack completes earlier edges, unknown and malformed ids refused; the embedded copy equals the files, the graph is acyclic | `crates/g9-app/tests/packs_and_backups.rs` (9; two mutation-checked: reversed edges, no idempotency check); `commands/packs.rs` unit tests |
| Backups listed newest first, path outside the roots not followed | `packs_and_backups.rs` |
| The mock answers the same, against the same response schemas | `apps/desktop/src/ipc/schemas.test.ts`, `transports/mock-setup.test.ts`, `transports/competency-catalog.test.ts` |
| The screens | `setup-flow.test.tsx`, `program-screens.test.tsx`, `journal-readback.test.tsx`, `competency-packs.test.tsx`, `settings-recovery.test.tsx`, `Warnings.test.tsx` |
| The whole flow in the real interface against the real backend | `scripts/e2e/personal.cjs` (22 checks, including the credited time, the calendar, the erased text absent from every file of the data directory, and an intact audit chain) |
| Layout of the first-run states at 1280×800 and 960×640 | `measure.cjs --fresh` (32 measurements, 32 clean) |

NOT RUN: the Tauri window, the native confirmation dialog and WebKitGTK rendering; a restore drill on the host.
