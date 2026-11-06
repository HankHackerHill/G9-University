//! Model and skill registry repository (M10/M11; migration `0008_knowledge_ai`,
//! tables `model_registry`, `model_evals`, `skill_registry`, `skill_versions`,
//! `skill_invocations`, `skill_evals`).
//!
//! * A model is loadable only in state `APPROVED`, which needs an approving
//!   operator (DDL CHECK) and an approval token; benchmark runs are immutable.
//! * Skill versions carry the package hashes the orchestrator verifies before
//!   running (supply-chain rule); they are never updated.
//! * `skill_invocations` is metadata only (hashes, counts, latency, validation
//!   outcome, policy decision) — never prompts or outputs. State changes follow
//!   `SkillInvocationState`; terminal rows are immutable (trigger) and a
//!   `COMPLETED` row must carry a validated output (DDL CHECK).
//!
//! The research tables of the same migration (`source_registry`,
//! `knowledge_documents`, `knowledge_chunks`, `knowledge_claims`,
//! `claim_versions`) belong to M15, which is PROPOSED; no repository exists for
//! them yet.

use super::{
    finish_update, from_json, from_ms, ms, opt_from_ms, opt_parse_id, opt_sha256, query_all,
    query_opt, required, rev_from_i64, rev_i64, to_json, u32_from_i64, ReadScope, RepoError,
    RepoResult,
};
use crate::error::{StorageError, StorageResult};
use crate::sql::{enum_from_text, enum_to_text, parse_id, require_sha256};
use g9_domain::skill::ResourceUsage;
use g9_domain::state::SkillInvocationEvent;
use g9_domain::{
    ActorId, ApprovalId, Classification, DataDomain, ErrorCategory, IdempotencyKey, InvocationId,
    ModelId, PolicyDecisionId, ResourceBudget, Revision, Scope, SkillCode, SkillFamily, SkillId,
    SkillInvocationState, SkillOutputStatus, TraceId, UtcMillis,
};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// model registry
// ---------------------------------------------------------------------------

/// Model lifecycle (mirrors `model_registry.state`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModelState {
    /// Registered, not loadable.
    Registered,
    /// Approved by the operator; loadable when the checksum matches.
    Approved,
    /// Disabled.
    Disabled,
}

/// A `model_registry` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRow {
    /// Model id.
    pub model_id: ModelId,
    /// Display name.
    pub display_name: String,
    /// Architecture.
    pub architecture: String,
    /// Where the weights come from.
    pub weights_source: String,
    /// License.
    pub license: String,
    /// Quantization.
    pub quantization: String,
    /// Hex SHA-256 of the weights file.
    pub checksum: String,
    /// Context length in tokens.
    pub context_length: u32,
    /// Tokenizer / chat template id.
    pub tokenizer_template: String,
    /// Runtime version the model was validated with.
    pub runtime_version: String,
    /// Lifecycle.
    pub state: ModelState,
    /// Approving operator.
    pub approved_by: Option<ActorId>,
    /// Approval instant.
    pub approved_at: Option<UtcMillis>,
    /// Approval token.
    pub approval_id: Option<ApprovalId>,
    /// Privacy domain (`SYSTEM`).
    pub data_domain: DataDomain,
    /// Classification.
    pub classification: Classification,
    /// Revision.
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

const MODEL_COLUMNS: &str = "model_id, display_name, architecture, weights_source, license, quantization, checksum, context_length,
    tokenizer_template, runtime_version, state, approved_by, approved_at, approval_id, data_domain, classification, revision,
    created_at, updated_at";

fn map_model(r: &Row<'_>) -> StorageResult<ModelRow> {
    Ok(ModelRow {
        model_id: parse_id(&r.get::<_, String>("model_id")?)?,
        display_name: r.get("display_name")?,
        architecture: r.get("architecture")?,
        weights_source: r.get("weights_source")?,
        license: r.get("license")?,
        quantization: r.get("quantization")?,
        checksum: r.get("checksum")?,
        context_length: u32_from_i64(r.get("context_length")?, "context_length")?,
        tokenizer_template: r.get("tokenizer_template")?,
        runtime_version: r.get("runtime_version")?,
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        approved_by: opt_parse_id(r.get("approved_by")?)?,
        approved_at: opt_from_ms(r.get("approved_at")?),
        approval_id: opt_parse_id(r.get("approval_id")?)?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Register a model (`REGISTERED`; approval is a separate operator command).
pub fn insert_model(conn: &Connection, row: &ModelRow) -> RepoResult<Revision> {
    require_sha256("checksum", &row.checksum)?;
    if row.state != ModelState::Registered {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "a new model is REGISTERED; approval is a separate command".to_string(),
        )));
    }
    conn.execute(
        &format!(
            "INSERT INTO model_registry ({MODEL_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'REGISTERED', NULL, NULL, NULL, ?11, ?12, 1, ?13, ?13)"
        ),
        params![
            row.model_id.as_str(),
            row.display_name,
            row.architecture,
            row.weights_source,
            row.license,
            row.quantization,
            row.checksum,
            i64::from(row.context_length),
            row.tokenizer_template,
            row.runtime_version,
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One model.
pub fn get_model(conn: &Connection, id: &ModelId) -> RepoResult<Option<ModelRow>> {
    query_opt(
        conn,
        &format!("SELECT {MODEL_COLUMNS} FROM model_registry WHERE model_id = ?1"),
        &[&id.as_str()],
        map_model,
    )
}

/// Every model, by id.
pub fn list_models(conn: &Connection) -> RepoResult<Vec<ModelRow>> {
    query_all(
        conn,
        &format!("SELECT {MODEL_COLUMNS} FROM model_registry ORDER BY model_id"),
        &[],
        map_model,
    )
}

/// Approval facts recorded when a model becomes `APPROVED`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelApproval {
    /// Approving operator.
    pub approved_by: ActorId,
    /// When.
    pub approved_at: UtcMillis,
    /// Consumed approval token (`Capability::ModelLoad`).
    pub approval_id: Option<ApprovalId>,
}

/// Change a model's state. `APPROVED` requires `approval`; other states clear it.
pub fn set_model_state(
    conn: &Connection,
    id: &ModelId,
    expected_revision: Revision,
    state: ModelState,
    approval: Option<&ModelApproval>,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let approval = match (state, approval) {
        (ModelState::Approved, None) => {
            return Err(RepoError::Storage(StorageError::Precondition(
                "model.approval_required".to_string(),
            )))
        }
        (ModelState::Approved, Some(a)) => Some(a),
        (_, _) => None,
    };
    let changed = conn.execute(
        "UPDATE model_registry SET state = ?3, approved_by = ?4, approved_at = ?5, approval_id = ?6,
             revision = revision + 1, updated_at = ?7
         WHERE model_id = ?1 AND revision = ?2",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            enum_to_text(&state)?,
            approval.map(|a| a.approved_by.as_str().to_string()),
            approval.map(|a| ms(a.approved_at)),
            approval.and_then(|a| a.approval_id.as_ref().map(|x| x.as_str().to_string())),
            ms(now),
        ],
    )?;
    finish_update(
        conn,
        "model_registry",
        "model_id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

/// A `model_evals` row (immutable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelEvalRow {
    /// Row id.
    pub id: String,
    /// Model evaluated.
    pub model_id: ModelId,
    /// Benchmark protocol version (`docs/evaluations/model-benchmark-protocol.md`).
    pub protocol_version: String,
    /// Evaluation set id.
    pub eval_set: String,
    /// Metrics.
    pub metrics: serde_json::Value,
    /// Hex SHA-256 of the full report.
    pub report_sha256: Option<String>,
    /// Who ran it.
    pub run_by: ActorId,
    /// When.
    pub run_at: UtcMillis,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Append a benchmark run (never updated or deleted; trigger-enforced).
pub fn append_model_eval(conn: &Connection, row: &ModelEvalRow) -> RepoResult<()> {
    opt_sha256("report_sha256", row.report_sha256.as_deref())?;
    conn.execute(
        "INSERT INTO model_evals (id, model_id, protocol_version, eval_set, metrics_json, report_sha256, run_by, run_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            row.id,
            row.model_id.as_str(),
            row.protocol_version,
            row.eval_set,
            to_json(&row.metrics)?,
            row.report_sha256,
            row.run_by.as_str(),
            ms(row.run_at),
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

/// Benchmark runs of a model, oldest first.
pub fn list_model_evals(conn: &Connection, model_id: &ModelId) -> RepoResult<Vec<ModelEvalRow>> {
    query_all(
        conn,
        "SELECT id, model_id, protocol_version, eval_set, metrics_json, report_sha256, run_by, run_at, created_at
         FROM model_evals WHERE model_id = ?1 ORDER BY run_at, id",
        &[&model_id.as_str()],
        |r| {
            Ok(ModelEvalRow {
                id: r.get("id")?,
                model_id: parse_id(&r.get::<_, String>("model_id")?)?,
                protocol_version: r.get("protocol_version")?,
                eval_set: r.get("eval_set")?,
                metrics: from_json(&r.get::<_, String>("metrics_json")?, "metrics_json")?,
                report_sha256: r.get("report_sha256")?,
                run_by: parse_id(&r.get::<_, String>("run_by")?)?,
                run_at: from_ms(r.get("run_at")?),
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

// ---------------------------------------------------------------------------
// skill registry
// ---------------------------------------------------------------------------

/// Skill lifecycle (mirrors `skill_registry.state`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SkillState {
    /// Registered.
    Registered,
    /// Package validated (schemas, hashes).
    Validated,
    /// Promoted after the three-pass evaluation.
    Promoted,
    /// Disabled.
    Disabled,
}

/// A `skill_registry` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillRow {
    /// Agent Skills name (`g9-session-director`).
    pub skill_id: SkillId,
    /// Logical code (`T01`).
    pub code: SkillCode,
    /// Family (derived from the code).
    pub family: SkillFamily,
    /// Title.
    pub title: String,
    /// Package directory under `.agents/skills/`.
    pub package_dir: String,
    /// Current version label.
    pub current_version_label: Option<String>,
    /// Lifecycle.
    pub state: SkillState,
    /// Revision.
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

const SKILL_COLUMNS: &str =
    "skill_id, code, family, title, package_dir, current_version_label, state, revision, created_at, updated_at";

fn map_skill(r: &Row<'_>) -> StorageResult<SkillRow> {
    let code = SkillCode::parse(&r.get::<_, String>("code")?).map_err(StorageError::RowDecode)?;
    Ok(SkillRow {
        skill_id: parse_id(&r.get::<_, String>("skill_id")?)?,
        family: enum_from_text(&r.get::<_, String>("family")?)?,
        code,
        title: r.get("title")?,
        package_dir: r.get("package_dir")?,
        current_version_label: r.get("current_version_label")?,
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Register a skill; the family is derived from the code.
pub fn insert_skill(conn: &Connection, row: &SkillRow) -> RepoResult<Revision> {
    let family = row.code.family();
    if family != row.family {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "skill family does not match its code".to_string(),
        )));
    }
    conn.execute(
        &format!(
            "INSERT INTO skill_registry ({SKILL_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?8)"
        ),
        params![
            row.skill_id.as_str(),
            row.code.as_str(),
            enum_to_text(&family)?,
            row.title,
            row.package_dir,
            row.current_version_label,
            enum_to_text(&row.state)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One skill.
pub fn get_skill(conn: &Connection, id: &SkillId) -> RepoResult<Option<SkillRow>> {
    query_opt(
        conn,
        &format!("SELECT {SKILL_COLUMNS} FROM skill_registry WHERE skill_id = ?1"),
        &[&id.as_str()],
        map_skill,
    )
}

/// Every skill, by code.
pub fn list_skills(conn: &Connection) -> RepoResult<Vec<SkillRow>> {
    query_all(
        conn,
        &format!("SELECT {SKILL_COLUMNS} FROM skill_registry ORDER BY code"),
        &[],
        map_skill,
    )
}

/// Change a skill's state and current version label.
pub fn set_skill_state(
    conn: &Connection,
    id: &SkillId,
    expected_revision: Revision,
    state: SkillState,
    current_version_label: Option<&str>,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let changed = conn.execute(
        "UPDATE skill_registry SET state = ?3, current_version_label = ?4, revision = revision + 1, updated_at = ?5
         WHERE skill_id = ?1 AND revision = ?2",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            enum_to_text(&state)?,
            current_version_label,
            ms(now)
        ],
    )?;
    finish_update(
        conn,
        "skill_registry",
        "skill_id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

/// A `skill_versions` row (immutable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillVersionRow {
    /// Row id.
    pub id: String,
    /// Skill.
    pub skill_id: SkillId,
    /// Version label.
    pub version_label: String,
    /// Hex SHA-256 of the whole package.
    pub package_sha256: String,
    /// Hex SHA-256 of `SKILL.md`.
    pub skill_md_sha256: String,
    /// Hex SHA-256 of `schemas/input.schema.json`.
    pub input_schema_sha256: String,
    /// Hex SHA-256 of `schemas/output.schema.json`.
    pub output_schema_sha256: String,
    /// Resource budget.
    pub budget: ResourceBudget,
    /// Allowed tool ids (empty for most skills).
    pub allowed_tool_ids: Vec<String>,
    /// Registering operator.
    pub registered_by: ActorId,
    /// When registered.
    pub registered_at: UtcMillis,
    /// Row creation.
    pub created_at: UtcMillis,
}

const SKILL_VERSION_COLUMNS: &str =
    "id, skill_id, version_label, package_sha256, skill_md_sha256, input_schema_sha256,
    output_schema_sha256, budget_json, allowed_tool_ids, registered_by, registered_at, created_at";

fn map_skill_version(r: &Row<'_>) -> StorageResult<SkillVersionRow> {
    Ok(SkillVersionRow {
        id: r.get("id")?,
        skill_id: parse_id(&r.get::<_, String>("skill_id")?)?,
        version_label: r.get("version_label")?,
        package_sha256: r.get("package_sha256")?,
        skill_md_sha256: r.get("skill_md_sha256")?,
        input_schema_sha256: r.get("input_schema_sha256")?,
        output_schema_sha256: r.get("output_schema_sha256")?,
        budget: from_json(&r.get::<_, String>("budget_json")?, "budget_json")?,
        allowed_tool_ids: from_json(&r.get::<_, String>("allowed_tool_ids")?, "allowed_tool_ids")?,
        registered_by: parse_id(&r.get::<_, String>("registered_by")?)?,
        registered_at: from_ms(r.get("registered_at")?),
        created_at: from_ms(r.get("created_at")?),
    })
}

/// Register a package version (no update function exists; trigger-enforced).
pub fn insert_skill_version(conn: &Connection, row: &SkillVersionRow) -> RepoResult<()> {
    require_sha256("package_sha256", &row.package_sha256)?;
    require_sha256("skill_md_sha256", &row.skill_md_sha256)?;
    require_sha256("input_schema_sha256", &row.input_schema_sha256)?;
    require_sha256("output_schema_sha256", &row.output_schema_sha256)?;
    conn.execute(
        &format!(
            "INSERT INTO skill_versions ({SKILL_VERSION_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
        ),
        params![
            row.id,
            row.skill_id.as_str(),
            row.version_label,
            row.package_sha256,
            row.skill_md_sha256,
            row.input_schema_sha256,
            row.output_schema_sha256,
            to_json(&row.budget)?,
            to_json(&row.allowed_tool_ids)?,
            row.registered_by.as_str(),
            ms(row.registered_at),
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

/// One version by row id.
pub fn get_skill_version(conn: &Connection, id: &str) -> RepoResult<Option<SkillVersionRow>> {
    query_opt(
        conn,
        &format!("SELECT {SKILL_VERSION_COLUMNS} FROM skill_versions WHERE id = ?1"),
        &[&id],
        map_skill_version,
    )
}

/// Versions of a skill, oldest registration first.
pub fn list_skill_versions(
    conn: &Connection,
    skill_id: &SkillId,
) -> RepoResult<Vec<SkillVersionRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {SKILL_VERSION_COLUMNS} FROM skill_versions WHERE skill_id = ?1 ORDER BY registered_at, id"
        ),
        &[&skill_id.as_str()],
        map_skill_version,
    )
}

/// The version named by the skill's `current_version_label`, if any.
///
/// A sub-select (not a join) keeps every selected column unqualified and
/// unambiguous: `skill_id` and `created_at` exist on both tables.
pub fn current_skill_version(
    conn: &Connection,
    skill_id: &SkillId,
) -> RepoResult<Option<SkillVersionRow>> {
    query_opt(
        conn,
        &format!(
            "SELECT {SKILL_VERSION_COLUMNS} FROM skill_versions
             WHERE skill_id = ?1
               AND version_label = (SELECT current_version_label FROM skill_registry WHERE skill_id = ?1)"
        ),
        &[&skill_id.as_str()],
        map_skill_version,
    )
}

// ---------------------------------------------------------------------------
// skill invocations
// ---------------------------------------------------------------------------

/// Actor kind tag (mirrors `skill_invocations.actor_kind`; the skill id of a
/// skill actor has no column).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActorKindTag {
    /// Operator.
    Operator,
    /// System.
    System,
    /// Skill.
    Skill,
    /// Model.
    Model,
}

impl ActorKindTag {
    /// Tag of a domain actor.
    pub fn of(actor: &g9_domain::Actor) -> Self {
        match actor.kind {
            g9_domain::ActorKind::Operator => Self::Operator,
            g9_domain::ActorKind::System => Self::System,
            g9_domain::ActorKind::Skill { .. } => Self::Skill,
            g9_domain::ActorKind::Model => Self::Model,
        }
    }
}

/// Output validation outcome (mirrors `skill_invocations.validation_outcome`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValidationOutcome {
    /// Not validated yet.
    NotRun,
    /// Output matched the skill's output schema.
    Valid,
    /// Output rejected (A11).
    Invalid,
}

/// A `skill_invocations` row (metadata only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInvocationRow {
    /// Invocation id.
    pub id: InvocationId,
    /// Skill invoked.
    pub skill_id: SkillId,
    /// Package version used.
    pub skill_version_id: String,
    /// Requesting actor.
    pub actor_id: ActorId,
    /// Actor kind.
    pub actor_kind: ActorKindTag,
    /// Intent label (not free text from the model).
    pub intent: String,
    /// Approved scope.
    pub scope: Scope,
    /// Classification ceiling of the assembled context.
    pub data_classification: Classification,
    /// Machine state.
    pub state: SkillInvocationState,
    /// Model used.
    pub model_id: Option<ModelId>,
    /// Model id label kept even if the registry row goes.
    pub model_id_label: Option<String>,
    /// Prompt template version.
    pub template_version: Option<String>,
    /// Hex SHA-256 of the canonical input envelope.
    pub input_sha256: String,
    /// Hex SHA-256 of the validated output.
    pub output_sha256: Option<String>,
    /// Output status reported by the skill.
    pub output_status: Option<SkillOutputStatus>,
    /// Schema validation outcome.
    pub validation_outcome: ValidationOutcome,
    /// Resources used.
    pub usage: ResourceUsage,
    /// Latency in ms.
    pub latency_ms: Option<u32>,
    /// Preflight policy decision.
    pub policy_decision_id: Option<PolicyDecisionId>,
    /// Approval consumed.
    pub approval_id: Option<ApprovalId>,
    /// Idempotency key (unique per actor).
    pub idempotency_key: IdempotencyKey,
    /// Trace id.
    pub trace_id: TraceId,
    /// Error category when the invocation failed.
    pub error_category: Option<ErrorCategory>,
    /// When requested.
    pub requested_at: UtcMillis,
    /// When finished.
    pub finished_at: Option<UtcMillis>,
    /// Revision.
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

const INVOCATION_COLUMNS: &str = "id, skill_id, skill_version_id, actor_id, actor_kind, intent, scope_domain, scope_mode,
    data_classification, state, model_id, model_id_label, template_version, input_sha256, output_sha256, output_status,
    validation_outcome, tokens_used, seconds_used, tool_calls_used, latency_ms, policy_decision_id, approval_id,
    idempotency_key, trace_id, error_category, requested_at, finished_at, revision, created_at, updated_at";

fn map_invocation(r: &Row<'_>) -> StorageResult<SkillInvocationRow> {
    Ok(SkillInvocationRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        skill_id: parse_id(&r.get::<_, String>("skill_id")?)?,
        skill_version_id: r.get("skill_version_id")?,
        actor_id: parse_id(&r.get::<_, String>("actor_id")?)?,
        actor_kind: enum_from_text(&r.get::<_, String>("actor_kind")?)?,
        intent: r.get("intent")?,
        scope: Scope {
            domain: enum_from_text(&r.get::<_, String>("scope_domain")?)?,
            mode: enum_from_text(&r.get::<_, String>("scope_mode")?)?,
        },
        data_classification: enum_from_text(&r.get::<_, String>("data_classification")?)?,
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        model_id: opt_parse_id(r.get("model_id")?)?,
        model_id_label: r.get("model_id_label")?,
        template_version: r.get("template_version")?,
        input_sha256: r.get("input_sha256")?,
        output_sha256: r.get("output_sha256")?,
        output_status: r
            .get::<_, Option<String>>("output_status")?
            .as_deref()
            .map(enum_from_text)
            .transpose()?,
        validation_outcome: enum_from_text(&r.get::<_, String>("validation_outcome")?)?,
        usage: ResourceUsage {
            tokens: u32_from_i64(r.get("tokens_used")?, "tokens_used")?,
            seconds: u32_from_i64(r.get("seconds_used")?, "seconds_used")?,
            tool_calls: u32_from_i64(r.get("tool_calls_used")?, "tool_calls_used")?,
        },
        latency_ms: r
            .get::<_, Option<i64>>("latency_ms")?
            .map(|v| u32_from_i64(v, "latency_ms"))
            .transpose()?,
        policy_decision_id: opt_parse_id(r.get("policy_decision_id")?)?,
        approval_id: opt_parse_id(r.get("approval_id")?)?,
        idempotency_key: parse_id(&r.get::<_, String>("idempotency_key")?)?,
        trace_id: parse_id(&r.get::<_, String>("trace_id")?)?,
        error_category: r
            .get::<_, Option<String>>("error_category")?
            .as_deref()
            .map(enum_from_text)
            .transpose()?,
        requested_at: from_ms(r.get("requested_at")?),
        finished_at: opt_from_ms(r.get("finished_at")?),
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Record a new invocation (`REQUESTED`, validation `NOT_RUN`, no output).
pub fn insert_invocation(conn: &Connection, row: &SkillInvocationRow) -> RepoResult<Revision> {
    require_sha256("input_sha256", &row.input_sha256)?;
    if row.state != SkillInvocationState::Requested
        || row.output_sha256.is_some()
        || row.validation_outcome != ValidationOutcome::NotRun
    {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "a new invocation is REQUESTED with no output and validation NOT_RUN".to_string(),
        )));
    }
    conn.execute(
        &format!(
            "INSERT INTO skill_invocations ({INVOCATION_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'REQUESTED', ?10, ?11, ?12, ?13, NULL, NULL, 'NOT_RUN', 0, 0, 0, NULL,
                     ?14, ?15, ?16, ?17, NULL, ?18, NULL, 1, ?19, ?19)"
        ),
        params![
            row.id.as_str(),
            row.skill_id.as_str(),
            row.skill_version_id,
            row.actor_id.as_str(),
            enum_to_text(&row.actor_kind)?,
            row.intent,
            enum_to_text(&row.scope.domain)?,
            enum_to_text(&row.scope.mode)?,
            enum_to_text(&row.data_classification)?,
            row.model_id.as_ref().map(|m| m.as_str().to_string()),
            row.model_id_label,
            row.template_version,
            row.input_sha256,
            row.policy_decision_id.as_ref().map(|p| p.as_str().to_string()),
            row.approval_id.as_ref().map(|a| a.as_str().to_string()),
            row.idempotency_key.as_str(),
            row.trace_id.as_str(),
            ms(row.requested_at),
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One invocation.
pub fn get_invocation(
    conn: &Connection,
    id: &InvocationId,
) -> RepoResult<Option<SkillInvocationRow>> {
    query_opt(
        conn,
        &format!("SELECT {INVOCATION_COLUMNS} FROM skill_invocations WHERE id = ?1"),
        &[&id.as_str()],
        map_invocation,
    )
}

/// The invocation stored under `(actor, idempotency_key)`, if any.
pub fn find_invocation_by_idempotency(
    conn: &Connection,
    actor_id: &ActorId,
    key: &IdempotencyKey,
) -> RepoResult<Option<SkillInvocationRow>> {
    query_opt(
        conn,
        &format!(
            "SELECT {INVOCATION_COLUMNS} FROM skill_invocations WHERE actor_id = ?1 AND idempotency_key = ?2"
        ),
        &[&actor_id.as_str(), &key.as_str()],
        map_invocation,
    )
}

/// Invocations of a skill, newest first, at most `limit`.
pub fn list_invocations_for_skill(
    conn: &Connection,
    skill_id: &SkillId,
    limit: u32,
) -> RepoResult<Vec<SkillInvocationRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {INVOCATION_COLUMNS} FROM skill_invocations WHERE skill_id = ?1 ORDER BY requested_at DESC, id LIMIT ?2"
        ),
        &[&skill_id.as_str(), &i64::from(limit)],
        map_invocation,
    )
}

/// Invocations in a scope domain, domain-filtered in SQL, newest first.
pub fn list_invocations_in_scope(
    conn: &Connection,
    scope: &ReadScope,
    limit: u32,
) -> RepoResult<Vec<SkillInvocationRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {INVOCATION_COLUMNS} FROM skill_invocations WHERE (?1 = 1 OR scope_domain = ?2)
             ORDER BY requested_at DESC, id LIMIT ?3"
        ),
        &[&scope.sql_flag(), &scope.sql_domain()?, &i64::from(limit)],
        map_invocation,
    )
}

/// Columns a transition may set (each written only when present).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InvocationPatch {
    /// Model used.
    pub model_id: Option<ModelId>,
    /// Model label.
    pub model_id_label: Option<String>,
    /// Template version.
    pub template_version: Option<String>,
    /// Output hash (`Complete`).
    pub output_sha256: Option<String>,
    /// Output status.
    pub output_status: Option<SkillOutputStatus>,
    /// Validation outcome (`Complete` requires `VALID`).
    pub validation_outcome: Option<ValidationOutcome>,
    /// Resources used so far.
    pub usage: Option<ResourceUsage>,
    /// Latency.
    pub latency_ms: Option<u32>,
    /// Preflight decision (`Authorize`/`Deny`).
    pub policy_decision_id: Option<PolicyDecisionId>,
    /// Approval consumed.
    pub approval_id: Option<ApprovalId>,
    /// Error category (`Fail`/`Timeout`/`Deny`).
    pub error_category: Option<ErrorCategory>,
}

/// Apply a `SkillInvocationEvent` under a revision check. Terminal states set
/// `finished_at`; `COMPLETED` needs `output_sha256` and a `VALID` outcome (the
/// DDL CHECK refuses anything else); terminal rows are immutable (trigger).
pub fn transition_invocation(
    conn: &Connection,
    id: &InvocationId,
    expected_revision: Revision,
    event: SkillInvocationEvent,
    patch: &InvocationPatch,
    now: UtcMillis,
) -> RepoResult<(SkillInvocationState, Revision)> {
    opt_sha256("output_sha256", patch.output_sha256.as_deref())?;
    let current = required("skill_invocations", get_invocation(conn, id)?)?;
    if current.revision != expected_revision {
        return Err(RepoError::Storage(StorageError::StaleRevision {
            expected: Some(expected_revision),
            actual: current.revision,
        }));
    }
    if current.state.is_terminal() {
        return Err(RepoError::Storage(StorageError::Immutable(
            "skill invocation in a terminal state cannot be updated".to_string(),
        )));
    }
    let next = current.state.transition(event)?;
    if next == SkillInvocationState::Completed
        && (patch.output_sha256.is_none()
            || patch.validation_outcome != Some(ValidationOutcome::Valid))
    {
        return Err(RepoError::Storage(StorageError::Precondition(
            "skill.invocation.completed_requires_validated_output".to_string(),
        )));
    }
    let finished_at = if next.is_terminal() {
        Some(ms(now))
    } else {
        None
    };
    let usage = patch.usage.as_ref();
    let changed = conn.execute(
        "UPDATE skill_invocations
         SET state = ?3, model_id = COALESCE(?4, model_id), model_id_label = COALESCE(?5, model_id_label),
             template_version = COALESCE(?6, template_version), output_sha256 = COALESCE(?7, output_sha256),
             output_status = COALESCE(?8, output_status), validation_outcome = COALESCE(?9, validation_outcome),
             tokens_used = COALESCE(?10, tokens_used), seconds_used = COALESCE(?11, seconds_used),
             tool_calls_used = COALESCE(?12, tool_calls_used), latency_ms = COALESCE(?13, latency_ms),
             policy_decision_id = COALESCE(?14, policy_decision_id), approval_id = COALESCE(?15, approval_id),
             error_category = COALESCE(?16, error_category), finished_at = COALESCE(?17, finished_at),
             revision = revision + 1, updated_at = ?18
         WHERE id = ?1 AND revision = ?2",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            enum_to_text(&next)?,
            patch.model_id.as_ref().map(|m| m.as_str().to_string()),
            patch.model_id_label,
            patch.template_version,
            patch.output_sha256,
            patch.output_status.as_ref().map(enum_to_text).transpose()?,
            patch.validation_outcome.as_ref().map(enum_to_text).transpose()?,
            usage.map(|u| i64::from(u.tokens)),
            usage.map(|u| i64::from(u.seconds)),
            usage.map(|u| i64::from(u.tool_calls)),
            patch.latency_ms.map(i64::from),
            patch.policy_decision_id.as_ref().map(|p| p.as_str().to_string()),
            patch.approval_id.as_ref().map(|a| a.as_str().to_string()),
            patch.error_category.as_ref().map(enum_to_text).transpose()?,
            finished_at,
            ms(now),
        ],
    )?;
    let revision = finish_update(
        conn,
        "skill_invocations",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )?;
    Ok((next, revision))
}

/// Evaluation pass (mirrors `skill_evals.pass_kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvalPassKind {
    /// Positive cases.
    Positive,
    /// Negative cases.
    Negative,
    /// Adversarial cases (A07).
    Adversarial,
    /// Acceptance cases.
    Acceptance,
}

/// A `skill_evals` row (immutable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillEvalRow {
    /// Row id.
    pub id: String,
    /// Version evaluated.
    pub skill_version_id: String,
    /// Pass.
    pub pass_kind: EvalPassKind,
    /// Cases run.
    pub total_cases: u32,
    /// Cases passed.
    pub passed_cases: u32,
    /// Cases failed (`passed + failed = total`).
    pub failed_cases: u32,
    /// Hex SHA-256 of the report.
    pub report_sha256: Option<String>,
    /// Who ran it.
    pub run_by: ActorId,
    /// When.
    pub run_at: UtcMillis,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Append an evaluation result (never updated or deleted; trigger-enforced).
pub fn append_skill_eval(conn: &Connection, row: &SkillEvalRow) -> RepoResult<()> {
    opt_sha256("report_sha256", row.report_sha256.as_deref())?;
    if row.passed_cases.saturating_add(row.failed_cases) != row.total_cases {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "passed + failed must equal total cases".to_string(),
        )));
    }
    conn.execute(
        "INSERT INTO skill_evals (id, skill_version_id, pass_kind, total_cases, passed_cases, failed_cases, report_sha256, run_by,
            run_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            row.id,
            row.skill_version_id,
            enum_to_text(&row.pass_kind)?,
            i64::from(row.total_cases),
            i64::from(row.passed_cases),
            i64::from(row.failed_cases),
            row.report_sha256,
            row.run_by.as_str(),
            ms(row.run_at),
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

/// Evaluation results of a version, oldest first.
pub fn list_skill_evals(
    conn: &Connection,
    skill_version_id: &str,
) -> RepoResult<Vec<SkillEvalRow>> {
    query_all(
        conn,
        "SELECT id, skill_version_id, pass_kind, total_cases, passed_cases, failed_cases, report_sha256, run_by, run_at, created_at
         FROM skill_evals WHERE skill_version_id = ?1 ORDER BY run_at, id",
        &[&skill_version_id],
        |r| {
            Ok(SkillEvalRow {
                id: r.get("id")?,
                skill_version_id: r.get("skill_version_id")?,
                pass_kind: enum_from_text(&r.get::<_, String>("pass_kind")?)?,
                total_cases: u32_from_i64(r.get("total_cases")?, "total_cases")?,
                passed_cases: u32_from_i64(r.get("passed_cases")?, "passed_cases")?,
                failed_cases: u32_from_i64(r.get("failed_cases")?, "failed_cases")?,
                report_sha256: r.get("report_sha256")?,
                run_by: parse_id(&r.get::<_, String>("run_by")?)?,
                run_at: from_ms(r.get("run_at")?),
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::testutil::{approval, assert_immutable, db, operator_id, sha, t};
    use g9_domain::InteractionMode;

    fn model() -> ModelRow {
        ModelRow {
            model_id: ModelId::parse("model-1").unwrap(),
            display_name: "Local 8B".into(),
            architecture: "llama".into(),
            weights_source: "local://weights".into(),
            license: "Apache-2.0".into(),
            quantization: "Q4_K_M".into(),
            checksum: sha("weights"),
            context_length: 8192,
            tokenizer_template: "chatml".into(),
            runtime_version: "llama.cpp-b4000".into(),
            state: ModelState::Registered,
            approved_by: None,
            approved_at: None,
            approval_id: None,
            data_domain: DataDomain::System,
            classification: Classification::Internal,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        }
    }

    fn skill() -> SkillRow {
        SkillRow {
            skill_id: SkillId::parse("g9-session-director").unwrap(),
            code: SkillCode::parse("T01").unwrap(),
            family: SkillFamily::Teaching,
            title: "Session director".into(),
            package_dir: ".agents/skills/g9-session-director".into(),
            current_version_label: None,
            state: SkillState::Registered,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        }
    }

    fn skill_version(id: &str, label: &str) -> SkillVersionRow {
        SkillVersionRow {
            id: id.into(),
            skill_id: SkillId::parse("g9-session-director").unwrap(),
            version_label: label.into(),
            package_sha256: sha(&format!("pkg-{label}")),
            skill_md_sha256: sha("skill.md"),
            input_schema_sha256: sha("in"),
            output_schema_sha256: sha("out"),
            budget: ResourceBudget {
                max_tokens: 2048,
                max_seconds: 30,
                max_tool_calls: 0,
            },
            allowed_tool_ids: vec![],
            registered_by: operator_id(),
            registered_at: t(0),
            created_at: t(0),
        }
    }

    fn invocation(id: &str, key: &str) -> SkillInvocationRow {
        SkillInvocationRow {
            id: InvocationId::parse(id).unwrap(),
            skill_id: SkillId::parse("g9-session-director").unwrap(),
            skill_version_id: "sv-1".into(),
            actor_id: operator_id(),
            actor_kind: ActorKindTag::Operator,
            intent: "plan_session".into(),
            scope: Scope {
                domain: DataDomain::Learning,
                mode: InteractionMode::Study,
            },
            data_classification: Classification::Internal,
            state: SkillInvocationState::Requested,
            model_id: None,
            model_id_label: None,
            template_version: None,
            input_sha256: sha("input"),
            output_sha256: None,
            output_status: None,
            validation_outcome: ValidationOutcome::NotRun,
            usage: ResourceUsage::default(),
            latency_ms: None,
            policy_decision_id: None,
            approval_id: None,
            idempotency_key: IdempotencyKey::parse(key).unwrap(),
            trace_id: TraceId::parse("trace-1").unwrap(),
            error_category: None,
            requested_at: t(0),
            finished_at: None,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        }
    }

    #[test]
    fn model_registry_lifecycle_and_immutable_evals() {
        let db = db();
        let conn = db.conn();
        let row = model();
        insert_model(conn, &row).unwrap();
        assert_eq!(get_model(conn, &row.model_id).unwrap().unwrap(), row);
        assert_eq!(list_models(conn).unwrap().len(), 1);
        assert!(matches!(
            insert_model(
                conn,
                &ModelRow {
                    model_id: ModelId::parse("m2").unwrap(),
                    state: ModelState::Approved,
                    ..row.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        assert!(matches!(
            set_model_state(
                conn,
                &row.model_id,
                Revision::INITIAL,
                ModelState::Approved,
                None,
                t(1)
            ),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        let apr = approval(conn, "apr-1");
        let approval_ref = ModelApproval {
            approved_by: operator_id(),
            approved_at: t(1),
            approval_id: Some(apr),
        };
        assert!(matches!(
            set_model_state(
                conn,
                &row.model_id,
                Revision::new(2),
                ModelState::Approved,
                Some(&approval_ref),
                t(1)
            ),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        let r2 = set_model_state(
            conn,
            &row.model_id,
            Revision::INITIAL,
            ModelState::Approved,
            Some(&approval_ref),
            t(1),
        )
        .unwrap();
        let approved = get_model(conn, &row.model_id).unwrap().unwrap();
        assert_eq!(approved.state, ModelState::Approved);
        assert_eq!(approved.approved_by, Some(operator_id()));
        assert_eq!(
            approved.approval_id.as_ref().map(|a| a.as_str()),
            Some("apr-1")
        );
        let r3 =
            set_model_state(conn, &row.model_id, r2, ModelState::Disabled, None, t(2)).unwrap();
        let disabled = get_model(conn, &row.model_id).unwrap().unwrap();
        assert_eq!(disabled.state, ModelState::Disabled);
        assert_eq!(disabled.approved_by, None);
        assert_eq!(disabled.revision, r3);

        let eval = ModelEvalRow {
            id: "me-1".into(),
            model_id: row.model_id.clone(),
            protocol_version: "1.0".into(),
            eval_set: "schema-adherence".into(),
            metrics: serde_json::json!({ "valid_json_rate": 0.98 }),
            report_sha256: Some(sha("report")),
            run_by: operator_id(),
            run_at: t(3),
            created_at: t(3),
        };
        append_model_eval(conn, &eval).unwrap();
        assert_eq!(list_model_evals(conn, &row.model_id).unwrap(), vec![eval]);
        assert_immutable(
            conn,
            "UPDATE model_evals SET eval_set = 'x' WHERE id = 'me-1'",
        );
        assert_immutable(conn, "DELETE FROM model_evals WHERE id = 'me-1'");
    }

    #[test]
    fn skill_registry_versions_and_evals() {
        let db = db();
        let conn = db.conn();
        let row = skill();
        insert_skill(conn, &row).unwrap();
        assert_eq!(get_skill(conn, &row.skill_id).unwrap().unwrap(), row);
        assert!(matches!(
            insert_skill(
                conn,
                &SkillRow {
                    skill_id: SkillId::parse("x").unwrap(),
                    code: SkillCode::parse("E01").unwrap(),
                    ..row.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        insert_skill_version(conn, &skill_version("sv-1", "1.0.0")).unwrap();
        let mut v2 = skill_version("sv-2", "1.1.0");
        v2.registered_at = t(10);
        insert_skill_version(conn, &v2).unwrap();
        assert_eq!(list_skill_versions(conn, &row.skill_id).unwrap().len(), 2);
        assert_eq!(get_skill_version(conn, "sv-2").unwrap().unwrap(), v2);
        assert!(current_skill_version(conn, &row.skill_id)
            .unwrap()
            .is_none());
        let r2 = set_skill_state(
            conn,
            &row.skill_id,
            Revision::INITIAL,
            SkillState::Validated,
            Some("1.1.0"),
            t(11),
        )
        .unwrap();
        assert_eq!(
            current_skill_version(conn, &row.skill_id)
                .unwrap()
                .unwrap()
                .id,
            "sv-2"
        );
        assert!(matches!(
            set_skill_state(
                conn,
                &row.skill_id,
                Revision::INITIAL,
                SkillState::Promoted,
                Some("1.1.0"),
                t(12)
            ),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        assert_eq!(list_skills(conn).unwrap()[0].revision, r2);
        assert_immutable(
            conn,
            "UPDATE skill_versions SET package_sha256 = '0' WHERE id = 'sv-1'",
        );
        assert_immutable(conn, "DELETE FROM skill_versions WHERE id = 'sv-1'");

        let eval = SkillEvalRow {
            id: "se-1".into(),
            skill_version_id: "sv-2".into(),
            pass_kind: EvalPassKind::Adversarial,
            total_cases: 10,
            passed_cases: 10,
            failed_cases: 0,
            report_sha256: None,
            run_by: operator_id(),
            run_at: t(20),
            created_at: t(20),
        };
        append_skill_eval(conn, &eval).unwrap();
        assert!(matches!(
            append_skill_eval(
                conn,
                &SkillEvalRow {
                    id: "se-bad".into(),
                    failed_cases: 3,
                    ..eval.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        assert_eq!(list_skill_evals(conn, "sv-2").unwrap(), vec![eval]);
        assert_immutable(
            conn,
            "UPDATE skill_evals SET passed_cases = 0, failed_cases = 10 WHERE id = 'se-1'",
        );
        assert_immutable(conn, "DELETE FROM skill_evals WHERE id = 'se-1'");
    }

    #[test]
    fn invocation_lifecycle_is_metadata_only_and_terminal_rows_freeze() {
        let db = db();
        let conn = db.conn();
        insert_skill(conn, &skill()).unwrap();
        insert_skill_version(conn, &skill_version("sv-1", "1.0.0")).unwrap();
        insert_model(conn, &model()).unwrap();
        let row = invocation("inv-1", "idem-1");
        insert_invocation(conn, &row).unwrap();
        assert_eq!(get_invocation(conn, &row.id).unwrap().unwrap(), row);
        assert_eq!(
            find_invocation_by_idempotency(conn, &operator_id(), &row.idempotency_key)
                .unwrap()
                .unwrap()
                .id,
            row.id
        );
        assert!(matches!(
            insert_invocation(conn, &invocation("inv-dup", "idem-1")),
            Err(RepoError::Storage(StorageError::UniqueViolation(_)))
        ));
        assert!(matches!(
            insert_invocation(
                conn,
                &SkillInvocationRow {
                    id: InvocationId::parse("inv-x").unwrap(),
                    state: SkillInvocationState::Running,
                    ..row.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));

        let (s, r) = transition_invocation(
            conn,
            &row.id,
            Revision::INITIAL,
            SkillInvocationEvent::Preflight,
            &InvocationPatch::default(),
            t(1),
        )
        .unwrap();
        assert_eq!(s, SkillInvocationState::Preflight);
        assert!(matches!(
            transition_invocation(
                conn,
                &row.id,
                Revision::INITIAL,
                SkillInvocationEvent::Authorize,
                &InvocationPatch::default(),
                t(1)
            ),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        assert!(matches!(
            transition_invocation(
                conn,
                &row.id,
                r,
                SkillInvocationEvent::Complete,
                &InvocationPatch::default(),
                t(1)
            ),
            Err(RepoError::Storage(StorageError::InvalidTransition(_)))
        ));
        let (_, r) = transition_invocation(
            conn,
            &row.id,
            r,
            SkillInvocationEvent::Authorize,
            &InvocationPatch::default(),
            t(2),
        )
        .unwrap();
        let (_, r) = transition_invocation(
            conn,
            &row.id,
            r,
            SkillInvocationEvent::Run,
            &InvocationPatch {
                model_id: Some(ModelId::parse("model-1").unwrap()),
                model_id_label: Some("model-1".into()),
                template_version: Some("t1".into()),
                ..InvocationPatch::default()
            },
            t(3),
        )
        .unwrap();
        let (_, r) = transition_invocation(
            conn,
            &row.id,
            r,
            SkillInvocationEvent::Validate,
            &InvocationPatch::default(),
            t(4),
        )
        .unwrap();
        // completion without a validated output is refused
        assert!(matches!(
            transition_invocation(
                conn,
                &row.id,
                r,
                SkillInvocationEvent::Complete,
                &InvocationPatch::default(),
                t(5)
            ),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        let (s, r) = transition_invocation(
            conn,
            &row.id,
            r,
            SkillInvocationEvent::Complete,
            &InvocationPatch {
                output_sha256: Some(sha("output")),
                output_status: Some(SkillOutputStatus::Completed),
                validation_outcome: Some(ValidationOutcome::Valid),
                usage: Some(ResourceUsage {
                    tokens: 512,
                    seconds: 3,
                    tool_calls: 0,
                }),
                latency_ms: Some(2_900),
                ..InvocationPatch::default()
            },
            t(5),
        )
        .unwrap();
        assert_eq!(s, SkillInvocationState::Completed);
        let done = get_invocation(conn, &row.id).unwrap().unwrap();
        assert_eq!(done.revision, r);
        assert_eq!(done.finished_at, Some(t(5)));
        assert_eq!(done.usage.tokens, 512);
        assert_eq!(done.model_id_label.as_deref(), Some("model-1"));
        assert_eq!(done.validation_outcome, ValidationOutcome::Valid);
        // terminal rows are frozen in the repository and by the trigger
        assert!(matches!(
            transition_invocation(
                conn,
                &row.id,
                r,
                SkillInvocationEvent::Cancel,
                &InvocationPatch::default(),
                t(6)
            ),
            Err(RepoError::Storage(StorageError::Immutable(_)))
        ));
        assert_immutable(
            conn,
            "UPDATE skill_invocations SET tokens_used = 0 WHERE id = 'inv-1'",
        );
        assert_immutable(conn, "DELETE FROM skill_invocations WHERE id = 'inv-1'");

        // a denied invocation records the error category and finishes
        let denied = invocation("inv-2", "idem-2");
        insert_invocation(conn, &denied).unwrap();
        let (_, r) = transition_invocation(
            conn,
            &denied.id,
            Revision::INITIAL,
            SkillInvocationEvent::Preflight,
            &InvocationPatch::default(),
            t(7),
        )
        .unwrap();
        let (s, _) = transition_invocation(
            conn,
            &denied.id,
            r,
            SkillInvocationEvent::Deny,
            &InvocationPatch {
                error_category: Some(ErrorCategory::Unauthorized),
                ..InvocationPatch::default()
            },
            t(8),
        )
        .unwrap();
        assert_eq!(s, SkillInvocationState::Denied);
        let stored = get_invocation(conn, &denied.id).unwrap().unwrap();
        assert_eq!(stored.error_category, Some(ErrorCategory::Unauthorized));
        assert_eq!(stored.finished_at, Some(t(8)));
        assert_eq!(
            list_invocations_for_skill(conn, &row.skill_id, 10)
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            list_invocations_in_scope(conn, &ReadScope::domain(DataDomain::Business), 10)
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            list_invocations_in_scope(conn, &ReadScope::domain(DataDomain::Learning), 10)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn invocation_rows_refuse_exam_secrets_unvalidated_completion_and_carry_no_content() {
        let db = db();
        let conn = db.conn();
        insert_skill(conn, &skill()).unwrap();
        insert_skill_version(conn, &skill_version("sv-1", "1.0.0")).unwrap();

        // EXAM mode with SECRET context is refused by the DDL CHECK (sealed keys, SEC-10)
        let mut exam = invocation("inv-exam", "idem-exam");
        exam.scope.mode = InteractionMode::Exam;
        exam.data_classification = Classification::Secret;
        assert!(matches!(
            insert_invocation(conn, &exam),
            Err(RepoError::Storage(StorageError::CheckViolation(_)))
        ));
        assert!(get_invocation(conn, &exam.id).unwrap().is_none());

        // an output that failed schema validation ends in ERROR, never COMPLETED (A11)
        let row = invocation("inv-1", "idem-1");
        insert_invocation(conn, &row).unwrap();
        let mut r = Revision::INITIAL;
        for event in [
            SkillInvocationEvent::Preflight,
            SkillInvocationEvent::Authorize,
            SkillInvocationEvent::Run,
            SkillInvocationEvent::Validate,
        ] {
            r = transition_invocation(conn, &row.id, r, event, &InvocationPatch::default(), t(1))
                .unwrap()
                .1;
        }
        assert!(matches!(
            transition_invocation(
                conn,
                &row.id,
                r,
                SkillInvocationEvent::Complete,
                &InvocationPatch {
                    output_sha256: Some(sha("bad-output")),
                    validation_outcome: Some(ValidationOutcome::Invalid),
                    ..InvocationPatch::default()
                },
                t(2),
            ),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        // the DDL CHECK is the second line of defence
        let raw: StorageError = conn
            .execute(
                "UPDATE skill_invocations SET state = 'COMPLETED', output_sha256 = ?1, validation_outcome = 'INVALID'
                 WHERE id = 'inv-1'",
                params![sha("bad-output")],
            )
            .expect_err("CHECK must refuse")
            .into();
        assert!(matches!(raw, StorageError::CheckViolation(_)), "{raw:?}");
        let (state, _) = transition_invocation(
            conn,
            &row.id,
            r,
            SkillInvocationEvent::Fail,
            &InvocationPatch {
                output_sha256: Some(sha("bad-output")),
                validation_outcome: Some(ValidationOutcome::Invalid),
                error_category: Some(ErrorCategory::InvalidInput),
                ..InvocationPatch::default()
            },
            t(3),
        )
        .unwrap();
        assert_eq!(state, SkillInvocationState::Error);
        let stored = get_invocation(conn, &row.id).unwrap().unwrap();
        assert_eq!(stored.validation_outcome, ValidationOutcome::Invalid);
        assert_eq!(stored.finished_at, Some(t(3)));
        assert_eq!(stored.error_category, Some(ErrorCategory::InvalidInput));

        // metadata only: the table has no column that could hold a prompt or a model output
        let columns: Vec<String> = conn
            .prepare("SELECT name FROM pragma_table_info('skill_invocations')")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(|c| c.unwrap())
            .collect();
        assert!(columns.contains(&"input_sha256".to_string()));
        assert!(
            !columns.iter().any(|c| c.contains("prompt")
                || c == "output"
                || c.contains("text")
                || c.contains("content")),
            "{columns:?}"
        );

        // a version label the registry does not name resolves to no current version
        set_skill_state(
            conn,
            &row.skill_id,
            Revision::INITIAL,
            SkillState::Validated,
            Some("9.9.9"),
            t(4),
        )
        .unwrap();
        assert!(current_skill_version(conn, &row.skill_id)
            .unwrap()
            .is_none());
    }
}
