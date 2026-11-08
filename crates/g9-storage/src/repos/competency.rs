//! Competency repository (M06/M09; migrations `0003_competency` and
//! `0007_retention` for `competency_observations`).
//!
//! * `competencies`, `competency_edges`, `competency_versions`,
//!   `objective_competencies`: the graph.
//! * `competency_observations`: append-only evidence; a correction is a new row
//!   whose `supersedes` names the earlier one ([`append_observation`] runs the
//!   crate's structural validation, so model-output provenance is refused before
//!   the CHECK constraint sees it).
//! * `competency_observation_evidence` (migration `0013`): one append-only row
//!   per observation derived from a grade decision (decision, reliability
//!   class, blind, validation-eligible). [`ineligible_observation_ids`] feeds
//!   `g9_competency::derive_state_excluding_validation`, so an observation
//!   whose decision is `SELF_ADJUDICATED`, failed or not blind never counts as
//!   validation evidence (ADR-0005/0012, Addendum A.2).
//! * `learner_hypotheses`: proposals from any actor, decisions by the operator
//!   only ([`decide_hypothesis`] follows the `HypothesisStatus` edges).
//! * `competency_state_snapshots`: derived projections, never authoritative,
//!   invalidated on correction.
//!
//! Observation source mapping (DDL has `attempt_id`/`episode_id` and no source
//! column): `Attempt(id)` ↔ `attempt_id`; `Episode(id)` ↔ `episode_id`;
//! `ManualCorrection` ↔ `supersedes` set, grounding columns copied from the
//! superseded row, provenance `OPERATOR_INPUT`. To keep the mapping bijective a
//! superseding `Attempt`/`Episode` observation with `OPERATOR_INPUT` provenance is
//! refused (use `ManualCorrection`). `ReviewAttempt(..)` cannot satisfy the CHECK
//! `(attempt_id IS NOT NULL OR episode_id IS NOT NULL)` and is refused (gap).
//!
//! `competencies.rules_json` holds [`CompetencyRules`] (derivation thresholds,
//! difficulty band, NICE refs) because the DDL has no dedicated columns for the
//! last two.

use super::assessment::ReliabilityClass;
use super::{
    bool_i64, finish_update, from_json, from_ms, i64_bool, ms, opt_from_ms, opt_parse_id,
    query_all, query_opt, required, rev_from_i64, rev_i64, to_json, u32_from_i64, u8_from_i64,
    ReadScope, RepoError, RepoResult,
};
use crate::error::{StorageError, StorageResult};
use crate::sql::{enum_from_text, enum_to_text, parse_id};
use g9_competency::{
    Competency, CompetencyGraphSpec, CompetencyObservation, CompetencyProjection, DerivationRules,
    HypothesisId, HypothesisStatus, LearnerHypothesis, ObservationSource,
};
use g9_domain::{
    Actor, ActorId, AssistanceLevel, AttemptId, Classification, CompetencyId, CompetencyState,
    Confidence, DataDomain, Difficulty, EpisodeId, GradeDecisionId, InteractionMode,
    LearningDimension, ObservationId, Provenance, Revision, SourceKind, UtcMillis,
};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Contents of `competencies.rules_json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CompetencyRules {
    /// Per-competency derivation thresholds (`None` = crate defaults).
    pub derivation: Option<DerivationRules>,
    /// Inclusive `(easiest, hardest)` difficulty band.
    pub difficulty_band: Option<(u8, u8)>,
    /// External framework references (labels only).
    pub nice_refs: Vec<String>,
}

/// A `competencies` row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompetencyRow {
    /// Competency id.
    pub id: CompetencyId,
    /// Unique code.
    pub code: String,
    /// Title (the competency name).
    pub title: String,
    /// Description.
    pub description: Option<String>,
    /// Domain area / knowledge pack.
    pub domain_area: String,
    /// Validity window in days before `RetestDue`.
    pub validity_days: u32,
    /// Rules (see [`CompetencyRules`]).
    pub rules: CompetencyRules,
    /// Privacy domain (always `LEARNING` for competencies).
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

impl CompetencyRow {
    /// Build a row from a validated competency.
    pub fn from_domain(
        competency: &Competency,
        code: &str,
        derivation: Option<DerivationRules>,
        now: UtcMillis,
    ) -> Self {
        Self {
            id: competency.id.clone(),
            code: code.to_string(),
            title: competency.name.clone(),
            description: Some(competency.description.clone()),
            domain_area: competency.pack.clone(),
            validity_days: competency.validity_window_days,
            rules: CompetencyRules {
                derivation,
                difficulty_band: Some((
                    competency.difficulty_band.0.get(),
                    competency.difficulty_band.1.get(),
                )),
                nice_refs: competency.nice_refs.clone(),
            },
            data_domain: competency.domain,
            classification: Classification::Internal,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        }
    }

    /// The competency crate's node. A missing difficulty band reads as the full
    /// 1..=5 scale.
    pub fn to_domain(&self) -> StorageResult<Competency> {
        let (lo, hi) = self.rules.difficulty_band.unwrap_or((1, 5));
        let band = (
            Difficulty::new(lo).map_err(StorageError::RowDecode)?,
            Difficulty::new(hi).map_err(StorageError::RowDecode)?,
        );
        let competency = Competency {
            id: self.id.clone(),
            name: self.title.clone(),
            pack: self.domain_area.clone(),
            domain: self.data_domain,
            description: self.description.clone().unwrap_or_default(),
            validity_window_days: self.validity_days,
            difficulty_band: band,
            nice_refs: self.rules.nice_refs.clone(),
        };
        competency
            .validate()
            .map_err(|e| StorageError::RowDecode(format!("competency: {e}")))?;
        Ok(competency)
    }
}

const COMPETENCY_COLUMNS: &str = "id, code, title, description, domain_area, validity_days, rules_json, data_domain, classification,
    revision, created_at, updated_at";

fn map_competency(r: &Row<'_>) -> StorageResult<CompetencyRow> {
    Ok(CompetencyRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        code: r.get("code")?,
        title: r.get("title")?,
        description: r.get("description")?,
        domain_area: r.get("domain_area")?,
        validity_days: u32_from_i64(r.get("validity_days")?, "validity_days")?,
        rules: from_json(&r.get::<_, String>("rules_json")?, "rules_json")?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert a competency.
pub fn insert_competency(conn: &Connection, row: &CompetencyRow) -> RepoResult<Revision> {
    conn.execute(
        &format!(
            "INSERT INTO competencies ({COMPETENCY_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10, ?10)"
        ),
        params![
            row.id.as_str(),
            row.code,
            row.title,
            row.description,
            row.domain_area,
            i64::from(row.validity_days),
            to_json(&row.rules)?,
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One competency, scope-checked.
pub fn get_competency(
    conn: &Connection,
    scope: &ReadScope,
    id: &CompetencyId,
) -> RepoResult<Option<CompetencyRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {COMPETENCY_COLUMNS} FROM competencies WHERE id = ?1"),
        &[&id.as_str()],
        map_competency,
    )?;
    if let Some(r) = &row {
        scope.check("competencies", r.data_domain)?;
    }
    Ok(row)
}

/// Every competency, domain-filtered in SQL, by code.
pub fn list_competencies(conn: &Connection, scope: &ReadScope) -> RepoResult<Vec<CompetencyRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {COMPETENCY_COLUMNS} FROM competencies WHERE (?1 = 1 OR data_domain = ?2) ORDER BY code"
        ),
        &[&scope.sql_flag(), &scope.sql_domain()?],
        map_competency,
    )
}

/// Replace the mutable columns (title, description, domain area, validity, rules, classification).
pub fn update_competency(
    conn: &Connection,
    row: &CompetencyRow,
    expected_revision: Revision,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let changed = conn.execute(
        "UPDATE competencies
         SET title = ?3, description = ?4, domain_area = ?5, validity_days = ?6, rules_json = ?7, classification = ?8,
             revision = revision + 1, updated_at = ?9
         WHERE id = ?1 AND revision = ?2",
        params![
            row.id.as_str(),
            rev_i64(expected_revision),
            row.title,
            row.description,
            row.domain_area,
            i64::from(row.validity_days),
            to_json(&row.rules)?,
            enum_to_text(&row.classification)?,
            ms(now),
        ],
    )?;
    finish_update(
        conn,
        "competencies",
        "id",
        row.id.as_str(),
        expected_revision,
        changed,
    )
}

/// A `competency_versions` row (immutable definition snapshot).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompetencyVersionRow {
    /// Row id.
    pub id: String,
    /// Competency.
    pub competency_id: CompetencyId,
    /// Label (unique per competency).
    pub version_label: String,
    /// Definition document (pack JSON).
    pub definition: serde_json::Value,
    /// Creating actor.
    pub created_by: ActorId,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Insert a version snapshot (no update function exists).
pub fn insert_competency_version(conn: &Connection, row: &CompetencyVersionRow) -> RepoResult<()> {
    conn.execute(
        "INSERT INTO competency_versions (id, competency_id, version_label, definition_json, created_by, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.id,
            row.competency_id.as_str(),
            row.version_label,
            to_json(&row.definition)?,
            row.created_by.as_str(),
            ms(row.created_at)
        ],
    )?;
    Ok(())
}

/// Versions of a competency, oldest first.
pub fn list_competency_versions(
    conn: &Connection,
    competency_id: &CompetencyId,
) -> RepoResult<Vec<CompetencyVersionRow>> {
    query_all(
        conn,
        "SELECT id, competency_id, version_label, definition_json, created_by, created_at
         FROM competency_versions WHERE competency_id = ?1 ORDER BY created_at, id",
        &[&competency_id.as_str()],
        |r| {
            Ok(CompetencyVersionRow {
                id: r.get("id")?,
                competency_id: parse_id(&r.get::<_, String>("competency_id")?)?,
                version_label: r.get("version_label")?,
                definition: from_json(&r.get::<_, String>("definition_json")?, "definition_json")?,
                created_by: parse_id(&r.get::<_, String>("created_by")?)?,
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

/// Edge kind (mirrors `competency_edges.edge_kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EdgeKind {
    /// `from` must be reached before `to`.
    Prerequisite,
    /// `from` supports `to`.
    Supports,
    /// Related.
    Related,
}

/// A `competency_edges` row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompetencyEdgeRow {
    /// Row id.
    pub id: String,
    /// Source competency.
    pub from_competency_id: CompetencyId,
    /// Target competency.
    pub to_competency_id: CompetencyId,
    /// Kind.
    pub edge_kind: EdgeKind,
    /// Weight 0..=1.
    pub weight: f64,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Insert an edge (`CONFLICT` on duplicate `(from, to, kind)`).
pub fn insert_edge(conn: &Connection, row: &CompetencyEdgeRow) -> RepoResult<()> {
    conn.execute(
        "INSERT INTO competency_edges (id, from_competency_id, to_competency_id, edge_kind, weight, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.id,
            row.from_competency_id.as_str(),
            row.to_competency_id.as_str(),
            enum_to_text(&row.edge_kind)?,
            row.weight,
            ms(row.created_at)
        ],
    )?;
    Ok(())
}

/// Remove an edge explicitly (graph maintenance; edges are not evidence).
pub fn delete_edge(conn: &Connection, id: &str) -> RepoResult<()> {
    let changed = conn.execute("DELETE FROM competency_edges WHERE id = ?1", params![id])?;
    if changed == 0 {
        return Err(RepoError::Storage(StorageError::NotFound(
            "competency_edges",
        )));
    }
    Ok(())
}

fn map_edge(r: &Row<'_>) -> StorageResult<CompetencyEdgeRow> {
    Ok(CompetencyEdgeRow {
        id: r.get("id")?,
        from_competency_id: parse_id(&r.get::<_, String>("from_competency_id")?)?,
        to_competency_id: parse_id(&r.get::<_, String>("to_competency_id")?)?,
        edge_kind: enum_from_text(&r.get::<_, String>("edge_kind")?)?,
        weight: r.get("weight")?,
        created_at: from_ms(r.get("created_at")?),
    })
}

/// Every edge.
pub fn list_edges(conn: &Connection) -> RepoResult<Vec<CompetencyEdgeRow>> {
    query_all(
        conn,
        "SELECT id, from_competency_id, to_competency_id, edge_kind, weight, created_at
         FROM competency_edges ORDER BY from_competency_id, to_competency_id, edge_kind",
        &[],
        map_edge,
    )
}

/// Edges touching a competency (either end).
pub fn edges_of(
    conn: &Connection,
    competency_id: &CompetencyId,
) -> RepoResult<Vec<CompetencyEdgeRow>> {
    query_all(
        conn,
        "SELECT id, from_competency_id, to_competency_id, edge_kind, weight, created_at
         FROM competency_edges WHERE from_competency_id = ?1 OR to_competency_id = ?1
         ORDER BY from_competency_id, to_competency_id, edge_kind",
        &[&competency_id.as_str()],
        map_edge,
    )
}

/// Graph specification for `g9_competency::CompetencyGraph`: every competency in
/// scope plus the `PREREQUISITE` edges as `(prerequisite, dependent)`.
pub fn graph_spec(conn: &Connection, scope: &ReadScope) -> RepoResult<CompetencyGraphSpec> {
    let mut competencies = Vec::new();
    for row in list_competencies(conn, scope)? {
        competencies.push(row.to_domain()?);
    }
    let edges = list_edges(conn)?
        .into_iter()
        .filter(|e| e.edge_kind == EdgeKind::Prerequisite)
        .map(|e| (e.from_competency_id, e.to_competency_id))
        .collect();
    Ok(CompetencyGraphSpec {
        competencies,
        edges,
    })
}

/// An `objective_competencies` row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectiveCompetencyRow {
    /// `daily_objectives.id`.
    pub objective_id: String,
    /// Competency.
    pub competency_id: CompetencyId,
    /// Dimension the objective evidences.
    pub dimension: LearningDimension,
    /// Weight (0, 1].
    pub weight: f64,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Map an objective row to a competency dimension.
pub fn link_objective_competency(
    conn: &Connection,
    row: &ObjectiveCompetencyRow,
) -> RepoResult<()> {
    conn.execute(
        "INSERT INTO objective_competencies (objective_id, competency_id, dimension, weight, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            row.objective_id,
            row.competency_id.as_str(),
            enum_to_text(&row.dimension)?,
            row.weight,
            ms(row.created_at)
        ],
    )?;
    Ok(())
}

fn map_objective_competency(r: &Row<'_>) -> StorageResult<ObjectiveCompetencyRow> {
    Ok(ObjectiveCompetencyRow {
        objective_id: r.get("objective_id")?,
        competency_id: parse_id(&r.get::<_, String>("competency_id")?)?,
        dimension: enum_from_text(&r.get::<_, String>("dimension")?)?,
        weight: r.get("weight")?,
        created_at: from_ms(r.get("created_at")?),
    })
}

/// Competency links of an objective row.
pub fn competencies_for_objective(
    conn: &Connection,
    objective_id: &str,
) -> RepoResult<Vec<ObjectiveCompetencyRow>> {
    query_all(
        conn,
        "SELECT objective_id, competency_id, dimension, weight, created_at FROM objective_competencies
         WHERE objective_id = ?1 ORDER BY competency_id, dimension",
        &[&objective_id],
        map_objective_competency,
    )
}

/// Objective links of a competency.
pub fn objectives_for_competency(
    conn: &Connection,
    competency_id: &CompetencyId,
) -> RepoResult<Vec<ObjectiveCompetencyRow>> {
    query_all(
        conn,
        "SELECT objective_id, competency_id, dimension, weight, created_at FROM objective_competencies
         WHERE competency_id = ?1 ORDER BY objective_id, dimension",
        &[&competency_id.as_str()],
        map_objective_competency,
    )
}

// ---------------------------------------------------------------------------
// observations
// ---------------------------------------------------------------------------

const OBSERVATION_COLUMNS: &str = "id, competency_id, dimension, attempt_id, episode_id, difficulty, assistance, score, environment,
    mode, reviewer_id, provenance_json, trust, uncertainty, observed_at, supersedes, created_at";

fn map_observation(r: &Row<'_>) -> StorageResult<CompetencyObservation> {
    let attempt_id: Option<AttemptId> = opt_parse_id(r.get("attempt_id")?)?;
    let episode_id: Option<EpisodeId> = opt_parse_id(r.get("episode_id")?)?;
    let supersedes: Option<ObservationId> = opt_parse_id(r.get("supersedes")?)?;
    let provenance: Provenance =
        from_json(&r.get::<_, String>("provenance_json")?, "provenance_json")?;
    let source = match (
        &supersedes,
        provenance.source_kind,
        &attempt_id,
        &episode_id,
    ) {
        (Some(_), SourceKind::OperatorInput, _, _) => ObservationSource::ManualCorrection,
        (_, _, Some(a), _) => ObservationSource::Attempt(a.clone()),
        (_, _, None, Some(e)) => ObservationSource::Episode(e.clone()),
        (_, _, None, None) => {
            return Err(StorageError::RowDecode(
                "observation without attempt or episode".to_string(),
            ))
        }
    };
    let assistance: AssistanceLevel = enum_from_text(&r.get::<_, String>("assistance")?)?;
    let mode: InteractionMode = enum_from_text(&r.get::<_, String>("mode")?)?;
    let dimension: LearningDimension = enum_from_text(&r.get::<_, String>("dimension")?)?;
    Ok(CompetencyObservation {
        id: parse_id(&r.get::<_, String>("id")?)?,
        competency_id: parse_id(&r.get::<_, String>("competency_id")?)?,
        dimension,
        source,
        mode,
        difficulty: Difficulty::new(u8_from_i64(r.get("difficulty")?, "difficulty")?)
            .map_err(StorageError::RowDecode)?,
        assistance,
        score: u8_from_i64(r.get("score")?, "score")?,
        environment: r.get("environment")?,
        reviewer: opt_parse_id(r.get("reviewer_id")?)?,
        provenance,
        uncertainty: Confidence::new(r.get("uncertainty")?).map_err(StorageError::RowDecode)?,
        observed_at: from_ms(r.get("observed_at")?),
        supersedes,
    })
}

/// Append an observation. Structural validation (`CompetencyObservation::validate`:
/// score range, authoritative provenance, non-empty environment, correction
/// names its target) runs first; the source is mapped as described in the
/// module docs. Rows are never updated or deleted (trigger-enforced).
pub fn append_observation(conn: &Connection, obs: &CompetencyObservation) -> RepoResult<()> {
    obs.validate()
        .map_err(|e| StorageError::InvalidInput(format!("observation: {e}")))?;
    let (attempt_id, episode_id): (Option<String>, Option<String>) = match &obs.source {
        ObservationSource::Attempt(a) => {
            if obs.supersedes.is_some() && obs.provenance.source_kind == SourceKind::OperatorInput {
                return Err(RepoError::Storage(StorageError::InvalidInput(
                    "a superseding operator-input observation must use source MANUAL_CORRECTION"
                        .to_string(),
                )));
            }
            (Some(a.as_str().to_string()), None)
        }
        ObservationSource::Episode(e) => {
            if obs.supersedes.is_some() && obs.provenance.source_kind == SourceKind::OperatorInput {
                return Err(RepoError::Storage(StorageError::InvalidInput(
                    "a superseding operator-input observation must use source MANUAL_CORRECTION"
                        .to_string(),
                )));
            }
            (None, Some(e.as_str().to_string()))
        }
        ObservationSource::ReviewAttempt(_) => {
            return Err(RepoError::Storage(StorageError::NotImplemented(
                "competency.observation.review_attempt_source",
            )))
        }
        ObservationSource::ManualCorrection => {
            if obs.provenance.source_kind != SourceKind::OperatorInput {
                return Err(RepoError::Storage(StorageError::InvalidInput(
                    "a MANUAL_CORRECTION must carry OPERATOR_INPUT provenance".to_string(),
                )));
            }
            let target = obs.supersedes.as_ref().ok_or_else(|| {
                StorageError::InvalidInput("MANUAL_CORRECTION without supersedes".to_string())
            })?;
            let grounding: Option<(Option<String>, Option<String>, String)> = query_opt(
                conn,
                "SELECT attempt_id, episode_id, competency_id FROM competency_observations WHERE id = ?1",
                &[&target.as_str()],
                |r| Ok((r.get("attempt_id")?, r.get("episode_id")?, r.get("competency_id")?)),
            )?;
            let (a, e, competency) = required("competency_observations", grounding)?;
            if competency != obs.competency_id.as_str() {
                return Err(RepoError::Storage(StorageError::Precondition(
                    "competency.observation.supersedes_other_competency".to_string(),
                )));
            }
            (a, e)
        }
    };
    if let Some(target) = &obs.supersedes {
        let already: i64 = conn.query_row(
            "SELECT COUNT(*) FROM competency_observations WHERE supersedes = ?1",
            params![target.as_str()],
            |r| r.get(0),
        )?;
        if already > 0 {
            return Err(RepoError::Storage(StorageError::Conflict(
                "competency.observation.already_superseded".to_string(),
            )));
        }
    }
    conn.execute(
        &format!(
            "INSERT INTO competency_observations ({OBSERVATION_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?15)"
        ),
        params![
            obs.id.as_str(),
            obs.competency_id.as_str(),
            enum_to_text(&obs.dimension)?,
            attempt_id,
            episode_id,
            i64::from(obs.difficulty.get()),
            enum_to_text(&obs.assistance)?,
            i64::from(obs.score),
            obs.environment,
            enum_to_text(&obs.mode)?,
            obs.reviewer.as_ref().map(|a| a.as_str().to_string()),
            to_json(&obs.provenance)?,
            enum_to_text(&obs.provenance.trust)?,
            obs.uncertainty.get(),
            ms(obs.observed_at),
            obs.supersedes.as_ref().map(|s| s.as_str().to_string()),
        ],
    )?;
    Ok(())
}

/// One observation.
pub fn get_observation(
    conn: &Connection,
    id: &ObservationId,
) -> RepoResult<Option<CompetencyObservation>> {
    query_opt(
        conn,
        &format!("SELECT {OBSERVATION_COLUMNS} FROM competency_observations WHERE id = ?1"),
        &[&id.as_str()],
        map_observation,
    )
}

/// Every observation of a competency (including superseded ones), oldest first;
/// this is the replay input for `derive_state`.
pub fn list_observations(
    conn: &Connection,
    competency_id: &CompetencyId,
) -> RepoResult<Vec<CompetencyObservation>> {
    query_all(
        conn,
        &format!(
            "SELECT {OBSERVATION_COLUMNS} FROM competency_observations WHERE competency_id = ?1 ORDER BY observed_at, id"
        ),
        &[&competency_id.as_str()],
        map_observation,
    )
}

/// Observations grounded in an attempt.
pub fn observations_for_attempt(
    conn: &Connection,
    attempt_id: &AttemptId,
) -> RepoResult<Vec<CompetencyObservation>> {
    query_all(
        conn,
        &format!(
            "SELECT {OBSERVATION_COLUMNS} FROM competency_observations WHERE attempt_id = ?1 ORDER BY observed_at, id"
        ),
        &[&attempt_id.as_str()],
        map_observation,
    )
}

// ---------------------------------------------------------------------------
// observation evidence (observations derived from grade decisions)
// ---------------------------------------------------------------------------

/// A `competency_observation_evidence` row (append-only; migration `0013`):
/// what an observation derived from a grade decision rests on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservationEvidenceRow {
    /// The derived observation (primary key).
    pub observation_id: ObservationId,
    /// The grade decision the observation was derived from.
    pub decision_id: GradeDecisionId,
    /// Reliability class of that decision (ADR-0012).
    pub reliability_class: ReliabilityClass,
    /// May count as validation evidence: the decision passed, the attempt was
    /// blind and the class is not `SELF_ADJUDICATED`
    /// (`g9_assessment::ObservationDraft::validation_eligible`).
    pub validation_eligible: bool,
    /// Whether the attempt was blind (EXAM, unaided, no key or hint exposure).
    pub blind: bool,
    /// Row creation.
    pub created_at: UtcMillis,
}

const EVIDENCE_COLUMNS: &str = "e.observation_id, e.decision_id, e.reliability_class, e.validation_eligible, e.blind, e.created_at";

fn map_evidence(r: &Row<'_>) -> StorageResult<ObservationEvidenceRow> {
    Ok(ObservationEvidenceRow {
        observation_id: parse_id(&r.get::<_, String>("observation_id")?)?,
        decision_id: parse_id(&r.get::<_, String>("decision_id")?)?,
        reliability_class: enum_from_text(&r.get::<_, String>("reliability_class")?)?,
        validation_eligible: i64_bool(r.get("validation_eligible")?)?,
        blind: i64_bool(r.get("blind")?)?,
        created_at: from_ms(r.get("created_at")?),
    })
}

/// Append the evidence row of an observation derived from a grade decision.
/// A `SELF_ADJUDICATED` decision can never be validation-eligible (ADR-0005),
/// and when both rows exist the observation must be grounded in the decision's
/// attempt; a missing observation or decision is refused by the foreign keys.
/// Rows are never updated or deleted (trigger-enforced).
pub fn append_observation_evidence(
    conn: &Connection,
    row: &ObservationEvidenceRow,
) -> RepoResult<()> {
    if row.validation_eligible && row.reliability_class == ReliabilityClass::SelfAdjudicated {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "a SELF_ADJUDICATED decision can never supply validation-eligible evidence".to_string(),
        )));
    }
    let attempts: Option<(Option<String>, String)> = query_opt(
        conn,
        "SELECT o.attempt_id, d.attempt_id
           FROM competency_observations o, grade_decisions d
          WHERE o.id = ?1 AND d.id = ?2",
        &[&row.observation_id.as_str(), &row.decision_id.as_str()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if let Some((observed, decided)) = attempts {
        if observed.as_deref() != Some(decided.as_str()) {
            return Err(RepoError::Storage(StorageError::Precondition(
                "competency.observation_evidence.attempt_mismatch".to_string(),
            )));
        }
    }
    conn.execute(
        "INSERT INTO competency_observation_evidence
           (observation_id, decision_id, reliability_class, validation_eligible, blind, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.observation_id.as_str(),
            row.decision_id.as_str(),
            enum_to_text(&row.reliability_class)?,
            bool_i64(row.validation_eligible),
            bool_i64(row.blind),
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

/// Evidence rows of every observation of a competency, in the order of the
/// observations (`observed_at`, id).
pub fn observation_evidence_for_competency(
    conn: &Connection,
    competency_id: &CompetencyId,
) -> RepoResult<Vec<ObservationEvidenceRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {EVIDENCE_COLUMNS}
               FROM competency_observation_evidence e
               JOIN competency_observations o ON o.id = e.observation_id
              WHERE o.competency_id = ?1
              ORDER BY o.observed_at, o.id"
        ),
        &[&competency_id.as_str()],
        map_evidence,
    )
}

/// Ids of the competency's observations that must never count as validation
/// evidence: the `validation_ineligible` input of
/// `g9_competency::derive_state_excluding_validation`. Listed are
///
/// * every observation whose evidence row says `validation_eligible = 0`, and
/// * every observation that *has no evidence row of its own* and supersedes a
///   listed one (transitively): an operator correction (`MANUAL_CORRECTION`)
///   cannot launder a `SELF_ADJUDICATED`, failed or non-blind grade into
///   validation evidence. A superseding observation derived from a later grade
///   decision has its own evidence row and is judged by that row alone.
///
/// Observations that cannot be validation evidence anyway (STUDY or LAB mode,
/// assisted, Recall or Understanding) are not listed. An EXAM-shaped
/// observation without an evidence row is listed (fail closed) unless it is a
/// correction chained to an explicitly eligible one.
pub fn ineligible_observation_ids(
    conn: &Connection,
    competency_id: &CompetencyId,
) -> RepoResult<BTreeSet<ObservationId>> {
    // Fail closed: an observation shaped like validation evidence (EXAM mode,
    // unaided, Application or Transfer) counts as eligible only when an
    // evidence row says so, or when it is an operator correction chained to
    // one that does. Without an evidence row nothing proves its reliability
    // class or blindness, so it is listed here.
    let ids: Vec<ObservationId> = query_all(
        conn,
        "WITH RECURSIVE ineligible(id) AS (
             SELECT e.observation_id
               FROM competency_observation_evidence e
              WHERE e.validation_eligible = 0
             UNION
             SELECT o.id
               FROM competency_observations o
               JOIN ineligible i ON o.supersedes = i.id
              WHERE NOT EXISTS (SELECT 1 FROM competency_observation_evidence x
                                 WHERE x.observation_id = o.id)
         ),
         eligible_root(id) AS (
             SELECT e.observation_id
               FROM competency_observation_evidence e
              WHERE e.validation_eligible = 1
             UNION
             SELECT o.id
               FROM competency_observations o
               JOIN eligible_root r ON o.supersedes = r.id
              WHERE NOT EXISTS (SELECT 1 FROM competency_observation_evidence x
                                 WHERE x.observation_id = o.id)
         ),
         unproven(id) AS (
             SELECT o.id
               FROM competency_observations o
              WHERE o.mode = 'EXAM'
                AND o.assistance = 'UNAIDED'
                AND o.dimension IN ('APPLICATION', 'TRANSFER')
                AND NOT EXISTS (SELECT 1 FROM competency_observation_evidence x
                                 WHERE x.observation_id = o.id)
                AND o.id NOT IN (SELECT id FROM eligible_root)
         )
         SELECT i.id
           FROM (SELECT id FROM ineligible UNION SELECT id FROM unproven) i
           JOIN competency_observations o ON o.id = i.id
          WHERE o.competency_id = ?1
          ORDER BY i.id",
        &[&competency_id.as_str()],
        |r| parse_id(&r.get::<_, String>("id")?),
    )?;
    Ok(ids.into_iter().collect())
}

// ---------------------------------------------------------------------------
// learner hypotheses
// ---------------------------------------------------------------------------

/// A `learner_hypotheses` row: the domain hypothesis plus storage attributes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearnerHypothesisRow {
    /// The hypothesis (`competency_ids` holds at most one id; gap).
    pub hypothesis: LearnerHypothesis,
    /// Invalidation instant (evidence erased).
    pub invalidated_at: Option<UtcMillis>,
    /// Invalidation reason.
    pub invalidation_reason: Option<String>,
    /// Privacy domain.
    pub data_domain: DataDomain,
    /// Classification.
    pub classification: Classification,
    /// Revision.
    pub revision: Revision,
    /// Last change.
    pub updated_at: UtcMillis,
}

const HYPOTHESIS_COLUMNS: &str = "id, competency_id, statement, kind, supporting_refs, contradicting_refs, confidence_low,
    confidence_high, status, provenance_json, trust, decided_by, decided_at, invalidated_at, invalidation_reason, data_domain,
    classification, revision, created_at, updated_at";

fn map_hypothesis(r: &Row<'_>) -> StorageResult<LearnerHypothesisRow> {
    let id = HypothesisId::parse(&r.get::<_, String>("id")?)
        .map_err(|e| StorageError::RowDecode(e.to_string()))?;
    let competency: Option<CompetencyId> = opt_parse_id(r.get("competency_id")?)?;
    Ok(LearnerHypothesisRow {
        hypothesis: LearnerHypothesis {
            id,
            kind: enum_from_text(&r.get::<_, String>("kind")?)?,
            statement: r.get("statement")?,
            competency_ids: competency.into_iter().collect(),
            supporting_refs: from_json(&r.get::<_, String>("supporting_refs")?, "supporting_refs")?,
            contradicting_refs: from_json(
                &r.get::<_, String>("contradicting_refs")?,
                "contradicting_refs",
            )?,
            confidence_low: Confidence::new(r.get("confidence_low")?)
                .map_err(StorageError::RowDecode)?,
            confidence_high: Confidence::new(r.get("confidence_high")?)
                .map_err(StorageError::RowDecode)?,
            status: enum_from_text(&r.get::<_, String>("status")?)?,
            provenance: from_json(&r.get::<_, String>("provenance_json")?, "provenance_json")?,
            created_at: from_ms(r.get("created_at")?),
            decided_by: opt_parse_id(r.get("decided_by")?)?,
            decided_at: opt_from_ms(r.get("decided_at")?),
        },
        invalidated_at: opt_from_ms(r.get("invalidated_at")?),
        invalidation_reason: r.get("invalidation_reason")?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert a `PROPOSED` hypothesis (any actor may propose; decisions are commands).
/// More than one competency is refused (single `competency_id` column; gap).
pub fn insert_hypothesis(conn: &Connection, row: &LearnerHypothesisRow) -> RepoResult<Revision> {
    let h = &row.hypothesis;
    h.validate_proposal()
        .map_err(|e| StorageError::InvalidInput(format!("hypothesis: {e}")))?;
    if h.competency_ids.len() > 1 {
        return Err(RepoError::Storage(StorageError::NotImplemented(
            "competency.hypothesis.multi_competency",
        )));
    }
    conn.execute(
        &format!(
            "INSERT INTO learner_hypotheses ({HYPOTHESIS_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'PROPOSED', ?9, ?10, NULL, NULL, NULL, NULL, ?11, ?12, 1, ?13, ?13)"
        ),
        params![
            h.id.as_str(),
            h.competency_ids.first().map(|c| c.as_str().to_string()),
            h.statement,
            enum_to_text(&h.kind)?,
            to_json(&h.supporting_refs)?,
            to_json(&h.contradicting_refs)?,
            h.confidence_low.get(),
            h.confidence_high.get(),
            to_json(&h.provenance)?,
            enum_to_text(&h.provenance.trust)?,
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(h.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One hypothesis, scope-checked.
pub fn get_hypothesis(
    conn: &Connection,
    scope: &ReadScope,
    id: &HypothesisId,
) -> RepoResult<Option<LearnerHypothesisRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {HYPOTHESIS_COLUMNS} FROM learner_hypotheses WHERE id = ?1"),
        &[&id.as_str()],
        map_hypothesis,
    )?;
    if let Some(r) = &row {
        scope.check("learner_hypotheses", r.data_domain)?;
    }
    Ok(row)
}

/// Hypotheses in a status (or all), domain-filtered in SQL, oldest first.
pub fn list_hypotheses(
    conn: &Connection,
    scope: &ReadScope,
    status: Option<HypothesisStatus>,
) -> RepoResult<Vec<LearnerHypothesisRow>> {
    let status_text = status.as_ref().map(enum_to_text).transpose()?;
    query_all(
        conn,
        &format!(
            "SELECT {HYPOTHESIS_COLUMNS} FROM learner_hypotheses
             WHERE (?1 IS NULL OR status = ?1) AND (?2 = 1 OR data_domain = ?3) ORDER BY created_at, id"
        ),
        &[&status_text, &scope.sql_flag(), &scope.sql_domain()?],
        map_hypothesis,
    )
}

fn hypothesis_edge_allowed(from: HypothesisStatus, to: HypothesisStatus) -> bool {
    matches!(
        (from, to),
        (HypothesisStatus::Proposed, HypothesisStatus::Accepted)
            | (HypothesisStatus::Proposed, HypothesisStatus::Rejected)
            | (HypothesisStatus::Proposed, HypothesisStatus::Retired)
            | (HypothesisStatus::Accepted, HypothesisStatus::Rejected)
            | (HypothesisStatus::Accepted, HypothesisStatus::Retired)
    )
}

/// Operator decision on a hypothesis (`ACCEPTED`, `REJECTED`, `RETIRED`) along
/// the `HypothesisStatus` edges, under a revision check.
///
/// `decider` is the backend-assigned session actor and must be the operator:
/// model or skill output can only create `PROPOSED` rows (M09), so no other
/// actor kind is ever recorded in `decided_by`.
pub fn decide_hypothesis(
    conn: &Connection,
    id: &HypothesisId,
    expected_revision: Revision,
    status: HypothesisStatus,
    decider: &Actor,
    decided_at: UtcMillis,
) -> RepoResult<Revision> {
    if !decider.is_operator() {
        return Err(RepoError::Storage(StorageError::Precondition(
            "hypothesis decisions require the operator".to_string(),
        )));
    }
    let decided_by = &decider.id;
    let scope = ReadScope::cross_domain(DataDomain::Learning);
    let current = required("learner_hypotheses", get_hypothesis(conn, &scope, id)?)?;
    if current.revision != expected_revision {
        return Err(RepoError::Storage(StorageError::StaleRevision {
            expected: Some(expected_revision),
            actual: current.revision,
        }));
    }
    if !hypothesis_edge_allowed(current.hypothesis.status, status) {
        return Err(RepoError::Storage(StorageError::InvalidTransition(
            g9_domain::state::TransitionError {
                machine: "learner_hypothesis",
                from: format!("{:?}", current.hypothesis.status),
                event: format!("-> {status:?}"),
            },
        )));
    }
    let changed = conn.execute(
        "UPDATE learner_hypotheses SET status = ?3, decided_by = ?4, decided_at = ?5, revision = revision + 1, updated_at = ?5
         WHERE id = ?1 AND revision = ?2",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            enum_to_text(&status)?,
            decided_by.as_str(),
            ms(decided_at)
        ],
    )?;
    finish_update(
        conn,
        "learner_hypotheses",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

/// Mark a hypothesis invalid (its evidence was erased or it depends on a
/// rejected hypothesis). Rows are never deleted.
pub fn invalidate_hypothesis(
    conn: &Connection,
    id: &HypothesisId,
    expected_revision: Revision,
    reason: &str,
    now: UtcMillis,
) -> RepoResult<Revision> {
    super::require_non_empty("reason", reason)?;
    let changed = conn.execute(
        "UPDATE learner_hypotheses SET invalidated_at = ?3, invalidation_reason = ?4, revision = revision + 1, updated_at = ?3
         WHERE id = ?1 AND revision = ?2 AND invalidated_at IS NULL",
        params![id.as_str(), rev_i64(expected_revision), ms(now), reason],
    )?;
    finish_update(
        conn,
        "learner_hypotheses",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

// ---------------------------------------------------------------------------
// state snapshots
// ---------------------------------------------------------------------------

/// A `competency_state_snapshots` row (derived, never authoritative).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompetencySnapshotRow {
    /// Row id.
    pub id: String,
    /// Competency.
    pub competency_id: CompetencyId,
    /// Summary state (equals `projection.state`).
    pub state: CompetencyState,
    /// The projection.
    pub projection: CompetencyProjection,
    /// Rules identity (`DerivationRules::hash`).
    pub rules_version: String,
    /// Newest observation folded in.
    pub last_observation_id: Option<ObservationId>,
    /// When computed.
    pub computed_at: UtcMillis,
    /// Invalidation instant.
    pub invalidated_at: Option<UtcMillis>,
    /// Invalidation reason.
    pub invalidation_reason: Option<String>,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Insert a snapshot (`state` must equal `projection.state`).
pub fn insert_snapshot(conn: &Connection, row: &CompetencySnapshotRow) -> RepoResult<()> {
    if row.state != row.projection.state || row.competency_id != row.projection.competency_id {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "snapshot columns disagree with the projection".to_string(),
        )));
    }
    conn.execute(
        "INSERT INTO competency_state_snapshots (id, competency_id, state, projection_json, rules_version, last_observation_id,
            computed_at, invalidated_at, invalidation_reason, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, NULL, ?8)",
        params![
            row.id,
            row.competency_id.as_str(),
            enum_to_text(&row.state)?,
            to_json(&row.projection)?,
            row.rules_version,
            row.last_observation_id.as_ref().map(|o| o.as_str().to_string()),
            ms(row.computed_at),
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

fn map_snapshot(r: &Row<'_>) -> StorageResult<CompetencySnapshotRow> {
    Ok(CompetencySnapshotRow {
        id: r.get("id")?,
        competency_id: parse_id(&r.get::<_, String>("competency_id")?)?,
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        projection: from_json(&r.get::<_, String>("projection_json")?, "projection_json")?,
        rules_version: r.get("rules_version")?,
        last_observation_id: opt_parse_id(r.get("last_observation_id")?)?,
        computed_at: from_ms(r.get("computed_at")?),
        invalidated_at: opt_from_ms(r.get("invalidated_at")?),
        invalidation_reason: r.get("invalidation_reason")?,
        created_at: from_ms(r.get("created_at")?),
    })
}

/// Newest valid snapshot of a competency.
pub fn latest_snapshot(
    conn: &Connection,
    competency_id: &CompetencyId,
) -> RepoResult<Option<CompetencySnapshotRow>> {
    query_opt(
        conn,
        "SELECT id, competency_id, state, projection_json, rules_version, last_observation_id, computed_at, invalidated_at,
                invalidation_reason, created_at
         FROM competency_state_snapshots WHERE competency_id = ?1 AND invalidated_at IS NULL
         ORDER BY computed_at DESC, id DESC LIMIT 1",
        &[&competency_id.as_str()],
        map_snapshot,
    )
}

/// Invalidate every valid snapshot of a competency (after a correction or a
/// rules change). Returns the number of rows marked.
pub fn invalidate_snapshots(
    conn: &Connection,
    competency_id: &CompetencyId,
    reason: &str,
    now: UtcMillis,
) -> RepoResult<usize> {
    super::require_non_empty("reason", reason)?;
    Ok(conn.execute(
        "UPDATE competency_state_snapshots SET invalidated_at = ?2, invalidation_reason = ?3
         WHERE competency_id = ?1 AND invalidated_at IS NULL",
        params![competency_id.as_str(), ms(now), reason],
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::testutil::{assert_immutable, db, learning, operator, operator_id, t};
    use g9_competency::{derive_state, HypothesisKind};
    use g9_domain::EvidenceId;
    use g9_domain::TrustClass;

    fn competency(id: &str) -> Competency {
        Competency {
            id: CompetencyId::parse(id).unwrap(),
            name: format!("Competency {id}"),
            pack: "linux".into(),
            domain: DataDomain::Learning,
            description: "Boot process".into(),
            validity_window_days: 90,
            difficulty_band: (Difficulty::new(1).unwrap(), Difficulty::new(4).unwrap()),
            nice_refs: vec!["K0001".into()],
        }
    }

    fn provenance(kind: SourceKind, trust: TrustClass) -> Provenance {
        Provenance {
            source_kind: kind,
            source_ref: "attempt-1".into(),
            trust,
            content_sha256: None,
            recorded_at: t(0),
            recorded_by: operator_id(),
            version: None,
        }
    }

    fn observation(
        id: &str,
        source: ObservationSource,
        provenance: Provenance,
        at: UtcMillis,
    ) -> CompetencyObservation {
        CompetencyObservation {
            id: ObservationId::parse(id).unwrap(),
            competency_id: CompetencyId::parse("comp-1").unwrap(),
            dimension: LearningDimension::Application,
            source,
            mode: InteractionMode::Exam,
            difficulty: Difficulty::new(3).unwrap(),
            assistance: AssistanceLevel::Unaided,
            score: 90,
            environment: "lab-vm".into(),
            reviewer: Some(operator_id()),
            provenance,
            uncertainty: Confidence::new(0.1).unwrap(),
            observed_at: at,
            supersedes: None,
        }
    }

    /// competency + an attempt (through a definition/version) + a learning episode
    fn seed(conn: &Connection) {
        insert_competency(
            conn,
            &CompetencyRow::from_domain(&competency("comp-1"), "LINUX-BOOT", None, t(0)),
        )
        .unwrap();
        conn.execute_batch(
            "INSERT INTO assessment_definitions (id, code, title, kind, created_at, updated_at)
               VALUES ('ad-1', 'W1', 'W1', 'BLIND_PRACTICAL', 1, 1);
             INSERT INTO assessment_versions (id, definition_id, version_label, task_statement, task_sha256, created_by, created_at)
               VALUES ('av-1', 'ad-1', '1', 'task', '0000000000000000000000000000000000000000000000000000000000000000', 'op', 1);
             INSERT INTO attempts (id, assessment_version_id, state, mode, created_at, updated_at)
               VALUES ('attempt-1', 'av-1', 'SUBMITTED', 'EXAM', 1, 1);
             INSERT INTO learning_episodes (id, competency_id, kind, started_at, created_at, updated_at)
               VALUES ('ep-1', 'comp-1', 'GUIDED_LAB', 1, 1, 1);",
        )
        .unwrap();
    }

    #[test]
    fn competency_roundtrip_edges_graph_and_objective_links() {
        let db = db();
        let conn = db.conn();
        let c1 = competency("comp-1");
        let c2 = competency("comp-2");
        let row =
            CompetencyRow::from_domain(&c1, "LINUX-BOOT", Some(DerivationRules::default()), t(0));
        insert_competency(conn, &row).unwrap();
        insert_competency(
            conn,
            &CompetencyRow::from_domain(&c2, "LINUX-FS", None, t(0)),
        )
        .unwrap();
        let stored = get_competency(conn, &learning(), &c1.id).unwrap().unwrap();
        assert_eq!(stored, row);
        assert_eq!(stored.to_domain().unwrap(), c1);
        assert_eq!(stored.rules.derivation, Some(DerivationRules::default()));
        assert_eq!(list_competencies(conn, &learning()).unwrap().len(), 2);
        assert!(
            list_competencies(conn, &ReadScope::domain(DataDomain::Business))
                .unwrap()
                .is_empty()
        );

        let mut changed = stored.clone();
        changed.validity_days = 60;
        assert!(matches!(
            update_competency(conn, &changed, Revision::new(2), t(1)),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        assert_eq!(
            update_competency(conn, &changed, Revision::INITIAL, t(1)).unwrap(),
            Revision::new(2)
        );
        assert_eq!(
            get_competency(conn, &learning(), &c1.id)
                .unwrap()
                .unwrap()
                .validity_days,
            60
        );

        insert_competency_version(
            conn,
            &CompetencyVersionRow {
                id: "cvv-1".into(),
                competency_id: c1.id.clone(),
                version_label: "1.0".into(),
                definition: serde_json::json!({ "name": "Competency comp-1" }),
                created_by: operator_id(),
                created_at: t(0),
            },
        )
        .unwrap();
        assert_eq!(list_competency_versions(conn, &c1.id).unwrap().len(), 1);
        assert_immutable(conn, "UPDATE competency_versions SET version_label = 'x'");
        assert_immutable(conn, "DELETE FROM competency_versions");

        let edge = CompetencyEdgeRow {
            id: "edge-1".into(),
            from_competency_id: c1.id.clone(),
            to_competency_id: c2.id.clone(),
            edge_kind: EdgeKind::Prerequisite,
            weight: 1.0,
            created_at: t(0),
        };
        insert_edge(conn, &edge).unwrap();
        insert_edge(
            conn,
            &CompetencyEdgeRow {
                id: "edge-2".into(),
                edge_kind: EdgeKind::Related,
                ..edge.clone()
            },
        )
        .unwrap();
        assert!(matches!(
            insert_edge(
                conn,
                &CompetencyEdgeRow {
                    id: "edge-3".into(),
                    ..edge.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::UniqueViolation(_)))
        ));
        assert_eq!(edges_of(conn, &c2.id).unwrap().len(), 2);
        let spec = graph_spec(conn, &learning()).unwrap();
        assert_eq!(spec.competencies.len(), 2);
        assert_eq!(spec.edges, vec![(c1.id.clone(), c2.id.clone())]);
        let mut graph = g9_competency::CompetencyGraph::new();
        for c in &spec.competencies {
            graph.add_node(c.clone()).unwrap();
        }
        for (p, d) in &spec.edges {
            graph.add_edge(p, d).unwrap();
        }
        assert_eq!(
            graph.topological_order(),
            vec![c1.id.clone(), c2.id.clone()]
        );
        delete_edge(conn, "edge-2").unwrap();
        assert!(matches!(
            delete_edge(conn, "edge-2"),
            Err(RepoError::Storage(StorageError::NotFound(_)))
        ));
        assert_eq!(list_edges(conn).unwrap().len(), 1);

        conn.execute_batch(
            "INSERT INTO curricula (id, title, created_at, updated_at) VALUES ('cur', 't', 1, 1);
             INSERT INTO curriculum_versions (id, curriculum_id, version_label, provenance, manifest_sha256, source_ref, invariants_json,
                 imported_by, imported_at, created_at, updated_at)
               VALUES ('cv', 'cur', 'v', 'PROPOSED_SEED', '0000000000000000000000000000000000000000000000000000000000000000', 's', '{}', 'op', 1, 1, 1);
             INSERT INTO weeks (id, curriculum_version_id, week_number, title, planned_minutes, created_at) VALUES ('w', 'cv', 1, 'w', 0, 1);
             INSERT INTO training_days (id, week_id, curriculum_version_id, day_code, day_number, day_of_week, planned_minutes, title, created_at)
               VALUES ('d', 'w', 'cv', 'D01', 1, 1, 420, 'D01', 1);
             INSERT INTO daily_objectives (id, training_day_id, curriculum_version_id, objective_code, ordinal, statement, created_at)
               VALUES ('o', 'd', 'cv', 'D01-O1', 1, 's', 1);",
        )
        .unwrap();
        let link = ObjectiveCompetencyRow {
            objective_id: "o".into(),
            competency_id: c1.id.clone(),
            dimension: LearningDimension::Understanding,
            weight: 1.0,
            created_at: t(0),
        };
        link_objective_competency(conn, &link).unwrap();
        assert_eq!(
            competencies_for_objective(conn, "o").unwrap(),
            vec![link.clone()]
        );
        assert_eq!(objectives_for_competency(conn, &c1.id).unwrap(), vec![link]);
    }

    #[test]
    fn observations_are_append_only_and_round_trip_their_source() {
        let db = db();
        let conn = db.conn();
        seed(conn);
        let comp = CompetencyId::parse("comp-1").unwrap();
        let attempt_obs = observation(
            "obs-1",
            ObservationSource::Attempt(AttemptId::parse("attempt-1").unwrap()),
            provenance(SourceKind::AssessmentSubmission, TrustClass::Operator),
            t(1),
        );
        append_observation(conn, &attempt_obs).unwrap();
        let mut episode_obs = observation(
            "obs-2",
            ObservationSource::Episode(EpisodeId::parse("ep-1").unwrap()),
            provenance(SourceKind::SystemGenerated, TrustClass::SystemDeterministic),
            t(2),
        );
        episode_obs.mode = InteractionMode::Lab;
        episode_obs.assistance = AssistanceLevel::Guided;
        episode_obs.score = 70;
        append_observation(conn, &episode_obs).unwrap();

        // model output is refused before the CHECK constraint
        let model_obs = observation(
            "obs-m",
            ObservationSource::Attempt(AttemptId::parse("attempt-1").unwrap()),
            provenance(SourceKind::ModelResponse, TrustClass::ModelOutput),
            t(3),
        );
        assert!(matches!(
            append_observation(conn, &model_obs),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        // review-attempt sources have no grounding column (gap)
        let review_obs = observation(
            "obs-r",
            ObservationSource::ReviewAttempt(g9_domain::ReviewItemId::parse("ri-1").unwrap()),
            provenance(SourceKind::SystemGenerated, TrustClass::SystemDeterministic),
            t(3),
        );
        assert!(matches!(
            append_observation(conn, &review_obs),
            Err(RepoError::Storage(StorageError::NotImplemented(_)))
        ));

        // an operator correction supersedes obs-1 and inherits its grounding
        let mut correction = observation(
            "obs-3",
            ObservationSource::ManualCorrection,
            provenance(SourceKind::OperatorInput, TrustClass::Operator),
            t(4),
        );
        correction.score = 60;
        correction.supersedes = Some(attempt_obs.id.clone());
        append_observation(conn, &correction).unwrap();
        // a second correction of the same target conflicts
        let mut again = correction.clone();
        again.id = ObservationId::parse("obs-4").unwrap();
        assert!(matches!(
            append_observation(conn, &again),
            Err(RepoError::Storage(StorageError::Conflict(_)))
        ));
        // ambiguous: superseding attempt observation with operator-input provenance
        let mut ambiguous = attempt_obs.clone();
        ambiguous.id = ObservationId::parse("obs-5").unwrap();
        ambiguous.supersedes = Some(episode_obs.id.clone());
        ambiguous.provenance = provenance(SourceKind::OperatorInput, TrustClass::Operator);
        assert!(matches!(
            append_observation(conn, &ambiguous),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));

        let listed = list_observations(conn, &comp).unwrap();
        assert_eq!(
            listed,
            vec![attempt_obs.clone(), episode_obs.clone(), correction.clone()]
        );
        assert_eq!(
            get_observation(conn, &correction.id)
                .unwrap()
                .unwrap()
                .source,
            ObservationSource::ManualCorrection
        );
        assert_eq!(
            observations_for_attempt(conn, &AttemptId::parse("attempt-1").unwrap())
                .unwrap()
                .len(),
            2
        );
        assert_immutable(
            conn,
            "UPDATE competency_observations SET score = 100 WHERE id = 'obs-1'",
        );
        assert_immutable(
            conn,
            "DELETE FROM competency_observations WHERE id = 'obs-1'",
        );

        // the stored history replays into a projection and snapshots it
        let rules = DerivationRules::default();
        let node = get_competency(conn, &learning(), &comp)
            .unwrap()
            .unwrap()
            .to_domain()
            .unwrap();
        let projection = derive_state(&node, &listed, &rules, t(10));
        let snapshot = CompetencySnapshotRow {
            id: "snap-1".into(),
            competency_id: comp.clone(),
            state: projection.state,
            projection: projection.clone(),
            rules_version: rules.hash(),
            last_observation_id: Some(correction.id.clone()),
            computed_at: t(10),
            invalidated_at: None,
            invalidation_reason: None,
            created_at: t(10),
        };
        insert_snapshot(conn, &snapshot).unwrap();
        assert_eq!(latest_snapshot(conn, &comp).unwrap().unwrap(), snapshot);
        let mut mismatch = snapshot.clone();
        mismatch.id = "snap-x".into();
        mismatch.state = CompetencyState::Validated;
        if projection.state != CompetencyState::Validated {
            assert!(matches!(
                insert_snapshot(conn, &mismatch),
                Err(RepoError::Storage(StorageError::InvalidInput(_)))
            ));
        }
        assert_eq!(
            invalidate_snapshots(conn, &comp, "correction appended", t(11)).unwrap(),
            1
        );
        assert!(latest_snapshot(conn, &comp).unwrap().is_none());
    }

    /// A final decision row for an attempt (the columns the evidence FK needs).
    fn decision(conn: &Connection, id: &str, attempt: &str) {
        conn.execute(
            "INSERT INTO grade_decisions (id, attempt_id, total_score, passed, is_final, decided_by, decided_at, created_at, updated_at)
             VALUES (?1, ?2, 85, 1, 1, 'op', 1, 1, 1)",
            params![id, attempt],
        )
        .unwrap();
    }

    fn evidence(
        observation: &str,
        decision: &str,
        class: ReliabilityClass,
        eligible: bool,
    ) -> ObservationEvidenceRow {
        ObservationEvidenceRow {
            observation_id: ObservationId::parse(observation).unwrap(),
            decision_id: GradeDecisionId::parse(decision).unwrap(),
            reliability_class: class,
            validation_eligible: eligible,
            blind: true,
            created_at: t(5),
        }
    }

    fn attempt_observation(id: &str, attempt: &str, at: UtcMillis) -> CompetencyObservation {
        observation(
            id,
            ObservationSource::Attempt(AttemptId::parse(attempt).unwrap()),
            provenance(SourceKind::AssessmentSubmission, TrustClass::Operator),
            at,
        )
    }

    #[test]
    fn observation_evidence_round_trips_and_lists_ineligible_ids() {
        let db = db();
        let conn = db.conn();
        seed(conn);
        insert_competency(
            conn,
            &CompetencyRow::from_domain(&competency("comp-2"), "LINUX-FS", None, t(0)),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO attempts (id, assessment_version_id, state, mode, created_at, updated_at)
             VALUES ('attempt-2', 'av-1', 'SUBMITTED', 'EXAM', 1, 1)",
            [],
        )
        .unwrap();
        decision(conn, "gd-1", "attempt-1");
        decision(conn, "gd-2", "attempt-2");
        let comp = CompetencyId::parse("comp-1").unwrap();
        let other = CompetencyId::parse("comp-2").unwrap();

        // nothing recorded yet
        assert!(observation_evidence_for_competency(conn, &comp)
            .unwrap()
            .is_empty());
        assert!(ineligible_observation_ids(conn, &comp).unwrap().is_empty());

        append_observation(conn, &attempt_observation("obs-1", "attempt-1", t(1))).unwrap();
        append_observation(conn, &attempt_observation("obs-2", "attempt-1", t(2))).unwrap();
        append_observation(conn, &attempt_observation("obs-3", "attempt-2", t(3))).unwrap();
        // an episode observation has no grade decision and no evidence row
        let mut lab = observation(
            "obs-lab",
            ObservationSource::Episode(EpisodeId::parse("ep-1").unwrap()),
            provenance(SourceKind::SystemGenerated, TrustClass::SystemDeterministic),
            t(0),
        );
        lab.mode = InteractionMode::Lab;
        append_observation(conn, &lab).unwrap();
        // an observation of another competency
        let mut foreign = attempt_observation("obs-x", "attempt-2", t(4));
        foreign.competency_id = other.clone();
        append_observation(conn, &foreign).unwrap();

        let eligible = evidence("obs-1", "gd-1", ReliabilityClass::ExternalReview, true);
        let self_graded = evidence("obs-2", "gd-1", ReliabilityClass::SelfAdjudicated, false);
        let failed = ObservationEvidenceRow {
            blind: false,
            ..evidence("obs-3", "gd-2", ReliabilityClass::Deterministic, false)
        };
        for row in [&eligible, &self_graded, &failed] {
            append_observation_evidence(conn, row).unwrap();
        }
        append_observation_evidence(
            conn,
            &evidence("obs-x", "gd-2", ReliabilityClass::Deterministic, true),
        )
        .unwrap();

        // round trip, in observation order, per competency
        assert_eq!(
            observation_evidence_for_competency(conn, &comp).unwrap(),
            vec![eligible.clone(), self_graded.clone(), failed.clone()]
        );
        assert_eq!(
            observation_evidence_for_competency(conn, &other)
                .unwrap()
                .len(),
            1
        );
        // only validation_eligible = 0 rows are listed as ineligible
        assert_eq!(
            ineligible_observation_ids(conn, &comp).unwrap(),
            [
                self_graded.observation_id.clone(),
                failed.observation_id.clone()
            ]
            .into_iter()
            .collect::<BTreeSet<_>>()
        );
        assert!(ineligible_observation_ids(conn, &other).unwrap().is_empty());
        // the episode observation is not listed anywhere
        assert!(observation_evidence_for_competency(conn, &comp)
            .unwrap()
            .iter()
            .all(|e| e.observation_id.as_str() != "obs-lab"));

        // the listed set is what derivation consumes: obs-1 (EXTERNAL_REVIEW)
        // validates; without it the SELF_ADJUDICATED and failed observations
        // stop at Independent and the projection is provisional
        let node = get_competency(conn, &learning(), &comp)
            .unwrap()
            .unwrap()
            .to_domain()
            .unwrap();
        let observations = list_observations(conn, &comp).unwrap();
        let rules = DerivationRules::default();
        let blocked = ineligible_observation_ids(conn, &comp).unwrap();
        let with_eligible = g9_competency::derive_state_excluding_validation(
            &node,
            &observations,
            &rules,
            t(10),
            &blocked,
        );
        assert_eq!(with_eligible.state, CompetencyState::Validated);
        assert!(!with_eligible.provisional);
        let without_eligible: Vec<_> = observations
            .iter()
            .filter(|o| o.id != eligible.observation_id)
            .cloned()
            .collect();
        let capped = g9_competency::derive_state_excluding_validation(
            &node,
            &without_eligible,
            &rules,
            t(10),
            &blocked,
        );
        assert_eq!(capped.state, CompetencyState::Independent);
        assert!(capped.provisional);
    }

    #[test]
    fn corrections_inherit_ineligibility_but_a_later_decision_is_judged_on_its_own_row() {
        let db = db();
        let conn = db.conn();
        seed(conn);
        conn.execute(
            "INSERT INTO attempts (id, assessment_version_id, state, mode, created_at, updated_at)
             VALUES ('attempt-2', 'av-1', 'SUBMITTED', 'EXAM', 1, 1)",
            [],
        )
        .unwrap();
        decision(conn, "gd-1", "attempt-1");
        decision(conn, "gd-2", "attempt-1");
        let comp = CompetencyId::parse("comp-1").unwrap();
        append_observation(conn, &attempt_observation("obs-1", "attempt-1", t(1))).unwrap();
        append_observation_evidence(
            conn,
            &evidence("obs-1", "gd-1", ReliabilityClass::SelfAdjudicated, false),
        )
        .unwrap();
        // an operator correction of the self-graded observation has no evidence
        // row of its own: it must not become validation evidence
        let mut correction = observation(
            "obs-2",
            ObservationSource::ManualCorrection,
            provenance(SourceKind::OperatorInput, TrustClass::Operator),
            t(2),
        );
        correction.score = 95;
        correction.supersedes = Some(ObservationId::parse("obs-1").unwrap());
        append_observation(conn, &correction).unwrap();
        // ... and neither may a correction of that correction
        let mut second = correction.clone();
        second.id = ObservationId::parse("obs-3").unwrap();
        second.supersedes = Some(correction.id.clone());
        second.observed_at = t(3);
        append_observation(conn, &second).unwrap();
        // an unrelated lab observation is not listed
        let mut lab = observation(
            "obs-lab",
            ObservationSource::Episode(EpisodeId::parse("ep-1").unwrap()),
            provenance(SourceKind::SystemGenerated, TrustClass::SystemDeterministic),
            t(0),
        );
        lab.mode = InteractionMode::Lab;
        append_observation(conn, &lab).unwrap();
        assert_eq!(
            ineligible_observation_ids(conn, &comp).unwrap(),
            ["obs-1", "obs-2", "obs-3"]
                .iter()
                .map(|i| ObservationId::parse(i).unwrap())
                .collect::<BTreeSet<_>>()
        );

        // an observation derived from a later (e.g. adjudicated) decision that
        // supersedes the old one carries its own evidence row and is judged by it
        let mut adjudicated = attempt_observation("obs-4", "attempt-1", t(4));
        adjudicated.supersedes = Some(second.id.clone());
        adjudicated.provenance = provenance(SourceKind::AssessmentSubmission, TrustClass::Operator);
        append_observation(conn, &adjudicated).unwrap();
        append_observation_evidence(
            conn,
            &evidence("obs-4", "gd-2", ReliabilityClass::ExternalReview, true),
        )
        .unwrap();
        let listed = ineligible_observation_ids(conn, &comp).unwrap();
        assert!(!listed.contains(&adjudicated.id));
        assert_eq!(listed.len(), 3);
    }

    #[test]
    fn an_exam_shaped_observation_without_an_evidence_row_is_ineligible_fail_closed() {
        let db = db();
        let conn = db.conn();
        seed(conn);
        conn.execute(
            "INSERT INTO attempts (id, assessment_version_id, state, mode, created_at, updated_at)
             VALUES ('attempt-2', 'av-1', 'SUBMITTED', 'EXAM', 1, 1)",
            [],
        )
        .unwrap();
        decision(conn, "gd-2", "attempt-2");
        let comp = CompetencyId::parse("comp-1").unwrap();
        let operator = || provenance(SourceKind::OperatorInput, TrustClass::Operator);
        let id = |s: &str| ObservationId::parse(s).unwrap();

        // proven eligible: an evidence row says so
        append_observation(conn, &attempt_observation("obs-ok", "attempt-2", t(2))).unwrap();
        append_observation_evidence(
            conn,
            &evidence("obs-ok", "gd-2", ReliabilityClass::ExternalReview, true),
        )
        .unwrap();
        // unproven: EXAM, unaided, Application, no evidence row (storage API only)
        append_observation(conn, &attempt_observation("obs-bare", "attempt-1", t(1))).unwrap();
        // a correction chained to the proven one inherits its eligibility ...
        let mut fix_ok = observation(
            "obs-fix-ok",
            ObservationSource::ManualCorrection,
            operator(),
            t(3),
        );
        fix_ok.supersedes = Some(id("obs-ok"));
        append_observation(conn, &fix_ok).unwrap();
        // ... a correction chained to the unproven one stays ineligible
        let mut fix_bare = observation(
            "obs-fix-bare",
            ObservationSource::ManualCorrection,
            operator(),
            t(3),
        );
        fix_bare.supersedes = Some(id("obs-bare"));
        append_observation(conn, &fix_bare).unwrap();
        // observations that can never be validation evidence are not listed:
        // LAB mode, and assisted EXAM work
        let mut lab = observation(
            "obs-lab",
            ObservationSource::Episode(EpisodeId::parse("ep-1").unwrap()),
            provenance(SourceKind::SystemGenerated, TrustClass::SystemDeterministic),
            t(0),
        );
        lab.mode = InteractionMode::Lab;
        append_observation(conn, &lab).unwrap();
        let mut hinted = lab.clone();
        hinted.id = id("obs-hinted");
        hinted.mode = InteractionMode::Exam;
        hinted.assistance = AssistanceLevel::Hinted;
        append_observation(conn, &hinted).unwrap();

        assert_eq!(
            ineligible_observation_ids(conn, &comp).unwrap(),
            [id("obs-bare"), id("obs-fix-bare")]
                .into_iter()
                .collect::<BTreeSet<_>>()
        );
    }

    #[test]
    fn observation_evidence_refuses_inconsistent_and_dangling_rows() {
        let db = db();
        let conn = db.conn();
        seed(conn);
        conn.execute(
            "INSERT INTO attempts (id, assessment_version_id, state, mode, created_at, updated_at)
             VALUES ('attempt-2', 'av-1', 'SUBMITTED', 'EXAM', 1, 1)",
            [],
        )
        .unwrap();
        decision(conn, "gd-1", "attempt-1");
        decision(conn, "gd-2", "attempt-2");
        append_observation(conn, &attempt_observation("obs-1", "attempt-1", t(1))).unwrap();

        // SELF_ADJUDICATED can never be validation-eligible (ADR-0005)
        assert!(matches!(
            append_observation_evidence(
                conn,
                &evidence("obs-1", "gd-1", ReliabilityClass::SelfAdjudicated, true)
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        // the observation must come from the decision's attempt
        assert!(matches!(
            append_observation_evidence(
                conn,
                &evidence("obs-1", "gd-2", ReliabilityClass::ExternalReview, true)
            ),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        // foreign keys: neither side may dangle
        assert!(matches!(
            append_observation_evidence(
                conn,
                &evidence("ghost", "gd-1", ReliabilityClass::ExternalReview, true)
            ),
            Err(RepoError::Storage(StorageError::ForeignKey(_)))
        ));
        assert!(matches!(
            append_observation_evidence(
                conn,
                &evidence("obs-1", "ghost", ReliabilityClass::ExternalReview, true)
            ),
            Err(RepoError::Storage(StorageError::ForeignKey(_)))
        ));
        // CHECK lists and the primary key
        for sql in [
            "INSERT INTO competency_observation_evidence VALUES ('obs-1', 'gd-1', 'GUESSED', 1, 1, 1)",
            "INSERT INTO competency_observation_evidence VALUES ('obs-1', 'gd-1', 'DETERMINISTIC', 2, 1, 1)",
            "INSERT INTO competency_observation_evidence VALUES ('obs-1', 'gd-1', 'DETERMINISTIC', 1, 2, 1)",
        ] {
            assert!(matches!(
                StorageError::from(conn.execute(sql, []).unwrap_err()),
                StorageError::CheckViolation(_)
            ));
        }
        let row = evidence("obs-1", "gd-1", ReliabilityClass::ExternalReview, true);
        append_observation_evidence(conn, &row).unwrap();
        assert!(matches!(
            append_observation_evidence(conn, &row),
            Err(RepoError::Storage(StorageError::UniqueViolation(_)))
        ));

        // append-only: no update, no delete
        assert_immutable(
            conn,
            "UPDATE competency_observation_evidence SET validation_eligible = 0 WHERE observation_id = 'obs-1'",
        );
        assert_immutable(
            conn,
            "UPDATE competency_observation_evidence SET decision_id = 'gd-2' WHERE observation_id = 'obs-1'",
        );
        assert_immutable(conn, "DELETE FROM competency_observation_evidence");

        // both parents are RESTRICT (the parents' own immutability triggers
        // refuse first; the declared actions are checked on the schema)
        let mut stmt = conn
            .prepare("PRAGMA foreign_key_list(competency_observation_evidence)")
            .unwrap();
        let mut fks: Vec<(String, String, String, String)> = stmt
            .query_map([], |r| Ok((r.get(2)?, r.get(3)?, r.get(5)?, r.get(6)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        fks.sort();
        assert_eq!(
            fks,
            vec![
                (
                    "competency_observations".to_string(),
                    "observation_id".to_string(),
                    "RESTRICT".to_string(),
                    "RESTRICT".to_string()
                ),
                (
                    "grade_decisions".to_string(),
                    "decision_id".to_string(),
                    "RESTRICT".to_string(),
                    "RESTRICT".to_string()
                ),
            ]
        );
        assert_immutable(
            conn,
            "DELETE FROM competency_observations WHERE id = 'obs-1'",
        );
        assert_immutable(conn, "DELETE FROM grade_decisions WHERE id = 'gd-1'");
    }

    #[test]
    fn hypotheses_follow_operator_decisions_and_scope() {
        let db = db();
        let conn = db.conn();
        seed(conn);
        let hypothesis = LearnerHypothesis {
            id: HypothesisId::parse("hyp-1").unwrap(),
            kind: HypothesisKind::Misconception,
            statement: "confuses initrd with initramfs".into(),
            competency_ids: vec![CompetencyId::parse("comp-1").unwrap()],
            supporting_refs: vec![EvidenceId::parse("evi-1").unwrap()],
            contradicting_refs: vec![],
            confidence_low: Confidence::new(0.2).unwrap(),
            confidence_high: Confidence::new(0.6).unwrap(),
            status: HypothesisStatus::Proposed,
            provenance: provenance(SourceKind::ModelResponse, TrustClass::ModelOutput),
            created_at: t(0),
            decided_by: None,
            decided_at: None,
        };
        let row = LearnerHypothesisRow {
            hypothesis: hypothesis.clone(),
            invalidated_at: None,
            invalidation_reason: None,
            data_domain: DataDomain::Learning,
            classification: Classification::Private,
            revision: Revision::INITIAL,
            updated_at: t(0),
        };
        insert_hypothesis(conn, &row).unwrap();
        let stored = get_hypothesis(conn, &learning(), &hypothesis.id)
            .unwrap()
            .unwrap();
        assert_eq!(stored, row);
        assert_eq!(
            list_hypotheses(conn, &learning(), Some(HypothesisStatus::Proposed))
                .unwrap()
                .len(),
            1
        );
        assert!(
            list_hypotheses(conn, &learning(), Some(HypothesisStatus::Accepted))
                .unwrap()
                .is_empty()
        );

        // a pre-decided hypothesis cannot be inserted; decisions are commands
        let mut accepted = row.clone();
        accepted.hypothesis.id = HypothesisId::parse("hyp-2").unwrap();
        accepted.hypothesis.status = HypothesisStatus::Accepted;
        assert!(matches!(
            insert_hypothesis(conn, &accepted),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        let mut multi = row.clone();
        multi.hypothesis.id = HypothesisId::parse("hyp-3").unwrap();
        multi
            .hypothesis
            .competency_ids
            .push(CompetencyId::parse("comp-2").unwrap());
        assert!(matches!(
            insert_hypothesis(conn, &multi),
            Err(RepoError::Storage(StorageError::NotImplemented(_)))
        ));

        // a model actor can never decide (only the operator accepts or rejects)
        let model = Actor::model(ActorId::parse("model-1").unwrap());
        assert!(matches!(
            decide_hypothesis(
                conn,
                &hypothesis.id,
                Revision::INITIAL,
                HypothesisStatus::Accepted,
                &model,
                t(1)
            ),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        // the DDL CHECK refuses an ACCEPTED row without a decider even through raw SQL
        let raw: StorageError = conn
            .execute(
                "UPDATE learner_hypotheses SET status = 'ACCEPTED' WHERE id = 'hyp-1'",
                [],
            )
            .expect_err("CHECK must refuse")
            .into();
        assert!(matches!(raw, StorageError::CheckViolation(_)), "{raw:?}");
        assert!(matches!(
            decide_hypothesis(
                conn,
                &hypothesis.id,
                Revision::new(5),
                HypothesisStatus::Accepted,
                &operator(),
                t(1)
            ),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        assert!(matches!(
            decide_hypothesis(
                conn,
                &hypothesis.id,
                Revision::INITIAL,
                HypothesisStatus::Proposed,
                &operator(),
                t(1)
            ),
            Err(RepoError::Storage(StorageError::InvalidTransition(_)))
        ));
        let r2 = decide_hypothesis(
            conn,
            &hypothesis.id,
            Revision::INITIAL,
            HypothesisStatus::Accepted,
            &operator(),
            t(1),
        )
        .unwrap();
        let decided = get_hypothesis(conn, &learning(), &hypothesis.id)
            .unwrap()
            .unwrap();
        assert_eq!(decided.hypothesis.status, HypothesisStatus::Accepted);
        assert_eq!(decided.hypothesis.decided_by, Some(operator_id()));
        assert_eq!(decided.hypothesis.decided_at, Some(t(1)));
        let r3 = decide_hypothesis(
            conn,
            &hypothesis.id,
            r2,
            HypothesisStatus::Rejected,
            &operator(),
            t(2),
        )
        .unwrap();
        assert!(matches!(
            decide_hypothesis(
                conn,
                &hypothesis.id,
                r3,
                HypothesisStatus::Accepted,
                &operator(),
                t(3)
            ),
            Err(RepoError::Storage(StorageError::InvalidTransition(_)))
        ));
        let r4 = invalidate_hypothesis(conn, &hypothesis.id, r3, "supporting note erased", t(4))
            .unwrap();
        let invalid = get_hypothesis(conn, &learning(), &hypothesis.id)
            .unwrap()
            .unwrap();
        assert_eq!(invalid.invalidated_at, Some(t(4)));
        assert_eq!(invalid.revision, r4);

        let mut personal = row.clone();
        personal.hypothesis.id = HypothesisId::parse("hyp-p").unwrap();
        personal.data_domain = DataDomain::Personal;
        insert_hypothesis(conn, &personal).unwrap();
        assert!(matches!(
            get_hypothesis(conn, &learning(), &personal.hypothesis.id),
            Err(RepoError::CrossDomainRefused {
                table: "learner_hypotheses",
                ..
            })
        ));
        assert_eq!(list_hypotheses(conn, &learning(), None).unwrap().len(), 1);
        assert_eq!(
            list_hypotheses(conn, &ReadScope::cross_domain(DataDomain::Learning), None)
                .unwrap()
                .len(),
            2
        );
    }
}
