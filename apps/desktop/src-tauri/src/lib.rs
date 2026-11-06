#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
// `G9Error` is the closed IPC error contract (ADR-0013) and is returned by value.
#![allow(clippy::result_large_err)]
//! # g9-mentor-desktop — the thin Tauri 2 bridge (M01)
//!
//! ARCHITECTURE.md §2 rule 6: the webview never touches SQLite, a model or the
//! host; it reaches the backend through exactly one Tauri command,
//! `g9_command(op, meta, payload)`, which forwards to [`g9_app::dispatch`]. The
//! command pipeline (§5: schema check → idempotency → policy → approval
//! consumption inside the transaction → revision check → mutation → events and
//! hash-chained audit → commit → idempotency outcome) runs entirely in `g9-app`;
//! this crate adds no business rule.
//!
//! | Concern | Where |
//! |---|---|
//! | Managed state | [`AppState`]: an `Arc<OnceLock<Bridge>>` managed before the window opens and filled once by `setup()`; commands that arrive earlier get `RESOURCE_UNAVAILABLE bridge.starting` |
//! | Bootstrap | [`bootstrap`]: data layout (`0700`), binding key ([`keyring`]: `0600` file, OS keyring not used), `Database::open` of `<data>/g9.sqlite`, `AppContext::builder(..).open()` (which runs `g9_app::recovery::run`) |
//! | Actor | fixed backend operator [`OPERATOR_ID`]; `g9_app::ipc::parse_meta` refuses any `actor` key in IPC metadata (ADR-0013, Addendum A.8) |
//! | Errors | the serialized [`G9Error`] (closed 13 categories, secret-free) is the command's `Err` value; never a Rust error string |
//! | Honest status | bridge facts the backend cannot see (key file fallback, zone fallback, unpinned or refused skills, read-only storage reason) are appended to `CommandOutcome.warnings` of `system.status` |
//!
//! Capabilities: `capabilities/default.json` grants only `core:default` to the
//! `main` window; no plugin is registered (no shell, fs, http, dialog, opener).
//!
//! STATUS: the Tauri wiring (`run`, module `shell`) cannot be compiled in the
//! authoring sandbox (no webkit2gtk-4.1/GTK3 system libraries): NOT RUN. The
//! Tauri-free part of this file and [`keyring`] are compiled and tested there
//! through a scratch harness that excludes the `shell` module; see
//! `apps/desktop/README.md` "Verification status".

pub mod keyring;

use g9_app::{AppConfig, AppContext, Transport};
use g9_domain::{ActorId, G9Error, IanaZone};
use g9_skills::Registry;
use g9_storage::{Database, OpenOptions};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Backend-assigned operator actor id (the local user of this installation).
/// Constant, so idempotency records and approvals stay attributable across
/// restarts and OS account renames.
pub const OPERATOR_ID: &str = "local-operator";
/// Application version recorded in migrations, backups and `system.status`.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Database file inside the app data directory.
pub const DATABASE_FILE: &str = "g9.sqlite";
/// Primary approved root (attachments; relative attachment paths resolve here).
pub const ATTACHMENTS_DIR: &str = "attachments";
/// Backup directory (inside an approved root, as `g9-app` requires).
pub const BACKUPS_DIR: &str = "backups";
/// Restore-test and migration dry-run scratch directory.
pub const SCRATCH_DIR: &str = "scratch";
/// Skill packages inside the Tauri resource directory.
pub const SKILLS_RESOURCE_DIR: &str = ".agents/skills";

/// Host facts the bridge resolves before bootstrapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeOptions {
    /// Tauri `app_data_dir()` (absolute; never hard-coded).
    pub data_dir: PathBuf,
    /// `<resource dir>/.agents/skills` when it exists.
    pub skills_dir: Option<PathBuf>,
    /// Operator time zone; `None` falls back to UTC with a warning.
    pub zone: Option<IanaZone>,
}

/// The app data directory layout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataLayout {
    /// `<data>` (mode `0700`).
    pub root: PathBuf,
    /// `<data>/g9.sqlite` (created `0600`).
    pub database: PathBuf,
    /// `<data>/attachments` (primary approved root).
    pub attachments: PathBuf,
    /// `<data>/backups`.
    pub backups: PathBuf,
    /// `<data>/scratch`.
    pub scratch: PathBuf,
}

impl DataLayout {
    /// The layout under `root`, which must be absolute.
    pub fn under(root: &Path) -> Result<Self, G9Error> {
        if !root.is_absolute() {
            return Err(G9Error::precondition_failed(
                "bridge.data_dir_not_absolute",
                "the application data directory must be an absolute path",
            ));
        }
        Ok(Self {
            root: root.to_path_buf(),
            database: root.join(DATABASE_FILE),
            attachments: root.join(ATTACHMENTS_DIR),
            backups: root.join(BACKUPS_DIR),
            scratch: root.join(SCRATCH_DIR),
        })
    }

    /// Create the directories (`0700`) and the database file (`0600`), and
    /// tighten existing ones that are accessible to other users.
    pub fn prepare(&self, warnings: &mut Vec<String>) -> Result<(), G9Error> {
        for dir in [&self.root, &self.attachments, &self.backups, &self.scratch] {
            platform::ensure_private_dir(dir).map_err(|_| {
                G9Error::resource_unavailable(
                    "bridge.data_dir_unavailable",
                    "the application data directory could not be created or is not a directory",
                )
            })?;
        }
        for dir in [&self.root, &self.attachments, &self.backups, &self.scratch] {
            if platform::restrict(dir, true).is_err() {
                warnings.push(keyring::warning(
                    "data_dir.permissions",
                    "a directory under the app data directory is accessible to other users \
                     and could not be restricted to the owner",
                ));
            }
        }
        platform::precreate_private_file(&self.database).map_err(|_| {
            G9Error::resource_unavailable(
                "bridge.database_unavailable",
                "the database file could not be created in the app data directory",
            )
        })?;
        let mut sidecars = vec![self.database.clone()];
        for suffix in ["-wal", "-shm"] {
            let mut name = self.database.clone().into_os_string();
            name.push(suffix);
            sidecars.push(PathBuf::from(name));
        }
        for file in sidecars {
            if file.exists() && platform::restrict(&file, false).is_err() {
                warnings.push(keyring::warning(
                    "data_dir.permissions",
                    "a database file is accessible to other users and could not be \
                     restricted to the owner",
                ));
            }
        }
        Ok(())
    }
}

/// Serialize a [`G9Error`] as the command's `Err` value (the shape of
/// `schemas/ipc/g9-error.schema.json`).
pub fn error_value(error: &G9Error) -> Value {
    serde_json::to_value(error).unwrap_or_else(|_| {
        json!({
            "category": "INTERNAL",
            "code": "bridge.serialize",
            "message": "internal error; see local operation log",
        })
    })
}

/// A bootstrapped backend: the context behind a mutex, or the startup error
/// that every command then returns (fail closed, visible in the UI).
#[derive(Debug)]
pub struct Bridge {
    ctx: Result<Mutex<AppContext>, G9Error>,
    warnings: Vec<String>,
}

impl Bridge {
    /// A bridge whose startup failed; every command answers `error`.
    pub fn unavailable(error: G9Error, warnings: Vec<String>) -> Self {
        Self {
            ctx: Err(error),
            warnings,
        }
    }

    /// Warnings appended to `system.status`.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// The startup error, when the backend is unavailable.
    pub fn startup_error(&self) -> Option<&G9Error> {
        self.ctx.as_ref().err()
    }

    /// Run one IPC call through `g9_app::dispatch`.
    pub fn handle(&self, op: &str, meta: Value, payload: Value) -> Result<Value, Value> {
        let ctx = match &self.ctx {
            Ok(ctx) => ctx,
            Err(error) => return Err(error_value(error)),
        };
        // A panic inside a previous command poisons the lock: the in-memory
        // state may be inconsistent, so every later command fails closed.
        let mut guard = ctx
            .lock()
            .map_err(|_| error_value(&G9Error::internal_redacted("bridge.state_poisoned")))?;
        let outcome =
            g9_app::dispatch(&mut guard, op, meta, payload).map_err(|e| error_value(&e))?;
        drop(guard);
        Ok(self.with_status_warnings(op, outcome))
    }

    fn with_status_warnings(&self, op: &str, mut outcome: Value) -> Value {
        if op != "system.status" || self.warnings.is_empty() {
            return outcome;
        }
        if let Some(envelope) = outcome.as_object_mut() {
            let entry = envelope
                .entry("warnings")
                .or_insert_with(|| Value::Array(Vec::new()));
            if let Some(list) = entry.as_array_mut() {
                list.extend(self.warnings.iter().cloned().map(Value::String));
            }
        }
        outcome
    }
}

/// The Tauri-managed state: managed before any window can call, filled once.
#[derive(Debug, Clone, Default)]
pub struct AppState {
    slot: Arc<OnceLock<Bridge>>,
}

impl AppState {
    /// State before `setup()` has bootstrapped the backend.
    pub fn starting() -> Self {
        Self::default()
    }

    /// Install the bootstrapped backend (exactly once).
    pub fn install(&self, bridge: Bridge) -> Result<(), G9Error> {
        self.slot.set(bridge).map_err(|_| {
            G9Error::conflict("bridge.already_started", "the backend was already started")
        })
    }

    /// The installed backend, if any.
    pub fn bridge(&self) -> Option<&Bridge> {
        self.slot.get()
    }

    /// Handle one IPC call (`Err` is a serialized `G9Error`).
    pub fn handle(&self, op: &str, meta: Value, payload: Value) -> Result<Value, Value> {
        match self.slot.get() {
            Some(bridge) => bridge.handle(op, meta, payload),
            None => Err(error_value(&G9Error::resource_unavailable(
                "bridge.starting",
                "the backend is still starting; retry",
            ))),
        }
    }
}

/// Bootstrap with the OS CSPRNG. Never fails: a startup error is kept in the
/// returned [`Bridge`] and answered to every command.
pub fn bootstrap(options: &BridgeOptions) -> Bridge {
    bootstrap_with(options, &mut keyring::os_random)
}

/// [`bootstrap`] with an injected random source (deterministic tests).
pub fn bootstrap_with(
    options: &BridgeOptions,
    random: &mut dyn FnMut(&mut [u8]) -> Result<(), G9Error>,
) -> Bridge {
    let mut warnings = Vec::new();
    let ctx = open_context(options, random, &mut warnings).map(Mutex::new);
    Bridge { ctx, warnings }
}

fn open_context(
    options: &BridgeOptions,
    random: &mut dyn FnMut(&mut [u8]) -> Result<(), G9Error>,
    warnings: &mut Vec<String>,
) -> Result<AppContext, G9Error> {
    let layout = DataLayout::under(&options.data_dir)?;
    layout.prepare(warnings)?;
    let provisioned = keyring::provision(&layout.root, random)?;
    warnings.extend(provisioned.warnings);

    let mut config = AppConfig::new(APP_VERSION)?;
    match &options.zone {
        Some(zone) => config.zone = zone.clone(),
        None => warnings.push(keyring::warning(
            "clock.zone_fallback",
            "the host time zone could not be determined; civil dates use UTC",
        )),
    }
    config.backup_dir = Some(layout.backups.clone());
    config.scratch_dir = Some(layout.scratch.clone());
    // `g9-app` requires the backup and scratch directories inside an approved
    // root; the attachments root stays primary (relative paths resolve there).
    config.approved_roots = vec![
        layout.attachments.clone(),
        layout.backups.clone(),
        layout.scratch.clone(),
    ];
    config.transport = Transport::Tauri;

    let db = Database::open(&layout.database, OpenOptions::new(APP_VERSION))?;
    let operator = ActorId::parse(OPERATOR_ID)
        .map_err(|_| G9Error::internal_redacted("bridge.operator_id"))?;
    let mut builder = AppContext::builder(db, config, provisioned.key, operator);
    if let Some(registry) = load_skills(options.skills_dir.as_deref(), warnings) {
        builder = builder.skills(registry);
    }
    let ctx = builder.open()?;

    let report = ctx.recovery_report();
    if let Some(reason) = report.write_block_reason() {
        warnings.push(keyring::warning(
            "storage.read_only",
            &format!("recovery refused writes ({reason}); see Settings/Recovery"),
        ));
    }
    if let Some(reason) = &report.migration_blocked_reason {
        warnings.push(keyring::warning("storage.migration_blocked", reason));
    }
    Ok(ctx)
}

/// Load the bundled skill packages. A refused registry leaves skills disabled
/// (`skills.invoke` answers `RESOURCE_UNAVAILABLE`) and is reported.
fn load_skills(dir: Option<&Path>, warnings: &mut Vec<String>) -> Option<Registry> {
    let dir = dir?;
    match Registry::load(dir) {
        Ok(registry) => {
            warnings.push(keyring::warning(
                "skills.registry_unpinned",
                &format!(
                    "{} skill package(s) loaded and validated; package hashes are not yet \
                     compared with a pinned manifest (PROPOSED)",
                    registry.len()
                ),
            ));
            Some(registry)
        }
        Err(error) => {
            let error = G9Error::from(error);
            warnings.push(keyring::warning(
                "skills.registry_refused",
                &format!(
                    "bundled skills were refused ({}); skills are disabled",
                    error.code
                ),
            ));
            None
        }
    }
}

/// Resolve an IANA zone from host hints, in order: `TZ` (a name, `:name` or a
/// zoneinfo path), the `/etc/localtime` link target, `/etc/timezone`.
pub fn zone_from_hints(
    tz: Option<&str>,
    localtime_target: Option<&Path>,
    etc_timezone: Option<&str>,
) -> Option<IanaZone> {
    let from_tz = tz.and_then(|value| {
        let value = value.trim().trim_start_matches(':');
        if value.starts_with('/') {
            zone_from_zoneinfo_path(value)
        } else {
            IanaZone::parse(value).ok()
        }
    });
    from_tz
        .or_else(|| {
            localtime_target
                .and_then(Path::to_str)
                .and_then(zone_from_zoneinfo_path)
        })
        .or_else(|| etc_timezone.and_then(|name| IanaZone::parse(name.trim()).ok()))
}

fn zone_from_zoneinfo_path(path: &str) -> Option<IanaZone> {
    let (_, name) = path.split_once("zoneinfo/")?;
    let name = name
        .strip_prefix("posix/")
        .or_else(|| name.strip_prefix("right/"))
        .unwrap_or(name);
    IanaZone::parse(name).ok()
}

/// Read the host zone hints (fixed, read-only system locations).
pub fn host_zone() -> Option<IanaZone> {
    let tz = std::env::var("TZ").ok();
    let localtime = fs::read_link("/etc/localtime").ok();
    let etc_timezone = fs::read_to_string("/etc/timezone").ok();
    zone_from_hints(tz.as_deref(), localtime.as_deref(), etc_timezone.as_deref())
}

#[cfg(unix)]
mod platform {
    use std::fs::{self, DirBuilder, OpenOptions};
    use std::io;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    use std::path::Path;

    /// Create `dir` (and missing parents) with mode `0700`; it must end up a directory.
    pub(super) fn ensure_private_dir(dir: &Path) -> io::Result<()> {
        DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
        if fs::metadata(dir)?.is_dir() {
            Ok(())
        } else {
            Err(io::Error::from(io::ErrorKind::InvalidInput))
        }
    }

    /// Remove group/other permission bits when present.
    pub(super) fn restrict(path: &Path, dir: bool) -> io::Result<()> {
        let mode = fs::metadata(path)?.permissions().mode();
        if mode & 0o077 == 0 {
            return Ok(());
        }
        let wanted = if dir { 0o700 } else { 0o600 };
        fs::set_permissions(path, fs::Permissions::from_mode(wanted))
    }

    /// Create an empty `0600` file unless the path exists (SQLite then opens
    /// it as an empty database and creates `-wal`/`-shm` with the same mode).
    pub(super) fn precreate_private_file(path: &Path) -> io::Result<()> {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
        {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(()),
            Err(e) => Err(e),
        }
    }
}

#[cfg(not(unix))]
mod platform {
    use std::fs;
    use std::io;
    use std::path::Path;

    pub(super) fn ensure_private_dir(dir: &Path) -> io::Result<()> {
        fs::create_dir_all(dir)
    }

    pub(super) fn restrict(_path: &Path, _dir: bool) -> io::Result<()> {
        Ok(())
    }

    pub(super) fn precreate_private_file(_path: &Path) -> io::Result<()> {
        Ok(())
    }
}

/// Tauri wiring. Everything above is Tauri-free.
mod shell {
    use super::{
        bootstrap, error_value, host_zone, AppState, Bridge, BridgeOptions, SKILLS_RESOURCE_DIR,
    };
    use g9_domain::G9Error;
    use serde_json::Value;
    use tauri::Manager;

    /// The only IPC command. `op`, `meta` and `payload` are untrusted webview
    /// input; `g9_app::dispatch` assigns the actor, validates the metadata and
    /// the payload schema before deserialization, and runs the pipeline. The
    /// `Err` value is a serialized `G9Error`. The work runs on the blocking
    /// pool so that long operations (backup, restore test) never stall the
    /// window's event loop.
    #[tauri::command]
    async fn g9_command(
        state: tauri::State<'_, AppState>,
        op: String,
        meta: Value,
        payload: Value,
    ) -> Result<Value, Value> {
        let state = state.inner().clone();
        tauri::async_runtime::spawn_blocking(move || state.handle(&op, meta, payload))
            .await
            .unwrap_or_else(|_| {
                Err(error_value(&G9Error::internal_redacted(
                    "bridge.worker_failed",
                )))
            })
    }

    /// Resolve host paths through Tauri's path resolver and bootstrap once.
    fn setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
        let state = app
            .try_state::<AppState>()
            .map(|state| state.inner().clone())
            .ok_or_else(|| G9Error::internal_redacted("bridge.state_missing"))?;
        let bridge = match app.path().app_data_dir() {
            Ok(data_dir) => {
                let skills_dir = app
                    .path()
                    .resource_dir()
                    .ok()
                    .map(|dir| dir.join(SKILLS_RESOURCE_DIR))
                    .filter(|dir| dir.is_dir());
                let options = BridgeOptions {
                    data_dir,
                    skills_dir,
                    zone: host_zone(),
                };
                bootstrap(&options)
            }
            Err(_) => Bridge::unavailable(
                G9Error::resource_unavailable(
                    "bridge.data_dir_unresolved",
                    "the application data directory could not be resolved",
                ),
                Vec::new(),
            ),
        };
        state.install(bridge)?;
        Ok(())
    }

    /// Start the desktop shell: one command, no plugins.
    pub fn run() {
        let result = tauri::Builder::default()
            .manage(AppState::starting())
            .invoke_handler(tauri::generate_handler![g9_command])
            .setup(setup)
            .run(tauri::generate_context!());
        if let Err(error) = result {
            eprintln!("g9-mentor: the desktop shell stopped with an error: {error}");
            std::process::exit(1);
        }
    }
}

pub use shell::run;

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::keyring::KEY_FILE_NAME;
    use g9_domain::ErrorCategory;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    /// Deterministic byte source.
    fn counter() -> impl FnMut(&mut [u8]) -> Result<(), G9Error> {
        let mut next: u8 = 0;
        move |buf: &mut [u8]| {
            for b in buf.iter_mut() {
                *b = next;
                next = next.wrapping_add(7);
            }
            Ok(())
        }
    }

    fn berlin() -> IanaZone {
        IanaZone::parse("Europe/Berlin").unwrap()
    }

    fn options(dir: &Path) -> BridgeOptions {
        BridgeOptions {
            data_dir: dir.join("data"),
            skills_dir: None,
            zone: Some(berlin()),
        }
    }

    fn start_with(options: &BridgeOptions) -> AppState {
        let state = AppState::starting();
        state
            .install(bootstrap_with(options, &mut counter()))
            .unwrap();
        state
    }

    fn start(dir: &Path) -> AppState {
        start_with(&options(dir))
    }

    fn scope(op: &str) -> (&'static str, &'static str) {
        match op {
            "system.status" | "approvals.list" | "approvals.decide" | "audit.recent"
            | "export.request" => ("SYSTEM", "STUDY"),
            _ => ("LEARNING", "STUDY"),
        }
    }

    /// Wire metadata as `apps/desktop/src/ipc/client.ts` builds it (no actor).
    fn meta(op: &str, n: &str, approval_id: Option<&str>) -> Value {
        let (domain, mode) = scope(op);
        let mut m = json!({
            "request_id": format!("req-{n}"),
            "operation": op,
            "idempotency_key": format!("idem-{n}"),
            "scope": { "domain": domain, "mode": mode },
            "payload_schema_version": { "major": 1, "minor": 0 },
            "trace_id": format!("trace-{n}"),
            "issued_at": 1_795_336_200_000_i64,
        });
        if let Some(id) = approval_id {
            m["approval_id"] = json!(id);
        }
        m
    }

    fn call(state: &AppState, op: &str, n: &str, payload: Value) -> Result<Value, Value> {
        let out = state.handle(op, meta(op, n, None), payload);
        assert_schema(op, &out);
        out
    }

    /// Every bridge answer conforms to the IPC schemas the UI is built against.
    fn assert_schema(op: &str, out: &Result<Value, Value>) {
        match out {
            Ok(outcome) => {
                g9_app::schemas::validate_outcome(outcome).unwrap();
                g9_app::schemas::validate_response(op, &outcome["result"]).unwrap();
            }
            Err(error) => g9_app::schemas::validate_error(error).unwrap(),
        }
    }

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    fn export_payload() -> Value {
        json!({ "domains": ["LEARNING"], "format": "JSON", "include_private": false })
    }

    fn approvals_in(state: &AppState, n: &str, wanted: &str) -> Vec<String> {
        let list = call(state, "approvals.list", n, json!({ "state": wanted })).unwrap();
        list["result"]["approvals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["approval_id"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn bootstrap_creates_private_layout_and_status_is_honest() {
        let dir = tempfile::tempdir().unwrap();
        let state = start(dir.path());
        let bridge = state.bridge().unwrap();
        assert!(
            bridge.startup_error().is_none(),
            "{:?}",
            bridge.startup_error()
        );
        let data = dir.path().join("data");
        for sub in ["", ATTACHMENTS_DIR, BACKUPS_DIR, SCRATCH_DIR] {
            assert_eq!(mode(&data.join(sub)), 0o700, "{sub}");
        }
        assert_eq!(mode(&data.join(DATABASE_FILE)), 0o600);
        assert_eq!(mode(&data.join(KEY_FILE_NAME)), 0o600);
        assert_eq!(fs::read(data.join(KEY_FILE_NAME)).unwrap().len(), 32);

        let status = call(&state, "system.status", "1", json!({})).unwrap();
        let r = &status["result"];
        assert_eq!(r["transport"], "tauri");
        assert_eq!(r["app_version"], APP_VERSION);
        assert_eq!(r["storage"]["state"], "READY");
        assert_eq!(r["storage"]["wal"], true);
        assert_eq!(r["model"]["available"], false);
        assert_eq!(r["model"]["reason"], "NO_RUNTIME_CONFIGURED");
        assert_eq!(r["network"]["egress_allowed"], false);
        assert_eq!(r["clock"]["zone"], "Europe/Berlin");
        assert_eq!(r["pending_approvals"], 0);
        let warnings = status["warnings"].as_array().unwrap();
        assert!(warnings.iter().any(|w| w
            .as_str()
            .unwrap()
            .starts_with("WARNING binding_key.file_fallback:")));
        assert_eq!(status["replayed"], false);
    }

    #[test]
    fn status_warnings_are_added_only_to_system_status() {
        let dir = tempfile::tempdir().unwrap();
        let state = start(dir.path());
        let list = call(&state, "approvals.list", "1", json!({})).unwrap();
        assert!(list.get("warnings").is_none(), "{list}");
    }

    #[test]
    fn ui_supplied_actor_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let state = start(dir.path());
        let mut m = meta("system.status", "1", None);
        m["actor"] = json!({ "id": "operator-x", "kind": { "kind": "OPERATOR" } });
        let out = state.handle("system.status", m, json!({}));
        assert_schema("system.status", &out);
        let err: G9Error = serde_json::from_value(out.unwrap_err()).unwrap();
        assert_eq!(err.category, ErrorCategory::InvalidInput);
        assert_eq!(err.code, "meta.actor_not_accepted");
    }

    #[test]
    fn errors_cross_as_serialized_g9error_objects() {
        let dir = tempfile::tempdir().unwrap();
        let state = start(dir.path());
        let err = state
            .handle("shell.exec", meta("shell.exec", "1", None), json!({}))
            .unwrap_err();
        g9_app::schemas::validate_error(&err).unwrap();
        let keys: Vec<&String> = err.as_object().unwrap().keys().collect();
        assert!(keys.iter().all(
            |k| ["category", "code", "message", "details", "approval_id"].contains(&k.as_str())
        ));
        assert_eq!(err["category"], "INVALID_INPUT");
        assert_eq!(err["code"], "ipc.unknown_operation");
        // A malformed payload is refused by schema validation before any handler runs.
        let bad = call(&state, "approvals.decide", "2", json!({ "approval_id": 5 })).unwrap_err();
        assert_eq!(bad["category"], "INVALID_INPUT");
    }

    #[test]
    fn approval_granted_before_restart_is_consumed_once_after_restart() {
        let dir = tempfile::tempdir().unwrap();
        let first = start(dir.path());
        let err = call(&first, "export.request", "1", export_payload()).unwrap_err();
        assert_eq!(err["category"], "APPROVAL_REQUIRED");
        let id = err["approval_id"].as_str().unwrap().to_string();
        let decided = call(
            &first,
            "approvals.decide",
            "2",
            json!({ "approval_id": id, "decision": "APPROVE" }),
        )
        .unwrap();
        assert_eq!(decided["result"]["state"], "APPROVED");
        drop(first);

        // Process restart: same data directory, same file key.
        let second = start(dir.path());
        let retry = second.handle(
            "export.request",
            meta("export.request", "3", Some(&id)),
            export_payload(),
        );
        assert_schema("export.request", &retry);
        let retry = retry.unwrap();
        assert_eq!(retry["result"]["state"], "REQUESTED");
        assert_eq!(approvals_in(&second, "4", "CONSUMED"), vec![id.clone()]);
        // Single use: a second retry with the consumed approval is refused.
        let again = second
            .handle(
                "export.request",
                meta("export.request", "5", Some(&id)),
                export_payload(),
            )
            .unwrap_err();
        g9_app::schemas::validate_error(&again).unwrap();
        assert_ne!(again["category"], "APPROVAL_REQUIRED");
    }

    #[test]
    fn replaced_binding_key_invalidates_granted_approval() {
        let dir = tempfile::tempdir().unwrap();
        let first = start(dir.path());
        let err = call(&first, "export.request", "1", export_payload()).unwrap_err();
        let id = err["approval_id"].as_str().unwrap().to_string();
        call(
            &first,
            "approvals.decide",
            "2",
            json!({ "approval_id": id, "decision": "APPROVE" }),
        )
        .unwrap();
        drop(first);
        fs::remove_file(dir.path().join("data").join(KEY_FILE_NAME)).unwrap();

        // A different random stream gives a different key.
        let state = AppState::starting();
        let mut other = |buf: &mut [u8]| {
            buf.fill(0x5a);
            Ok(())
        };
        state
            .install(bootstrap_with(&options(dir.path()), &mut other))
            .unwrap();
        let refused = state
            .handle(
                "export.request",
                meta("export.request", "3", Some(&id)),
                export_payload(),
            )
            .unwrap_err();
        g9_app::schemas::validate_error(&refused).unwrap();
        assert_eq!(refused["category"], "UNAUTHORIZED", "{refused}");
        assert!(approvals_in(&state, "4", "CONSUMED").is_empty());
    }

    #[test]
    fn approval_is_not_usable_for_different_parameters() {
        let dir = tempfile::tempdir().unwrap();
        let state = start(dir.path());
        let err = call(&state, "export.request", "1", export_payload()).unwrap_err();
        let id = err["approval_id"].as_str().unwrap().to_string();
        call(
            &state,
            "approvals.decide",
            "2",
            json!({ "approval_id": id, "decision": "APPROVE" }),
        )
        .unwrap();
        // Same approval, wider export: refused, and the approval is not spent.
        let wider = json!({
            "domains": ["LEARNING", "PERSONAL"],
            "format": "JSON",
            "include_private": true,
        });
        let refused = state.handle(
            "export.request",
            meta("export.request", "3", Some(&id)),
            wider,
        );
        assert_schema("export.request", &refused);
        let refused = refused.unwrap_err();
        assert_eq!(refused["category"], "UNAUTHORIZED", "{refused}");
        assert_eq!(approvals_in(&state, "4", "APPROVED"), vec![id.clone()]);
        assert!(approvals_in(&state, "5", "CONSUMED").is_empty());
        // The approved parameters still succeed exactly once.
        let ok = state
            .handle(
                "export.request",
                meta("export.request", "6", Some(&id)),
                export_payload(),
            )
            .unwrap();
        assert_eq!(ok["result"]["state"], "REQUESTED");
        assert_eq!(approvals_in(&state, "7", "CONSUMED"), vec![id]);
    }

    #[test]
    fn idempotent_retry_replays_and_changed_retry_conflicts() {
        let dir = tempfile::tempdir().unwrap();
        let state = start(dir.path());
        let note = json!({
            "title": "Bridge note",
            "body_markdown": "# heading",
            "classification": "INTERNAL",
            "domain": "LEARNING",
        });
        let first = call(&state, "notes.create", "1", note.clone()).unwrap();
        assert_eq!(first["replayed"], false);
        // Same idempotency key and request: the stored outcome is replayed.
        let again = call(&state, "notes.create", "1", note.clone()).unwrap();
        assert_eq!(again["replayed"], true);
        assert_eq!(again["result"], first["result"]);
        // Same key, different request: refused, nothing new is written.
        let mut changed = note;
        changed["title"] = json!("Another title");
        let conflict = call(&state, "notes.create", "1", changed).unwrap_err();
        assert_eq!(conflict["category"], "CONFLICT", "{conflict}");
        // The replay survives a restart (idempotency records are persisted).
        drop(state);
        let restarted = start(dir.path());
        let after = call(
            &restarted,
            "notes.create",
            "1",
            json!({
                "title": "Bridge note",
                "body_markdown": "# heading",
                "classification": "INTERNAL",
                "domain": "LEARNING",
            }),
        )
        .unwrap();
        assert_eq!(after["replayed"], true);
        assert_eq!(after["result"], first["result"]);
    }

    #[test]
    fn startup_failure_is_answered_to_every_command_without_paths() {
        let dir = tempfile::tempdir().unwrap();
        // Relative data directory.
        let relative = start_with(&BridgeOptions {
            data_dir: PathBuf::from("relative/data"),
            skills_dir: None,
            zone: Some(berlin()),
        });
        let err = relative
            .handle("system.status", meta("system.status", "1", None), json!({}))
            .unwrap_err();
        g9_app::schemas::validate_error(&err).unwrap();
        assert_eq!(err["category"], "PRECONDITION_FAILED");
        assert_eq!(err["code"], "bridge.data_dir_not_absolute");
        // Data directory path occupied by a regular file.
        let blocked = dir.path().join("data");
        fs::write(&blocked, b"not a directory").unwrap();
        let state = start(dir.path());
        assert!(state.bridge().unwrap().startup_error().is_some());
        let err = state
            .handle("system.status", meta("system.status", "2", None), json!({}))
            .unwrap_err();
        assert_eq!(err["category"], "RESOURCE_UNAVAILABLE");
        assert_eq!(err["code"], "bridge.data_dir_unavailable");
        let text = err.to_string();
        assert!(
            !text.contains(&dir.path().to_string_lossy().to_string()),
            "{text}"
        );
    }

    #[test]
    fn starting_state_answers_resource_unavailable() {
        let state = AppState::starting();
        let err = state
            .handle("system.status", meta("system.status", "1", None), json!({}))
            .unwrap_err();
        g9_app::schemas::validate_error(&err).unwrap();
        assert_eq!(err["category"], "RESOURCE_UNAVAILABLE");
        assert_eq!(err["code"], "bridge.starting");
    }

    #[test]
    fn second_install_is_refused() {
        let state = AppState::starting();
        state
            .install(Bridge::unavailable(
                G9Error::internal_redacted("x"),
                Vec::new(),
            ))
            .unwrap();
        let err = state
            .install(Bridge::unavailable(
                G9Error::internal_redacted("y"),
                Vec::new(),
            ))
            .unwrap_err();
        assert_eq!(err.category, ErrorCategory::Conflict);
        assert_eq!(
            state.bridge().unwrap().startup_error().unwrap().code,
            "x",
            "the first bridge stays installed"
        );
    }

    #[test]
    fn poisoned_state_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let state = start(dir.path());
        let ctx = state.bridge().unwrap().ctx.as_ref().unwrap();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = ctx.lock().unwrap();
            panic!("simulated handler panic");
        }));
        let err = call(&state, "system.status", "1", json!({})).unwrap_err();
        assert_eq!(err["category"], "INTERNAL");
        assert_eq!(err["code"], "bridge.state_poisoned");
    }

    #[test]
    fn permissive_existing_data_dir_is_restricted() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        fs::create_dir_all(&data).unwrap();
        fs::set_permissions(&data, fs::Permissions::from_mode(0o755)).unwrap();
        let state = start(dir.path());
        assert!(state.bridge().unwrap().startup_error().is_none());
        assert_eq!(mode(&data), 0o700);
    }

    #[test]
    fn missing_zone_falls_back_to_utc_with_warning() {
        let dir = tempfile::tempdir().unwrap();
        let state = start_with(&BridgeOptions {
            zone: None,
            ..options(dir.path())
        });
        let status = call(&state, "system.status", "1", json!({})).unwrap();
        assert_eq!(status["result"]["clock"]["zone"], "UTC");
        assert!(status["warnings"].as_array().unwrap().iter().any(|w| w
            .as_str()
            .unwrap()
            .starts_with("WARNING clock.zone_fallback:")));
    }

    #[test]
    fn bundled_skills_load_and_are_reported_unpinned() {
        let dir = tempfile::tempdir().unwrap();
        let skills = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../.agents/skills");
        assert!(skills.is_dir(), "repository skills directory is missing");
        let state = start_with(&BridgeOptions {
            skills_dir: Some(skills),
            ..options(dir.path())
        });
        let warnings = state.bridge().unwrap().warnings();
        assert!(
            warnings
                .iter()
                .any(|w| w.starts_with("WARNING skills.registry_unpinned: 37 skill package(s)")),
            "{warnings:?}"
        );
        assert!(!warnings
            .iter()
            .any(|w| w.contains("skills.registry_refused")));
    }

    #[test]
    fn invalid_skills_dir_disables_skills_with_warning() {
        let dir = tempfile::tempdir().unwrap();
        let skills = dir.path().join("skills");
        fs::create_dir_all(skills.join("not-a-package")).unwrap();
        let state = start_with(&BridgeOptions {
            skills_dir: Some(skills),
            ..options(dir.path())
        });
        let bridge = state.bridge().unwrap();
        assert!(bridge.startup_error().is_none());
        assert!(bridge
            .warnings()
            .iter()
            .any(|w| w.starts_with("WARNING skills.registry_refused:")));
    }

    #[test]
    fn zone_hints_resolve_iana_names_in_order() {
        let name = |z: Option<IanaZone>| z.map(|z| z.as_str().to_string());
        assert_eq!(
            name(zone_from_hints(Some(":Europe/Berlin"), None, None)),
            Some("Europe/Berlin".into())
        );
        assert_eq!(
            name(zone_from_hints(
                Some("/usr/share/zoneinfo/America/New_York"),
                None,
                None
            )),
            Some("America/New_York".into())
        );
        assert_eq!(
            name(zone_from_hints(
                Some("CET-1CEST"),
                Some(Path::new("/usr/share/zoneinfo/posix/Australia/Sydney")),
                None
            )),
            Some("Australia/Sydney".into())
        );
        assert_eq!(
            name(zone_from_hints(None, None, Some("UTC\n"))),
            Some("UTC".into())
        );
        assert_eq!(
            name(zone_from_hints(
                Some("Mars/Olympus"),
                None,
                Some("nonsense")
            )),
            None
        );
    }

    #[test]
    fn app_state_is_send_sync_and_static() {
        fn assert_bounds<T: Send + Sync + 'static>() {}
        assert_bounds::<AppState>();
        assert_bounds::<Bridge>();
    }
}
