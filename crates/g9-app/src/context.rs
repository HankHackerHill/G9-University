//! Application context: everything a command needs, owned in one place
//! (ARCHITECTURE.md §2 rule 5: `g9-app` is the only crate that composes policy,
//! storage and skills).
//!
//! * `db` — the verified [`Database`]; every SQL statement lives in `g9-storage`.
//! * `policy` — the deny-by-default [`Engine`] (built-in rule table), sharing the
//!   context clock.
//! * `approvals` — an in-memory [`ApprovalLedger`] hydrated from
//!   `approval_requests` on open and re-synchronised from the database whenever a
//!   command touches an approval (the database row is the source of truth; the
//!   ledger performs the MAC/binding checks of `verify_and_consume`).
//! * `binding_key` — the per-installation HMAC key, supplied by the bridge from
//!   the OS keyring or a `0600` file; never serialized, never logged.
//! * `runtime` — the inference runtime ([`NullRuntime`] unless the bridge
//!   configures one).
//! * `skills` — the validated skill registry, or `None` (then `skills.invoke`
//!   returns `RESOURCE_UNAVAILABLE`).
//! * `clock` / `monotonic` — injected time sources (tests use fixed/stepping clocks).
//! * `operator` — the backend-assigned actor; IPC metadata can never set it.

use crate::recovery::{self, RecoveryReport};
use g9_ai_adapter::{InferenceRuntime, NullRuntime};
use g9_domain::{
    Actor, ActorId, ApprovalId, ApprovalState, AuditRef, BackupId, CurriculumId, G9Error, IanaZone,
    Revision, UtcMillis,
};
use g9_evidence::ApprovedRoots;
use g9_policy::{ApprovalLedger, ApprovalRecord, ApprovalRequest, BindingKey, Engine};
use g9_skills::Registry;
use g9_storage::approvals_store::{self, ApprovalRequestRow};
use g9_storage::{BackupManifest, Database};
use rusqlite::Connection;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Wall-clock source (`UtcMillis`).
pub type WallClockFn = Arc<dyn Fn() -> UtcMillis + Send + Sync>;
/// Monotonic clock source in milliseconds; only comparable within one process lifetime.
pub type MonotonicFn = Arc<dyn Fn() -> u64 + Send + Sync>;

/// Static configuration supplied by the bridge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    /// Application version recorded in status, migrations and backup manifests.
    pub app_version: String,
    /// Operator time zone for civil dates (daily plan, session dates).
    pub zone: IanaZone,
    /// Curriculum the daily plan reads.
    pub curriculum_id: CurriculumId,
    /// Monday of program week 1 (operator setting); `None` until configured.
    pub program_start: Option<g9_curriculum::CivilDate>,
    /// Directory backups are written to (must lie inside `approved_roots`).
    pub backup_dir: Option<PathBuf>,
    /// Scratch directory for restore tests and migration dry runs (must lie
    /// inside `approved_roots`; a file database with pending migrations is not
    /// migrated without it).
    pub scratch_dir: Option<PathBuf>,
    /// Root of the live content-addressed attachment store (must lie inside
    /// `approved_roots`): the source every backup copies the referenced files
    /// from into `<backup_dir>/attachments`, where verification hashes them.
    /// Without it a snapshot that holds attachment rows fails verification.
    pub attachment_dir: Option<PathBuf>,
    /// Directory `export.request` writes exports to (must lie inside
    /// `approved_roots`); without it an approved export is refused.
    pub export_dir: Option<PathBuf>,
    /// Host directories the backend may write to.
    pub approved_roots: Vec<PathBuf>,
    /// Where a backup reads the free space of the backup directory's
    /// filesystem before it starts (STOR-4): `statvfs` by default; a fixed
    /// reading in tests and drills.
    pub free_space: g9_storage::backup::FreeSpaceSource,
    /// Reported `SystemStatus.transport`.
    pub transport: Transport,
    /// An approval is granted only through a confirmation the operator gives
    /// outside the webview (ADR-0014, SEC-03): [`crate::dispatch_confirming`]
    /// shows a summary Rust writes from the stored request (the desktop bridge:
    /// a native dialog). `true` by default (deny by default); the CLI, a terminal
    /// surface the operator types into, sets it to `false`.
    pub native_confirmation: bool,
}

impl AppConfig {
    /// Minimal configuration: UTC, the G9/CT9 curriculum id, no host directories.
    pub fn new(app_version: impl Into<String>) -> Result<Self, G9Error> {
        Ok(Self {
            app_version: app_version.into(),
            zone: IanaZone::utc(),
            curriculum_id: CurriculumId::parse("g9-ct9-480")
                .map_err(|_| G9Error::internal_redacted("config.curriculum_id"))?,
            program_start: None,
            backup_dir: None,
            scratch_dir: None,
            attachment_dir: None,
            export_dir: None,
            approved_roots: Vec::new(),
            free_space: g9_storage::backup::FreeSpaceSource::Filesystem,
            transport: Transport::Tauri,
            native_confirmation: true,
        })
    }
}

/// Which transport the UI is talking through (reported verbatim in `system.status`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transport {
    /// The Tauri bridge (the only production transport).
    Tauri,
}

impl Transport {
    /// Wire spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Transport::Tauri => "tauri",
        }
    }
}

/// A backup manifest as the commands act on it: the `backup_manifests` row
/// (read with `g9_storage::backup::read_manifest`) plus the bookkeeping that is
/// not stored (operator label, snapshot size from the file's metadata).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownBackup {
    /// The manifest (state machine + verification report).
    pub manifest: BackupManifest,
    /// `backup_manifests.revision` as maintained by `record_manifest`.
    pub revision: Revision,
    /// Operator label (not persisted by storage; `None` after a restart).
    pub label: Option<String>,
    /// Snapshot size in bytes, when known.
    pub size_bytes: Option<u64>,
}

/// In-memory approval ledger kept in step with `approval_requests`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApprovalBook {
    ledger: ApprovalLedger,
}

impl ApprovalBook {
    /// Hydrate every stored approval request.
    pub fn hydrate(conn: &Connection, operator: &Actor) -> Result<Self, G9Error> {
        let mut book = Self::default();
        for state in ApprovalState::ALL {
            for row in approvals_store::list_by_state(conn, *state)? {
                book.put(record_from_row(&row, operator))?;
            }
        }
        Ok(book)
    }

    /// Re-read one request from the database (inside or after the command transaction).
    pub fn refresh(
        &mut self,
        conn: &Connection,
        id: &ApprovalId,
        operator: &Actor,
    ) -> Result<(), G9Error> {
        match approvals_store::get_request(conn, id)? {
            Some(row) => self.put(record_from_row(&row, operator)),
            None => self.remove(id),
        }
    }

    /// The ledger (binding checks, consumption).
    pub fn ledger(&self) -> &ApprovalLedger {
        &self.ledger
    }

    /// Mutable ledger (the pipeline passes it to `g9_policy::ApprovalGate`).
    pub fn ledger_mut(&mut self) -> &mut ApprovalLedger {
        &mut self.ledger
    }

    /// Insert a pending request that was just persisted.
    pub fn insert_pending(&mut self, request: ApprovalRequest) -> Result<(), G9Error> {
        self.ledger.request(request)?;
        Ok(())
    }

    /// Upsert one record. `ApprovalLedger` exposes no record-level restore, so
    /// the snapshot JSON form (`ApprovalLedger::{to_json, from_json}`) is used.
    fn put(&mut self, record: ApprovalRecord) -> Result<(), G9Error> {
        let mut snapshot = self.snapshot()?;
        if let Some(records) = snapshot
            .get_mut("records")
            .and_then(serde_json::Value::as_object_mut)
        {
            let value = serde_json::to_value(&record)
                .map_err(|_| G9Error::internal_redacted("approvals.snapshot"))?;
            records.insert(record.request.id.as_str().to_string(), value);
        }
        self.restore(&snapshot)
    }

    fn remove(&mut self, id: &ApprovalId) -> Result<(), G9Error> {
        if self.ledger.get(id).is_none() {
            return Ok(());
        }
        let mut snapshot = self.snapshot()?;
        if let Some(records) = snapshot
            .get_mut("records")
            .and_then(serde_json::Value::as_object_mut)
        {
            records.remove(id.as_str());
        }
        self.restore(&snapshot)
    }

    fn snapshot(&self) -> Result<serde_json::Value, G9Error> {
        let text = self
            .ledger
            .to_json()
            .map_err(|_| G9Error::internal_redacted("approvals.snapshot"))?;
        serde_json::from_str(&text).map_err(|_| G9Error::internal_redacted("approvals.snapshot"))
    }

    fn restore(&mut self, snapshot: &serde_json::Value) -> Result<(), G9Error> {
        self.ledger = ApprovalLedger::from_json(&snapshot.to_string())
            .map_err(|_| G9Error::internal_redacted("approvals.snapshot"))?;
        Ok(())
    }
}

/// Rebuild a ledger record from its row. The row stores the requester id only;
/// every approval in this installation is requested by the backend-assigned
/// operator, so a matching id is the operator and anything else is treated as
/// a system actor (which can never consume an operator approval).
fn record_from_row(row: &ApprovalRequestRow, operator: &Actor) -> ApprovalRecord {
    let requested_by = if row.requested_by == operator.id {
        operator.clone()
    } else {
        Actor::system(row.requested_by.clone())
    };
    let consumed_by = row.consumed_at.map(|_| requested_by.id.clone());
    ApprovalRecord {
        request: ApprovalRequest {
            id: row.id.clone(),
            operation: row.operation.clone(),
            requested_by,
            requested_at: row.requested_at,
            expires_at: row.expires_at,
            expected_revision: row.expected_revision,
            // A request without a capability can never be consumed: the
            // ledger's capability check fails closed against `ModelLoad`,
            // which no approval-gated IPC operation requires.
            required_capability: row
                .required_capability
                .unwrap_or(g9_domain::Capability::ModelLoad),
            reason: String::new(),
        },
        state: row.state,
        decided_by: row.decided_by.clone(),
        decided_at: row.decided_at,
        decision_note: None,
        binding_mac: row.binding_mac.clone(),
        revoked_by: None,
        revoked_at: None,
        expired_at: None,
        consumed_by,
        consumed_at: row.consumed_at,
    }
}

/// Everything except the database handle; split from [`AppContext`] so that a
/// command can hold the storage transaction and the services at the same time.
pub struct Services {
    /// Policy engine (shared with the skills orchestrator adapter).
    pub policy: Arc<Engine>,
    /// Approval ledger.
    pub approvals: ApprovalBook,
    /// Per-installation HMAC key (backend only).
    pub binding_key: BindingKey,
    /// Inference runtime.
    pub runtime: Arc<dyn InferenceRuntime>,
    /// Validated skill registry, when a skills directory is configured.
    pub skills: Option<Registry>,
    /// Wall clock.
    pub clock: WallClockFn,
    /// Monotonic clock.
    pub monotonic: MonotonicFn,
    /// Backend-assigned operator actor.
    pub operator: Actor,
    /// Backend system actor (recovery, expiry sweeps).
    pub system: Actor,
    /// Configuration.
    pub config: AppConfig,
    /// Validated host roots (`None` when no root is configured).
    pub approved_roots: Option<ApprovedRoots>,
    /// Backups created or advanced in this process (a cache over
    /// `backup_manifests`, which is authoritative; see `commands::backup`).
    pub backups: BTreeMap<BackupId, KnownBackup>,
    /// Ids of timer intervals opened in this process lifetime (monotonic epoch).
    pub epoch_intervals: BTreeSet<String>,
    /// Recovery outcome of the last open.
    pub recovery: RecoveryReport,
    /// Whether the database runs in WAL mode (observed PRAGMA at open).
    pub storage_wal: bool,
    /// Audit rows appended by this process, oldest first (bounded).
    pub recent_audit: VecDeque<AuditRef>,
    /// The operations log, when the host gave one (`crate::oplog`): one line per
    /// dispatched command. `None` in tests and tools that need no log.
    pub oplog: Option<Arc<crate::oplog::OpLog>>,
    last_monotonic: AtomicU64,
    /// Exclusive hold on a file database's data (app-4); `None` in memory.
    _data_lock: Option<crate::lock::DataLock>,
    /// The approval the operator confirmed outside the webview for the command
    /// being dispatched (set and cleared by [`crate::dispatch_confirming`]).
    pub(crate) native_confirmed: Option<g9_domain::ApprovalId>,
}

impl fmt::Debug for Services {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Services")
            .field("policy", &self.policy)
            .field("approvals", &self.approvals.ledger().len())
            .field("binding_key", &self.binding_key)
            .field("runtime", &self.runtime.describe().name)
            .field("skills", &self.skills.as_ref().map(Registry::len))
            .field("operator", &self.operator)
            .field("config", &self.config)
            .field("backups", &self.backups.len())
            .field("recovery", &self.recovery)
            .finish_non_exhaustive()
    }
}

/// Bound of [`Services::recent_audit`].
pub const RECENT_AUDIT_CAP: usize = 1_000;

impl Services {
    /// Remember an audit row appended by this process (for `audit.recent`).
    pub fn note_audit(&mut self, audit_ref: AuditRef) {
        if self.recent_audit.len() >= RECENT_AUDIT_CAP {
            self.recent_audit.pop_front();
        }
        self.recent_audit.push_back(audit_ref);
    }

    /// Current wall-clock instant.
    pub fn now(&self) -> UtcMillis {
        (self.clock)()
    }

    /// Current monotonic reading; records the highest reading seen.
    pub fn mono(&self) -> u64 {
        let v = (self.monotonic)();
        self.last_monotonic.fetch_max(v, Ordering::SeqCst);
        v
    }

    /// Whether the monotonic source has never gone backwards in this process.
    pub fn monotonic_ok(&self) -> bool {
        let seen = self.last_monotonic.load(Ordering::SeqCst);
        (self.monotonic)() >= seen
    }
}

/// The application context the bridge holds for the lifetime of the process.
#[derive(Debug)]
pub struct AppContext {
    pub(crate) db: Database,
    pub(crate) svc: Services,
}

impl AppContext {
    /// Start building a context over an opened database.
    pub fn builder(
        db: Database,
        config: AppConfig,
        binding_key: BindingKey,
        operator_id: ActorId,
    ) -> AppContextBuilder {
        AppContextBuilder {
            db,
            config,
            binding_key,
            operator_id,
            runtime: None,
            skills: None,
            clock: None,
            monotonic: None,
            oplog: None,
            data_lock: None,
        }
    }

    /// Borrow the database (read-only inspection, tests).
    pub fn database(&self) -> &Database {
        &self.db
    }

    /// Give up the context and keep the database (a process restart in tests:
    /// building a new context over it runs recovery again).
    pub fn into_database(self) -> Database {
        self.db
    }

    /// The backend-assigned operator actor.
    pub fn operator(&self) -> &Actor {
        &self.svc.operator
    }

    /// Recovery report of the open that produced this context.
    pub fn recovery_report(&self) -> &RecoveryReport {
        &self.svc.recovery
    }

    /// The in-memory approval ledger.
    pub fn approvals(&self) -> &ApprovalLedger {
        self.svc.approvals.ledger()
    }

    /// Current wall-clock instant of the context clock.
    pub fn now(&self) -> UtcMillis {
        self.svc.now()
    }
}

/// Builder for [`AppContext`]; `open` runs recovery (`crate::recovery`).
pub struct AppContextBuilder {
    db: Database,
    config: AppConfig,
    binding_key: BindingKey,
    operator_id: ActorId,
    runtime: Option<Arc<dyn InferenceRuntime>>,
    skills: Option<Registry>,
    clock: Option<WallClockFn>,
    monotonic: Option<MonotonicFn>,
    oplog: Option<Arc<crate::oplog::OpLog>>,
    data_lock: Option<crate::lock::DataLock>,
}

impl fmt::Debug for AppContextBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppContextBuilder")
            .field("config", &self.config)
            .field("operator_id", &self.operator_id)
            .finish_non_exhaustive()
    }
}

impl AppContextBuilder {
    /// Use a configured inference runtime (default: [`NullRuntime`]).
    pub fn runtime(mut self, runtime: Arc<dyn InferenceRuntime>) -> Self {
        self.runtime = Some(runtime);
        self
    }

    /// Use a validated skill registry (default: none → `skills.invoke` is `RESOURCE_UNAVAILABLE`).
    pub fn skills(mut self, registry: Registry) -> Self {
        self.skills = Some(registry);
        self
    }

    /// Inject the wall clock (default: system clock).
    pub fn clock(mut self, clock: Box<dyn Fn() -> UtcMillis + Send + Sync>) -> Self {
        self.clock = Some(Arc::from(clock));
        self
    }

    /// Inject the monotonic clock (default: `std::time::Instant` since build).
    ///
    /// Suspend semantics of the default differ per OS (INFERRED from the Rust
    /// std documentation, not measured here): on Linux `CLOCK_MONOTONIC` stops
    /// while the machine is suspended, on Windows the performance counter keeps
    /// counting. The session timer compares wall and monotonic elapsed time at
    /// every stop (`commands::sessions::CLOCK_DIVERGENCE_TOLERANCE_MS`), which
    /// detects the first case; the second needs the TEC-07 heartbeat ledger
    /// (PROPOSED).
    pub fn monotonic(mut self, mono: Box<dyn Fn() -> u64 + Send + Sync>) -> Self {
        self.monotonic = Some(Arc::from(mono));
        self
    }

    /// Write one line per dispatched command to `log` (default: no log). The
    /// host opens the log of its data directory and, to record the backend's
    /// diagnostic events as well, installs it with
    /// [`crate::oplog::install_event_log`] before calling [`Self::open`].
    pub fn oplog(mut self, log: Arc<crate::oplog::OpLog>) -> Self {
        self.oplog = Some(log);
        self
    }

    /// Keep the data lock the host took before it opened the database, instead
    /// of taking it in [`Self::open`] (a second, separately opened lock on the
    /// same file would be refused). A host opening a file database takes the
    /// lock first: `Database::open` migrates a fresh file, and two processes
    /// starting together must not both migrate it (storage-core-8). The lock
    /// must be the one of this builder's database file.
    pub fn data_lock(mut self, lock: crate::lock::DataLock) -> Self {
        self.data_lock = Some(lock);
        self
    }

    /// Run recovery (migrations, audit chain, open intervals, approval expiry)
    /// and return the ready context.
    pub fn open(self) -> Result<AppContext, G9Error> {
        let clock: WallClockFn = self.clock.unwrap_or_else(|| Arc::new(UtcMillis::now));
        let monotonic: MonotonicFn = self.monotonic.unwrap_or_else(|| {
            let start = std::time::Instant::now();
            Arc::new(move || u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX))
        });
        let engine_clock = clock.clone();
        let policy = Arc::new(Engine::builtin_v1(Box::new(move || engine_clock())));
        let operator = Actor::operator(self.operator_id);
        let system = Actor::system(
            ActorId::parse("system")
                .map_err(|_| G9Error::internal_redacted("context.system_actor"))?,
        );
        let approved_roots = if self.config.approved_roots.is_empty() {
            None
        } else {
            Some(
                ApprovedRoots::new(self.config.approved_roots.clone()).map_err(|_| {
                    G9Error::invalid_input(
                        "config.approved_roots_invalid",
                        "approved roots must be absolute, normalized directories",
                    )
                })?,
            )
        };
        // Before recovery: a second process must not close the first one's
        // open intervals (app-4).
        let data_lock = match (self.db.path(), self.data_lock) {
            (Some(path), Some(held)) if held.path() == crate::lock::lock_path(path) => Some(held),
            (Some(_), Some(_)) => {
                return Err(G9Error::internal_redacted("context.data_lock_mismatch"))
            }
            (Some(path), None) => Some(crate::lock::DataLock::acquire(path)?),
            (None, held) => held,
        };
        let mut svc = Services {
            policy,
            approvals: ApprovalBook::default(),
            binding_key: self.binding_key,
            runtime: self.runtime.unwrap_or_else(|| Arc::new(NullRuntime::new())),
            skills: self.skills,
            clock,
            monotonic,
            operator,
            system,
            config: self.config,
            approved_roots,
            backups: BTreeMap::new(),
            epoch_intervals: BTreeSet::new(),
            recovery: RecoveryReport::default(),
            storage_wal: self.db.profile().journal_mode.eq_ignore_ascii_case("wal"),
            recent_audit: VecDeque::new(),
            oplog: self.oplog,
            last_monotonic: AtomicU64::new(0),
            _data_lock: data_lock,
            native_confirmed: None,
        };
        let mut db = self.db;
        svc.recovery = recovery::run(&mut db, &mut svc)?;
        svc.approvals = ApprovalBook::hydrate(db.conn(), &svc.operator)?;
        Ok(AppContext { db, svc })
    }
}
