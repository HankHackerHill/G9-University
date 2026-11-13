#![allow(
    dead_code,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::result_large_err
)]
//! Deterministic harness: in-memory (or temp-file) database, fixed binding key,
//! stepping wall and monotonic clocks, backend-assigned operator `operator-1`.

use g9_ai_adapter::InferenceRuntime;
use g9_app::{AppConfig, AppContext};
use g9_domain::{
    ActorId, AssessmentId, CompetencyId, DataDomain, Difficulty, EvidenceId, G9Error, UtcMillis,
};
use g9_policy::BindingKey;
use g9_skills::Registry;
use g9_storage::repos::{assessment as assessment_repo, competency as competency_repo};
use g9_storage::{Database, OpenOptions};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

/// 2026-11-22T08:30:00Z (fixed test instant).
pub const T0: i64 = 1_795_336_200_000;

pub fn key() -> BindingKey {
    BindingKey::from_bytes([7u8; 32])
}

pub fn operator_id() -> ActorId {
    ActorId::parse("operator-1").unwrap()
}

pub fn seed_manifest() -> Value {
    serde_json::from_str(include_str!(
        "../../../../data/curriculum/g9-ct9-480.seed.v0.json"
    ))
    .unwrap()
}

pub fn skills_fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../g9-skills/tests/fixtures/valid")
}

/// Default (domain, mode) per operation, mirroring `operations.ts`.
pub fn default_scope(op: &str) -> (&'static str, &'static str) {
    match op {
        "system.status" | "approvals.list" | "approvals.decide" | "audit.recent"
        | "export.request" => ("SYSTEM", "STUDY"),
        "backup.create" | "backup.verify" | "backup.restore_test" => ("SYSTEM", "MAINTENANCE"),
        "curriculum.import" | "curriculum.promote" => ("LEARNING", "MAINTENANCE"),
        "assessment.begin" | "assessment.submit" => ("LEARNING", "EXAM"),
        "assessment.list"
        | "assessment.review_evidence"
        | "assessment.grade_provisional"
        | "assessment.approve_final" => ("LEARNING", "REVIEW"),
        _ => ("LEARNING", "STUDY"),
    }
}

#[derive(Debug, Default, Clone)]
pub struct Opts {
    pub approval_id: Option<String>,
    pub expected_revision: Option<u64>,
    pub idempotency_key: Option<String>,
    pub scope: Option<(&'static str, &'static str)>,
}

impl Opts {
    pub fn approval(id: &str) -> Self {
        Self {
            approval_id: Some(id.to_string()),
            ..Self::default()
        }
    }
    pub fn key(k: &str) -> Self {
        Self {
            idempotency_key: Some(k.to_string()),
            ..Self::default()
        }
    }
    pub fn revision(r: u64) -> Self {
        Self {
            expected_revision: Some(r),
            ..Self::default()
        }
    }
}

pub struct Setup {
    pub config: AppConfig,
    pub runtime: Option<Arc<dyn InferenceRuntime>>,
    pub skills: Option<Registry>,
}

pub struct Harness {
    pub ctx: Option<AppContext>,
    pub clock: Arc<AtomicI64>,
    pub mono: Arc<AtomicU64>,
    pub dir: Option<tempfile::TempDir>,
    config: AppConfig,
    runtime: Option<Arc<dyn InferenceRuntime>>,
    skills: Option<Registry>,
    n: u64,
}

impl Harness {
    pub fn new() -> Self {
        Self::with(|_| {})
    }

    /// Harness with a temp directory used as approved root, backup and scratch dir.
    pub fn with_dirs() -> Self {
        let dir = tempfile::Builder::new().prefix("g9app").tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("backups")).unwrap();
        std::fs::create_dir_all(root.join("scratch")).unwrap();
        let mut h = Self::with(|s| {
            s.config.approved_roots = vec![root.clone()];
            s.config.backup_dir = Some(root.join("backups"));
            s.config.scratch_dir = Some(root.join("scratch"));
        });
        h.dir = Some(dir);
        h
    }

    pub fn with(f: impl FnOnce(&mut Setup)) -> Self {
        let db = Database::open_in_memory(
            OpenOptions::new("0.1.0-test").applied_at(UtcMillis::from_millis(T0)),
        )
        .unwrap();
        Self::with_database(db, f).unwrap()
    }

    /// Build a harness over an already opened database (returns the open error).
    pub fn with_database(db: Database, f: impl FnOnce(&mut Setup)) -> Result<Self, G9Error> {
        let mut setup = Setup {
            config: AppConfig::new("0.1.0-test").unwrap(),
            runtime: None,
            skills: None,
        };
        f(&mut setup);
        let clock = Arc::new(AtomicI64::new(T0));
        let mono = Arc::new(AtomicU64::new(1_000));
        let mut h = Self {
            ctx: None,
            clock,
            mono,
            dir: None,
            config: setup.config,
            runtime: setup.runtime,
            skills: setup.skills,
            n: 0,
        };
        h.try_open(db)?;
        Ok(h)
    }

    fn open(&mut self, db: Database) {
        self.try_open(db).unwrap();
    }

    fn try_open(&mut self, db: Database) -> Result<(), G9Error> {
        let c = self.clock.clone();
        let m = self.mono.clone();
        let mut b = AppContext::builder(db, self.config.clone(), key(), operator_id())
            .clock(Box::new(move || {
                UtcMillis::from_millis(c.load(Ordering::SeqCst))
            }))
            .monotonic(Box::new(move || m.load(Ordering::SeqCst)));
        if let Some(rt) = &self.runtime {
            b = b.runtime(rt.clone());
        }
        if let Some(reg) = &self.skills {
            b = b.skills(reg.clone());
        }
        self.ctx = Some(b.open()?);
        Ok(())
    }

    /// Simulate a process restart: same database, new context (recovery runs),
    /// monotonic clock restarts near zero.
    pub fn restart(&mut self) {
        let db = self.ctx.take().unwrap().into_database();
        self.mono.store(5, Ordering::SeqCst);
        self.open(db);
    }

    /// Continue the request/idempotency counter at `n`. A harness opened over a
    /// database an earlier harness used must not reuse its ids (production ids
    /// are uuid v7); give each lifetime its own range.
    pub fn continue_ids_from(&mut self, n: u64) {
        self.n = n;
    }

    pub fn ctx(&mut self) -> &mut AppContext {
        self.ctx.as_mut().unwrap()
    }

    pub fn conn(&self) -> &rusqlite::Connection {
        self.ctx.as_ref().unwrap().database().conn()
    }

    pub fn now(&self) -> UtcMillis {
        UtcMillis::from_millis(self.clock.load(Ordering::SeqCst))
    }

    pub fn advance(&self, ms: i64) {
        self.clock.fetch_add(ms, Ordering::SeqCst);
        self.mono
            .fetch_add(u64::try_from(ms).unwrap(), Ordering::SeqCst);
    }

    pub fn meta(&mut self, op: &str, opts: &Opts) -> Value {
        self.n += 1;
        let (domain, mode) = opts.scope.unwrap_or_else(|| default_scope(op));
        let mut m = json!({
            "request_id": format!("req-{}", self.n),
            "operation": op,
            "idempotency_key": opts.idempotency_key.clone().unwrap_or_else(|| format!("idem-{}", self.n)),
            "scope": { "domain": domain, "mode": mode },
            "payload_schema_version": { "major": 1, "minor": 0 },
            "trace_id": format!("trace-{}", self.n),
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

    pub fn call(&mut self, op: &str, payload: Value) -> Result<Value, G9Error> {
        self.call_with(op, payload, &Opts::default())
    }

    pub fn call_with(&mut self, op: &str, payload: Value, opts: &Opts) -> Result<Value, G9Error> {
        let meta = self.meta(op, opts);
        self.raw(op, meta, payload)
    }

    pub fn raw(&mut self, op: &str, meta: Value, payload: Value) -> Result<Value, G9Error> {
        let out = g9_app::dispatch(self.ctx(), op, meta, payload);
        match &out {
            Ok(v) => g9_app::schemas::validate_outcome(v).unwrap(),
            Err(e) => g9_app::schemas::validate_error(&serde_json::to_value(e).unwrap()).unwrap(),
        }
        out
    }

    /// Run an approval-gated command: expect APPROVAL_REQUIRED, approve, retry.
    pub fn approved(&mut self, op: &str, payload: Value, opts: &Opts) -> Value {
        let err = self.call_with(op, payload.clone(), opts).unwrap_err();
        assert_eq!(
            err.category,
            g9_domain::ErrorCategory::ApprovalRequired,
            "{err:?}"
        );
        let id = err.approval_id.clone().unwrap();
        self.decide(id.as_str(), "APPROVE");
        let retry = Opts {
            approval_id: Some(id.as_str().to_string()),
            ..opts.clone()
        };
        self.call_with(op, payload, &retry).unwrap()
    }

    pub fn decide(&mut self, approval_id: &str, decision: &str) -> Value {
        self.call(
            "approvals.decide",
            json!({ "approval_id": approval_id, "decision": decision }),
        )
        .unwrap()
    }

    pub fn audit_rows(&self) -> u64 {
        g9_storage::AuditChain::verify_chain(self.conn())
            .unwrap()
            .rows
    }
}

/// The competency every blind-practical fixture below assesses.
pub const COMPETENCY: &str = "comp-boot";

/// Competency [`COMPETENCY`], the blind-practical definition `ad-w1` (mode EXAM)
/// and `versions` baseline versions labelled "1".."n". `difficulty` is the value
/// the versions' author declares: `Some(d)` persists it, `None` models a
/// version authored without one (the column keeps NULL; nothing is guessed).
pub fn seed_blind_practical(h: &Harness, versions: u32, difficulty: Option<u8>) -> AssessmentId {
    let conn = h.conn();
    let now = h.now();
    let competency = g9_competency::Competency {
        id: CompetencyId::parse(COMPETENCY).unwrap(),
        name: "Linux boot chain".into(),
        pack: "linux".into(),
        domain: DataDomain::Learning,
        description: "Recover a broken boot chain".into(),
        validity_window_days: 90,
        difficulty_band: (Difficulty::new(1).unwrap(), Difficulty::new(5).unwrap()),
        nice_refs: Vec::new(),
    };
    competency_repo::insert_competency(
        conn,
        &competency_repo::CompetencyRow::from_domain(&competency, "LINUX-BOOT", None, now),
    )
    .unwrap();
    let def = g9_assessment::AssessmentDefinition::new(
        AssessmentId::parse("ad-w1").unwrap(),
        "Week 1 blind practical",
        vec![competency.id.clone()],
        g9_assessment::AssessmentKind::BlindPractical,
        DataDomain::Learning,
    )
    .unwrap();
    assessment_repo::insert_definition(
        conn,
        &assessment_repo::AssessmentDefinitionRow::from_domain(&def, "W1-BLIND", now).unwrap(),
    )
    .unwrap();
    for n in 1..=versions {
        let version = g9_assessment::AssessmentVersion {
            definition_id: def.id.clone(),
            version: n,
            rubric: g9_assessment::Rubric::baseline(),
            task_ref: EvidenceId::parse(&format!("evi-task-{n}")).unwrap(),
            sealed_key_ref: None,
            // The domain type requires a value; a version "without difficulty"
            // is stored by clearing it on the row below.
            difficulty: Difficulty::new(difficulty.unwrap_or(3)).unwrap(),
            equivalence_group: "W1".into(),
            created_at: now,
            checksum: String::new(),
        }
        .with_checksum()
        .unwrap();
        let mut row = assessment_repo::AssessmentVersionRow::from_domain(
            &version,
            &format!("av-w1-{n}"),
            &format!("Recover the boot chain (variant {n})"),
            None,
            operator_id(),
        )
        .unwrap();
        if difficulty.is_none() {
            row.difficulty = None;
        }
        assessment_repo::insert_version(conn, &row).unwrap();
        let (rubric, criteria) = assessment_repo::rubric_rows_from_domain(
            &version.rubric,
            &format!("rub-w1-{n}"),
            &format!("av-w1-{n}"),
            "Baseline",
            "v1",
            now,
        );
        assessment_repo::insert_rubric(conn, &rubric, &criteria).unwrap();
    }
    def.id
}

/// An independent (unaided, difficulty 3, application) LAB observation of
/// [`COMPETENCY`] from a learning episode, `days_ago` days before the
/// harness clock. Stands for earlier practice recorded outside `g9-app`.
pub fn lab_observation(h: &Harness, id: &str, score: u8, days_ago: i64) {
    use g9_competency::{CompetencyObservation, ObservationSource};
    use g9_domain::{
        AssistanceLevel, Confidence, EpisodeId, InteractionMode, LearningDimension, ObservationId,
        Provenance, SourceKind, TrustClass,
    };
    let at = UtcMillis::from_millis(h.now().as_millis() - days_ago * 86_400_000);
    let episode = format!("ep-{id}");
    h.conn()
        .execute(
            "INSERT INTO learning_episodes (id, competency_id, kind, started_at, created_at, updated_at)
             VALUES (?1, ?2, 'INDEPENDENT_LAB', ?3, ?3, ?3)",
            rusqlite::params![episode, COMPETENCY, at.as_millis()],
        )
        .unwrap();
    competency_repo::append_observation(
        h.conn(),
        &CompetencyObservation {
            id: ObservationId::parse(id).unwrap(),
            competency_id: CompetencyId::parse(COMPETENCY).unwrap(),
            dimension: LearningDimension::Application,
            source: ObservationSource::Episode(EpisodeId::parse(&episode).unwrap()),
            mode: InteractionMode::Lab,
            difficulty: Difficulty::new(3).unwrap(),
            assistance: AssistanceLevel::Unaided,
            score,
            environment: "lab-vm".into(),
            reviewer: None,
            provenance: Provenance {
                source_kind: SourceKind::SystemGenerated,
                source_ref: format!("episode:{episode}"),
                trust: TrustClass::SystemDeterministic,
                content_sha256: None,
                recorded_at: at,
                recorded_by: ActorId::parse("system").unwrap(),
                version: None,
            },
            uncertainty: Confidence::new(0.0).unwrap(),
            observed_at: at,
            supersedes: None,
        },
    )
    .unwrap();
}

/// Insert a PLANNED study session `id` directly (seed data; sessions are not
/// created through IPC in these tests).
pub fn insert_planned_session(h: &Harness, id: &str) {
    g9_storage::repos::sessions::insert_session(
        h.conn(),
        &g9_storage::repos::sessions::StudySessionRow {
            id: g9_domain::SessionId::parse(id).unwrap(),
            training_day_id: None,
            objective_id: None,
            study_block_id: None,
            calendar_event_id: None,
            state: g9_domain::SessionState::Planned,
            mode: g9_domain::InteractionMode::Study,
            zone: g9_domain::IanaZone::utc(),
            planned_start_utc: Some(h.now()),
            planned_end_utc: None,
            planned_minutes: 60,
            started_at: None,
            ended_at: None,
            credited_ms: g9_domain::DurationMs::ZERO,
            uncertain_ms: g9_domain::DurationMs::ZERO,
            data_domain: g9_domain::DataDomain::Learning,
            classification: g9_domain::Classification::Internal,
            revision: g9_domain::Revision::INITIAL,
            created_at: h.now(),
            updated_at: h.now(),
        },
    )
    .unwrap();
}
