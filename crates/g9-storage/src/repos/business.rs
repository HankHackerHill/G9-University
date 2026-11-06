//! Executive workspace repository (M14; migration `0009_business`, tables
//! `goals`, `projects`, `milestones`, `milestone_dependencies`, `commitments`,
//! `execution_observations`, `decisions`, `decision_outcomes`, `opt_in_reflections`).
//!
//! Every row carries `data_domain` (`BUSINESS` by default, `PERSONAL` for
//! reflections) and every read goes through [`ReadScope`]: a learning-scoped
//! command cannot read business rows unless `g9-policy` granted
//! `Capability::CrossDomainRead` (R-017, SEC-32). `execution_observations` and
//! `decision_outcomes` are immutable measurements; a revisited decision is a new
//! row linked by `supersedes`; model output can only propose decisions and is
//! never an execution measurement (DDL CHECKs, mirrored here). Reflections are
//! erased through [`crate::deletion`], never deleted.

use super::{
    finish_update, from_json, from_ms, i64_bool, ms, opt_from_ms, opt_ms, opt_parse_id, query_all,
    query_opt, required, rev_from_i64, rev_i64, to_json, u32_from_i64, u8_from_i64, ReadScope,
    RepoError, RepoResult,
};
use crate::error::{StorageError, StorageResult};
use crate::sql::{enum_from_text, enum_to_text, parse_id};
use g9_calendar::CivilDate;
use g9_domain::hashing::sha256_hex;
use g9_domain::{
    Actor, ActorId, CalendarEventId, Classification, DataDomain, DecisionId, GoalId, IanaZone,
    MilestoneId, ProjectId, Provenance, Revision, SessionId, SourceKind, TrustClass, UtcMillis,
};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// goals
// ---------------------------------------------------------------------------

/// Goal horizon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GoalHorizon {
    /// Week.
    Week,
    /// Month.
    Month,
    /// Quarter.
    Quarter,
    /// Year.
    Year,
    /// Whole program.
    Program,
}

/// Goal lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GoalState {
    /// Proposed.
    Proposed,
    /// Active.
    Active,
    /// Achieved.
    Achieved,
    /// Dropped.
    Dropped,
}

/// A `goals` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GoalRow {
    /// Goal id.
    pub id: GoalId,
    /// Title.
    pub title: String,
    /// Description.
    pub description: Option<String>,
    /// Horizon.
    pub horizon: GoalHorizon,
    /// Priority 1 (highest) .. 5.
    pub priority: u8,
    /// Lifecycle.
    pub state: GoalState,
    /// Parent goal.
    pub parent_goal_id: Option<GoalId>,
    /// Target civil date with its zone (both or neither).
    pub target_date: Option<(CivilDate, IanaZone)>,
    /// Owner.
    pub owner_id: ActorId,
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

const GOAL_COLUMNS: &str = "id, title, description, horizon, priority, state, parent_goal_id, target_date_civil, zone, owner_id,
    data_domain, classification, revision, created_at, updated_at";

fn civil_date_text(d: &CivilDate) -> String {
    format!("{:04}-{:02}-{:02}", d.year, d.month, d.day)
}

fn civil_date_parse(text: &str) -> StorageResult<CivilDate> {
    let bad = || StorageError::RowDecode("civil date column".to_string());
    let mut parts = text.split('-');
    let year: i32 = parts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let month: u8 = parts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    let day: u8 = parts.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    if parts.next().is_some() {
        return Err(bad());
    }
    CivilDate::new(year, month, day).map_err(|_| bad())
}

fn map_goal(r: &Row<'_>) -> StorageResult<GoalRow> {
    let target = match (
        r.get::<_, Option<String>>("target_date_civil")?,
        r.get::<_, Option<String>>("zone")?,
    ) {
        (Some(d), Some(z)) => Some((
            civil_date_parse(&d)?,
            IanaZone::parse(&z).map_err(|e| StorageError::RowDecode(e.to_string()))?,
        )),
        _ => None,
    };
    Ok(GoalRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        title: r.get("title")?,
        description: r.get("description")?,
        horizon: enum_from_text(&r.get::<_, String>("horizon")?)?,
        priority: u8_from_i64(r.get("priority")?, "priority")?,
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        parent_goal_id: opt_parse_id(r.get("parent_goal_id")?)?,
        target_date: target,
        owner_id: parse_id(&r.get::<_, String>("owner_id")?)?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert a goal.
pub fn insert_goal(conn: &Connection, row: &GoalRow) -> RepoResult<Revision> {
    conn.execute(
        &format!(
            "INSERT INTO goals ({GOAL_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 1, ?13, ?13)"
        ),
        params![
            row.id.as_str(),
            row.title,
            row.description,
            enum_to_text(&row.horizon)?,
            i64::from(row.priority),
            enum_to_text(&row.state)?,
            row.parent_goal_id.as_ref().map(|g| g.as_str().to_string()),
            row.target_date.as_ref().map(|(d, _)| civil_date_text(d)),
            row.target_date.as_ref().map(|(_, z)| z.as_str().to_string()),
            row.owner_id.as_str(),
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One goal, scope-checked.
pub fn get_goal(conn: &Connection, scope: &ReadScope, id: &GoalId) -> RepoResult<Option<GoalRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {GOAL_COLUMNS} FROM goals WHERE id = ?1"),
        &[&id.as_str()],
        map_goal,
    )?;
    if let Some(r) = &row {
        scope.check("goals", r.data_domain)?;
    }
    Ok(row)
}

/// Goals in a state (or all), domain-filtered in SQL, by priority.
pub fn list_goals(
    conn: &Connection,
    scope: &ReadScope,
    state: Option<GoalState>,
) -> RepoResult<Vec<GoalRow>> {
    let state_text = state.as_ref().map(enum_to_text).transpose()?;
    query_all(
        conn,
        &format!(
            "SELECT {GOAL_COLUMNS} FROM goals WHERE (?1 IS NULL OR state = ?1) AND (?2 = 1 OR data_domain = ?3)
             ORDER BY priority, created_at, id"
        ),
        &[&state_text, &scope.sql_flag(), &scope.sql_domain()?],
        map_goal,
    )
}

/// Replace the mutable columns of a goal (everything but id, owner, domain, created_at).
pub fn update_goal(
    conn: &Connection,
    row: &GoalRow,
    expected_revision: Revision,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let changed = conn.execute(
        "UPDATE goals SET title = ?3, description = ?4, horizon = ?5, priority = ?6, state = ?7, parent_goal_id = ?8,
             target_date_civil = ?9, zone = ?10, classification = ?11, revision = revision + 1, updated_at = ?12
         WHERE id = ?1 AND revision = ?2",
        params![
            row.id.as_str(),
            rev_i64(expected_revision),
            row.title,
            row.description,
            enum_to_text(&row.horizon)?,
            i64::from(row.priority),
            enum_to_text(&row.state)?,
            row.parent_goal_id.as_ref().map(|g| g.as_str().to_string()),
            row.target_date.as_ref().map(|(d, _)| civil_date_text(d)),
            row.target_date.as_ref().map(|(_, z)| z.as_str().to_string()),
            enum_to_text(&row.classification)?,
            ms(now),
        ],
    )?;
    finish_update(
        conn,
        "goals",
        "id",
        row.id.as_str(),
        expected_revision,
        changed,
    )
}

// ---------------------------------------------------------------------------
// projects and milestones
// ---------------------------------------------------------------------------

/// Project lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProjectState {
    /// Planned.
    Planned,
    /// Active.
    Active,
    /// Blocked.
    Blocked,
    /// Done.
    Done,
    /// Cancelled.
    Cancelled,
}

/// A `projects` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectRow {
    /// Project id.
    pub id: ProjectId,
    /// Goal served.
    pub goal_id: Option<GoalId>,
    /// Title.
    pub title: String,
    /// Description.
    pub description: Option<String>,
    /// Lifecycle.
    pub state: ProjectState,
    /// Start.
    pub starts_at: Option<UtcMillis>,
    /// End.
    pub ends_at: Option<UtcMillis>,
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

const PROJECT_COLUMNS: &str =
    "id, goal_id, title, description, state, starts_at, ends_at, data_domain, classification, revision, created_at, updated_at";

fn map_project(r: &Row<'_>) -> StorageResult<ProjectRow> {
    Ok(ProjectRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        goal_id: opt_parse_id(r.get("goal_id")?)?,
        title: r.get("title")?,
        description: r.get("description")?,
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        starts_at: opt_from_ms(r.get("starts_at")?),
        ends_at: opt_from_ms(r.get("ends_at")?),
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert a project.
pub fn insert_project(conn: &Connection, row: &ProjectRow) -> RepoResult<Revision> {
    conn.execute(
        &format!("INSERT INTO projects ({PROJECT_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, ?10, ?10)"),
        params![
            row.id.as_str(),
            row.goal_id.as_ref().map(|g| g.as_str().to_string()),
            row.title,
            row.description,
            enum_to_text(&row.state)?,
            opt_ms(row.starts_at),
            opt_ms(row.ends_at),
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One project, scope-checked.
pub fn get_project(
    conn: &Connection,
    scope: &ReadScope,
    id: &ProjectId,
) -> RepoResult<Option<ProjectRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {PROJECT_COLUMNS} FROM projects WHERE id = ?1"),
        &[&id.as_str()],
        map_project,
    )?;
    if let Some(r) = &row {
        scope.check("projects", r.data_domain)?;
    }
    Ok(row)
}

/// Projects of a goal (or all when `None`), domain-filtered in SQL.
pub fn list_projects(
    conn: &Connection,
    scope: &ReadScope,
    goal_id: Option<&GoalId>,
) -> RepoResult<Vec<ProjectRow>> {
    let goal = goal_id.map(|g| g.as_str().to_string());
    query_all(
        conn,
        &format!(
            "SELECT {PROJECT_COLUMNS} FROM projects WHERE (?1 IS NULL OR goal_id = ?1) AND (?2 = 1 OR data_domain = ?3)
             ORDER BY created_at, id"
        ),
        &[&goal, &scope.sql_flag(), &scope.sql_domain()?],
        map_project,
    )
}

/// Replace the mutable columns of a project.
pub fn update_project(
    conn: &Connection,
    row: &ProjectRow,
    expected_revision: Revision,
    now: UtcMillis,
) -> RepoResult<Revision> {
    let changed = conn.execute(
        "UPDATE projects SET goal_id = ?3, title = ?4, description = ?5, state = ?6, starts_at = ?7, ends_at = ?8,
             classification = ?9, revision = revision + 1, updated_at = ?10
         WHERE id = ?1 AND revision = ?2",
        params![
            row.id.as_str(),
            rev_i64(expected_revision),
            row.goal_id.as_ref().map(|g| g.as_str().to_string()),
            row.title,
            row.description,
            enum_to_text(&row.state)?,
            opt_ms(row.starts_at),
            opt_ms(row.ends_at),
            enum_to_text(&row.classification)?,
            ms(now),
        ],
    )?;
    finish_update(
        conn,
        "projects",
        "id",
        row.id.as_str(),
        expected_revision,
        changed,
    )
}

/// Milestone lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MilestoneState {
    /// Planned.
    Planned,
    /// Due.
    Due,
    /// Done.
    Done,
    /// Missed.
    Missed,
    /// Cancelled.
    Cancelled,
}

/// A `milestones` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MilestoneRow {
    /// Milestone id.
    pub id: MilestoneId,
    /// Project.
    pub project_id: ProjectId,
    /// Title.
    pub title: String,
    /// Description.
    pub description: Option<String>,
    /// Due instant.
    pub due_at: Option<UtcMillis>,
    /// Lifecycle (`DONE` requires `completed_at`).
    pub state: MilestoneState,
    /// Completion instant.
    pub completed_at: Option<UtcMillis>,
    /// Order within the project.
    pub ordinal: u32,
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

const MILESTONE_COLUMNS: &str =
    "id, project_id, title, description, due_at, state, completed_at, ordinal, data_domain,
    classification, revision, created_at, updated_at";

fn map_milestone(r: &Row<'_>) -> StorageResult<MilestoneRow> {
    Ok(MilestoneRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        project_id: parse_id(&r.get::<_, String>("project_id")?)?,
        title: r.get("title")?,
        description: r.get("description")?,
        due_at: opt_from_ms(r.get("due_at")?),
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        completed_at: opt_from_ms(r.get("completed_at")?),
        ordinal: u32_from_i64(r.get("ordinal")?, "ordinal")?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert a milestone.
pub fn insert_milestone(conn: &Connection, row: &MilestoneRow) -> RepoResult<Revision> {
    conn.execute(
        &format!(
            "INSERT INTO milestones ({MILESTONE_COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1, ?11, ?11)"
        ),
        params![
            row.id.as_str(),
            row.project_id.as_str(),
            row.title,
            row.description,
            opt_ms(row.due_at),
            enum_to_text(&row.state)?,
            opt_ms(row.completed_at),
            i64::from(row.ordinal),
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One milestone, scope-checked.
pub fn get_milestone(
    conn: &Connection,
    scope: &ReadScope,
    id: &MilestoneId,
) -> RepoResult<Option<MilestoneRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {MILESTONE_COLUMNS} FROM milestones WHERE id = ?1"),
        &[&id.as_str()],
        map_milestone,
    )?;
    if let Some(r) = &row {
        scope.check("milestones", r.data_domain)?;
    }
    Ok(row)
}

/// Milestones of a project in ordinal order, domain-filtered in SQL.
pub fn list_milestones(
    conn: &Connection,
    scope: &ReadScope,
    project_id: &ProjectId,
) -> RepoResult<Vec<MilestoneRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {MILESTONE_COLUMNS} FROM milestones WHERE project_id = ?1 AND (?2 = 1 OR data_domain = ?3)
             ORDER BY ordinal, id"
        ),
        &[&project_id.as_str(), &scope.sql_flag(), &scope.sql_domain()?],
        map_milestone,
    )
}

/// Replace the mutable columns of a milestone.
pub fn update_milestone(
    conn: &Connection,
    row: &MilestoneRow,
    expected_revision: Revision,
    now: UtcMillis,
) -> RepoResult<Revision> {
    if row.state == MilestoneState::Done && row.completed_at.is_none() {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "DONE requires completed_at".to_string(),
        )));
    }
    let changed = conn.execute(
        "UPDATE milestones SET title = ?3, description = ?4, due_at = ?5, state = ?6, completed_at = ?7, ordinal = ?8,
             classification = ?9, revision = revision + 1, updated_at = ?10
         WHERE id = ?1 AND revision = ?2",
        params![
            row.id.as_str(),
            rev_i64(expected_revision),
            row.title,
            row.description,
            opt_ms(row.due_at),
            enum_to_text(&row.state)?,
            opt_ms(row.completed_at),
            i64::from(row.ordinal),
            enum_to_text(&row.classification)?,
            ms(now),
        ],
    )?;
    finish_update(
        conn,
        "milestones",
        "id",
        row.id.as_str(),
        expected_revision,
        changed,
    )
}

/// Dependency kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DependencyKind {
    /// `from` blocks `to`.
    Blocks,
    /// `from` requires `to`.
    Requires,
    /// Related.
    Related,
}

/// A `milestone_dependencies` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MilestoneDependencyRow {
    /// Row id.
    pub id: String,
    /// Source milestone.
    pub from_milestone_id: MilestoneId,
    /// Target milestone.
    pub to_milestone_id: MilestoneId,
    /// Kind.
    pub kind: DependencyKind,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Insert a dependency (`CONFLICT` on duplicate `(from, to, kind)`).
pub fn insert_dependency(conn: &Connection, row: &MilestoneDependencyRow) -> RepoResult<()> {
    conn.execute(
        "INSERT INTO milestone_dependencies (id, from_milestone_id, to_milestone_id, kind, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            row.id,
            row.from_milestone_id.as_str(),
            row.to_milestone_id.as_str(),
            enum_to_text(&row.kind)?,
            ms(row.created_at)
        ],
    )?;
    Ok(())
}

/// Remove a dependency explicitly.
pub fn delete_dependency(conn: &Connection, id: &str) -> RepoResult<()> {
    let changed = conn.execute(
        "DELETE FROM milestone_dependencies WHERE id = ?1",
        params![id],
    )?;
    if changed == 0 {
        return Err(RepoError::Storage(StorageError::NotFound(
            "milestone_dependencies",
        )));
    }
    Ok(())
}

/// Dependencies touching a milestone (either end).
pub fn dependencies_of(
    conn: &Connection,
    milestone_id: &MilestoneId,
) -> RepoResult<Vec<MilestoneDependencyRow>> {
    query_all(
        conn,
        "SELECT id, from_milestone_id, to_milestone_id, kind, created_at FROM milestone_dependencies
         WHERE from_milestone_id = ?1 OR to_milestone_id = ?1 ORDER BY created_at, id",
        &[&milestone_id.as_str()],
        |r| {
            Ok(MilestoneDependencyRow {
                id: r.get("id")?,
                from_milestone_id: parse_id(&r.get::<_, String>("from_milestone_id")?)?,
                to_milestone_id: parse_id(&r.get::<_, String>("to_milestone_id")?)?,
                kind: enum_from_text(&r.get::<_, String>("kind")?)?,
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

// ---------------------------------------------------------------------------
// commitments
// ---------------------------------------------------------------------------

/// Commitment lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommitmentState {
    /// Open.
    Open,
    /// Kept.
    Kept,
    /// Missed.
    Missed,
    /// Renegotiated.
    Renegotiated,
    /// Cancelled.
    Cancelled,
}

/// A `commitments` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitmentRow {
    /// Row id.
    pub id: String,
    /// Project.
    pub project_id: Option<ProjectId>,
    /// Milestone.
    pub milestone_id: Option<MilestoneId>,
    /// Mirrored calendar event.
    pub calendar_event_id: Option<CalendarEventId>,
    /// Title.
    pub title: String,
    /// Description.
    pub description: Option<String>,
    /// Due instant.
    pub due_at: UtcMillis,
    /// Lifecycle (`OPEN` has no `resolved_at`; every other state needs one).
    pub state: CommitmentState,
    /// Resolution instant.
    pub resolved_at: Option<UtcMillis>,
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

const COMMITMENT_COLUMNS: &str = "id, project_id, milestone_id, calendar_event_id, title, description, due_at, state, resolved_at,
    data_domain, classification, revision, created_at, updated_at";

fn map_commitment(r: &Row<'_>) -> StorageResult<CommitmentRow> {
    Ok(CommitmentRow {
        id: r.get("id")?,
        project_id: opt_parse_id(r.get("project_id")?)?,
        milestone_id: opt_parse_id(r.get("milestone_id")?)?,
        calendar_event_id: opt_parse_id(r.get("calendar_event_id")?)?,
        title: r.get("title")?,
        description: r.get("description")?,
        due_at: from_ms(r.get("due_at")?),
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        resolved_at: opt_from_ms(r.get("resolved_at")?),
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert an `OPEN` commitment.
pub fn insert_commitment(conn: &Connection, row: &CommitmentRow) -> RepoResult<Revision> {
    if row.state != CommitmentState::Open {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "a new commitment is OPEN".to_string(),
        )));
    }
    conn.execute(
        &format!(
            "INSERT INTO commitments ({COMMITMENT_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'OPEN', NULL, ?8, ?9, 1, ?10, ?10)"
        ),
        params![
            row.id,
            row.project_id.as_ref().map(|p| p.as_str().to_string()),
            row.milestone_id.as_ref().map(|m| m.as_str().to_string()),
            row.calendar_event_id
                .as_ref()
                .map(|c| c.as_str().to_string()),
            row.title,
            row.description,
            ms(row.due_at),
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One commitment, scope-checked.
pub fn get_commitment(
    conn: &Connection,
    scope: &ReadScope,
    id: &str,
) -> RepoResult<Option<CommitmentRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {COMMITMENT_COLUMNS} FROM commitments WHERE id = ?1"),
        &[&id],
        map_commitment,
    )?;
    if let Some(r) = &row {
        scope.check("commitments", r.data_domain)?;
    }
    Ok(row)
}

/// Open commitments due before `before`, earliest first, domain-filtered in SQL.
pub fn list_open_commitments(
    conn: &Connection,
    scope: &ReadScope,
    before: UtcMillis,
) -> RepoResult<Vec<CommitmentRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {COMMITMENT_COLUMNS} FROM commitments WHERE state = 'OPEN' AND due_at < ?1 AND (?2 = 1 OR data_domain = ?3)
             ORDER BY due_at, id"
        ),
        &[&ms(before), &scope.sql_flag(), &scope.sql_domain()?],
        map_commitment,
    )
}

/// Resolve an open commitment (`KEPT`, `MISSED`, `RENEGOTIATED`, `CANCELLED`).
pub fn resolve_commitment(
    conn: &Connection,
    id: &str,
    expected_revision: Revision,
    state: CommitmentState,
    resolved_at: UtcMillis,
) -> RepoResult<Revision> {
    if state == CommitmentState::Open {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "a commitment cannot be resolved to OPEN".to_string(),
        )));
    }
    let changed = conn.execute(
        "UPDATE commitments SET state = ?3, resolved_at = ?4, revision = revision + 1, updated_at = ?4
         WHERE id = ?1 AND revision = ?2 AND state = 'OPEN'",
        params![id, rev_i64(expected_revision), enum_to_text(&state)?, ms(resolved_at)],
    )?;
    finish_update(conn, "commitments", "id", id, expected_revision, changed)
}

// ---------------------------------------------------------------------------
// execution observations
// ---------------------------------------------------------------------------

/// An `execution_observations` row (immutable measurement).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionObservationRow {
    /// Row id.
    pub id: String,
    /// Project.
    pub project_id: Option<ProjectId>,
    /// Milestone.
    pub milestone_id: Option<MilestoneId>,
    /// Commitment.
    pub commitment_id: Option<String>,
    /// Metric name.
    pub metric: String,
    /// Value.
    pub value: f64,
    /// Unit.
    pub unit: String,
    /// When observed.
    pub observed_at: UtcMillis,
    /// Provenance (`trust` must not be `MODEL_OUTPUT`).
    pub provenance: Provenance,
    /// Privacy domain.
    pub data_domain: DataDomain,
    /// Classification.
    pub classification: Classification,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Append a measurement (never updated or deleted; trigger-enforced). Model
/// output is refused before the CHECK constraint sees it.
pub fn append_execution_observation(
    conn: &Connection,
    row: &ExecutionObservationRow,
) -> RepoResult<()> {
    if row.provenance.trust == TrustClass::ModelOutput {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "model output is never an execution measurement".to_string(),
        )));
    }
    conn.execute(
        "INSERT INTO execution_observations (id, project_id, milestone_id, commitment_id, metric, value, unit, observed_at,
            source_kind, trust, provenance_json, data_domain, classification, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            row.id,
            row.project_id.as_ref().map(|p| p.as_str().to_string()),
            row.milestone_id.as_ref().map(|m| m.as_str().to_string()),
            row.commitment_id,
            row.metric,
            row.value,
            row.unit,
            ms(row.observed_at),
            enum_to_text(&row.provenance.source_kind)?,
            enum_to_text(&row.provenance.trust)?,
            to_json(&row.provenance)?,
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

/// Measurements of a project in `[from, to)`, domain-filtered in SQL.
pub fn list_execution_observations(
    conn: &Connection,
    scope: &ReadScope,
    project_id: &ProjectId,
    from: UtcMillis,
    to: UtcMillis,
) -> RepoResult<Vec<ExecutionObservationRow>> {
    query_all(
        conn,
        "SELECT id, project_id, milestone_id, commitment_id, metric, value, unit, observed_at, provenance_json, data_domain,
                classification, created_at
         FROM execution_observations
         WHERE project_id = ?1 AND observed_at >= ?2 AND observed_at < ?3 AND (?4 = 1 OR data_domain = ?5)
         ORDER BY observed_at, id",
        &[
            &project_id.as_str(),
            &ms(from),
            &ms(to),
            &scope.sql_flag(),
            &scope.sql_domain()?,
        ],
        |r| {
            Ok(ExecutionObservationRow {
                id: r.get("id")?,
                project_id: opt_parse_id(r.get("project_id")?)?,
                milestone_id: opt_parse_id(r.get("milestone_id")?)?,
                commitment_id: r.get("commitment_id")?,
                metric: r.get("metric")?,
                value: r.get("value")?,
                unit: r.get("unit")?,
                observed_at: from_ms(r.get("observed_at")?),
                provenance: from_json(&r.get::<_, String>("provenance_json")?, "provenance_json")?,
                data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
                classification: enum_from_text(&r.get::<_, String>("classification")?)?,
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

// ---------------------------------------------------------------------------
// decisions
// ---------------------------------------------------------------------------

/// Decision lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DecisionState {
    /// Proposed (the only state model output can create).
    Proposed,
    /// Decided by the operator.
    Decided,
    /// Revisited (a superseding row exists or is expected).
    Revisited,
    /// Reversed.
    Reversed,
}

/// A `decisions` row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionRow {
    /// Decision id.
    pub id: DecisionId,
    /// Goal.
    pub goal_id: Option<GoalId>,
    /// Project.
    pub project_id: Option<ProjectId>,
    /// Title.
    pub title: String,
    /// Context.
    pub context: String,
    /// Options considered.
    pub options: Vec<serde_json::Value>,
    /// Chosen option (required once decided).
    pub chosen_option: Option<String>,
    /// Rationale.
    pub rationale: Option<String>,
    /// Lifecycle.
    pub state: DecisionState,
    /// Deciding operator.
    pub decided_by: Option<ActorId>,
    /// Decision instant.
    pub decided_at: Option<UtcMillis>,
    /// When to review the decision.
    pub review_due_at: Option<UtcMillis>,
    /// Prior decision this one supersedes.
    pub supersedes: Option<DecisionId>,
    /// Trust of the proposer.
    pub trust: TrustClass,
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

const DECISION_COLUMNS: &str = "id, goal_id, project_id, title, context, options_json, chosen_option, rationale, state, decided_by,
    decided_at, review_due_at, supersedes, trust, data_domain, classification, revision, created_at, updated_at";

fn map_decision(r: &Row<'_>) -> StorageResult<DecisionRow> {
    Ok(DecisionRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        goal_id: opt_parse_id(r.get("goal_id")?)?,
        project_id: opt_parse_id(r.get("project_id")?)?,
        title: r.get("title")?,
        context: r.get("context")?,
        options: from_json(&r.get::<_, String>("options_json")?, "options_json")?,
        chosen_option: r.get("chosen_option")?,
        rationale: r.get("rationale")?,
        state: enum_from_text(&r.get::<_, String>("state")?)?,
        decided_by: opt_parse_id(r.get("decided_by")?)?,
        decided_at: opt_from_ms(r.get("decided_at")?),
        review_due_at: opt_from_ms(r.get("review_due_at")?),
        supersedes: opt_parse_id(r.get("supersedes")?)?,
        trust: enum_from_text(&r.get::<_, String>("trust")?)?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert a decision: `PROPOSED` for any trust, or `DECIDED` by an operator
/// (model output can only propose; a decided row needs decider, instant and
/// chosen option — both rules are DDL CHECKs mirrored here).
pub fn insert_decision(conn: &Connection, row: &DecisionRow) -> RepoResult<Revision> {
    match row.state {
        DecisionState::Proposed => {}
        DecisionState::Decided => {
            if row.trust == TrustClass::ModelOutput {
                return Err(RepoError::Storage(StorageError::InvalidInput(
                    "model output can only propose a decision".to_string(),
                )));
            }
            if row.decided_by.is_none() || row.decided_at.is_none() || row.chosen_option.is_none() {
                return Err(RepoError::Storage(StorageError::InvalidInput(
                    "a DECIDED decision needs decided_by, decided_at and chosen_option".to_string(),
                )));
            }
        }
        DecisionState::Revisited | DecisionState::Reversed => {
            return Err(RepoError::Storage(StorageError::InvalidInput(
                "a new decision is PROPOSED or DECIDED".to_string(),
            )))
        }
    }
    conn.execute(
        &format!(
            "INSERT INTO decisions ({DECISION_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, 1, ?17, ?17)"
        ),
        params![
            row.id.as_str(),
            row.goal_id.as_ref().map(|g| g.as_str().to_string()),
            row.project_id.as_ref().map(|p| p.as_str().to_string()),
            row.title,
            row.context,
            to_json(&row.options)?,
            row.chosen_option,
            row.rationale,
            enum_to_text(&row.state)?,
            row.decided_by.as_ref().map(|d| d.as_str().to_string()),
            opt_ms(row.decided_at),
            opt_ms(row.review_due_at),
            row.supersedes.as_ref().map(|s| s.as_str().to_string()),
            enum_to_text(&row.trust)?,
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One decision, scope-checked.
pub fn get_decision(
    conn: &Connection,
    scope: &ReadScope,
    id: &DecisionId,
) -> RepoResult<Option<DecisionRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {DECISION_COLUMNS} FROM decisions WHERE id = ?1"),
        &[&id.as_str()],
        map_decision,
    )?;
    if let Some(r) = &row {
        scope.check("decisions", r.data_domain)?;
    }
    Ok(row)
}

/// Decisions in a state (or all), domain-filtered in SQL.
pub fn list_decisions(
    conn: &Connection,
    scope: &ReadScope,
    state: Option<DecisionState>,
) -> RepoResult<Vec<DecisionRow>> {
    let state_text = state.as_ref().map(enum_to_text).transpose()?;
    query_all(
        conn,
        &format!(
            "SELECT {DECISION_COLUMNS} FROM decisions WHERE (?1 IS NULL OR state = ?1) AND (?2 = 1 OR data_domain = ?3)
             ORDER BY created_at, id"
        ),
        &[&state_text, &scope.sql_flag(), &scope.sql_domain()?],
        map_decision,
    )
}

/// Operator decides a `PROPOSED` decision; the row's `trust` becomes `OPERATOR`.
///
/// `decider` is the backend-assigned session actor and must be the operator: a
/// model/skill proposal never becomes an operator decision without one.
#[allow(clippy::too_many_arguments)]
pub fn decide(
    conn: &Connection,
    id: &DecisionId,
    expected_revision: Revision,
    chosen_option: &str,
    rationale: Option<&str>,
    decider: &Actor,
    decided_at: UtcMillis,
    review_due_at: Option<UtcMillis>,
) -> RepoResult<Revision> {
    if !decider.is_operator() {
        return Err(RepoError::Storage(StorageError::Precondition(
            "business decisions are decided by the operator".to_string(),
        )));
    }
    let decided_by = &decider.id;
    super::require_non_empty("chosen_option", chosen_option)?;
    let changed = conn.execute(
        "UPDATE decisions SET state = 'DECIDED', chosen_option = ?3, rationale = ?4, decided_by = ?5, decided_at = ?6,
             review_due_at = ?7, trust = 'OPERATOR', revision = revision + 1, updated_at = ?6
         WHERE id = ?1 AND revision = ?2 AND state = 'PROPOSED'",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            chosen_option,
            rationale,
            decided_by.as_str(),
            ms(decided_at),
            opt_ms(review_due_at)
        ],
    )?;
    finish_update(
        conn,
        "decisions",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

/// Mark a `DECIDED` decision `REVISITED` or `REVERSED` (the replacement is a new
/// row whose `supersedes` names this one).
pub fn close_decision(
    conn: &Connection,
    id: &DecisionId,
    expected_revision: Revision,
    state: DecisionState,
    now: UtcMillis,
) -> RepoResult<Revision> {
    if !matches!(state, DecisionState::Revisited | DecisionState::Reversed) {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "close_decision accepts REVISITED or REVERSED".to_string(),
        )));
    }
    let changed = conn.execute(
        "UPDATE decisions SET state = ?3, revision = revision + 1, updated_at = ?4
         WHERE id = ?1 AND revision = ?2 AND state = 'DECIDED'",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            enum_to_text(&state)?,
            ms(now)
        ],
    )?;
    finish_update(
        conn,
        "decisions",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

/// Verdict on what happened after a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DecisionVerdict {
    /// Better than expected.
    Better,
    /// As expected.
    AsExpected,
    /// Worse.
    Worse,
    /// Unknown.
    Unknown,
}

/// A `decision_outcomes` row (immutable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionOutcomeRow {
    /// Row id.
    pub id: String,
    /// Decision.
    pub decision_id: DecisionId,
    /// When measured.
    pub measured_at: UtcMillis,
    /// Summary.
    pub summary: String,
    /// Verdict.
    pub verdict: DecisionVerdict,
    /// Recording operator.
    pub recorded_by: ActorId,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Append an outcome (never updated or deleted; trigger-enforced).
pub fn append_decision_outcome(conn: &Connection, row: &DecisionOutcomeRow) -> RepoResult<()> {
    conn.execute(
        "INSERT INTO decision_outcomes (id, decision_id, measured_at, summary, verdict, recorded_by, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            row.id,
            row.decision_id.as_str(),
            ms(row.measured_at),
            row.summary,
            enum_to_text(&row.verdict)?,
            row.recorded_by.as_str(),
            ms(row.created_at)
        ],
    )?;
    Ok(())
}

/// Outcomes of a decision, oldest first.
pub fn list_decision_outcomes(
    conn: &Connection,
    scope: &ReadScope,
    decision_id: &DecisionId,
) -> RepoResult<Vec<DecisionOutcomeRow>> {
    // outcomes carry no domain of their own; the decision's domain decides
    if !super::check_parent_domain(conn, scope, "decisions", "id", decision_id.as_str())? {
        return Ok(Vec::new());
    }
    query_all(
        conn,
        "SELECT id, decision_id, measured_at, summary, verdict, recorded_by, created_at FROM decision_outcomes
         WHERE decision_id = ?1 ORDER BY measured_at, id",
        &[&decision_id.as_str()],
        |r| {
            Ok(DecisionOutcomeRow {
                id: r.get("id")?,
                decision_id: parse_id(&r.get::<_, String>("decision_id")?)?,
                measured_at: from_ms(r.get("measured_at")?),
                summary: r.get("summary")?,
                verdict: enum_from_text(&r.get::<_, String>("verdict")?)?,
                recorded_by: parse_id(&r.get::<_, String>("recorded_by")?)?,
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

// ---------------------------------------------------------------------------
// opt-in reflections
// ---------------------------------------------------------------------------

/// An `opt_in_reflections` row (`PERSONAL`; erasable through `deletion_requests`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReflectionRow {
    /// Row id.
    pub id: String,
    /// Session reflected on.
    pub session_id: Option<SessionId>,
    /// When recorded.
    pub recorded_at: UtcMillis,
    /// Body (private content; never placed in errors).
    pub body: String,
    /// Hex SHA-256 of the body (set by the repository).
    pub body_sha256: String,
    /// Energy 1..=5.
    pub energy_level: Option<u8>,
    /// Focus 1..=5.
    pub focus_level: Option<u8>,
    /// Privacy domain (`PERSONAL`).
    pub data_domain: DataDomain,
    /// Classification.
    pub classification: Classification,
    /// Erased flag.
    pub erased: bool,
    /// Erasure instant.
    pub erased_at: Option<UtcMillis>,
    /// Erasure request id.
    pub erasure_request_id: Option<String>,
    /// Revision.
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

const REFLECTION_COLUMNS: &str =
    "id, session_id, recorded_at, body, body_sha256, energy_level, focus_level, data_domain,
    classification, erased, erased_at, erasure_request_id, revision, created_at, updated_at";

fn map_reflection(r: &Row<'_>) -> StorageResult<ReflectionRow> {
    Ok(ReflectionRow {
        id: r.get("id")?,
        session_id: opt_parse_id(r.get("session_id")?)?,
        recorded_at: from_ms(r.get("recorded_at")?),
        body: r.get("body")?,
        body_sha256: r.get("body_sha256")?,
        energy_level: r
            .get::<_, Option<i64>>("energy_level")?
            .map(|v| u8_from_i64(v, "energy_level"))
            .transpose()?,
        focus_level: r
            .get::<_, Option<i64>>("focus_level")?
            .map(|v| u8_from_i64(v, "focus_level"))
            .transpose()?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        erased: i64_bool(r.get("erased")?)?,
        erased_at: opt_from_ms(r.get("erased_at")?),
        erasure_request_id: r.get("erasure_request_id")?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert a reflection (`PERSONAL` only; `body_sha256` computed here).
pub fn insert_reflection(conn: &Connection, row: &ReflectionRow) -> RepoResult<Revision> {
    if row.data_domain != DataDomain::Personal {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "opt-in reflections are PERSONAL".to_string(),
        )));
    }
    conn.execute(
        &format!(
            "INSERT INTO opt_in_reflections ({REFLECTION_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'PERSONAL', ?8, 0, NULL, NULL, 1, ?9, ?9)"
        ),
        params![
            row.id,
            row.session_id.as_ref().map(|s| s.as_str().to_string()),
            ms(row.recorded_at),
            row.body,
            sha256_hex(row.body.as_bytes()),
            row.energy_level.map(i64::from),
            row.focus_level.map(i64::from),
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One reflection, scope-checked (a non-personal scope is refused unless cross-domain).
pub fn get_reflection(
    conn: &Connection,
    scope: &ReadScope,
    id: &str,
) -> RepoResult<Option<ReflectionRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {REFLECTION_COLUMNS} FROM opt_in_reflections WHERE id = ?1"),
        &[&id],
        map_reflection,
    )?;
    if let Some(r) = &row {
        scope.check("opt_in_reflections", r.data_domain)?;
    }
    Ok(row)
}

/// Reflections recorded in `[from, to)`, domain-filtered in SQL.
pub fn list_reflections_between(
    conn: &Connection,
    scope: &ReadScope,
    from: UtcMillis,
    to: UtcMillis,
) -> RepoResult<Vec<ReflectionRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {REFLECTION_COLUMNS} FROM opt_in_reflections
             WHERE recorded_at >= ?1 AND recorded_at < ?2 AND (?3 = 1 OR data_domain = ?4) ORDER BY recorded_at, id"
        ),
        &[&ms(from), &ms(to), &scope.sql_flag(), &scope.sql_domain()?],
        map_reflection,
    )
}

/// Keep `required`/`SourceKind` linked for symmetry with sibling modules.
#[allow(dead_code)]
fn _required(kind: SourceKind) -> RepoResult<SourceKind> {
    required("x", Some(kind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::testutil::{assert_immutable, db, operator_id, sha, t};

    fn business() -> ReadScope {
        ReadScope::domain(DataDomain::Business)
    }

    fn goal(id: &str) -> GoalRow {
        GoalRow {
            id: GoalId::parse(id).unwrap(),
            title: "Pass CT9 capstone".into(),
            description: None,
            horizon: GoalHorizon::Program,
            priority: 1,
            state: GoalState::Active,
            parent_goal_id: None,
            target_date: Some((
                CivilDate::new(2026, 2, 28).unwrap(),
                IanaZone::parse("Europe/Berlin").unwrap(),
            )),
            owner_id: operator_id(),
            data_domain: DataDomain::Business,
            classification: Classification::Private,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        }
    }

    fn project(id: &str, goal: &str) -> ProjectRow {
        ProjectRow {
            id: ProjectId::parse(id).unwrap(),
            goal_id: Some(GoalId::parse(goal).unwrap()),
            title: "Week 1 lab build".into(),
            description: None,
            state: ProjectState::Active,
            starts_at: Some(t(0)),
            ends_at: None,
            data_domain: DataDomain::Business,
            classification: Classification::Private,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        }
    }

    fn milestone(id: &str, project: &str, ordinal: u32) -> MilestoneRow {
        MilestoneRow {
            id: MilestoneId::parse(id).unwrap(),
            project_id: ProjectId::parse(project).unwrap(),
            title: format!("Milestone {ordinal}"),
            description: None,
            due_at: Some(t(7 * 86_400_000)),
            state: MilestoneState::Planned,
            completed_at: None,
            ordinal,
            data_domain: DataDomain::Business,
            classification: Classification::Private,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        }
    }

    fn provenance(trust: TrustClass) -> Provenance {
        Provenance {
            source_kind: SourceKind::OperatorInput,
            source_ref: "weekly-review".into(),
            trust,
            content_sha256: None,
            recorded_at: t(0),
            recorded_by: operator_id(),
            version: None,
        }
    }

    #[test]
    fn goals_projects_milestones_roundtrip_with_scope_and_revision() {
        let db = db();
        let conn = db.conn();
        let g = goal("goal-1");
        insert_goal(conn, &g).unwrap();
        assert_eq!(get_goal(conn, &business(), &g.id).unwrap().unwrap(), g);
        // business data is invisible to a learning-scoped read
        let learning = ReadScope::domain(DataDomain::Learning);
        assert!(matches!(
            get_goal(conn, &learning, &g.id),
            Err(RepoError::CrossDomainRefused {
                table: "goals",
                requested: DataDomain::Learning,
                actual: DataDomain::Business
            })
        ));
        assert!(list_goals(conn, &learning, None).unwrap().is_empty());
        assert_eq!(
            list_goals(conn, &ReadScope::cross_domain(DataDomain::Learning), None)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            list_goals(conn, &business(), Some(GoalState::Active))
                .unwrap()
                .len(),
            1
        );

        let mut changed = g.clone();
        changed.priority = 2;
        changed.target_date = None;
        assert!(matches!(
            update_goal(conn, &changed, Revision::new(3), t(1)),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        let r2 = update_goal(conn, &changed, Revision::INITIAL, t(1)).unwrap();
        let after = get_goal(conn, &business(), &g.id).unwrap().unwrap();
        assert_eq!(after.priority, 2);
        assert_eq!(after.target_date, None);
        assert_eq!(after.revision, r2);

        let p = project("proj-1", "goal-1");
        insert_project(conn, &p).unwrap();
        assert_eq!(get_project(conn, &business(), &p.id).unwrap().unwrap(), p);
        assert_eq!(
            list_projects(conn, &business(), Some(&g.id)).unwrap().len(),
            1
        );
        assert!(list_projects(conn, &learning, None).unwrap().is_empty());
        let mut p2 = p.clone();
        p2.state = ProjectState::Blocked;
        assert_eq!(
            update_project(conn, &p2, Revision::INITIAL, t(2)).unwrap(),
            Revision::new(2)
        );

        let m1 = milestone("ms-1", "proj-1", 1);
        let m2 = milestone("ms-2", "proj-1", 2);
        insert_milestone(conn, &m1).unwrap();
        insert_milestone(conn, &m2).unwrap();
        assert_eq!(list_milestones(conn, &business(), &p.id).unwrap().len(), 2);
        assert!(matches!(
            get_milestone(conn, &learning, &m1.id),
            Err(RepoError::CrossDomainRefused {
                table: "milestones",
                ..
            })
        ));
        let mut done = m1.clone();
        done.state = MilestoneState::Done;
        assert!(matches!(
            update_milestone(conn, &done, Revision::INITIAL, t(3)),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        done.completed_at = Some(t(3));
        assert_eq!(
            update_milestone(conn, &done, Revision::INITIAL, t(3)).unwrap(),
            Revision::new(2)
        );

        let dep = MilestoneDependencyRow {
            id: "dep-1".into(),
            from_milestone_id: m1.id.clone(),
            to_milestone_id: m2.id.clone(),
            kind: DependencyKind::Blocks,
            created_at: t(0),
        };
        insert_dependency(conn, &dep).unwrap();
        assert!(matches!(
            insert_dependency(
                conn,
                &MilestoneDependencyRow {
                    id: "dep-2".into(),
                    ..dep.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::UniqueViolation(_)))
        ));
        assert_eq!(dependencies_of(conn, &m2.id).unwrap(), vec![dep]);
        delete_dependency(conn, "dep-1").unwrap();
        assert!(matches!(
            delete_dependency(conn, "dep-1"),
            Err(RepoError::Storage(StorageError::NotFound(_)))
        ));
    }

    #[test]
    fn commitments_resolve_once_and_execution_observations_are_immutable() {
        let db = db();
        let conn = db.conn();
        insert_goal(conn, &goal("goal-1")).unwrap();
        insert_project(conn, &project("proj-1", "goal-1")).unwrap();
        let project_id = ProjectId::parse("proj-1").unwrap();
        let c = CommitmentRow {
            id: "com-1".into(),
            project_id: Some(project_id.clone()),
            milestone_id: None,
            calendar_event_id: None,
            title: "Ship lab VM image".into(),
            description: None,
            due_at: t(86_400_000),
            state: CommitmentState::Open,
            resolved_at: None,
            data_domain: DataDomain::Business,
            classification: Classification::Private,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        };
        insert_commitment(conn, &c).unwrap();
        assert_eq!(
            get_commitment(conn, &business(), &c.id).unwrap().unwrap(),
            c
        );
        assert!(matches!(
            insert_commitment(
                conn,
                &CommitmentRow {
                    id: "com-x".into(),
                    state: CommitmentState::Kept,
                    ..c.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        assert_eq!(
            list_open_commitments(conn, &business(), t(2 * 86_400_000))
                .unwrap()
                .len(),
            1
        );
        assert!(list_open_commitments(
            conn,
            &ReadScope::domain(DataDomain::Learning),
            t(2 * 86_400_000)
        )
        .unwrap()
        .is_empty());
        assert!(matches!(
            resolve_commitment(conn, &c.id, Revision::INITIAL, CommitmentState::Open, t(5)),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        let r2 = resolve_commitment(conn, &c.id, Revision::INITIAL, CommitmentState::Kept, t(5))
            .unwrap();
        let kept = get_commitment(conn, &business(), &c.id).unwrap().unwrap();
        assert_eq!(kept.state, CommitmentState::Kept);
        assert_eq!(kept.resolved_at, Some(t(5)));
        assert!(matches!(
            resolve_commitment(conn, &c.id, r2, CommitmentState::Missed, t(6)),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        assert!(list_open_commitments(conn, &business(), t(2 * 86_400_000))
            .unwrap()
            .is_empty());

        let obs = ExecutionObservationRow {
            id: "eo-1".into(),
            project_id: Some(project_id.clone()),
            milestone_id: None,
            commitment_id: Some("com-1".into()),
            metric: "kept_commitment_ratio".into(),
            value: 1.0,
            unit: "ratio".into(),
            observed_at: t(10),
            provenance: provenance(TrustClass::Operator),
            data_domain: DataDomain::Business,
            classification: Classification::Private,
            created_at: t(10),
        };
        append_execution_observation(conn, &obs).unwrap();
        assert!(matches!(
            append_execution_observation(
                conn,
                &ExecutionObservationRow {
                    id: "eo-m".into(),
                    provenance: provenance(TrustClass::ModelOutput),
                    ..obs.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        assert_eq!(
            list_execution_observations(conn, &business(), &project_id, t(0), t(100)).unwrap(),
            vec![obs]
        );
        assert!(list_execution_observations(
            conn,
            &ReadScope::domain(DataDomain::Learning),
            &project_id,
            t(0),
            t(100)
        )
        .unwrap()
        .is_empty());
        assert_immutable(
            conn,
            "UPDATE execution_observations SET value = 0 WHERE id = 'eo-1'",
        );
        assert_immutable(conn, "DELETE FROM execution_observations WHERE id = 'eo-1'");
    }

    #[test]
    fn decisions_are_decided_by_the_operator_and_outcomes_are_immutable() {
        let db = db();
        let conn = db.conn();
        let proposal = DecisionRow {
            id: DecisionId::parse("dec-1").unwrap(),
            goal_id: None,
            project_id: None,
            title: "Lab platform".into(),
            context: "Choose between local VMs and a hosted range".into(),
            options: vec![
                serde_json::json!({ "id": "local" }),
                serde_json::json!({ "id": "hosted" }),
            ],
            chosen_option: None,
            rationale: None,
            state: DecisionState::Proposed,
            decided_by: None,
            decided_at: None,
            review_due_at: None,
            supersedes: None,
            trust: TrustClass::ModelOutput,
            data_domain: DataDomain::Business,
            classification: Classification::Private,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        };
        insert_decision(conn, &proposal).unwrap();
        assert_eq!(
            get_decision(conn, &business(), &proposal.id)
                .unwrap()
                .unwrap(),
            proposal
        );
        // model output cannot insert a decided decision
        assert!(matches!(
            insert_decision(
                conn,
                &DecisionRow {
                    id: DecisionId::parse("dec-m").unwrap(),
                    state: DecisionState::Decided,
                    chosen_option: Some("local".into()),
                    decided_by: Some(operator_id()),
                    decided_at: Some(t(1)),
                    ..proposal.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        assert!(matches!(
            decide(
                conn,
                &proposal.id,
                Revision::INITIAL,
                "local",
                None,
                &Actor::model(ActorId::parse("model-1").unwrap()),
                t(1),
                None
            ),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        assert!(matches!(
            decide(
                conn,
                &proposal.id,
                Revision::new(4),
                "local",
                None,
                &Actor::operator(operator_id()),
                t(1),
                None
            ),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        let r2 = decide(
            conn,
            &proposal.id,
            Revision::INITIAL,
            "local",
            Some("offline-first"),
            &Actor::operator(operator_id()),
            t(1),
            Some(t(100)),
        )
        .unwrap();
        let decided = get_decision(conn, &business(), &proposal.id)
            .unwrap()
            .unwrap();
        assert_eq!(decided.state, DecisionState::Decided);
        assert_eq!(decided.trust, TrustClass::Operator);
        assert_eq!(decided.chosen_option.as_deref(), Some("local"));
        assert_eq!(decided.decided_by, Some(operator_id()));
        assert_eq!(
            list_decisions(conn, &business(), Some(DecisionState::Decided))
                .unwrap()
                .len(),
            1
        );
        assert!(
            list_decisions(conn, &ReadScope::domain(DataDomain::Learning), None)
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            decide(
                conn,
                &proposal.id,
                r2,
                "hosted",
                None,
                &Actor::operator(operator_id()),
                t(2),
                None
            ),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));

        let outcome = DecisionOutcomeRow {
            id: "do-1".into(),
            decision_id: proposal.id.clone(),
            measured_at: t(50),
            summary: "VMs stable, no range costs".into(),
            verdict: DecisionVerdict::AsExpected,
            recorded_by: operator_id(),
            created_at: t(50),
        };
        append_decision_outcome(conn, &outcome).unwrap();
        assert_eq!(
            list_decision_outcomes(conn, &business(), &proposal.id).unwrap(),
            vec![outcome]
        );
        // business outcomes are refused to a learning scope (R-017)
        assert!(matches!(
            list_decision_outcomes(conn, &ReadScope::domain(DataDomain::Learning), &proposal.id),
            Err(RepoError::CrossDomainRefused {
                table: "decisions",
                ..
            })
        ));
        assert_immutable(
            conn,
            "UPDATE decision_outcomes SET verdict = 'BETTER' WHERE id = 'do-1'",
        );
        assert_immutable(conn, "DELETE FROM decision_outcomes WHERE id = 'do-1'");

        // revisiting: the old row is closed, the new one supersedes it
        assert!(matches!(
            close_decision(conn, &proposal.id, r2, DecisionState::Proposed, t(60)),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        let r3 = close_decision(conn, &proposal.id, r2, DecisionState::Revisited, t(60)).unwrap();
        assert_eq!(
            get_decision(conn, &business(), &proposal.id)
                .unwrap()
                .unwrap()
                .revision,
            r3
        );
        let replacement = DecisionRow {
            id: DecisionId::parse("dec-2").unwrap(),
            state: DecisionState::Decided,
            chosen_option: Some("hosted".into()),
            rationale: Some("range needed for week 9".into()),
            decided_by: Some(operator_id()),
            decided_at: Some(t(61)),
            supersedes: Some(proposal.id.clone()),
            trust: TrustClass::Operator,
            created_at: t(61),
            updated_at: t(61),
            ..proposal.clone()
        };
        insert_decision(conn, &replacement).unwrap();
        assert_eq!(
            get_decision(conn, &business(), &replacement.id)
                .unwrap()
                .unwrap()
                .supersedes,
            Some(proposal.id.clone())
        );
    }

    #[test]
    fn reflections_are_personal_and_hidden_from_other_scopes() {
        let db = db();
        let conn = db.conn();
        let row = ReflectionRow {
            id: "refl-1".into(),
            session_id: None,
            recorded_at: t(0),
            body: "felt focused after the recall block".into(),
            body_sha256: String::new(),
            energy_level: Some(4),
            focus_level: Some(5),
            data_domain: DataDomain::Personal,
            classification: Classification::Private,
            erased: false,
            erased_at: None,
            erasure_request_id: None,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        };
        insert_reflection(conn, &row).unwrap();
        assert!(matches!(
            insert_reflection(
                conn,
                &ReflectionRow {
                    id: "refl-b".into(),
                    data_domain: DataDomain::Business,
                    ..row.clone()
                }
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        let personal = ReadScope::domain(DataDomain::Personal);
        let stored = get_reflection(conn, &personal, &row.id).unwrap().unwrap();
        assert_eq!(stored.body_sha256, sha(&row.body));
        assert_eq!(stored.energy_level, Some(4));
        assert!(matches!(
            get_reflection(conn, &business(), &row.id),
            Err(RepoError::CrossDomainRefused {
                table: "opt_in_reflections",
                requested: DataDomain::Business,
                actual: DataDomain::Personal
            })
        ));
        assert!(list_reflections_between(conn, &business(), t(0), t(10))
            .unwrap()
            .is_empty());
        assert_eq!(
            list_reflections_between(conn, &personal, t(0), t(10))
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            list_reflections_between(
                conn,
                &ReadScope::cross_domain(DataDomain::Learning),
                t(0),
                t(10)
            )
            .unwrap()
            .len(),
            1
        );
    }
}
