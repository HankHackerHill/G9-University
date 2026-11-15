# Path to 10/10

**Status:** PROPOSED (2026-11-15). A plan: nothing in it is done unless a row says so.
**Baseline:** the ratings of 2026-11-15 at commit `2e0b344` (section 2).
**Labels:** repository facts below were read on 2026-11-15 (VERIFIED). Names, flags and versions of tools that are not in the repository are ASSUMED and are checked against their current documentation when the step is executed. No threshold is invented here: a threshold comes from an existing protocol document, from a measured baseline, or is marked "your choice".
**Related:** `docs/audit/improvement-backlog.md` (Stage 00 backlog, ids `IMP-*` and register rows), `docs/requirements/rtm.md`, `docs/release/stage-report.md`.

## 0. Rules

### 0.1 What 10 means, and who can get there

The scale of the rating: 10 = complete with no known material gap, kept that way by an automated gate, run in the real environment, and validated in real use by someone other than the builder. An agent session can deliver only the first two.

| Code | Who | What it proves | Highest score it supports |
|---|---|---|---|
| A | an agent session in the sandbox (no display, no GPU, no access to your data directory) | complete and gated | 8.5 |
| H | you on the G9 host (display, GPU, the real data directory) | it works where it is used | 9 |
| O | you as owner: a decision or a review that needs no host | the decision is yours and recorded | unblocks other steps |
| X | an independent reviewer: a person who did not build it; for security, someone who has not read the audit reports first | the builder's blind spots | 9.5 |
| T | elapsed real use: the 12-week run | it holds up | 10 |

Consequence: no plan reaches 10 in every area sooner than one complete 12-week run plus the independent reviews. Agent work alone lifts the areas to about 8 to 8.5 (section 2).

### 0.2 Evidence

- A step is done when its "Done when" evidence exists in the repository or as a row of `docs/release/path-to-10-evidence.md` (created with the first finished step; format in section 8). Not executed means NOT RUN.
- A score moves only when every step of that area for that level is done. The final re-rating is made by you and the reviewers against the "10 means" line of each area (READY-7), not by the agent that did the work.

### 0.3 Constraints this plan respects

| Constraint | Effect on the steps |
|---|---|
| CI is manual-only (your decision) | No step starts a workflow. The gate is `scripts/ci_preflight.sh` (jobs `rust`, `desktop-shell`, `frontend`, `content-gates`, `sbom`). You may dispatch `ci.yml` yourself |
| Installing software needs your approval | Steps that need a new tool are marked D10 |
| `ARCHITECTURE.md`, `CLAUDE.md` and the root `Cargo.toml` are edited by their owner | Steps that need such an edit are yours (O) |
| No shell, network or arbitrary filesystem capability in the mentor runtime | The model runs locally; downloads are yours; no updater |
| Exact dependency versions | A new dependency is pinned at the version current on the day and must pass `cargo deny` |
| Nothing invented | Thresholds: from `docs/evaluations/*`, `docs/ui/performance-budgets.md`, a measured baseline, or "your choice" |

## 1. Corrections to the rating of 2026-11-15

Found while grounding this plan. No score changes; the wording was wrong or incomplete.

| The rating said | Fact (VERIFIED 2026-11-15) |
|---|---|
| Area 21: "one crate declares a bench target" | False match on the word "criterion" in the description of `crates/g9-assessment/Cargo.toml`. No bench target exists. `crates/g9-ai-adapter/src/bench.rs` describes the model benchmark protocol and holds no measurement |
| Areas 11, 16: the single-writer rule is "documented, not verified as enforced" | Enforced: `crates/g9-app/src/lock.rs` holds an exclusive OS lock on `<database>.lock`; a second process is refused with `RESOURCE_UNAVAILABLE app.data_in_use` |
| Areas 14, 20: no advisory result recorded | The audit of 2026-11-13 recorded an offline RustSec scan (database of 2026-11-06, 486 packages: 0 vulnerable, 2 informational) and `npm audit` (production 0, development 7 moderate). Not run: `cargo deny` with a current database (licences, bans, sources) |
| Area 22: "no axe-style scanner" | True for runtime scanning. Static accessibility lint is on: `eslint-plugin-jsx-a11y`, strict configuration |
| Area 10: "never compiled" | Type-checked and linted with a stub `pkg-config` (audit of 2026-11-13, section 4). Never linked, built or run; its tests NOT RUN |
| Area 15: no at-rest encryption (INFERRED) | Confirmed by `docs/privacy/data-lifecycle.md`: the default is full-disk encryption of the host (ASSUMED present); SQLCipher and encrypted backups are PROPOSED. `secure_delete = ON` and WAL truncation are implemented |
| Area 23: "at least 29 tracing calls" | No log sink exists: no crate installs a subscriber or an appender, so the calls write nowhere |
| Area 18 (new) | `ARCHITECTURE.md` section 11 says approvals expire after 15 minutes; the build uses 10 (`docs/runbooks/first-run.md`, ADR-0021) |
| Area 5 (new) | The lesson text shown in the Classroom is not bound by the promotion: a sheet is matched by objective text (ADR-0022) and `apps/desktop/src/lib/lesson-sheets.ts` checks no checksum. Step CUR-2 |

## 2. Score ladder

Expected score after each kind of work, if its steps pass (a forecast, ASSUMED). A failed step holds the score. "n/a" = that kind of work is not needed for the area.

| # | Area | Now | After A | After H | After X | After T |
|---|---|---|---|---|---|---|
| 1 | Architecture and layering | 8 | 8.5 | 9 | 9.5 | 10 |
| 2 | Storage and data integrity | 8.5 | 8.5 | 9 | 9.5 | 10 |
| 3 | Policy, approvals and authority | 8.5 | 8.5 | 9 | 9.5 | 10 |
| 4 | Application layer and command pipeline | 8 | 8.5 | 9 | 9.5 | 10 |
| 5 | Curriculum engine | 8.5 | 8.5 | 9 | 9.5 | 10 |
| 6 | Curriculum content | 6 | 7.5 | 9 | 9.5 | 10 |
| 7 | Competency, assessment and retention | 6.5 | 8 | 8.5 | 9.5 | 10 |
| 8 | Calendar and evidence | 7.5 | 8.5 | 9 | n/a | 10 |
| 9 | AI mentor and skills runtime | 5 | 6.5 | 8.5 | 9.5 | 10 |
| 10 | Desktop shell and IPC | 5 | 6 | 9 | 9.5 | 10 |
| 11 | CLI and tooling | 8 | 8.5 | 9 | n/a | 10 |
| 12 | UI/UX and design system | 7.5 | 8 | 9 | n/a | 10 |
| 13 | Frontend engineering | 8 | 8.5 | 9 | n/a | 10 |
| 14 | Security engineering | 7.5 | 8 | 9 | 9.5 | 10 |
| 15 | Privacy and data lifecycle | 8 | 8.5 | 9 | 9.5 | 10 |
| 16 | Reliability and recovery | 7 | 8 | 9 | n/a | 10 |
| 17 | Testing and verification | 8 | 8.5 | 9 | 9.5 | 10 |
| 18 | Documentation and governance | 8.5 | 8.5 | 9 | 9.5 | 10 |
| 19 | Build, release and packaging | 4 | 6 | 9 | 9.5 | 10 |
| 20 | Dependency and supply chain | 6.5 | 8.5 | 9 | 9.5 | 10 |
| 21 | Performance | 6 | 7.5 | 9 | n/a | 10 |
| 22 | Accessibility and localisation | 6.5 | 8 | 9 | 9.5 | 10 |
| 23 | Observability and diagnostics | 6 | 8 | 9 | 9.5 | 10 |
| 24 | Build process and provenance | 7 | 7.5 | 8 | 9.5 | 10 |
| 25 | Personal-use readiness | 6.5 | 7 | 9 | n/a | 10 |

## 3. Decisions needed from you

| # | Decision | Options | Recommended | Blocks |
|---|---|---|---|---|
| D1 | Who reviews independently (X) | a G9 colleague; an outside reviewer; nobody | a colleague for security (X1) and for double grading (LEARN-7); section 6 lists the four engagements | every step from 9 to 9.5 |
| D2 | How the model runtime is reached | in-process llama.cpp binding; a child process over a Unix socket or stdio; loopback HTTP (needs an exception to the `reqwest`/`hyper` ban of `deny.toml`) | decide by `docs/evaluations/model-benchmark-protocol.md` and the trade-off summary PLT-05, as an ADR; do not weaken `deny.toml` outside that ADR | area 9 |
| D3 | Source of the 12 Saturday blind practicals | (a) a colleague prepares 12 sealed briefs; (b) an agent prepares sealed briefs that you do not open before the day; (c) external pre-built targets chosen by id only | (a) if D1 gives you a colleague, else (b) | areas 6, 7; the first Saturday |
| D4 | Weights of the 296 objective-to-competency links | one declared rule (equal weight per objective); weights set by hand; links stay unstored | the declared rule, recorded in an ADR as a rule, not as a measurement | areas 5, 7 |
| D5 | Encryption | full-disk encryption only (verify it, PRIV-1); encrypted backup sets (IMP-M08); SQLCipher (IMP-M06) | verify the disk; encrypt the copies that leave the machine; SQLCipher only if the disk is shared | area 15 |
| D6 | Audit chain: keyed, externally anchored, or as it is (ADR-0004 PROPOSED) | leave; anchor outside the file; key the chain | the external anchor in the copied backup manifest (STOR-3) | areas 2, 14 |
| D7 | `executive.*` and `research.*` (8 PROPOSED operations, 2 screens without operations) | build now; build after week 6 of the run; remove by ADR | after week 6, or remove: they are not on the learning path, and `research.source_fetch` needs the separately approved network module | areas 4, 12 |
| D8 | Language | English only; i18n-ready; translated | English only by ADR. For the literal 10: i18n-ready (A11Y-6) | area 22 |
| D9 | Release form | a `.deb` built on the host, checksummed and signed with your key, no updater | as stated | area 19 |
| D10 | Tools in the agent sandbox: `cargo-llvm-cov`, `cargo-mutants`, `cargo-deny`, `cargo-fuzz`, a secret scanner, `@axe-core/playwright`, vitest V8 coverage | approve; or run those steps on the host | approve (development only, exact versions) | areas 13, 14, 17, 20, 22 |

## 4. Order of work

| Phase | Who | Content | Unblocks | Size (ASSUMED) |
|---|---|---|---|---|
| 0 Host bring-up | H | section 4.1 | every H step; areas 10, 16, 19, 25 | half a day |
| 1 Gates and measurement | A | ARCH-1, ARCH-2, STOR-1, STOR-2, STOR-4, POL-1, POL-2, APP-2 to APP-5, CLI-1, CLI-2, UX-1, FE-1, FE-3 to FE-5, SECUR-2 to SECUR-5, PRIV-3, PRIV-4, PRIV-7, PRIV-8, REL-3, REL-5, TEST-1, TEST-2, TEST-4, TEST-5, DOC-1, DOC-2, DOC-4, BUILD-2, BUILD-3, SUP-1, SUP-2, SUP-4, PERF-1 to PERF-4, A11Y-1 to A11Y-3, OBS-1 to OBS-6, PROC-7 | the "After A" column | 10 to 14 sessions |
| 2 Content | A, O, H | CONT-1 to CONT-8, CUR-1 to CUR-4, LEARN-6 | promotion; the first Saturday | 6 to 9 sessions, plus your reading |
| 3 Mentor | O, A, H | AI-1 to AI-11, LEARN-1 to LEARN-5 | area 9; the mentor weeks of the run | 2 to 4 weeks elapsed |
| 4 Independent review | X | section 6 | the "After X" column | 8 to 14 reviewer days in total |
| 5 The 12-week run | T | every T step; evidence at weeks 1, 2, 4, 6, 8, 12 | the "After T" column | 12 weeks |

Dependencies:

- Phase 0 comes first. Phases 1 and 2 run in parallel with it.
- The run (phase 5) can start on the PROPOSED draft as soon as phase 0 and D3 are done: read week N+1 during week N (CONT-8) and promote when the whole draft is read (CUR-4).
- The mentor must be on by week 8 at the latest, to leave four weeks of use for AI-13.
- X1 and X2 need phases 0 and 1 finished. X3 runs during the run.
- Earliest date for 10 in every area: about 13 to 16 weeks after phase 0 (ASSUMED).

### 4.1 Phase 0: host bring-up, exact steps

Clone the repository first if it is not on the host. Every line is yours to run; nothing here starts CI.

```sh
# 1. Record the host (read-only). Paste the output into docs/evaluations/host-inventory.md (AI-1, PRIV-1)
lsb_release -ds; echo "$XDG_SESSION_TYPE"; lscpu | head -20; free -h; df -h "$HOME"
nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv
findmnt -no SOURCE -T "$HOME"; lsblk -o NAME,TYPE,FSTYPE,MOUNTPOINT   # expect a "crypt" device under the home filesystem
rustup show active-toolchain; node -v                                  # expect 1.97.0 and 22.13 or later

# 2. Packages (apps/desktop/README.md)
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev

# 3. Source
git fetch origin ccr-0cea681c-p9jbru && git checkout ccr-0cea681c-p9jbru

# 4. The gates on a clean export, shell job included (the sbom job needs cargo-cyclonedx: skip it the first time)
scripts/ci_preflight.sh --skip sbom          # expect: no FAIL; "desktop-shell" PASS, not SKIPPED (SHELL-2)

# 5. Build, install, start (SHELL-3)
(cd apps/desktop && npm ci && npm run tauri:build)
sudo apt install ./<the .deb named at the end of the build>

# 6. In the window: docs/runbooks/first-run.md steps 1 to 5, then the human checklist of
#    docs/audit/security-audit-2026-11-13.md section 4 (SHELL-4, POL-3). Keep the screenshots.

# 7. In the window: Settings, Backups: Create backup, Verify, Test restore. Then close the window.
cargo build -p g9-cli
G9=target/debug/g9; D="$HOME/.local/share/com.g9.mentor"
$G9 status --data-dir "$D"                   # note last_backup.backup_id
$G9 copy-backup --backup-id <id> --to /media/usb/g9-backups --data-dir "$D"

# 8. Scratch restore from the copy; the live data is not touched (REL-1)
$G9 restore --snapshot <the snapshot file copy-backup wrote> \
  --expect-sha256 <sha256 in /media/usb/g9-backups/<id>.manifest.json> \
  --data-dir /tmp/g9-drill --confirm-replace-live-database
$G9 init --data-dir /tmp/g9-drill            # expect audit_chain_ok: true

# 9. Once, while the data is a day old: the live restore (docs/runbooks/backup-restore.md, "Procedure" steps 1 to 6)
$G9 restore --backup-id <id> --data-dir "$D"                                      # prints the plan, changes nothing
$G9 restore --backup-id <id> --data-dir "$D" --confirm-replace-live-database     # old database goes to quarantine/
```

Failure signatures: a blank window on the NVIDIA driver (record the override of register row TEC-36 with the driver and WebKitGTK versions); `restore.database_in_use` (the window is still open); `app.data_in_use` (a second process holds the lock); `desktop-shell` SKIPPED in step 4 (a package of step 2 is missing).

## 5. Plans per area

### 5.1 Architecture and layering (8 to 10)

10 means: the dependency law is checked by a test; IPC contracts cannot drift between Rust, the schemas and TypeScript; the shell boundary has run on the host; an independent reader finds no layering violation; none appears during the run.

| ID | Who | Step | Done when |
|---|---|---|---|
| ARCH-1 | A | Write the layering test planned as IMP-M05 at `crates/g9-evaluation/tests/layering.rs`: run `cargo metadata --format-version 1 --locked --offline`, keep the workspace-internal edges, compare them with an allow-list in the test (one line per crate, from `ARCHITECTURE.md` section 2 rules 1 to 5 and 7), and assert that `g9-ai-adapter` and the pure domain crates have no `rusqlite` in their dependency closure. Today the preflight only counts the dependency tree | Fail-first: a scratch copy in which `g9-storage` depends on `g9-app` fails and names the edge; the real tree passes |
| ARCH-2 | A | IPC drift gate (IMP-M01, register row TEC-38). First, with no new dependency: one recorded request and response per routed operation (53, recorded through `apps/cli/examples/bridge_stdio.rs`), validated against the schemas in a Rust test and in vitest (`ajv` is already a development dependency), and an assertion that every operation of `g9 ops` has a fixture. Generating schemas and TypeScript from the Rust types is a second step, by ADR only | A field renamed on one side only fails `cargo test` or `npm test`; the assertion prints 53 of 53 |
| ARCH-3 | A, O | ADR "module boundaries of `g9-storage`" (30% of the Rust source): lines per module, modules over 2,000 lines, split or no split with the reason. A split needs your edit of the root `Cargo.toml` and of `ARCHITECTURE.md` | ADR accepted by you; if split, ARCH-1 covers the new crates |
| ARCH-4 | H | Run the shell: SHELL-1 to SHELL-4 | SHELL-4 evidence |
| ARCH-5 | X | Engagement X2: read `ARCHITECTURE.md` and ADR-0001 to ADR-0022, then trace `notes.create`, `assessment.approve_final` and `curriculum.promote` from `g9_command` to the audit row | Signed entry in `docs/release/review-log.md`; every HIGH or MEDIUM finding closed with a test |
| ARCH-6 | T | Every change during the run passes `scripts/ci_preflight.sh --only rust` | No layering failure merged in 12 weeks |

### 5.2 Storage and data integrity (8.5 to 10)

10 means: a kill at any named point of a multi-file operation leaves a state that opens clean or is reported; backup verification covers attachment bytes; the audit head is anchored outside the database file; restore is proven on the host; 12 weeks without an unexplained integrity finding.

| ID | Who | Step | Done when |
|---|---|---|---|
| STOR-1 | A | Crash matrix with named fault points behind a cargo feature `fault-injection` (off by default) at the boundaries of: `evidence.attach_file` (file written, row not committed); `backup.create` (snapshot written, manifest row not written; attachments half copied); an approved `deletion.request` (files removed, rows not yet redacted); `export.request` (directory half written); a migration (pre-migration backup taken, migration not committed); `g9 restore` (the three points `restore::tests` simulates). A driver runs `g9` once per point, selected by an environment variable, lets the process abort, then runs `g9 init` | For every point `g9 init` reports `audit_chain_ok: true` and a consistent state or a named warning; the matrix is committed; "NOT RUN: a real `SIGKILL` of the CLI" in `docs/runbooks/backup-restore.md` becomes TESTED; a test proves the feature is absent from the default build |
| STOR-2 | A | `backup.verify` and `backup.restore_test` hash the attachment files (PROPOSED in the runbook) | Fail-first: one flipped byte under `<data>/backups/attachments/` is reported |
| STOR-3 | O, A | D6. Recommended: `g9 copy-backup` already writes `<id>.manifest.json`; add the audit head (sequence and row hash, keyed under a new digest domain) and compare it in `g9 restore` and in a new `g9 verify-anchor --manifest <file>` | Fail-first on a scratch database: the attack the runbook lists as undetected (tail rewritten, counter reset, manifests rewritten) is detected with the anchor file; ADR accepted |
| STOR-4 | A | Disk full: the proposed free-space threshold for `backup.create` (runbook: twice the database size; your choice) and a test of `SQLITE_FULL` with `PRAGMA max_page_count` on a scratch database | The command answers `RETRYABLE_STORAGE`, nothing is half written, the audit chain verifies |
| STOR-5 | H | Restore drill: REL-1 | REL-1 evidence |
| STOR-6 | X | Engagement X1, on a scratch copy: `UPDATE` and `DELETE` with the `sqlite3` tool on every immutable table; open a database with a planted schema object | Every attempt refused; outputs in the review log |
| STOR-7 | T | Weekly: Settings, Backups: Create, Verify, Test restore, then `g9 copy-backup`. Monthly: section 4.1 step 8 from the copy | 12 weekly and 3 monthly rows; no `DATA_CORRUPTION` |

### 5.3 Policy, approvals and authority (8.5 to 10)

10 means: decisions are property-tested; every operation is proven to pass the pipeline; a person has exercised the OS dialog; an independent attacker fails; the approval ledger of 12 weeks has no unexplained refusal.

| ID | Who | Step | Done when |
|---|---|---|---|
| POL-1 | A | Property tests in `crates/g9-policy` (`proptest` is already a workspace dependency): for random actor, action, resource kind, mode and domain the decision is Deny unless a built-in rule matches; model and skill actors never receive an operator-only capability; changing any one field of an approved `OperationDescriptor` yields a refusal | Tests pass; TEST-2 leaves no unexplained surviving mutant in the engine and the binding |
| POL-2 | A | Registry completeness test over `crates/g9-app/src/ops.rs`: each operation has a policy rule, a derived interaction mode, an audit row on success and a decision row on refusal; each approval-expected operation refuses without an approval. Confirm first what `crates/g9-policy/tests/a06_policy_matrix.rs` already covers; add what is missing. Closes RTM R-005 "PARTIAL (`g9-app` wiring per command)" | The test iterates the registry: a new operation without a rule fails the build; R-005 row updated |
| POL-3 | H | The native dialog (audit of 2026-11-13, section 4): raise an approval; the dialog names the record; "Approve" does nothing for about 1.5 s; decline once; raise two requests | Screenshots; `audit.recent` shows one metadata-only row for the declined confirmation; two dialogs do not stack |
| POL-4 | X | Engagement X1, black box through the CLI and the installed window, before reading `tests/adversarial`: replay, parameter swap, stale revision, expired approval, approval for another operation, replaced `binding.key` | No bypass; every scenario not in the ST catalogue becomes a fixture |
| POL-5 | T | At weeks 4, 8 and 12 count approvals requested, approved, expired and refused (`g9 approvals`); explain every `approval.mac_invalid` and `approval.operation_mismatch` | Three dated rows; none unexplained |

### 5.4 Application layer and command pipeline (8 to 10)

10 means: every planned operation is built to one checklist or removed from scope by an ADR; no undecided refusal code is left; the RTM has no PARTIAL row for the pipeline.

| ID | Who | Step | Done when |
|---|---|---|---|
| APP-1 | O | D7: scope of the 15 PROPOSED operations. `teaching.*` (4) is built in AI-6, `analytics.*` (3) in LEARN-1 to LEARN-3. `executive.*` (4) and `research.*` (4): after week 6 of the run, or removed by ADR | ADR accepted |
| APP-2 | A | `docs/release/operation-checklist.md`, the definition of done of one operation: request and response schema, `OpSpec` row, policy rule, handler, repository, idempotency, audit row, refusal codes, unit and integration tests, an adversarial case when it takes free text, mock parity, UI, RTM row | File exists; ARCH-2 and POL-2 enforce its mechanical parts |
| APP-3 | A | Close each explicit refusal or keep it by ADR: `calendar.fill_curriculum.not_implemented`, `calendar.reschedule.not_implemented`, `deletion.study_session.not_implemented`, `skills.eval.not_implemented`, and `StorageError::NotImplemented` for `calendar.event_kind`, `calendar.recurrence.monthly` and the one in `crates/g9-storage/src/repos/competency.rs` | A gate script compares the refusal codes in the source with an allow-list file in which each kept code names its ADR |
| APP-4 | A | RTM R-017 "PARTIAL (repository scope checks in `g9-app`)": a cross-domain test for every list and get operation | R-017 IMPLEMENTED with the test names |
| APP-5 | A | INFO item `already_stored` (`evidence.attach_file`): keep it and document it as an answer within one scope, or answer uniformly | Decision in the audit document; a test of the chosen behaviour |
| APP-6 | X, T | The traces of ARCH-5; in use, every refusal you meet is a correct refusal or a filed defect | No open defect against the pipeline at week 12 |

### 5.5 Curriculum engine (8.5 to 10)

10 means: what you approve is everything the Classroom shows; the links to competencies are stored under a rule you set; the window can promote; a second version has gone through the route.

| ID | Who | Step | Done when |
|---|---|---|---|
| CUR-1 | O, A | D4, then manifest schema 1.1.0 (register row SEC-30 belongs here): the link rule or explicit weights, `lesson_sheets {schema, sha256}`, `source_document {title, sha256}`. The importer persists `objective_competencies`. `scripts/build_curriculum.py`, `scripts/check_curriculum_seed.py`, the validator of `crates/g9-curriculum` and the A01 tests change together | Fail-first tests; importing the rebuilt draft stores 296 links; ADR accepted |
| CUR-2 | A | Bind the lesson text (section 1, last row). The Classroom shows a sheet as covered only when the SHA-256 of the sheets file equals the value stored with the imported version; otherwise it says that the lesson text is not covered by this curriculum version | Fail-first: one changed word in the sheets file removes the covered state; the reconciliation diff changes when the sheets change |
| CUR-3 | A | Promotion in the window: a read-only `curriculum.reconcile` operation, a "Curriculum versions" panel in Settings, a native confirmation that names the three checksums; built by APP-2 | `scripts/e2e/personal.cjs` promotes in a scratch directory; each refusal of `docs/curriculum/promotion.md` is shown in words |
| CUR-4 | H | Promote on your data directory: `docs/curriculum/promotion.md` steps 0 to 6, after CONT-8 | `provenance: APPROVED_CANONICAL`; Classroom and Navigator read "Verified" |
| CUR-5 | X | Engagement X2: recompute the candidate checksum with `python3 -I scripts/check_curriculum_seed.py` and compare it with the promoted row | Equal; noted in the review log |
| CUR-6 | T | One real revision: version 1.1 from the content defects of the run (CONT-10), reconciled against the canonical 1.0 and promoted | A second `APPROVED_CANONICAL` row; earlier plans unchanged |

### 5.6 Curriculum content (6 to 10)

10 means: every reference opened and recorded; every sheet meets a written rubric; every guided lab executed from the sheet alone; the blind practicals sealed; reviewed by a second practitioner; completed once, with the defects folded into version 1.1.

| ID | Who | Step | Done when |
|---|---|---|---|
| CONT-1 | A, O | Generate the hosts of the 138 references from the draft manifest into `docs/curriculum/resource-hosts.txt`; you allow them in the session's network settings (known blocked: `genai.owasp.org`, `modelcontextprotocol.io`, `attack.mitre.org`) | List committed; one page per host can be fetched |
| CONT-2 | A | Open every reference. Record per reference: address, title as served, version or date, terms of use, access (free, account, paid), state `CHECKED <date>`, `MOVED`, `DEAD` or `PAYWALLED`. Replace dead ones in `docs/curriculum/weeks/`. Add `--require-checked` to `scripts/build_curriculum.py` | The builder prints 0 ASSUMED of 138; each lesson statement that cites a reference was re-read against it |
| CONT-3 | A | Write `docs/curriculum/sheet-rubric.md` and apply it to the 72 sheets: a checkable success criterion per objective; guided lab with prerequisites, exact steps, expected output and failure signatures; independent lab without its solution; defence verification names what is verified; evidence artifact with its acceptance; time per block equal to the block's minutes; at least three self-check questions with the answers kept apart | A table of 72 rows by criterion with no failing cell; the mechanical criteria are builder checks |
| CONT-4 | A, H | Labs as files: `labs/week-NN/` with the stack, image names and digests, bring-up and teardown scripts, and a smoke check for the first step of each guided lab. Targets are your own isolated machines only, with no route to the internet or your LAN. The application never starts them | On the host `labs/week-NN/smoke.sh` exits 0 for the 12 weeks |
| CONT-5 | O or X | D3: twelve sealed blind practicals (scenario, target, rubric whose weights sum to 100, sealed key). Then define the twelve assessments (`docs/runbooks/first-run.md`, "What the program does not include") | 12 definitions in your database before the first Saturday; you have not read the briefs |
| CONT-6 | O, A | Pin a NICE Framework version; fill the `K-UNPINNED` and `S-UNPINNED` references through `scripts/gen_competency_packs.py` | `scripts/check_competency_packs.py` passes with no UNPINNED entry; gate B01 (i) has 0 orphans |
| CONT-7 | H | Execute every guided lab once from the sheet alone, at the latest on its day; record the failing step and the minutes against the block | 60 weekday rows in `docs/curriculum/content-defects.md` |
| CONT-8 | O | Read the sheets: week N+1 during week N is enough to start; the whole draft before CUR-4 | Your review noted per week in the content defect file |
| CONT-9 | X | Engagement X3: a second practitioner reviews the weeks outside your specialty and weeks 9 to 11 against current primary sources | Findings in the content defect file; every wrong statement corrected |
| CONT-10 | T | Complete the 12 weeks; log each unclear step, wrong fact and time overrun on the day; fold them into version 1.1 (CUR-6) | Every row of the defect file has a state; version 1.1 promoted |

### 5.7 Competency, assessment and retention (6.5 to 10)

10 means: the KPI, probe and calibration reports are backend operations that show their sample sizes; blindness is enforced by the exposure ledger; a second grader has checked rubric validity; the fixed review ladder is confirmed or changed from this learner's data.

| ID | Who | Step | Done when |
|---|---|---|---|
| LEARN-1 | A | `analytics.kpi_report`: the 15 KPIs of `docs/evaluations/kpi-definitions.md` as stored queries with numerator, denominator, window and n; below the minimum n the answer is `INSUFFICIENT DATA`. The Performance Dashboard reads this operation instead of computing in the browser from five read operations | A fixture database with a known answer per KPI; the screen shows n beside every figure |
| LEARN-2 | A | `analytics.recall_probe_report` and the probe sampling itself (seeded 10% sample, `review_attempts.probe`), with the Wilson interval | Same seed, same sample; fixture test |
| LEARN-3 | A | `analytics.calibration_report` (Brier score, reliability bins) and the system forecast before each blind practical (register row LRN-31) | Fixture test; a forecast row exists before the attempt starts |
| LEARN-4 | A | Confirm IMP-M03 and IMP-M04 end to end. Domain code exists (`crates/g9-assessment/src/{exposure,integrity,definition}.rs`; `ReliabilityClass` in the storage repositories): check that the operations and the Assessments screen use it, then close or fix the register rows | A test through `assessment.begin` proves a seen variant is not served as blind; register rows updated |
| LEARN-5 | A | Tests for the thinnest engines: property tests for the review ladder (a due date is never in the past; a failed recall returns to offset 1; the daily budget reports overflow and drops nothing); an integration test directory for `g9-competency` through `g9-app` | Tests pass; TEST-2 kills the mutants in `g9-retention` |
| LEARN-6 | O | Before week 1, pre-register in `docs/evaluations/preregistration-<date>.md`: outcomes, windows, minimum n and your acceptance bands | Committed before the first session |
| LEARN-7 | X | Gate B01: the reviewer of D1 scores at least 5 of your 12 Saturday submissions without seeing your score; a difference above 10 points on a criterion triggers a rubric revision (`docs/evaluations/learning-evaluation-protocol.md`) | `assessment_versions.validity_review_ref` filled; report `b01-<date>.md` |
| LEARN-8 | T | Gates B02 and B06 from real data: probe recall at 7 and 21 days (at least 10 probes), transfer checks (at least 3), planning overhead over 2 weeks (provisional target 0.15, yours to set), calibration (at least 20 observations) | Reports under `docs/evaluations/`; every figure with its n |
| LEARN-9 | T, O | Decide the ladder from LEARN-8: keep 1, 3, 7, 21, 60 days, or run `docs/evaluations/fsrs-evaluation-protocol.md` (6 weeks, then a 6-week crossover) | ADR with the report attached |

### 5.8 Calendar and evidence (7.5 to 10)

10 means: no refusal code left in the calendar path; every transition of your time zone tested; the attachment checks decided; 12 weeks of real days planned without a wrong time.

| ID | Who | Step | Done when |
|---|---|---|---|
| CAL-1 | A | Implement or close by ADR: `calendar.propose` with `FILL_CURRICULUM`, reschedule, monthly recurrence, the other event kinds (part of APP-3) | APP-3 gate |
| CAL-2 | A | Zone test over data, not samples: for the host zone and three others, every DST transition from 2026 to 2030: a planned block keeps its civil start and its minutes. Wire gate A02 into the harness (the RTM says "NOT RUN by harness") | A02 is TESTED in `docs/release/gate-evidence.md` |
| CAL-3 | A | Attachments: for PNG, parse to the end chunk and refuse trailing bytes (the open INFO polyglot); list the accepted types and apply the same rule to each | The audit's PNG-plus-archive file is refused |
| CAL-4 | H | Suspend the machine during a running session; change the clock by an hour during another | The interval is UNCERTAIN, not credited; "Correct the time" in Session Tracker fixes it |
| CAL-5 | H | Attach files at and above 16 MiB; erase a note that has a file; look in `<data>/attachments` | The file is gone; the limit is enforced with a message |
| CAL-6 | T | 72 planned days | No block at a wrong time; every Saturday slot in EXAM mode; evidence attached for every day |

### 5.9 AI mentor and skills runtime (5 to 10)

10 means: a local model selected by the benchmark protocol runs on the host; the four teaching operations exist; the 37 skills meet the thresholds of the skill protocol against that model; the adversarial catalogue passes with the model in the loop; a person has graded the mentor's output over at least 4 weeks of use.

| ID | Who | Step | Done when |
|---|---|---|---|
| AI-1 | H | Host inventory (register row TEC-39): section 4.1 step 1 into `docs/evaluations/host-inventory.md` | File committed |
| AI-2 | O, A | D2 as an ADR, from the adapter table of `docs/evaluations/model-benchmark-protocol.md` and PLT-05. `deny.toml` bans `reqwest` and `hyper`; an HTTP client needs an explicit exception inside that ADR | ADR accepted |
| AI-3 | A | Author the fixtures the protocol names; `data/fixtures/evals/` does not exist yet: 100 known-answer items across the 8 packs, each with its source; the rubric for 20 lesson generations; 200 prompts across 10 skills | Fixtures validate against a schema; you or X3 reviewed the answers |
| AI-4 | A | The adapter behind `InferenceRuntime` in `crates/g9-ai-adapter`: timeout, cancellation, budgets, schema-constrained decoding where the runtime has it (IMP-M07), load only with `Capability::ModelLoad` and a checksum match. `NullRuntime` stays the default | Contract tests pass against `ScriptedRuntime` in the sandbox and against the real runtime on the host |
| AI-5 | H, O | Candidate registry: 3 to 5 quantized candidates in the 7B to 14B range of the protocol, with publisher, source, licence class (PLT-06) and SHA-256. You download and check them; the application fetches nothing | `model_registry` rows; hashes match |
| AI-6 | A | `teaching.propose_lesson`, `teaching.propose_hint`, `teaching.propose_intervention`, `teaching.review_proposal`, built by APP-2. Model output creates PROPOSED records only; EXAM mode refuses hints; tighten the intervention schema the RTM flags (R-011: `steps: []` and a negative budget are accepted today) | A07 and A11 tests; adversarial fixtures per operation |
| AI-7 | A | Execute skill evaluations (IMP-M10; today `skills.eval.not_implemented`): run `evals/{positive,negative,adversarial,acceptance}.json` of each package, write `skill_evals` and `docs/evaluations/skill-evals/<name>.md` | The harness runs against `ScriptedRuntime` in the sandbox |
| AI-8 | H | Run the 13 measurements of the benchmark protocol for every candidate and apply its decision rule: structured validity of at least 98% with constrained decoding, zero security-regression failures, correctness inside the band you pre-register before the run, memory headroom, licence | ADR "model selected" with the numbers; it ends the deferral of ADR-0009 |
| AI-9 | H | Run AI-7 with the selected model. Thresholds of `docs/evaluations/skill-evaluation-protocol.md`: positive activation at least 95%, false activation at most 2%, schema validity 100%, protected action without approval 0, fabricated evidence 0, model-only final grade 0 | A skill becomes EVALUATED only when it passes; the others stay off |
| AI-10 | H | The adversarial catalogue ST01 to ST12 with the model in the loop, plus instructions injected into notes, resources and skill descriptions (register row SEC-19) | 0 failures |
| AI-11 | A | Mentor Memory Explorer and Intervention Center become working screens: accept or reject a hypothesis; gate B05 (dependents are invalidated) | B05 report; `scripts/e2e/tour.cjs` finds no placeholder on these screens |
| AI-12 | X | Engagement X3: a colleague tries to make model output change state without an approval, read SECRET content in EXAM mode, or forge evidence references | No success; new cases become fixtures |
| AI-13 | T | At least 4 weeks of the run with the mentor on. Each week grade 10 sampled outputs: correct, grounded in the cited evidence, at the right level, safe | `docs/evaluations/mentor-quality-<week>.md`; every wrong statement logged |

### 5.10 Desktop shell and IPC (5 to 10)

10 means: built, installed and used on the host; its 23 tests pass there; the capability list is explicit and minimal; a scripted run exercises the real window; no shell crash in 12 weeks.

| ID | Who | Step | Done when |
|---|---|---|---|
| SHELL-1 | H | Section 4.1 steps 1 to 3 | `cargo check -p g9-mentor-desktop` succeeds |
| SHELL-2 | H | Section 4.1 step 4, or `scripts/ci_preflight.sh --only desktop-shell` | "cargo check" and "shell tests" PASS, not SKIPPED |
| SHELL-3 | H | Section 4.1 step 5 | The window shows Mission Control |
| SHELL-4 | H | Section 4.1 step 6: `docs/runbooks/first-run.md` steps 1 to 5, then the checklist of `docs/audit/security-audit-2026-11-13.md` section 4 (a link in a note does not navigate; the dialog names the record and waits; an erased note's file is gone) | Each line ticked with a screenshot; the three NOT RUN fixes of that audit become TESTED |
| SHELL-5 | H, O | ADR-0020 row 7: set `"permissions": []` in `apps/desktop/src-tauri/capabilities/default.json`, rebuild, repeat SHELL-4. If the window fails, add named permissions back one at a time. Then you edit the documents that name `core:default` (`ARCHITECTURE.md`, `SECURITY.md`, the threat model, the RTM) | The final list is committed; in a development build a core plugin call from the inspector console is refused |
| SHELL-6 | A, H | Port `scripts/e2e/personal.cjs` and `scripts/e2e/tour.cjs` to drive the built application through WebDriver (ASSUMED: `tauri-driver` with WebKitWebDriver; confirm for Tauri 2.12) | The scenario passes in the real window on the host |
| SHELL-7 | A | A shell test asserts one command in the handler (`g9_command`) and no plugin crate in `apps/desktop/src-tauri/Cargo.toml` (confirm first that no such test exists) | A second command or a plugin fails the test |
| SHELL-8 | X | Engagement X1 on the installed application: CSP in effect, navigation handler, capability list, dialog | Review log entry |
| SHELL-9 | T | Daily use | No crash or hang in 12 weeks, or each one diagnosed from OBS-4 and fixed |

### 5.11 CLI and tooling (8 to 10)

10 means: no mutating command can reach the wrong data by default; the gates are themselves tested; you have driven every runbook command on the host.

| ID | Who | Step | Done when |
|---|---|---|---|
| CLI-1 | O, A | INFO item: the CLI default data directory (`g9-mentor`) differs from the application's (`com.g9.mentor`). Recommended: mutating commands require `--data-dir`; read-only `g9 status` defaults to the application's directory | Test: a mutating command without `--data-dir` exits 1 and names the flag |
| CLI-2 | A | Tests for the gates: `scripts/tests/` holds one deliberately broken input per check of `build_curriculum.py`, `check_curriculum_seed.py`, `check_skill_packages.py`, `check_competency_packs.py`, `check_schemas.py` and `check_source_register.py`; `python3 -I -m unittest discover scripts/tests` joins the content-gates job of the preflight | A gate that accepts its broken fixture fails the job |
| CLI-3 | A | Commands other areas add: `g9 verify-anchor` (STOR-3), `g9 doctor` (OBS-4), the 12-week fixture builder (PERF-1) | Each with tests and a line in `apps/cli/README.md` |
| CLI-4 | H | "The same steps from the command line" of the first-run runbook, the promotion and section 4.1, from the documents alone | Every stumble is a row for DOC-5 |
| CLI-5 | T | Weekly `g9 copy-backup`, monthly scratch restore | No wrong-directory incident |

### 5.12 UI/UX and design system (7.5 to 10)

10 means: every screen in every theme checked by machine and sampled by eye; no placeholder screen; seen in WebKitGTK; the research plan executed and its findings fixed.

| ID | Who | Step | Done when |
|---|---|---|---|
| UX-1 | A | Screenshot matrix from `scripts/e2e/tour.cjs`: 16 screens, 21 themes, light and dark (672 images), with assertions: no horizontal overflow, no clipped text, no contrast finding (A11Y-1). One contact sheet per theme | 0 assertion failures; you looked at the 21 contact sheets |
| UX-2 | O, A | Four screens have no operation of their own today: Executive Workspace and Research Center call none; Mentor Memory Explorer and Intervention Center call only `interventions.list`. By D7 and AI-11: build them, or take them out of the navigation until their operations exist | The tour finds no placeholder |
| UX-3 | H | The tour in the real window (SHELL-6, or by hand): fonts, colours and focus rings against the Chromium screenshots; record the WebKitGTK version | Differences listed, then fixed or accepted |
| UX-4 | H, T | Execute `docs/research/research-plan.md`: the first-use task run on day 1 (about 25 minutes), the two-week diary, the counts at the end of weeks 1 and 2, the analysis | `docs/research/research-synthesis-<date>.md`; its top findings fixed |
| UX-5 | T | Repeat the counts of the plan at weeks 6 and 12 | Compared in a short note; regressions fixed |

### 5.13 Frontend engineering (8 to 10)

10 means: a visual or contract regression cannot pass unnoticed; sizes are budgeted; a long session does not leak.

| ID | Who | Step | Done when |
|---|---|---|---|
| FE-1 | A | Visual regression: fixed clock and fixture; 16 screens in light and dark, default theme, compared with stored images | A deliberate 2 px shift fails |
| FE-2 | A | Contract parity between the mock and the backend: ARCH-2 | ARCH-2 |
| FE-3 | A | Bundle budget: record the current chunk sizes; the build fails when a chunk grows more than a margin over its recorded size without the record being updated (margin: your choice, 10% proposed) | A padded chunk fails `npm run build` in the preflight |
| FE-4 | A | TypeScript coverage (TEST-1) and an error boundary test per screen | Baseline recorded; ratchet on |
| FE-5 | A | Long session: 200 navigations in the e2e harness with heap samples | Measured first; the tolerance is then set from the measurement |
| FE-6 | A | The 7 moderate development advisories (`minimatch` through `typescript-eslint`) closed when upstream ships; a monthly `npm outdated` review | `npm audit` shows 0 at every level, or each remaining one is explained |
| FE-7 | H, T | vitest and the e2e on the host; in use, no open defect that blocks a task at week 12 | Evidence rows |

### 5.14 Security engineering (7.5 to 10)

10 means: the display-dependent fixes are confirmed; the threat model covers every boundary, the model runtime included; the parsers of untrusted input are fuzzed; the probes are a repeatable pack; an independent assessor finds nothing above LOW; advisories stay clean for 12 weeks.

| ID | Who | Step | Done when |
|---|---|---|---|
| SECUR-1 | H | SHELL-4 and SHELL-5 | Their evidence |
| SECUR-2 | A | Threat model: the full STRIDE table for boundaries B1 to B8 (PROPOSED in `docs/threat-model/threat-model.md` section 3), the model runtime boundary after D2, the OWASP agentic column (register row SEC-19) | No boundary without a row; each control names its test |
| SECUR-3 | A | Fuzz the parsers of untrusted input: curriculum manifest, skill package and frontmatter, backup manifest, IPC envelope, attachment sniffing; a generated corpus for the Markdown sanitizer. Tool ASSUMED: `cargo-fuzz`, which needs a nightly toolchain (D10, or on the host) | A time-boxed run per target with 0 crashes (the time box is your choice); every crash found becomes a regression test |
| SECUR-4 | A | Commit the audit probes as a pack under `scripts/security/` (export oracle, request-hash oracle, restore-plan quoting, file modes, planted schema, future revision, Markdown corpus) and add a preflight job | One command re-runs all of them; each fails on the code it was written against |
| SECUR-5 | A | Secret scan over the whole git history (D10; tool ASSUMED: gitleaks or equivalent) | Only the documented test fixtures |
| SECUR-6 | O, A | D6 (STOR-3) and D5 (PRIV-2) | Their evidence |
| SECUR-7 | X | Engagement X1 by someone who has not read the audit reports: the installed `.deb`, the data directory, the CLI, exports and backups; black box first, then with the threat model | Report under `docs/audit/`; every HIGH and MEDIUM fixed fail-first; every LOW decided |
| SECUR-8 | X | The assessor then reads the two audits and marks which of their findings were already known | The miss rate of the self-audit is a recorded number (PROC-5) |
| SECUR-9 | T | SUP-1 with a fresh advisory database at weeks 4, 8 and 12; the probe pack on every change | Three dated rows; no unreviewed advisory |

### 5.15 Privacy and data lifecycle (8 to 10)

10 means: disk encryption verified; copies that leave the machine are encrypted; an erasure is proven on raw bytes, also after a restore; every table is classified; an independent person has repeated the erasure probe.

| ID | Who | Step | Done when |
|---|---|---|---|
| PRIV-1 | H | Verify the assumption of `docs/privacy/data-lifecycle.md` (section 4.1 step 1): the filesystem that holds `~/.local/share/com.g9.mentor` sits on a `crypt` device | Recorded in the host inventory |
| PRIV-2 | O, A | D5. Recommended: encrypted backup sets (IMP-M08) for `g9 copy-backup`, the key id in the manifest; SQLCipher (IMP-M06) only if the disk is shared | Fail-first: a marker note is not found in the bytes of the encrypted copy; a restore from it works |
| PRIV-3 | A | Raw-byte erasure probe: after an approved erasure, search `g9.sqlite`, its `-wal` and `-shm`, later snapshots, `attachments/`, `exports/`, `restore-log.jsonl` and the logs of OBS-1 for the erased text and for its SHA-256, as bytes | 0 hits; snapshots taken before the erasure are listed to you by id |
| PRIV-4 | A | Backup retention (proposed in the runbook: 14 daily, 8 weekly, 12 monthly; your choice) so that old snapshots holding erased content leave on schedule | Test with a fixed clock |
| PRIV-5 | A | A restore must not bring erased content back silently (PROPOSED in the runbook): after the swap, replay the erasures recorded after the snapshot from the quarantined database, or refuse to finish without an explicit acknowledgement. ADR first | Fail-first: erase, restore an older snapshot; the content is absent again, or the command names what returned |
| PRIV-6 | A | `deletion.study_session.not_implemented`: implement or close by ADR (APP-3) | APP-3 gate |
| PRIV-7 | A | Data inventory: each of the 81 tables with domain, classification, retention, erasable and exported, in `docs/data-model.md`; a test fails when a migration adds a table without a row | Test passes with 81 rows |
| PRIV-8 | A | Sign the export manifest with the local key (PROPOSED in the data-lifecycle document) | A changed export file fails verification |
| PRIV-9 | H | With the mentor on: no socket other than the transport decided in D2 (`ss -tunap`, filtered on the process) | Output recorded |
| PRIV-10 | X, T | Engagement X1 repeats PRIV-3 on the installed application; one real erasure on real data during the run with the probe clean | Review log; evidence row |

### 5.16 Reliability and recovery (7 to 10)

10 means: restore proven on the host and repeated monthly; every crash point tested; a 12-week load simulated; no data loss in real use.

| ID | Who | Step | Done when |
|---|---|---|---|
| REL-1 | H | Restore drill: section 4.1 steps 7 to 9 | Step 8 prints `audit_chain_ok: true` and the notes and sessions of the backup are there; step 9 leaves the old database in `quarantine/` and the window opens |
| REL-2 | A | STOR-1, STOR-2, STOR-4 | Their evidence |
| REL-3 | A | Soak: 12 simulated weeks in one scratch directory (extend `g9 simulate-week --scratch` or add a program driver), then 5,000 more operations | Invariants hold; the WAL stays bounded; duration and database size recorded |
| REL-4 | O, A | The recovery point the runbook proposes: a backup request after the first session of each day and before every migration, as a prompt in Mission Control or automatic (your choice) | Test with a fixed clock; the prompt appears in the e2e |
| REL-5 | A | Upgrade path: a stored database at each schema version, with data, opens under the current build with a verified pre-migration backup. Confirm what the `a08_*` tests already cover; add the missing versions | One fixture per version passes |
| REL-6 | H | Kill the window during a session (`kill -9`) and restart; pull the USB drive during `g9 copy-backup` | The recovery report is sane; a partial copy never has the final name |
| REL-7 | T | STOR-7 | No data loss in 12 weeks |

### 5.17 Testing and verification (8 to 10)

10 means: coverage measured and ratcheted on the authority-critical code; surviving mutants explained; parsers fuzzed; no flaky test; every A gate executed; tests written by someone else agree with mine.

| ID | Who | Step | Done when |
|---|---|---|---|
| TEST-1 | A | D10. Measure coverage: Rust with `cargo llvm-cov` on the default members, TypeScript with vitest's V8 coverage; record `docs/release/coverage-<date>.md`. Ratchet: the preflight fails when line coverage of `g9-policy`, `crates/g9-app/src/pipeline.rs`, the migration, backup, restore and audit modules of `g9-storage`, or `g9-curriculum` falls below the recorded value. No target is set before the measurement | Baseline file; a deleted test fails the ratchet |
| TEST-2 | A | D10. Mutation testing (tool ASSUMED: `cargo-mutants`) on `g9-policy`, the pipeline, the reconciliation of `g9-curriculum`, the finalization of `g9-assessment`, `g9-retention` | Every surviving mutant has a new test or a written reason why it is equivalent |
| TEST-3 | A | Fuzzing: SECUR-3. Property tests: POL-1, LEARN-5, CAL-2 | Their evidence |
| TEST-4 | A | Flake check: the Rust and vitest suites 20 times, default threads and one thread; the e2e 5 times (the counts are a choice) | 0 differing results |
| TEST-5 | A | Gates: A02 into the harness (CAL-2), the runtime scanner into the e2e (A11Y-1), then `scripts/collect_gate_evidence.py` | `docs/release/gate-evidence.md` shows no NOT RUN among A01 to A12 except what only the host runs, each named |
| TEST-6 | H | `scripts/ci_preflight.sh` with all five jobs; the e2e in the real window (SHELL-6) | No FAIL, no SKIPPED |
| TEST-7 | X | Engagement X2 writes at least 20 tests from `ARCHITECTURE.md`, the RTM and the runbooks without reading the existing tests | Each failure is filed as a defect or a document error and closed |
| TEST-8 | T | For every defect found in use, name the test that should have caught it and add it | Trend at weeks 4, 8, 12 |

### 5.18 Documentation and governance (8.5 to 10)

10 means: one backlog; references and counts in documents are checked by machine; `ARCHITECTURE.md` agrees with the build; someone has executed the runbooks cold.

| ID | Who | Step | Done when |
|---|---|---|---|
| DOC-1 | A | One backlog: every OPEN and PARTIAL row of `docs/audit/defect-register.md` and every PROPOSED, PARTIAL or NOT RUN row of `docs/requirements/rtm.md` gets a step id of this file or a "will not do" ADR; `scripts/check_backlog.py` fails on a row with neither | The script is in the content-gates job; 0 orphans |
| DOC-2 | A | Drift gates: `scripts/check_doc_refs.py` (every path and `file::test` named in `docs/`, the ADRs and the READMEs exists); counts are generated into `docs/release/metrics.md` and other documents link to it instead of repeating numbers | A renamed test fails the gate |
| DOC-3 | O | `ARCHITECTURE.md`, yours to edit: section 11 (15 minutes against the 10 in use); section 2 ("reviewed in CI": name ARCH-1); section 3 row M01 after SHELL-5; section 14 after TEST-5 | Edits committed by you |
| DOC-4 | A | Runbooks for what this plan adds: host bring-up (from section 4.1), release and upgrade, model install, diagnostics, error codes (OBS-5) | Files under `docs/runbooks/` |
| DOC-5 | H, X | Cold read: you execute first-run, backup-restore and promotion from the documents alone; a colleague repeats it on another machine (X2) | Every stumble fixed in the document |
| DOC-6 | T | The gates stay green; the stage report gets a dated update at weeks 4, 8 and 12 | Three updates |

### 5.19 Build, release and packaging (4 to 10)

10 means: a signed, checksummed package built by one script on the host, proven on a clean machine, rebuilt reproducibly, upgraded at least twice without data loss.

| ID | Who | Step | Done when |
|---|---|---|---|
| BUILD-1 | H | SHELL-1 to SHELL-3: the first `.deb` | Installed and running |
| BUILD-2 | A | `scripts/release.sh`, run by you on the host, never by CI: preflight on a clean export, `npm run tauri:build`, `SHA256SUMS`, the two SBOMs of the preflight's sbom job, a `LICENSES.md` check, a detached signature of `SHA256SUMS` with your key (D9), release notes from `CHANGELOG.md` | One command produces the release directory; the signature verifies |
| BUILD-3 | A | One version source: a test fails when the workspace version, the shell crate and `apps/desktop/package.json` disagree | Test passes |
| BUILD-4 | H | Reproducibility: build the same commit twice in clean directories; compare the SHA-256 of the binary and of the package | Equal, or the cause recorded and removed |
| BUILD-5 | H | Clean machine: a fresh VM of the host's OS; install the package; first-run steps 1 to 5; inspect `dpkg -c` for setuid files, services and policy files (RTM R-018) | Works; the package installs nothing privileged |
| BUILD-6 | A, H | Upgrade runbook and test: backup, verify, install the next version, read the `g9 init` report; rollback is a restore (no downgrade, correction C12) | An upgrade with data passes on the VM |
| BUILD-7 | O | D9 by ADR: no updater (it needs the network); `bundle.targets` stays `deb` (register row PLT-12) | ADR accepted |
| BUILD-8 | X | Engagement X2 installs from the release directory and verifies the signature from the runbook alone | Review log entry |
| BUILD-9 | T | At least two releases installed over real data during the run | No data loss; both in the evidence register |

### 5.20 Dependency and supply chain (6.5 to 10)

10 means: advisory, licence, ban and source checks clean against a current database; versions pinned by decision; an SBOM per release; three monthly review cycles done.

| ID | Who | Step | Done when |
|---|---|---|---|
| SUP-1 | A or H | D10. `cargo deny --all-features --locked check` with a current advisory database: advisories, bans, licences, sources. The last recorded scan is offline, database of 2026-11-06 | 0 errors; the one `ignore` entry of `deny.toml` re-reviewed |
| SUP-2 | A | `npm audit` at every level (the preflight gates at high); list the packages of `package-lock.json` that run install scripts and record an allow-list | Report committed |
| SUP-3 | O | Workspace dependencies use caret requirements and the lock file fixes them (audit INFO item; for example `proptest = "1"`), while `CLAUDE.md` asks for exact versions. Decide: exact pins in the root `Cargo.toml` (your edit), or the lock file as the pin | Decision recorded in `docs/release/dependency-policy.md` |
| SUP-4 | A | An SBOM per release (BUILD-2) and a diff against the previous release | New and changed packages listed in the release notes |
| SUP-5 | H | A `cargo vendor` archive stored with each release, so it rebuilds without the network | A rebuild with `--offline` from the archive succeeds |
| SUP-6 | O, A | After D2: the model weights, the runtime and its build flags are supply chain too: publisher, source, licence class, checksum and version in `model_registry` | Rows complete; a load with a wrong checksum is refused |
| SUP-7 | X | Engagement X1 samples 10 direct dependencies: maintainers, release history, unsafe code | Notes in the review log |
| SUP-8 | T | Monthly: a fresh advisory database, a `cargo update --dry-run` review, the Tauri release notes | Three dated rows |

### 5.21 Performance (6 to 10)

10 means: the budgets of `docs/ui/performance-budgets.md` are measured by its method on the host, met, and still met on the real database at week 12.

| ID | Who | Step | Done when |
|---|---|---|---|
| PERF-1 | A | A fixture at the sizes the budget document requires: 72 days, 216 objectives, about 1,700 sessions, 5,000 intervals, 2,000 notes with 3 revisions, 500 attempts, 20,000 observations, 50,000 audit rows; fixed checksum | Deterministic: two runs, the same checksum |
| PERF-2 | A | Latency harness at the IPC boundary for `session.list`, `calendar.list` (one week), `retention.due`, `approvals.list`, `audit.recent` (limit 50) and the five most used writes: 200 samples after 10 warm-ups; p50, p95 and max as JSON. No bench target exists today | Runs in the sandbox; numbers measured there are not a verdict (other hardware) |
| PERF-3 | A | Query-plan gate: `EXPLAIN QUERY PLAN` for every repository query on the fixture; fail on a scan of a large table without an index unless allow-listed with a reason | Gate in `cargo test` |
| PERF-4 | A | The instrumentation the budget document calls PROPOSED: `performance.mark` and `measure` from input to paint; the backend's own duration per command recorded as metadata (`crates/g9-app/src/pipeline.rs` records none today); a `curriculum.week` read model (the Navigator issues six `curriculum.daily_plan` calls per week) | Marks exported through OBS-4; one call per week view |
| PERF-5 | H | Execute the measurement method: cold and warm, three repetitions, host state recorded | `docs/release/performance-<date>.md` against the provisional budgets: 150 ms interaction, 100 ms indexed query, 3 s cold start |
| PERF-6 | A | Fix whatever exceeds its budget; re-measure on the host | All within budget, or the budget changed by your decision |
| PERF-7 | T | Re-measure at weeks 6 and 12 on the real database; record database size and backup duration | Within budget; trend recorded |

### 5.22 Accessibility and localisation (6.5 to 10)

10 means: a runtime scanner on every screen; the core flows proven with the keyboard alone; a WCAG 2.2 AA audit with evidence per criterion; a screen-reader pass on the host, repeated by a person who relies on assistive technology; the language scope decided.

| ID | Who | Step | Done when |
|---|---|---|---|
| A11Y-1 | A | D10. A runtime scanner in the e2e (gate A12 and RTM R-015, both NOT RUN): scan the 16 screens in light, dark and the high-contrast setting inside `scripts/e2e/tour.cjs` (ASSUMED: `@axe-core/playwright`) | 0 serious or critical findings; moderate ones reviewed |
| A11Y-2 | A | Keyboard-only e2e of five flows: first run, plan a day, run a session, write a note and attach a file, approve. Assert a visible focus indicator at each stop | Passes without a pointer event |
| A11Y-3 | A | Audit against WCAG 2.2 AA in `docs/ui/a11y-audit-<date>.md`: reflow at 320 CSS px and at 400% zoom, text spacing, target size, focus not obscured, reduced motion; each criterion pass, fail or not applicable, with its evidence | No failing criterion left open |
| A11Y-4 | H | Orca on the host: read Mission Control, plan the day, start and pause a session, approve in the native dialog, read a lesson sheet | Each task completed by speech alone, or the defect fixed |
| A11Y-5 | X | Engagement X4: a daily user of assistive technology, or an accessibility auditor, repeats A11Y-4. Without such a person this area stops at 9 | Their notes; findings fixed |
| A11Y-6 | O | D8: "English only" by ADR, or i18n-ready (strings externalised, dates and numbers through `Intl`, a pseudo-locale e2e with longer strings) | The ADR, or the pseudo-locale run without clipping |
| A11Y-7 | T | The scanner gate stays green; once a month you work one day with the keyboard alone | Three notes |

### 5.23 Observability and diagnostics (6 to 10)

10 means: a redacted log exists; one bundle is enough to diagnose a fault; every error code has an operator action; someone has diagnosed seeded faults from the bundle alone.

| ID | Who | Step | Done when |
|---|---|---|---|
| OBS-1 | A | A log sink (none exists, section 1): a size-capped rotating file under `<data>/logs/` (mode 0600), JSON lines with time, level, operation, request id, error code, duration; never a payload, a title or a path. New dependencies with exact pins that pass `cargo deny`, or a small writer of our own | Probe: notes, titles and file names seeded with markers never appear under `<data>/logs/` |
| OBS-2 | A | The request id of `CommandMeta` in the log line and in the audit row | One id finds both |
| OBS-3 | A | A panic hook writes a crash record (message, location, version; no payload) under `<data>/logs/`; the UI error boundary shows the code and a "copy details" action | A forced panic in a test build leaves the record |
| OBS-4 | A | `g9 doctor` and a `diagnostics.bundle` operation (MAINTENANCE mode, approval-gated): versions, schema version, the PRAGMA profile, integrity results, the audit head, the last 200 log lines, the last 50 audit metadata rows, the marks of PERF-4 | The probe of PRIV-3 finds no content in a bundle |
| OBS-5 | A | Error-code catalogue: every code in the source with category, meaning and operator action in `docs/runbooks/error-codes.md`, generated and tested for completeness; `docs/ui/ux-copy.md` is checked against it | A new code without a row fails the test |
| OBS-6 | A | Settings shows p50 and p95 per operation for the last 7 days from the durations of PERF-4 (local only) | Visible in the tour |
| OBS-7 | X | Blind diagnosis (engagement X2): one person seeds five faults in a scratch install (a changed audit row, a missing attachment, a replaced `binding.key`, a full disk, a held lock); another names each cause from the bundle alone | 5 of 5 |
| OBS-8 | T | Every incident of the run is diagnosed from the bundle | Gaps become new fields |

### 5.24 Build process and provenance (7 to 10)

10 means: the highest-risk code has been read by a person; changes to it need a second key; releases are signed by you; the miss rate of the self-audit is known.

| ID | Who | Step | Done when |
|---|---|---|---|
| PROC-1 | O | Create `docs/release/review-log.md`: reviewer, date, commit, scope, findings, sign-off | First entry |
| PROC-2 | X | Engagement X2 reads the five highest-risk units: `crates/g9-policy/src/binding.rs`; `crates/g9-app/src/pipeline.rs`; the migration, backup, restore and audit code of `g9-storage`; `apps/desktop/src-tauri/src/lib.rs`; `crates/g9-curriculum/src/checksum.rs` and `reconcile.rs` | Five signed entries |
| PROC-3 | O | Two-key rule from now on: a change to those units is accepted only after a review by someone other than the session that wrote it. A second agent session that gets the diff and the specification, but not the author's reasoning, is the weakest acceptable form and supports 8.5 only | Rule added to `GOVERNANCE.md` by you |
| PROC-4 | O | Release tags signed with your key | `git tag -v` verifies |
| PROC-5 | X | SECUR-8 | The number is in the review log; the classes that were missed become checklist items |
| PROC-6 | O, T | Soak rule: a release is called stable after a period of your use without a severity-1 defect (the period is your choice) | Recorded per release |
| PROC-7 | A | Task list: item 26 is phase 0. Item 28 (triage of the review findings of item 27) looks stale: confirm against `docs/audit/defect-register.md` that its findings were closed under items 37 to 44 and 63 to 68, then close it or re-scope it | Done at the start of the next session |

### 5.25 Personal-use readiness (6.5 to 10)

10 means: every area above at 10 and the run finished.

| ID | Who | Step | Done when |
|---|---|---|---|
| READY-1 | H | Phase 0 | Installed, first run done, REL-1 done |
| READY-2 | O | D3 and CONT-5 before the first Saturday | 12 definitions |
| READY-3 | T | Week 1 without the agent's help: six days planned, timed and evidenced; the day-1 task run of UX-4 | Evidence rows |
| READY-4 | H | CUR-4 | "Verified" in the window |
| READY-5 | H | The mentor on (AI-9 passed) no later than week 8 | ADR "model selected" |
| READY-6 | T | 12 weeks done; the capstone graded twice (LEARN-7); `docs/release/retro-12-weeks.md`: defects, credited against planned hours (480), changes | File committed |
| READY-7 | O, X | Re-rating against the "10 means" lines of this file by you and the reviewers; the agent prepares the evidence table | New scorecard |

## 6. The four reviewer engagements (D1)

| Engagement | Reviewer | Steps | When | Size (ASSUMED) | Areas |
|---|---|---|---|---|---|
| X1 Security | a security practitioner who has not read `docs/audit/security-audit-*.md` | SECUR-7, SECUR-8, POL-4, STOR-6, SHELL-8, PRIV-10, SUP-7 | after phases 0 and 1 | 3 to 5 days | 2, 3, 10, 14, 15, 20 |
| X2 Engineering | an engineer who reads Rust | ARCH-5, CUR-5, TEST-7, DOC-5, BUILD-8, OBS-7, PROC-2 | after phase 1 | 3 to 5 days | 1, 4, 5, 17, 18, 19, 23, 24 |
| X3 Learning and AI | a second practitioner | CONT-9, CONT-5 (option a), LEARN-7, the answers of AI-3, AI-12 | CONT-5 before week 1; the rest during the run | 2 to 3 days, plus grading | 6, 7, 9 |
| X4 Accessibility | a daily user of assistive technology or an auditor | A11Y-5 | after A11Y-1 to A11Y-4 | 1 day | 22 |

## 7. Not worth doing, unless you want the literal 10

| Item | Why | What the plan does instead |
|---|---|---|
| Translation of the interface | one English-speaking operator | D8: scope by ADR |
| Windows and macOS packages | the target is one Linux host; `deny.toml` checks one target | none; register row PLT-12 |
| An automatic updater | it needs the network, which the runtime forbids | BUILD-2, BUILD-6 |
| SQLCipher on a single-user encrypted disk | a second key to lose for no added protection | PRIV-1, PRIV-2 |
| FSRS | the fixed ladder may be enough; the evaluation takes 12 weeks | LEARN-9 decides from data |
| Restore from inside the window | the offline restore is tested and safer | REL-1 |
| `executive.*` and `research.*` before the run | not on the learning path | D7 |
| A throughput-class model runtime | the benchmark protocol puts it out of scope without a measured need | AI-2 |

## 8. Evidence register

`docs/release/path-to-10-evidence.md`, one row per finished step:

| Date | Step | Who | Commit | Command or action | Result | Evidence |
|---|---|---|---|---|---|---|
| (example) 2026-11-17 | SHELL-2 | owner | `<sha>` | `scripts/ci_preflight.sh --only desktop-shell` | PASS: cargo check, shell tests | log path or screenshot |

## 9. What an agent session can start at once

No decision and no new tool is needed for: PROC-7, ARCH-1, POL-2, STOR-2, STOR-4, APP-2, CLI-2, DOC-1, DOC-2, OBS-1, OBS-2, OBS-5, PERF-1, PERF-2, PERF-3, PRIV-3, PRIV-7, REL-5, BUILD-3, TEST-4.
With D10: TEST-1, TEST-2, SUP-1, SECUR-3, SECUR-5, A11Y-1.
With the hosts of CONT-1 allowed: CONT-2.
