# G9 MENTOR desktop (`apps/desktop`)

Two parts:

| Part | Path | Built by |
|---|---|---|
| React 19 + strict TypeScript UI (16 screens, typed IPC client, `mock` and `tauri` transports) | `apps/desktop/src`, `index.html`, `vite.config.ts` | `npm ci && npm run build` (output `apps/desktop/dist`) |
| Thin Tauri 2 bridge (Rust crate `g9-mentor-desktop`, lib `g9_mentor_desktop_lib`) | `apps/desktop/src-tauri` | `cargo build -p g9-mentor-desktop` on a host with WebKitGTK 4.1 |

The bridge is excluded from the workspace `default-members`, so `cargo test --workspace` stays runnable on hosts without WebKitGTK.

## Verification status

| Item | Status | Evidence |
|---|---|---|
| Tauri wiring (`run`, `g9_command`, `setup`; module `shell` in `src-tauri/src/lib.rs`) | NOT RUN | `cargo check -p g9-mentor-desktop` fails in the authoring sandbox (Ubuntu 24.04.5): `failed to run custom build command for gdk-sys v0.18.2` — `The system library gdk-3.0 required by crate gdk-sys was not found`. pkg-config there also lacks `gtk+-3.0`, `pango`, `gdk-pixbuf-2.0`, `atk`, `libsoup-3.0`, `javascriptcoregtk-4.1`, `webkit2gtk-4.1`, `librsvg-2.0`. Written against the tauri 2.12.5 sources (`Builder::setup`, `Manager::try_state`, `PathResolver::app_data_dir`/`resource_dir`, `async_runtime::spawn_blocking`) — INFERRED to compile, not compiled. |
| Tauri-free bridge logic (`DataLayout`, `bootstrap`, `Bridge`, `AppState`, zone hints) and `src-tauri/src/keyring.rs` | TESTED (harness) | The same source files, with only the `mod shell { … }` block and `pub use shell::run;` stripped, compiled as a standalone crate with the workspace dependency pins and lock file: `cargo clippy --all-targets -- -D warnings` clean, `cargo test` 30 passed / 0 failed. The harness lives outside the repository (scratchpad); reproduce by copying `src/keyring.rs` and `src/lib.rs` minus the `shell` module into a crate with the `[dependencies]` of `src-tauri/Cargo.toml` except `tauri`. |
| `tauri.conf.json` and `capabilities/default.json` parse as Tauri config | TESTED | Deserialized with `tauri-utils` 2.10.2 (`Config`, `acl::capability::Capability`): CSP, `withGlobalTauri=false`, window `main`, capability `default` (`local=true`, no `remote`, `windows=["main"]`, `permissions=["core:default"]`). |
| Window renders, IPC round trip, keyboard/axe pass on Ubuntu 26.04 (KDE/Wayland) | NOT RUN | Needs the target host. |
| A12 supply-chain gate with this crate's dependencies | TESTED | `cargo-deny --all-features check` from the repo root (2026-11-06): `advisories ok, licenses ok, sources ok` after removing `keyring` (see "Binding key"); `bans` fails only on 18 wildcard path dependencies of `g9-evaluation` (not this crate; this crate's 5 were cleared by `publish.workspace = true`). |
| IPC operation names | TESTED (static) | The 35 operation names in `src/ipc/operations.ts` equal the 35 names in `crates/g9-app/src/ops.rs` and the 35 `schemas/ipc/*.request.schema.json` files. |

## Building on Ubuntu 26.04

```sh
sudo apt-get update
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
# toolchain: rust-toolchain.toml pins 1.97.0
cd apps/desktop && npm ci && npm run build && cd ../..
cargo check -p g9-mentor-desktop
cargo build -p g9-mentor-desktop --release
```

The package list is the one used by the CI job `desktop-shell` (`.github/workflows/ci.yml`); its install step succeeded on the `ubuntu-26.04` GitHub runner (runs 27 and 28, VERIFIED from the job records). It has not been tried on the G9 host: ASSUMED there.

Icons: `src-tauri/icons/` holds the four PNGs listed in `tauri.conf.json` `bundle.icon` (`32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`; added in commit `698ef3c`), which `tauri::generate_context!()` needs at compile time. The shell cannot be compiled in the build sandbox (no GTK/WebKit); it compiles and its `--lib` tests pass in the `desktop-shell` CI job (runs 27 and 28, VERIFIED from the job conclusions; test counts not read). Building and running it on the G9 host, and every window flow, remain NOT RUN.

## How the bridge is wired

```
webview (React)                               Rust process
  ipc/transports/tauri.ts                       src-tauri/src/lib.rs
  invoke('g9_command', {op, meta, payload}) ──▶ shell::g9_command (async, spawn_blocking)
                                                 └─ AppState::handle → Bridge::handle
                                                     └─ Mutex<g9_app::AppContext>
                                                         └─ g9_app::dispatch(ctx, op, meta, payload)
  ◀── Ok(CommandOutcome JSON) | Err(G9Error JSON)
```

| Concern | Implementation |
|---|---|
| Commands | Exactly one: `g9_command(op: String, meta: Value, payload: Value) -> Result<Value, Value>`. No other commands, no plugins (`tauri::Builder::default().manage(..).invoke_handler(generate_handler![g9_command]).setup(..).run(generate_context!())`). |
| Pipeline | Entirely in `g9-app` (ARCHITECTURE.md §5): unknown op → `INVALID_INPUT`; metadata parsed by `g9_app::ipc::parse_meta` (any `actor` key → `INVALID_INPUT meta.actor_not_accepted`); payload validated against `schemas/ipc/<op>.request.schema.json` before deserialization; idempotency → policy → approval consumption inside `BEGIN IMMEDIATE` → revision → mutation → events/audit → commit → idempotency record. The bridge adds no business rule and no SQL. |
| Actor | Backend-assigned operator `local-operator` (`OPERATOR_ID`), passed to `AppContext::builder`; never read from IPC (ADR-0013, Addendum A.8). |
| Errors | The command's `Err` is the serialized `G9Error` (`category`, `code`, `message`, optional `details`/`approval_id`; closed 13 categories). Bridge-level failures use the same shape: `RESOURCE_UNAVAILABLE bridge.starting`, `INTERNAL bridge.state_poisoned`, `INTERNAL bridge.worker_failed`, and the stored startup error. Never a Rust error string, never a host path or key byte. |
| Managed state | `AppState` (an `Arc<OnceLock<Bridge>>`) is managed before the window exists; `setup()` fills it once. Tauri 2.12.5 creates config windows before the setup hook runs (`app.rs` `fn setup`), so a call that arrives first gets `bridge.starting` instead of Tauri's "state not managed" string. |
| Startup failure | Fail closed and visible: `Bridge` keeps the startup `G9Error` and answers it to every command, including `system.status`. |
| Concurrency | One `Mutex<AppContext>`; commands run serially on Tauri's blocking pool, so a backup or restore test never blocks the window event loop. A panic inside a command poisons the mutex and every later command answers `INTERNAL bridge.state_poisoned` (in-memory state may be inconsistent). |
| Recovery | `AppContext::builder(db, config, key, operator).open()` runs `g9_app::recovery::run`: migrations only after a verified pre-migration backup into `<data>/backups`, audit-chain verification, open intervals closed `UNCERTAIN`, approval expiry. It runs inside `setup()` (window content waits). |
| Honest status | Facts only the bridge knows are appended to `CommandOutcome.warnings` of `system.status` (nothing else is changed): `binding_key.file_fallback` (always), `binding_key.file_rotated`, `binding_key.stale_file_not_removed`, `data_dir.permissions`, `clock.zone_fallback`, `skills.registry_unpinned`, `skills.registry_refused`, `storage.read_only`, `storage.migration_blocked`. Format `WARNING <code>: <message>`. The UI client accepts `warnings` but no screen renders them yet (UI owner). |
| Time zone | `TZ` (name, `:name` or zoneinfo path) → `/etc/localtime` link target → `/etc/timezone`; otherwise UTC with `clock.zone_fallback`. |
| Skills | `<resource dir>/.agents/skills` (bundled by `bundle.resources`) loaded with `g9_skills::Registry::load_pinned` against `skill-pins.json`, which is compiled into the binary (`include_str!`) so changing the bundled folder cannot change the pins. A modified, missing or unpinned package refuses the whole registry: skills are disabled (`skills.invoke` → `RESOURCE_UNAVAILABLE`) and the reason is reported in `system.status` warnings. The pins are checked against the packages by `python3 -I scripts/skill_hashes.py --check` (CI `content-gates`) and by the test `bundled_skills_match_compiled_pins_and_load` (NOT RUN here: needs the shell to compile). |

## Binding key

Decision (2026-11-06): the OS keyring is not used; the approval binding key lives only in the documented fallback file. Recorded here and in `src-tauri/src/keyring.rs`.

| Candidate | Result | Reason |
|---|---|---|
| `keyring` 4.2.0, feature `v1` (Secret Service over zbus) | Rejected | Pulls `zbus` 5.19 → `zvariant` 5.15.1, `zvariant_derive` 5.15.1, `zbus_names` 4.3.5: RUSTSEC-2026-0370/0372/0371 (unmaintained, merged into zbus 6, "no safe upgrade"). A12 `cargo-deny check advisories` failed. No advisory ignore is added. `keyring` 4.2.0 has no feature set without zbus (`v1` or `cli` is mandatory; both pull it). |
| `keyring-core` 1.0.1 + `zbus-secret-service-keyring-store` 1.0.1 | Rejected | Same zbus 5 tree (`secret-service` 5.2.0). |
| `keyring-core` 1.0.1 + `dbus-secret-service-keyring-store` 1.0.1 (`vendored`, `crypto-rust`) | Rejected | Advisory status NOT RUN (rejected first on licence): `libdbus-sys` `vendored` statically links libdbus C sources (AFL-2.1 OR GPL-2.0-or-later). That licence class is outside the `deny.toml` allow-list and invisible to `cargo-deny` (it reads the crate's `MIT OR Apache-2.0`). Without `vendored` it needs `libdbus-1-dev` (a system library). Needs a licence decision by the dependency-policy owner. |
| `secret-service` 6.0.0-rc.1 / `oo7` 0.7 (zbus 6) | Rejected | Release candidate, resp. a new async client tree not reviewed for A12; not adopted without that review. |
| `linux-keyutils-keyring-store` | Rejected | Kernel keyring, not the Secret Service; not persistent across reboots (INFERRED from the kernel keyring model; not examined further). |
| `0600` file `<data>/binding.key` (`getrandom` =0.4.3) | Adopted | The fallback documented in `docs/privacy/data-lifecycle.md` ("Approval binding key"). |

File rules (`keyring.rs`): 32 bytes from the OS CSPRNG; created as a private temporary file (`create_new`, mode `0600`, `fsync`) and hard-linked into place, so a concurrent start never sees a partial key and an existing path is never replaced; a group/other-accessible or wrong-length file is replaced (`binding_key.file_rotated`; approvals bound to the old key then fail verification — fail closed); a symlink or directory at the path is refused (`DATA_CORRUPTION binding_key.file_not_regular`) and left untouched; the opened file must be the inspected inode. Key bytes never appear in errors, warnings or `Debug` output (`BindingKey(<redacted>)`). Every start reports `WARNING binding_key.file_fallback` in `system.status`.

Risk accepted by this decision: any process running as the same user, and anyone who can read the data directory (unencrypted disk, copied home directory, a backup of `<data>`), can read the key and compute approval MACs. A same-user process can already write `g9.sqlite` directly, so for it the file adds no new capability; an offline reader gains the key, which the full-disk-encryption baseline (`docs/privacy/data-lifecycle.md`) is meant to prevent. On a desktop session the Secret Service would not isolate same-user processes either (INFERRED); it would protect the key at rest and keep it out of copies of `<data>`. A Secret Service backend is PROPOSED once a client passes A12 (advisories and licences); on adoption, an existing valid file key should be moved into the keyring so granted approvals stay verifiable.

## Data directory layout

`<data>` = Tauri `app_data_dir()` = `$XDG_DATA_HOME/com.g9.mentor` (default `~/.local/share/com.g9.mentor`). Nothing is hard-coded; a relative path is refused (`PRECONDITION_FAILED bridge.data_dir_not_absolute`).

| Path | Mode | Purpose |
|---|---|---|
| `<data>/` | `0700` | created on first start; tightened if group/other bits are set |
| `<data>/g9.sqlite` (+ `-wal`, `-shm`) | `0600` | the database; pre-created `0600` before SQLite opens it, opened with `g9_storage::Database::open` (WAL profile) |
| `<data>/binding.key` | `0600` | approval binding key (see above) |
| `<data>/attachments/` | `0700` | primary approved root |
| `<data>/backups/` | `0700` | `AppConfig.backup_dir`; pre-migration and operator backups |
| `<data>/scratch/` | `0700` | `AppConfig.scratch_dir`; restore tests, migration dry runs |

`AppConfig.approved_roots = [attachments, backups, scratch]`. Deviation from the brief ("approved roots = `<data>/attachments`"): `g9-app` refuses a backup or scratch directory outside an approved root (`recovery.rs` `migrate_after_backup`), and nesting backups inside the attachment store would mix them with user content.

## Capability scope

| Setting | Value | Source |
|---|---|---|
| Capability file | `src-tauri/capabilities/default.json`: `local: true`, `windows: ["main"]`, `permissions: ["core:default"]`; no plugin permissions | ARCHITECTURE.md §3 M01, SECURITY.md, threat model B1 |
| Plugins | none registered; no `tauri-plugin-*` dependency | SEC-04 exclusion list (shell, http, fs, sql, updater, deep-link, process, autostart, clipboard, opener) |
| App command ACL | `g9_command` is reachable from the local origin only. Tauri 2.12.5 (`webview/mod.rs`) ACL-checks app commands only for remote origins or when `build.rs` declares `tauri_build::AppManifest::commands`; no remote capability exists, so remote content can never call it | SEC-01/SEC-04 verifier notes |
| `withGlobalTauri` | `false` (the UI imports `@tauri-apps/api/core` dynamically) | SEC-03 |
| CSP | `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'self'` | ARCHITECTURE.md §12, RTM R-001 |
| Asset protocol | disabled (default) | SEC-04 (d) |

IPC under `connect-src 'none'` (INFERRED from tauri 2.12.5 `scripts/ipc-protocol.js`): the first `fetch` to the `ipc:` custom protocol is refused by the CSP, the injected script logs "IPC custom protocol failed" once and switches to the `window.ipc.postMessage` channel, which the CSP does not govern. Commands therefore work without opening `connect-src`; NOT RUN on a host.

PROPOSED hardening (not applied; each needs the named owner or a host test):

| Item | Why not applied here |
|---|---|
| `freezePrototype: true` (SEC-03/SEC-04) | Freezing `Object.prototype` breaks code that assigns inherited names such as `constructor`/`toString` on plain objects (override mistake); needs a UI run on the host. |
| CSP additions `form-action 'none'; frame-ancestors 'none'; base-uri 'none'` (SEC-05, PLT-04) | Tightening only; deferred to keep `tauri.conf.json` identical to the reviewed UI configuration until a host run confirms it. |
| Reduce `core:default` to the permissions the UI uses (none: it only invokes an app command) | `core:default` grants `core:image:allow-from-path` (decode an image file from any readable path), menu/tray creation and path resolution to the webview. ARCHITECTURE.md, SECURITY.md, threat model and RTM all state `core:default`; changing it needs those owners. |
| Navigation guard (refuse non-local top-level navigation) | CSP does not govern `window.location` navigation, an egress channel for injected script. Needs the window built in `setup()` with `on_navigation` (and `create: false` in the config); untestable here. |
| Local operation log (`tracing` subscriber writing metadata-only events under `<data>`) | `g9-app` emits `tracing` events and `INTERNAL` errors say "see local operation log", but the bridge installs no subscriber, so those events are dropped. Needs a `tracing-subscriber` dependency reviewed for A12. |
| `build.rs` `AppManifest::commands(&["g9_command"])` + per-window grant | Outside the bridge's assigned files; only one window exists today. |
| Native confirmation for protected operations (Addendum A.7, SEC-03) | Needs a Rust-side dialog (plugin or GTK dialog crate). Today `approvals.decide` from the webview grants approvals, so script injected into the webview can approve its own requests; the MAC binding still prevents parameter substitution and replay. |
