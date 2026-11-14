//! Hash-chained, metadata-only audit ledger (ARCHITECTURE.md §6 "Audit chain";
//! handoff C19/SEC-08).
//!
//! `audit_metadata` rows are never updated or deleted (triggers in
//! `0001_governance.sql`). Each row carries `prev_hash` (the previous row's
//! `row_hash`, or the genesis constant for the first row) and
//! `row_hash = sha256(prev_hash || canonical_json(body))` where `body` is the
//! row without its hash columns and without `seq`. [`AuditChain::verify_chain`]
//! recomputes every hash from the stored columns, so an inserted, reordered or
//! edited row is detected. Periodic anchors (`seq`, `row_hash`) are copied into
//! `backup_manifests` by the backup module.
//!
//! The links cannot show that the *newest* rows are gone: what is left still verifies.
//! [`AuditChain::verify_chain`] therefore also checks two facts kept beside the rows, the
//! sequence counter that `AUTOINCREMENT` maintains (the largest `seq` ever allocated) and every
//! anchor that a backup manifest names. Both live in the database file, so they raise the bar
//! against a naive edit only; a writer who rewrites them too is not detected (ADR-0004).
//!
//! The body never contains content: callers hash their details JSON into
//! `details_sha256` ([`AuditEntry::details_hash`]).

use crate::error::{StorageError, StorageResult};
use crate::sql::{actor_columns, actor_from_columns, enum_from_text, enum_to_text, require_sha256};
use g9_domain::hashing::{canonical_json, sha256_hex, sha256_json};
use g9_domain::{
    Actor, AuditRef, ErrorCategory, RequestId, ResourceKind, ResourceRef, TraceId, UtcMillis,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// `prev_hash` of the first row.
pub const GENESIS_PREV_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

/// Policy decision recorded with the audit row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditDecision {
    /// Policy allowed the operation.
    Allow,
    /// Policy denied the operation.
    Deny,
    /// Policy required an approval (granted or not).
    ApprovalRequired,
    /// Read-only or system operations that skipped policy evaluation.
    NotEvaluated,
}

/// Outcome of the operation: success or the closed error category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuditOutcome {
    /// The command succeeded.
    Success,
    /// Mirrors `ErrorCategory::InvalidInput`.
    InvalidInput,
    /// Mirrors `ErrorCategory::Unauthorized`.
    Unauthorized,
    /// Mirrors `ErrorCategory::ApprovalRequired`.
    ApprovalRequired,
    /// Mirrors `ErrorCategory::StaleRevision`.
    StaleRevision,
    /// Mirrors `ErrorCategory::PreconditionFailed`.
    PreconditionFailed,
    /// Mirrors `ErrorCategory::NotFound`.
    NotFound,
    /// Mirrors `ErrorCategory::Conflict`.
    Conflict,
    /// Mirrors `ErrorCategory::ResourceUnavailable`.
    ResourceUnavailable,
    /// Mirrors `ErrorCategory::Timeout`.
    Timeout,
    /// Mirrors `ErrorCategory::Cancelled`.
    Cancelled,
    /// Mirrors `ErrorCategory::RetryableStorage`.
    RetryableStorage,
    /// Mirrors `ErrorCategory::DataCorruption`.
    DataCorruption,
    /// Mirrors `ErrorCategory::Internal`.
    Internal,
}

impl From<ErrorCategory> for AuditOutcome {
    fn from(c: ErrorCategory) -> Self {
        match c {
            ErrorCategory::InvalidInput => AuditOutcome::InvalidInput,
            ErrorCategory::Unauthorized => AuditOutcome::Unauthorized,
            ErrorCategory::ApprovalRequired => AuditOutcome::ApprovalRequired,
            ErrorCategory::StaleRevision => AuditOutcome::StaleRevision,
            ErrorCategory::PreconditionFailed => AuditOutcome::PreconditionFailed,
            ErrorCategory::NotFound => AuditOutcome::NotFound,
            ErrorCategory::Conflict => AuditOutcome::Conflict,
            ErrorCategory::ResourceUnavailable => AuditOutcome::ResourceUnavailable,
            ErrorCategory::Timeout => AuditOutcome::Timeout,
            ErrorCategory::Cancelled => AuditOutcome::Cancelled,
            ErrorCategory::RetryableStorage => AuditOutcome::RetryableStorage,
            ErrorCategory::DataCorruption => AuditOutcome::DataCorruption,
            ErrorCategory::Internal => AuditOutcome::Internal,
        }
    }
}

/// Metadata of one audited operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    /// When the operation happened.
    pub occurred_at: UtcMillis,
    /// Who performed it.
    pub actor: Actor,
    /// Operation name, e.g. `session.start`.
    pub operation: String,
    /// Target resource, when any.
    pub resource: Option<ResourceRef>,
    /// Policy decision.
    pub decision: AuditDecision,
    /// Result category.
    pub outcome: AuditOutcome,
    /// Request id of the command.
    pub request_id: RequestId,
    /// Trace id of the command.
    pub trace_id: TraceId,
    /// Hex SHA-256 of the caller's details JSON (content stays out of the ledger).
    pub details_sha256: Option<String>,
}

impl AuditEntry {
    /// Hash a details document for `details_sha256`.
    pub fn details_hash(details: &serde_json::Value) -> String {
        sha256_json(details)
    }
}

/// Result of an append.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Row id, returned to the caller as `CommandOutcome::audit_ref`.
    pub audit_ref: AuditRef,
    /// Chain position.
    pub seq: i64,
    /// Previous row hash.
    pub prev_hash: String,
    /// This row's hash.
    pub row_hash: String,
}

/// Result of [`AuditChain::verify_chain`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainReport {
    /// Rows examined.
    pub rows: u64,
    /// `true` when every link verified and nothing is missing at the end of the chain.
    pub ok: bool,
    /// `seq` of the first row whose `prev_hash` or `row_hash` did not verify, or the first
    /// position at which rows are missing (the counter or a backup anchor says they existed).
    pub first_bad_seq: Option<i64>,
    /// Why the first bad row failed, or what is missing.
    pub first_bad_reason: Option<String>,
    /// `row_hash` of the last row (chain head), when any.
    pub head_hash: Option<String>,
}

/// Canonical body hashed into `row_hash`. Field names are part of the chain
/// format; changing them invalidates existing ledgers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct RowBody<'a> {
    id: &'a str,
    occurred_at: i64,
    actor_id: &'a str,
    actor_kind: &'a str,
    actor_skill_id: Option<&'a str>,
    operation: &'a str,
    resource_kind: Option<&'a str>,
    resource_id: Option<&'a str>,
    decision: &'a str,
    outcome: &'a str,
    request_id: &'a str,
    trace_id: &'a str,
    details_sha256: Option<&'a str>,
}

/// Compute `sha256(prev_hash || canonical_json(body))`.
fn chain_hash(prev_hash: &str, body: &RowBody<'_>) -> StorageResult<String> {
    let value = serde_json::to_value(body).map_err(|_| StorageError::Internal("audit body"))?;
    let mut bytes = Vec::with_capacity(64 + 512);
    bytes.extend_from_slice(prev_hash.as_bytes());
    bytes.extend_from_slice(canonical_json(&value).as_bytes());
    Ok(sha256_hex(&bytes))
}

/// What is wrong at the end of an otherwise intact chain.
struct TailProblem {
    seq: Option<i64>,
    reason: String,
}

/// Checks that the rows the chain cannot vouch for itself are all there: `newest` is the `seq` of
/// the last row whose links verified (0 when there is none).
///
/// 1. `sqlite_sequence` holds the largest `seq` ever allocated for `audit_metadata`; rows are
///    never deleted, so it equals the newest row. Deleting the newest rows leaves it behind.
/// 2. Every backup manifest that names an anchor (`audit_anchor_seq`, `audit_anchor_hash`) names
///    a row that exists with that hash, which also covers a counter that was reset to match.
fn tail_problem(conn: &Connection, newest: i64) -> StorageResult<Option<TailProblem>> {
    use rusqlite::types::Value;

    let counter: Option<Value> = conn.query_row(
        "SELECT MAX(seq) FROM sqlite_sequence WHERE name = 'audit_metadata'",
        [],
        |r| r.get(0),
    )?;
    let allocated = match counter {
        None | Some(Value::Null) => 0,
        Some(Value::Integer(n)) => n,
        Some(_) => {
            return Ok(Some(TailProblem {
                seq: None,
                reason: "the audit sequence counter is not an integer".to_string(),
            }))
        }
    };
    if allocated > newest {
        return Ok(Some(TailProblem {
            seq: Some(newest + 1),
            reason: format!(
                "audit rows after seq {newest} are missing: the sequence counter reached {allocated}"
            ),
        }));
    }
    if allocated < newest {
        return Ok(Some(TailProblem {
            seq: Some(allocated + 1),
            reason: format!(
                "the audit sequence counter ({allocated}) is behind the newest row ({newest})"
            ),
        }));
    }

    let mut stmt = conn.prepare(
        "SELECT id, audit_anchor_seq, audit_anchor_hash FROM backup_manifests
          WHERE audit_anchor_seq IS NOT NULL OR audit_anchor_hash IS NOT NULL
          ORDER BY audit_anchor_seq, id",
    )?;
    let anchors = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<i64>>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    })?;
    for anchor in anchors {
        let (backup, seq, hash) = anchor?;
        let (Some(seq), Some(hash)) = (seq, hash) else {
            return Ok(Some(TailProblem {
                seq,
                reason: format!("backup {backup} records half an audit anchor"),
            }));
        };
        let stored: Option<String> = conn
            .query_row(
                "SELECT row_hash FROM audit_metadata WHERE seq = ?1",
                params![seq],
                |r| r.get(0),
            )
            .optional()?;
        match stored {
            Some(h) if h == hash => {}
            Some(_) => {
                return Ok(Some(TailProblem {
                    seq: Some(seq),
                    reason: format!(
                        "the audit row at seq {seq} differs from the anchor recorded by backup {backup}"
                    ),
                }))
            }
            None => {
                return Ok(Some(TailProblem {
                    seq: Some(seq),
                    reason: format!(
                        "the audit row at seq {seq}, the anchor recorded by backup {backup}, is missing"
                    ),
                }))
            }
        }
    }
    Ok(None)
}

/// Append-only hash chain over `audit_metadata`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditChain;

impl AuditChain {
    /// Append an entry with a fresh UUIDv7 id. Call inside the command's
    /// `BEGIN IMMEDIATE` transaction so that the chain head cannot move between
    /// reading `prev_hash` and inserting the row.
    pub fn append(conn: &Connection, entry: &AuditEntry) -> StorageResult<AuditRecord> {
        Self::append_with_id(conn, AuditRef::new_v7(), entry)
    }

    /// Append an entry with a caller-supplied id (idempotent replays, fixtures).
    pub fn append_with_id(
        conn: &Connection,
        id: AuditRef,
        entry: &AuditEntry,
    ) -> StorageResult<AuditRecord> {
        if let Some(d) = &entry.details_sha256 {
            require_sha256("details_sha256", d)?;
        }
        let (actor_id, actor_kind, actor_skill_id) = actor_columns(&entry.actor);
        let decision = enum_to_text(&entry.decision)?;
        let outcome = enum_to_text(&entry.outcome)?;
        let resource_kind = match &entry.resource {
            Some(r) => Some(enum_to_text(&r.kind)?),
            None => None,
        };
        let resource_id = entry.resource.as_ref().map(|r| r.id.as_str());
        let prev_hash = Self::head(conn)?
            .map(|(_, h)| h)
            .unwrap_or_else(|| GENESIS_PREV_HASH.to_string());
        let body = RowBody {
            id: id.as_str(),
            occurred_at: entry.occurred_at.as_millis(),
            actor_id: &actor_id,
            actor_kind,
            actor_skill_id: actor_skill_id.as_deref(),
            operation: &entry.operation,
            resource_kind: resource_kind.as_deref(),
            resource_id,
            decision: &decision,
            outcome: &outcome,
            request_id: entry.request_id.as_str(),
            trace_id: entry.trace_id.as_str(),
            details_sha256: entry.details_sha256.as_deref(),
        };
        let row_hash = chain_hash(&prev_hash, &body)?;
        conn.execute(
            "INSERT INTO audit_metadata (id, prev_hash, row_hash, occurred_at, actor_id, actor_kind, actor_skill_id, operation,
               resource_kind, resource_id, decision, outcome, request_id, trace_id, details_sha256)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            params![
                body.id,
                prev_hash,
                row_hash,
                body.occurred_at,
                body.actor_id,
                body.actor_kind,
                body.actor_skill_id,
                body.operation,
                body.resource_kind,
                body.resource_id,
                body.decision,
                body.outcome,
                body.request_id,
                body.trace_id,
                body.details_sha256,
            ],
        )?;
        let seq = conn.last_insert_rowid();
        Ok(AuditRecord {
            audit_ref: id,
            seq,
            prev_hash,
            row_hash,
        })
    }

    /// Chain head `(seq, row_hash)`, if any row exists.
    pub fn head(conn: &Connection) -> StorageResult<Option<(i64, String)>> {
        Ok(conn
            .query_row(
                "SELECT seq, row_hash FROM audit_metadata ORDER BY seq DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?)
    }

    /// Recompute every link of the chain from the stored columns.
    pub fn verify_chain(conn: &Connection) -> StorageResult<ChainReport> {
        let mut stmt = conn.prepare(
            "SELECT seq, id, prev_hash, row_hash, occurred_at, actor_id, actor_kind, actor_skill_id, operation,
                    resource_kind, resource_id, decision, outcome, request_id, trace_id, details_sha256
             FROM audit_metadata ORDER BY seq",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, Option<String>>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, Option<String>>(9)?,
                r.get::<_, Option<String>>(10)?,
                r.get::<_, String>(11)?,
                r.get::<_, String>(12)?,
                r.get::<_, String>(13)?,
                r.get::<_, String>(14)?,
                r.get::<_, Option<String>>(15)?,
            ))
        })?;
        let mut expected_prev = GENESIS_PREV_HASH.to_string();
        let mut count = 0_u64;
        let mut head = None;
        let mut newest = 0_i64;
        for row in rows {
            let (
                seq,
                id,
                prev_hash,
                row_hash,
                occurred_at,
                actor_id,
                actor_kind,
                actor_skill_id,
                operation,
                resource_kind,
                resource_id,
                decision,
                outcome,
                request_id,
                trace_id,
                details_sha256,
            ) = row?;
            count += 1;
            if prev_hash != expected_prev {
                return Ok(ChainReport {
                    rows: count,
                    ok: false,
                    first_bad_seq: Some(seq),
                    first_bad_reason: Some(
                        "prev_hash does not match previous row_hash".to_string(),
                    ),
                    head_hash: head,
                });
            }
            let body = RowBody {
                id: &id,
                occurred_at,
                actor_id: &actor_id,
                actor_kind: &actor_kind,
                actor_skill_id: actor_skill_id.as_deref(),
                operation: &operation,
                resource_kind: resource_kind.as_deref(),
                resource_id: resource_id.as_deref(),
                decision: &decision,
                outcome: &outcome,
                request_id: &request_id,
                trace_id: &trace_id,
                details_sha256: details_sha256.as_deref(),
            };
            let recomputed = chain_hash(&prev_hash, &body)?;
            if recomputed != row_hash {
                return Ok(ChainReport {
                    rows: count,
                    ok: false,
                    first_bad_seq: Some(seq),
                    first_bad_reason: Some("row_hash does not match recomputed hash".to_string()),
                    head_hash: head,
                });
            }
            expected_prev = row_hash.clone();
            head = Some(row_hash);
            newest = seq;
        }
        if let Some(problem) = tail_problem(conn, newest)? {
            return Ok(ChainReport {
                rows: count,
                ok: false,
                first_bad_seq: problem.seq,
                first_bad_reason: Some(problem.reason),
                head_hash: head,
            });
        }
        Ok(ChainReport {
            rows: count,
            ok: true,
            first_bad_seq: None,
            first_bad_reason: None,
            head_hash: head,
        })
    }

    /// Read one entry back (metadata only) by id.
    pub fn get(
        conn: &Connection,
        id: &AuditRef,
    ) -> StorageResult<Option<(AuditRecord, AuditEntry)>> {
        let row = conn
            .query_row(
                "SELECT seq, prev_hash, row_hash, occurred_at, actor_id, actor_kind, actor_skill_id, operation,
                        resource_kind, resource_id, decision, outcome, request_id, trace_id, details_sha256
                 FROM audit_metadata WHERE id = ?1",
                params![id.as_str()],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, Option<String>>(6)?,
                        r.get::<_, String>(7)?,
                        r.get::<_, Option<String>>(8)?,
                        r.get::<_, Option<String>>(9)?,
                        r.get::<_, String>(10)?,
                        r.get::<_, String>(11)?,
                        r.get::<_, String>(12)?,
                        r.get::<_, String>(13)?,
                        r.get::<_, Option<String>>(14)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            seq,
            prev_hash,
            row_hash,
            occurred_at,
            actor_id,
            actor_kind,
            actor_skill_id,
            operation,
            resource_kind,
            resource_id,
            decision,
            outcome,
            request_id,
            trace_id,
            details_sha256,
        )) = row
        else {
            return Ok(None);
        };
        let resource = match (resource_kind, resource_id) {
            (Some(k), Some(i)) => Some(ResourceRef::new(enum_from_text::<ResourceKind>(&k)?, i)),
            _ => None,
        };
        let entry = AuditEntry {
            occurred_at: UtcMillis::from_millis(occurred_at),
            actor: actor_from_columns(&actor_id, &actor_kind, actor_skill_id.as_deref())?,
            operation,
            resource,
            decision: enum_from_text(&decision)?,
            outcome: enum_from_text(&outcome)?,
            request_id: crate::sql::parse_id(&request_id)?,
            trace_id: crate::sql::parse_id(&trace_id)?,
            details_sha256,
        };
        Ok(Some((
            AuditRecord {
                audit_ref: id.clone(),
                seq,
                prev_hash,
                row_hash,
            },
            entry,
        )))
    }
}
