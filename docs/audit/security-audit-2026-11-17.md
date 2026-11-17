# G9 MENTOR — final security review (2026-11-17)

Labels: VERIFIED (checked by running it or reading the exact code), TESTED (a test in the repository proves it), INFERRED, ASSUMED, PROPOSED, NOT RUN.

Scope: every code, schema, script and workflow file changed between `2e0b344` (the fixes of the 2026-11-13 audit) and `324161a` (130 files: the operations log, the gates, backup attachment hashing, disk-full handling, the upgrade path, the performance tooling and diagnostics, the payload caps, and the fixes of the 2026-11-10 review leads), following the data flow into unchanged code where a changed file reaches it. The threat model is `docs/threat-model/threat-model.md`. Findings recorded as fixed or accepted by `docs/audit/security-audit-2026-11-12.md` and `docs/audit/security-audit-2026-11-13.md` were not re-reported unless they had regressed (none had).

## Remediation status

| Finding | Severity | Status | Change | Proof |
|---|---|---|---|---|
| ipc-2 / privacy-1: a failed backup verification stored the content hash (storage ref) of each missing attachment file in `backup_manifests.verification_json`; an erasure left it, and a SYSTEM export carried an erased SECRET file's SHA-256 out | LOW (one reviewer rated it MEDIUM; the verifier LOW: it needs a missing backup copy first) | Fixed | `AttachmentCheck::Checked.missing` is a count, like `damaged`; a stored list of refs reads as its length; every attachment erasure and every open rewrite stored lists to their count (`backup::count_listed_missing_files`) | `crates/g9-app/tests/erasure_digests.rs::a_backup_report_that_found_a_file_missing_keeps_no_digest_of_it_after_the_erasure`, `::an_erasure_rewrites_a_report_stored_before_missing_files_were_counted`, `::a_report_stored_before_missing_files_were_counted_is_rewritten_at_open` (all failed before; the open rewrite was also mutation-checked); `crates/g9-storage/src/backup.rs::a_report_stored_with_a_list_of_missing_refs_reads_as_its_count` |
| authz-1: the 30-day idempotency purge used the wall clock alone; an open with the clock 31 days ahead purged fresh records, and a retry with the same key then ran the command again | LOW | Fixed | `idempotency::purge_cutoff`: 30 days before the earlier of the clock and the newest stored record. Trade-off, documented: a record can be kept up to 30 days past the last recorded command | `crates/g9-app/tests/idempotency_retention.rs::a_forward_clock_step_at_open_does_not_purge_recent_records`, `::retention_counts_from_the_newest_recorded_command` (both failed before) |
| authz-2: a skill version row was registered from whatever registry was loaded; an edited package run once through the unpinned CLI became the immutable `<name>@<version>` row and the pinned package was refused for good | LOW | Fixed | `Registry::is_pinned` (set only by `load_pinned`); an unpinned package registers as `<name>@<version>+<12 hex of its package hash>` (version label with SemVer build metadata), so it never occupies the pinned package's id | `crates/g9-app/tests/skills_invoke_records.rs::an_unpinned_first_registration_does_not_block_the_pinned_package` (the reviewer's probe showed `skills.version_hash_mismatch` before); the version-bump refusal test now uses pinned registries |
| supply-1: the layering gate recorded a dependency under its TOML key, so `x = { package = "g9-ai-adapter" }` or `[ dependencies ]` hid a forbidden edge (and a direct `rusqlite` in the CLI) | LOW | Fixed | `parse_manifest` trims header whitespace and records the `package =` name of inline and `[dependencies.x]` tables | `tests/integration/layering_gate.rs::layering_parser_sees_renamed_dependencies_and_spaced_headers` (failed before) |
| privacy-2: `calendar.delete_event` removed the title from every table but left it in the live `-wal` file while the process ran | LOW (verifier: INFO; no document claimed byte-level removal) | Fixed | `Mutation::truncating_wal`: the pipeline runs a TRUNCATE checkpoint after the commit, as the erasure path does | `apps/cli/tests/erasure_probe.rs::a_deleted_calendar_title_is_not_left_in_the_live_wal` (failed before) |
| ipc-1: the shell parses the whole IPC body into `serde_json::Value` (and copies each argument) before dispatch's byte cap runs, so the cap does not bound backend memory; a 300 MB body from a compromised webview can exhaust memory | LOW | **Not fixed (PROPOSED)** | Needs the UI transport to send raw bytes and `g9_command` to take `tauri::ipc::Request` and check the length before parsing. Neither the shell nor WebKitGTK can be run in this environment, so the change could not be tested end to end; a broken shell transport is a worse production risk than this LOW. Committed data survives (each command is one IMMEDIATE transaction, `synchronous=FULL`); an interrupted erasure is reported at the next open (`deletion.erasure_incomplete`) | reviewer's probe (24 MB body → 562 MB RSS after parse); owner action below |
| authz-3: `creation_id` reused an approval's resource id for any create of the same kind that presented it, so a plain create with other content took the approved id | INFO | Fixed | Reuse only for a PENDING or APPROVED request whose keyed parameters digest equals this payload's (constant-time comparison) | `crates/g9-app/tests/approval_gated_create.rs::an_approval_presented_on_other_content_does_not_take_the_approved_id` (failed before) |
| ipc-3: the `json_tree` self-join that finds stored results quoting a deleted event was quadratic (134 s for one result repeating the id 20,000 times; reachable through a restored snapshot) | INFO | Fixed | One `json_tree` pass, the sibling key read by path, and an `instr` pre-filter | `crates/g9-storage/src/idempotency.rs::results_quoting_is_linear_in_the_size_of_a_stored_outcome` (134 s before, milliseconds after); the structure test is unchanged and passes |
| privacy-3: `docs/data-model.md` still said the purge never ran and deleted calendar titles were kept; `docs/privacy/data-lifecycle.md` stated a restore-time deletion replay that does not exist | INFO | Fixed | Both documents corrected; the replay is marked PROPOSED (PRIV-5, gap G7) | `scripts/check_data_inventory.py`, `scripts/check_doc_refs.py` |
| supply-2: the ignored deny-target graph test was documented as run by the preflight and ran nowhere | INFO | Fixed | Run in the `rust` job of `scripts/ci_preflight.sh` and `.github/workflows/ci.yml` (still `workflow_dispatch` only; no workflow was started) | `cargo test -p g9-evaluation --test layering_gate -- --ignored`: 1 passed |

Hardening items fixed in the same round (each with a test that failed first):

| Item | Change | Test |
|---|---|---|
| `approvals.decide` raised the native dialog before the meta, cap, schema and name checks, so the operator could be asked about a request that was then refused | the boundary checks run before the dialog | `crates/g9-app/tests/native_confirmation.rs::a_request_the_boundary_refuses_is_never_shown_to_the_operator` (2 dialogs before, 0 after) |
| `attachment_store::copy_between` created destination shard directories before checking the source, and opened the source by path after an lstat check | destination and source are checked first; the source is opened with `O_NOFOLLOW | O_NONBLOCK` and its type checked on the handle | `crates/g9-storage/src/attachment_store.rs::a_copy_whose_source_is_missing_creates_nothing_at_the_destination` (failed before), `::a_copy_never_reads_a_source_that_is_not_a_regular_file` |
| The operations log's process-wide subscriber recorded INFO+ events of dependencies, which are outside the call-site register | only targets `g9_*` are recorded | `crates/g9-app/src/oplog.rs::events_of_other_crates_are_not_recorded` (failed before) |
| `storage.row_decode` messages quoted the column and sometimes the stored value | fixed sentence; the payload stays for tests | `crates/g9-storage/src/error.rs::a_row_that_cannot_be_decoded_is_reported_without_its_column_or_value` (failed before) |

## How the review was run

Five independent reviewers, one per lens (IPC boundary and resource use; authorization, approvals, idempotency and ledgers; keys and integrity; privacy, erasure and logs; supply chain, build and scripts). Each had to give a source-to-sink path with file and line, the attacker capability, a scenario and a severity, and could write temporary probe tests (deleted afterwards; none remained). For each lens a second agent tried to refute every finding at the commit. All ten findings were CONFIRMED; the keys-and-integrity lens reported none. The fixes were then made in the working tree, each fail-first, and verified as below.

## Checked and clean (from the reviewers' coverage lists)

- Keys and integrity: binding key provisioning (recreated-key warning, permissions, lock-before-provision order), zeroize-on-drop and no `Clone`, constant-time MAC comparison, keyed digests in replay records, audit chain rows of the new paths, attachment hashing in backup verification (the store is the snapshot's own directory), migration version handling and the test-only hook (absent from non-test builds).
- Operations log: allow-listed fields only, JSON-encoded lines, 0700/0600, rotation never follows links, a write failure never fails a command; the test-only helpers are not used in production.
- `statement-trace` stays out of release builds (resolver 2, dev-dependency feature); no run-time switch.
- Idempotency: a retry with the same key and a different request is refused by the keyed request hash; approval-gated commands cannot be re-run after a purge (their approval was consumed).
- Grade evidence and submission pre-checks, the READ_ONLY guard (all 16 read operations), `repos::atomic`.
- CI workflow: `workflow_dispatch` only, `contents: read`, actions pinned by commit, no untrusted input interpolated.

## Not fixed: hardening worth doing (PROPOSED, no exploit path today)

| Item | Where |
|---|---|
| IPC body length check before parsing (ipc-1) | `apps/desktop/src-tauri/src/lib.rs`, `apps/desktop/src/ipc/transports/` |
| A refusal that cannot be recorded (disk full) leaves an approval refused as expired on a backwards clock unpersisted | `crates/g9-app/src/pipeline.rs::record_refusal` |
| Bind approval state, expiry and decision time into the MAC under a version label (closes a backup-at-rest edit; core-1 residual) | `crates/g9-policy/src/binding.rs` |
| A restored snapshot is queried (`schema_migrations`) before its schema is compared; a recursive view could hang `g9 restore` | `crates/g9-storage/src/restore.rs` |
| `deletion.rs` ignores a checkpoint that a reader blocked; surface a warning | `crates/g9-storage/src/deletion.rs` |
| Keep `g9_storage::diagnostics` and the seed and perf subcommands behind cargo features so release builds do not carry them | `crates/g9-storage/Cargo.toml`, `apps/cli/src/cli.rs` |
| `simulate-week` and `crash-drill` accept any named `--data-dir` with `--scratch`; apply `perf-fixture`'s "refuse an existing database" check | `apps/cli/src/cli.rs` |
| A source gate for `std::net`, sockets and `std::process::Command` (the network gate is a crate denylist) | `crates/g9-domain/tests/source_text.rs` |
| Pin the SHA-256 of the committed upgrade fixtures; `--require-hashes` for the CI Python venv | `crates/g9-app/tests/upgrade_path.rs`, `.github/workflows/ci.yml` |
| `storage.cross_domain_read` names the table in `details` (a deliberate R-017 contract) | `crates/g9-storage/src/repos/mod.rs` |
| `restore-log.jsonl` records the snapshot path the operator chose; state its rule in the lifecycle document | `apps/cli/src/commands/restore.rs` |
| `scripts/ci_preflight.sh` deletes `--target` when it lies inside `--work`; `check_skill_packages.py` follows symlinks (the loader refuses them) | `scripts/` |

## Verification of the fixes

On `a2e1378` (the fixes) in this sandbox: the CI preflight on a clean export passed 20 steps with 0 failures and 5 SKIPPED (no cargo-deny, cargo-cyclonedx or WebKit; `--offline`), with the Rust suite at 1152 passed, 0 failed, 1 ignored; fmt, clippy and the shell clippy clean; vitest 487 passed; smoke 100 checks pass; `personal.cjs` 29/29; the 16-screen tour without a console, page or CSP problem; the flake check with 0 failures and 0 differing outcomes. Commands and labels: `docs/release/stage-report.md`, "Update 2026-11-17". NOT RUN: the shell on a display, `cargo deny`.

## Owner action

1. ipc-1: decide whether to change the shell's IPC transport to raw bytes (it must be tested on the G9 host with the shell running).
2. Run the shell on the G9 host (task 26): the native dialog, the window and WebKitGTK were not run in this review.
