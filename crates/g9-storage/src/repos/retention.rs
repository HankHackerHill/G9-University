//! Retention, episodes, interventions and experiments repository (M08/M12;
//! migration `0007_retention`, tables `review_items`, `review_attempts`,
//! `practical_transfer_checks`, `learning_episodes`, `calibration_observations`,
//! `interventions`, `intervention_outcomes`, `method_experiments`).
//!
//! * Review items carry the scheduler state (`due_at`, `stage`, `interval_days`,
//!   optional FSRS blob); `review_attempts` is the append-only recall history and
//!   [`item_state`] folds both into `g9_retention::ItemState`.
//! * Practical transfer checks are scheduled by competency validity windows and
//!   live apart from item reviews (B02).
//! * Interventions follow `g9_domain::InterventionState`; a proposal that did
//!   not originate with the operator (model, skill, imported) stays
//!   `HYPOTHESIS`/`PROPOSED` until [`review_intervention`] records an operator
//!   review that elevates `trust` to `OPERATOR` (the proposer's `origin_trust`
//!   is kept, migration `0010`); outcomes are immutable.
//! * `method_experiments.design_json` is pre-registered: it is written once and
//!   never updated (ADR-0007); only the lifecycle columns move, forward only.
//!
//! DDL ↔ wave-1 gaps: `review_items.kind` has no `CONCEPT` value; `prompt`
//! stores the evidence reference (g9-retention's rule: prompts are references,
//! never inline content); `TransferStatus::Skipped` maps to `WAIVED`.

use super::{
    bool_i64, finish_update, from_json, from_ms, i64_bool, ms, opt_from_ms, opt_ms, opt_parse_id,
    query_all, query_opt, required, rev_from_i64, rev_i64, to_json, u32_from_i64, u8_from_i64,
    ReadScope, RepoError, RepoResult,
};
use crate::error::{StorageError, StorageResult};
use crate::sql::{enum_from_text, enum_to_text, parse_id};
use g9_competency::HypothesisId;
use g9_domain::state::InterventionEvent;
use g9_domain::{
    Actor, ActorId, ApprovalId, AssistanceLevel, AttemptId, Classification, CompetencyId,
    Confidence, DataDomain, Difficulty, DurationMs, EpisodeId, EvidenceId, ExperimentId,
    InterventionId, InterventionState, ReviewItemId, Revision, SessionId, TrustClass, UtcMillis,
};
use g9_retention::{
    CalibrationObservation, ItemKind, ItemState, PracticalTransferCheck, ReviewItem, ReviewOutcome,
    TransferStatus,
};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// review items
// ---------------------------------------------------------------------------

/// Review item kind (mirrors `review_items.kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewItemKind {
    /// Declarative fact (FSRS-eligible).
    Declarative,
    /// Procedure (baseline schedule only).
    Procedural,
}

impl ReviewItemKind {
    /// Map the retention crate's kind; `CONCEPT` has no column value (gap).
    pub fn from_domain(kind: ItemKind) -> StorageResult<Self> {
        Ok(match kind {
            ItemKind::DeclarativeFact => Self::Declarative,
            ItemKind::Procedure => Self::Procedural,
            ItemKind::Concept => {
                return Err(StorageError::NotImplemented("retention.item_kind.concept"))
            }
        })
    }

    /// The retention crate's kind.
    pub const fn to_domain(self) -> ItemKind {
        match self {
            Self::Declarative => ItemKind::DeclarativeFact,
            Self::Procedural => ItemKind::Procedure,
        }
    }
}

/// Scheduler owning an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SchedulerKind {
    /// Fixed baseline offsets (default).
    Baseline,
    /// FSRS (declarative items only; feature-gated).
    Fsrs,
}

impl SchedulerKind {
    /// Name used in `ItemState::scheduler`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Baseline => "BASELINE",
            Self::Fsrs => "FSRS",
        }
    }
}

/// Review item lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewItemState {
    /// In the queue.
    Active,
    /// Temporarily out of the queue.
    Suspended,
    /// Retired.
    Retired,
}

/// A `review_items` row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewItemRow {
    /// Item id.
    pub id: ReviewItemId,
    /// Kind.
    pub kind: ReviewItemKind,
    /// Competency.
    pub competency_id: Option<CompetencyId>,
    /// Linked objective row.
    pub objective_id: Option<String>,
    /// Prompt evidence reference.
    pub prompt: String,
    /// Answer evidence reference (`Private`).
    pub answer_ref: Option<String>,
    /// Scheduler.
    pub scheduler: SchedulerKind,
    /// Next due instant.
    pub due_at: UtcMillis,
    /// Last successful recall.
    pub last_success_at: Option<UtcMillis>,
    /// Baseline stage index reached.
    pub stage: u32,
    /// Current interval in days.
    pub interval_days: u32,
    /// Scheduler-specific state blob (FSRS card).
    pub fsrs_state: Option<serde_json::Value>,
    /// Lifecycle.
    pub state: ReviewItemState,
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

impl ReviewItemRow {
    /// Build a row from a retention item with its initial due instant.
    pub fn from_domain(
        item: &ReviewItem,
        scheduler: SchedulerKind,
        due_at: UtcMillis,
    ) -> StorageResult<Self> {
        let kind = ReviewItemKind::from_domain(item.kind)?;
        if scheduler == SchedulerKind::Fsrs && kind != ReviewItemKind::Declarative {
            return Err(StorageError::InvalidInput(
                "FSRS schedules declarative items only".to_string(),
            ));
        }
        Ok(Self {
            id: item.id.clone(),
            kind,
            competency_id: Some(item.competency_id.clone()),
            objective_id: None,
            prompt: item.prompt_ref.as_str().to_string(),
            answer_ref: Some(item.answer_ref.as_str().to_string()),
            scheduler,
            due_at,
            last_success_at: None,
            stage: 0,
            interval_days: 1,
            fsrs_state: None,
            state: ReviewItemState::Active,
            data_domain: item.data_domain,
            classification: Classification::Internal,
            revision: Revision::INITIAL,
            created_at: item.created_at,
            updated_at: item.created_at,
        })
    }

    /// The retention crate's item (requires competency and answer reference).
    pub fn to_domain(&self) -> StorageResult<ReviewItem> {
        Ok(ReviewItem {
            id: self.id.clone(),
            competency_id: self.competency_id.clone().ok_or_else(|| {
                StorageError::RowDecode("review item without competency".to_string())
            })?,
            kind: self.kind.to_domain(),
            prompt_ref: parse_id::<EvidenceId>(&self.prompt)?,
            answer_ref: parse_id::<EvidenceId>(self.answer_ref.as_deref().ok_or_else(|| {
                StorageError::RowDecode("review item without answer".to_string())
            })?)?,
            created_at: self.created_at,
            data_domain: self.data_domain,
        })
    }
}

const ITEM_COLUMNS: &str = "id, kind, competency_id, objective_id, prompt, answer_ref, scheduler, due_at, last_success_at, stage,
    interval_days, fsrs_state_json, state, data_domain, classification, revision, created_at, updated_at";

fn map_item(r: &Row<'_>) -> StorageResult<ReviewItemRow> {
    Ok(ReviewItemRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        kind: enum_from_text(&r.get::<_, String>("kind")?)?,
        competency_id: opt_parse_id(r.get("competency_id")?)?,
        objective_id: r.get("objective_id")?,
        prompt: r.get("prompt")?,
        answer_ref: r.get("answer_ref")?,
        scheduler: enum_from_text(&r.get::<_, String>("scheduler")?)?,
        due_at: from_ms(r.get("due_at")?),
        last_success_at: opt_from_ms(r.get("last_success_at")?),
        stage: u32_from_i64(r.get("stage")?, "stage")?,
        interval_days: u32_from_i64(r.get("interval_days")?, "interval_days")?,
        fsrs_state: r
            .get::<_, Option<String>>("fsrs_state_json")?
            .as_deref()
            .map(|t| from_json(t, "fsrs_state_json"))
            .transpose()?,
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert a review item.
pub fn insert_review_item(conn: &Connection, row: &ReviewItemRow) -> RepoResult<Revision> {
    conn.execute(
        &format!(
            "INSERT INTO review_items ({ITEM_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, 1, ?16, ?16)"
        ),
        params![
            row.id.as_str(),
            enum_to_text(&row.kind)?,
            row.competency_id.as_ref().map(|c| c.as_str().to_string()),
            row.objective_id,
            row.prompt,
            row.answer_ref,
            enum_to_text(&row.scheduler)?,
            ms(row.due_at),
            opt_ms(row.last_success_at),
            i64::from(row.stage),
            i64::from(row.interval_days),
            row.fsrs_state.as_ref().map(to_json).transpose()?,
            enum_to_text(&row.state)?,
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One item, scope-checked.
pub fn get_review_item(
    conn: &Connection,
    scope: &ReadScope,
    id: &ReviewItemId,
) -> RepoResult<Option<ReviewItemRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {ITEM_COLUMNS} FROM review_items WHERE id = ?1"),
        &[&id.as_str()],
        map_item,
    )?;
    if let Some(r) = &row {
        scope.check("review_items", r.data_domain)?;
    }
    Ok(row)
}

/// Active items due at or before `now`, earliest first, domain-filtered in SQL.
pub fn list_due_items(
    conn: &Connection,
    scope: &ReadScope,
    now: UtcMillis,
    limit: u32,
) -> RepoResult<Vec<ReviewItemRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {ITEM_COLUMNS} FROM review_items
             WHERE state = 'ACTIVE' AND due_at <= ?1 AND (?2 = 1 OR data_domain = ?3)
             ORDER BY due_at, id LIMIT ?4"
        ),
        &[
            &ms(now),
            &scope.sql_flag(),
            &scope.sql_domain()?,
            &i64::from(limit),
        ],
        map_item,
    )
}

/// Items of a competency, domain-filtered in SQL.
pub fn list_items_for_competency(
    conn: &Connection,
    scope: &ReadScope,
    competency_id: &CompetencyId,
) -> RepoResult<Vec<ReviewItemRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {ITEM_COLUMNS} FROM review_items WHERE competency_id = ?1 AND (?2 = 1 OR data_domain = ?3)
             ORDER BY due_at, id"
        ),
        &[&competency_id.as_str(), &scope.sql_flag(), &scope.sql_domain()?],
        map_item,
    )
}

/// Scheduler columns written after a review.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchedulePatch {
    /// Next due instant.
    pub due_at: UtcMillis,
    /// Last successful recall.
    pub last_success_at: Option<UtcMillis>,
    /// Stage index.
    pub stage: u32,
    /// Interval in days.
    pub interval_days: u32,
    /// Scheduler blob.
    pub fsrs_state: Option<serde_json::Value>,
}

/// Write the next schedule of an item under a revision check.
pub fn update_schedule(
    conn: &Connection,
    id: &ReviewItemId,
    expected_revision: Revision,
    patch: &SchedulePatch,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let changed = conn.execute(
        "UPDATE review_items SET due_at = ?3, last_success_at = ?4, stage = ?5, interval_days = ?6, fsrs_state_json = ?7,
             revision = revision + 1, updated_at = ?8
         WHERE id = ?1 AND revision = ?2",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            ms(patch.due_at),
            opt_ms(patch.last_success_at),
            i64::from(patch.stage),
            i64::from(patch.interval_days),
            patch.fsrs_state.as_ref().map(to_json).transpose()?,
            ms(now),
        ],
    )?;
    finish_update(
        conn,
        "review_items",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

/// Change the lifecycle state of an item.
pub fn set_item_state(
    conn: &Connection,
    id: &ReviewItemId,
    expected_revision: Revision,
    state: ReviewItemState,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let changed = conn.execute(
        "UPDATE review_items SET state = ?3, revision = revision + 1, updated_at = ?4 WHERE id = ?1 AND revision = ?2",
        params![id.as_str(), rev_i64(expected_revision), enum_to_text(&state)?, ms(now)],
    )?;
    finish_update(
        conn,
        "review_items",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

/// Recall outcome (mirrors `review_attempts.outcome`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewAttemptOutcome {
    /// Recalled.
    Success,
    /// Not recalled.
    Failure,
    /// Skipped without an answer.
    Skipped,
}

impl ReviewAttemptOutcome {
    /// The scheduler outcome (`None` for skips, which do not reschedule).
    pub const fn to_review_outcome(self) -> Option<ReviewOutcome> {
        match self {
            Self::Success => Some(ReviewOutcome::Success),
            Self::Failure => Some(ReviewOutcome::Fail),
            Self::Skipped => None,
        }
    }
}

/// A `review_attempts` row (immutable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewAttemptRow {
    /// Row id.
    pub id: String,
    /// Item reviewed.
    pub review_item_id: ReviewItemId,
    /// Session, if reviewed inside one.
    pub session_id: Option<SessionId>,
    /// When.
    pub attempted_at: UtcMillis,
    /// Outcome.
    pub outcome: ReviewAttemptOutcome,
    /// Response time.
    pub response_ms: Option<DurationMs>,
    /// Stated confidence before answering.
    pub confidence: Option<Confidence>,
    /// Self-rating 1..=4 (not a probability).
    pub rating: Option<u8>,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Append a recall attempt (never updated or deleted; trigger-enforced).
pub fn append_review_attempt(conn: &Connection, row: &ReviewAttemptRow) -> RepoResult<()> {
    if let Some(rating) = row.rating {
        if !(1..=4).contains(&rating) {
            return Err(RepoError::Storage(StorageError::InvalidInput(
                "rating must be 1..=4".to_string(),
            )));
        }
    }
    conn.execute(
        "INSERT INTO review_attempts (id, review_item_id, session_id, attempted_at, outcome, response_ms, confidence, rating, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            row.id,
            row.review_item_id.as_str(),
            row.session_id.as_ref().map(|s| s.as_str().to_string()),
            ms(row.attempted_at),
            enum_to_text(&row.outcome)?,
            row.response_ms.map(DurationMs::as_millis),
            row.confidence.map(Confidence::get),
            row.rating.map(i64::from),
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

/// Recall history of an item, oldest first.
pub fn list_review_attempts(
    conn: &Connection,
    item_id: &ReviewItemId,
) -> RepoResult<Vec<ReviewAttemptRow>> {
    query_all(
        conn,
        "SELECT id, review_item_id, session_id, attempted_at, outcome, response_ms, confidence, rating, created_at
         FROM review_attempts WHERE review_item_id = ?1 ORDER BY attempted_at, id",
        &[&item_id.as_str()],
        |r| {
            Ok(ReviewAttemptRow {
                id: r.get("id")?,
                review_item_id: parse_id(&r.get::<_, String>("review_item_id")?)?,
                session_id: opt_parse_id(r.get("session_id")?)?,
                attempted_at: from_ms(r.get("attempted_at")?),
                outcome: enum_from_text(&r.get::<_, String>("outcome")?)?,
                response_ms: r
                    .get::<_, Option<i64>>("response_ms")?
                    .map(|v| DurationMs::new(v).map_err(|_| StorageError::RowDecode("response_ms".to_string())))
                    .transpose()?,
                confidence: r
                    .get::<_, Option<f64>>("confidence")?
                    .map(|c| Confidence::new(c).map_err(StorageError::RowDecode))
                    .transpose()?,
                rating: r
                    .get::<_, Option<i64>>("rating")?
                    .map(|v| u8_from_i64(v, "rating"))
                    .transpose()?,
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

/// Fold the item row and its recall history into the scheduler-agnostic `ItemState`.
pub fn item_state(conn: &Connection, id: &ReviewItemId) -> RepoResult<Option<ItemState>> {
    let scope = ReadScope::cross_domain(DataDomain::Learning);
    let Some(item) = get_review_item(conn, &scope, id)? else {
        return Ok(None);
    };
    let (successes, lapses, last): (i64, i64, Option<i64>) = conn.query_row(
        "SELECT COALESCE(SUM(outcome = 'SUCCESS'), 0), COALESCE(SUM(outcome = 'FAILURE'), 0), MAX(attempted_at)
         FROM review_attempts WHERE review_item_id = ?1 AND outcome <> 'SKIPPED'",
        params![id.as_str()],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    Ok(Some(ItemState {
        item_id: item.id,
        stage_index: usize::try_from(item.stage).unwrap_or(usize::MAX),
        last_reviewed_at: opt_from_ms(last),
        due_at: item.due_at,
        lapses: u32_from_i64(lapses, "lapses")?,
        successes: u32_from_i64(successes, "successes")?,
        scheduler: item.scheduler.name().to_string(),
        engine_state: item.fsrs_state,
    }))
}

// ---------------------------------------------------------------------------
// practical transfer checks
// ---------------------------------------------------------------------------

/// Who scheduled a transfer check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScheduledBy {
    /// The competency validity window.
    ValidityWindow,
    /// The operator.
    Operator,
}

/// Transfer check lifecycle (mirrors `practical_transfer_checks.state`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferCheckState {
    /// Due, not yet placed.
    Due,
    /// Placed on the calendar.
    Scheduled,
    /// Completed through an attempt.
    Completed,
    /// Waived by the operator.
    Waived,
}

/// A `practical_transfer_checks` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferCheckRow {
    /// Row id.
    pub id: String,
    /// Competency re-verified.
    pub competency_id: CompetencyId,
    /// Due instant.
    pub due_at: UtcMillis,
    /// Scheduler.
    pub scheduled_by: ScheduledBy,
    /// Lifecycle.
    pub state: TransferCheckState,
    /// Attempt that completed it.
    pub attempt_id: Option<AttemptId>,
    /// Completion instant.
    pub completed_at: Option<UtcMillis>,
    /// Revision.
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

impl TransferCheckRow {
    /// Build a row from the retention crate's check (`Skipped` → `WAIVED`;
    /// `novelty_requirement`/`last_validated_at` have no column).
    pub fn from_domain(
        check: &PracticalTransferCheck,
        scheduled_by: ScheduledBy,
        now: UtcMillis,
    ) -> Self {
        let (state, completed_at) = match &check.status {
            TransferStatus::Scheduled => (TransferCheckState::Scheduled, None),
            TransferStatus::Completed { at, .. } => (TransferCheckState::Completed, Some(*at)),
            TransferStatus::Skipped { at, .. } => (TransferCheckState::Waived, Some(*at)),
        };
        Self {
            id: check.id.clone(),
            competency_id: check.competency_id.clone(),
            due_at: check.due_at,
            scheduled_by,
            state,
            attempt_id: None,
            completed_at,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        }
    }
}

const TRANSFER_COLUMNS: &str =
    "id, competency_id, due_at, scheduled_by, state, attempt_id, completed_at, revision, created_at, updated_at";

fn map_transfer(r: &Row<'_>) -> StorageResult<TransferCheckRow> {
    Ok(TransferCheckRow {
        id: r.get("id")?,
        competency_id: parse_id(&r.get::<_, String>("competency_id")?)?,
        due_at: from_ms(r.get("due_at")?),
        scheduled_by: enum_from_text(&r.get::<_, String>("scheduled_by")?)?,
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        attempt_id: opt_parse_id(r.get("attempt_id")?)?,
        completed_at: opt_from_ms(r.get("completed_at")?),
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert a transfer check.
pub fn insert_transfer_check(conn: &Connection, row: &TransferCheckRow) -> RepoResult<Revision> {
    conn.execute(
        &format!(
            "INSERT INTO practical_transfer_checks ({TRANSFER_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?8, ?8)"
        ),
        params![
            row.id,
            row.competency_id.as_str(),
            ms(row.due_at),
            enum_to_text(&row.scheduled_by)?,
            enum_to_text(&row.state)?,
            row.attempt_id.as_ref().map(|a| a.as_str().to_string()),
            opt_ms(row.completed_at),
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One transfer check.
pub fn get_transfer_check(conn: &Connection, id: &str) -> RepoResult<Option<TransferCheckRow>> {
    query_opt(
        conn,
        &format!("SELECT {TRANSFER_COLUMNS} FROM practical_transfer_checks WHERE id = ?1"),
        &[&id],
        map_transfer,
    )
}

/// Checks that are `DUE` or `SCHEDULED` with `due_at <= now`, earliest first.
pub fn list_due_transfer_checks(
    conn: &Connection,
    now: UtcMillis,
) -> RepoResult<Vec<TransferCheckRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {TRANSFER_COLUMNS} FROM practical_transfer_checks
             WHERE state IN ('DUE', 'SCHEDULED') AND due_at <= ?1 ORDER BY due_at, id"
        ),
        &[&ms(now)],
        map_transfer,
    )
}

/// Move a transfer check forward (`DUE → SCHEDULED → COMPLETED`, or `WAIVED`);
/// `COMPLETED` requires the attempt and the completion instant.
pub fn update_transfer_check(
    conn: &Connection,
    id: &str,
    expected_revision: Revision,
    state: TransferCheckState,
    attempt_id: Option<&AttemptId>,
    completed_at: Option<UtcMillis>,
    now: UtcMillis,
) -> RepoResult<Revision> {
    if state == TransferCheckState::Completed && (attempt_id.is_none() || completed_at.is_none()) {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "COMPLETED requires attempt_id and completed_at".to_string(),
        )));
    }
    let changed = conn.execute(
        "UPDATE practical_transfer_checks SET state = ?3, attempt_id = ?4, completed_at = ?5, revision = revision + 1, updated_at = ?6
         WHERE id = ?1 AND revision = ?2 AND state IN ('DUE', 'SCHEDULED')",
        params![
            id,
            rev_i64(expected_revision),
            enum_to_text(&state)?,
            attempt_id.map(|a| a.as_str().to_string()),
            opt_ms(completed_at),
            ms(now)
        ],
    )?;
    finish_update(
        conn,
        "practical_transfer_checks",
        "id",
        id,
        expected_revision,
        changed,
    )
}

// ---------------------------------------------------------------------------
// learning episodes
// ---------------------------------------------------------------------------

/// Episode kind (mirrors `learning_episodes.kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EpisodeKind {
    /// Guided lab.
    GuidedLab,
    /// Independent lab.
    IndependentLab,
    /// Theory.
    Theory,
    /// Recall.
    Recall,
    /// Defense verification.
    Defense,
    /// Report.
    Report,
    /// Other.
    Other,
}

/// A `learning_episodes` row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EpisodeRow {
    /// Episode id.
    pub id: EpisodeId,
    /// Session.
    pub session_id: Option<SessionId>,
    /// Linked objective row.
    pub objective_id: Option<String>,
    /// Competency exercised.
    pub competency_id: Option<CompetencyId>,
    /// Kind.
    pub kind: EpisodeKind,
    /// Start.
    pub started_at: UtcMillis,
    /// End (`None` while open).
    pub ended_at: Option<UtcMillis>,
    /// Assistance received.
    pub assistance_level: AssistanceLevel,
    /// Difficulty, when known.
    pub difficulty: Option<Difficulty>,
    /// Structured outcome (facts and ids).
    pub outcome: serde_json::Value,
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

const EPISODE_COLUMNS: &str = "id, session_id, objective_id, competency_id, kind, started_at, ended_at, assistance_level, difficulty,
    outcome_json, data_domain, classification, revision, created_at, updated_at";

fn map_episode(r: &Row<'_>) -> StorageResult<EpisodeRow> {
    Ok(EpisodeRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        session_id: opt_parse_id(r.get("session_id")?)?,
        objective_id: r.get("objective_id")?,
        competency_id: opt_parse_id(r.get("competency_id")?)?,
        kind: enum_from_text(&r.get::<_, String>("kind")?)?,
        started_at: from_ms(r.get("started_at")?),
        ended_at: opt_from_ms(r.get("ended_at")?),
        assistance_level: enum_from_text(&r.get::<_, String>("assistance_level")?)?,
        difficulty: r
            .get::<_, Option<i64>>("difficulty")?
            .map(|d| {
                Difficulty::new(u8_from_i64(d, "difficulty")?).map_err(StorageError::RowDecode)
            })
            .transpose()?,
        outcome: from_json(&r.get::<_, String>("outcome_json")?, "outcome_json")?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert an episode.
pub fn insert_episode(conn: &Connection, row: &EpisodeRow) -> RepoResult<Revision> {
    conn.execute(
        &format!(
            "INSERT INTO learning_episodes ({EPISODE_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 1, ?13, ?13)"
        ),
        params![
            row.id.as_str(),
            row.session_id.as_ref().map(|s| s.as_str().to_string()),
            row.objective_id,
            row.competency_id.as_ref().map(|c| c.as_str().to_string()),
            enum_to_text(&row.kind)?,
            ms(row.started_at),
            opt_ms(row.ended_at),
            enum_to_text(&row.assistance_level)?,
            row.difficulty.map(|d| i64::from(d.get())),
            to_json(&row.outcome)?,
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One episode, scope-checked.
pub fn get_episode(
    conn: &Connection,
    scope: &ReadScope,
    id: &EpisodeId,
) -> RepoResult<Option<EpisodeRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {EPISODE_COLUMNS} FROM learning_episodes WHERE id = ?1"),
        &[&id.as_str()],
        map_episode,
    )?;
    if let Some(r) = &row {
        scope.check("learning_episodes", r.data_domain)?;
    }
    Ok(row)
}

/// Episodes of a session, domain-filtered in SQL.
pub fn list_episodes_for_session(
    conn: &Connection,
    scope: &ReadScope,
    session_id: &SessionId,
) -> RepoResult<Vec<EpisodeRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {EPISODE_COLUMNS} FROM learning_episodes WHERE session_id = ?1 AND (?2 = 1 OR data_domain = ?3)
             ORDER BY started_at, id"
        ),
        &[&session_id.as_str(), &scope.sql_flag(), &scope.sql_domain()?],
        map_episode,
    )
}

/// Close an open episode with its outcome; assistance is recorded as the maximum.
#[allow(clippy::too_many_arguments)]
pub fn close_episode(
    conn: &Connection,
    id: &EpisodeId,
    expected_revision: Revision,
    ended_at: UtcMillis,
    assistance_level: AssistanceLevel,
    difficulty: Option<Difficulty>,
    outcome: &serde_json::Value,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let scope = ReadScope::cross_domain(DataDomain::Learning);
    let current = required("learning_episodes", get_episode(conn, &scope, id)?)?;
    if current.revision != expected_revision {
        return Err(RepoError::Storage(StorageError::StaleRevision {
            expected: Some(expected_revision),
            actual: current.revision,
        }));
    }
    if current.ended_at.is_some() {
        return Err(RepoError::Storage(StorageError::Precondition(
            "learning_episodes.closed: episode already ended".to_string(),
        )));
    }
    if ended_at < current.started_at {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "ended_at before started_at".to_string(),
        )));
    }
    let assistance = assistance_level.max(current.assistance_level);
    let changed = conn.execute(
        "UPDATE learning_episodes SET ended_at = ?3, assistance_level = ?4, difficulty = ?5, outcome_json = ?6,
             revision = revision + 1, updated_at = ?7
         WHERE id = ?1 AND revision = ?2 AND ended_at IS NULL",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            ms(ended_at),
            enum_to_text(&assistance)?,
            difficulty.map(|d| i64::from(d.get())),
            to_json(outcome)?,
            ms(now)
        ],
    )?;
    finish_update(
        conn,
        "learning_episodes",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

// ---------------------------------------------------------------------------
// calibration
// ---------------------------------------------------------------------------

/// A `calibration_observations` row (immutable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationRow {
    /// Row id.
    pub id: String,
    /// Attempt the confidence was stated on.
    pub attempt_id: Option<AttemptId>,
    /// Review attempt the confidence was stated on.
    pub review_attempt_id: Option<String>,
    /// Stated probability of being correct.
    pub stated_confidence: Confidence,
    /// Whether the answer was correct.
    pub outcome: bool,
    /// Brier score (computed by the repository on insert).
    pub brier: f64,
    /// When.
    pub observed_at: UtcMillis,
    /// Row creation.
    pub created_at: UtcMillis,
}

impl CalibrationRow {
    /// The retention crate's observation (`question_ref` is the attempt or
    /// review-attempt id).
    pub fn to_domain(&self) -> StorageResult<CalibrationObservation> {
        let question = match (&self.attempt_id, &self.review_attempt_id) {
            (Some(a), _) => parse_id::<EvidenceId>(a.as_str())?,
            (None, Some(r)) => parse_id::<EvidenceId>(r)?,
            (None, None) => {
                return Err(StorageError::RowDecode(
                    "calibration observation without a question".to_string(),
                ))
            }
        };
        Ok(CalibrationObservation {
            question_ref: question,
            stated_probability: self.stated_confidence,
            correct: self.outcome,
            at: self.observed_at,
        })
    }
}

/// Append a calibration observation; `brier` is recomputed from the stated
/// confidence and the outcome.
pub fn append_calibration(conn: &Connection, row: &CalibrationRow) -> RepoResult<()> {
    if row.attempt_id.is_none() && row.review_attempt_id.is_none() {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "calibration observation needs an attempt or a review attempt".to_string(),
        )));
    }
    let brier = row.stated_confidence.brier(row.outcome);
    conn.execute(
        "INSERT INTO calibration_observations (id, attempt_id, review_attempt_id, stated_confidence, outcome, brier, observed_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            row.id,
            row.attempt_id.as_ref().map(|a| a.as_str().to_string()),
            row.review_attempt_id,
            row.stated_confidence.get(),
            bool_i64(row.outcome),
            brier,
            ms(row.observed_at),
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

/// Calibration observations in `[from, to)`, oldest first.
pub fn list_calibration_between(
    conn: &Connection,
    from: UtcMillis,
    to: UtcMillis,
) -> RepoResult<Vec<CalibrationRow>> {
    query_all(
        conn,
        "SELECT id, attempt_id, review_attempt_id, stated_confidence, outcome, brier, observed_at, created_at
         FROM calibration_observations WHERE observed_at >= ?1 AND observed_at < ?2 ORDER BY observed_at, id",
        &[&ms(from), &ms(to)],
        |r| {
            Ok(CalibrationRow {
                id: r.get("id")?,
                attempt_id: opt_parse_id(r.get("attempt_id")?)?,
                review_attempt_id: r.get("review_attempt_id")?,
                stated_confidence: Confidence::new(r.get("stated_confidence")?).map_err(StorageError::RowDecode)?,
                outcome: i64_bool(r.get("outcome")?)?,
                brier: r.get("brier")?,
                observed_at: from_ms(r.get("observed_at")?),
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

// ---------------------------------------------------------------------------
// interventions
// ---------------------------------------------------------------------------

/// An `interventions` row.
///
/// Two trust columns (migration `0010`): `origin_trust` is the proposer's trust
/// class, written once and immutable; `trust` is the authority of the row's
/// current state. A proposal that did not originate with the operator stays
/// non-authoritative (`HYPOTHESIS`/`PROPOSED`/`REJECTED`/`RETIRED` only, DDL
/// CHECK + trigger) until [`review_intervention`] records an operator review,
/// which elevates `trust` to `OPERATOR` and keeps `origin_trust` for display
/// ("model recommendation, operator reviewed").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterventionRow {
    /// Intervention id.
    pub id: InterventionId,
    /// Hypothesis it addresses.
    pub hypothesis_id: Option<HypothesisId>,
    /// Competency it targets.
    pub competency_id: Option<CompetencyId>,
    /// Kind label (`worked_example`, `spaced_drill`, ...).
    pub kind: String,
    /// Description (IPC `statement`).
    pub description: String,
    /// Ordered plan steps (IPC `steps`).
    pub steps: Vec<String>,
    /// Time budget of the plan (IPC `time_budget_ms`).
    pub time_budget: DurationMs,
    /// Pre-declared measurable acceptance (IPC `acceptance`, LRN-16).
    pub acceptance: String,
    /// Machine state.
    pub state: InterventionState,
    /// Proposing actor.
    pub proposed_by: ActorId,
    /// When proposed.
    pub proposed_at: UtcMillis,
    /// Trust class of the proposer (immutable; IPC `provenance`).
    pub origin_trust: TrustClass,
    /// Authority of the current state: equal to `origin_trust` until an
    /// operator review elevates it to `Operator`.
    pub trust: TrustClass,
    /// Operator who reviewed the plan (`PROPOSED -> REVIEWED`).
    pub reviewed_by: Option<ActorId>,
    /// When the plan was reviewed.
    pub reviewed_at: Option<UtcMillis>,
    /// Approval consumed when approved.
    pub approval_id: Option<ApprovalId>,
    /// Scheduled instant.
    pub scheduled_for: Option<UtcMillis>,
    /// Applied instant.
    pub applied_at: Option<UtcMillis>,
    /// Invalidation instant (hypothesis rejected).
    pub invalidated_at: Option<UtcMillis>,
    /// Invalidation reason.
    pub invalidation_reason: Option<String>,
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

const INTERVENTION_COLUMNS: &str = "id, hypothesis_id, competency_id, kind, description, steps_json, time_budget_ms, acceptance,
    state, proposed_by, proposed_at, origin_trust, trust, reviewed_by, reviewed_at, approval_id, scheduled_for, applied_at,
    invalidated_at, invalidation_reason, data_domain, classification, revision, created_at, updated_at";

fn map_intervention(r: &Row<'_>) -> StorageResult<InterventionRow> {
    Ok(InterventionRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        hypothesis_id: r
            .get::<_, Option<String>>("hypothesis_id")?
            .as_deref()
            .map(|h| HypothesisId::parse(h).map_err(|e| StorageError::RowDecode(e.to_string())))
            .transpose()?,
        competency_id: opt_parse_id(r.get("competency_id")?)?,
        kind: r.get("kind")?,
        description: r.get("description")?,
        steps: from_json(&r.get::<_, String>("steps_json")?, "steps_json")?,
        time_budget: DurationMs::new(r.get("time_budget_ms")?)
            .map_err(|_| StorageError::RowDecode("time_budget_ms".to_string()))?,
        acceptance: r.get("acceptance")?,
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        proposed_by: parse_id(&r.get::<_, String>("proposed_by")?)?,
        proposed_at: from_ms(r.get("proposed_at")?),
        origin_trust: enum_from_text(&r.get::<_, String>("origin_trust")?)?,
        trust: enum_from_text(&r.get::<_, String>("trust")?)?,
        reviewed_by: opt_parse_id(r.get("reviewed_by")?)?,
        reviewed_at: opt_from_ms(r.get("reviewed_at")?),
        approval_id: opt_parse_id(r.get("approval_id")?)?,
        scheduled_for: opt_from_ms(r.get("scheduled_for")?),
        applied_at: opt_from_ms(r.get("applied_at")?),
        invalidated_at: opt_from_ms(r.get("invalidated_at")?),
        invalidation_reason: r.get("invalidation_reason")?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// States a non-authoritative intervention may hold (mirror of the DDL CHECK/trigger).
fn allowed_without_authority(state: InterventionState) -> bool {
    matches!(
        state,
        InterventionState::Hypothesis
            | InterventionState::Proposed
            | InterventionState::Rejected
            | InterventionState::Retired
    )
}

fn validate_plan(steps: &[String], acceptance: &str) -> StorageResult<()> {
    if steps.iter().any(|s| s.trim().is_empty()) {
        return Err(StorageError::InvalidInput(
            "intervention steps must not be blank".to_string(),
        ));
    }
    // acceptance may be empty only while the plan is being drafted; review requires it
    if acceptance.len() > 4_096 {
        return Err(StorageError::InvalidInput(
            "intervention acceptance exceeds 4096 bytes".to_string(),
        ));
    }
    Ok(())
}

/// Insert an intervention in `HYPOTHESIS` or `PROPOSED` (every later state is an
/// operator decision applied through [`review_intervention`] and
/// [`transition_intervention`]). `trust` must equal `origin_trust` and no review
/// may be present on a new row.
pub fn insert_intervention(conn: &Connection, row: &InterventionRow) -> RepoResult<Revision> {
    if !matches!(
        row.state,
        InterventionState::Hypothesis | InterventionState::Proposed
    ) {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "a new intervention must be HYPOTHESIS or PROPOSED".to_string(),
        )));
    }
    if row.trust != row.origin_trust || row.reviewed_by.is_some() || row.reviewed_at.is_some() {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "a new intervention carries its origin trust and no review".to_string(),
        )));
    }
    super::require_non_empty("description", &row.description)?;
    validate_plan(&row.steps, &row.acceptance)?;
    conn.execute(
        &format!(
            "INSERT INTO interventions ({INTERVENTION_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12, NULL, NULL, NULL, ?13, NULL, NULL, NULL,
                     ?14, ?15, 1, ?16, ?16)"
        ),
        params![
            row.id.as_str(),
            row.hypothesis_id.as_ref().map(|h| h.as_str().to_string()),
            row.competency_id.as_ref().map(|c| c.as_str().to_string()),
            row.kind,
            row.description,
            to_json(&row.steps)?,
            row.time_budget.as_millis(),
            row.acceptance,
            enum_to_text(&row.state)?,
            row.proposed_by.as_str(),
            ms(row.proposed_at),
            enum_to_text(&row.origin_trust)?,
            opt_ms(row.scheduled_for),
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One intervention, scope-checked.
pub fn get_intervention(
    conn: &Connection,
    scope: &ReadScope,
    id: &InterventionId,
) -> RepoResult<Option<InterventionRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {INTERVENTION_COLUMNS} FROM interventions WHERE id = ?1"),
        &[&id.as_str()],
        map_intervention,
    )?;
    if let Some(r) = &row {
        scope.check("interventions", r.data_domain)?;
    }
    Ok(row)
}

/// Interventions in a state (or all), domain-filtered in SQL.
pub fn list_interventions(
    conn: &Connection,
    scope: &ReadScope,
    state: Option<InterventionState>,
) -> RepoResult<Vec<InterventionRow>> {
    let state_text = state.as_ref().map(enum_to_text).transpose()?;
    query_all(
        conn,
        &format!(
            "SELECT {INTERVENTION_COLUMNS} FROM interventions
             WHERE (?1 IS NULL OR state = ?1) AND (?2 = 1 OR data_domain = ?3) ORDER BY proposed_at, id"
        ),
        &[&state_text, &scope.sql_flag(), &scope.sql_domain()?],
        map_intervention,
    )
}

/// Load a row for a write, with the revision check (writes are policy-checked by
/// `g9-app`; the domain filter applies to reads only).
fn load_for_write(
    conn: &Connection,
    id: &InterventionId,
    expected_revision: Revision,
) -> RepoResult<InterventionRow> {
    let scope = ReadScope::cross_domain(DataDomain::Learning);
    let current = required("interventions", get_intervention(conn, &scope, id)?)?;
    if current.revision != expected_revision {
        return Err(RepoError::Storage(StorageError::StaleRevision {
            expected: Some(expected_revision),
            actual: current.revision,
        }));
    }
    Ok(current)
}

/// Columns an intervention transition may set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InterventionPatch {
    /// Approval consumed (`Approve`).
    pub approval_id: Option<ApprovalId>,
    /// Scheduled instant (`Schedule`).
    pub scheduled_for: Option<UtcMillis>,
    /// Applied instant (`Apply`).
    pub applied_at: Option<UtcMillis>,
}

/// Operator review of a proposed intervention (`PROPOSED -> REVIEWED`).
///
/// `reviewer` is the backend-assigned session actor (never a UI-supplied id) and
/// must be the operator. The review records `reviewed_by`/`reviewed_at` and
/// elevates `trust` to `OPERATOR` in the same revision-checked UPDATE; the
/// proposer's `origin_trust` is kept. A plan without a declared acceptance
/// cannot be reviewed (LRN-16).
pub fn review_intervention(
    conn: &Connection,
    id: &InterventionId,
    expected_revision: Revision,
    reviewer: &Actor,
    now: UtcMillis,
) -> RepoResult<(InterventionState, Revision)> {
    if !reviewer.is_operator() {
        return Err(RepoError::Storage(StorageError::Precondition(
            "intervention review requires the operator".to_string(),
        )));
    }
    let current = load_for_write(conn, id, expected_revision)?;
    let next = current.state.transition(InterventionEvent::Review)?;
    if current.acceptance.trim().is_empty() {
        return Err(RepoError::Storage(StorageError::Precondition(
            "intervention has no declared acceptance; it cannot be reviewed".to_string(),
        )));
    }
    let changed = conn.execute(
        "UPDATE interventions
         SET state = ?3, trust = 'OPERATOR', reviewed_by = ?4, reviewed_at = ?5, revision = revision + 1, updated_at = ?5
         WHERE id = ?1 AND revision = ?2 AND state = 'PROPOSED'",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            enum_to_text(&next)?,
            reviewer.id.as_str(),
            ms(now)
        ],
    )?;
    let revision = finish_update(
        conn,
        "interventions",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )?;
    Ok((next, revision))
}

/// Apply an `InterventionEvent` to a stored intervention under a revision check.
/// Patch columns are only written when present.
///
/// `Review` is refused here (use [`review_intervention`], which records the
/// reviewer). A transition into a state that needs authority is refused with
/// `PRECONDITION_FAILED` while the row is non-authoritative (not yet reviewed by
/// the operator); the DDL CHECK and trigger are the second line of defence.
pub fn transition_intervention(
    conn: &Connection,
    id: &InterventionId,
    expected_revision: Revision,
    event: InterventionEvent,
    patch: &InterventionPatch,
    now: UtcMillis,
) -> RepoResult<(InterventionState, Revision)> {
    if event == InterventionEvent::Review {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "PROPOSED -> REVIEWED goes through review_intervention".to_string(),
        )));
    }
    let current = load_for_write(conn, id, expected_revision)?;
    let next = current.state.transition(event)?;
    if !current.trust.may_be_authoritative() && !allowed_without_authority(next) {
        return Err(RepoError::Storage(StorageError::Precondition(
            "intervention.operator_review_required: a non-authoritative intervention cannot advance".to_string(),
        )));
    }
    let changed = conn.execute(
        "UPDATE interventions
         SET state = ?3, approval_id = COALESCE(?4, approval_id), scheduled_for = COALESCE(?5, scheduled_for),
             applied_at = COALESCE(?6, applied_at), revision = revision + 1, updated_at = ?7
         WHERE id = ?1 AND revision = ?2",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            enum_to_text(&next)?,
            patch.approval_id.as_ref().map(|a| a.as_str().to_string()),
            opt_ms(patch.scheduled_for),
            opt_ms(patch.applied_at),
            ms(now)
        ],
    )?;
    let revision = finish_update(
        conn,
        "interventions",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )?;
    Ok((next, revision))
}

/// The plan columns of an intervention (description, steps, budget, acceptance).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterventionPlan {
    /// Description (IPC `statement`).
    pub description: String,
    /// Ordered steps.
    pub steps: Vec<String>,
    /// Time budget.
    pub time_budget: DurationMs,
    /// Pre-declared measurable acceptance.
    pub acceptance: String,
}

/// Replace the plan of an intervention that is still being drafted
/// (`HYPOTHESIS`, `PROPOSED` or `REVISED`). A reviewed plan is frozen (DDL
/// trigger); a different plan goes through `REVISED -> PROPOSED` and a new review.
pub fn update_intervention_plan(
    conn: &Connection,
    id: &InterventionId,
    expected_revision: Revision,
    plan: &InterventionPlan,
    now: UtcMillis,
) -> RepoResult<Revision> {
    super::require_non_empty("description", &plan.description)?;
    validate_plan(&plan.steps, &plan.acceptance)?;
    let changed = conn.execute(
        "UPDATE interventions
         SET description = ?3, steps_json = ?4, time_budget_ms = ?5, acceptance = ?6, revision = revision + 1, updated_at = ?7
         WHERE id = ?1 AND revision = ?2 AND state IN ('HYPOTHESIS','PROPOSED','REVISED')",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            plan.description,
            to_json(&plan.steps)?,
            plan.time_budget.as_millis(),
            plan.acceptance,
            ms(now)
        ],
    )?;
    finish_update(
        conn,
        "interventions",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

/// Mark an intervention invalid (its hypothesis was rejected).
pub fn invalidate_intervention(
    conn: &Connection,
    id: &InterventionId,
    expected_revision: Revision,
    reason: &str,
    now: UtcMillis,
) -> RepoResult<Revision> {
    super::require_non_empty("reason", reason)?;
    let changed = conn.execute(
        "UPDATE interventions SET invalidated_at = ?3, invalidation_reason = ?4, revision = revision + 1, updated_at = ?3
         WHERE id = ?1 AND revision = ?2 AND invalidated_at IS NULL",
        params![id.as_str(), rev_i64(expected_revision), ms(now), reason],
    )?;
    finish_update(
        conn,
        "interventions",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

/// Verdict of an outcome measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutcomeVerdict {
    /// Improved.
    Improved,
    /// No measurable effect.
    NoEffect,
    /// Worse.
    Worse,
    /// Inconclusive.
    Inconclusive,
}

/// An `intervention_outcomes` row (immutable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterventionOutcomeRow {
    /// Row id.
    pub id: String,
    /// Intervention measured.
    pub intervention_id: InterventionId,
    /// When measured.
    pub measured_at: UtcMillis,
    /// Metric name (from `docs/evaluations/kpi-definitions.md`).
    pub metric: String,
    /// Baseline value.
    pub baseline_value: Option<f64>,
    /// Observed value.
    pub observed_value: Option<f64>,
    /// Effect size.
    pub effect_size: Option<f64>,
    /// Uncertainty description.
    pub uncertainty: Option<serde_json::Value>,
    /// Verdict.
    pub verdict: OutcomeVerdict,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Append an outcome measurement (never updated or deleted; trigger-enforced).
pub fn append_intervention_outcome(
    conn: &Connection,
    row: &InterventionOutcomeRow,
) -> RepoResult<()> {
    conn.execute(
        "INSERT INTO intervention_outcomes (id, intervention_id, measured_at, metric, baseline_value, observed_value, effect_size,
            uncertainty_json, verdict, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            row.id,
            row.intervention_id.as_str(),
            ms(row.measured_at),
            row.metric,
            row.baseline_value,
            row.observed_value,
            row.effect_size,
            row.uncertainty.as_ref().map(to_json).transpose()?,
            enum_to_text(&row.verdict)?,
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

/// Outcomes of an intervention, oldest first.
pub fn list_intervention_outcomes(
    conn: &Connection,
    intervention_id: &InterventionId,
) -> RepoResult<Vec<InterventionOutcomeRow>> {
    query_all(
        conn,
        "SELECT id, intervention_id, measured_at, metric, baseline_value, observed_value, effect_size, uncertainty_json, verdict,
                created_at
         FROM intervention_outcomes WHERE intervention_id = ?1 ORDER BY measured_at, id",
        &[&intervention_id.as_str()],
        |r| {
            Ok(InterventionOutcomeRow {
                id: r.get("id")?,
                intervention_id: parse_id(&r.get::<_, String>("intervention_id")?)?,
                measured_at: from_ms(r.get("measured_at")?),
                metric: r.get("metric")?,
                baseline_value: r.get("baseline_value")?,
                observed_value: r.get("observed_value")?,
                effect_size: r.get("effect_size")?,
                uncertainty: r
                    .get::<_, Option<String>>("uncertainty_json")?
                    .as_deref()
                    .map(|t| from_json(t, "uncertainty_json"))
                    .transpose()?,
                verdict: enum_from_text(&r.get::<_, String>("verdict")?)?,
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

// ---------------------------------------------------------------------------
// method experiments
// ---------------------------------------------------------------------------

/// Experiment lifecycle (forward only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExperimentState {
    /// Pre-registered, not started.
    Planned,
    /// Collecting data.
    Running,
    /// Analysis done.
    Analyzed,
    /// Closed.
    Closed,
}

/// A `method_experiments` row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExperimentRow {
    /// Experiment id.
    pub id: ExperimentId,
    /// Title.
    pub title: String,
    /// Hypothesis under test.
    pub hypothesis: String,
    /// Intervention evaluated.
    pub intervention_id: Option<InterventionId>,
    /// Pre-registered design (unit of randomization, allocation seed/sequence,
    /// outcomes, analysis query, stop rule). Immutable after insert.
    pub design: serde_json::Value,
    /// Start.
    pub started_at: Option<UtcMillis>,
    /// End.
    pub ended_at: Option<UtcMillis>,
    /// Lifecycle.
    pub state: ExperimentState,
    /// Result document (only once analyzed).
    pub result: Option<serde_json::Value>,
    /// Revision.
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

const EXPERIMENT_COLUMNS: &str =
    "id, title, hypothesis, intervention_id, design_json, started_at, ended_at, state, result_json,
    revision, created_at, updated_at";

fn map_experiment(r: &Row<'_>) -> StorageResult<ExperimentRow> {
    Ok(ExperimentRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        title: r.get("title")?,
        hypothesis: r.get("hypothesis")?,
        intervention_id: opt_parse_id(r.get("intervention_id")?)?,
        design: from_json(&r.get::<_, String>("design_json")?, "design_json")?,
        started_at: opt_from_ms(r.get("started_at")?),
        ended_at: opt_from_ms(r.get("ended_at")?),
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        result: r
            .get::<_, Option<String>>("result_json")?
            .as_deref()
            .map(|t| from_json(t, "result_json"))
            .transpose()?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Pre-register an experiment (`PLANNED`, no result).
pub fn insert_experiment(conn: &Connection, row: &ExperimentRow) -> RepoResult<Revision> {
    if row.state != ExperimentState::Planned || row.result.is_some() {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "a new experiment is PLANNED and has no result".to_string(),
        )));
    }
    if !row.design.is_object() {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "design must be a JSON object".to_string(),
        )));
    }
    conn.execute(
        &format!(
            "INSERT INTO method_experiments ({EXPERIMENT_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'PLANNED', NULL, 1, ?8, ?8)"
        ),
        params![
            row.id.as_str(),
            row.title,
            row.hypothesis,
            row.intervention_id.as_ref().map(|i| i.as_str().to_string()),
            to_json(&row.design)?,
            opt_ms(row.started_at),
            opt_ms(row.ended_at),
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One experiment.
pub fn get_experiment(conn: &Connection, id: &ExperimentId) -> RepoResult<Option<ExperimentRow>> {
    query_opt(
        conn,
        &format!("SELECT {EXPERIMENT_COLUMNS} FROM method_experiments WHERE id = ?1"),
        &[&id.as_str()],
        map_experiment,
    )
}

/// Experiments in a state (or all).
pub fn list_experiments(
    conn: &Connection,
    state: Option<ExperimentState>,
) -> RepoResult<Vec<ExperimentRow>> {
    let state_text = state.as_ref().map(enum_to_text).transpose()?;
    query_all(
        conn,
        &format!(
            "SELECT {EXPERIMENT_COLUMNS} FROM method_experiments WHERE (?1 IS NULL OR state = ?1) ORDER BY created_at, id"
        ),
        &[&state_text],
        map_experiment,
    )
}

/// Advance the lifecycle (forward only); the design never changes; a result is
/// accepted only when the experiment is `ANALYZED` or `CLOSED`.
#[allow(clippy::too_many_arguments)]
pub fn advance_experiment(
    conn: &Connection,
    id: &ExperimentId,
    expected_revision: Revision,
    state: ExperimentState,
    started_at: Option<UtcMillis>,
    ended_at: Option<UtcMillis>,
    result: Option<&serde_json::Value>,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let current = required("method_experiments", get_experiment(conn, id)?)?;
    if current.revision != expected_revision {
        return Err(RepoError::Storage(StorageError::StaleRevision {
            expected: Some(expected_revision),
            actual: current.revision,
        }));
    }
    if state <= current.state {
        return Err(RepoError::Storage(StorageError::Precondition(format!(
            "method_experiments.not_forward: {:?} -> {:?}",
            current.state, state
        ))));
    }
    if result.is_some() && state < ExperimentState::Analyzed {
        return Err(RepoError::Storage(StorageError::Precondition(
            "method_experiments.result_before_analysis".to_string(),
        )));
    }
    let changed = conn.execute(
        "UPDATE method_experiments
         SET state = ?3, started_at = COALESCE(?4, started_at), ended_at = COALESCE(?5, ended_at),
             result_json = COALESCE(?6, result_json), revision = revision + 1, updated_at = ?7
         WHERE id = ?1 AND revision = ?2",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            enum_to_text(&state)?,
            opt_ms(started_at),
            opt_ms(ended_at),
            result.map(to_json).transpose()?,
            ms(now)
        ],
    )?;
    finish_update(
        conn,
        "method_experiments",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::testutil::{approval, assert_immutable, db, learning, operator_id, t};
    use g9_retention::{schedule_transfer_check, BaselineSchedule};

    fn seed(conn: &Connection) {
        conn.execute_batch(
            "INSERT INTO competencies (id, code, title, domain_area, created_at, updated_at)
               VALUES ('comp-1', 'LINUX-BOOT', 'Linux boot', 'linux', 1, 1);
             INSERT INTO study_sessions (id, state, mode, zone, created_at, updated_at)
               VALUES ('sess-1', 'RUNNING', 'STUDY', 'UTC', 1, 1);
             INSERT INTO assessment_definitions (id, code, title, kind, created_at, updated_at)
               VALUES ('ad-1', 'W1', 'W1', 'BLIND_PRACTICAL', 1, 1);
             INSERT INTO assessment_versions (id, definition_id, version_label, task_statement, task_sha256, created_by, created_at)
               VALUES ('av-1', 'ad-1', '1', 'task', '0000000000000000000000000000000000000000000000000000000000000000', 'op', 1);
             INSERT INTO attempts (id, assessment_version_id, state, mode, created_at, updated_at)
               VALUES ('attempt-1', 'av-1', 'SUBMITTED', 'EXAM', 1, 1);",
        )
        .unwrap();
    }

    fn item() -> ReviewItem {
        ReviewItem {
            id: ReviewItemId::parse("ri-1").unwrap(),
            competency_id: CompetencyId::parse("comp-1").unwrap(),
            kind: ItemKind::DeclarativeFact,
            prompt_ref: EvidenceId::parse("evi-prompt").unwrap(),
            answer_ref: EvidenceId::parse("evi-answer").unwrap(),
            created_at: t(0),
            data_domain: DataDomain::Learning,
        }
    }

    #[test]
    fn review_items_schedule_and_recall_history_roundtrip() {
        let db = db();
        let conn = db.conn();
        seed(conn);
        let domain_item = item();
        let row = ReviewItemRow::from_domain(&domain_item, SchedulerKind::Baseline, t(86_400_000))
            .unwrap();
        insert_review_item(conn, &row).unwrap();
        let stored = get_review_item(conn, &learning(), &row.id)
            .unwrap()
            .unwrap();
        assert_eq!(stored, row);
        assert_eq!(stored.to_domain().unwrap(), domain_item);
        assert!(list_due_items(conn, &learning(), t(0), 10)
            .unwrap()
            .is_empty());
        assert_eq!(
            list_due_items(conn, &learning(), t(86_400_000), 10)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            list_items_for_competency(conn, &learning(), &domain_item.competency_id)
                .unwrap()
                .len(),
            1
        );

        // gaps and rules
        let mut concept = domain_item.clone();
        concept.id = ReviewItemId::parse("ri-c").unwrap();
        concept.kind = ItemKind::Concept;
        assert!(matches!(
            ReviewItemRow::from_domain(&concept, SchedulerKind::Baseline, t(0)),
            Err(StorageError::NotImplemented("retention.item_kind.concept"))
        ));
        let mut procedure = domain_item.clone();
        procedure.id = ReviewItemId::parse("ri-p").unwrap();
        procedure.kind = ItemKind::Procedure;
        assert!(ReviewItemRow::from_domain(&procedure, SchedulerKind::Fsrs, t(0)).is_err());
        insert_review_item(
            conn,
            &ReviewItemRow::from_domain(&procedure, SchedulerKind::Baseline, t(0)).unwrap(),
        )
        .unwrap();

        // a recall attempt, then the baseline schedule moves the item
        let attempt = ReviewAttemptRow {
            id: "ra-1".into(),
            review_item_id: row.id.clone(),
            session_id: Some(SessionId::parse("sess-1").unwrap()),
            attempted_at: t(86_400_000),
            outcome: ReviewAttemptOutcome::Success,
            response_ms: Some(DurationMs::new(4_000).unwrap()),
            confidence: Some(Confidence::new(0.8).unwrap()),
            rating: Some(3),
            created_at: t(86_400_000),
        };
        append_review_attempt(conn, &attempt).unwrap();
        assert!(matches!(
            append_review_attempt(
                conn,
                &ReviewAttemptRow {
                    id: "ra-bad".into(),
                    rating: Some(9),
                    ..attempt.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        assert_eq!(
            list_review_attempts(conn, &row.id).unwrap(),
            vec![attempt.clone()]
        );
        assert_immutable(
            conn,
            "UPDATE review_attempts SET outcome = 'FAILURE' WHERE id = 'ra-1'",
        );
        assert_immutable(conn, "DELETE FROM review_attempts WHERE id = 'ra-1'");

        let state = item_state(conn, &row.id).unwrap().unwrap();
        assert_eq!(state.successes, 1);
        assert_eq!(state.lapses, 0);
        assert_eq!(state.last_reviewed_at, Some(t(86_400_000)));
        assert_eq!(state.scheduler, "BASELINE");
        let schedule = BaselineSchedule::default();
        let next = schedule
            .next_after(
                &state,
                attempt.outcome.to_review_outcome().unwrap(),
                t(86_400_000),
            )
            .unwrap();
        let patch = SchedulePatch {
            due_at: next.due_at,
            last_success_at: Some(t(86_400_000)),
            stage: u32::try_from(next.stage_index).unwrap(),
            interval_days: 3,
            fsrs_state: None,
        };
        assert!(matches!(
            update_schedule(conn, &row.id, Revision::new(4), &patch, t(86_400_001)),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        let r2 = update_schedule(conn, &row.id, Revision::INITIAL, &patch, t(86_400_001)).unwrap();
        let moved = get_review_item(conn, &learning(), &row.id)
            .unwrap()
            .unwrap();
        assert_eq!(moved.due_at, next.due_at);
        assert!(moved.due_at > t(86_400_000));
        assert_eq!(moved.revision, r2);
        let r3 =
            set_item_state(conn, &row.id, r2, ReviewItemState::Suspended, t(86_400_002)).unwrap();
        assert_eq!(r3, Revision::new(3));
        assert!(list_due_items(conn, &learning(), t(10 * 86_400_000), 10)
            .unwrap()
            .iter()
            .all(|i| i.id != row.id));

        let mut personal = row.clone();
        personal.id = ReviewItemId::parse("ri-personal").unwrap();
        personal.data_domain = DataDomain::Personal;
        insert_review_item(conn, &personal).unwrap();
        assert!(matches!(
            get_review_item(conn, &learning(), &personal.id),
            Err(RepoError::CrossDomainRefused {
                table: "review_items",
                ..
            })
        ));
    }

    #[test]
    fn transfer_checks_episodes_and_calibration() {
        let db = db();
        let conn = db.conn();
        seed(conn);
        let comp = CompetencyId::parse("comp-1").unwrap();
        let check = schedule_transfer_check(comp.clone(), 90, Some(t(0)), "new toolchain", t(0));
        let row = TransferCheckRow::from_domain(&check, ScheduledBy::ValidityWindow, t(0));
        insert_transfer_check(conn, &row).unwrap();
        assert_eq!(get_transfer_check(conn, &row.id).unwrap().unwrap(), row);
        assert!(list_due_transfer_checks(conn, t(0)).unwrap().is_empty());
        assert_eq!(
            list_due_transfer_checks(conn, check.due_at).unwrap().len(),
            1
        );
        assert!(matches!(
            update_transfer_check(
                conn,
                &row.id,
                Revision::INITIAL,
                TransferCheckState::Completed,
                None,
                None,
                t(1)
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        let attempt = AttemptId::parse("attempt-1").unwrap();
        let r2 = update_transfer_check(
            conn,
            &row.id,
            Revision::INITIAL,
            TransferCheckState::Completed,
            Some(&attempt),
            Some(t(5)),
            t(5),
        )
        .unwrap();
        let done = get_transfer_check(conn, &row.id).unwrap().unwrap();
        assert_eq!(done.state, TransferCheckState::Completed);
        assert_eq!(done.attempt_id, Some(attempt.clone()));
        assert!(matches!(
            update_transfer_check(
                conn,
                &row.id,
                r2,
                TransferCheckState::Waived,
                None,
                None,
                t(6)
            ),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));

        let episode = EpisodeRow {
            id: EpisodeId::parse("ep-1").unwrap(),
            session_id: Some(SessionId::parse("sess-1").unwrap()),
            objective_id: None,
            competency_id: Some(comp.clone()),
            kind: EpisodeKind::GuidedLab,
            started_at: t(10),
            ended_at: None,
            assistance_level: AssistanceLevel::Prompted,
            difficulty: None,
            outcome: serde_json::json!({}),
            data_domain: DataDomain::Learning,
            classification: Classification::Private,
            revision: Revision::INITIAL,
            created_at: t(10),
            updated_at: t(10),
        };
        insert_episode(conn, &episode).unwrap();
        assert_eq!(
            get_episode(conn, &learning(), &episode.id)
                .unwrap()
                .unwrap(),
            episode
        );
        let sess = SessionId::parse("sess-1").unwrap();
        assert_eq!(
            list_episodes_for_session(conn, &learning(), &sess)
                .unwrap()
                .len(),
            1
        );
        let outcome = serde_json::json!({ "checks_passed": 3 });
        let r2 = close_episode(
            conn,
            &episode.id,
            Revision::INITIAL,
            t(20),
            AssistanceLevel::Unaided,
            Some(Difficulty::new(2).unwrap()),
            &outcome,
            t(20),
        )
        .unwrap();
        let closed = get_episode(conn, &learning(), &episode.id)
            .unwrap()
            .unwrap();
        assert_eq!(closed.ended_at, Some(t(20)));
        // assistance is a maximum: never lowered to UNAIDED after PROMPTED
        assert_eq!(closed.assistance_level, AssistanceLevel::Prompted);
        assert_eq!(closed.outcome, outcome);
        assert!(matches!(
            close_episode(
                conn,
                &episode.id,
                r2,
                t(30),
                AssistanceLevel::Unaided,
                None,
                &outcome,
                t(30)
            ),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));

        let calibration = CalibrationRow {
            id: "cal-1".into(),
            attempt_id: Some(attempt.clone()),
            review_attempt_id: None,
            stated_confidence: Confidence::new(0.8).unwrap(),
            outcome: true,
            brier: 0.0,
            observed_at: t(40),
            created_at: t(40),
        };
        append_calibration(conn, &calibration).unwrap();
        let listed = list_calibration_between(conn, t(0), t(100)).unwrap();
        assert_eq!(listed.len(), 1);
        assert!((listed[0].brier - 0.04).abs() < 1e-9);
        let domain = listed[0].to_domain().unwrap();
        assert_eq!(domain.question_ref.as_str(), "attempt-1");
        assert!(domain.correct);
        assert!(matches!(
            append_calibration(
                conn,
                &CalibrationRow {
                    id: "cal-2".into(),
                    attempt_id: None,
                    ..calibration
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        assert_immutable(
            conn,
            "UPDATE calibration_observations SET outcome = 0 WHERE id = 'cal-1'",
        );
        assert_immutable(
            conn,
            "DELETE FROM calibration_observations WHERE id = 'cal-1'",
        );
    }

    fn intervention(id: &str, origin: TrustClass, proposer: &str) -> InterventionRow {
        InterventionRow {
            id: InterventionId::parse(id).unwrap(),
            hypothesis_id: None,
            competency_id: Some(CompetencyId::parse("comp-1").unwrap()),
            kind: "worked_example".into(),
            description: "Walk through initramfs unpacking".into(),
            steps: vec!["unpack the image".into(), "list the init script".into()],
            time_budget: DurationMs::new(20 * 60_000).unwrap(),
            acceptance: "unaided retest >= 70 on an unexposed variant within 7 days".into(),
            state: InterventionState::Proposed,
            proposed_by: ActorId::parse(proposer).unwrap(),
            proposed_at: t(0),
            origin_trust: origin,
            trust: origin,
            reviewed_by: None,
            reviewed_at: None,
            approval_id: None,
            scheduled_for: None,
            applied_at: None,
            invalidated_at: None,
            invalidation_reason: None,
            data_domain: DataDomain::Learning,
            classification: Classification::Private,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        }
    }

    fn err_of(conn: &Connection, sql: &str) -> StorageError {
        conn.execute(sql, [])
            .expect_err("statement must be refused")
            .into()
    }

    #[test]
    fn model_proposed_intervention_cannot_advance_without_an_operator_review() {
        let db = db();
        let conn = db.conn();
        seed(conn);
        let row = intervention("int-1", TrustClass::ModelOutput, "model-1");
        insert_intervention(conn, &row).unwrap();
        assert_eq!(
            get_intervention(conn, &learning(), &row.id)
                .unwrap()
                .unwrap(),
            row
        );

        // inserts: only HYPOTHESIS/PROPOSED, origin == trust, no review
        assert!(matches!(
            insert_intervention(
                conn,
                &InterventionRow {
                    id: InterventionId::parse("int-x").unwrap(),
                    state: InterventionState::Approved,
                    ..row.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        assert!(matches!(
            insert_intervention(
                conn,
                &InterventionRow {
                    id: InterventionId::parse("int-x").unwrap(),
                    trust: TrustClass::Operator,
                    ..row.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        assert!(matches!(
            insert_intervention(
                conn,
                &InterventionRow {
                    id: InterventionId::parse("int-x").unwrap(),
                    steps: vec![" ".into()],
                    ..row.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        // the SQL trigger refuses a forged insert that claims operator authority for a model proposal
        assert!(matches!(
            err_of(conn, "INSERT INTO interventions (id, kind, description, state, proposed_by, proposed_at, origin_trust, trust, created_at, updated_at)
                          VALUES ('int-forged', 'k', 'd', 'PROPOSED', 'model-1', 1, 'MODEL_OUTPUT', 'OPERATOR', 1, 1)"),
            StorageError::Precondition(_)
        ));

        // Review only through review_intervention, only by the operator, with the current revision
        assert!(matches!(
            transition_intervention(
                conn,
                &row.id,
                Revision::INITIAL,
                InterventionEvent::Review,
                &InterventionPatch::default(),
                t(1)
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        let model_actor = Actor {
            id: ActorId::parse("model-1").unwrap(),
            kind: g9_domain::ActorKind::Model,
        };
        assert!(matches!(
            review_intervention(conn, &row.id, Revision::INITIAL, &model_actor, t(1)),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        assert!(matches!(
            review_intervention(
                conn,
                &row.id,
                Revision::new(3),
                &Actor::operator(operator_id()),
                t(1)
            ),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        assert!(matches!(
            transition_intervention(
                conn,
                &row.id,
                Revision::INITIAL,
                InterventionEvent::Apply,
                &InterventionPatch::default(),
                t(1)
            ),
            Err(RepoError::Storage(StorageError::InvalidTransition(_)))
        ));

        // a raw UPDATE past PROPOSED without elevation is refused by the 0010 trigger ...
        assert!(matches!(
            err_of(conn, "UPDATE interventions SET state = 'REVIEWED', reviewed_by = 'operator-1', reviewed_at = 5 WHERE id = 'int-1'"),
            StorageError::Precondition(_)
        ));
        // ... and the 0007 CHECK still refuses a MODEL_OUTPUT row past PROPOSED (not weakened)
        assert!(matches!(
            err_of(conn, "INSERT INTO interventions (id, kind, description, state, proposed_by, proposed_at, origin_trust, trust, created_at, updated_at)
                          VALUES ('int-check', 'k', 'd', 'APPROVED', 'model-1', 1, 'MODEL_OUTPUT', 'MODEL_OUTPUT', 1, 1)"),
            StorageError::CheckViolation(m) if m.contains("MODEL_OUTPUT")
        ));
        // ... trust cannot be elevated outside a review, and the origin is history
        assert!(matches!(
            err_of(
                conn,
                "UPDATE interventions SET trust = 'OPERATOR' WHERE id = 'int-1'"
            ),
            StorageError::Immutable(_)
        ));
        assert!(matches!(
            err_of(
                conn,
                "UPDATE interventions SET origin_trust = 'OPERATOR' WHERE id = 'int-1'"
            ),
            StorageError::Immutable(_)
        ));
        assert!(matches!(
            err_of(
                conn,
                "UPDATE interventions SET proposed_by = 'operator-1' WHERE id = 'int-1'"
            ),
            StorageError::Immutable(_)
        ));

        // the operator may reject a model proposal without reviewing it; trust stays MODEL_OUTPUT
        let (state, r2) = transition_intervention(
            conn,
            &row.id,
            Revision::INITIAL,
            InterventionEvent::Reject,
            &InterventionPatch::default(),
            t(2),
        )
        .unwrap();
        assert_eq!(state, InterventionState::Rejected);
        assert_eq!(r2, Revision::new(2));
        let rejected = get_intervention(conn, &learning(), &row.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            (rejected.trust, rejected.origin_trust),
            (TrustClass::ModelOutput, TrustClass::ModelOutput)
        );
        assert_eq!(rejected.reviewed_by, None);

        // a plan without a declared acceptance cannot be reviewed
        let mut vague = intervention("int-vague", TrustClass::ModelOutput, "model-1");
        vague.acceptance = String::new();
        insert_intervention(conn, &vague).unwrap();
        assert!(matches!(
            review_intervention(
                conn,
                &vague.id,
                Revision::INITIAL,
                &Actor::operator(operator_id()),
                t(3)
            ),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));

        // the generalised rule covers every non-authoritative origin, not only MODEL_OUTPUT
        insert_intervention(
            conn,
            &intervention("int-skill", TrustClass::SkillPackage, "skill-actor"),
        )
        .unwrap();
        assert!(matches!(
            err_of(conn, "UPDATE interventions SET state = 'REVIEWED', reviewed_by = 'operator-1', reviewed_at = 5 WHERE id = 'int-skill'"),
            StorageError::Precondition(_)
        ));
        assert_eq!(
            get_intervention(
                conn,
                &learning(),
                &InterventionId::parse("int-skill").unwrap()
            )
            .unwrap()
            .unwrap()
            .state,
            InterventionState::Proposed
        );
    }

    #[test]
    fn interventions_follow_the_state_machine_and_outcomes_are_immutable() {
        let db = db();
        let conn = db.conn();
        seed(conn);
        let operator = Actor::operator(operator_id());
        // a model proposal adopted through an operator review runs the full loop
        let row = intervention("int-1", TrustClass::ModelOutput, "model-1");
        insert_intervention(conn, &row).unwrap();
        assert_eq!(
            list_interventions(conn, &learning(), Some(InterventionState::Proposed))
                .unwrap()
                .len(),
            1
        );
        let (state, r) =
            review_intervention(conn, &row.id, Revision::INITIAL, &operator, t(1)).unwrap();
        assert_eq!((state, r), (InterventionState::Reviewed, Revision::new(2)));
        let reviewed = get_intervention(conn, &learning(), &row.id)
            .unwrap()
            .unwrap();
        assert_eq!(reviewed.trust, TrustClass::Operator);
        assert_eq!(
            reviewed.origin_trust,
            TrustClass::ModelOutput,
            "the proposer's trust is kept for display"
        );
        assert_eq!(reviewed.proposed_by.as_str(), "model-1");
        assert_eq!(reviewed.reviewed_by, Some(operator_id()));
        assert_eq!(reviewed.reviewed_at, Some(t(1)));
        // a recorded review is not rewritten outside a review step
        assert!(matches!(
            err_of(
                conn,
                "UPDATE interventions SET reviewed_by = 'someone-else' WHERE id = 'int-1'"
            ),
            StorageError::Immutable(_)
        ));

        let apr = approval(conn, "apr-1");
        let patch = InterventionPatch {
            approval_id: Some(apr),
            scheduled_for: None,
            applied_at: None,
        };
        assert!(matches!(
            transition_intervention(
                conn,
                &row.id,
                Revision::INITIAL,
                InterventionEvent::Approve,
                &patch,
                t(2)
            ),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        let (_, r) =
            transition_intervention(conn, &row.id, r, InterventionEvent::Approve, &patch, t(2))
                .unwrap();
        // the reviewed plan is frozen
        let plan = InterventionPlan {
            description: "something else".into(),
            steps: vec!["x".into()],
            time_budget: DurationMs::new(1).unwrap(),
            acceptance: "y".into(),
        };
        assert!(matches!(
            update_intervention_plan(conn, &row.id, r, &plan, t(3)),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        assert!(matches!(
            err_of(
                conn,
                "UPDATE interventions SET acceptance = 'weaker' WHERE id = 'int-1'"
            ),
            StorageError::Immutable(_)
        ));
        let (_, r) = transition_intervention(
            conn,
            &row.id,
            r,
            InterventionEvent::Schedule,
            &InterventionPatch {
                approval_id: None,
                scheduled_for: Some(t(100)),
                applied_at: None,
            },
            t(6),
        )
        .unwrap();
        let (state, r) = transition_intervention(
            conn,
            &row.id,
            r,
            InterventionEvent::Apply,
            &InterventionPatch {
                approval_id: None,
                scheduled_for: None,
                applied_at: Some(t(100)),
            },
            t(100),
        )
        .unwrap();
        assert_eq!(state, InterventionState::Applied);
        let applied = get_intervention(conn, &learning(), &row.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            applied.approval_id.as_ref().map(|a| a.as_str()),
            Some("apr-1")
        );
        assert_eq!(applied.scheduled_for, Some(t(100)));
        assert_eq!(applied.applied_at, Some(t(100)));
        assert_eq!(applied.revision, r);

        // revise loop: a revised plan is edited and re-reviewed (a fresh review is recorded)
        let (_, r) = transition_intervention(
            conn,
            &row.id,
            r,
            InterventionEvent::Retest,
            &InterventionPatch::default(),
            t(110),
        )
        .unwrap();
        let (_, r) = transition_intervention(
            conn,
            &row.id,
            r,
            InterventionEvent::Evaluate,
            &InterventionPatch::default(),
            t(111),
        )
        .unwrap();
        let (state, r) = transition_intervention(
            conn,
            &row.id,
            r,
            InterventionEvent::Revise,
            &InterventionPatch::default(),
            t(112),
        )
        .unwrap();
        assert_eq!(state, InterventionState::Revised);
        let r = update_intervention_plan(conn, &row.id, r, &plan, t(113)).unwrap();
        let (state, r) = transition_intervention(
            conn,
            &row.id,
            r,
            InterventionEvent::Propose,
            &InterventionPatch::default(),
            t(114),
        )
        .unwrap();
        assert_eq!(state, InterventionState::Proposed);
        let (_, r) = review_intervention(conn, &row.id, r, &operator, t(115)).unwrap();
        let rereviewed = get_intervention(conn, &learning(), &row.id)
            .unwrap()
            .unwrap();
        assert_eq!(rereviewed.reviewed_at, Some(t(115)));
        assert_eq!(rereviewed.description, "something else");
        assert_eq!(rereviewed.origin_trust, TrustClass::ModelOutput);
        assert_eq!(rereviewed.revision, r);

        let r = invalidate_intervention(conn, &row.id, r, "hypothesis rejected", t(120)).unwrap();
        assert!(matches!(
            invalidate_intervention(conn, &row.id, r, "again", t(121)),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        assert_eq!(
            get_intervention(conn, &learning(), &row.id)
                .unwrap()
                .unwrap()
                .revision,
            r
        );

        // scope: a learning row is refused to a business scope and filtered out of its list
        let business = ReadScope::domain(DataDomain::Business);
        assert!(matches!(
            get_intervention(conn, &business, &row.id),
            Err(RepoError::CrossDomainRefused {
                table: "interventions",
                ..
            })
        ));
        assert!(list_interventions(conn, &business, None)
            .unwrap()
            .is_empty());
        assert_eq!(
            list_interventions(conn, &ReadScope::cross_domain(DataDomain::Business), None)
                .unwrap()
                .len(),
            1
        );

        let outcome = InterventionOutcomeRow {
            id: "io-1".into(),
            intervention_id: row.id.clone(),
            measured_at: t(200),
            metric: "first_attempt_pass_rate".into(),
            baseline_value: Some(0.5),
            observed_value: Some(0.7),
            effect_size: Some(0.2),
            uncertainty: Some(serde_json::json!({ "n": 4 })),
            verdict: OutcomeVerdict::Inconclusive,
            created_at: t(200),
        };
        append_intervention_outcome(conn, &outcome).unwrap();
        assert_eq!(
            list_intervention_outcomes(conn, &row.id).unwrap(),
            vec![outcome]
        );
        assert_immutable(
            conn,
            "UPDATE intervention_outcomes SET verdict = 'IMPROVED' WHERE id = 'io-1'",
        );
        assert_immutable(conn, "DELETE FROM intervention_outcomes WHERE id = 'io-1'");
    }

    #[test]
    fn experiments_are_pre_registered_and_move_forward_only() {
        let db = db();
        let conn = db.conn();
        let row = ExperimentRow {
            id: ExperimentId::parse("exp-1").unwrap(),
            title: "Worked examples vs. retrieval drills".into(),
            hypothesis: "retrieval drills raise 7-day recall".into(),
            intervention_id: None,
            design: serde_json::json!({ "unit": "week", "allocation_seed": 42, "stop_rule": "8 weeks" }),
            started_at: None,
            ended_at: None,
            state: ExperimentState::Planned,
            result: None,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        };
        insert_experiment(conn, &row).unwrap();
        assert_eq!(get_experiment(conn, &row.id).unwrap().unwrap(), row);
        assert!(matches!(
            insert_experiment(
                conn,
                &ExperimentRow {
                    id: ExperimentId::parse("exp-x").unwrap(),
                    state: ExperimentState::Running,
                    ..row.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        // result before analysis is refused
        assert!(matches!(
            advance_experiment(
                conn,
                &row.id,
                Revision::INITIAL,
                ExperimentState::Running,
                Some(t(1)),
                None,
                Some(&serde_json::json!({})),
                t(1)
            ),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        let r2 = advance_experiment(
            conn,
            &row.id,
            Revision::INITIAL,
            ExperimentState::Running,
            Some(t(1)),
            None,
            None,
            t(1),
        )
        .unwrap();
        assert!(matches!(
            advance_experiment(
                conn,
                &row.id,
                r2,
                ExperimentState::Planned,
                None,
                None,
                None,
                t(2)
            ),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        let result = serde_json::json!({ "effect": 0.1, "ci": [0.0, 0.2] });
        let r3 = advance_experiment(
            conn,
            &row.id,
            r2,
            ExperimentState::Analyzed,
            None,
            Some(t(50)),
            Some(&result),
            t(50),
        )
        .unwrap();
        let analyzed = get_experiment(conn, &row.id).unwrap().unwrap();
        assert_eq!(analyzed.result, Some(result));
        assert_eq!(analyzed.started_at, Some(t(1)));
        assert_eq!(analyzed.ended_at, Some(t(50)));
        // the design is exactly what was pre-registered
        assert_eq!(analyzed.design, row.design);
        assert_eq!(analyzed.revision, r3);
        assert_eq!(
            list_experiments(conn, Some(ExperimentState::Analyzed))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(list_experiments(conn, None).unwrap().len(), 1);
    }
}
