# Runbook: first run and the first day (single operator)

Scope: one person using the desktop window on one machine, from an empty install to a first planned and timed day, with the 12-week G9/CT9 curriculum imported. Labels: **TESTED** (executed in this repository), **NOT RUN** (needs the G9 host), **PROPOSED**.

| What | Status |
|---|---|
| Every step below, in the real interface against the real Rust backend (Chromium, `scripts/e2e/personal.cjs`: 29 checks, including the Classroom's lesson sheet) | TESTED, 2026-11-14 |
| The same steps against the mock backend (`setup-flow.test.tsx`, `program-screens.test.tsx`) and the backend alone (`crates/g9-app/tests/program_setup.rs`, `notes_read_back.rs`, `packs_and_backups.rs`) | TESTED |
| Building and starting the Tauri window, the native confirmation dialog, WebKitGTK rendering | NOT RUN (no GTK or WebKit in the authoring sandbox) |
| A restore drill on the host (`docs/runbooks/backup-restore.md`) | NOT RUN |

## Before the first start

| Item | Detail |
|---|---|
| Build | `cd apps/desktop && npm ci && npm run tauri:build`. Never `cargo build --release` for the shell: it would be a development build that loads its page from `http://localhost:5173` (`apps/desktop/README.md`). NOT RUN here |
| Data folder | `~/.local/share/com.g9.mentor` (Tauri `app_data_dir`): `g9.sqlite`, `binding.key` (0600), `attachments/`, `backups/`, `exports/`. Created `0700` on the first start. Keep it on an encrypted disk (`docs/privacy/data-lifecycle.md`) |
| Time zone | the host's zone, read at start; the window shows it next to the clock. Planned times and "today" follow it |
| Model | none is configured by choice. Planning, notes, timers, reviews and grading checks work without one |
| Network | off. The window offers no network action |

## The first run, step by step

Mission Control opens on **Set up your program** until the curriculum is imported and a start is chosen. The same panel is under Settings, headed **Program**.

| # | You do | The window then shows | What is stored |
|---|---|---|---|
| 1 | **Import the curriculum**, then **Approve**, then **Apply with approval**. **Approve** opens the application's own confirmation window; it names the curriculum and what policy grants. Confirm there | "Imported 12 weeks, 72 days and 216 objectives as PROPOSED_SEED." Step 1 reads "Curriculum imported", with the version and 480 h | a curriculum version `g9-ct9-480:1.0.0-draft1` (the draft built in this repository, `docs/curriculum/`), provenance PROPOSED_SEED, 12 weeks, 72 days, 216 objectives, 28 800 planned minutes, 256 resource references (trust UNKNOWN). One approval, consumed once, expiring after 10 minutes |
| 2 | Choose the **Monday week 1 starts** and the time a day starts (default 09:00), **Set the start** | the panel disappears from Mission Control; the page now plans today. A date that is not a Monday is refused before anything is sent ("Tuesday 2026-11-03 is not a Monday") | one `program_plan` row: the Monday, the minute of the day, a revision. No approval |
| 3 | Optional: **Load the competency packs** (also on Competency Graph) | "Loaded 64 competencies and N prerequisite links from 8 packs" | 64 competencies in the `competencies` table, one per pack entry, with PREREQUISITE edges between them. Nothing is graded; every competency stays "not assessed" |
| 4 | **Plan today's blocks** (Mission Control, Daily Classroom or the day's line in Curriculum Navigator) | the next block, and every block of today as a PLANNED session | one PLANNED session per block, linked to its day, objective and block, in the block's mode; one calendar event per block (placed by the scheduler: you cannot edit it) |
| 5 | **Session Tracker**: select a block, **Prepare**, **Start**; **Pause**, **Resume** and **Complete** as you work | credited time comes from the backend's own monotonic clock; time it cannot vouch for waits for your correction | session events and time intervals; credited and uncertain time recomputed from the ledger |

A day starts at the time you chose and its blocks follow one another without a gap: a weekday is 7 h (recall 15 min, theory 45, guided lab 120, independent lab 120, defence verification 60, evidence reporting 60), a Saturday is 5 h and starts with the blind practical (blind practical 120 min, remediation 60, report 60, performance review 60; the blind practical is planned in EXAM mode and is an assessment slot the scheduler cannot move). Sunday has no blocks; planning a Sunday answers `curriculum.no_training_day`.

Moving the start later changes which date is which program day. Sessions that were already planned keep the times they were planned for.

## Every day

| When | Where | What |
|---|---|---|
| Start of the day | Mission Control | **Plan today's blocks** once; the now block names what is next |
| During | Daily Classroom | the lesson sheet: one folded section per block of the day, the first block that is not finished open; what to watch for and the sources are in the margin (an address is text to copy: the window opens no link). Reading only |
| During | Session Tracker | start, pause and complete each block. Notes go in the Learning Journal |
| After | Learning Journal | write a note (kept as versions under a content hash); attach the day's evidence (the Classroom names it); the list shows every stored note, newest change first, also after a restart |
| Reviews | Retention Center | the ladder (1, 3, 7, 21, 60 days) shows what is due |
| Weekly | Settings, **Backups** | **Create backup**, then **Verify**, then **Test restore**. Only a verified and restore-tested backup counts as the last one. Copy it to another drive: `g9 copy-backup` (`docs/runbooks/backup-restore.md`) |

Erasing a note: Learning Journal, open the note, **Erase this note…**, give a reason (1 to 500 characters), **Request the erasure**, then approve. The confirmation window says how many files go with it. What stays is that an erasure was approved, when, and your reason; no digest of the text is kept (ADR-0017).

## What the program does not include

| Gap | Why | Status |
|---|---|---|
| Assessments for the Saturday blind practicals | twelve definitions are written (`data/curriculum/assessments/`: task statement, competency, difficulty; no separate rubric field exists) but none is defined in your database: load the competency packs, then `g9 call assessment.define --payload-file data/curriculum/assessments/W01-BLIND.json` (or paste into **Define an assessment**) for each week as it comes | PROPOSED: the definitions are the draft's; each blind practical still needs its scenario source (`docs/curriculum/design.md`) |
| A mapping of the 216 objectives to the 64 competencies | written (296 links in the manifest, `docs/curriculum/coverage.md`) and validated, but the importer stores none: `objective_competencies` needs a dimension and a weight that the manifest does not carry, and inventing them would be invented numbers | PROPOSED |
| Verified resources | 138 distinct external references (184 uses): none was opened; 15 were checked by search result only, 104 say ASSUMED, and 19 are numbered publications (RFCs, NIST special publications) cited by number and date from knowledge | ASSUMED: check the ones you use |
| NICE framework references on competencies | the packs carry `K-UNPINNED` / `S-UNPINNED` with confidence ASSUMED, so none is stored | PROPOSED until a NICE version is pinned |
| An approved canonical curriculum | the shipped version is the built draft (PROPOSED_SEED, 1.0.0-draft1); promoting it is `g9 reconcile`, then two approved operations, `docs/curriculum/promotion.md`. The window has no promote action | the plans say "Proposed" until then |
| Curriculum blocks placed on the calendar by a proposal (`FILL_CURRICULUM`) | not implemented; planning a day places its blocks instead | PROPOSED |
| A model | none is configured | by choice |

## If something does not look right

| You see | It means | Do |
|---|---|---|
| "Program not set up" in the header, the Classroom or the Navigator | no curriculum is imported, or no start is set (`curriculum.not_imported`, `curriculum.program_start_not_configured`) | Mission Control, steps 1 and 2 |
| "Nothing was approved: you cancelled the confirmation" | you cancelled the application's confirmation window | **Approve** again; the request stays pending for 10 minutes |
| "is not a Monday" | week 1 starts on a Monday | pick the Monday of that week |
| "The program has no training day on this date" (`curriculum.no_training_day`) | a Sunday, or a date before week 1 or after week 12 | plan another date |
| A block has "No session" | the day was not planned, or one block was added later | **Plan the missing blocks** |
| A plan labelled Proposed, with a quiet note under the page head | the curriculum version is not promoted to canonical | expected for the seed |
| "The reason is longer than 500 characters" | the erasure reason limit | shorten it |
| Time in doubt after a suspend or a crash | the backend could not vouch for the interval and does not credit it | Session Tracker, **Correct the time** |

## The same steps from the command line

```sh
G9=target/debug/g9; D=~/.local/share/com.g9.mentor      # close the window first: one writer at a time
$G9 import-seed --data-dir "$D" --manifest data/curriculum/g9-ct9-480.draft.v1.json   # step 1 (approval round trip inside)
$G9 call curriculum.set_program_start --data-dir "$D" --payload '{"date":"2026-11-16","day_start":"09:00"}'   # step 2
$G9 call competency.import_pack --data-dir "$D" --payload '{}'                # step 3
$G9 call session.plan_day --data-dir "$D" --payload '{"date":"2026-11-16"}'   # step 4
$G9 call curriculum.program --data-dir "$D" --payload '{}'                    # what is set up
```

`--program-start YYYY-MM-DD` on any `g9` command overrides the stored start for that process only; without it the stored start is used.

## Re-running the verification

```sh
cargo build -p g9-cli --example bridge_stdio                  # the stand-in for the shell: same backend, JSON lines
(cd apps/desktop && npm run build)
node scripts/e2e/personal.cjs --out /tmp/g9-e2e                # 22 checks, screenshots, report.json
```

The scenario refuses to run on a Sunday (the program has no blocks then). It does not exercise the Tauri shell itself.
