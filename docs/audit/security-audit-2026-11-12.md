# G9 MENTOR — triple security audit (2026-11-12 to 2026-11-13)

## Remediation status (2026-11-13)

| Finding | Severity | Status | Change | Proof |
|---|---|---|---|---|
| Backup snapshot with planted schema objects passes verification and restore | MEDIUM | Fixed | `migrations::verify_schema` (SQL text compared exactly; only `sqlite_stat*` exempt); checked in `backup::verify_connection` (verify, restore copy) and at open (`recovery`: `schema_drift` blocks writes); restore plan no longer prints a mismatching hash; `table_counts` quotes names | `crates/g9-storage/tests/schema_integrity.rs` (9; 5 tamper cases failed before the fix, 2 more before the exact-text correction below), `crates/g9-app/tests/schema_drift.rs` (2; the tamper case failed before); auditors' probes re-run after the fix |
| Approval bound to a future revision (approve_final) | MEDIUM | Fixed | `pipeline.rs`: bind to the stored revision; refuse a stale approval request with STALE_REVISION | `crates/g9-app/tests/approval_binding.rs` (4; both exploit cases failed before); the auditor's probe re-run after the fix |
| Release build path produces a Tauri dev binary | LOW | Fixed | `src-tauri/build.rs` refuses a release build when `tauri_build::is_dev()`; README builds with `npm run tauri:build` | Read against the pinned tauri-build 2.7.4 and tauri 2.12.5 sources (the shell cannot compile in this sandbox; CI `desktop-shell` compiles it in debug, where the guard is inactive) |
| Audit tail truncation not detected | LOW | Open | — | needs anchors checked at open |
| Client-declared interaction mode | LOW | Open | — | needs a backend-derived mode for every operation |
| `g9 --human` prints control characters | LOW | Open | — | escape C0/C1/bidi in `output.rs` |
| Skill pin hashed from a second read | LOW | Open | — | read once, hash and parse the same bytes |
| Revision 0 refusal leaves no audit row; SQLite text in errors | LOW | Open | — | validate `Revision` at the boundary |
| INFO items | INFO | Open | — | see section 2 |

Found while verifying the fix (2026-11-13): the first version of `verify_schema` compared SQL with whitespace collapsed. A newline ends a `--` comment, and 66 tables carry such comments in their stored SQL; replacing the newline after the comment above a CHECK constraint removed the constraint while the collapsed text stayed identical. Seven variants on four tables (`competency_observations`, `decisions`, `skill_invocations`, `source_registry`) still opened with `integrity_check` ok and no foreign-key errors while their whitespace-collapsed text stayed identical, so the first version reported no difference (no other check reads `sqlite_master` text). Shown for `decisions` by the fail-first test: the first version's `verify_snapshot` passed the tampered file while it held a DECIDED decision with no operator, a row the genuine schema refuses. The other six variants are inferred from the same evidence, not run through verification. After the correction, a confirmed CLI restore of such a file is refused (`storage.backup_verification_failed`, live file unchanged) and the same tamper on the live file blocks writes at open (`schema_drift`). Because `integrity_check` validates CHECK constraints, the bypass also let a snapshot carry constraint-violating fabricated rows past every check. No trigger, view or index SQL contains a comment, so no code could be planted this way. Corrected before release: exact text, and the `sqlite_sequence` and constraint-autoindex exemptions removed (both are deterministic); regression tests `a_line_comment_stretched_over_a_check_constraint_fails_verification` and `only_planner_statistics_are_exempt_from_the_comparison`, both failing before the correction. Exact text makes any change of stored SQL break existing databases (`schema_drift`), so the expected schema at version 13 is pinned by hash and such a change fails CI instead; the hash was reproduced independently with SQLite 3.53.1 against the bundled 3.54.1 (452 objects, identical).

Every probe of the audit was kept outside the repository (session scratch space), with scratch data directories; none is needed to reproduce the fixes, whose regression tests are listed above.

Revision audited: `3fcf7ec` (branch `ccr-0cea681c-p9jbru`). Read-only: nothing in the repository was changed by the audit. Every probe ran outside the repository (scratchpad `secaudit/`), on scratch data directories.

## How the audit was run

| Pass | Who | Method |
|---|---|---|
| A | Two independent auditors with no access to each other's results or to the lead auditor's: A1 core boundary (g9-app, g9-policy, g9-domain, g9-storage), A2 edges and perimeter (Tauri shell, CLI, skills, AI adapter, evidence, curriculum import, UI rendering, supply chain, CI) | Code reading in full of their scope, executable probes (Rust probe crates, CLI drills, a 28-payload sanitizer probe) |
| B | Lead auditor | Attack-surface map, whole-repository sink sweeps, supply chain and CI review, probes of the unverified leads from the 2026-11-10 code review |
| C | Lead auditor | Adversarial verification: every candidate from A, B and the earlier review was re-read in the code, and every MEDIUM was reproduced by re-running the probe or by an independent CLI reproduction |

Severity follows the security-review-method rubric, calibrated for a single-operator offline desktop app: the realistic attackers are hostile content (files, notes, packages, model output), a compromised webview, a tampered file at rest, and supply chain. A same-user local process is out of scope except where a documented control claims to hold against it.

## 1. Attack surface map

- Entry points: one Tauri command `g9_command(op, meta, payload)` (apps/desktop/src-tauri/src/lib.rs:539), local origin only (capabilities/default.json: `core:default`, no plugins; remote origins are refused by Tauri's ACL); CLI `g9` (call, approve/reject, import-seed, restore, simulate-week and crash-drill behind `--scratch`); files read from disk (skill packages bundled as resources, curriculum manifests, backup snapshots handed to restore).
- Pipeline (g9-app/src/pipeline.rs): the backend assigns the actor (`ipc::parse_meta` refuses an `actor` key); one IMMEDIATE transaction runs idempotency, facts, policy, approval consumption, revision check, the handler, events, policy decision and audit; refusals are recorded in a second transaction.
- Approval-gated operations: curriculum.import, curriculum.promote, calendar.apply, assessment.approve_final, export.request, deletion.request. approvals.decide is a plain Allow for the operator (known gap SEC-03).
- Data stores: `<data>/g9.sqlite` (WAL, STRICT tables, 43 tables with BEFORE UPDATE/DELETE immutability triggers, SHA-256 audit chain), `<data>/binding.key` (32 bytes from the OS CSPRNG, 0600; approval HMAC key), backups/, quarantine/, restore-log.jsonl; localStorage holds theme ids only.
- Outbound: none. No process spawn, no network client in the Linux build, no fetch/XHR/WebSocket in the UI. CSP: `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'self'`.
- Model: only NullRuntime and ScriptedRuntime exist; model output is schema-validated twice, limited to READ/PROPOSE/CREATE actions, never rendered or executed.

## 2. Findings (most severe first)

### [MEDIUM] A backup snapshot with planted schema objects passes verification and restore; its triggers then run inside every later command

```
Location:   crates/g9-storage/src/migrations.rs:256-290 (verify_checksums); crates/g9-storage/src/backup.rs:322-399 (verify_connection);
            crates/g9-storage/src/restore.rs:395-424 (verify_copy); crates/g9-storage/src/connection.rs (open path);
            apps/cli/src/commands/restore.rs:282-300 (plan output); claim: docs/threat-model/threat-model.md ST10, ARCHITECTURE.md:106
Class:      OWASP A08:2021 Software and Data Integrity Failures / CWE-345 Insufficient Verification of Data Authenticity
Confidence: Confirmed (found independently by A1 and A2; reproduced by the lead auditor through the real CLI and by re-running A1's probes)
```

Evidence (read): `verify_checksums` compares only the rows of `schema_migrations` with the embedded migrations; `sqlite_master` (tables, triggers, views) is never compared with the schema the migrations produce:

```rust
for row in applied {
    match EMBEDDED_MIGRATIONS.iter().find(|m| m.version == row.version) {
        Some(m) if m.sha256() == row.sha256 => verified.push(row.version),
```

The restore plan prints the snapshot's own hash next to the command that confirms it (`"snapshot_sha256": actual_sha256, "snapshot_matches_expected": ...`), so `--expect-sha256` authenticates nothing when the file did not come from this installation.

Exploit path:
1. The attacker copies any genuine G9 database (migration checksums and the audit chain verify because the data is genuine).
2. In the copy: `DROP TRIGGER trg_audit_metadata_no_delete; CREATE TRIGGER zz AFTER INSERT ON audit_metadata BEGIN DELETE FROM audit_metadata WHERE seq = NEW.seq; END;` (and optionally drop `trg_grade_decisions_final_no_update`, add a trigger that sets `passed = 1` on every final grade, or one that copies note bodies before erasure).
3. The file reaches the operator as "a backup" (or a real backup is modified at rest, for example on removable or synced storage).
4. `g9 restore --snapshot F --expect-sha256 <hash printed by the plan> --confirm-replace-live-database`: every check passes; boot reports READY.
5. Every later command runs the planted SQL inside the app's own transaction.

Result (lead-auditor reproduction, CLI): after the restore, `backup.create` succeeded and its audit rows were deleted by the planted trigger (count unchanged at 2); `g9 init` then reported `audit_chain_ok: true, audit_rows: 2, write_block_reason: None`, with the trigger still present. A1's probes (re-run): a failing provisional grade (total 0) was stored final with `passed=true total_score=100`; an approved erasure left the note body in an attacker table; a tampered live schema propagated into a backup that `backup.verify` and `backup.restore_test` drove to RETAINED.

Impact: persistent, silent subversion of the authoritative store: audit rows of approvals, exports and deletions vanish while the chain reports OK (repudiation); finalized grades and their derived competency observations can be forged; erasure (ADR-0002, A10) can be defeated; restore-tested backups carry the tampering. Not OS-level code execution (no extension loading; `trusted_schema=OFF` limits functions) and approvals cannot be forged (the HMAC key is not in the database).

Fix:
- Compute the expected schema fingerprint per schema version: apply `EMBEDDED_MIGRATIONS[..=v]` to an in-memory database and take SHA-256 over sorted `(type, name, tbl_name, sql)` from `sqlite_schema`, with the SQL text exact (corrected 2026-11-13: "normalized sql" is unsafe, because a newline ends a `--` comment; see Remediation status) (cache in a `OnceLock`, or generate at build time).
- Require an exact match in `verify_connection` (backups), `restore::verify_copy` and every `Database::open`; on mismatch return `DATA_CORRUPTION storage.schema_drift`, which recovery turns into READ_ONLY.
- Open the restore copy with `trusted_schema=OFF` and `query_only=ON` before its first query; quote identifiers in `table_counts` (double embedded `"`).
- Do not print the actual hash in the restore plan; require the hash from this installation's backup manifest or the operator's own record, and say so.
- Tests: a snapshot with one trigger dropped and one added is refused at verify, at restore and at open (fail first).

Residual risk: a forged snapshot with the correct schema and fabricated rows still verifies; backups carry no keyed MAC (IMP-M08 PROPOSED) and the audit chain is unkeyed (ADR-0004, SEC-08). Authenticating a foreign file needs a MAC with a key held outside the database.

### [MEDIUM] An approval can be bound to a future revision, so assessment.approve_final can finalize a grade the operator never saw

```
Location:   crates/g9-app/src/pipeline.rs:665, 687-715, 799-808; crates/g9-policy/src/approvals.rs:211-214;
            crates/g9-app/src/commands/assessment.rs:1143-1179 (approve_final parameters); claim: pipeline.rs:47-51
Class:      OWASP A01:2021 Broken Access Control / CWE-367 Time-of-check Time-of-use (authorization bound to a client value)
Confidence: Confirmed (A1 probe p1_prebind re-run by the lead auditor: 3 of 3; code path read)
```

Evidence (read):

```rust
// pipeline.rs:665 — the client's value wins
let bound_revision = meta.expected_revision.or(found.current_revision);
// ... Decision::ApprovalRequired: request created with bound_revision, returns Err(Refusal { new_approval: Some(request), .. })
// pipeline.rs:799-801 — the only comparison of the client revision with the stored one, never reached on that path
if let (Some(expected), Some(actual)) = (meta.expected_revision, found.current_revision) {
    if expected != actual {
```

The approve_final parameters hash covers only `{operation, payload: {attempt_id}}`; the grade content is bound only through the revision.

Exploit path:
1. A hostile client (a compromised webview, or `g9 call --expected-revision`) sends `assessment.approve_final` for attempt X at stored revision R (failing grade) with `meta.expected_revision = R + 2`.
2. The pipeline answers APPROVAL_REQUIRED and stores an approval bound to R + 2; no STALE_REVISION is raised.
3. The operator inspects attempt X (failing, revision R) and approves, for example with `g9 approve <id>` from the CLI.
4. The client calls `assessment.review_evidence` (R + 1) and `assessment.grade_provisional` with top levels (R + 2).
5. The client retries approve_final with the approval id: binding R + 2 equals the stored revision, the MAC verifies.

Result (probe output): `stored approval ... bound to expected_revision=Some(6) (attempt is at 4)`; `operator sees: state="GRADED_PROVISIONAL" revision=4 pass=false`; `consumption -> state="APPROVED_FINAL" final=true pass=true total=100.0`; control without pre-binding: `approval.revision_mismatch`.

Impact: an immutable final grade (and the competency observations derived from it) that the operator never saw, carrying a valid consumed approval. A compromised webview can already self-approve (known SEC-03), so the added exposure today is approval from another surface (the CLI); the same flaw would defeat the planned native confirmation dialog (ADR-0014).

Fix:
- In `run_main_transaction`, before requesting or consuming an approval: if `meta.expected_revision` and `found.current_revision` are both present and differ, return `Refusal::not_evaluated(G9Error::stale_revision(..))`.
- Bind to the stored revision: `let bound_revision = found.current_revision.or(meta.expected_revision);`.
- Bind what is being approved: add a backend-computed hash of the effective criterion results to approve_final's parameters (covered by the MAC) and show it in the approval summary.
- Test (fail first): approve_final with `expected_revision = stored + 2` returns STALE_REVISION and creates no `approval_requests` row.

Residual risk: the operator must still see what the approval covers; today the approval summary shows no grade content, and SEC-03 (webview self-approval) remains open.

### [LOW] Deleting the newest audit rows is not detected although the backup anchors prove the rows existed

```
Location:   crates/g9-storage/src/audit.rs:274 (verify_chain); crates/g9-app/src/recovery.rs:96 (open-time check); backup_manifests.audit_anchor_seq
Class:      Insufficient tamper evidence (repudiation) / CWE-345
Confidence: Confirmed (lead-auditor probe P1; earlier review lead storage-core-7)
```

Exploit path: a process with write access to the database file drops the two audit triggers, deletes the newest audit rows, restores the triggers. Probe: 6 rows, newest manifest anchor at seq 5, two newest rows deleted; `g9 init` reported `audit_chain_ok: true, audit_rows: 4, audit_first_bad_seq: None`.

Impact: recent audited actions can be erased without detection. Precondition: same-user write access (attacker model outside the main scope); a deliberate attacker could also rewrite the anchors, which live in the same file, so the fix raises the bar only against naive edits. ADR-0004 does not state this limit.

Fix: at open, require the row at each recorded `audit_anchor_seq` to carry `audit_anchor_hash`, and `MAX(seq)` to equal `sqlite_sequence` for `audit_metadata`; treat a mismatch as a broken chain (READ_ONLY). Document in ADR-0004 that tamper evidence does not hold against a same-user writer until anchors are keyed or kept outside the file.

### [LOW] The README's release build produces a Tauri dev binary that loads its UI from http://localhost:5173 and gives that origin IPC

```
Location:   apps/desktop/README.md (Building on Ubuntu 26.04: `cargo build -p g9-mentor-desktop --release`);
            apps/desktop/src-tauri/Cargo.toml (no custom-protocol feature); tauri.conf.json `build.devUrl`
Class:      OWASP A05:2021 Security Misconfiguration
Confidence: Likely (A2 read the pinned Tauri 2.12.5 sources: without the `custom-protocol` feature the build is a dev build that loads `devUrl`, and that origin counts as local for the ACL; the shell cannot be compiled in this sandbox)
```

Exploit path: the operator builds following the README; another local principal binds port 5173 first; the app loads that page as its own origin, and the page calls `g9_command` (approvals.decide, export.request, deletion.request, backup.*).

Impact: full webview-level control by whoever owns the port. Preconditions: the README build path and a second local principal. `npm run tauri:build` passes the feature and is not affected.

Fix: README to `npm run tauri:build`; add `[features] custom-protocol = ["tauri/custom-protocol"]`; fail a release-profile build in build.rs when `tauri_build::is_dev()` is true.

### [LOW] Interaction mode is client-declared for 33 of 35 operations; declaring STUDY during an open exam is allowed

```
Location:   crates/g9-app/src/commands/mod.rs:115-128 (with_backend_mode, used only by skills.invoke and session.correct_time); crates/g9-policy/src/rules.rs:977-999
Class:      Client-controlled authorization input (OWASP A01:2021)
Confidence: Confirmed (A1 probe p4 re-run: `declared EXAM -> Err("policy.denied"); declared STUDY during the same open exam -> Ok`)
```

Impact today: small (no operator rule protects exam integrity through mode on the other operations); any future mode-gated rule would be bypassable, and MAINTENANCE is accepted as declared. docs/audit/defect-register.md marks TEC-25 IMPLEMENTED.

Fix: derive the effective scope once in dispatch for every operation (EXAM while an EXAM attempt is STARTED; MAINTENANCE only from a backend-held state); or set TEC-25 to PARTIAL and document modes as declared intent.

### [LOW] `g9 --human` writes untrusted strings to the terminal without escaping control characters

```
Location:   apps/cli/src/output.rs (compact: `Value::String(s) => s.clone()`; keys printed raw); sources: crates/g9-curriculum/src/validate.rs messages built from manifest fields
Class:      CWE-150 Improper Neutralization of Escape, Meta, or Control Sequences
Confidence: Confirmed (A2 probe: 16 raw ESC bytes and an OSC 52 sequence in --human output; JSON output escapes them; mechanism read by the lead auditor)
```

Exploit path: a hostile curriculum manifest puts escape sequences in `day_id`; `g9 --human import-seed --manifest hostile.json` prints the validator messages raw. Impact: window-title spoofing, forged or cleared output, and on terminals that honour OSC 52 a replaced clipboard. Fix: escape C0, C1, DEL and bidi override characters in `compact` and in keys; format raw identifiers in validator messages with `{:?}`.

### [LOW] Skill pin verification hashes a second read of the package, not the bytes that are loaded; non-regular files are skipped

```
Location:   crates/g9-skills/src/package.rs (SKILL.md read and parsed, then `package_sha256(dir)` re-reads every file; `else if ft.is_file()` skips FIFOs and devices); crates/g9-skills/src/registry.rs (load_pinned)
Class:      CWE-367 Time-of-check Time-of-use
Confidence: Confirmed (A2 probe: body with an injected marker loaded while `package_sha256 equals compiled pin: true`; mechanism read by the lead auditor)
```

Impact: content that differs from the pin can be loaded with SkillPackage trust and recorded under the pinned hash; a FIFO in a package hangs bridge start-up. Precondition: write access to the skills directory (the .deb resource directory is root-owned, INFERRED); no capability is gained today (NullRuntime). Fix: read each file once (O_NOFOLLOW, regular file only, size cap), hash and parse the same buffers, refuse non-regular entries, compare with the pin before parsing.

### [LOW] An approval-gated command with expected revision 0 leaves no decision or audit row and returns raw SQLite text

```
Location:   crates/g9-domain/src/revision.rs (Revision accepts 0); crates/g9-storage/src/approvals_store.rs (CHECK expected_revision >= 1); crates/g9-storage/src/error.rs (constraint text in messages)
Class:      Insufficient logging of a refused authorization attempt / information exposure through an error message
Confidence: Confirmed (lead-auditor probe P2: `export.request --expected-revision 0` -> `INVALID_INPUT storage.check_violation "CHECK constraint failed: expected_revision IS NULL OR expected_revision >= 1"`; policy_decisions and audit counts unchanged)
```

Fix: validate `Revision` as 1..=i64::MAX at deserialization (schema minimum 1); map constraint failures to a code without SQLite text; record the refusal even when the approval insert fails.

### INFO

- `table_counts` interpolates table names from a snapshot's own `sqlite_master` (`SELECT COUNT(*) FROM "{table}"`); a crafted name is accepted (read-only connection, single statement, no impact found). Folded into the first fix.
- The orchestrator's `detail.truncate(1000)` panics on a UTF-8 boundary (A2 probe); unreachable through g9-app today (the runtime validates first), but a future adapter would poison the bridge mutex. Fix: `floor_char_boundary(1000)`.
- The Markdown link allow-list keeps `href="/\evil.example/x"` (A2 probe, 28 payloads); no sink, because clicks on non-`#` links are cancelled and new-window requests are ignored. Fix: `/^(?:https?:|mailto:|#|\/(?![\/\\]))/i`.
- Approval MAC does not cover state, expiry, requester or capability (earlier lead core-1): moot for this deployment, because a same-user process can read the key and approve through the CLI.
- Latent policy and repository gaps with no production path today: wildcard rules allow Process/NetworkHost/FilesystemPath kinds (core-2); skill and model actors could read PRIVATE content (core-4); repositories accept a `supersedes` from another attempt (storage-repos-3); `REPLACE` conflict resolution would skip delete triggers (`recursive_triggers` off; no code uses REPLACE). Close before adding any path that reaches them.
- CI hygiene: `npx --yes @cyclonedx/cyclonedx-npm@6.0.1` and `pip install jsonschema==4.26.0` resolve transitive dependencies without a lock or hashes (read-only jobs).
- The repository is public and 27 commits carry the owner's personal e-mail address in author metadata (reported to the owner on 2026-11-10).

## 3. Checked and clean

| Class | What was checked |
|---|---|
| A Access control | Actor assigned by the backend; `actor` key refused; schemas `additionalProperties: false` and `deny_unknown_fields`; one bridge command, no plugins, `core:default` only, remote origins refused; every approval-gated operation maps to its rule; READ_ONLY refuses every mutation including replays; single-use approvals hold sequentially and under two concurrent contexts (A1 p3 re-run) |
| B Injection | All SQL parameterized; `format!` only splices compile-time names (plus the INFO above); `VACUUM INTO ?1` bound; no process spawn, network client or eval in Rust or TypeScript runtime code; one HTML sink (DOMPurify allow-list; 28 XSS payloads neutralized); backup paths checked lexically, restore paths canonicalized, sidecars refused; skills: symlinks refused, custom frontmatter parser (no YAML engine), `$ref` banned, no remote schema fetch |
| C Approvals and keys | HMAC verified in constant time (`subtle`); key from the OS CSPRNG; key file 0600, symlink refused, inode re-checked, atomic creation; MAC never returned over IPC; expiry fails closed on a backwards clock; replay with the same idempotency key returns the stored outcome without a second consumption |
| D Secrets and configuration | Working tree and full git history: only documented test fixtures; strict CSP; no devtools feature; `withGlobalTauri: false`; asset protocol off; errors carry codes, never host paths or backtraces (the SQLite CHECK text is the LOW above) |
| E Data protection | Audit and policy rows are metadata only (ids and hashes); no logging subscriber; localStorage holds theme ids only; export writes no file (known) |
| F Supply chain and CI | cargo-deny: advisories, bans, licenses, sources ok (offline database 2026-11-06; CI runs it live and passed on runs 31 and 32); npm audit: 0 production advisories, 7 moderate dev-only (recorded); lockfiles with integrity hashes; install scripts only in esbuild (dev) and fsevents (optional); CI: `contents: read`, `pull_request` (not `pull_request_target`), every action pinned by SHA, no secrets |
| G Business logic | Idempotency check and store inside the same IMMEDIATE transaction as the mutation; audit appends serialized (no chain fork across processes); advisory scores never count toward totals |
| H AI and LLM | Model context is fixed system text, the pinned SKILL.md body and operator input (no note or imported text reaches it today); output validated twice; actions limited to READ, PROPOSE and CREATE; native resources and operator-only capabilities refused; output never rendered or executed; recursion blocked; only NullRuntime and ScriptedRuntime exist |

## 4. Coverage and limits

- The Tauri shell cannot be compiled here (no GTK/WebKit): conclusions about the ACL, dev mode and new-window handling come from reading the pinned Tauri, tauri-runtime-wry and wry sources (A2). WebKitGTK HTML parsing was not tested (the XSS probe used jsdom).
- Two processes were simulated with two contexts in one process; separate OS processes were not run.
- Ownership of the installed .deb resource directory is INFERRED root-owned.
- The cargo-deny advisory database used here dates from 2026-11-06.
- No fuzzing and no dynamic analysis of a built desktop binary.
- Partially read: g9-storage repositories (targeted functions), some CLI commands (simulate, seed, crash-drill: scanned), parts of g9-ai-adapter (context, scripted, bench) and g9-curriculum (validate, reconcile, plan).
- For a human: run the hostile-snapshot restore drill on the G9 host and decide whether `--snapshot` should accept files this installation did not create; confirm which command builds the binary the operator runs; check on the host that dropped links cannot navigate the main window; decide whether the CLI is an intended approval surface.

## 5. Do this first

1. Schema fingerprint check at verify, restore and open, plus a restore plan that no longer prints the hash it then accepts (closes the first MEDIUM and blocks planted triggers at the next open).
2. Revision binding: refuse STALE_REVISION before requesting or consuming an approval, bind to the stored revision, bind a hash of the graded state into approve_final (closes the second MEDIUM; prerequisite for the native confirmation of SEC-03).
3. Release build path: `npm run tauri:build` in the README, a `custom-protocol` feature and a build-time guard (one small change closes the dev-binary LOW).

→ Recommended next: `/securitymaxxing:fix` for the two MEDIUM findings (both already proven by executable probes, so the proof step is done), then re-run these probes to confirm they come back clean.
