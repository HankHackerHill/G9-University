//! Assessment repository (M07; migration `0006_assessment`, tables
//! `assessment_definitions`, `assessment_versions`, `rubrics`, `rubric_criteria`,
//! `attempts`, `attempt_evidence`, `criterion_results`, `grade_decisions`,
//! `retake_links`, `exposure_log`).
//!
//! Storage rules (A04): versions, rubrics, criterion results, retake links and
//! exposure rows are never updated; attempts stop accepting updates at
//! `APPROVED_FINAL`; a grade decision with `is_final = 1` is immutable and a
//! correction is a new superseding row. The repository checks that an attempt
//! update follows the `AssessmentAttemptState` table, never lowers the recorded
//! assistance level and never decreases the hint count; the triggers are the
//! second line of defence and the tests prove they fire.
//!
//! DDL ↔ wave-1 gaps (adapters refuse rather than guess): `assessment_definitions.kind`
//! lacks `WEEKLY_ASSESSMENT`/`DIAGNOSTIC_CALIBRATION` and has `QUIZ`/`REPORT`;
//! a definition stores one `competency_id`; `assessment_versions` has no
//! `equivalence_group` or `checksum` column (its `difficulty` column exists
//! since `0013`, is nullable and is never defaulted: `None` means the author
//! declared none and no competency observation may be derived from the
//! version); `rubric_criteria` has
//! no `dimension` column; `exposure_log.content_kind` lacks `HINT` and folds the
//! reference solution into `KEY`. Rubric anchors are stored since `0012`, but a
//! non-standard level scale is refused until `g9-app` regrades through
//! [`rubric_from_stored`] (PROPOSED). `g9_assessment::Attempt` keeps its fields
//! private, so [`AttemptRow::to_attempt`] rehydrates through serde with the
//! definition-level facts the row does not carry.

use super::{
    bool_i64, finish_update, from_json, from_ms, i64_bool, ms, opt_from_ms, opt_ms, opt_parse_id,
    opt_sha256, query_all, query_opt, required, rev_from_i64, rev_i64, to_json, u32_from_i64,
    u8_from_i64, ReadScope, RepoError, RepoResult,
};
use crate::error::{StorageError, StorageResult};
use crate::sql::{enum_from_text, enum_to_text, parse_id, require_sha256};
use g9_assessment::{
    AssessmentDefinition, AssessmentKind, AssessmentVersion, Attempt, CriterionResult,
    ExposureKind, ExposureRecord, GradeDecision, LevelScale, Rubric, RubricAnchors,
    RubricCriterionDef, Score,
};
use g9_domain::{
    ActorId, ApprovalId, AssessmentAttemptState, AssessmentId, AssistanceLevel, AttemptId,
    Classification, CompetencyId, DataDomain, Difficulty, EvidenceId, GradeDecisionId, GradeSource,
    InteractionMode, LearningDimension, Revision, SessionId, UtcMillis,
};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// definitions and versions
// ---------------------------------------------------------------------------

/// Definition kind (mirrors the `assessment_definitions.kind` CHECK list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssessmentDefinitionKind {
    /// Blind practical.
    BlindPractical,
    /// Quiz.
    Quiz,
    /// Written report.
    Report,
    /// Capstone.
    Capstone,
    /// Practical transfer check.
    TransferCheck,
}

impl AssessmentDefinitionKind {
    /// Map the assessment crate's kind; unsupported kinds are refused (gap).
    pub fn from_domain(kind: AssessmentKind) -> StorageResult<Self> {
        Ok(match kind {
            AssessmentKind::BlindPractical => Self::BlindPractical,
            AssessmentKind::TransferCheck => Self::TransferCheck,
            AssessmentKind::Capstone => Self::Capstone,
            AssessmentKind::WeeklyAssessment => {
                return Err(StorageError::NotImplemented(
                    "assessment.kind.weekly_assessment",
                ))
            }
            AssessmentKind::DiagnosticCalibration => {
                return Err(StorageError::NotImplemented(
                    "assessment.kind.diagnostic_calibration",
                ))
            }
        })
    }

    /// The assessment crate's kind, if representable.
    pub const fn to_domain(self) -> Option<AssessmentKind> {
        Some(match self {
            Self::BlindPractical => AssessmentKind::BlindPractical,
            Self::TransferCheck => AssessmentKind::TransferCheck,
            Self::Capstone => AssessmentKind::Capstone,
            Self::Quiz | Self::Report => return None,
        })
    }
}

/// An `assessment_definitions` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssessmentDefinitionRow {
    /// Definition id.
    pub id: AssessmentId,
    /// Unique code (`W1-BLIND`).
    pub code: String,
    /// Title.
    pub title: String,
    /// Kind.
    pub kind: AssessmentDefinitionKind,
    /// Linked objective row.
    pub objective_id: Option<String>,
    /// Linked training-day row.
    pub training_day_id: Option<String>,
    /// Assessed competency.
    pub competency_id: Option<CompetencyId>,
    /// Interaction mode attempts run in.
    pub mode: InteractionMode,
    /// Privacy domain.
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

impl AssessmentDefinitionRow {
    /// Build a row from a validated definition. A definition with more than one
    /// competency is refused (single `competency_id` column; gap).
    pub fn from_domain(
        definition: &AssessmentDefinition,
        code: &str,
        now: UtcMillis,
    ) -> StorageResult<Self> {
        if definition.competency_ids.len() > 1 {
            return Err(StorageError::NotImplemented(
                "assessment.definition.multi_competency",
            ));
        }
        Ok(Self {
            id: definition.id.clone(),
            code: code.to_string(),
            title: definition.title.clone(),
            kind: AssessmentDefinitionKind::from_domain(definition.kind)?,
            objective_id: None,
            training_day_id: None,
            competency_id: definition.competency_ids.first().cloned(),
            mode: definition
                .kind
                .required_mode()
                .unwrap_or(InteractionMode::Study),
            data_domain: definition.data_domain,
            classification: Classification::Internal,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        })
    }

    /// The assessment crate's definition (requires a representable kind and a competency).
    pub fn to_domain(&self) -> StorageResult<AssessmentDefinition> {
        let kind = self
            .kind
            .to_domain()
            .ok_or(StorageError::NotImplemented("assessment.kind.unmapped"))?;
        let competency = self
            .competency_id
            .clone()
            .ok_or_else(|| StorageError::RowDecode("definition without competency".to_string()))?;
        AssessmentDefinition::new(
            self.id.clone(),
            self.title.clone(),
            vec![competency],
            kind,
            self.data_domain,
        )
        .map_err(|e| StorageError::RowDecode(format!("assessment definition: {e}")))
    }
}

const DEFINITION_COLUMNS: &str =
    "id, code, title, kind, objective_id, training_day_id, competency_id, mode, data_domain,
    classification, revision, created_at, updated_at";

fn map_definition(r: &Row<'_>) -> StorageResult<AssessmentDefinitionRow> {
    Ok(AssessmentDefinitionRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        code: r.get("code")?,
        title: r.get("title")?,
        kind: enum_from_text(&r.get::<_, String>("kind")?)?,
        objective_id: r.get("objective_id")?,
        training_day_id: r.get("training_day_id")?,
        competency_id: opt_parse_id(r.get("competency_id")?)?,
        mode: enum_from_text(&r.get::<_, String>("mode")?)?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert a definition.
pub fn insert_definition(conn: &Connection, row: &AssessmentDefinitionRow) -> RepoResult<Revision> {
    conn.execute(
        &format!(
            "INSERT INTO assessment_definitions ({DEFINITION_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?11)"
        ),
        params![
            row.id.as_str(),
            row.code,
            row.title,
            enum_to_text(&row.kind)?,
            row.objective_id,
            row.training_day_id,
            row.competency_id.as_ref().map(|c| c.as_str().to_string()),
            enum_to_text(&row.mode)?,
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One definition, scope-checked.
pub fn get_definition(
    conn: &Connection,
    scope: &ReadScope,
    id: &AssessmentId,
) -> RepoResult<Option<AssessmentDefinitionRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {DEFINITION_COLUMNS} FROM assessment_definitions WHERE id = ?1"),
        &[&id.as_str()],
        map_definition,
    )?;
    if let Some(r) = &row {
        scope.check("assessment_definitions", r.data_domain)?;
    }
    Ok(row)
}

/// Every definition, domain-filtered in SQL, by code.
pub fn list_definitions(
    conn: &Connection,
    scope: &ReadScope,
) -> RepoResult<Vec<AssessmentDefinitionRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {DEFINITION_COLUMNS} FROM assessment_definitions WHERE (?1 = 1 OR data_domain = ?2) ORDER BY code"
        ),
        &[&scope.sql_flag(), &scope.sql_domain()?],
        map_definition,
    )
}

/// Replace the mutable columns of a definition (title, kind, links, mode, classification).
pub fn update_definition(
    conn: &Connection,
    row: &AssessmentDefinitionRow,
    expected_revision: Revision,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let changed = conn.execute(
        "UPDATE assessment_definitions
         SET title = ?3, kind = ?4, objective_id = ?5, training_day_id = ?6, competency_id = ?7, mode = ?8,
             classification = ?9, revision = revision + 1, updated_at = ?10
         WHERE id = ?1 AND revision = ?2",
        params![
            row.id.as_str(),
            rev_i64(expected_revision),
            row.title,
            enum_to_text(&row.kind)?,
            row.objective_id,
            row.training_day_id,
            row.competency_id.as_ref().map(|c| c.as_str().to_string()),
            enum_to_text(&row.mode)?,
            enum_to_text(&row.classification)?,
            ms(now),
        ],
    )?;
    finish_update(
        conn,
        "assessment_definitions",
        "id",
        row.id.as_str(),
        expected_revision,
        changed,
    )
}

/// An `assessment_versions` row (immutable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssessmentVersionRow {
    /// Row id.
    pub id: String,
    /// Definition.
    pub definition_id: AssessmentId,
    /// Version label (the domain version number as text).
    pub version_label: String,
    /// Task statement shown to the learner.
    pub task_statement: String,
    /// Hex SHA-256 of the task statement.
    pub task_sha256: String,
    /// Content-addressed reference to the sealed answer key (`SECRET`).
    pub sealed_key_ref: Option<String>,
    /// Hex SHA-256 of the sealed key bytes.
    pub sealed_key_sha256: Option<String>,
    /// Pass threshold (default 80).
    pub pass_threshold: u8,
    /// Task difficulty declared by the version's author (`assessment_versions.difficulty`,
    /// migration `0013`). `None` = none declared (rows written before `0013`, or an
    /// author who declared none): it is never defaulted or inferred, and no
    /// competency observation is derived from an attempt on such a version.
    pub difficulty: Option<Difficulty>,
    /// Creating operator.
    pub created_by: ActorId,
    /// Row creation.
    pub created_at: UtcMillis,
}

impl AssessmentVersionRow {
    /// Build a row from a checksummed version. `task_statement` is the resolved
    /// task text (the domain type holds an evidence reference). The version's
    /// declared `difficulty` is persisted; `equivalence_group` and `checksum`
    /// have no column and are not persisted (gap).
    pub fn from_domain(
        version: &AssessmentVersion,
        id: &str,
        task_statement: &str,
        sealed_key_sha256: Option<String>,
        created_by: ActorId,
    ) -> StorageResult<Self> {
        version
            .verify_checksum()
            .map_err(|e| StorageError::InvalidInput(format!("assessment version: {e}")))?;
        Ok(Self {
            id: id.to_string(),
            definition_id: version.definition_id.clone(),
            version_label: version.version.to_string(),
            task_statement: task_statement.to_string(),
            task_sha256: g9_domain::hashing::sha256_hex(task_statement.as_bytes()),
            sealed_key_ref: version
                .sealed_key_ref
                .as_ref()
                .map(|a| a.as_str().to_string()),
            sealed_key_sha256,
            pass_threshold: g9_domain::PASS_THRESHOLD,
            difficulty: Some(version.difficulty),
            created_by,
            created_at: version.created_at,
        })
    }

    /// Version number parsed from the label.
    pub fn version_number(&self) -> StorageResult<u32> {
        self.version_label
            .parse()
            .map_err(|_| StorageError::RowDecode("version_label is not a number".to_string()))
    }
}

const VERSION_COLUMNS: &str = "id, definition_id, version_label, task_statement, task_sha256, sealed_key_ref, sealed_key_sha256,
    pass_threshold, difficulty, created_by, created_at";

fn map_version(r: &Row<'_>) -> StorageResult<AssessmentVersionRow> {
    Ok(AssessmentVersionRow {
        id: r.get("id")?,
        definition_id: parse_id(&r.get::<_, String>("definition_id")?)?,
        version_label: r.get("version_label")?,
        task_statement: r.get("task_statement")?,
        task_sha256: r.get("task_sha256")?,
        sealed_key_ref: r.get("sealed_key_ref")?,
        sealed_key_sha256: r.get("sealed_key_sha256")?,
        pass_threshold: u8_from_i64(r.get("pass_threshold")?, "pass_threshold")?,
        difficulty: r
            .get::<_, Option<i64>>("difficulty")?
            .map(|d| {
                Difficulty::new(u8_from_i64(d, "difficulty")?).map_err(StorageError::RowDecode)
            })
            .transpose()?,
        created_by: parse_id(&r.get::<_, String>("created_by")?)?,
        created_at: from_ms(r.get("created_at")?),
    })
}

/// Insert an immutable version (no update function exists).
pub fn insert_version(conn: &Connection, row: &AssessmentVersionRow) -> RepoResult<()> {
    require_sha256("task_sha256", &row.task_sha256)?;
    opt_sha256("sealed_key_sha256", row.sealed_key_sha256.as_deref())?;
    conn.execute(
        &format!(
            "INSERT INTO assessment_versions ({VERSION_COLUMNS}, key_classification)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'SECRET')"
        ),
        params![
            row.id,
            row.definition_id.as_str(),
            row.version_label,
            row.task_statement,
            row.task_sha256,
            row.sealed_key_ref,
            row.sealed_key_sha256,
            i64::from(row.pass_threshold),
            row.difficulty.map(|d| i64::from(d.get())),
            row.created_by.as_str(),
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

/// One version by row id.
pub fn get_version(conn: &Connection, id: &str) -> RepoResult<Option<AssessmentVersionRow>> {
    query_opt(
        conn,
        &format!("SELECT {VERSION_COLUMNS} FROM assessment_versions WHERE id = ?1"),
        &[&id],
        map_version,
    )
}

/// One version by `(definition, label)`.
pub fn version_by_label(
    conn: &Connection,
    definition_id: &AssessmentId,
    label: &str,
) -> RepoResult<Option<AssessmentVersionRow>> {
    query_opt(
        conn,
        &format!(
            "SELECT {VERSION_COLUMNS} FROM assessment_versions WHERE definition_id = ?1 AND version_label = ?2"
        ),
        &[&definition_id.as_str(), &label],
        map_version,
    )
}

/// Versions of a definition, oldest first.
pub fn list_versions(
    conn: &Connection,
    definition_id: &AssessmentId,
) -> RepoResult<Vec<AssessmentVersionRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {VERSION_COLUMNS} FROM assessment_versions WHERE definition_id = ?1 ORDER BY created_at, id"
        ),
        &[&definition_id.as_str()],
        map_version,
    )
}

// ---------------------------------------------------------------------------
// rubrics
// ---------------------------------------------------------------------------

/// A `rubrics` row (immutable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubricRow {
    /// Row id.
    pub id: String,
    /// Version the rubric grades.
    pub assessment_version_id: String,
    /// Title.
    pub title: String,
    /// Label (unique per version).
    pub version_label: String,
    /// Total weight (must be 100).
    pub total_weight: u32,
    /// Anchored scoring standard (level scale, standard-setting note,
    /// descriptors; `rubrics.anchors_json`, migration `0012`, hash-checked on
    /// read). `None` only for rows written before `0012`; they read back with
    /// the standard anchors.
    pub anchors: Option<RubricAnchors>,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// A `rubric_criteria` row (immutable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubricCriterionRow {
    /// Row id.
    pub id: String,
    /// Rubric.
    pub rubric_id: String,
    /// Criterion code.
    pub code: String,
    /// Title.
    pub title: String,
    /// Order.
    pub ordinal: u32,
    /// Weight 0..=100.
    pub weight: u8,
    /// Machine-checkable flag.
    pub machine_checkable: bool,
    /// Critical gate flag.
    pub critical_gate: bool,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Rows for a validated `Rubric` (criterion ids are `<rubric_id>:<code>`). The
/// rubric's anchors travel on the [`RubricRow`] and are persisted by
/// [`insert_rubric`].
pub fn rubric_rows_from_domain(
    rubric: &Rubric,
    rubric_id: &str,
    assessment_version_id: &str,
    title: &str,
    version_label: &str,
    now: UtcMillis,
) -> (RubricRow, Vec<RubricCriterionRow>) {
    let criteria = rubric
        .criteria()
        .iter()
        .enumerate()
        .map(|(i, c)| RubricCriterionRow {
            id: format!("{rubric_id}:{}", c.code),
            rubric_id: rubric_id.to_string(),
            code: c.code.clone(),
            title: c.title.clone(),
            ordinal: u32::try_from(i).unwrap_or(u32::MAX).saturating_add(1),
            weight: c.weight,
            machine_checkable: c.machine_checkable,
            critical_gate: c.critical_gate,
            created_at: now,
        })
        .collect();
    (
        RubricRow {
            id: rubric_id.to_string(),
            assessment_version_id: assessment_version_id.to_string(),
            title: title.to_string(),
            version_label: version_label.to_string(),
            total_weight: rubric.total_weight(),
            anchors: Some(rubric.anchors().clone()),
            created_at: now,
        },
        criteria,
    )
}

/// Rebuild a `Rubric` from criterion rows with the STANDARD anchors. Correct
/// for every stored rubric because [`insert_rubric`] refuses a non-standard
/// level scale (the only anchor part that changes scores); prefer
/// [`rubric_from_stored`], which also restores the stored note and descriptors.
/// `rubric_criteria` has no dimension column (gap), so the caller supplies the
/// code → dimension mapping (for the baseline rubric `Rubric::baseline_dimension`).
pub fn rubric_from_rows(
    criteria: &[RubricCriterionRow],
    dimension_of: impl Fn(&str) -> Option<LearningDimension>,
) -> StorageResult<Rubric> {
    let defs = criterion_defs(criteria, dimension_of)?;
    Rubric::new(defs).map_err(|e| StorageError::RowDecode(format!("rubric: {e}")))
}

/// Rebuild a `Rubric` exactly as stored: criteria plus the persisted anchors
/// (`Rubric::with_anchors`); a pre-`0012` row without anchors gets the standard ones.
pub fn rubric_from_stored(
    rubric: &RubricRow,
    criteria: &[RubricCriterionRow],
    dimension_of: impl Fn(&str) -> Option<LearningDimension>,
) -> StorageResult<Rubric> {
    let defs = criterion_defs(criteria, dimension_of)?;
    match &rubric.anchors {
        Some(anchors) => Rubric::with_anchors(defs, anchors.clone()),
        None => Rubric::new(defs),
    }
    .map_err(|e| StorageError::RowDecode(format!("rubric: {e}")))
}

fn criterion_defs(
    criteria: &[RubricCriterionRow],
    dimension_of: impl Fn(&str) -> Option<LearningDimension>,
) -> StorageResult<Vec<RubricCriterionDef>> {
    let mut defs = Vec::with_capacity(criteria.len());
    for c in criteria {
        let dimension = dimension_of(&c.code).ok_or_else(|| {
            StorageError::RowDecode(format!("no learning dimension for criterion {}", c.code))
        })?;
        defs.push(RubricCriterionDef {
            code: c.code.clone(),
            title: c.title.clone(),
            weight: c.weight,
            machine_checkable: c.machine_checkable,
            critical_gate: c.critical_gate,
            dimension,
        });
    }
    Ok(defs)
}

/// Insert a rubric with its criteria and anchors (immutable afterwards).
///
/// A non-standard level scale is refused (`assessment.rubric.custom_level_scale`,
/// PROPOSED): the anchors are stored, but `g9-app` still regrades through
/// [`rubric_from_rows`] (standard scale), so storing a custom scale would let a
/// grade be silently recomputed on a different level → score mapping. Custom
/// notes and descriptors (which do not change scores) are stored as given.
pub fn insert_rubric(
    conn: &Connection,
    rubric: &RubricRow,
    criteria: &[RubricCriterionRow],
) -> RepoResult<()> {
    super::atomic(conn, || insert_rubric_statements(conn, rubric, criteria))
}

/// The statements of [`insert_rubric`], which runs them as one unit (`super::atomic`).
fn insert_rubric_statements(
    conn: &Connection,
    rubric: &RubricRow,
    criteria: &[RubricCriterionRow],
) -> RepoResult<()> {
    let (anchors_json, anchors_sha256) = match &rubric.anchors {
        Some(anchors) => {
            if anchors.scale != LevelScale::STANDARD {
                return Err(RepoError::Storage(StorageError::NotImplemented(
                    "assessment.rubric.custom_level_scale",
                )));
            }
            let json = to_json(anchors)?;
            let sha = g9_domain::hashing::sha256_hex(json.as_bytes());
            (Some(json), Some(sha))
        }
        None => (None, None),
    };
    conn.execute(
        "INSERT INTO rubrics (id, assessment_version_id, title, version_label, total_weight, anchors_json,
             anchors_sha256, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            rubric.id,
            rubric.assessment_version_id,
            rubric.title,
            rubric.version_label,
            i64::from(rubric.total_weight),
            anchors_json,
            anchors_sha256,
            ms(rubric.created_at)
        ],
    )?;
    for c in criteria {
        conn.execute(
            "INSERT INTO rubric_criteria (id, rubric_id, code, title, ordinal, weight, machine_checkable, critical_gate, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                c.id,
                rubric.id,
                c.code,
                c.title,
                i64::from(c.ordinal),
                i64::from(c.weight),
                bool_i64(c.machine_checkable),
                bool_i64(c.critical_gate),
                ms(c.created_at)
            ],
        )?;
    }
    Ok(())
}

/// Stored anchors: both columns or neither; the hash must match the JSON bytes.
fn stored_anchors(
    json: Option<String>,
    sha256: Option<String>,
) -> StorageResult<Option<RubricAnchors>> {
    match (json, sha256) {
        (None, None) => Ok(None),
        (Some(json), Some(sha)) => {
            if g9_domain::hashing::sha256_hex(json.as_bytes()) != sha {
                return Err(StorageError::Corrupt(
                    "rubrics.anchors_json does not match anchors_sha256".to_string(),
                ));
            }
            from_json(&json, "anchors_json").map(Some)
        }
        _ => Err(StorageError::Corrupt(
            "rubrics.anchors_json and anchors_sha256 are not paired".to_string(),
        )),
    }
}

/// The rubric (and criteria in ordinal order) of a version, if any. The stored
/// anchors are hash-checked (`DATA_CORRUPTION` on mismatch).
pub fn rubric_for_version(
    conn: &Connection,
    assessment_version_id: &str,
) -> RepoResult<Option<(RubricRow, Vec<RubricCriterionRow>)>> {
    let Some(rubric) = query_opt(
        conn,
        "SELECT id, assessment_version_id, title, version_label, total_weight, anchors_json, anchors_sha256,
                created_at
         FROM rubrics WHERE assessment_version_id = ?1 ORDER BY created_at DESC, id LIMIT 1",
        &[&assessment_version_id],
        |r| {
            Ok(RubricRow {
                id: r.get("id")?,
                assessment_version_id: r.get("assessment_version_id")?,
                title: r.get("title")?,
                version_label: r.get("version_label")?,
                total_weight: u32_from_i64(r.get("total_weight")?, "total_weight")?,
                anchors: stored_anchors(r.get("anchors_json")?, r.get("anchors_sha256")?)?,
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )?
    else {
        return Ok(None);
    };
    let criteria = query_all(
        conn,
        "SELECT id, rubric_id, code, title, ordinal, weight, machine_checkable, critical_gate, created_at
         FROM rubric_criteria WHERE rubric_id = ?1 ORDER BY ordinal",
        &[&rubric.id],
        |r| {
            Ok(RubricCriterionRow {
                id: r.get("id")?,
                rubric_id: r.get("rubric_id")?,
                code: r.get("code")?,
                title: r.get("title")?,
                ordinal: u32_from_i64(r.get("ordinal")?, "ordinal")?,
                weight: u8_from_i64(r.get("weight")?, "weight")?,
                machine_checkable: i64_bool(r.get("machine_checkable")?)?,
                critical_gate: i64_bool(r.get("critical_gate")?)?,
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )?;
    Ok(Some((rubric, criteria)))
}

// ---------------------------------------------------------------------------
// attempts
// ---------------------------------------------------------------------------

/// An `attempts` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptRow {
    /// Attempt id.
    pub id: AttemptId,
    /// Version row attempted.
    pub assessment_version_id: String,
    /// Session the attempt ran in.
    pub session_id: Option<SessionId>,
    /// Original attempt when this is a retake.
    pub retake_of: Option<AttemptId>,
    /// Machine state.
    pub state: AssessmentAttemptState,
    /// Interaction mode.
    pub mode: InteractionMode,
    /// Highest assistance received (never lowered).
    pub assistance_level: AssistanceLevel,
    /// Hints given (never decreased).
    pub hint_count: u32,
    /// Start instant.
    pub started_at: Option<UtcMillis>,
    /// Submission instant.
    pub submitted_at: Option<UtcMillis>,
    /// Finalization instant (required at `APPROVED_FINAL`).
    pub finalized_at: Option<UtcMillis>,
    /// Hex SHA-256 of the submission.
    pub submission_sha256: Option<String>,
    /// Privacy domain.
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

/// Mutable attempt columns written by [`update_attempt`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptPatch {
    /// New state (must be reachable from the stored state in one edge, or equal).
    pub state: AssessmentAttemptState,
    /// Assistance level (>= stored).
    pub assistance_level: AssistanceLevel,
    /// Hint count (>= stored).
    pub hint_count: u32,
    /// Start instant.
    pub started_at: Option<UtcMillis>,
    /// Submission instant.
    pub submitted_at: Option<UtcMillis>,
    /// Finalization instant.
    pub finalized_at: Option<UtcMillis>,
    /// Submission hash.
    pub submission_sha256: Option<String>,
}

/// Serde wire form of `g9_assessment::Attempt` (its fields are private).
#[derive(Serialize, Deserialize)]
struct AttemptWire {
    id: AttemptId,
    definition_id: AssessmentId,
    version: u32,
    learner: ActorId,
    mode: InteractionMode,
    state: AssessmentAttemptState,
    started_at: Option<UtcMillis>,
    submitted_at: Option<UtcMillis>,
    assistance: AssistanceLevel,
    retake_of: Option<AttemptId>,
    exposure: Vec<ExposureRecord>,
    revision: Revision,
    competency_ids: Vec<CompetencyId>,
    difficulty: Difficulty,
    created_at: UtcMillis,
    updated_at: UtcMillis,
}

impl AttemptRow {
    /// Build a row from a domain attempt plus the storage facts it does not carry.
    pub fn from_attempt(
        attempt: &Attempt,
        assessment_version_id: &str,
        session_id: Option<SessionId>,
        hint_count: u32,
        submission_sha256: Option<String>,
        data_domain: DataDomain,
        finalized_at: Option<UtcMillis>,
    ) -> Self {
        Self {
            id: attempt.id().clone(),
            assessment_version_id: assessment_version_id.to_string(),
            session_id,
            retake_of: attempt.retake_of().cloned(),
            state: attempt.state(),
            mode: attempt.mode(),
            assistance_level: attempt.assistance(),
            hint_count,
            started_at: attempt.started_at(),
            submitted_at: attempt.submitted_at(),
            finalized_at,
            submission_sha256,
            data_domain,
            classification: Classification::Private,
            revision: attempt.revision(),
            created_at: attempt.created_at(),
            updated_at: attempt.updated_at(),
        }
    }

    /// Patch view of the row's mutable columns.
    pub fn patch(&self) -> AttemptPatch {
        AttemptPatch {
            state: self.state,
            assistance_level: self.assistance_level,
            hint_count: self.hint_count,
            started_at: self.started_at,
            submitted_at: self.submitted_at,
            finalized_at: self.finalized_at,
            submission_sha256: self.submission_sha256.clone(),
        }
    }

    /// Rehydrate the domain attempt through serde with the definition-level facts
    /// the row does not store (version number, learner, competencies, difficulty,
    /// exposure records from [`list_exposures`]).
    #[allow(clippy::too_many_arguments)]
    pub fn to_attempt(
        &self,
        definition_id: &AssessmentId,
        version: u32,
        learner: &ActorId,
        competency_ids: Vec<CompetencyId>,
        difficulty: Difficulty,
        exposure: Vec<ExposureRecord>,
    ) -> StorageResult<Attempt> {
        let wire = AttemptWire {
            id: self.id.clone(),
            definition_id: definition_id.clone(),
            version,
            learner: learner.clone(),
            mode: self.mode,
            state: self.state,
            started_at: self.started_at,
            submitted_at: self.submitted_at,
            assistance: self.assistance_level,
            retake_of: self.retake_of.clone(),
            exposure,
            revision: self.revision,
            competency_ids,
            difficulty,
            created_at: self.created_at,
            updated_at: self.updated_at,
        };
        let value =
            serde_json::to_value(&wire).map_err(|_| StorageError::Internal("attempt wire"))?;
        serde_json::from_value(value)
            .map_err(|e| StorageError::RowDecode(format!("attempt rehydration: {e}")))
    }
}

const ATTEMPT_COLUMNS: &str = "id, assessment_version_id, session_id, retake_of, state, mode, assistance_level, hint_count,
    started_at, submitted_at, finalized_at, submission_sha256, data_domain, classification, revision, created_at, updated_at";

fn map_attempt(r: &Row<'_>) -> StorageResult<AttemptRow> {
    Ok(AttemptRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        assessment_version_id: r.get("assessment_version_id")?,
        session_id: opt_parse_id(r.get("session_id")?)?,
        retake_of: opt_parse_id(r.get("retake_of")?)?,
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        mode: enum_from_text(&r.get::<_, String>("mode")?)?,
        assistance_level: enum_from_text(&r.get::<_, String>("assistance_level")?)?,
        hint_count: u32_from_i64(r.get("hint_count")?, "hint_count")?,
        started_at: opt_from_ms(r.get("started_at")?),
        submitted_at: opt_from_ms(r.get("submitted_at")?),
        finalized_at: opt_from_ms(r.get("finalized_at")?),
        submission_sha256: r.get("submission_sha256")?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert an attempt (revision 1).
pub fn insert_attempt(conn: &Connection, row: &AttemptRow) -> RepoResult<Revision> {
    opt_sha256("submission_sha256", row.submission_sha256.as_deref())?;
    conn.execute(
        &format!(
            "INSERT INTO attempts ({ATTEMPT_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, 1, ?15, ?15)"
        ),
        params![
            row.id.as_str(),
            row.assessment_version_id,
            row.session_id.as_ref().map(|s| s.as_str().to_string()),
            row.retake_of.as_ref().map(|a| a.as_str().to_string()),
            enum_to_text(&row.state)?,
            enum_to_text(&row.mode)?,
            enum_to_text(&row.assistance_level)?,
            i64::from(row.hint_count),
            opt_ms(row.started_at),
            opt_ms(row.submitted_at),
            opt_ms(row.finalized_at),
            row.submission_sha256,
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One attempt, scope-checked.
pub fn get_attempt(
    conn: &Connection,
    scope: &ReadScope,
    id: &AttemptId,
) -> RepoResult<Option<AttemptRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {ATTEMPT_COLUMNS} FROM attempts WHERE id = ?1"),
        &[&id.as_str()],
        map_attempt,
    )?;
    if let Some(r) = &row {
        scope.check("attempts", r.data_domain)?;
    }
    Ok(row)
}

/// Attempts of a version, domain-filtered in SQL, creation order.
pub fn list_attempts_for_version(
    conn: &Connection,
    scope: &ReadScope,
    assessment_version_id: &str,
) -> RepoResult<Vec<AttemptRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {ATTEMPT_COLUMNS} FROM attempts WHERE assessment_version_id = ?1 AND (?2 = 1 OR data_domain = ?3)
             ORDER BY created_at, id"
        ),
        &[&assessment_version_id, &scope.sql_flag(), &scope.sql_domain()?],
        map_attempt,
    )
}

/// Attempts in a state, domain-filtered in SQL.
pub fn list_attempts_by_state(
    conn: &Connection,
    scope: &ReadScope,
    state: AssessmentAttemptState,
) -> RepoResult<Vec<AttemptRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {ATTEMPT_COLUMNS} FROM attempts WHERE state = ?1 AND (?2 = 1 OR data_domain = ?3)
             ORDER BY created_at, id"
        ),
        &[&enum_to_text(&state)?, &scope.sql_flag(), &scope.sql_domain()?],
        map_attempt,
    )
}

fn state_reachable(from: AssessmentAttemptState, to: AssessmentAttemptState) -> bool {
    from == to
        || AssessmentAttemptState::TRANSITIONS
            .iter()
            .any(|(f, _, t)| *f == from && *t == to)
}

/// Persist the next snapshot of an attempt under a revision check. The new
/// state must equal the stored one or be one edge away in the
/// `AssessmentAttemptState` table; assistance and hint count never go down;
/// `APPROVED_FINAL` needs `finalized_at`. Finalized attempts refuse every update
/// (trigger + `IMMUTABLE` error).
pub fn update_attempt(
    conn: &Connection,
    id: &AttemptId,
    expected_revision: Revision,
    patch: &AttemptPatch,
    now: UtcMillis,
) -> RepoResult<Revision> {
    opt_sha256("submission_sha256", patch.submission_sha256.as_deref())?;
    let scope = ReadScope::cross_domain(DataDomain::Learning);
    let current = required("attempts", get_attempt(conn, &scope, id)?)?;
    if current.revision != expected_revision {
        return Err(RepoError::Storage(StorageError::StaleRevision {
            expected: Some(expected_revision),
            actual: current.revision,
        }));
    }
    if current.state == AssessmentAttemptState::ApprovedFinal {
        return Err(RepoError::Storage(StorageError::Immutable(
            "finalized attempt cannot be updated; create a retake attempt".to_string(),
        )));
    }
    if !state_reachable(current.state, patch.state) {
        return Err(RepoError::Storage(StorageError::InvalidTransition(
            g9_domain::state::TransitionError {
                machine: AssessmentAttemptState::MACHINE,
                from: current.state.to_string(),
                event: format!("-> {}", patch.state),
            },
        )));
    }
    if patch.assistance_level < current.assistance_level || patch.hint_count < current.hint_count {
        return Err(RepoError::Storage(StorageError::Precondition(
            "assessment.assistance.never_erased: assistance level and hint count cannot decrease"
                .to_string(),
        )));
    }
    if patch.state == AssessmentAttemptState::ApprovedFinal && patch.finalized_at.is_none() {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "APPROVED_FINAL requires finalized_at".to_string(),
        )));
    }
    let changed = conn.execute(
        "UPDATE attempts
         SET state = ?3, assistance_level = ?4, hint_count = ?5, started_at = ?6, submitted_at = ?7, finalized_at = ?8,
             submission_sha256 = ?9, revision = revision + 1, updated_at = ?10
         WHERE id = ?1 AND revision = ?2 AND state <> 'APPROVED_FINAL'",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            enum_to_text(&patch.state)?,
            enum_to_text(&patch.assistance_level)?,
            i64::from(patch.hint_count),
            opt_ms(patch.started_at),
            opt_ms(patch.submitted_at),
            opt_ms(patch.finalized_at),
            patch.submission_sha256,
            ms(now),
        ],
    )?;
    finish_update(
        conn,
        "attempts",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

/// Role of an evidence item on an attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttemptEvidenceRole {
    /// The submission itself.
    Submission,
    /// Supporting material.
    Supporting,
    /// Reviewer-produced evidence.
    Reviewer,
}

/// An `attempt_evidence` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptEvidenceRow {
    /// Attempt.
    pub attempt_id: AttemptId,
    /// Evidence item.
    pub evidence_item_id: EvidenceId,
    /// Role.
    pub role: AttemptEvidenceRole,
    /// When linked.
    pub added_at: UtcMillis,
}

/// Bind an evidence item to an attempt (`CONFLICT` if already bound).
pub fn link_attempt_evidence(conn: &Connection, row: &AttemptEvidenceRow) -> RepoResult<()> {
    conn.execute(
        "INSERT INTO attempt_evidence (attempt_id, evidence_item_id, role, added_at) VALUES (?1, ?2, ?3, ?4)",
        params![
            row.attempt_id.as_str(),
            row.evidence_item_id.as_str(),
            enum_to_text(&row.role)?,
            ms(row.added_at)
        ],
    )?;
    Ok(())
}

/// Evidence bound to an attempt.
pub fn list_attempt_evidence(
    conn: &Connection,
    attempt_id: &AttemptId,
) -> RepoResult<Vec<AttemptEvidenceRow>> {
    query_all(
        conn,
        "SELECT attempt_id, evidence_item_id, role, added_at FROM attempt_evidence WHERE attempt_id = ?1
         ORDER BY added_at, evidence_item_id",
        &[&attempt_id.as_str()],
        |r| {
            Ok(AttemptEvidenceRow {
                attempt_id: parse_id(&r.get::<_, String>("attempt_id")?)?,
                evidence_item_id: parse_id(&r.get::<_, String>("evidence_item_id")?)?,
                role: enum_from_text(&r.get::<_, String>("role")?)?,
                added_at: from_ms(r.get("added_at")?),
            })
        },
    )
}

// ---------------------------------------------------------------------------
// grading
// ---------------------------------------------------------------------------

/// A `criterion_results` row (immutable; corrections supersede).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriterionResultRow {
    /// Row id.
    pub id: String,
    /// Attempt graded.
    pub attempt_id: AttemptId,
    /// `rubric_criteria.id` scored.
    pub criterion_id: String,
    /// Score 0..=100.
    pub score: u8,
    /// Who produced the score.
    pub source: GradeSource,
    /// Evidence the score rests on.
    pub evidence_refs: Vec<EvidenceId>,
    /// Reviewer/checker note (never an instruction).
    pub rationale: Option<String>,
    /// Recording actor.
    pub scored_by: ActorId,
    /// When.
    pub scored_at: UtcMillis,
    /// Earlier result this one corrects.
    pub supersedes: Option<String>,
    /// Row creation.
    pub created_at: UtcMillis,
}

impl CriterionResultRow {
    /// Build a row from a domain result and the criterion row it scores.
    pub fn from_domain(
        result: &CriterionResult,
        id: &str,
        criterion_id: &str,
        supersedes: Option<String>,
    ) -> Self {
        Self {
            id: id.to_string(),
            attempt_id: result.attempt_id.clone(),
            criterion_id: criterion_id.to_string(),
            score: result.score.get(),
            source: result.source,
            evidence_refs: result.evidence_refs.clone(),
            rationale: if result.note.is_empty() {
                None
            } else {
                Some(result.note.clone())
            },
            scored_by: result.recorded_by.clone(),
            scored_at: result.recorded_at,
            supersedes,
            created_at: result.recorded_at,
        }
    }
}

/// A stored criterion result joined with its criterion code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredCriterionResult {
    /// The row.
    pub row: CriterionResultRow,
    /// `rubric_criteria.code`.
    pub criterion_code: String,
}

impl StoredCriterionResult {
    /// The assessment crate's result.
    pub fn to_domain(&self) -> StorageResult<CriterionResult> {
        Ok(CriterionResult {
            attempt_id: self.row.attempt_id.clone(),
            criterion_code: self.criterion_code.clone(),
            score: Score::new(self.row.score)
                .map_err(|e| StorageError::RowDecode(format!("score: {e}")))?,
            source: self.row.source,
            evidence_refs: self.row.evidence_refs.clone(),
            note: self.row.rationale.clone().unwrap_or_default(),
            recorded_at: self.row.scored_at,
            recorded_by: self.row.scored_by.clone(),
        })
    }
}

/// `rubric_criteria.id` for a code within a rubric.
pub fn criterion_id(conn: &Connection, rubric_id: &str, code: &str) -> RepoResult<Option<String>> {
    query_opt(
        conn,
        "SELECT id FROM rubric_criteria WHERE rubric_id = ?1 AND code = ?2",
        &[&rubric_id, &code],
        |r| Ok(r.get("id")?),
    )
}

/// Append a criterion result (never updated or deleted; trigger-enforced).
pub fn append_criterion_result(conn: &Connection, row: &CriterionResultRow) -> RepoResult<()> {
    if row.score > 100 {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "score must be 0..=100".to_string(),
        )));
    }
    conn.execute(
        "INSERT INTO criterion_results (id, attempt_id, criterion_id, score, source, evidence_refs, rationale, scored_by,
            scored_at, supersedes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            row.id,
            row.attempt_id.as_str(),
            row.criterion_id,
            i64::from(row.score),
            enum_to_text(&row.source)?,
            to_json(&row.evidence_refs)?,
            row.rationale,
            row.scored_by.as_str(),
            ms(row.scored_at),
            row.supersedes,
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

/// Criterion results of an attempt with their codes, in scoring order.
pub fn list_criterion_results(
    conn: &Connection,
    attempt_id: &AttemptId,
) -> RepoResult<Vec<StoredCriterionResult>> {
    query_all(
        conn,
        "SELECT cr.id, cr.attempt_id, cr.criterion_id, cr.score, cr.source, cr.evidence_refs, cr.rationale, cr.scored_by,
                cr.scored_at, cr.supersedes, cr.created_at, rc.code
         FROM criterion_results cr JOIN rubric_criteria rc ON rc.id = cr.criterion_id
         WHERE cr.attempt_id = ?1 ORDER BY cr.scored_at, cr.id",
        &[&attempt_id.as_str()],
        |r| {
            Ok(StoredCriterionResult {
                row: CriterionResultRow {
                    id: r.get("id")?,
                    attempt_id: parse_id(&r.get::<_, String>("attempt_id")?)?,
                    criterion_id: r.get("criterion_id")?,
                    score: u8_from_i64(r.get("score")?, "score")?,
                    source: enum_from_text(&r.get::<_, String>("source")?)?,
                    evidence_refs: from_json(&r.get::<_, String>("evidence_refs")?, "evidence_refs")?,
                    rationale: r.get("rationale")?,
                    scored_by: parse_id(&r.get::<_, String>("scored_by")?)?,
                    scored_at: from_ms(r.get("scored_at")?),
                    supersedes: r.get("supersedes")?,
                    created_at: from_ms(r.get("created_at")?),
                },
                criterion_code: r.get("code")?,
            })
        },
    )
}

/// Results that are not superseded by a later row (the effective set).
pub fn effective_criterion_results(
    conn: &Connection,
    attempt_id: &AttemptId,
) -> RepoResult<Vec<StoredCriterionResult>> {
    query_all(
        conn,
        "SELECT cr.id, cr.attempt_id, cr.criterion_id, cr.score, cr.source, cr.evidence_refs, cr.rationale, cr.scored_by,
                cr.scored_at, cr.supersedes, cr.created_at, rc.code
         FROM criterion_results cr JOIN rubric_criteria rc ON rc.id = cr.criterion_id
         WHERE cr.attempt_id = ?1
           AND NOT EXISTS (SELECT 1 FROM criterion_results later WHERE later.supersedes = cr.id)
         ORDER BY cr.scored_at, cr.id",
        &[&attempt_id.as_str()],
        |r| {
            Ok(StoredCriterionResult {
                row: CriterionResultRow {
                    id: r.get("id")?,
                    attempt_id: parse_id(&r.get::<_, String>("attempt_id")?)?,
                    criterion_id: r.get("criterion_id")?,
                    score: u8_from_i64(r.get("score")?, "score")?,
                    source: enum_from_text(&r.get::<_, String>("source")?)?,
                    evidence_refs: from_json(&r.get::<_, String>("evidence_refs")?, "evidence_refs")?,
                    rationale: r.get("rationale")?,
                    scored_by: parse_id(&r.get::<_, String>("scored_by")?)?,
                    scored_at: from_ms(r.get("scored_at")?),
                    supersedes: r.get("supersedes")?,
                    created_at: from_ms(r.get("created_at")?),
                },
                criterion_code: r.get("code")?,
            })
        },
    )
}

/// Reliability class of a grade decision (`grade_decisions.reliability_class`,
/// migration `0010`; ADR-0012, LRN-11). `SelfAdjudicated` decisions never
/// satisfy validation evidence (ADR-0005). `g9-assessment` does not model this
/// yet (wave-1 gap), so the class is chosen by `g9-app` and stored here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReliabilityClass {
    /// Every criterion was scored by a deterministic checker.
    Deterministic,
    /// A reviewer other than the learner decided.
    ExternalReview,
    /// The learner (operator) graded their own attempt.
    SelfAdjudicated,
}

/// A `grade_decisions` row (append-only; final rows immutable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GradeDecisionRow {
    /// Decision id.
    pub id: GradeDecisionId,
    /// Attempt decided.
    pub attempt_id: AttemptId,
    /// Weighted total 0..=100, rounded down from the exact weighted total
    /// ([`GradeDecisionRow::from_domain`]) so it never crosses the pass cut.
    pub total_score: u8,
    /// Pass/fail.
    pub passed: bool,
    /// Final (immutable) or provisional.
    pub is_final: bool,
    /// Deciding reviewer.
    pub decided_by: ActorId,
    /// When.
    pub decided_at: UtcMillis,
    /// Approval consumed (`Capability::GradeFinalize`).
    pub approval_id: Option<ApprovalId>,
    /// Rationale.
    pub rationale: Option<String>,
    /// Hex SHA-256 over the criterion results the decision rests on.
    pub criterion_results_sha256: Option<String>,
    /// Prior decision this one supersedes.
    pub supersedes: Option<GradeDecisionId>,
    /// Reliability class (ADR-0012).
    pub reliability_class: ReliabilityClass,
    /// Revision (bookkeeping only).
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

impl GradeDecisionRow {
    /// Build a row from a domain decision. `weighted_total` must be present
    /// (`finalize` guarantees it); the hash over the results is supplied by the caller.
    /// The reliability class defaults to the least-trusted `SelfAdjudicated`;
    /// [`GradeDecisionRow::with_reliability_class`] states a stronger class.
    pub fn from_domain(
        decision: &GradeDecision,
        approval_id: Option<ApprovalId>,
        criterion_results_sha256: Option<String>,
    ) -> StorageResult<Self> {
        let total = decision.breakdown.weighted_total.ok_or_else(|| {
            StorageError::InvalidInput("grade decision without a weighted total".to_string())
        })?;
        if !(0.0..=100.0).contains(&total) {
            return Err(StorageError::InvalidInput(
                "weighted total outside 0..=100".to_string(),
            ));
        }
        // Round DOWN, never to nearest: the stored integer must never exceed the
        // exact total the pass rule evaluated, so `total_score >= pass_threshold`
        // holds exactly when the exact weighted total met the cut (79.5 is a FAIL
        // and is stored as 79, not 80). The exact value is recomputable from the
        // hashed criterion results. Bounded above, so the cast cannot truncate.
        let total_score = total.floor().clamp(0.0, 100.0) as u8;
        Ok(Self {
            id: decision.id.clone(),
            attempt_id: decision.attempt_id.clone(),
            total_score,
            passed: decision.pass,
            is_final: decision.is_final,
            decided_by: decision.decided_by.clone(),
            decided_at: decision.decided_at,
            approval_id,
            rationale: decision.rationale.clone(),
            criterion_results_sha256,
            supersedes: decision.supersedes.clone(),
            reliability_class: ReliabilityClass::SelfAdjudicated,
            revision: Revision::INITIAL,
            created_at: decision.decided_at,
            updated_at: decision.decided_at,
        })
    }

    /// Same row with an explicit reliability class.
    #[must_use]
    pub fn with_reliability_class(mut self, class: ReliabilityClass) -> Self {
        self.reliability_class = class;
        self
    }
}

const GRADE_COLUMNS: &str =
    "id, attempt_id, total_score, passed, is_final, decided_by, decided_at, approval_id, rationale,
    criterion_results_sha256, supersedes, reliability_class, revision, created_at, updated_at";

fn map_grade(r: &Row<'_>) -> StorageResult<GradeDecisionRow> {
    Ok(GradeDecisionRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        attempt_id: parse_id(&r.get::<_, String>("attempt_id")?)?,
        total_score: u8_from_i64(r.get("total_score")?, "total_score")?,
        passed: i64_bool(r.get("passed")?)?,
        is_final: i64_bool(r.get("is_final")?)?,
        decided_by: parse_id(&r.get::<_, String>("decided_by")?)?,
        decided_at: from_ms(r.get("decided_at")?),
        approval_id: opt_parse_id(r.get("approval_id")?)?,
        rationale: r.get("rationale")?,
        criterion_results_sha256: r.get("criterion_results_sha256")?,
        supersedes: opt_parse_id(r.get("supersedes")?)?,
        reliability_class: enum_from_text(&r.get::<_, String>("reliability_class")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Append a grade decision. A correction of a final decision is a new row with
/// `supersedes`; decisions are never updated or deleted by this repository.
pub fn append_grade_decision(conn: &Connection, row: &GradeDecisionRow) -> RepoResult<()> {
    opt_sha256(
        "criterion_results_sha256",
        row.criterion_results_sha256.as_deref(),
    )?;
    if row.total_score > 100 {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "total_score must be 0..=100".to_string(),
        )));
    }
    conn.execute(
        &format!(
            "INSERT INTO grade_decisions ({GRADE_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 1, ?13, ?13)"
        ),
        params![
            row.id.as_str(),
            row.attempt_id.as_str(),
            i64::from(row.total_score),
            bool_i64(row.passed),
            bool_i64(row.is_final),
            row.decided_by.as_str(),
            ms(row.decided_at),
            row.approval_id.as_ref().map(|a| a.as_str().to_string()),
            row.rationale,
            row.criterion_results_sha256,
            row.supersedes.as_ref().map(|s| s.as_str().to_string()),
            enum_to_text(&row.reliability_class)?,
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

/// Every decision on an attempt, oldest first.
pub fn list_grade_decisions(
    conn: &Connection,
    attempt_id: &AttemptId,
) -> RepoResult<Vec<GradeDecisionRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {GRADE_COLUMNS} FROM grade_decisions WHERE attempt_id = ?1 ORDER BY decided_at, id"
        ),
        &[&attempt_id.as_str()],
        map_grade,
    )
}

/// The effective final decision of an attempt: final and not superseded.
pub fn effective_final_decision(
    conn: &Connection,
    attempt_id: &AttemptId,
) -> RepoResult<Option<GradeDecisionRow>> {
    query_opt(
        conn,
        &format!(
            "SELECT {GRADE_COLUMNS} FROM grade_decisions gd
             WHERE gd.attempt_id = ?1 AND gd.is_final = 1
               AND NOT EXISTS (SELECT 1 FROM grade_decisions later WHERE later.supersedes = gd.id)
             ORDER BY gd.decided_at DESC, gd.id DESC LIMIT 1"
        ),
        &[&attempt_id.as_str()],
        map_grade,
    )
}

/// A `retake_links` row (immutable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetakeLinkRow {
    /// Original attempt.
    pub original_attempt_id: AttemptId,
    /// Retake attempt.
    pub retake_attempt_id: AttemptId,
    /// Why a retake was allowed.
    pub reason: String,
    /// Creating operator.
    pub created_by: ActorId,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Link a retake to its original.
pub fn insert_retake_link(conn: &Connection, row: &RetakeLinkRow) -> RepoResult<()> {
    super::require_non_empty("reason", &row.reason)?;
    conn.execute(
        "INSERT INTO retake_links (original_attempt_id, retake_attempt_id, reason, created_by, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            row.original_attempt_id.as_str(),
            row.retake_attempt_id.as_str(),
            row.reason,
            row.created_by.as_str(),
            ms(row.created_at)
        ],
    )?;
    Ok(())
}

/// Retakes of an original attempt.
pub fn list_retakes(conn: &Connection, original: &AttemptId) -> RepoResult<Vec<RetakeLinkRow>> {
    query_all(
        conn,
        "SELECT original_attempt_id, retake_attempt_id, reason, created_by, created_at
         FROM retake_links WHERE original_attempt_id = ?1 ORDER BY created_at, retake_attempt_id",
        &[&original.as_str()],
        |r| {
            Ok(RetakeLinkRow {
                original_attempt_id: parse_id(&r.get::<_, String>("original_attempt_id")?)?,
                retake_attempt_id: parse_id(&r.get::<_, String>("retake_attempt_id")?)?,
                reason: r.get("reason")?,
                created_by: parse_id(&r.get::<_, String>("created_by")?)?,
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

// ---------------------------------------------------------------------------
// exposure log
// ---------------------------------------------------------------------------

/// What content was exposed (mirrors `exposure_log.content_kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExposureContentKind {
    /// Task statement.
    Task,
    /// Answer key or reference solution.
    Key,
    /// Rubric.
    Rubric,
}

impl ExposureContentKind {
    /// Map the assessment crate's kind; hints are recorded on the attempt
    /// (`assistance_level`/`hint_count`) and have no exposure content kind (gap).
    pub fn from_domain(kind: ExposureKind) -> StorageResult<Self> {
        Ok(match kind {
            ExposureKind::Task => Self::Task,
            ExposureKind::AnswerKey | ExposureKind::ReferenceSolution => Self::Key,
            ExposureKind::Hint => {
                return Err(StorageError::NotImplemented("assessment.exposure.hint"))
            }
        })
    }
}

/// Who content was exposed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExposedTo {
    /// The operator/learner (displayed).
    Operator,
    /// The model (context assembly).
    Model,
    /// A skill.
    Skill,
    /// An export.
    Export,
}

/// An `exposure_log` row (immutable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExposureLogRow {
    /// Row id.
    pub id: String,
    /// Version whose content was exposed.
    pub assessment_version_id: String,
    /// Attempt in progress, if any.
    pub attempt_id: Option<AttemptId>,
    /// Audience.
    pub exposed_to: ExposedTo,
    /// Actor that caused the exposure.
    pub actor_id: ActorId,
    /// Interaction mode.
    pub mode: InteractionMode,
    /// Content kind.
    pub content_kind: ExposureContentKind,
    /// Hex SHA-256 of the exposed content.
    pub content_sha256: String,
    /// Reason.
    pub reason: Option<String>,
    /// When.
    pub exposed_at: UtcMillis,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Append an exposure (never updated or deleted; trigger-enforced).
pub fn append_exposure(conn: &Connection, row: &ExposureLogRow) -> RepoResult<()> {
    require_sha256("content_sha256", &row.content_sha256)?;
    conn.execute(
        "INSERT INTO exposure_log (id, assessment_version_id, attempt_id, exposed_to, actor_id, mode, content_kind,
            content_sha256, reason, exposed_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            row.id,
            row.assessment_version_id,
            row.attempt_id.as_ref().map(|a| a.as_str().to_string()),
            enum_to_text(&row.exposed_to)?,
            row.actor_id.as_str(),
            enum_to_text(&row.mode)?,
            enum_to_text(&row.content_kind)?,
            row.content_sha256,
            row.reason,
            ms(row.exposed_at),
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

fn map_exposure(r: &Row<'_>) -> StorageResult<ExposureLogRow> {
    Ok(ExposureLogRow {
        id: r.get("id")?,
        assessment_version_id: r.get("assessment_version_id")?,
        attempt_id: opt_parse_id(r.get("attempt_id")?)?,
        exposed_to: enum_from_text(&r.get::<_, String>("exposed_to")?)?,
        actor_id: parse_id(&r.get::<_, String>("actor_id")?)?,
        mode: enum_from_text(&r.get::<_, String>("mode")?)?,
        content_kind: enum_from_text(&r.get::<_, String>("content_kind")?)?,
        content_sha256: r.get("content_sha256")?,
        reason: r.get("reason")?,
        exposed_at: from_ms(r.get("exposed_at")?),
        created_at: from_ms(r.get("created_at")?),
    })
}

const EXPOSURE_COLUMNS: &str = "id, assessment_version_id, attempt_id, exposed_to, actor_id, mode, content_kind, content_sha256,
    reason, exposed_at, created_at";

/// Exposures of a version in time order.
pub fn list_exposures(
    conn: &Connection,
    assessment_version_id: &str,
) -> RepoResult<Vec<ExposureLogRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {EXPOSURE_COLUMNS} FROM exposure_log WHERE assessment_version_id = ?1 ORDER BY exposed_at, id"
        ),
        &[&assessment_version_id],
        map_exposure,
    )
}

/// Exposures recorded for an attempt.
pub fn list_exposures_for_attempt(
    conn: &Connection,
    attempt_id: &AttemptId,
) -> RepoResult<Vec<ExposureLogRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {EXPOSURE_COLUMNS} FROM exposure_log WHERE attempt_id = ?1 ORDER BY exposed_at, id"
        ),
        &[&attempt_id.as_str()],
        map_exposure,
    )
}

/// Blind-eligibility probe (LRN-13): whether `content_kind` of the version was
/// exposed to `exposed_to` strictly before `before`.
pub fn exposed_before(
    conn: &Connection,
    assessment_version_id: &str,
    content_kind: ExposureContentKind,
    exposed_to: ExposedTo,
    before: UtcMillis,
) -> RepoResult<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM exposure_log
         WHERE assessment_version_id = ?1 AND content_kind = ?2 AND exposed_to = ?3 AND exposed_at < ?4",
        params![
            assessment_version_id,
            enum_to_text(&content_kind)?,
            enum_to_text(&exposed_to)?,
            ms(before)
        ],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::testutil::{
        approval, assert_immutable, count, db, learning, operator, operator_id, sha, t,
    };
    use g9_assessment::{score_breakdown, Rubric};
    use g9_domain::Actor;

    fn competency(conn: &Connection) {
        conn.execute(
            "INSERT INTO competencies (id, code, title, domain_area, created_at, updated_at)
             VALUES ('comp-1', 'LINUX-BOOT', 'Linux boot', 'linux', 1, 1)",
            [],
        )
        .unwrap();
    }

    fn definition() -> AssessmentDefinition {
        AssessmentDefinition::new(
            AssessmentId::parse("ad-1").unwrap(),
            "Week 1 blind practical",
            vec![CompetencyId::parse("comp-1").unwrap()],
            AssessmentKind::BlindPractical,
            DataDomain::Learning,
        )
        .unwrap()
    }

    fn version(def: &AssessmentDefinition) -> AssessmentVersion {
        AssessmentVersion {
            definition_id: def.id.clone(),
            version: 1,
            rubric: Rubric::baseline(),
            task_ref: EvidenceId::parse("evi-task").unwrap(),
            sealed_key_ref: Some(g9_domain::AttachmentId::parse("att-key").unwrap()),
            difficulty: Difficulty::new(3).unwrap(),
            equivalence_group: "W1-boot".into(),
            created_at: t(0),
            checksum: String::new(),
        }
        .with_checksum()
        .unwrap()
    }

    /// Definition + version + baseline rubric; returns the version row id.
    fn seed(conn: &Connection) -> (AssessmentDefinition, AssessmentVersion, String) {
        competency(conn);
        let def = definition();
        insert_definition(
            conn,
            &AssessmentDefinitionRow::from_domain(&def, "W1-BLIND", t(0)).unwrap(),
        )
        .unwrap();
        let ver = version(&def);
        let row = AssessmentVersionRow::from_domain(
            &ver,
            "av-1",
            "Recover the boot chain",
            Some(sha("key")),
            operator_id(),
        )
        .unwrap();
        insert_version(conn, &row).unwrap();
        let (rubric, criteria) =
            rubric_rows_from_domain(&ver.rubric, "rub-1", "av-1", "Baseline", "v1", t(0));
        insert_rubric(conn, &rubric, &criteria).unwrap();
        (def, ver, "av-1".to_string())
    }

    fn attempt_row(id: &str, version_id: &str, state: AssessmentAttemptState) -> AttemptRow {
        AttemptRow {
            id: AttemptId::parse(id).unwrap(),
            assessment_version_id: version_id.to_string(),
            session_id: None,
            retake_of: None,
            state,
            mode: InteractionMode::Exam,
            assistance_level: AssistanceLevel::Unaided,
            hint_count: 0,
            started_at: None,
            submitted_at: None,
            finalized_at: None,
            submission_sha256: None,
            data_domain: DataDomain::Learning,
            classification: Classification::Private,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        }
    }

    #[test]
    fn definitions_versions_and_rubrics_roundtrip() {
        let db = db();
        let conn = db.conn();
        let (def, ver, version_id) = seed(conn);
        let stored = get_definition(conn, &learning(), &def.id).unwrap().unwrap();
        assert_eq!(stored.kind, AssessmentDefinitionKind::BlindPractical);
        assert_eq!(stored.mode, InteractionMode::Exam);
        assert_eq!(stored.to_domain().unwrap(), def);
        assert_eq!(list_definitions(conn, &learning()).unwrap().len(), 1);
        assert!(
            list_definitions(conn, &ReadScope::domain(DataDomain::Business))
                .unwrap()
                .is_empty()
        );

        let mut changed = stored.clone();
        changed.title = "Week 1 blind practical (v2)".into();
        assert!(matches!(
            update_definition(conn, &changed, Revision::new(2), t(1)),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        assert_eq!(
            update_definition(conn, &changed, Revision::INITIAL, t(1)).unwrap(),
            Revision::new(2)
        );

        let v = get_version(conn, &version_id).unwrap().unwrap();
        assert_eq!(v.version_label, "1");
        assert_eq!(v.version_number().unwrap(), 1);
        assert_eq!(v.task_sha256, sha("Recover the boot chain"));
        assert_eq!(v.sealed_key_ref.as_deref(), Some("att-key"));
        assert_eq!(v.pass_threshold, 80);
        // the declared difficulty of the domain version is persisted (0013)
        assert_eq!(v.difficulty, Some(ver.difficulty));
        assert_eq!(version_by_label(conn, &def.id, "1").unwrap().unwrap(), v);
        assert_eq!(list_versions(conn, &def.id).unwrap().len(), 1);

        let (rubric, criteria) = rubric_for_version(conn, &version_id).unwrap().unwrap();
        assert_eq!(rubric.total_weight, 100);
        assert_eq!(criteria.len(), 6);
        let rebuilt = rubric_from_rows(&criteria, Rubric::baseline_dimension).unwrap();
        assert_eq!(rebuilt, ver.rubric);
        assert!(rubric_from_rows(&criteria, |_| None).is_err());
        // versioned grading definitions are frozen (0010 triggers); a change is a new version
        assert_immutable(
            conn,
            &format!(
                "UPDATE assessment_versions SET pass_threshold = 50 WHERE id = '{version_id}'"
            ),
        );
        assert_immutable(
            conn,
            &format!("UPDATE rubrics SET title = 'x' WHERE id = '{}'", rubric.id),
        );
        assert_immutable(
            conn,
            &format!(
                "UPDATE rubric_criteria SET weight = 1 WHERE id = '{}'",
                criteria[0].id
            ),
        );

        // gaps are refused, not guessed
        let diag = AssessmentDefinition::new(
            AssessmentId::parse("ad-diag").unwrap(),
            "Diagnostic",
            vec![CompetencyId::parse("comp-1").unwrap()],
            AssessmentKind::DiagnosticCalibration,
            DataDomain::Learning,
        )
        .unwrap();
        assert!(matches!(
            AssessmentDefinitionRow::from_domain(&diag, "DIAG", t(0)),
            Err(StorageError::NotImplemented(
                "assessment.kind.diagnostic_calibration"
            ))
        ));
        let mut tampered = ver.clone();
        tampered.equivalence_group = "other".into();
        assert!(
            AssessmentVersionRow::from_domain(&tampered, "av-x", "task", None, operator_id())
                .is_err()
        );
    }

    #[test]
    fn version_difficulty_is_declared_never_defaulted_and_frozen() {
        let db = db();
        let conn = db.conn();
        let (def, ver, _) = seed(conn);
        // a version whose author declared no difficulty keeps NULL: no default, no guess
        let mut v2 = ver.clone();
        v2.version = 2;
        let v2 = v2.with_checksum().unwrap();
        let mut undeclared =
            AssessmentVersionRow::from_domain(&v2, "av-2", "variant 2", None, operator_id())
                .unwrap();
        assert_eq!(undeclared.difficulty, Some(v2.difficulty));
        undeclared.difficulty = None;
        insert_version(conn, &undeclared).unwrap();
        let stored = get_version(conn, "av-2").unwrap().unwrap();
        assert_eq!(stored.difficulty, None);
        assert_eq!(stored, undeclared);
        let raw: Option<i64> = conn
            .query_row(
                "SELECT difficulty FROM assessment_versions WHERE id = 'av-2'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(raw, None);
        // raw SQL that omits the column also stores NULL (the column has no DEFAULT)
        conn.execute(
            "INSERT INTO assessment_versions (id, definition_id, version_label, task_statement, task_sha256, created_by, created_at)
             VALUES ('av-3', ?1, '3', 't', ?2, 'op', 1)",
            params![def.id.as_str(), sha("t")],
        )
        .unwrap();
        assert_eq!(get_version(conn, "av-3").unwrap().unwrap().difficulty, None);
        // every difficulty 1..=5 round-trips; 0 and 6 are refused by the CHECK
        for (n, d) in (1_i64..=5).enumerate() {
            conn.execute(
                "INSERT INTO assessment_versions (id, definition_id, version_label, task_statement, task_sha256, difficulty, created_by, created_at)
                 VALUES (?1, ?2, ?3, 't', ?4, ?5, 'op', 1)",
                params![format!("av-d{d}"), def.id.as_str(), format!("1{n}"), sha("t"), d],
            )
            .unwrap();
            assert_eq!(
                get_version(conn, &format!("av-d{d}"))
                    .unwrap()
                    .unwrap()
                    .difficulty
                    .map(Difficulty::get),
                Some(u8::try_from(d).unwrap())
            );
        }
        for bad in [0_i64, 6] {
            assert!(conn
                .execute(
                    "INSERT INTO assessment_versions (id, definition_id, version_label, task_statement, task_sha256, difficulty, created_by, created_at)
                     VALUES (?1, ?2, ?3, 't', ?4, ?5, 'op', 1)",
                    params![format!("av-bad{bad}"), def.id.as_str(), format!("9{bad}"), sha("t"), bad],
                )
                .is_err());
        }
        // a declared difficulty is frozen with the rest of the version (0010 trigger)
        assert_immutable(
            conn,
            "UPDATE assessment_versions SET difficulty = 5 WHERE id = 'av-1'",
        );
        assert_immutable(
            conn,
            "UPDATE assessment_versions SET difficulty = 4 WHERE id = 'av-2'",
        );
    }

    #[test]
    fn attempt_lifecycle_revision_assistance_and_a04_immutability() {
        let db = db();
        let conn = db.conn();
        let (def, ver, version_id) = seed(conn);
        let attempt = Attempt::new(
            AttemptId::parse("attempt-1").unwrap(),
            &def,
            &ver,
            operator_id(),
            InteractionMode::Exam,
            t(0),
        )
        .unwrap();
        let row = AttemptRow::from_attempt(
            &attempt,
            &version_id,
            None,
            0,
            None,
            DataDomain::Learning,
            None,
        );
        insert_attempt(conn, &row).unwrap();
        let stored = get_attempt(conn, &learning(), &row.id).unwrap().unwrap();
        assert_eq!(stored, row);
        let rehydrated = stored
            .to_attempt(
                &def.id,
                ver.version,
                &operator_id(),
                def.competency_ids.clone(),
                ver.difficulty,
                vec![],
            )
            .unwrap();
        assert_eq!(rehydrated, attempt);

        // begin -> started
        let mut patch = stored.patch();
        patch.state = AssessmentAttemptState::Started;
        patch.started_at = Some(t(1));
        assert!(matches!(
            update_attempt(conn, &row.id, Revision::new(9), &patch, t(1)),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        let r2 = update_attempt(conn, &row.id, Revision::INITIAL, &patch, t(1)).unwrap();
        // illegal jump
        let mut jump = patch.clone();
        jump.state = AssessmentAttemptState::ApprovedFinal;
        jump.finalized_at = Some(t(2));
        assert!(matches!(
            update_attempt(conn, &row.id, r2, &jump, t(2)),
            Err(RepoError::Storage(StorageError::InvalidTransition(_)))
        ));
        // assistance recorded, never lowered
        let mut hinted = patch.clone();
        hinted.assistance_level = AssistanceLevel::Hinted;
        hinted.hint_count = 1;
        let r3 = update_attempt(conn, &row.id, r2, &hinted, t(2)).unwrap();
        let mut lowered = hinted.clone();
        lowered.assistance_level = AssistanceLevel::Unaided;
        assert!(matches!(
            update_attempt(conn, &row.id, r3, &lowered, t(3)),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        // submit -> evidence review -> provisional -> final
        let mut next = hinted.clone();
        next.state = AssessmentAttemptState::Submitted;
        next.submitted_at = Some(t(3));
        next.submission_sha256 = Some(sha("submission"));
        let r4 = update_attempt(conn, &row.id, r3, &next, t(3)).unwrap();
        next.state = AssessmentAttemptState::EvidenceReview;
        let r5 = update_attempt(conn, &row.id, r4, &next, t(4)).unwrap();
        next.state = AssessmentAttemptState::GradedProvisional;
        let r6 = update_attempt(conn, &row.id, r5, &next, t(5)).unwrap();
        next.state = AssessmentAttemptState::ApprovedFinal;
        assert!(matches!(
            update_attempt(conn, &row.id, r6, &next, t(6)),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        next.finalized_at = Some(t(6));
        let r7 = update_attempt(conn, &row.id, r6, &next, t(6)).unwrap();
        assert_eq!(r7, Revision::new(7));
        let final_row = get_attempt(conn, &learning(), &row.id).unwrap().unwrap();
        assert_eq!(final_row.state, AssessmentAttemptState::ApprovedFinal);
        assert_eq!(final_row.assistance_level, AssistanceLevel::Hinted);
        assert_eq!(
            list_attempts_by_state(conn, &learning(), AssessmentAttemptState::ApprovedFinal)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            list_attempts_for_version(conn, &learning(), &version_id)
                .unwrap()
                .len(),
            1
        );

        // A04: finalized attempts refuse updates in the repository and at the trigger
        let mut again = next.clone();
        again.hint_count = 2;
        assert!(matches!(
            update_attempt(conn, &row.id, r7, &again, t(7)),
            Err(RepoError::Storage(StorageError::Immutable(_)))
        ));
        assert_immutable(
            conn,
            "UPDATE attempts SET hint_count = 5 WHERE id = 'attempt-1'",
        );
        assert_immutable(conn, "DELETE FROM attempts WHERE id = 'attempt-1'");

        // retake is a new row linked to the original
        let mut retake = attempt_row("attempt-2", &version_id, AssessmentAttemptState::Available);
        retake.retake_of = Some(row.id.clone());
        insert_attempt(conn, &retake).unwrap();
        insert_retake_link(
            conn,
            &RetakeLinkRow {
                original_attempt_id: row.id.clone(),
                retake_attempt_id: retake.id.clone(),
                reason: "validity window elapsed".into(),
                created_by: operator_id(),
                created_at: t(8),
            },
        )
        .unwrap();
        assert_eq!(list_retakes(conn, &row.id).unwrap().len(), 1);
        assert_immutable(conn, "UPDATE retake_links SET reason = 'x'");
        assert_immutable(conn, "DELETE FROM retake_links");

        // cross-domain read refused
        let mut business = attempt_row("attempt-b", &version_id, AssessmentAttemptState::Available);
        business.data_domain = DataDomain::Business;
        insert_attempt(conn, &business).unwrap();
        assert!(matches!(
            get_attempt(conn, &learning(), &business.id),
            Err(RepoError::CrossDomainRefused {
                table: "attempts",
                ..
            })
        ));
        assert_eq!(
            list_attempts_for_version(conn, &learning(), &version_id)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn criterion_results_and_grade_decisions_are_append_only_with_supersession() {
        let db = db();
        let conn = db.conn();
        let (_def, ver, version_id) = seed(conn);
        let mut attempt = attempt_row(
            "attempt-1",
            &version_id,
            AssessmentAttemptState::GradedProvisional,
        );
        attempt.submitted_at = Some(t(1));
        insert_attempt(conn, &attempt).unwrap();
        let (rubric, _criteria) = rubric_for_version(conn, &version_id).unwrap().unwrap();

        let mut results = Vec::new();
        for (i, c) in ver.rubric.criteria().iter().enumerate() {
            let result = CriterionResult::new(
                attempt.id.clone(),
                c.code.clone(),
                Score::new(90).unwrap(),
                GradeSource::HumanReviewer,
                &operator(),
                t(2),
            )
            .unwrap()
            .with_note("meets the anchor");
            let criterion = criterion_id(conn, &rubric.id, &c.code).unwrap().unwrap();
            append_criterion_result(
                conn,
                &CriterionResultRow::from_domain(&result, &format!("cr-{i}"), &criterion, None),
            )
            .unwrap();
            results.push(result);
        }
        let stored = list_criterion_results(conn, &attempt.id).unwrap();
        assert_eq!(stored.len(), 6);
        let domain: Vec<CriterionResult> = stored.iter().map(|s| s.to_domain().unwrap()).collect();
        assert_eq!(domain, results);
        assert_immutable(
            conn,
            "UPDATE criterion_results SET score = 0 WHERE id = 'cr-0'",
        );
        assert_immutable(conn, "DELETE FROM criterion_results WHERE id = 'cr-0'");

        // a model-advisory correction superseding cr-0 is appended; effective set drops cr-0
        let model = Actor::model(ActorId::parse("model-1").unwrap());
        let advisory = CriterionResult::new(
            attempt.id.clone(),
            stored[0].criterion_code.clone(),
            Score::new(40).unwrap(),
            GradeSource::ModelAdvisory,
            &model,
            t(3),
        )
        .unwrap();
        append_criterion_result(
            conn,
            &CriterionResultRow::from_domain(
                &advisory,
                "cr-adv",
                &stored[0].row.criterion_id,
                Some("cr-0".into()),
            ),
        )
        .unwrap();
        let effective = effective_criterion_results(conn, &attempt.id).unwrap();
        assert_eq!(effective.len(), 6);
        assert!(effective.iter().any(|e| e.row.id == "cr-adv"));
        assert!(!effective.iter().any(|e| e.row.id == "cr-0"));

        let breakdown = score_breakdown(&ver.rubric, &results);
        let decision = GradeDecision {
            id: GradeDecisionId::parse("gd-1").unwrap(),
            attempt_id: attempt.id.clone(),
            breakdown,
            pass: true,
            is_final: true,
            decided_by: operator_id(),
            decided_at: t(4),
            supersedes: None,
            rationale: None,
        };
        let apr = approval(conn, "apr-1");
        let row =
            GradeDecisionRow::from_domain(&decision, Some(apr), Some(sha("results"))).unwrap();
        assert_eq!(row.total_score, 90);
        assert_eq!(
            row.reliability_class,
            ReliabilityClass::SelfAdjudicated,
            "least-trusted default"
        );
        let row = row.with_reliability_class(ReliabilityClass::Deterministic);
        append_grade_decision(conn, &row).unwrap();
        assert_eq!(
            effective_final_decision(conn, &attempt.id)
                .unwrap()
                .unwrap(),
            row
        );
        assert_immutable(
            conn,
            "UPDATE grade_decisions SET total_score = 100 WHERE id = 'gd-1'",
        );
        assert_immutable(
            conn,
            "UPDATE grade_decisions SET reliability_class = 'EXTERNAL_REVIEW' WHERE id = 'gd-1'",
        );
        assert_immutable(conn, "DELETE FROM grade_decisions WHERE id = 'gd-1'");

        // a correction is a new superseding row
        let mut corrected = row.clone();
        corrected.id = GradeDecisionId::parse("gd-2").unwrap();
        corrected.total_score = 85;
        corrected.supersedes = Some(row.id.clone());
        corrected.rationale = Some("adjudicated after appeal".into());
        corrected.decided_at = t(5);
        corrected.created_at = t(5);
        corrected.updated_at = t(5);
        append_grade_decision(conn, &corrected).unwrap();
        assert_eq!(list_grade_decisions(conn, &attempt.id).unwrap().len(), 2);
        assert_eq!(
            effective_final_decision(conn, &attempt.id)
                .unwrap()
                .unwrap()
                .id
                .as_str(),
            "gd-2"
        );

        // decisions without a weighted total cannot be persisted
        let incomplete = GradeDecision {
            breakdown: score_breakdown(&ver.rubric, &[]),
            ..decision.clone()
        };
        assert!(GradeDecisionRow::from_domain(&incomplete, None, None).is_err());

        // attempt evidence
        conn.execute(
            "INSERT INTO evidence_items (id, kind, summary, content_sha256, trust, provenance_json, created_at, updated_at)
             VALUES ('evi-1', 'ATTEMPT_SUBMISSION', 'submission', ?1, 'OPERATOR', '{}', 1, 1)",
            params![sha("submission")],
        )
        .unwrap();
        let link = AttemptEvidenceRow {
            attempt_id: attempt.id.clone(),
            evidence_item_id: EvidenceId::parse("evi-1").unwrap(),
            role: AttemptEvidenceRole::Submission,
            added_at: t(1),
        };
        link_attempt_evidence(conn, &link).unwrap();
        assert!(matches!(
            link_attempt_evidence(conn, &link),
            Err(RepoError::Storage(StorageError::UniqueViolation(_)))
        ));
        assert_eq!(
            list_attempt_evidence(conn, &attempt.id).unwrap(),
            vec![link]
        );
        assert_immutable(conn, "UPDATE attempt_evidence SET role = 'REVIEWER'");
        assert_immutable(conn, "DELETE FROM attempt_evidence");
    }

    /// Baseline-rubric decision from anchored levels (rubric order), with the
    /// pass bit taken from the crate's exact-integer pass rule.
    fn decision_from_levels(levels: [u8; 6]) -> GradeDecision {
        let rubric = Rubric::baseline();
        let attempt = AttemptId::parse("attempt-cut").unwrap();
        let results: Vec<CriterionResult> = rubric
            .criteria()
            .iter()
            .zip(levels)
            .map(|(c, level)| {
                CriterionResult::new(
                    attempt.clone(),
                    c.code.clone(),
                    Score::new(rubric.score_for_level(level).unwrap()).unwrap(),
                    GradeSource::HumanReviewer,
                    &operator(),
                    t(2),
                )
                .unwrap()
            })
            .collect();
        let breakdown = score_breakdown(&rubric, &results);
        let pass = breakdown.pass_recommendation.unwrap();
        GradeDecision {
            id: GradeDecisionId::parse("gd-cut").unwrap(),
            attempt_id: attempt,
            breakdown,
            pass,
            is_final: true,
            decided_by: operator_id(),
            decided_at: t(4),
            supersedes: None,
            rationale: None,
        }
    }

    #[test]
    fn rubric_anchors_are_persisted_hash_checked_and_custom_scales_refused() {
        let db = db();
        let conn = db.conn();
        let (_def, ver, version_id) = seed(conn);
        // the seeded baseline rubric round-trips with its anchors
        let (row, criteria) = rubric_for_version(conn, &version_id).unwrap().unwrap();
        assert_eq!(row.anchors.as_ref(), Some(ver.rubric.anchors()));
        assert_eq!(
            rubric_from_stored(&row, &criteria, Rubric::baseline_dimension).unwrap(),
            ver.rubric
        );

        // an authored note and descriptors are stored with the version, not dropped
        let defs = Rubric::baseline().criteria().to_vec();
        let mut anchors = RubricAnchors::standard_for(&defs);
        anchors.standard_setting_note = "Modified Angoff panel of three reviewers".into();
        anchors.criteria[0].descriptors = ["absent", "fragmentary", "partial", "solid", "expert"]
            .map(String::from)
            .to_vec();
        let authored = Rubric::with_anchors(defs.clone(), anchors.clone()).unwrap();
        let (r2, c2) =
            rubric_rows_from_domain(&authored, "rub-2", &version_id, "Authored", "v2", t(1));
        insert_rubric(conn, &r2, &c2).unwrap();
        let (row, criteria) = rubric_for_version(conn, &version_id).unwrap().unwrap();
        assert_eq!(row.id, "rub-2");
        assert_eq!(row.anchors.as_ref(), Some(&anchors));
        assert_eq!(
            rubric_from_stored(&row, &criteria, Rubric::baseline_dimension).unwrap(),
            authored
        );

        // a custom level scale is refused before any row is written: the app still
        // regrades on the standard scale, so 82.75 (PASS) would silently become 79.5
        let mut custom = RubricAnchors::standard_for(&defs);
        custom.scale = LevelScale::new([0, 20, 55, 85, 100]).unwrap();
        let rescaled = Rubric::with_anchors(defs.clone(), custom).unwrap();
        let (r3, c3) =
            rubric_rows_from_domain(&rescaled, "rub-3", &version_id, "Rescaled", "v3", t(2));
        let rubrics_before = count(conn, "rubrics");
        let criteria_before = count(conn, "rubric_criteria");
        assert!(matches!(
            insert_rubric(conn, &r3, &c3),
            Err(RepoError::Storage(StorageError::NotImplemented(
                "assessment.rubric.custom_level_scale"
            )))
        ));
        assert_eq!(count(conn, "rubrics"), rubrics_before);
        assert_eq!(count(conn, "rubric_criteria"), criteria_before);

        // a pre-0012 row (no anchors) reads back with the standard anchors
        let (mut r4, c4) = rubric_rows_from_domain(
            &Rubric::baseline(),
            "rub-4",
            &version_id,
            "Legacy",
            "v4",
            t(3),
        );
        r4.anchors = None;
        insert_rubric(conn, &r4, &c4).unwrap();
        let (row, criteria) = rubric_for_version(conn, &version_id).unwrap().unwrap();
        assert_eq!((row.id.as_str(), row.anchors.as_ref()), ("rub-4", None));
        assert_eq!(
            rubric_from_stored(&row, &criteria, Rubric::baseline_dimension).unwrap(),
            Rubric::baseline()
        );

        // anchors whose bytes do not match the stored hash are corruption
        conn.execute(
            "INSERT INTO rubrics (id, assessment_version_id, title, version_label, total_weight, anchors_json,
                 anchors_sha256, created_at)
             VALUES ('rub-5', ?1, 'Tampered', 'v5', 100, '{}', ?2, ?3)",
            params![version_id, sha("not the anchors"), ms(t(4))],
        )
        .unwrap();
        assert!(matches!(
            rubric_for_version(conn, &version_id),
            Err(RepoError::Storage(StorageError::Corrupt(_)))
        ));
        // the two anchor columns are written together (trigger backstop)
        let unpaired: StorageError = conn
            .execute(
                "INSERT INTO rubrics (id, assessment_version_id, title, version_label, total_weight, anchors_json,
                     created_at)
                 VALUES ('rub-6', ?1, 'Unpaired', 'v6', 100, '{}', ?2)",
                params![version_id, ms(t(5))],
            )
            .expect_err("unpaired anchors must be refused")
            .into();
        assert!(
            matches!(&unpaired, StorageError::Precondition(m) if m.contains("written together")),
            "{unpaired:?}"
        );
    }

    #[test]
    fn grade_total_is_rounded_down_and_never_crosses_the_pass_cut() {
        let threshold = g9_domain::PASS_THRESHOLD;
        // (levels, exact weighted total, stored total, pass)
        let cases: [([u8; 6], f64, u8, bool); 5] = [
            // 25*80 + 20*100 + 20*80 + 15*50 + 10*80 + 10*80 = 7950 → 79.5, every floor met
            ([3, 4, 3, 2, 3, 3], 79.5, 79, false),
            // 25*50 + 20*80 + 20*80 + 15*100 + 10*100 + 10*100 = 7950 → 79.5
            ([2, 3, 3, 4, 4, 4], 79.5, 79, false),
            ([3, 3, 3, 3, 3, 3], 80.0, 80, true),
            // 25*100 + 20*80 + 20*80 + 15*80 + 10*80 + 10*80 = 8500 → 85.0
            ([4, 3, 3, 3, 3, 3], 85.0, 85, true),
            ([4, 4, 4, 4, 4, 4], 100.0, 100, true),
        ];
        for (levels, exact, stored, pass) in cases {
            let decision = decision_from_levels(levels);
            assert_eq!(decision.breakdown.weighted_total, Some(exact), "{levels:?}");
            assert_eq!(decision.pass, pass, "{levels:?}");
            let row = GradeDecisionRow::from_domain(&decision, None, None).unwrap();
            assert_eq!(row.total_score, stored, "{levels:?}");
            assert_eq!(row.passed, pass, "{levels:?}");
            // the stored total and the stored pass bit never contradict the cut
            assert_eq!(
                row.total_score >= threshold,
                exact >= f64::from(threshold),
                "{levels:?}"
            );
        }
        // a fractional total just under the cut (79.9) is stored as 79, never 80
        let mut decision = decision_from_levels([3, 3, 3, 3, 3, 3]);
        decision.breakdown.weighted_total = Some(79.9);
        decision.pass = false;
        let row = GradeDecisionRow::from_domain(&decision, None, None).unwrap();
        assert_eq!(row.total_score, 79);
        assert!(row.total_score < threshold);
    }

    #[test]
    fn exposure_log_is_immutable_and_drives_blind_eligibility() {
        let db = db();
        let conn = db.conn();
        let (_def, _ver, version_id) = seed(conn);
        insert_attempt(
            conn,
            &attempt_row("attempt-1", &version_id, AssessmentAttemptState::Started),
        )
        .unwrap();
        assert!(!exposed_before(
            conn,
            &version_id,
            ExposureContentKind::Task,
            ExposedTo::Operator,
            t(100)
        )
        .unwrap());
        let row = ExposureLogRow {
            id: "exp-1".into(),
            assessment_version_id: version_id.clone(),
            attempt_id: Some(AttemptId::parse("attempt-1").unwrap()),
            exposed_to: ExposedTo::Operator,
            actor_id: operator_id(),
            mode: InteractionMode::Exam,
            content_kind: ExposureContentKind::from_domain(ExposureKind::Task).unwrap(),
            content_sha256: sha("Recover the boot chain"),
            reason: Some("attempt started".into()),
            exposed_at: t(10),
            created_at: t(10),
        };
        append_exposure(conn, &row).unwrap();
        assert_eq!(
            list_exposures(conn, &version_id).unwrap(),
            vec![row.clone()]
        );
        assert_eq!(
            list_exposures_for_attempt(conn, &AttemptId::parse("attempt-1").unwrap())
                .unwrap()
                .len(),
            1
        );
        assert!(exposed_before(
            conn,
            &version_id,
            ExposureContentKind::Task,
            ExposedTo::Operator,
            t(100)
        )
        .unwrap());
        assert!(!exposed_before(
            conn,
            &version_id,
            ExposureContentKind::Task,
            ExposedTo::Operator,
            t(10)
        )
        .unwrap());
        assert!(!exposed_before(
            conn,
            &version_id,
            ExposureContentKind::Key,
            ExposedTo::Operator,
            t(100)
        )
        .unwrap());
        assert_eq!(
            ExposureContentKind::from_domain(ExposureKind::AnswerKey).unwrap(),
            ExposureContentKind::Key
        );
        assert!(matches!(
            ExposureContentKind::from_domain(ExposureKind::Hint),
            Err(StorageError::NotImplemented("assessment.exposure.hint"))
        ));
        assert_immutable(
            conn,
            "UPDATE exposure_log SET reason = 'x' WHERE id = 'exp-1'",
        );
        assert_immutable(conn, "DELETE FROM exposure_log WHERE id = 'exp-1'");
    }

    /// storage-repos-10: a multi-statement write is all-or-nothing on any
    /// connection. On an autocommit connection (the CLI seed) a rubric whose
    /// second criterion fails used to leave the rubric row and its first
    /// criterion behind; inside a caller's transaction the failure undoes only
    /// this write and leaves the transaction usable.
    #[test]
    fn a_failed_rubric_insert_leaves_no_row_on_a_bare_connection_or_in_a_transaction() {
        let mut db = db();
        let (_def, ver, version_id) = seed(db.conn());
        let (rubric, mut criteria) =
            rubric_rows_from_domain(&ver.rubric, "rub-dup", &version_id, "Dup", "v2", t(1));
        criteria[1].code = criteria[0].code.clone();
        let rubrics_before = count(db.conn(), "rubrics");
        let criteria_before = count(db.conn(), "rubric_criteria");
        assert!(db.conn().is_autocommit());
        assert!(matches!(
            insert_rubric(db.conn(), &rubric, &criteria),
            Err(RepoError::Storage(StorageError::UniqueViolation(_)))
        ));
        assert!(db.conn().is_autocommit(), "no transaction is left open");
        assert_eq!(count(db.conn(), "rubrics"), rubrics_before);
        assert_eq!(count(db.conn(), "rubric_criteria"), criteria_before);

        let tx = db.begin_immediate().unwrap();
        let (good, good_criteria) =
            rubric_rows_from_domain(&ver.rubric, "rub-ok", &version_id, "Ok", "v3", t(2));
        insert_rubric(&tx, &good, &good_criteria).unwrap();
        assert!(insert_rubric(&tx, &rubric, &criteria).is_err());
        tx.commit().unwrap();
        assert_eq!(count(db.conn(), "rubrics"), rubrics_before + 1);
        assert_eq!(
            count(db.conn(), "rubric_criteria"),
            criteria_before + i64::try_from(good_criteria.len()).unwrap()
        );
    }
}
