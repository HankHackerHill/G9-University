//! Deterministic evaluation harness over the real application context.
//!
//! [`Harness`] owns a [`g9_app::AppContext`] built exactly as the Tauri bridge
//! builds it — same pipeline, same policy table, same storage — with every
//! source of non-determinism pinned:
//!
//! | Input | Pinned value |
//! |---|---|
//! | database | in-memory SQLite (or a file in a disposable directory for recovery drills) |
//! | wall clock | [`T0`] (2026-11-16T08:00:00Z), advanced only by [`Harness::advance`] |
//! | monotonic clock | starts at 1 000 ms, advanced with the wall clock, reset on [`Harness::restart`] |
//! | binding key | [`FIXED_BINDING_KEY`] (32 fixed bytes; never a production secret) |
//! | runtime | [`ScriptedRuntime`] (FIFO of scripted replies; empty queue = model unavailable) or [`NullRuntime`] |
//! | skills | the real packages under `.agents/skills`, loaded and validated by `g9_skills::Registry::load` when present |
//! | operator | `operator-1`, assigned by the backend (IPC metadata never carries an actor) |
//!
//! Every IPC call goes through [`g9_app::dispatch`]; the success value and the
//! error are checked against `schemas/ipc/command-outcome.schema.json` and
//! `g9-error.schema.json`. [`Harness::step`] additionally reads back the audit
//! row the backend wrote for the request (decision and outcome) so a scenario
//! records the *actual* decision, not the expected one.

use crate::report::{ScenarioReport, ScenarioStep};
use g9_ai_adapter::{InferenceRuntime, ModelRef, NullRuntime, ScriptedRuntime};
use g9_app::{AppConfig, AppContext};
use g9_domain::{
    ActorId, ApprovalId, DataDomain, ErrorCategory, G9Error, InteractionMode, ModelId, Scope,
    UtcMillis,
};
use g9_policy::BindingKey;
use g9_skills::Registry;
use g9_storage::{Database, OpenOptions};
use serde_json::{json, Value};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

/// Fixed scenario start: 2026-11-16T08:00:00Z (Monday of program week 3 in the fixtures).
pub const T0: i64 = 1_794_816_000_000;
/// One minute in milliseconds.
pub const MINUTE_MS: i64 = 60_000;
/// Backend-assigned operator id used by every scenario.
pub const OPERATOR_ID: &str = "operator-1";
/// Application version recorded in migrations, status and backup manifests.
pub const APP_VERSION: &str = "0.1.0-eval";
/// Fixed per-installation binding key for approvals (test value, not a secret).
pub const FIXED_BINDING_KEY: [u8; 32] = [0x47; 32];
/// Context window reported by the scripted runtime (large enough for every real package).
pub const DEFAULT_MAX_CONTEXT_TOKENS: u32 = 1_000_000;
/// Model id reported by the scripted runtime.
pub const SCRIPTED_MODEL_ID: &str = "scripted-eval-model";

/// Root of the repository (compile-time path of this crate, two levels up).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Default skills root: `<repo>/.agents/skills`.
pub fn default_skills_root() -> PathBuf {
    repo_root().join(".agents").join("skills")
}

/// Which inference runtime the context is built with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeChoice {
    /// [`ScriptedRuntime`]: replies are queued by the scenario.
    Scripted,
    /// [`NullRuntime`]: the model-down path (gate A09).
    Null,
}

/// Harness construction options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessOptions {
    /// Inference runtime.
    pub runtime: RuntimeChoice,
    /// Context window reported by the scripted runtime.
    pub max_context_tokens: u32,
    /// Skills root to load; `None` disables the registry (`skills.invoke` is then `RESOURCE_UNAVAILABLE`).
    pub skills_root: Option<PathBuf>,
    /// Create a disposable approved root with `backups/` and `scratch/`.
    pub host_dirs: bool,
    /// Use a file database at this path instead of an in-memory database.
    pub database_path: Option<PathBuf>,
    /// Approval binding key.
    pub binding_key: [u8; 32],
}

impl Default for HarnessOptions {
    fn default() -> Self {
        Self {
            runtime: RuntimeChoice::Scripted,
            max_context_tokens: DEFAULT_MAX_CONTEXT_TOKENS,
            skills_root: Some(default_skills_root()),
            host_dirs: false,
            database_path: None,
            binding_key: FIXED_BINDING_KEY,
        }
    }
}

impl HarnessOptions {
    /// Default options with a disposable approved root (backups, restore tests).
    pub fn with_host_dirs() -> Self {
        Self {
            host_dirs: true,
            ..Self::default()
        }
    }

    /// Default options with the model-down runtime.
    pub fn model_down() -> Self {
        Self {
            runtime: RuntimeChoice::Null,
            ..Self::default()
        }
    }
}

/// What happened when the skills registry was loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryStatus {
    /// No skills root was requested.
    NotRequested,
    /// The root does not exist (the registry is absent, not invented).
    Absent(PathBuf),
    /// Every package validated.
    Loaded {
        /// Number of packages.
        packages: usize,
    },
    /// The registry refused the packages (the context is built without it).
    Refused {
        /// `G9Error` code of the refusal.
        code: String,
    },
}

/// Per-call metadata options.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CallOpts {
    /// Approval presented for an approval-gated retry.
    pub approval_id: Option<String>,
    /// Optimistic-concurrency revision.
    pub expected_revision: Option<u64>,
    /// Idempotency key (default: a fresh key per call).
    pub idempotency_key: Option<String>,
    /// Scope override (default: the operation's default scope from `operations.ts`).
    pub scope: Option<Scope>,
}

impl CallOpts {
    /// Present an approval.
    pub fn approval(id: impl Into<String>) -> Self {
        Self {
            approval_id: Some(id.into()),
            ..Self::default()
        }
    }

    /// Expect a revision.
    pub fn revision(r: u64) -> Self {
        Self {
            expected_revision: Some(r),
            ..Self::default()
        }
    }

    /// Use a scope.
    pub fn scope(domain: DataDomain, mode: InteractionMode) -> Self {
        Self {
            scope: Some(Scope { domain, mode }),
            ..Self::default()
        }
    }

    /// Reuse an idempotency key.
    pub fn idempotency(key: impl Into<String>) -> Self {
        Self {
            idempotency_key: Some(key.into()),
            ..Self::default()
        }
    }
}

/// Default (domain, mode) per operation, mirroring `defaultDomain`/`defaultMode`
/// in `apps/desktop/src/ipc/operations.ts`.
pub fn default_scope(op: &str) -> Scope {
    let (domain, mode) = match op {
        "system.status" | "approvals.list" | "approvals.decide" | "audit.recent"
        | "export.request" => (DataDomain::System, InteractionMode::Study),
        "backup.create" | "backup.verify" | "backup.restore_test" => {
            (DataDomain::System, InteractionMode::Maintenance)
        }
        "curriculum.import" | "curriculum.promote" => {
            (DataDomain::Learning, InteractionMode::Maintenance)
        }
        "assessment.begin" | "assessment.submit" => (DataDomain::Learning, InteractionMode::Exam),
        "assessment.list"
        | "assessment.review_evidence"
        | "assessment.grade_provisional"
        | "assessment.approve_final" => (DataDomain::Learning, InteractionMode::Review),
        _ => (DataDomain::Learning, InteractionMode::Study),
    };
    Scope { domain, mode }
}

/// Result of one recorded IPC call.
#[derive(Debug, Clone, PartialEq)]
pub struct StepOutcome {
    /// Request id sent in the metadata.
    pub request_id: String,
    /// The dispatch result.
    pub result: Result<Value, G9Error>,
    /// Decision read back from the audit row for this request (`ALLOW`,
    /// `DENY`, `APPROVAL_REQUIRED`, `NOT_EVALUATED`), or `NO_AUDIT_ROW` when
    /// the backend wrote none (allowed reads; requests refused before the
    /// pipeline, e.g. malformed metadata or payload).
    pub decision: String,
    /// `OK` or the closed error category (`SCREAMING_SNAKE_CASE`).
    pub outcome_category: String,
}

impl StepOutcome {
    /// The error, when the call failed.
    pub fn error(&self) -> Option<&G9Error> {
        self.result.as_ref().err()
    }

    /// The `result` member of a successful outcome (`Value::Null` otherwise).
    pub fn value(&self) -> Value {
        self.result
            .as_ref()
            .ok()
            .and_then(|v| v.get("result").cloned())
            .unwrap_or(Value::Null)
    }

    /// Approval id carried by an `APPROVAL_REQUIRED` error.
    pub fn approval_id(&self) -> Option<String> {
        self.error()
            .and_then(|e| e.approval_id.as_ref())
            .map(|a| a.as_str().to_string())
    }

    /// Error category, when the call failed.
    pub fn category(&self) -> Option<ErrorCategory> {
        self.error().map(|e| e.category)
    }
}

/// Wire spelling of an error category (`APPROVAL_REQUIRED`, …).
pub fn category_name(c: ErrorCategory) -> String {
    serde_json::to_value(c)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{c:?}"))
}

/// The evaluation harness. See the module docs for what is pinned.
pub struct Harness {
    ctx: Option<AppContext>,
    clock: Arc<AtomicI64>,
    mono: Arc<AtomicU64>,
    scripted: Option<Arc<ScriptedRuntime>>,
    runtime: Arc<dyn InferenceRuntime>,
    registry: Option<Registry>,
    registry_status: RegistryStatus,
    config: AppConfig,
    key: [u8; 32],
    host: Option<tempfile::TempDir>,
    seq: u64,
}

impl fmt::Debug for Harness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Harness")
            .field("now", &self.clock.load(Ordering::SeqCst))
            .field("runtime", &self.runtime.describe().name)
            .field("registry_status", &self.registry_status)
            .field(
                "host_dirs",
                &self.host.as_ref().map(tempfile::TempDir::path),
            )
            .field("seq", &self.seq)
            .finish_non_exhaustive()
    }
}

fn io_err(code: &str, e: &std::io::Error) -> G9Error {
    G9Error::resource_unavailable(code, format!("host filesystem: {}", e.kind()))
}

fn storage_err(e: g9_storage::StorageError) -> G9Error {
    G9Error::from(e)
}

impl Harness {
    /// Harness with [`HarnessOptions::default`].
    pub fn new() -> Result<Self, G9Error> {
        Self::with_options(HarnessOptions::default())
    }

    /// Harness with explicit options.
    pub fn with_options(opts: HarnessOptions) -> Result<Self, G9Error> {
        let mut config = AppConfig::new(APP_VERSION)?;
        let host = if opts.host_dirs {
            let dir = tempfile::Builder::new()
                .prefix("g9-eval-")
                .tempdir()
                .map_err(|e| io_err("evaluation.tempdir", &e))?;
            let root =
                std::fs::canonicalize(dir.path()).map_err(|e| io_err("evaluation.tempdir", &e))?;
            for sub in ["backups", "scratch"] {
                std::fs::create_dir_all(root.join(sub))
                    .map_err(|e| io_err("evaluation.tempdir", &e))?;
            }
            config.approved_roots = vec![root.clone()];
            config.backup_dir = Some(root.join("backups"));
            config.scratch_dir = Some(root.join("scratch"));
            Some(dir)
        } else {
            None
        };
        let (scripted, runtime): (Option<Arc<ScriptedRuntime>>, Arc<dyn InferenceRuntime>) =
            match opts.runtime {
                RuntimeChoice::Scripted => {
                    let model = ModelId::parse(SCRIPTED_MODEL_ID)
                        .map_err(|_| G9Error::internal_redacted("evaluation.model_id"))?;
                    let rt = Arc::new(ScriptedRuntime::new(
                        ModelRef::new(model),
                        opts.max_context_tokens,
                    ));
                    (Some(rt.clone()), rt)
                }
                RuntimeChoice::Null => (None, Arc::new(NullRuntime::new())),
            };
        let (registry, registry_status) = match &opts.skills_root {
            None => (None, RegistryStatus::NotRequested),
            Some(root) if !root.is_dir() => (None, RegistryStatus::Absent(root.clone())),
            Some(root) => match Registry::load(root) {
                Ok(r) => {
                    let packages = r.len();
                    (Some(r), RegistryStatus::Loaded { packages })
                }
                Err(e) => (
                    None,
                    RegistryStatus::Refused {
                        code: G9Error::from(e).code,
                    },
                ),
            },
        };
        let mut h = Self {
            ctx: None,
            clock: Arc::new(AtomicI64::new(T0)),
            mono: Arc::new(AtomicU64::new(1_000)),
            scripted,
            runtime,
            registry,
            registry_status,
            config,
            key: opts.binding_key,
            host,
            seq: 0,
        };
        let db = match &opts.database_path {
            Some(path) => Database::open(path, h.open_options()).map_err(storage_err)?,
            None => Database::open_in_memory(h.open_options()).map_err(storage_err)?,
        };
        h.open_database(db)?;
        Ok(h)
    }

    /// Open options used for every database the harness opens (migration
    /// bookkeeping stamped with the harness clock).
    pub fn open_options(&self) -> OpenOptions {
        OpenOptions::new(APP_VERSION).applied_at(self.now())
    }

    /// Build a fresh context over `db` (recovery runs, exactly as at process start).
    pub fn open_database(&mut self, db: Database) -> Result<(), G9Error> {
        let operator = ActorId::parse(OPERATOR_ID)
            .map_err(|_| G9Error::internal_redacted("evaluation.operator_id"))?;
        let clock = self.clock.clone();
        let mono = self.mono.clone();
        let mut builder = AppContext::builder(
            db,
            self.config.clone(),
            BindingKey::from_bytes(self.key),
            operator,
        )
        .runtime(self.runtime.clone())
        .clock(Box::new(move || {
            UtcMillis::from_millis(clock.load(Ordering::SeqCst))
        }))
        .monotonic(Box::new(move || mono.load(Ordering::SeqCst)));
        if let Some(registry) = &self.registry {
            builder = builder.skills(registry.clone());
        }
        self.ctx = Some(builder.open()?);
        Ok(())
    }

    /// Give up the context and keep its database (a process exit).
    pub fn take_database(&mut self) -> Result<Database, G9Error> {
        self.ctx
            .take()
            .map(AppContext::into_database)
            .ok_or_else(|| G9Error::precondition_failed("evaluation.no_context", "no open context"))
    }

    /// Simulate a process restart: same database, new context (recovery runs);
    /// the monotonic clock restarts near zero (a new process lifetime).
    pub fn restart(&mut self) -> Result<(), G9Error> {
        let db = self.take_database()?;
        self.mono.store(5, Ordering::SeqCst);
        self.open_database(db)
    }

    /// Open another database (e.g. a crash image) as a new process lifetime:
    /// the monotonic clock restarts near zero and recovery runs.
    pub fn reopen(&mut self, db: Database) -> Result<(), G9Error> {
        if let Some(ctx) = self.ctx.take() {
            drop(ctx.into_database());
        }
        self.mono.store(5, Ordering::SeqCst);
        self.open_database(db)
    }

    /// Restart with a different binding key (a forged or replaced install secret).
    pub fn restart_with_key(&mut self, key: [u8; 32]) -> Result<(), G9Error> {
        self.key = key;
        self.restart()
    }

    /// The application context.
    pub fn ctx(&mut self) -> Result<&mut AppContext, G9Error> {
        self.ctx
            .as_mut()
            .ok_or_else(|| G9Error::precondition_failed("evaluation.no_context", "no open context"))
    }

    /// The database (read-only inspection through the g9-storage repositories).
    pub fn db(&self) -> Result<&Database, G9Error> {
        self.ctx
            .as_ref()
            .map(AppContext::database)
            .ok_or_else(|| G9Error::precondition_failed("evaluation.no_context", "no open context"))
    }

    /// Recovery report of the last open.
    pub fn recovery_report(&self) -> Option<g9_app::RecoveryReport> {
        self.ctx.as_ref().map(|c| c.recovery_report().clone())
    }

    /// The scripted runtime, when the harness uses one.
    pub fn scripted(&self) -> Option<&Arc<ScriptedRuntime>> {
        self.scripted.as_ref()
    }

    /// Queue a structured reply on the scripted runtime.
    pub fn script_reply(&self, structured: &Value) -> Result<(), G9Error> {
        let rt = self.scripted.as_ref().ok_or_else(|| {
            G9Error::precondition_failed(
                "evaluation.no_scripted_runtime",
                "this harness runs the model-down runtime",
            )
        })?;
        rt.push_response(rt.response(structured.to_string(), Some(structured.clone())));
        Ok(())
    }

    /// Queue a plain-text reply (no structured value) on the scripted runtime.
    pub fn script_text_reply(&self, text: &str) -> Result<(), G9Error> {
        let rt = self.scripted.as_ref().ok_or_else(|| {
            G9Error::precondition_failed(
                "evaluation.no_scripted_runtime",
                "this harness runs the model-down runtime",
            )
        })?;
        rt.push_response(rt.response(text, None));
        Ok(())
    }

    /// Registry load outcome.
    pub fn registry_status(&self) -> &RegistryStatus {
        &self.registry_status
    }

    /// The loaded registry, if any.
    pub fn registry(&self) -> Option<&Registry> {
        self.registry.as_ref()
    }

    /// Disposable approved root, when created.
    pub fn host_root(&self) -> Option<PathBuf> {
        self.config.approved_roots.first().cloned()
    }

    /// Configured backup directory.
    pub fn backup_dir(&self) -> Option<PathBuf> {
        self.config.backup_dir.clone()
    }

    /// Configured scratch directory.
    pub fn scratch_dir(&self) -> Option<PathBuf> {
        self.config.scratch_dir.clone()
    }

    /// Current harness wall-clock instant.
    pub fn now(&self) -> UtcMillis {
        UtcMillis::from_millis(self.clock.load(Ordering::SeqCst))
    }

    /// Advance wall and monotonic clocks together by `ms` (no sleeping).
    pub fn advance(&self, ms: i64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
        self.mono
            .fetch_add(u64::try_from(ms).unwrap_or(0), Ordering::SeqCst);
    }

    /// Wire metadata for `op` (no actor: the backend assigns it).
    pub fn meta(&mut self, op: &str, opts: &CallOpts) -> Value {
        self.seq += 1;
        let n = self.seq;
        let scope = opts.scope.unwrap_or_else(|| default_scope(op));
        let mut m = json!({
            "request_id": format!("req-{n}"),
            "operation": op,
            "idempotency_key": opts.idempotency_key.clone().unwrap_or_else(|| format!("idem-{n}")),
            "scope": scope,
            "payload_schema_version": { "major": 1, "minor": 0 },
            "trace_id": format!("trace-{n}"),
            "issued_at": self.now().as_millis(),
        });
        if let Some(a) = &opts.approval_id {
            m["approval_id"] = json!(a);
        }
        if let Some(r) = opts.expected_revision {
            m["expected_revision"] = json!(r);
        }
        m
    }

    /// Dispatch one call with default options.
    pub fn call(&mut self, op: &str, payload: Value) -> Result<Value, G9Error> {
        self.call_with(op, payload, &CallOpts::default())
    }

    /// Dispatch one call.
    pub fn call_with(
        &mut self,
        op: &str,
        payload: Value,
        opts: &CallOpts,
    ) -> Result<Value, G9Error> {
        let meta = self.meta(op, opts);
        self.raw(op, meta, payload)
    }

    /// Dispatch with caller-built metadata. The outcome is checked against
    /// `command-outcome.schema.json` and its `result` against
    /// `<op>.response.schema.json` (the contract the UI is built against); an
    /// error against `g9-error.schema.json`. A violation is returned as an
    /// `INTERNAL` error with code `evaluation.envelope_schema_violation` so
    /// that scenarios fail.
    pub fn raw(&mut self, op: &str, meta: Value, payload: Value) -> Result<Value, G9Error> {
        let out = g9_app::dispatch(self.ctx()?, op, meta, payload);
        let envelope_ok = match &out {
            Ok(v) => {
                g9_app::schemas::validate_outcome(v).is_ok()
                    && v.get("result")
                        .is_some_and(|r| g9_app::schemas::validate_response(op, r).is_ok())
            }
            Err(e) => serde_json::to_value(e)
                .ok()
                .is_some_and(|v| g9_app::schemas::validate_error(&v).is_ok()),
        };
        if envelope_ok {
            out
        } else {
            Err(G9Error::internal_redacted(
                "evaluation.envelope_schema_violation",
            ))
        }
    }

    /// Dispatch and capture the backend's recorded decision for the request.
    pub fn step(&mut self, op: &str, payload: Value, opts: &CallOpts) -> StepOutcome {
        let meta = self.meta(op, opts);
        self.step_raw(op, meta, payload)
    }

    /// [`Harness::step`] with caller-built metadata.
    pub fn step_raw(&mut self, op: &str, meta: Value, payload: Value) -> StepOutcome {
        let request_id = meta["request_id"].as_str().unwrap_or_default().to_string();
        let result = self.raw(op, meta, payload);
        let decision = self
            .audit_decision(op, &request_id)
            .unwrap_or_else(|| "NO_AUDIT_ROW".to_string());
        let outcome_category = match &result {
            Ok(_) => "OK".to_string(),
            Err(e) => category_name(e.category),
        };
        StepOutcome {
            request_id,
            result,
            decision,
            outcome_category,
        }
    }

    /// [`Harness::step`] and append it to `report` with a state probe.
    pub fn record<P>(
        &mut self,
        report: &mut ScenarioReport,
        op: &str,
        payload: Value,
        opts: &CallOpts,
        probe: P,
    ) -> StepOutcome
    where
        P: FnOnce(&Harness) -> Result<Value, G9Error>,
    {
        let meta = self.meta(op, opts);
        self.record_raw(report, op, meta, payload, probe)
    }

    /// [`Harness::record`] with caller-built metadata (e.g. a forged `actor` key).
    pub fn record_raw<P>(
        &mut self,
        report: &mut ScenarioReport,
        op: &str,
        meta: Value,
        payload: Value,
        probe: P,
    ) -> StepOutcome
    where
        P: FnOnce(&Harness) -> Result<Value, G9Error>,
    {
        let out = self.step_raw(op, meta, payload);
        let state_after = probe(self).unwrap_or_else(|e| json!({ "probe_error": e.code }));
        report.push_step(ScenarioStep {
            op: op.to_string(),
            request_id: Some(out.request_id.clone()),
            decision: out.decision.clone(),
            outcome_category: out.outcome_category.clone(),
            error_code: out.error().map(|e| e.code.clone()),
            state_after,
        });
        out
    }

    /// Decision of the audit row the backend appended for `request_id`
    /// (read through `audit.recent`, which lists this process's rows). Rows of
    /// a post-commit step (`<op>.post_commit`) are ignored in favour of the
    /// command row.
    pub fn audit_decision(&mut self, op: &str, request_id: &str) -> Option<String> {
        let entries = self.audit_entries(500).ok()?;
        let matching: Vec<&Value> = entries
            .iter()
            .filter(|e| e["request_id"] == json!(request_id))
            .collect();
        matching
            .iter()
            .find(|e| e["operation"] == json!(op))
            .or_else(|| matching.first())
            .and_then(|e| e["decision"].as_str().map(str::to_string))
    }

    /// Audit entries appended by this process (newest first) via `audit.recent`.
    pub fn audit_entries(&mut self, limit: u32) -> Result<Vec<Value>, G9Error> {
        let out = self.call("audit.recent", json!({ "limit": limit }))?;
        Ok(out["result"]["entries"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    /// `approvals.decide` with `APPROVE` (operator decision through the pipeline).
    pub fn approve(&mut self, approval_id: &str) -> Result<Value, G9Error> {
        self.call(
            "approvals.decide",
            json!({ "approval_id": approval_id, "decision": "APPROVE" }),
        )
    }

    /// `approvals.decide` with `REJECT`.
    pub fn reject(&mut self, approval_id: &str) -> Result<Value, G9Error> {
        self.call(
            "approvals.decide",
            json!({ "approval_id": approval_id, "decision": "REJECT" }),
        )
    }

    /// Run an approval-gated command end to end: the first call must answer
    /// `APPROVAL_REQUIRED`, the operator approves, the retry presents the
    /// approval. Returns the approval id and the retry's outcome.
    pub fn run_approved(
        &mut self,
        op: &str,
        payload: Value,
        opts: &CallOpts,
    ) -> Result<(ApprovalId, Value), G9Error> {
        let first = self.call_with(op, payload.clone(), opts);
        let approval = match first {
            Err(e) if e.category == ErrorCategory::ApprovalRequired => e
                .approval_id
                .ok_or_else(|| G9Error::internal_redacted("evaluation.approval_id_missing"))?,
            Err(e) => return Err(e),
            Ok(_) => {
                return Err(G9Error::precondition_failed(
                    "evaluation.approval_not_required",
                    format!("{op} succeeded without an approval"),
                ))
            }
        };
        self.approve(approval.as_str())?;
        let retry = CallOpts {
            approval_id: Some(approval.as_str().to_string()),
            ..opts.clone()
        };
        let out = self.call_with(op, payload, &retry)?;
        Ok((approval, out))
    }
}
