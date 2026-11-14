//! Curriculum repository (M02; migration `0002_curriculum`).
//!
//! A manifest is persisted as one immutable `curriculum_versions` row plus its
//! structural tree (`weeks` → `training_days` → `daily_objectives`,
//! `study_blocks`, `resource_references`, `deliverable_definitions`). The tree is
//! written once by [`insert_version`] and never rewritten: promotion
//! ([`set_provenance`]) touches only the provenance columns of a non-canonical
//! row, and an `APPROVED_CANONICAL` row refuses every further change (C01).
//!
//! The repository does not decide whether a promotion is allowed (that is
//! `g9_curriculum::check_transition` with the reconciliation report, applied by
//! `g9-app` after policy and approval); it only enforces the storage invariants:
//! provenance moves forward, a seed never becomes canonical directly, canonical
//! promotion carries an approval reference, and only a canonical version can be
//! current.
//!
//! Row-id scheme for structural rows (deterministic, no randomness):
//! `<version_id>:W<week>`, `<version_id>:<day_id>`, `<version_id>:<objective_id>`,
//! `<version_id>:<day_id>:B<seq>`, `<version_id>:<day_id>:R<n>`,
//! `<version_id>:<day_id>:EVIDENCE`.
//!
//! Mapping decisions (INFERRED, documented here because the manifest format has
//! no counterpart column): `study_blocks.mode` is derived from the block code
//! ([`block_mode`]); `training_days.title` is the day id; a day's
//! `evidence_artifact` becomes one `deliverable_definitions` row of kind
//! `ATTEMPT_SUBMISSION` on assessment days and `NOTE` otherwise; a day's
//! resources are linked to the day's first objective (`SET NULL` informational
//! link) because `resource_references` has no training-day column.

use super::{
    bool_i64, finish_update, from_json, from_ms, i64_bool, ms, opt_from_ms, opt_parse_id,
    query_all, query_opt, required, rev_from_i64, rev_i64, to_json, u32_from_i64, u8_from_i64,
    RepoError, RepoResult,
};
use crate::error::{StorageError, StorageResult};
use crate::sql::{enum_from_text, enum_to_text, parse_id};
use g9_curriculum::{Checksum, Manifest, ValidationReport};
use g9_domain::{
    ActorId, ApprovalId, CurriculumId, CurriculumVersionId, DayId, DayOfWeek, InteractionMode,
    LearningDimension, ManifestProvenance, ObjectiveId, Revision, UtcMillis,
};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

const TABLE: &str = "curriculum_versions";

/// Input for [`insert_version`].
#[derive(Debug, Clone, PartialEq)]
pub struct NewCurriculumVersion<'a> {
    /// Version id chosen by the caller (UUIDv7 from `g9-app`; fixtures in tests).
    pub id: CurriculumVersionId,
    /// The parsed manifest (data, never instructions).
    pub manifest: &'a Manifest,
    /// Validation report produced by `g9_curriculum::validate`; an invalid report
    /// blocks the import (gate A01).
    pub validation: &'a ValidationReport,
    /// Provenance recorded on the row.
    pub provenance: ManifestProvenance,
    /// Checksum of the canonical manifest JSON (what approvals bind to).
    pub checksum: &'a Checksum,
    /// Where the manifest came from (approved-root path or source id).
    pub source_ref: String,
    /// Importing operator.
    pub imported_by: ActorId,
    /// Import instant.
    pub imported_at: UtcMillis,
}

/// Approval reference recorded when a version becomes `APPROVED_CANONICAL`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceApproval {
    /// Consumed approval token (`approval_requests.id`).
    pub approval_id: ApprovalId,
    /// Operator who approved.
    pub approved_by: ActorId,
    /// When.
    pub approved_at: UtcMillis,
}

/// A `curriculum_versions` row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurriculumVersionRow {
    /// Version id.
    pub id: CurriculumVersionId,
    /// Curriculum (program) id.
    pub curriculum_id: CurriculumId,
    /// Manifest `version` string.
    pub version_label: String,
    /// Provenance state.
    pub provenance: ManifestProvenance,
    /// Checksum of the manifest.
    pub manifest_sha256: String,
    /// Source reference.
    pub source_ref: String,
    /// Stored validation report.
    pub invariants: serde_json::Value,
    /// Importing operator.
    pub imported_by: ActorId,
    /// Import instant.
    pub imported_at: UtcMillis,
    /// Approving operator (canonical only).
    pub approved_by: Option<ActorId>,
    /// Approval instant.
    pub approved_at: Option<UtcMillis>,
    /// Approval token.
    pub approval_id: Option<ApprovalId>,
    /// Whether this is the current canonical version of its curriculum.
    pub is_current: bool,
    /// Optimistic-concurrency revision.
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

/// A `weeks` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeekRow {
    /// Row id.
    pub id: String,
    /// Owning version.
    pub curriculum_version_id: CurriculumVersionId,
    /// 1..=12.
    pub week_number: u32,
    /// Title.
    pub title: String,
    /// Theme (none in manifest v1.0.0).
    pub theme: Option<String>,
    /// Sum of the days' planned minutes.
    pub planned_minutes: u32,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// A `training_days` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainingDayRow {
    /// Row id.
    pub id: String,
    /// Owning week row.
    pub week_id: String,
    /// Owning version.
    pub curriculum_version_id: CurriculumVersionId,
    /// Manifest day id (`D01`).
    pub day_code: DayId,
    /// 1..=72 within the version.
    pub day_number: u32,
    /// Monday = 1 .. Saturday = 6.
    pub day_of_week: DayOfWeek,
    /// Planned minutes.
    pub planned_minutes: u32,
    /// Title (the day id).
    pub title: String,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// A `daily_objectives` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DailyObjectiveRow {
    /// Row id.
    pub id: String,
    /// Owning day row.
    pub training_day_id: String,
    /// Owning version.
    pub curriculum_version_id: CurriculumVersionId,
    /// Manifest objective id (`D01-O1`).
    pub objective_code: ObjectiveId,
    /// 1..=3.
    pub ordinal: u8,
    /// Objective statement.
    pub statement: String,
    /// Success criteria (none in manifest v1.0.0).
    pub success_criteria: Option<String>,
    /// Learning dimension, when assigned.
    pub dimension: Option<LearningDimension>,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// A `study_blocks` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyBlockRow {
    /// Row id.
    pub id: String,
    /// Owning day row.
    pub training_day_id: String,
    /// Linked objective row, if any.
    pub objective_id: Option<String>,
    /// Order within the day.
    pub ordinal: u32,
    /// Block code (`recall`, `theory`, ...).
    pub block_code: String,
    /// Minutes.
    pub minutes: u32,
    /// Interaction mode of the block.
    pub mode: InteractionMode,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// A `resource_references` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceReferenceRow {
    /// Row id.
    pub id: String,
    /// Owning version.
    pub curriculum_version_id: CurriculumVersionId,
    /// Linked objective row, if any.
    pub objective_id: Option<String>,
    /// Title (the manifest reference string).
    pub title: String,
    /// Stable reference.
    pub source_ref: String,
    /// URI, if the reference is external.
    pub uri: Option<String>,
    /// Trust class (`UNKNOWN` until approved as a source).
    pub trust: g9_domain::TrustClass,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// A `deliverable_definitions` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliverableDefinitionRow {
    /// Row id.
    pub id: String,
    /// Owning day row.
    pub training_day_id: String,
    /// Linked objective row, if any.
    pub objective_id: Option<String>,
    /// Deliverable code within the day.
    pub code: String,
    /// Title (the manifest `evidence_artifact`).
    pub title: String,
    /// Evidence kind text (mirrors the CHECK list).
    pub evidence_kind: String,
    /// Whether the deliverable is required.
    pub is_required: bool,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Interaction mode a study block runs in, by block code (INFERRED from the
/// handoff's block descriptions: labs run in LAB, the blind practical in EXAM,
/// recall and performance review in REVIEW, everything else in STUDY).
pub fn block_mode(block_code: &str) -> Option<InteractionMode> {
    Some(match block_code {
        "recall" | "performance_review" => InteractionMode::Review,
        "theory" | "evidence_reporting" | "remediation" | "report" => InteractionMode::Study,
        "guided_lab" | "independent_lab" | "defense_verification" => InteractionMode::Lab,
        "blind_practical" => InteractionMode::Exam,
        _ => return None,
    })
}

fn provenance_rank(p: ManifestProvenance) -> u8 {
    match p {
        ManifestProvenance::ProposedSeed => 0,
        ManifestProvenance::ReconciledCandidate => 1,
        ManifestProvenance::ApprovedCanonical => 2,
    }
}

fn minutes_u32(v: i64, what: &str) -> StorageResult<u32> {
    u32::try_from(v)
        .map_err(|_| StorageError::InvalidInput(format!("{what} minutes must be 0..=u32::MAX")))
}

/// Persist a manifest as a new immutable version with its structural tree.
/// Refuses an invalid validation report (`PRECONDITION_FAILED`) and a duplicate
/// `(curriculum_id, version)` (`CONFLICT`). Runs inside the caller's transaction.
pub fn insert_version(
    conn: &Connection,
    input: &NewCurriculumVersion<'_>,
) -> RepoResult<CurriculumVersionId> {
    if !input.validation.is_valid() {
        return Err(RepoError::Storage(StorageError::Precondition(format!(
            "curriculum.import_blocked: {} error(s), {} invariant violation(s)",
            input.validation.errors.len(),
            input.validation.violations.len()
        ))));
    }
    let manifest = input.manifest;
    let curriculum_id: CurriculumId = parse_id(&manifest.curriculum_id)?;
    let now = ms(input.imported_at);
    let version_id = input.id.as_str();

    conn.execute(
        "INSERT INTO curricula (id, title, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)
         ON CONFLICT (id) DO NOTHING",
        params![curriculum_id.as_str(), manifest.name, now],
    )?;
    conn.execute(
        "INSERT INTO curriculum_versions (id, curriculum_id, version_label, provenance, manifest_sha256, source_ref,
            invariants_json, imported_by, imported_at, is_current, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 0, ?9, ?9)",
        params![
            version_id,
            curriculum_id.as_str(),
            manifest.version,
            enum_to_text(&input.provenance)?,
            input.checksum.as_str(),
            input.source_ref,
            to_json(input.validation)?,
            input.imported_by.as_str(),
            now,
        ],
    )?;

    let days_per_week = manifest.structure.days_per_week;
    for week in &manifest.weeks {
        let week_id = format!("{version_id}:W{:02}", week.week_number);
        let planned: i64 = week.days.iter().map(|d| d.planned_minutes.max(0)).sum();
        conn.execute(
            "INSERT INTO weeks (id, curriculum_version_id, week_number, title, theme, planned_minutes, created_at)
             VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6)",
            params![
                week_id,
                version_id,
                i64::from(week.week_number),
                week.title,
                i64::from(minutes_u32(planned, "week")?),
                now
            ],
        )?;
        for day in &week.days {
            let day_code: DayId = parse_id(&day.day_id)?;
            let day_row_id = format!("{version_id}:{}", day.day_id);
            let day_number = (week.week_number.saturating_sub(1))
                .saturating_mul(days_per_week)
                .saturating_add(u32::from(day.day_of_week));
            conn.execute(
                "INSERT INTO training_days (id, week_id, curriculum_version_id, day_code, day_number, day_of_week,
                    planned_minutes, title, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    day_row_id,
                    week_id,
                    version_id,
                    day_code.as_str(),
                    i64::from(day_number),
                    i64::from(day.day_of_week),
                    i64::from(minutes_u32(day.planned_minutes, "day")?),
                    day.day_id,
                    now
                ],
            )?;
            let mut first_objective_row: Option<String> = None;
            for objective in &day.objectives {
                let code: ObjectiveId = parse_id(&objective.objective_id)?;
                let row_id = format!("{version_id}:{}", objective.objective_id);
                conn.execute(
                    "INSERT INTO daily_objectives (id, training_day_id, curriculum_version_id, objective_code, ordinal,
                        statement, success_criteria, dimension, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, ?7)",
                    params![
                        row_id,
                        day_row_id,
                        version_id,
                        code.as_str(),
                        i64::from(objective.seq),
                        objective.text,
                        now
                    ],
                )?;
                if first_objective_row.is_none() {
                    first_objective_row = Some(row_id);
                }
            }
            for block in &day.blocks {
                let mode = block_mode(&block.block).ok_or_else(|| {
                    StorageError::InvalidInput(format!(
                        "unknown study block code {:?} on day {}",
                        block.block, day.day_id
                    ))
                })?;
                conn.execute(
                    "INSERT INTO study_blocks (id, training_day_id, objective_id, ordinal, block_code, minutes, mode, created_at)
                     VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        format!("{day_row_id}:B{}", block.seq),
                        day_row_id,
                        i64::from(block.seq),
                        block.block,
                        i64::from(minutes_u32(block.minutes, "block")?),
                        enum_to_text(&mode)?,
                        now
                    ],
                )?;
            }
            for (n, resource) in day.resources.iter().enumerate() {
                let uri = match resource.kind {
                    g9_curriculum::ResourceKind::External => Some(resource.reference.as_str()),
                    g9_curriculum::ResourceKind::Internal => None,
                };
                conn.execute(
                    "INSERT INTO resource_references (id, curriculum_version_id, objective_id, title, source_ref, uri, trust,
                        data_domain, classification, created_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'UNKNOWN', 'LEARNING', 'INTERNAL', ?7)",
                    params![
                        format!("{day_row_id}:R{}", n + 1),
                        version_id,
                        first_objective_row,
                        resource.reference,
                        resource.reference,
                        uri,
                        now
                    ],
                )?;
            }
            let evidence_kind = if day.is_assessment_day {
                "ATTEMPT_SUBMISSION"
            } else {
                "NOTE"
            };
            conn.execute(
                "INSERT INTO deliverable_definitions (id, training_day_id, objective_id, code, title, description,
                    evidence_kind, is_required, created_at)
                 VALUES (?1, ?2, ?3, 'EVIDENCE', ?4, NULL, ?5, 1, ?6)",
                params![
                    format!("{day_row_id}:EVIDENCE"),
                    day_row_id,
                    first_objective_row,
                    day.evidence_artifact,
                    evidence_kind,
                    now
                ],
            )?;
        }
    }
    Ok(input.id.clone())
}

const VERSION_COLUMNS: &str = "id, curriculum_id, version_label, provenance, manifest_sha256, source_ref, invariants_json,
    imported_by, imported_at, approved_by, approved_at, approval_id, is_current, revision, created_at, updated_at";

fn map_version(r: &Row<'_>) -> StorageResult<CurriculumVersionRow> {
    Ok(CurriculumVersionRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        curriculum_id: parse_id(&r.get::<_, String>("curriculum_id")?)?,
        version_label: r.get("version_label")?,
        provenance: enum_from_text(&r.get::<_, String>("provenance")?)?,
        manifest_sha256: r.get("manifest_sha256")?,
        source_ref: r.get("source_ref")?,
        invariants: from_json(&r.get::<_, String>("invariants_json")?, "invariants_json")?,
        imported_by: parse_id(&r.get::<_, String>("imported_by")?)?,
        imported_at: from_ms(r.get("imported_at")?),
        approved_by: opt_parse_id(r.get("approved_by")?)?,
        approved_at: opt_from_ms(r.get("approved_at")?),
        approval_id: opt_parse_id(r.get("approval_id")?)?,
        is_current: i64_bool(r.get("is_current")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// One version by id.
pub fn get_version(
    conn: &Connection,
    id: &CurriculumVersionId,
) -> RepoResult<Option<CurriculumVersionRow>> {
    query_opt(
        conn,
        &format!("SELECT {VERSION_COLUMNS} FROM curriculum_versions WHERE id = ?1"),
        &[&id.as_str()],
        map_version,
    )
}

/// Every version of a curriculum, oldest import first.
pub fn list_versions(
    conn: &Connection,
    curriculum_id: &CurriculumId,
) -> RepoResult<Vec<CurriculumVersionRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {VERSION_COLUMNS} FROM curriculum_versions WHERE curriculum_id = ?1 ORDER BY imported_at, id"
        ),
        &[&curriculum_id.as_str()],
        map_version,
    )
}

/// The current canonical version of a curriculum, if one was promoted.
pub fn current_version(
    conn: &Connection,
    curriculum_id: &CurriculumId,
) -> RepoResult<Option<CurriculumVersionRow>> {
    query_opt(
        conn,
        &format!(
            "SELECT {VERSION_COLUMNS} FROM curriculum_versions WHERE curriculum_id = ?1 AND is_current = 1"
        ),
        &[&curriculum_id.as_str()],
        map_version,
    )
}

/// Move a non-canonical version forward in provenance, touching only the
/// provenance columns (`provenance`, `approved_*`, `approval_id`, `is_current`).
///
/// Rules: the target must rank above the stored provenance; `PROPOSED_SEED →
/// APPROVED_CANONICAL` is refused (reconciliation first, C01); becoming
/// `APPROVED_CANONICAL` requires `approval`, marks the version current and
/// clears `is_current` on the curriculum's other versions; a stored
/// `APPROVED_CANONICAL` row is immutable (`PRECONDITION_FAILED`). The caller has
/// already checked `g9_curriculum::check_transition` with the reconciliation
/// report and consumed the approval in the same transaction.
pub fn set_provenance(
    conn: &Connection,
    id: &CurriculumVersionId,
    expected_revision: Revision,
    target: ManifestProvenance,
    approval: Option<&ProvenanceApproval>,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let current = required(TABLE, get_version(conn, id)?)?;
    if current.revision != expected_revision {
        return Err(RepoError::Storage(StorageError::StaleRevision {
            expected: Some(expected_revision),
            actual: current.revision,
        }));
    }
    if current.provenance == ManifestProvenance::ApprovedCanonical {
        return Err(RepoError::Storage(StorageError::Immutable(
            "approved canonical curriculum version cannot change provenance".to_string(),
        )));
    }
    if provenance_rank(target) <= provenance_rank(current.provenance) {
        return Err(RepoError::Storage(StorageError::Precondition(format!(
            "curriculum.provenance.not_forward: {:?} -> {:?}",
            current.provenance, target
        ))));
    }
    if current.provenance == ManifestProvenance::ProposedSeed
        && target == ManifestProvenance::ApprovedCanonical
    {
        return Err(RepoError::Storage(StorageError::Precondition(
            "curriculum.provenance.seed_requires_reconciliation".to_string(),
        )));
    }
    let canonical = target == ManifestProvenance::ApprovedCanonical;
    let approval = match (canonical, approval) {
        (true, None) => {
            return Err(RepoError::Storage(StorageError::Precondition(
                "curriculum.provenance.approval_required".to_string(),
            )))
        }
        (true, Some(a)) => Some(a),
        (false, _) => None,
    };
    if canonical {
        conn.execute(
            "UPDATE curriculum_versions SET is_current = 0, revision = revision + 1, updated_at = ?3
             WHERE curriculum_id = ?1 AND id <> ?2 AND is_current = 1",
            params![current.curriculum_id.as_str(), id.as_str(), ms(now)],
        )?;
    }
    let changed = conn.execute(
        "UPDATE curriculum_versions
         SET provenance = ?3, approved_by = ?4, approved_at = ?5, approval_id = ?6, is_current = ?7,
             revision = revision + 1, updated_at = ?8
         WHERE id = ?1 AND revision = ?2 AND provenance <> 'APPROVED_CANONICAL'",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            enum_to_text(&target)?,
            approval.map(|a| a.approved_by.as_str().to_string()),
            approval.map(|a| ms(a.approved_at)),
            approval.map(|a| a.approval_id.as_str().to_string()),
            bool_i64(canonical),
            ms(now),
        ],
    )?;
    finish_update(conn, TABLE, "id", id.as_str(), expected_revision, changed)
}

/// Links from outside the curriculum tree into a version's days, objectives or
/// blocks, as `(table, column, parent table)`. The DDL declares them all
/// `ON DELETE SET NULL` (informational links, ADR-0002), so the database alone
/// would let a delete silently drop them; [`delete_version`] checks them first.
/// The unit test `version_links_cover_every_outside_reference` keeps this list
/// equal to the schema.
pub const VERSION_LINKS: &[(&str, &str, &str)] = &[
    ("assessment_definitions", "objective_id", "daily_objectives"),
    ("assessment_definitions", "training_day_id", "training_days"),
    ("calendar_events", "objective_id", "daily_objectives"),
    ("calendar_events", "study_block_id", "study_blocks"),
    ("calendar_events", "training_day_id", "training_days"),
    ("learning_episodes", "objective_id", "daily_objectives"),
    ("notes", "objective_id", "daily_objectives"),
    ("review_items", "objective_id", "daily_objectives"),
    ("study_sessions", "objective_id", "daily_objectives"),
    ("study_sessions", "study_block_id", "study_blocks"),
    ("study_sessions", "training_day_id", "training_days"),
];

/// Ids of a version's rows in one structural table (`?1` = version id).
fn version_member_ids(parent: &str) -> StorageResult<&'static str> {
    Ok(match parent {
        "training_days" => "SELECT id FROM training_days WHERE curriculum_version_id = ?1",
        "daily_objectives" => "SELECT id FROM daily_objectives WHERE curriculum_version_id = ?1",
        "study_blocks" => {
            "SELECT b.id FROM study_blocks b JOIN training_days d ON d.id = b.training_day_id
             WHERE d.curriculum_version_id = ?1"
        }
        _ => return Err(StorageError::Internal("curriculum version link parent")),
    })
}

/// Outside rows that link to the version, as `table.column` names (empty when none).
pub fn version_dependents(conn: &Connection, id: &CurriculumVersionId) -> RepoResult<Vec<String>> {
    let mut found = Vec::new();
    for (table, column, parent) in VERSION_LINKS {
        // table, column and parent come from the constant above, never from input
        let sql = format!(
            "SELECT EXISTS (SELECT 1 FROM {table} WHERE {column} IN ({}))",
            version_member_ids(parent)?
        );
        let linked: bool = conn.query_row(&sql, params![id.as_str()], |r| r.get(0))?;
        if linked {
            found.push(format!("{table}.{column}"));
        }
    }
    Ok(found)
}

/// Delete a non-canonical version and its structural tree (CASCADE inside the
/// version). Refused (`StorageError::Immutable`) for `APPROVED_CANONICAL` rows and
/// (`StorageError::ForeignKey`, RESTRICT semantics) while any session, calendar
/// event, note, learning episode, review item or assessment definition links to
/// the version's days, objectives or blocks ([`VERSION_LINKS`]): those links are
/// `ON DELETE SET NULL` in the DDL and would otherwise vanish silently (ADR-0002:
/// deleting a curriculum version with attempts is refused).
pub fn delete_version(
    conn: &Connection,
    id: &CurriculumVersionId,
    expected_revision: Revision,
) -> RepoResult<()> {
    let current = required(TABLE, get_version(conn, id)?)?;
    if current.revision != expected_revision {
        return Err(RepoError::Storage(StorageError::StaleRevision {
            expected: Some(expected_revision),
            actual: current.revision,
        }));
    }
    if current.provenance == ManifestProvenance::ApprovedCanonical {
        return Err(RepoError::Storage(StorageError::Immutable(
            "approved canonical curriculum version cannot be deleted".to_string(),
        )));
    }
    let dependents = version_dependents(conn, id)?;
    if !dependents.is_empty() {
        return Err(RepoError::Storage(StorageError::ForeignKey(format!(
            "curriculum_versions.referenced: {} still link to this version",
            dependents.join(", ")
        ))));
    }
    let changed = conn.execute(
        "DELETE FROM curriculum_versions WHERE id = ?1 AND revision = ?2 AND provenance <> 'APPROVED_CANONICAL'",
        params![id.as_str(), rev_i64(expected_revision)],
    )?;
    finish_update(conn, TABLE, "id", id.as_str(), expected_revision, changed).map(|_| ())
}

fn map_week(r: &Row<'_>) -> StorageResult<WeekRow> {
    Ok(WeekRow {
        id: r.get("id")?,
        curriculum_version_id: parse_id(&r.get::<_, String>("curriculum_version_id")?)?,
        week_number: u32_from_i64(r.get("week_number")?, "week_number")?,
        title: r.get("title")?,
        theme: r.get("theme")?,
        planned_minutes: u32_from_i64(r.get("planned_minutes")?, "planned_minutes")?,
        created_at: from_ms(r.get("created_at")?),
    })
}

/// Weeks of a version in order.
pub fn list_weeks(conn: &Connection, version: &CurriculumVersionId) -> RepoResult<Vec<WeekRow>> {
    query_all(
        conn,
        "SELECT id, curriculum_version_id, week_number, title, theme, planned_minutes, created_at
         FROM weeks WHERE curriculum_version_id = ?1 ORDER BY week_number",
        &[&version.as_str()],
        map_week,
    )
}

const DAY_COLUMNS: &str =
    "id, week_id, curriculum_version_id, day_code, day_number, day_of_week, planned_minutes, title, created_at";

fn map_day(r: &Row<'_>) -> StorageResult<TrainingDayRow> {
    let dow = u8_from_i64(r.get("day_of_week")?, "day_of_week")?;
    Ok(TrainingDayRow {
        id: r.get("id")?,
        week_id: r.get("week_id")?,
        curriculum_version_id: parse_id(&r.get::<_, String>("curriculum_version_id")?)?,
        day_code: parse_id(&r.get::<_, String>("day_code")?)?,
        day_number: u32_from_i64(r.get("day_number")?, "day_number")?,
        day_of_week: DayOfWeek::new(dow)
            .map_err(|e| StorageError::RowDecode(format!("day_of_week: {e}")))?,
        planned_minutes: u32_from_i64(r.get("planned_minutes")?, "planned_minutes")?,
        title: r.get("title")?,
        created_at: from_ms(r.get("created_at")?),
    })
}

/// Training days of a version in program order.
pub fn list_training_days(
    conn: &Connection,
    version: &CurriculumVersionId,
) -> RepoResult<Vec<TrainingDayRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {DAY_COLUMNS} FROM training_days WHERE curriculum_version_id = ?1 ORDER BY day_number"
        ),
        &[&version.as_str()],
        map_day,
    )
}

/// One training day by manifest day id.
pub fn training_day(
    conn: &Connection,
    version: &CurriculumVersionId,
    day: &DayId,
) -> RepoResult<Option<TrainingDayRow>> {
    query_opt(
        conn,
        &format!(
            "SELECT {DAY_COLUMNS} FROM training_days WHERE curriculum_version_id = ?1 AND day_code = ?2"
        ),
        &[&version.as_str(), &day.as_str()],
        map_day,
    )
}

const OBJECTIVE_COLUMNS: &str =
    "id, training_day_id, curriculum_version_id, objective_code, ordinal, statement,
    success_criteria, dimension, created_at";

fn map_objective(r: &Row<'_>) -> StorageResult<DailyObjectiveRow> {
    Ok(DailyObjectiveRow {
        id: r.get("id")?,
        training_day_id: r.get("training_day_id")?,
        curriculum_version_id: parse_id(&r.get::<_, String>("curriculum_version_id")?)?,
        objective_code: parse_id(&r.get::<_, String>("objective_code")?)?,
        ordinal: u8_from_i64(r.get("ordinal")?, "ordinal")?,
        statement: r.get("statement")?,
        success_criteria: r.get("success_criteria")?,
        dimension: r
            .get::<_, Option<String>>("dimension")?
            .as_deref()
            .map(enum_from_text)
            .transpose()?,
        created_at: from_ms(r.get("created_at")?),
    })
}

/// Objectives of one day addressed by `(version, week_number, day_of_week)`, in
/// ordinal order. Empty when the day does not exist.
pub fn daily_objectives(
    conn: &Connection,
    version: &CurriculumVersionId,
    week_number: u32,
    day_of_week: DayOfWeek,
) -> RepoResult<Vec<DailyObjectiveRow>> {
    query_all(
        conn,
        "SELECT o.id, o.training_day_id, o.curriculum_version_id, o.objective_code, o.ordinal, o.statement,
                o.success_criteria, o.dimension, o.created_at
         FROM daily_objectives o
         JOIN training_days d ON d.id = o.training_day_id
         JOIN weeks w ON w.id = d.week_id
         WHERE o.curriculum_version_id = ?1 AND w.week_number = ?2 AND d.day_of_week = ?3
         ORDER BY o.ordinal",
        &[
            &version.as_str(),
            &i64::from(week_number),
            &i64::from(day_of_week.get()),
        ],
        map_objective,
    )
}

/// Objectives of a training-day row.
pub fn objectives_for_day(
    conn: &Connection,
    training_day_id: &str,
) -> RepoResult<Vec<DailyObjectiveRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {OBJECTIVE_COLUMNS} FROM daily_objectives WHERE training_day_id = ?1 ORDER BY ordinal"
        ),
        &[&training_day_id],
        map_objective,
    )
}

/// One objective by manifest code.
pub fn objective_by_code(
    conn: &Connection,
    version: &CurriculumVersionId,
    code: &ObjectiveId,
) -> RepoResult<Option<DailyObjectiveRow>> {
    query_opt(
        conn,
        &format!(
            "SELECT {OBJECTIVE_COLUMNS} FROM daily_objectives WHERE curriculum_version_id = ?1 AND objective_code = ?2"
        ),
        &[&version.as_str(), &code.as_str()],
        map_objective,
    )
}

fn map_study_block(r: &Row<'_>) -> StorageResult<StudyBlockRow> {
    Ok(StudyBlockRow {
        id: r.get("id")?,
        training_day_id: r.get("training_day_id")?,
        objective_id: r.get("objective_id")?,
        ordinal: u32_from_i64(r.get("ordinal")?, "ordinal")?,
        block_code: r.get("block_code")?,
        minutes: u32_from_i64(r.get("minutes")?, "minutes")?,
        mode: enum_from_text(&r.get::<_, String>("mode")?)?,
        created_at: from_ms(r.get("created_at")?),
    })
}

/// Study blocks of a training-day row in order.
pub fn study_blocks(conn: &Connection, training_day_id: &str) -> RepoResult<Vec<StudyBlockRow>> {
    query_all(
        conn,
        "SELECT id, training_day_id, objective_id, ordinal, block_code, minutes, mode, created_at
         FROM study_blocks WHERE training_day_id = ?1 ORDER BY ordinal",
        &[&training_day_id],
        map_study_block,
    )
}

/// One study block by row id (resolves `study_sessions.study_block_id` and
/// `calendar_events.study_block_id` to the IPC `block`/`block_seq`).
pub fn get_study_block(conn: &Connection, id: &str) -> RepoResult<Option<StudyBlockRow>> {
    query_opt(
        conn,
        "SELECT id, training_day_id, objective_id, ordinal, block_code, minutes, mode, created_at
         FROM study_blocks WHERE id = ?1",
        &[&id],
        map_study_block,
    )
}

/// Resource references of a version (optionally restricted to one day through
/// the objective link).
pub fn resource_references(
    conn: &Connection,
    version: &CurriculumVersionId,
    training_day_id: Option<&str>,
) -> RepoResult<Vec<ResourceReferenceRow>> {
    let map = |r: &Row<'_>| {
        Ok(ResourceReferenceRow {
            id: r.get("id")?,
            curriculum_version_id: parse_id(&r.get::<_, String>("curriculum_version_id")?)?,
            objective_id: r.get("objective_id")?,
            title: r.get("title")?,
            source_ref: r.get("source_ref")?,
            uri: r.get("uri")?,
            trust: enum_from_text(&r.get::<_, String>("trust")?)?,
            created_at: from_ms(r.get("created_at")?),
        })
    };
    match training_day_id {
        None => query_all(
            conn,
            "SELECT id, curriculum_version_id, objective_id, title, source_ref, uri, trust, created_at
             FROM resource_references WHERE curriculum_version_id = ?1 ORDER BY id",
            &[&version.as_str()],
            map,
        ),
        Some(day) => query_all(
            conn,
            "SELECT r.id, r.curriculum_version_id, r.objective_id, r.title, r.source_ref, r.uri, r.trust, r.created_at
             FROM resource_references r
             JOIN daily_objectives o ON o.id = r.objective_id
             WHERE r.curriculum_version_id = ?1 AND o.training_day_id = ?2 ORDER BY r.id",
            &[&version.as_str(), &day],
            map,
        ),
    }
}

/// Deliverable definitions of a training-day row.
pub fn deliverables(
    conn: &Connection,
    training_day_id: &str,
) -> RepoResult<Vec<DeliverableDefinitionRow>> {
    query_all(
        conn,
        "SELECT id, training_day_id, objective_id, code, title, evidence_kind, is_required, created_at
         FROM deliverable_definitions WHERE training_day_id = ?1 ORDER BY code",
        &[&training_day_id],
        |r| {
            Ok(DeliverableDefinitionRow {
                id: r.get("id")?,
                training_day_id: r.get("training_day_id")?,
                objective_id: r.get("objective_id")?,
                code: r.get("code")?,
                title: r.get("title")?,
                evidence_kind: r.get("evidence_kind")?,
                is_required: i64_bool(r.get("is_required")?)?,
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

/// How many objectives a version holds.
pub fn count_objectives(conn: &Connection, version: &CurriculumVersionId) -> RepoResult<u32> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM daily_objectives WHERE curriculum_version_id = ?1",
        params![version.as_str()],
        |r| r.get(0),
    )?;
    Ok(u32::try_from(n).unwrap_or(u32::MAX))
}

/// The title of a curriculum (`curricula.title`), if it exists.
pub fn curriculum_title(conn: &Connection, id: &CurriculumId) -> RepoResult<Option<String>> {
    query_opt(
        conn,
        "SELECT title FROM curricula WHERE id = ?1",
        &[&id.as_str()],
        |r| Ok(r.get::<_, String>("title")?),
    )
}

// ---------------------------------------------------------------------------
// The program plan (migration 0015): the Monday of week 1 and the time of day a
// planned day's first block starts.
// ---------------------------------------------------------------------------

/// Where the program sits on the calendar. The curriculum names weeks, days and
/// blocks but no dates; the operator anchors it here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramPlanRow {
    /// The curriculum the plan belongs to.
    pub curriculum_id: CurriculumId,
    /// `YYYY-MM-DD`: the Monday of week 1 (the caller checks the weekday; storage keeps the format).
    pub start_monday: String,
    /// Minutes after local midnight at which a planned day's first block starts.
    pub day_start_minute: u32,
    /// Optimistic concurrency revision.
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

/// The plan of `curriculum_id`, if the operator set one.
pub fn get_program_plan(
    conn: &Connection,
    curriculum_id: &CurriculumId,
) -> RepoResult<Option<ProgramPlanRow>> {
    query_opt(
        conn,
        "SELECT curriculum_id, start_monday, day_start_minute, revision, created_at, updated_at
           FROM program_plan WHERE curriculum_id = ?1",
        &[&curriculum_id.as_str()],
        |r| {
            Ok(ProgramPlanRow {
                curriculum_id: parse_id(&r.get::<_, String>("curriculum_id")?)?,
                start_monday: r.get("start_monday")?,
                day_start_minute: u32::try_from(r.get::<_, i64>("day_start_minute")?)
                    .map_err(|_| StorageError::Internal("program_plan.day_start_minute"))?,
                revision: rev_from_i64(r.get("revision")?),
                created_at: from_ms(r.get("created_at")?),
                updated_at: from_ms(r.get("updated_at")?),
            })
        },
    )
}

/// Set the plan of `curriculum_id`: insert it, or replace it under a revision check
/// (`expected_revision` of `None` on an existing plan checks nothing, like every
/// other update the interface sends without a revision). Returns the new revision.
pub fn put_program_plan(
    conn: &Connection,
    curriculum_id: &CurriculumId,
    start_monday: &str,
    day_start_minute: u32,
    expected_revision: Option<Revision>,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let bytes = start_monday.as_bytes();
    let dated = bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        });
    if !dated || day_start_minute > 1439 {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "program plan: a date as YYYY-MM-DD and a start minute from 0 to 1439".to_string(),
        )));
    }
    match (get_program_plan(conn, curriculum_id)?, expected_revision) {
        (None, None) => {
            conn.execute(
                "INSERT INTO program_plan (curriculum_id, start_monday, day_start_minute, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![curriculum_id.as_str(), start_monday, i64::from(day_start_minute), ms(now)],
            )?;
            Ok(Revision::INITIAL)
        }
        (None, Some(_)) => Err(RepoError::Storage(StorageError::NotFound("program_plan"))),
        (Some(current), expected) => {
            let expected = expected.unwrap_or(current.revision);
            let changed = conn.execute(
                "UPDATE program_plan
                    SET start_monday = ?3, day_start_minute = ?4, revision = revision + 1, updated_at = ?5
                  WHERE curriculum_id = ?1 AND revision = ?2",
                params![
                    curriculum_id.as_str(),
                    rev_i64(expected),
                    start_monday,
                    i64::from(day_start_minute),
                    ms(now)
                ],
            )?;
            finish_update(
                conn,
                "program_plan",
                "curriculum_id",
                curriculum_id.as_str(),
                expected,
                changed,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::testutil::{approval, assert_immutable, count, db, operator_id, t};
    use g9_curriculum::{
        manifest_checksum, Block, BlockSpec, Day, Objective, Resource, Structure, Week,
    };
    use g9_domain::CurriculumCounts;

    const SEED: &str = include_str!("../../../../data/curriculum/g9-ct9-480.seed.v0.json");

    fn tiny_manifest() -> Manifest {
        Manifest {
            manifest_schema_version: "1.0.0".into(),
            curriculum_id: "tiny-cur".into(),
            name: "Tiny".into(),
            version: "0.1.0".into(),
            provenance: ManifestProvenance::ProposedSeed,
            provenance_note: None,
            structure: Structure {
                weeks: 1,
                days_per_week: 6,
                objectives_per_day: 3,
                weekday_minutes: 420,
                saturday_minutes: 300,
                weekly_minutes: 2400,
                total_minutes: 2400,
                sunday: "rest".into(),
            },
            weekday_blocks: vec![BlockSpec {
                block: "recall".into(),
                minutes: 15,
            }],
            saturday_blocks: vec![BlockSpec {
                block: "blind_practical".into(),
                minutes: 120,
            }],
            weeks: vec![Week {
                week_number: 1,
                title: "Week one".into(),
                days: vec![
                    Day {
                        day_id: "D01".into(),
                        day_of_week: 1,
                        planned_minutes: 420,
                        objectives: vec![
                            Objective {
                                seq: 1,
                                objective_id: "D01-O1".into(),
                                text: "Explain boot".into(),
                                competency_ids: vec![],
                            },
                            Objective {
                                seq: 2,
                                objective_id: "D01-O2".into(),
                                text: "Map isolation".into(),
                                competency_ids: vec![],
                            },
                        ],
                        evidence_artifact: "Lab design".into(),
                        blocks: vec![
                            Block {
                                seq: 1,
                                block: "recall".into(),
                                minutes: 15,
                            },
                            Block {
                                seq: 2,
                                block: "guided_lab".into(),
                                minutes: 120,
                            },
                        ],
                        is_assessment_day: false,
                        resources: vec![Resource {
                            reference: "https://example.org/linux-boot".into(),
                            kind: g9_curriculum::ResourceKind::External,
                            license_note: None,
                            version: Some("1".into()),
                        }],
                    },
                    Day {
                        day_id: "D06".into(),
                        day_of_week: 6,
                        planned_minutes: 300,
                        objectives: vec![Objective {
                            seq: 1,
                            objective_id: "D06-O1".into(),
                            text: "Pass week 1 practical".into(),
                            competency_ids: vec![],
                        }],
                        evidence_artifact: "Week 1 assessment".into(),
                        blocks: vec![Block {
                            seq: 1,
                            block: "blind_practical".into(),
                            minutes: 120,
                        }],
                        is_assessment_day: true,
                        resources: vec![],
                    },
                ],
            }],
            reconciliation: None,
        }
    }

    fn ok_report() -> ValidationReport {
        ValidationReport {
            counts: CurriculumCounts::default(),
            violations: vec![],
            errors: vec![],
            warnings: vec![],
        }
    }

    fn insert_tiny(conn: &Connection, id: &str, manifest: &Manifest) -> CurriculumVersionId {
        let checksum = manifest_checksum(manifest).unwrap();
        insert_version(
            conn,
            &NewCurriculumVersion {
                id: CurriculumVersionId::parse(id).unwrap(),
                manifest,
                validation: &ok_report(),
                provenance: manifest.provenance,
                checksum: &checksum,
                source_ref: "tests/tiny".into(),
                imported_by: operator_id(),
                imported_at: t(0),
            },
        )
        .unwrap()
    }

    #[test]
    fn roundtrip_version_tree_and_daily_objective_lookup() {
        let db = db();
        let conn = db.conn();
        let manifest = tiny_manifest();
        let id = insert_tiny(conn, "cv-tiny", &manifest);

        let row = get_version(conn, &id).unwrap().unwrap();
        assert_eq!(row.provenance, ManifestProvenance::ProposedSeed);
        assert_eq!(row.version_label, "0.1.0");
        assert_eq!(row.revision, Revision::INITIAL);
        assert!(!row.is_current);
        assert_eq!(
            row.manifest_sha256,
            manifest_checksum(&manifest).unwrap().as_str()
        );
        let cur = CurriculumId::parse("tiny-cur").unwrap();
        assert_eq!(list_versions(conn, &cur).unwrap().len(), 1);
        assert!(current_version(conn, &cur).unwrap().is_none());

        assert_eq!(count(conn, "weeks"), 1);
        assert_eq!(count(conn, "training_days"), 2);
        assert_eq!(count(conn, "daily_objectives"), 3);
        assert_eq!(count(conn, "study_blocks"), 3);
        assert_eq!(count(conn, "resource_references"), 1);
        assert_eq!(count(conn, "deliverable_definitions"), 2);

        let weeks = list_weeks(conn, &id).unwrap();
        assert_eq!(weeks[0].planned_minutes, 720);
        let days = list_training_days(conn, &id).unwrap();
        assert_eq!(days.len(), 2);
        assert_eq!(days[1].day_number, 6);
        assert_eq!(days[1].day_of_week.get(), 6);
        let d01 = training_day(conn, &id, &DayId::parse("D01").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(d01.title, "D01");

        let objectives = daily_objectives(conn, &id, 1, DayOfWeek::new(1).unwrap()).unwrap();
        assert_eq!(objectives.len(), 2);
        assert_eq!(objectives[0].objective_code.as_str(), "D01-O1");
        assert_eq!(objectives[1].statement, "Map isolation");
        assert!(daily_objectives(conn, &id, 1, DayOfWeek::new(3).unwrap())
            .unwrap()
            .is_empty());
        assert_eq!(objectives_for_day(conn, &d01.id).unwrap().len(), 2);
        assert!(
            objective_by_code(conn, &id, &ObjectiveId::parse("D06-O1").unwrap())
                .unwrap()
                .is_some()
        );

        let blocks = study_blocks(conn, &d01.id).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].mode, InteractionMode::Review);
        assert_eq!(blocks[1].mode, InteractionMode::Lab);
        assert_eq!(
            get_study_block(conn, &blocks[1].id).unwrap().as_ref(),
            Some(&blocks[1])
        );
        assert!(get_study_block(conn, "missing").unwrap().is_none());
        let d06 = &days[1];
        assert_eq!(
            study_blocks(conn, &d06.id).unwrap()[0].mode,
            InteractionMode::Exam
        );

        let resources = resource_references(conn, &id, Some(&d01.id)).unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(
            resources[0].uri.as_deref(),
            Some("https://example.org/linux-boot")
        );
        assert!(resource_references(conn, &id, Some(&d06.id))
            .unwrap()
            .is_empty());
        assert_eq!(resource_references(conn, &id, None).unwrap().len(), 1);

        let deliverables_d06 = deliverables(conn, &d06.id).unwrap();
        assert_eq!(deliverables_d06[0].evidence_kind, "ATTEMPT_SUBMISSION");
        assert_eq!(
            deliverables(conn, &d01.id).unwrap()[0].evidence_kind,
            "NOTE"
        );
    }

    #[test]
    fn seed_manifest_imports_full_tree() {
        let db = db();
        let conn = db.conn();
        let (manifest, report) = g9_curriculum::load_manifest(SEED).unwrap();
        let checksum = manifest_checksum(&manifest).unwrap();
        let id = insert_version(
            conn,
            &NewCurriculumVersion {
                id: CurriculumVersionId::parse("cv-seed").unwrap(),
                manifest: &manifest,
                validation: &report,
                provenance: ManifestProvenance::ProposedSeed,
                checksum: &checksum,
                source_ref: "data/curriculum/g9-ct9-480.seed.v0.json".into(),
                imported_by: operator_id(),
                imported_at: t(0),
            },
        )
        .unwrap();
        assert_eq!(count(conn, "weeks"), 12);
        assert_eq!(count(conn, "training_days"), 72);
        assert_eq!(count(conn, "daily_objectives"), 216);
        assert_eq!(count(conn, "deliverable_definitions"), 72);
        let total: i64 = conn
            .query_row("SELECT SUM(planned_minutes) FROM training_days", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(total, 28_800);
        let row = get_version(conn, &id).unwrap().unwrap();
        assert_eq!(row.invariants["counts"]["objectives"], 216);
        assert_eq!(
            daily_objectives(conn, &id, 12, DayOfWeek::new(6).unwrap())
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn invalid_report_blocks_import_and_duplicate_label_conflicts() {
        let db = db();
        let conn = db.conn();
        let manifest = tiny_manifest();
        let checksum = manifest_checksum(&manifest).unwrap();
        let mut bad = ok_report();
        bad.errors.push(g9_curriculum::ManifestError {
            code: "x".into(),
            path: "/".into(),
            message: "broken".into(),
        });
        let err = insert_version(
            conn,
            &NewCurriculumVersion {
                id: CurriculumVersionId::parse("cv-bad").unwrap(),
                manifest: &manifest,
                validation: &bad,
                provenance: ManifestProvenance::ProposedSeed,
                checksum: &checksum,
                source_ref: "x".into(),
                imported_by: operator_id(),
                imported_at: t(0),
            },
        )
        .unwrap_err();
        assert!(matches!(
            err,
            RepoError::Storage(StorageError::Precondition(_))
        ));
        assert_eq!(count(conn, "curriculum_versions"), 0);

        insert_tiny(conn, "cv-1", &manifest);
        let dup = insert_version(
            conn,
            &NewCurriculumVersion {
                id: CurriculumVersionId::parse("cv-2").unwrap(),
                manifest: &manifest,
                validation: &ok_report(),
                provenance: ManifestProvenance::ProposedSeed,
                checksum: &checksum,
                source_ref: "x".into(),
                imported_by: operator_id(),
                imported_at: t(0),
            },
        )
        .unwrap_err();
        assert!(matches!(
            dup,
            RepoError::Storage(StorageError::UniqueViolation(_))
        ));
    }

    #[test]
    fn set_provenance_touches_only_provenance_columns_and_checks_revision() {
        let db = db();
        let conn = db.conn();
        let manifest = tiny_manifest();
        let id = insert_tiny(conn, "cv-1", &manifest);
        let objectives_before = daily_objectives(conn, &id, 1, DayOfWeek::new(1).unwrap()).unwrap();

        // seed -> canonical directly is refused (C01)
        let apr = approval(conn, "apr-1");
        let approval_ref = ProvenanceApproval {
            approval_id: apr,
            approved_by: operator_id(),
            approved_at: t(10),
        };
        let err = set_provenance(
            conn,
            &id,
            Revision::INITIAL,
            ManifestProvenance::ApprovedCanonical,
            Some(&approval_ref),
            t(10),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            RepoError::Storage(StorageError::Precondition(_))
        ));

        // stale revision
        let stale = set_provenance(
            conn,
            &id,
            Revision::new(7),
            ManifestProvenance::ReconciledCandidate,
            None,
            t(10),
        )
        .unwrap_err();
        assert!(matches!(
            stale,
            RepoError::Storage(StorageError::StaleRevision { actual, .. }) if actual == Revision::INITIAL
        ));

        let r2 = set_provenance(
            conn,
            &id,
            Revision::INITIAL,
            ManifestProvenance::ReconciledCandidate,
            None,
            t(10),
        )
        .unwrap();
        assert_eq!(r2, Revision::new(2));
        // canonical without approval is refused
        assert!(matches!(
            set_provenance(
                conn,
                &id,
                r2,
                ManifestProvenance::ApprovedCanonical,
                None,
                t(20)
            ),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        let r3 = set_provenance(
            conn,
            &id,
            r2,
            ManifestProvenance::ApprovedCanonical,
            Some(&approval_ref),
            t(20),
        )
        .unwrap();
        assert_eq!(r3, Revision::new(3));
        let row = get_version(conn, &id).unwrap().unwrap();
        assert_eq!(row.provenance, ManifestProvenance::ApprovedCanonical);
        assert!(row.is_current);
        assert_eq!(row.approved_by, Some(operator_id()));
        assert_eq!(row.approval_id.as_ref().map(|a| a.as_str()), Some("apr-1"));
        assert_eq!(row.updated_at, t(20));
        assert_eq!(
            current_version(conn, &CurriculumId::parse("tiny-cur").unwrap())
                .unwrap()
                .map(|v| v.id),
            Some(id.clone())
        );
        // objectives are untouched
        let objectives_after = daily_objectives(conn, &id, 1, DayOfWeek::new(1).unwrap()).unwrap();
        assert_eq!(objectives_before, objectives_after);

        // canonical is immutable: no further provenance change, no delete
        assert!(matches!(
            set_provenance(
                conn,
                &id,
                r3,
                ManifestProvenance::ApprovedCanonical,
                Some(&approval_ref),
                t(30)
            ),
            Err(RepoError::Storage(StorageError::Immutable(_)))
        ));
        assert!(matches!(
            delete_version(conn, &id, r3),
            Err(RepoError::Storage(StorageError::Immutable(_)))
        ));

        // a second version becoming canonical takes over is_current
        let mut m2 = manifest.clone();
        m2.version = "0.2.0".into();
        let id2 = insert_tiny(conn, "cv-2", &m2);
        set_provenance(
            conn,
            &id2,
            Revision::INITIAL,
            ManifestProvenance::ReconciledCandidate,
            None,
            t(40),
        )
        .unwrap();
        let apr2 = approval(conn, "apr-2");
        set_provenance(
            conn,
            &id2,
            Revision::new(2),
            ManifestProvenance::ApprovedCanonical,
            Some(&ProvenanceApproval {
                approval_id: apr2,
                approved_by: operator_id(),
                approved_at: t(50),
            }),
            t(50),
        )
        .unwrap();
        assert!(!get_version(conn, &id).unwrap().unwrap().is_current);
        assert!(get_version(conn, &id2).unwrap().unwrap().is_current);

        // the SQL layer enforces the same rules (0010 triggers), whatever the caller
        let cv = id.as_str();
        assert_immutable(
            conn,
            &format!(
                "UPDATE curriculum_versions SET provenance = 'PROPOSED_SEED' WHERE id = '{cv}'"
            ),
        );
        assert_immutable(
            conn,
            &format!("UPDATE curriculum_versions SET approved_by = 'someone' WHERE id = '{cv}'"),
        );
        assert_immutable(
            conn,
            &format!(
                "UPDATE curriculum_versions SET manifest_sha256 = '{}' WHERE id = '{cv}'",
                "1".repeat(64)
            ),
        );
        assert_immutable(
            conn,
            &format!("DELETE FROM curriculum_versions WHERE id = '{cv}'"),
        );
        assert_immutable(conn, &format!("UPDATE daily_objectives SET statement = 'rewritten' WHERE curriculum_version_id = '{cv}'"));
        assert_immutable(conn, &format!("UPDATE study_blocks SET minutes = 1 WHERE training_day_id IN (SELECT id FROM training_days WHERE curriculum_version_id = '{cv}')"));
        assert_immutable(
            conn,
            &format!("UPDATE training_days SET title = 'x' WHERE curriculum_version_id = '{cv}'"),
        );
        assert_immutable(
            conn,
            &format!("UPDATE weeks SET title = 'x' WHERE curriculum_version_id = '{cv}'"),
        );
        assert_eq!(
            daily_objectives(conn, &id, 1, DayOfWeek::new(1).unwrap()).unwrap(),
            objectives_before
        );
    }

    #[test]
    fn delete_non_canonical_version_cascades_inside_the_version() {
        let db = db();
        let conn = db.conn();
        let manifest = tiny_manifest();
        let id = insert_tiny(conn, "cv-1", &manifest);
        let mut m2 = manifest.clone();
        m2.version = "0.2.0".into();
        let _id2 = insert_tiny(conn, "cv-2", &m2);
        assert_eq!(count(conn, "daily_objectives"), 6);
        assert!(matches!(
            delete_version(conn, &id, Revision::new(9)),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        assert!(version_dependents(conn, &id).unwrap().is_empty());
        delete_version(conn, &id, Revision::INITIAL).unwrap();
        assert_eq!(count(conn, "curriculum_versions"), 1);
        assert_eq!(count(conn, "daily_objectives"), 3);
        assert_eq!(count(conn, "study_blocks"), 3);
        assert_eq!(count(conn, "training_days"), 2);
        assert!(get_version(conn, &id).unwrap().is_none());
    }

    #[test]
    fn version_links_cover_every_outside_reference() {
        let db = db();
        let conn = db.conn();
        // the version's structural tree (CASCADE children, deleted with the version)
        let tree = [
            "curriculum_versions",
            "weeks",
            "training_days",
            "daily_objectives",
            "study_blocks",
            "resource_references",
            "deliverable_definitions",
            "objective_competencies",
        ];
        let mut found = std::collections::BTreeSet::new();
        for table in crate::migrations::app_tables(conn).unwrap() {
            if tree.contains(&table.as_str()) {
                continue;
            }
            let mut stmt = conn
                .prepare(&format!("PRAGMA foreign_key_list(\"{table}\")"))
                .unwrap();
            let fks: Vec<(String, String)> = stmt
                .query_map([], |r| Ok((r.get(2)?, r.get(3)?)))
                .unwrap()
                .map(|r| r.unwrap())
                .collect();
            for (parent, column) in fks {
                if tree.contains(&parent.as_str()) {
                    found.insert((table.clone(), column, parent));
                }
            }
        }
        let listed: std::collections::BTreeSet<(String, String, String)> = VERSION_LINKS
            .iter()
            .map(|(t, c, p)| ((*t).to_string(), (*c).to_string(), (*p).to_string()))
            .collect();
        assert_eq!(found, listed);
        for (_, _, parent) in VERSION_LINKS {
            assert!(version_member_ids(parent).is_ok(), "{parent}");
        }
    }

    #[test]
    fn delete_version_is_refused_while_outside_rows_link_to_it() {
        let db = db();
        let conn = db.conn();
        let manifest = tiny_manifest();
        let id = insert_tiny(conn, "cv-1", &manifest);
        let objective: String = conn
            .query_row(
                "SELECT id FROM daily_objectives WHERE curriculum_version_id = 'cv-1' ORDER BY id LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let body = "linked note";
        conn.execute(
            "INSERT INTO notes (id, title, body_markdown, body_sha256, kind, objective_id, source_kind, trust, created_at,
                 updated_at)
             VALUES ('note-1', 'Note', ?1, ?2, 'JOURNAL', ?3, 'OPERATOR_INPUT', 'OPERATOR', 1, 1)",
            params![
                body,
                g9_domain::hashing::sha256_hex(body.as_bytes()),
                objective
            ],
        )
        .unwrap();
        assert_eq!(
            version_dependents(conn, &id).unwrap(),
            vec!["notes.objective_id".to_string()]
        );
        assert!(matches!(
            delete_version(conn, &id, Revision::INITIAL),
            Err(RepoError::Storage(StorageError::ForeignKey(m))) if m.contains("notes.objective_id")
        ));
        // nothing was deleted and the note keeps its link
        assert!(get_version(conn, &id).unwrap().is_some());
        let linked: Option<String> = conn
            .query_row(
                "SELECT objective_id FROM notes WHERE id = 'note-1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(linked, Some(objective));
    }
    #[test]
    fn a_program_plan_is_set_then_moved_under_a_revision_check() {
        let db = db();
        let conn = db.conn();
        let id: CurriculumId = parse_id("g9-ct9-480").unwrap();
        // The curriculum must exist (foreign key).
        assert!(put_program_plan(conn, &id, "2026-11-16", 540, None, t(1)).is_err());
        conn.execute(
            "INSERT INTO curricula (id, title, created_at, updated_at) VALUES ('g9-ct9-480', 'p', 1, 1)",
            [],
        )
        .unwrap();
        assert_eq!(get_program_plan(conn, &id).unwrap(), None);
        // A first set cannot carry a revision.
        assert!(matches!(
            put_program_plan(conn, &id, "2026-11-16", 540, Some(Revision::INITIAL), t(2)),
            Err(RepoError::Storage(StorageError::NotFound("program_plan")))
        ));
        assert_eq!(
            put_program_plan(conn, &id, "2026-11-16", 540, None, t(2)).unwrap(),
            Revision::INITIAL
        );
        let row = get_program_plan(conn, &id).unwrap().unwrap();
        assert_eq!(
            (
                row.start_monday.as_str(),
                row.day_start_minute,
                row.revision
            ),
            ("2026-11-16", 540, Revision::INITIAL)
        );
        // Moved without a revision (the interface does not track one), then with the right and a wrong one.
        assert_eq!(
            put_program_plan(conn, &id, "2026-11-23", 480, None, t(3)).unwrap(),
            Revision::new(2)
        );
        assert_eq!(
            put_program_plan(conn, &id, "2026-11-30", 480, Some(Revision::new(2)), t(4)).unwrap(),
            Revision::new(3)
        );
        assert!(matches!(
            put_program_plan(conn, &id, "2026-12-07", 480, Some(Revision::new(2)), t(5)),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        let row = get_program_plan(conn, &id).unwrap().unwrap();
        assert_eq!(
            (row.start_monday.as_str(), row.revision),
            ("2026-11-30", Revision::new(3))
        );
        // Only a date and a minute of the day are stored.
        for bad in [
            "2026-1-16",
            "16.11.2026",
            "2026-11-16T09:00",
            "",
            "'; DROP TABLE program_plan; --",
        ] {
            assert!(
                put_program_plan(conn, &id, bad, 540, None, t(6)).is_err(),
                "{bad}"
            );
        }
        assert!(put_program_plan(conn, &id, "2026-11-16", 1440, None, t(6)).is_err());
    }
}
