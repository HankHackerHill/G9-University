//! Typed repositories over the migrated schema (ARCHITECTURE.md §6).
//!
//! One module per table group. Every function takes a `&Connection` (a
//! `rusqlite::Transaction` derefs to one, so `&*tx` works inside the command
//! pipeline's `BEGIN IMMEDIATE` transaction), uses parameterized SQL only, maps
//! rows onto the wave-1 crate types where the DDL and the type agree and onto
//! typed row structs otherwise, and:
//!
//! * enforces `expected_revision` on every UPDATE (`revision.stale` →
//!   `STALE_REVISION`), returning the new [`Revision`];
//! * never issues UPDATE or DELETE against append-only tables (the immutability
//!   triggers in the migrations are the second line of defence; tests prove they
//!   fire);
//! * filters reads by [`ReadScope`]: list queries carry the domain filter in SQL,
//!   single-row reads refuse a row whose `data_domain` differs from the requested
//!   domain unless `cross_domain` is set ([`RepoError::CrossDomainRefused`],
//!   mapped to `UNAUTHORIZED`; `g9-app` sets `cross_domain` only after
//!   `g9-policy` granted `Capability::CrossDomainRead`).
//!
//! * content-bearing child rows without their own `data_domain`
//!   (`note_versions`, `decision_outcomes`) are scope-checked through their
//!   parent;
//! * writes that turn a proposal into operator authority
//!   ([`retention::review_intervention`], [`competency::decide_hypothesis`],
//!   [`business::decide`]) take the backend-assigned [`g9_domain::Actor`] and
//!   refuse any actor that is not the operator.
//!
//! Nothing here evaluates policy, consumes approvals or writes audit rows: the
//! command pipeline composes these repositories with [`crate::approvals_store`],
//! [`crate::audit`], [`crate::events`] and [`crate::idempotency`].
//!
//! Migration `0010_intervention_review_and_reliability` adds the columns these
//! repositories need that `0001`–`0009` lacked (intervention origin/review/plan,
//! `grade_decisions.reliability_class`, `study_block_id` links, the calendar DST
//! flag) and the immutability triggers that were missing on append-only and
//! versioned tables (assessment versions/rubrics, retake links, attempt
//! evidence, competency versions, content classifications, note versions,
//! curriculum manifests and structure).
//!
//! | Module | Tables |
//! |---|---|
//! | [`curriculum`] | `curricula`, `curriculum_versions`, `weeks`, `training_days`, `daily_objectives`, `study_blocks`, `resource_references`, `deliverable_definitions` |
//! | [`calendar`] | `calendar_events`, `recurrence_rules`, `schedule_proposals`, `schedule_decisions` |
//! | [`sessions`] | `study_sessions`, `session_events`, `time_intervals`, `manual_time_corrections` |
//! | [`evidence`] | `notes`, `note_versions`, `attachments`, `evidence_items`, `artifact_links`, `source_provenance`, `content_classifications` |
//! | [`assessment`] | `assessment_definitions`, `assessment_versions`, `rubrics`, `rubric_criteria`, `attempts`, `attempt_evidence`, `criterion_results`, `grade_decisions`, `retake_links`, `exposure_log` |
//! | [`competency`] | `competencies`, `competency_versions`, `competency_edges`, `objective_competencies`, `competency_observations`, `learner_hypotheses`, `competency_state_snapshots` |
//! | [`retention`] | `review_items`, `review_attempts`, `practical_transfer_checks`, `learning_episodes`, `calibration_observations`, `interventions`, `intervention_outcomes`, `method_experiments` |
//! | [`knowledge`] | `model_registry`, `model_evals`, `skill_registry`, `skill_versions`, `skill_invocations`, `skill_evals` |
//! | [`business`] | `goals`, `projects`, `milestones`, `milestone_dependencies`, `commitments`, `execution_observations`, `decisions`, `decision_outcomes`, `opt_in_reflections` |

pub mod assessment;
pub mod business;
pub mod calendar;
pub mod competency;
pub mod curriculum;
pub mod evidence;
pub mod knowledge;
pub mod retention;
pub mod sessions;

use crate::error::{StorageError, StorageResult};
use g9_domain::{DataDomain, G9Error, Revision, Scope, UtcMillis};
use rusqlite::{Connection, Row};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

/// Errors raised by the repositories.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RepoError {
    /// A storage-level failure (constraint, revision, decode, I/O ...).
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// A single-row read hit a row outside the requested privacy domain and the
    /// caller did not hold `cross_domain` (R-017, SEC-32).
    #[error("cross-domain read refused on {table}: requested {requested:?}, row is {actual:?}")]
    CrossDomainRefused {
        /// Table of the refused row.
        table: &'static str,
        /// Domain the caller asked for.
        requested: DataDomain,
        /// Domain stored on the row.
        actual: DataDomain,
    },
}

/// Result alias for repository functions.
pub type RepoResult<T> = Result<T, RepoError>;

impl RepoError {
    /// Stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            RepoError::Storage(e) => e.code(),
            RepoError::CrossDomainRefused { .. } => "storage.cross_domain_read",
        }
    }
}

impl From<rusqlite::Error> for RepoError {
    fn from(e: rusqlite::Error) -> Self {
        RepoError::Storage(StorageError::from(e))
    }
}

impl From<g9_domain::state::TransitionError> for RepoError {
    fn from(e: g9_domain::state::TransitionError) -> Self {
        RepoError::Storage(StorageError::InvalidTransition(e))
    }
}

impl From<RepoError> for G9Error {
    fn from(e: RepoError) -> Self {
        match e {
            RepoError::Storage(inner) => inner.into(),
            RepoError::CrossDomainRefused {
                table,
                requested,
                actual,
            } => G9Error::unauthorized(
                "storage.cross_domain_read",
                "cross-domain read refused; Capability::CrossDomainRead not granted",
            )
            .with_details(serde_json::json!({
                "table": table,
                "requested_domain": requested,
                "row_domain": actual,
            })),
        }
    }
}

impl From<RepoError> for StorageError {
    /// For callers that stay inside [`StorageResult`]: the refusal maps to
    /// `CONFLICT` (the requested scope conflicts with the stored domain).
    fn from(e: RepoError) -> Self {
        match e {
            RepoError::Storage(inner) => inner,
            RepoError::CrossDomainRefused {
                table,
                requested,
                actual,
            } => StorageError::Conflict(format!(
                "cross-domain read refused on {table}: requested {requested:?}, row is {actual:?}"
            )),
        }
    }
}

/// Privacy-domain filter applied to every read (ARCHITECTURE.md §6 "Privacy").
///
/// `cross_domain = true` is set by `g9-app` only after `g9-policy` granted
/// `Capability::CrossDomainRead` for the command; the repositories never decide it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReadScope {
    /// Domain the caller is operating in.
    pub domain: DataDomain,
    /// Whether rows of other domains may be returned.
    pub cross_domain: bool,
}

impl ReadScope {
    /// Scope restricted to one domain.
    pub const fn domain(domain: DataDomain) -> Self {
        Self {
            domain,
            cross_domain: false,
        }
    }

    /// Scope that may read every domain (requires `Capability::CrossDomainRead`).
    pub const fn cross_domain(domain: DataDomain) -> Self {
        Self {
            domain,
            cross_domain: true,
        }
    }

    /// Build from a command scope.
    pub const fn from_scope(scope: &Scope, cross_domain: bool) -> Self {
        Self {
            domain: scope.domain,
            cross_domain,
        }
    }

    /// Refuse a row outside the scope.
    pub(crate) fn check(&self, table: &'static str, actual: DataDomain) -> RepoResult<()> {
        if self.cross_domain || actual == self.domain {
            Ok(())
        } else {
            Err(RepoError::CrossDomainRefused {
                table,
                requested: self.domain,
                actual,
            })
        }
    }

    /// `1` when the SQL filter `(?flag = 1 OR data_domain = ?domain)` must pass everything.
    pub(crate) fn sql_flag(&self) -> i64 {
        i64::from(self.cross_domain)
    }

    /// TEXT value of the scope domain for the SQL filter.
    pub(crate) fn sql_domain(&self) -> StorageResult<String> {
        crate::sql::enum_to_text(&self.domain)
    }
}

// ---------------------------------------------------------------------------
// shared helpers (crate-private)
// ---------------------------------------------------------------------------

/// Run a query and map every row with `f`.
pub(crate) fn query_all<T>(
    conn: &Connection,
    sql: &str,
    params: &[&dyn rusqlite::ToSql],
    mut f: impl FnMut(&Row<'_>) -> StorageResult<T>,
) -> RepoResult<Vec<T>> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query(params)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(f(row)?);
    }
    Ok(out)
}

/// Run a query expected to return at most one row.
pub(crate) fn query_opt<T>(
    conn: &Connection,
    sql: &str,
    params: &[&dyn rusqlite::ToSql],
    f: impl FnOnce(&Row<'_>) -> StorageResult<T>,
) -> RepoResult<Option<T>> {
    let mut stmt = conn.prepare(sql)?;
    let mut rows = stmt.query(params)?;
    match rows.next()? {
        Some(row) => Ok(Some(f(row)?)),
        None => Ok(None),
    }
}

/// Current revision of a row, if it exists. `table`/`id_column` are compile-time
/// constants supplied by the repositories, never caller input.
pub(crate) fn current_revision(
    conn: &Connection,
    table: &'static str,
    id_column: &'static str,
    id: &str,
) -> RepoResult<Option<Revision>> {
    let sql = format!("SELECT revision FROM {table} WHERE {id_column} = ?1");
    query_opt(conn, &sql, &[&id], |r| Ok(rev_from_i64(r.get("revision")?)))
}

/// Interpret the row count of a revision-guarded UPDATE
/// (`... WHERE <id> = ?id AND revision = ?expected`).
pub(crate) fn finish_update(
    conn: &Connection,
    table: &'static str,
    id_column: &'static str,
    id: &str,
    expected: Revision,
    changed: usize,
) -> RepoResult<Revision> {
    if changed == 1 {
        return Ok(expected.next());
    }
    match current_revision(conn, table, id_column, id)? {
        None => Err(RepoError::Storage(StorageError::NotFound(table))),
        Some(actual) if actual != expected => {
            Err(RepoError::Storage(StorageError::StaleRevision {
                expected: Some(expected),
                actual,
            }))
        }
        // revision matched but another guard in the WHERE clause refused the row
        Some(_) => Err(RepoError::Storage(StorageError::Precondition(format!(
            "{table} row is not in a state that allows this update"
        )))),
    }
}

/// Scope check for child rows that carry no `data_domain` of their own (note
/// versions, decision outcomes): the parent's domain decides. Returns `false`
/// when the parent does not exist. `table`/`id_column` are compile-time
/// constants supplied by the repositories, never caller input.
pub(crate) fn check_parent_domain(
    conn: &Connection,
    scope: &ReadScope,
    table: &'static str,
    id_column: &'static str,
    id: &str,
) -> RepoResult<bool> {
    let sql = format!("SELECT data_domain FROM {table} WHERE {id_column} = ?1");
    let domain: Option<String> = query_opt(conn, &sql, &[&id], |r| Ok(r.get("data_domain")?))?;
    match domain {
        None => Ok(false),
        Some(text) => {
            scope.check(table, crate::sql::enum_from_text(&text)?)?;
            Ok(true)
        }
    }
}

/// Require a row to exist (`NotFound` otherwise).
pub(crate) fn required<T>(table: &'static str, value: Option<T>) -> RepoResult<T> {
    value.ok_or(RepoError::Storage(StorageError::NotFound(table)))
}

pub(crate) fn rev_i64(r: Revision) -> i64 {
    i64::try_from(r.get()).unwrap_or(i64::MAX)
}

pub(crate) fn rev_from_i64(v: i64) -> Revision {
    Revision::new(u64::try_from(v).unwrap_or(1))
}

pub(crate) fn ms(t: UtcMillis) -> i64 {
    t.as_millis()
}

pub(crate) fn opt_ms(t: Option<UtcMillis>) -> Option<i64> {
    t.map(UtcMillis::as_millis)
}

pub(crate) fn from_ms(v: i64) -> UtcMillis {
    UtcMillis::from_millis(v)
}

pub(crate) fn opt_from_ms(v: Option<i64>) -> Option<UtcMillis> {
    v.map(UtcMillis::from_millis)
}

pub(crate) fn bool_i64(b: bool) -> i64 {
    i64::from(b)
}

pub(crate) fn i64_bool(v: i64) -> StorageResult<bool> {
    match v {
        0 => Ok(false),
        1 => Ok(true),
        other => Err(StorageError::RowDecode(format!(
            "boolean column holds {other}"
        ))),
    }
}

pub(crate) fn u32_from_i64(v: i64, column: &str) -> StorageResult<u32> {
    u32::try_from(v).map_err(|_| StorageError::RowDecode(format!("column {column} out of range")))
}

pub(crate) fn u8_from_i64(v: i64, column: &str) -> StorageResult<u8> {
    u8::try_from(v).map_err(|_| StorageError::RowDecode(format!("column {column} out of range")))
}

pub(crate) fn u64_from_i64(v: i64, column: &str) -> StorageResult<u64> {
    u64::try_from(v).map_err(|_| StorageError::RowDecode(format!("column {column} out of range")))
}

pub(crate) fn i64_from_u64(v: u64, field: &str) -> StorageResult<i64> {
    i64::try_from(v).map_err(|_| StorageError::InvalidInput(format!("{field} exceeds i64")))
}

/// Serialize a value for a `json_valid()` column.
pub(crate) fn to_json<T: Serialize>(value: &T) -> StorageResult<String> {
    serde_json::to_string(value).map_err(|_| StorageError::Internal("json encode"))
}

/// Decode a `json_valid()` column into a typed value.
pub(crate) fn from_json<T: DeserializeOwned>(text: &str, column: &str) -> StorageResult<T> {
    serde_json::from_str(text).map_err(|e| StorageError::RowDecode(format!("column {column}: {e}")))
}

/// Parse an optional typed id column.
pub(crate) fn opt_parse_id<T>(text: Option<String>) -> StorageResult<Option<T>>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    text.as_deref().map(crate::sql::parse_id).transpose()
}

/// Validate that a text field is non-empty after trimming.
pub(crate) fn require_non_empty(field: &'static str, value: &str) -> StorageResult<()> {
    if value.trim().is_empty() {
        Err(StorageError::InvalidInput(format!(
            "{field} must not be empty"
        )))
    } else {
        Ok(())
    }
}

/// Validate an optional hex SHA-256.
pub(crate) fn opt_sha256(field: &'static str, value: Option<&str>) -> StorageResult<()> {
    match value {
        Some(v) => crate::sql::require_sha256(field, v),
        None => Ok(()),
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    //! Deterministic fixtures shared by the repository tests.

    use crate::approvals_store::{insert_request, NewApprovalRequest};
    use crate::connection::{Database, OpenOptions};
    use g9_domain::hashing::sha256_hex;
    use g9_domain::{
        Action, Actor, ActorId, ApprovalId, Capability, DataDomain, InteractionMode,
        OperationDescriptor, ResourceKind, ResourceRef, Revision, Scope, UtcMillis,
    };
    use rusqlite::Connection;

    /// 2025-11-06T00:00:00Z.
    pub const T0: i64 = 1_762_387_200_000;

    pub fn t(offset_ms: i64) -> UtcMillis {
        UtcMillis::from_millis(T0 + offset_ms)
    }

    pub fn sha(s: &str) -> String {
        sha256_hex(s.as_bytes())
    }

    pub fn db() -> Database {
        Database::open_in_memory(OpenOptions::new("0.1.0-test").applied_at(t(0))).unwrap()
    }

    pub fn operator_id() -> ActorId {
        ActorId::parse("operator-1").unwrap()
    }

    pub fn operator() -> Actor {
        Actor::operator(operator_id())
    }

    pub fn learning() -> super::ReadScope {
        super::ReadScope::domain(DataDomain::Learning)
    }

    pub fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    /// Insert a pending approval request so FK columns can reference it.
    pub fn approval(conn: &Connection, id: &str) -> ApprovalId {
        insert_request(
            conn,
            &NewApprovalRequest {
                id: ApprovalId::parse(id).unwrap(),
                operation: OperationDescriptor {
                    action: Action::Approve,
                    resource: ResourceRef::new(ResourceKind::CurriculumVersion, "x"),
                    scope: Scope {
                        domain: DataDomain::Learning,
                        mode: InteractionMode::Maintenance,
                    },
                    parameters_sha256: sha("params"),
                },
                required_capability: Some(Capability::CurriculumPromote),
                requested_by: operator_id(),
                requested_at: t(0),
                expires_at: t(15 * 60_000),
                expected_revision: Some(Revision::INITIAL),
            },
        )
        .unwrap()
        .id
    }

    /// Assert that an UPDATE/DELETE statement is refused by an immutability trigger.
    pub fn assert_immutable(conn: &Connection, sql: &str) {
        let err = conn
            .execute(sql, [])
            .expect_err("statement must be refused");
        let e: crate::error::StorageError = err.into();
        assert!(
            matches!(e, crate::error::StorageError::Immutable(_)),
            "expected IMMUTABLE trigger, got {e:?} for {sql}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::ErrorCategory;

    #[test]
    fn cross_domain_refusal_maps_to_unauthorized_and_conflict() {
        let scope = ReadScope::domain(DataDomain::Learning);
        let err = scope
            .check("goals", DataDomain::Business)
            .expect_err("different domain must be refused");
        assert_eq!(err.code(), "storage.cross_domain_read");
        let g: G9Error = err.clone().into();
        assert_eq!(g.category, ErrorCategory::Unauthorized);
        assert!(!g.message.contains("backtrace"));
        let s: StorageError = err.into();
        assert_eq!(s.category(), ErrorCategory::Conflict);
        assert!(ReadScope::cross_domain(DataDomain::Learning)
            .check("goals", DataDomain::Business)
            .is_ok());
        assert!(scope.check("notes", DataDomain::Learning).is_ok());
    }

    #[test]
    fn finish_update_distinguishes_not_found_stale_and_guarded() {
        let db = testutil::db();
        let conn = db.conn();
        conn.execute(
            "INSERT INTO goals (id, title, horizon, owner_id, created_at, updated_at) VALUES ('g', 't', 'WEEK', 'op', 1, 1)",
            [],
        )
        .unwrap();
        assert!(matches!(
            finish_update(conn, "goals", "id", "missing", Revision::INITIAL, 0),
            Err(RepoError::Storage(StorageError::NotFound("goals")))
        ));
        assert!(matches!(
            finish_update(conn, "goals", "id", "g", Revision::new(3), 0),
            Err(RepoError::Storage(StorageError::StaleRevision { actual, .. })) if actual == Revision::INITIAL
        ));
        assert!(matches!(
            finish_update(conn, "goals", "id", "g", Revision::INITIAL, 0),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
        assert_eq!(
            finish_update(conn, "goals", "id", "g", Revision::INITIAL, 1).unwrap(),
            Revision::new(2)
        );
    }
}
