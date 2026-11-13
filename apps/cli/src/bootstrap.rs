//! Context construction, mirroring `apps/desktop/src-tauri/src/lib.rs`
//! (`DataLayout`, `open_context`): data layout `0700`/`0600`, binding key from
//! `<data>/binding.key` through [`g9_app::keyfile`], `Database::open` of
//! `<data>/g9.sqlite`, backup/scratch directories inside approved roots, the
//! skill registry when a skills directory exists, the backend-assigned operator
//! and `AppContext::builder(..).open()` (which runs `g9_app::recovery`).
//!
//! The inference runtime is the builder default (`NullRuntime`): the CLI never
//! configures a model, so `system.status` reports `model.available = false`.

use crate::clock::SimClock;
use crate::output::CliError;
use g9_app::{keyfile, AppConfig, AppContext, Transport};
use g9_curriculum::CivilDate;
use g9_domain::{ActorId, G9Error, IanaZone};
use g9_skills::Registry;
use g9_storage::Database;
use std::fs;
use std::path::{Path, PathBuf};

/// Backend-assigned operator actor id. Equal to the Tauri bridge's
/// `OPERATOR_ID` so that a data directory used by both front-ends keeps one
/// attributable operator (idempotency records and approvals are keyed by it);
/// `--operator-id` overrides it for tests.
pub const OPERATOR_ID: &str = "local-operator";
/// Application version recorded in migrations, backups and `system.status`.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// Database file inside the data directory.
pub const DATABASE_FILE: &str = "g9.sqlite";
/// Primary approved root (attachments).
pub const ATTACHMENTS_DIR: &str = "attachments";
/// Backup directory (inside an approved root, as `g9-app` requires).
pub const BACKUPS_DIR: &str = "backups";
/// Restore-test and migration dry-run scratch directory.
pub const SCRATCH_DIR: &str = "scratch";
/// Where `g9 restore` moves the replaced database and its sidecars (never deleted).
pub const QUARANTINE_DIR: &str = "quarantine";
/// Append-only, metadata-only log of `g9 restore` runs.
pub const RESTORE_LOG_FILE: &str = "restore-log.jsonl";
/// Default data directory name under `$XDG_DATA_HOME` / `~/.local/share`.
pub const DEFAULT_DATA_DIR_NAME: &str = "g9-mentor";

/// `$XDG_DATA_HOME/g9-mentor`, else `~/.local/share/g9-mentor`.
pub fn default_data_dir() -> Result<PathBuf, CliError> {
    if let Some(xdg) = std::env::var_os("XDG_DATA_HOME").filter(|v| !v.is_empty()) {
        let base = PathBuf::from(xdg);
        if base.is_absolute() {
            return Ok(base.join(DEFAULT_DATA_DIR_NAME));
        }
    }
    let home = std::env::var_os("HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| {
            CliError::Usage(
                "no data directory: pass --data-dir or set XDG_DATA_HOME or HOME".to_string(),
            )
        })?;
    Ok(home
        .join(".local")
        .join("share")
        .join(DEFAULT_DATA_DIR_NAME))
}

/// Repository root at compile time (`apps/cli` → two levels up).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `<repo>/.agents/skills` when it exists, else `None`.
pub fn default_skills_dir() -> Option<PathBuf> {
    let dir = repo_root().join(".agents").join("skills");
    dir.is_dir().then_some(dir)
}

/// Default seed manifest shipped in the repository.
pub fn default_seed_manifest() -> PathBuf {
    repo_root()
        .join("data")
        .join("curriculum")
        .join("g9-ct9-480.seed.v0.json")
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

/// The data directory layout (same names as the bridge).
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
    /// Create the root if needed and build the layout over its canonical path
    /// (`g9-app` requires absolute, normalized approved roots).
    pub fn prepare(requested: &Path) -> Result<Self, CliError> {
        let root = prepare_root(requested)?;
        let layout = Self {
            database: root.join(DATABASE_FILE),
            attachments: root.join(ATTACHMENTS_DIR),
            backups: root.join(BACKUPS_DIR),
            scratch: root.join(SCRATCH_DIR),
            root,
        };
        for dir in [&layout.attachments, &layout.backups, &layout.scratch] {
            platform::ensure_private_dir(dir).map_err(|e| CliError::io("data directory", &e))?;
        }
        for dir in [
            &layout.root,
            &layout.attachments,
            &layout.backups,
            &layout.scratch,
        ] {
            platform::restrict(dir, true).map_err(|e| CliError::io("data directory", &e))?;
        }
        platform::precreate_private_file(&layout.database)
            .map_err(|e| CliError::io("database file", &e))?;
        for suffix in ["", "-wal", "-shm"] {
            let mut name = layout.database.clone().into_os_string();
            name.push(suffix);
            let file = PathBuf::from(name);
            if file.exists() {
                platform::restrict(&file, false).map_err(|e| CliError::io("database file", &e))?;
            }
        }
        Ok(layout)
    }
}

/// `requested` made absolute against the working directory (no filesystem access
/// beyond the working directory lookup).
pub fn absolute_path(requested: &Path) -> Result<PathBuf, CliError> {
    if requested.is_absolute() {
        Ok(requested.to_path_buf())
    } else {
        Ok(std::env::current_dir()
            .map_err(|e| CliError::io("current directory", &e))?
            .join(requested))
    }
}

/// Create the data directory root when needed (`0700`) and return its canonical path.
pub fn prepare_root(requested: &Path) -> Result<PathBuf, CliError> {
    let absolute = absolute_path(requested)?;
    platform::ensure_private_dir(&absolute).map_err(|e| CliError::io("data directory", &e))?;
    fs::canonicalize(&absolute).map_err(|e| CliError::io("data directory", &e))
}

/// Create (when needed) and append one line to a `0600` log file, then sync it.
pub fn append_private_line(path: &Path, line: &str) -> std::io::Result<()> {
    platform::append_private_line(path, line)
}

/// Everything needed to open the backend.
#[derive(Debug, Clone)]
pub struct BootOptions {
    /// Data directory (absolute or relative to the working directory).
    pub data_dir: PathBuf,
    /// Skill packages directory; `None` disables skills.
    pub skills_dir: Option<PathBuf>,
    /// Operator time zone; `None` falls back to UTC with a warning.
    pub zone: Option<IanaZone>,
    /// Monday of program week 1 (needed by `curriculum.daily_plan`).
    pub program_start: Option<CivilDate>,
    /// Backend-assigned operator actor id.
    pub operator_id: String,
    /// Deterministic clock; `None` uses the host clock.
    pub clock: Option<SimClock>,
}

impl BootOptions {
    /// Host clock, host zone, default skills, bridge operator id.
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            skills_dir: default_skills_dir(),
            zone: host_zone(),
            program_start: None,
            operator_id: OPERATOR_ID.to_string(),
            clock: None,
        }
    }
}

/// Skill registry load outcome (reported, never assumed).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SkillsStatus {
    /// Directory that was requested, if any.
    pub dir: Option<PathBuf>,
    /// Packages loaded and validated (`None` when skills are disabled).
    pub packages: Option<usize>,
    /// Refusal code when the registry was refused.
    pub refused: Option<String>,
}

/// An opened backend.
#[derive(Debug)]
pub struct Booted {
    /// The application context (recovery has run).
    pub ctx: AppContext,
    /// Layout in use.
    pub layout: DataLayout,
    /// Warnings in the bridge's `WARNING <code>: <text>` form (secret-free).
    pub warnings: Vec<String>,
    /// Skill registry outcome.
    pub skills: SkillsStatus,
    /// Zone in use.
    pub zone: IanaZone,
}

/// Open the backend exactly as the bridge does.
pub fn open(opts: &BootOptions) -> Result<Booted, CliError> {
    let layout = DataLayout::prepare(&opts.data_dir)?;
    let mut warnings = Vec::new();
    let provisioned = keyfile::provision(&layout.root, &mut keyfile::os_random)?;
    warnings.extend(provisioned.warnings);

    let mut config = AppConfig::new(APP_VERSION)?;
    let zone = match &opts.zone {
        Some(zone) => zone.clone(),
        None => {
            warnings.push(keyfile::warning(
                "clock.zone_fallback",
                "the host time zone could not be determined; civil dates use UTC",
            ));
            IanaZone::utc()
        }
    };
    config.zone = zone.clone();
    config.program_start = opts.program_start;
    config.backup_dir = Some(layout.backups.clone());
    config.scratch_dir = Some(layout.scratch.clone());
    // Same list as the desktop bridge: the attachments directory is the
    // primary root (relative attachment paths resolve there) and the backup and
    // scratch directories are roots of their own. The data directory itself is
    // deliberately NOT a root: it holds `binding.key` and the database, which
    // must stay outside the attachment-read boundary. Directory checks use
    // `ApprovedRoots::contains_directory`, which accepts a root itself.
    config.approved_roots = vec![
        layout.attachments.clone(),
        layout.backups.clone(),
        layout.scratch.clone(),
    ];
    // `g9_app::Transport` has a single variant; `system.status.transport`
    // therefore reads `tauri` from the CLI as well (reported in the testing guide).
    config.transport = Transport::Tauri;

    let open_options = match &opts.clock {
        Some(clock) => g9_app::database_options(APP_VERSION).applied_at(clock.now()),
        None => g9_app::database_options(APP_VERSION),
    };
    let db = Database::open(&layout.database, open_options).map_err(G9Error::from)?;
    let operator = ActorId::parse(&opts.operator_id).map_err(|_| {
        CliError::Usage(format!(
            "operator id {:?} is not a valid identifier ([A-Za-z0-9._:-]{{1,128}})",
            opts.operator_id
        ))
    })?;
    let mut builder = AppContext::builder(db, config, provisioned.key, operator);
    if let Some(clock) = &opts.clock {
        builder = builder.clock(clock.wall_fn()).monotonic(clock.mono_fn());
    }
    let (registry, skills) = load_skills(opts.skills_dir.as_deref(), &mut warnings);
    if let Some(registry) = registry {
        builder = builder.skills(registry);
    }
    let ctx = builder.open()?;

    let report = ctx.recovery_report();
    if let Some(reason) = report.write_block_reason() {
        warnings.push(keyfile::warning(
            "storage.read_only",
            &format!("recovery refused writes ({reason}); see docs/runbooks/backup-restore.md"),
        ));
    }
    if let Some(reason) = &report.migration_blocked_reason {
        warnings.push(keyfile::warning("storage.migration_blocked", reason));
    }
    Ok(Booted {
        ctx,
        layout,
        warnings,
        skills,
        zone,
    })
}

/// Load the skill packages. A refused registry leaves skills disabled
/// (`skills.invoke` answers `RESOURCE_UNAVAILABLE`) and is reported.
fn load_skills(dir: Option<&Path>, warnings: &mut Vec<String>) -> (Option<Registry>, SkillsStatus) {
    let Some(dir) = dir else {
        return (
            None,
            SkillsStatus {
                dir: None,
                packages: None,
                refused: None,
            },
        );
    };
    match Registry::load(dir) {
        Ok(registry) => {
            let packages = registry.len();
            warnings.push(keyfile::warning(
                "skills.registry_unpinned",
                &format!(
                    "{packages} skill package(s) loaded and validated; package hashes are not yet \
                     compared with a pinned manifest (PROPOSED)"
                ),
            ));
            (
                Some(registry),
                SkillsStatus {
                    dir: Some(dir.to_path_buf()),
                    packages: Some(packages),
                    refused: None,
                },
            )
        }
        Err(error) => {
            let error = G9Error::from(error);
            warnings.push(keyfile::warning(
                "skills.registry_refused",
                &format!(
                    "skill packages were refused ({}); skills are disabled",
                    error.code
                ),
            ));
            (
                None,
                SkillsStatus {
                    dir: Some(dir.to_path_buf()),
                    packages: None,
                    refused: Some(error.code),
                },
            )
        }
    }
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

    /// Append `line` plus a newline to `path`, creating it `0600`; the data is synced.
    pub(super) fn append_private_line(path: &Path, line: &str) -> io::Result<()> {
        use std::io::Write;
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()
    }

    /// Create an empty `0600` file unless the path exists.
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

    pub(super) fn append_private_line(path: &Path, line: &str) -> io::Result<()> {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)?;
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zone_hints_resolve_in_order() {
        let name = |z: Option<IanaZone>| z.map(|z| z.as_str().to_string());
        assert_eq!(
            name(zone_from_hints(Some(":Europe/Berlin"), None, None)),
            Some("Europe/Berlin".into())
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
            name(zone_from_hints(Some("Mars/Olympus"), None, None)),
            None
        );
    }

    #[test]
    fn default_seed_manifest_is_in_the_repository() {
        assert!(default_seed_manifest().is_file());
        assert_eq!(
            default_skills_dir().is_some(),
            repo_root().join(".agents/skills").is_dir()
        );
    }

    #[cfg(unix)]
    #[test]
    fn layout_is_private_and_canonical() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let layout = DataLayout::prepare(&dir.path().join("data")).unwrap();
        assert!(layout.root.is_absolute());
        for d in [
            &layout.root,
            &layout.attachments,
            &layout.backups,
            &layout.scratch,
        ] {
            assert_eq!(fs::metadata(d).unwrap().permissions().mode() & 0o777, 0o700);
        }
        assert_eq!(
            fs::metadata(&layout.database).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
