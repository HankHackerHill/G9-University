# G9 MENTOR — triple security audit (2026-11-13)

Labels: VERIFIED (checked by running it or reading the exact code), TESTED (a test in the repository proves it), INFERRED, ASSUMED, PROPOSED, NOT RUN.

## Remediation status (2026-11-13)

| Finding | Severity | Status | Change | Proof |
|---|---|---|---|---|
| Digests of erased and of SECRET content, erasure reasons and approval MACs leave in exports; the digest of a short text is the text | MEDIUM | Fixed in two rounds (the second found by pass C) | ADR-0017: erasure replaces every stored digest of the erased content with a tombstone identity digest (migration 0014); append-only tables never receive a digest of erasable content; the approved erasure removes the files; exports withhold governance digests, reasons, MACs, **every hash of a whole request and the audit chain's check values**; the attempt's submission digest no longer covers the notes text; `deletion.request.reason` is 1 to 500 characters | `crates/g9-app/tests/erasure_digests.rs` (10; scans every column of every table and every file of every export; the export cases failed before), `crates/g9-storage/tests/gates.rs::a10_deletion_request_redacts_and_tombstones`, `repos::evidence::tests::note_create_revise_and_load_ledger` (the schema refuses an erasure that keeps the digest), `migrations::tests::the_schema_at_version_14_is_pinned_too`, `attachment_store::tests::remove_*`, `an_export_holds_no_hash_of_a_request_that_carried_a_secret_and_no_value_that_could_confirm_one` and `an_erased_submission_note_leaves_no_digest_of_its_text_in_the_attempt` (both failed first); the auditor's probe re-run after the fix |
| The native approval confirmation does not say what is approved | MEDIUM | Fixed in the backend; the GTK dialog itself NOT RUN | `ApprovalSummary::text()` is written by the backend from storage: what the record is, what policy granted, the request's own context, a fingerprint of the exact values; stored words are sanitised; "Approve" is inert for 1.5 s | `crates/g9-app/tests/native_confirmation.rs` (10; the six new ones did not exist before, and the text they check could not have passed on the old code), `crates/g9-storage/src/describe.rs` tests; the shell type-checks and lints clean (stub pkg-config) |
| `g9 restore` prints a paste-ready command built from unquoted paths; pasting it runs the commands in a folder name | MEDIUM | Fixed | The command is printed as an exact argument list and as one line quoted for a POSIX shell, and the line is `null` when a path holds a control or invisible character; the same for `g9 crash-drill`; no `g9` output carries a raw control character | `apps/cli/tests/restore.rs` (2 new; both failed before), `apps/cli/src/output.rs` tests (4); the auditor's probe re-run after the fix |
| The window can be navigated away from the application (links in notes, redirects, forms) | MEDIUM (plausible) | Fixed in three layers; NOT RUN on a display | Rendered Markdown carries no link; the shell refuses every navigation but the application's (`may_load`, one command-less plugin); CSP `form-action 'none'` | `lib/markdown.test.ts` (14 link forms; failed before), `navigation_tests` (URL logic, run in a scratch crate against the same `url` 2.5.8), shell clippy |
| `evidence.attach_file` with a `note_id` drops the stricter classification that was asked for | LOW | Fixed | The file takes the stricter of its own and the note's classification | `crates/g9-app/tests/feature_evidence.rs::a_file_on_a_note_takes_the_stricter_of_its_own_and_the_notes_classification` (failed before; also reads the export), the same in the mock |
| Backup snapshots are created `0644`; `g9 copy-backup` keeps that mode | LOW | Fixed | Snapshots and copies are created `0600` before SQLite or `copy` writes to them | `apps/cli/tests/restore.rs::a_backup_copied_to_another_drive_restores_from_its_own_manifest` (mode check; failed before) |
| Event titles, file names and labels accept blank text, control characters and bidirectional overrides (`pin<U+202E>txt.exe` is shown as `pinexe.txt`) | LOW | Fixed | One check at the boundary, by key (`title`, `filename`, `label`), for every operation: `input.name_not_printable`; the refusal names the field and the code point, never the text | `crates/g9-app/tests/names.rs` (3; the refusal case failed before), `g9_domain::text` tests, the same in the mock |
| The confirmation accepts a click at once; two confirmations can stack; `core:default` exposes cursor and window functions to the webview | LOW (plausible) | Partly fixed | "Approve" is inert for 1.5 s. Stacked dialogs and `core:default` are left: see section 2 | NOT RUN (the dialog needs a display); `core:default` is named by ARCHITECTURE.md, which this change may not edit |
| `g9 --human` writes untrusted strings to the terminal (earlier audit, LOW) | LOW | Fixed | Escaped in `--human` (`\u{...}`) and in JSON (a `\u` escape) | `apps/cli/src/output.rs` tests |
| `--scratch` accepted without `--data-dir`; `g9 ops` says `approval_expected: false` for `calendar.delete_event` | INFO | Fixed | Both commands name the directory; the list includes the deletion | `apps/cli/src/commands/mod.rs`, `meta.rs` tests |
| Markdown allow-list kept `href="/\evil.example/x"` (earlier audit, INFO) | INFO | Moot | Rendered Markdown carries no link | `lib/markdown.test.ts` |
| Audit tail truncation; client-declared interaction mode; skill pin hashed from a second read; revision 0 refusal; the INFO items below | LOW, INFO | The four LOW items fixed 2026-11-14; the INFO items below partly | see the 2026-11-12 report's remediation table, ADR-0004 addendum and ADR-0019 | `crates/g9-storage/tests/audit_tail.rs`, `crates/g9-app/tests/{audit_tail,exam_mode_derivation,boundary_revision}.rs`, `crates/g9-skills/tests/package_hardening.rs` |

Found while fixing (2026-11-13), kept because each changes what to trust:

- **The tool that wrote two of the fixes wrote raw hidden characters into source.** Text I passed to the editing tool as a JavaScript escape (a backslash, `u`, four hex digits) arrived as the character it names: `apps/desktop/src/lib/text.ts` and `text.test.ts` held U+202E, U+2028 and U+FEFF raw, and a doc comment in the new Rust test held U+202E (rustc's `text_direction_codepoint_in_comment` lint stopped that one; the TypeScript compiler and the test runner did not notice the others). This is the class of defect the audit was about, in the repository itself. `crates/g9-domain/tests/source_text.rs` now scans every text file of the repository for `g9_domain::text::is_hidden` characters on every `cargo test`; one UTF-8 byte order mark in a test fixture is allowed by exact count. The scan found nothing else in 1,120 files.
- **The first version of the confirmation sanitiser turned line breaks into the replacement character** instead of a space, because the control-character test ran before the whitespace test; the forged-line test caught it. A stored title can now never add a line to the confirmation.
- **The approved erasure had to change more than the audit asked.** The trigger of migration 0010 required a version's body digest to survive its erasure; erasing the digest was refused by the schema (`storage.immutable`). Migration 0014 recreates that trigger so that the one allowed update must change the digest. Schema pin for version 14: `7f6b7505670598abc96287fea675ae2457aa92153248329e0274e04378dbfc9e` (452 objects).
- **Decision taken without the owner:** ADR-0016 left it open whether an approved erasure may also remove the attachment files. ADR-0017 decides yes, because a file is named by the digest of its content and keeping it makes the digest fix void. The confirmation says how many files go with a note. Reversal is one argument (ADR-0017, Consequences).

Revision audited: `9aed4a9` (branch `ccr-0cea681c-p9jbru`). The audit passes were read-only: every probe ran outside the repository (scratchpad `audit3/`), on scratch data directories. The fixes were made afterwards, each with a test that failed first where a test could fail, and are in the commit that adds this report.

## How the audit was run

| Pass | Who | Method |
|---|---|---|
| A | Two independent auditors with no access to each other's results: A1 core boundary (g9-app, g9-policy, g9-domain, g9-storage; a model of the Opus tier), A2 perimeter (Tauri shell, CLI, skills, AI adapter, evidence, curriculum import, UI rendering, supply chain, CI; a model of the Fable tier) | Code reading in full of their scope, executable probes (shell scripts against the real `g9` binary, Rust probe crates, a Markdown sanitiser probe), a standing instruction to label NOT RUN what could not run |
| B | Lead auditor | Attack-surface map, whole-repository sink sweeps (`unsafe`, process spawn, network, SQL identifiers, HTML sinks), supply chain and CI review, offline RustSec scan, `npm audit` |
| C | Lead auditor | Adversarial verification: every candidate from A and B was re-read in the code; the export finding was reproduced by running A1's probe myself (it printed `RECOVERED SECRET BODY: 4711`) and the restore-command finding through the real CLI; the confirmation finding was confirmed by reading the code (the dialog needs a display); after the fixes every probe was re-run against a rebuilt `g9`, and a new probe was written for what the re-run suggested (section 4) |

Severity follows the security-review-method rubric, calibrated for a single-operator offline desktop app: the realistic attackers are hostile content (files, notes, packages, model output, a path or a title that someone else chose), a compromised webview, a tampered file at rest, and supply chain. A same-user local process is out of scope except where a documented control claims to hold against it.

## 1. Attack surface map

Unchanged from the 2026-11-12 report, except:

- The window now has one navigation policy (`may_load`: the application, `about:blank`, and in a development build the dev server) and the CSP has `form-action 'none'`. Rendered Markdown has no links.
- `ApprovalSummary` carries `subject`, `grants`, `scope` and `fingerprint`; the confirmation text is built from them.
- `deletion.request` removes files from `<data>/attachments`: the only command that deletes files, only inside that folder, only the files of the erased attachments.
- Exports withhold fourteen more columns (section 2, first finding).
- Schema version 14.
- Outbound: none. No process spawn, no network client in the Linux build, no fetch/XHR/WebSocket in the UI.

## 2. Findings (most severe first)

### [MEDIUM] Digests of erased and of SECRET content, erasure reasons and approval MACs leave in exports

```
Location:   crates/g9-storage/src/deletion.rs (tombstone content_sha256_before; redaction kept notes.body_sha256, note_versions.body_sha256,
            attachments.content_sha256, evidence_items.content_sha256); crates/g9-storage/src/export.rs (WITHHELD list);
            crates/g9-app/src/commands/notes.rs and evidence.rs (events and provenance carried content_sha256)
Class:      CWE-759 Use of a One-Way Hash without a Salt (low-entropy input) / OWASP A02:2021 Cryptographic Failures
Confidence: Confirmed (A1; reproduced by the lead auditor with A1's probe)
```

Evidence (read, then run): a digest was stored in tables that cannot be erased (events, provenance, tombstones, deletion reports) and in the erased rows themselves, and the export wrote governance tables as system metadata. The probe created a SECRET note whose body was four digits, erased it through the approval flow and exported the SYSTEM area only, without private rows; `deletion_tombstones.csv` held the SHA-256 of the body in `content_sha256_before`, and ten thousand guesses recovered it. `deletion_requests.csv` held the reason the operator had typed (`erase my PIN 4711 note`):

```
RECOVERED SECRET BODY: 4711
```

Impact: erasure did not erase. A file or text that was erased, or that was SECRET and never exported, was recoverable from an export that the operator may copy, mail or keep on a USB drive; a known file was recognised without any guessing. The deletion reason (operator text) and the approval MACs left in the same files.

Fix (ADR-0017): see the status table. The identity digest of a tombstone is `SHA-256("g9-tombstone-v1\n<request>\n<table>\n<row>\n<time>")`, derived from nothing the content holds; no append-only table receives a digest of erasable content; the approved erasure removes exactly the files it made unreferenced, before it commits; seven columns are withheld from exports.

**Found by pass C: the first fix was incomplete.** Re-running A1's probe against the fixed build showed no digest of the body, and I then asked whether a hash of the whole request could stand in for it. It can: `policy_decisions.parameters_sha256` is the SHA-256 of `{operation, payload}`, the SYSTEM export carried it, and with the title guessed (`pin`) a script recovered the body (`4711`) from it in 10,000 tries (`passc_request_oracle.sh`, VERIFIED, before the second fix). `approval_requests.parameters_sha256` and `.operation_json`, `approval_decisions.parameters_sha256` and the audit chain's `details_sha256`, `prev_hash` and `row_hash` carry the same value or confirm it one hash further on. A second, direct instance sat in an evidentiary row: `attempts.submission_sha256` was the hash of `{evidence_refs, notes}`, so after the submission notes were erased, anyone with the (exported) evidence ids could attack the text. Fixed by withholding those columns from exports and by hashing `has_notes` instead of the text; after the second fix the same probe finds no match.

Residual: rows erased before migration 0014 keep their old digests (PROPOSED: a data migration; no such database is known), and backup snapshots keep everything they held. **The immutable ledgers still hold hashes of whole requests in the database file and in every snapshot** (`policy_decisions`, `approval_requests`, `approval_decisions`, `audit_metadata`, `idempotency_records`): an erased low-entropy text can be confirmed from a snapshot by whoever can guess the rest of the request. PROPOSED: key those hashes (HMAC with the binding key); it changes what every approval is bound to, so it needs its own decision.

### [MEDIUM] The native approval confirmation does not say what is approved

```
Location:   crates/g9-app/src/dispatch.rs (ApprovalSummary::text); apps/desktop/src-tauri/src/lib.rs (native_confirm)
Class:      CWE-451 User Interface Misrepresentation of Critical Information
Confidence: Confirmed by reading (A1, A2); the dialog itself NOT RUN
```

Evidence: the text read `Approve: <operation>`, the resource kind, an opaque id (a UUID, or for `curriculum.import` words chosen by the requester), a revision and two times. The operator could not tell which note an erasure covered, which export an approval allowed or which grade it would finalize; a compromised webview shows one record in its own card and asks the backend about another. The dialog also accepted a click that was already under way when it opened.

Impact: the control that exists so that a compromised webview cannot approve for the operator (ADR-0014) asked the operator to approve text that did not identify the request.

Fix: see the status table and ADR-0014 addendum 2. Every line is written by the backend from storage and sanitised (`g9_domain::text::one_line`); a record that is not stored yet says so; the requester's own context is labelled as that. The note's description says how many files an erasure takes.

Residual (NOT RUN): two dialogs raised in a row stack as nested modal loops; each names its own request and waits its own 1.5 s, so no request is approved unread, but the stacking is untested on a display.

### [MEDIUM] `g9 restore` prints a command that runs the contents of a folder name when pasted

```
Location:   apps/cli/src/commands/restore.rs:347-360,386 (to_confirm); apps/cli/src/commands/crash_drill.rs:46 (next)
Class:      CWE-78 OS Command Injection (through the operator's copy and paste) / OWASP A03:2021 Injection
Confidence: Confirmed (A2; reproduced by the lead auditor through the real CLI)
```

Evidence: the plan joined paths into a string with `format!`. A snapshot in a folder named `usb backup $(touch PWNED) ; echo INJECTED #` produced a `to_confirm` that, pasted into a shell, ran `touch` and `echo` before the restore command. Snapshots arrive on USB drives and network shares, which is what `--snapshot` is for.

Impact: arbitrary commands as the operator, from a file name the operator did not choose, at the moment the operator follows the program's own instruction.

Fix: `output::next_command` prints `to_confirm_argv` (the exact arguments) and `to_confirm` (a line quoted for POSIX shells: single quotes, `'\''` for a quote), and `to_confirm` is `null` when an argument holds a control or invisible character, because a terminal acts on such a character when it is pasted, before the shell reads any quote. `render_value` escapes hidden characters in every output.

### [MEDIUM] The window can be navigated away from the application

```
Location:   apps/desktop/src-tauri/src/lib.rs (builder: no navigation handler); apps/desktop/src-tauri/tauri.conf.json (CSP without form-action);
            apps/desktop/src/app/components/MarkdownPreview.tsx (only a click was intercepted)
Class:      CWE-601 URL Redirection to Untrusted Site / CWE-1021 Improper Restriction of Rendered UI Layers
Confidence: Plausible (A2, from the pinned tauri 2.12.5, tauri-runtime-wry and wry sources); NOT RUN on a display
```

Evidence (read): a webview follows a link, a form, a redirect or an assignment to `location` by itself; no Tauri capability is involved and no CSP directive governs a top-level navigation. The Markdown sanitiser allowed `http(s)` and `mailto` links and `MarkdownPreview` cancelled a left click only; "Open Link" in the native context menu, a middle click and the keyboard were not covered. wry installs a navigation policy only when a handler exists; none existed.

Impact: a link in a note, a skill package's documentation or a model reply could replace the application with a page from another origin inside the same window. Tauri refuses IPC from that origin (no remote capability exists), so the page cannot reach the backend; it can show anything the operator would take for the application, and a request to its address leaves the machine from an offline application.

Fix: three layers, any one of which closes the path. (1) `lib/markdown.ts` renders no link: the words stay, a web or mail address is shown after them as text to copy, every other address is dropped. (2) `may_load` allows `tauri://localhost`, `about:blank`, `http(s)://tauri.localhost` on Windows and Android only, and in a development build the origin of `build.devUrl`; a plugin without commands passes every navigation through it (tauri 2.12.5 consults each plugin's `on_navigation` for windows created from the configuration). (3) CSP `form-action 'none'`.

Residual (NOT RUN): a click, the context menu and a redirect in the real window.

### [LOW] `evidence.attach_file` with a `note_id` drops the classification that was asked for

```
Location:   crates/g9-app/src/commands/evidence.rs:137-166 (owner())
Class:      CWE-284 Improper Access Control (classification downgrade)
Confidence: Confirmed (A2)
```

A SECRET file attached to an INTERNAL note was stored INTERNAL and left in a plain export. Fixed: the stricter of the two wins. A file can be more sensitive than its note, never less.

### [LOW] Backup snapshots are created `0644` and `g9 copy-backup` keeps the mode

```
Location:   crates/g9-storage/src/backup.rs:222,321; apps/cli/src/commands/copy_backup.rs:152,158
Class:      CWE-732 Incorrect Permission Assignment for Critical Resource
Confidence: Confirmed (A2; the probe printed 644 for the snapshot and for the copy, secret text readable)
```

A snapshot is a copy of the database, which is `0600`. Fixed: `backup::create_new_private` creates the file `0600` with `create_new` (which is also the no-overwrite check, without a gap between check and use) before SQLite writes to it; `copy-backup` writes its copy and its manifest through the same function instead of `fs::copy`, which carried the source's mode over.

### [LOW] Event titles, file names and labels accept blank text, control characters and bidirectional overrides

```
Location:   crates/g9-app/src/commands/calendar.rs (title); crates/g9-app/src/commands/evidence.rs clean_filename (only is_control and separators)
Class:      CWE-451 / CWE-176 Improper Handling of Unicode Encoding
Confidence: Confirmed (A1)
```

U+202E is a format character, not a control character, so it passed `clean_filename`. Names are shown as facts in lists, in the confirmation and in exports. Fixed once, at the boundary, by key, so that an operation added later is covered without its handler knowing the rule; free text (a note's body, a reason) keeps its line breaks.

### [LOW] Two confirmations can stack; `core:default` exposes more to the webview than the UI uses

Confidence: Plausible (A2). `core:default` grants the webview, among other things, window and cursor functions, event listening, path resolution, menu and tray creation and decoding an image file from any readable path. The UI only invokes one application command. ARCHITECTURE.md, SECURITY.md, the threat model and the RTM name `core:default`; reducing it to an empty list needs those owners and a run on a display (a Tauri application command needs no permission unless an app manifest exists, which is INFERRED from the pinned source and NOT RUN). Not changed.

### INFO

- Still open from 2026-11-12: audit tail truncation is not detected; interaction mode is client-declared for 33 of 35 operations; the skill pin is hashed from a second read of the package; a refusal for expected revision 0 leaves no decision or audit row. **All four fixed 2026-11-14** (2026-11-12 report, remediation table).
- A declined native confirmation leaves no audit row (the refusal is `approval.not_confirmed`; nothing is written). Fix: a refusal transaction in `dispatch_confirming`. **Fixed 2026-11-14:** one metadata-only audit row (`approvals.decide`, `NOT_EVALUATED`, `UNAUTHORIZED`, the approval id and the code hashed); nothing is written over a read-only database; the request stays pending (`native_confirmation.rs`).
- `session.create` has no upper bound on `planned_start` (**fixed 2026-11-14**: at most 2100-01-01T00:00:00Z, schema and mock); an attachment of 16 MiB or less that is a PNG followed by an archive is accepted as a PNG (a polyglot: the signature check reads the start only; still open).
- The production bundle falls back to the mock transport when the Tauri object is absent from the window; the shell shows a banner "Synthetic data", and no data is stored, but the bundle should refuse instead (`VITE_G9_TRANSPORT`). **Fixed 2026-11-14:** a production bundle without the runtime keeps the Tauri transport and every call answers `RESOURCE_UNAVAILABLE ipc.tauri_runtime_missing`; `npm run dev`, the tests and a build with `VITE_G9_TRANSPORT=mock` still use the mock (`ipc/index.test.ts`).
- `evidence.attach_file` answers `already_stored` for identical bytes in the caller's scope: an oracle for known files within that scope (INFERRED; no cross-scope path).
- The CLI's default data directory (`g9-mentor`) differs from the desktop application's (`com.g9.mentor`); `g9` against the default directory inspects nothing of the installed application.
- Workspace dependencies use caret requirements; the lock file fixes the versions (VERIFIED in `Cargo.lock`); `npm` dependencies are pinned exactly.
- The 2026-11-12 INFO items that were not listed above are unchanged.

## 3. Checked and clean

| Class | What was checked |
|---|---|
| A Access control | Actor assigned by the backend (`actor` key refused); schemas `additionalProperties: false`; one bridge command; every approval-gated operation maps to a policy rule; deletion of a calendar event needs `DataDelete`; READ_ONLY refuses every mutation; single-use approvals; approvals bind to the stored revision and the exact parameters (both MEDIUM findings of 2026-11-12 hold: their regression tests pass) |
| B Injection | All SQL parameterized; `format!` only splices compile-time or quoted names; no process spawn, network client or `eval` in Rust or TypeScript runtime code; one HTML sink (`dangerouslySetInnerHTML`, DOMPurify allow-list, now without links); backup paths checked lexically, restore paths canonicalized; the erasure removes a file only from a path that is a content address inside the attachment folder and refuses a link at any component (`attachment_store::remove`) |
| C Approvals and keys | HMAC verified in constant time; key from the OS CSPRNG, `0600`; MAC never returned over IPC, never exported (new); expiry fails closed |
| D Secrets and configuration | Working tree: only documented test fixtures; strict CSP now with `form-action 'none'`; `withGlobalTauri: false`; asset protocol off; no devtools feature; errors carry codes, never host paths or backtraces |
| E Data protection | Audit and policy rows are metadata only; no digest of erasable content in any append-only table (tested over every column); exports hold no digest of erased or SECRET content, no erasure reason and no MAC; snapshots `0600` |
| F Supply chain and CI | RustSec advisory database 2026-11-06, 486 packages, scanned offline: 0 vulnerable, 2 informational (RUSTSEC-2024-0429 glib unsound, RUSTSEC-2024-0370 proc-macro-error unmaintained; both through GTK and Tauri build-time macros, no path from untrusted input); `npm audit`: production 0, development 7 moderate (`minimatch` 10.2.6 through `typescript-eslint` 8.72.1, GHSA-g3hv-75wg-m249, test tooling only); CI is manual-only, `contents: read`, every action pinned by SHA; no workflow was triggered or changed |
| G Business logic | Idempotency and mutation in one IMMEDIATE transaction; audit appends serialized; advisory scores never count toward totals; an erasure that fails after its files were removed leaves rows that name a missing file, which verification reports (documented order, ADR-0017) |
| H AI and LLM | Unchanged: no runtime beyond NullRuntime and ScriptedRuntime; output validated twice, never rendered or executed |

## 4. Coverage and limits

- The Tauri shell cannot be built here (no GTK or WebKit libraries). It was type-checked and linted with a stub `pkg-config` (`cargo clippy -p g9-mentor-desktop -- -D warnings`: clean); `may_load` was run in a scratch crate against the same `url` release. NOT RUN: the window, the dialog, its 1.5 s timer, stacked dialogs, a link click, the context menu, a redirect, WebKitGTK HTML parsing.
- Two processes were simulated with two contexts in one process; the CLI tests run separate processes.
- No fuzzing and no dynamic analysis of a built desktop binary.
- The RustSec database is dated 2026-11-06; CI's live `cargo deny` was not run (CI is manual-only).
- Probes of pass C: see the next section.
- For a human, on the host: build with `npm run tauri:build`; open a note whose text contains a link and confirm the window stays; raise an approval and confirm the dialog names the record and that "Approve" does nothing for a moment; erase a note with a file and confirm the file is gone from `<data>/attachments`; run the restore drill; decide the questions in section 5.

### Pass C: probes re-run after the fixes

Every probe was run again against a `g9` rebuilt from the fixed tree (`target/integration/debug/g9`), on scratch data directories, outside the repository. VERIFIED means the output was read.

| Probe | Before the fix | After the fix |
|---|---|---|
| A1 export probe: SECRET note `4711`, erased, exported | `RECOVERED SECRET BODY: 4711` from `deletion_tombstones.csv`; the typed reason in `deletion_requests.csv` | Four exports (SYSTEM only and all five areas, CSV and JSON, private rows included): no 64-hex token in any file is the digest of any integer below ten million, no reason, no approval MAC (VERIFIED) |
| Pass C request-hash probe (`passc_request_oracle.sh`): SECRET note `pin` / `4711`, SYSTEM export, body guessed from 0000 to 9999 with the title known | the first fix still left it: `policy_decisions.csv` held the request hash and the probe matched `4711` | no match; 91 other 64-hex tokens in the export, none a request hash (VERIFIED after the second fix) |
| A2 p1: restore plan from a folder named with shell syntax | `to_confirm` unquoted; pasted into a shell it created `PWNED_BY_PLAN` | `to_confirm` single-quotes the path; the pasting shell ran nothing and created nothing; `to_confirm_argv` lists the exact arguments (VERIFIED) |
| A2 p2: SECRET file on an INTERNAL note, exported | stored INTERNAL; its marker was in the export | stored SECRET; the export holds `attachments: 0`; the only marker found is the INTERNAL note's own body (VERIFIED) |
| A2 p3: backup file modes | snapshot `644`, copy `644` | live database, snapshot, copy and manifest all `600` with umask `0022` (VERIFIED) |
| A2 p4: text of the confirmation | observation of the old text with the probe's own template | cannot show the fix; proven by `native_confirmation.rs` (9 tests) instead |
| A2 p5: `--scratch` without `--data-dir`; control characters in `--human` output | the drill ran in the default data directory; escape sequences printed raw | refused with the message that names `--data-dir`; the probe's title with escape sequences is refused at the boundary (`input.name_not_printable`), so it never reaches an output; escaping is proven by the `output.rs` tests (VERIFIED) |
| A2 r1: planted schema objects (2026-11-12 MEDIUM) | refused | still refused, now at schema version 14 (VERIFIED) |
| A2 r2: approval bound to a future revision (2026-11-12 MEDIUM) | refused | still refused (VERIFIED) |
| A2 Markdown probe (28 payloads) | one link form survived (`/\evil.example`) | not re-run as a probe: `lib/markdown.test.ts` renders 14 link forms and asserts no `<a`, no `href`, no `target` |

## 5. Do this first

1. Run the shell on the host (see section 4): the three fixes marked NOT RUN are the ones a display decides.
2. ~~Confirm or reverse the decision in ADR-0017 that an approved erasure also removes the files.~~ **Confirmed by the owner 2026-11-14 (ADR-0020).**
3. Decide whether `core:default` may become an empty list (needs the owners of ARCHITECTURE.md, SECURITY.md, the threat model and the RTM, and one run on a display).
4. ~~Decide whether a declined confirmation should leave an audit row, and whether the production bundle should refuse to start on the mock transport.~~ **Done 2026-11-14 without the owner, both as the audit proposed (yes, yes).** Each is reversible in one place: `record_declined_confirmation` is one call in `dispatch_confirming`; the refusal is the third argument of `chooseTransport`.
5. ~~Close the four LOW items still open from 2026-11-12~~ **Done 2026-11-14** (audit tail truncation, backend-derived interaction mode, single read of a skill package, revision 0 at the boundary).

→ Recommended next: run the preflight on a clean export of the commit that contains this report (`scripts/ci_preflight.sh`), then the host checklist above.
