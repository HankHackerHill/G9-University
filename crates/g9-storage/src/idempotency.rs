//! Idempotent replay store (ARCHITECTURE.md §5 steps 2 and 8, §6 "Idempotency").
//!
//! Key: `(actor_id, idempotency_key)`. A hit with the same `request_sha256`
//! returns the stored outcome (the caller sets `CommandOutcome::replayed`);
//! a hit with a different hash is a `CONFLICT` (same key reused for a different
//! request). Records are purged by retention (`purge_older_than`, 30 days by
//! default per `docs/privacy/data-lifecycle.md`).

use crate::error::{StorageError, StorageResult};
use crate::sql::{json_from_text, require_sha256};
use g9_domain::hashing::sha256_json;
use g9_domain::{ActorId, IdempotencyKey, UtcMillis};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// A stored command outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredOutcome {
    /// Hash of the request that produced the outcome.
    pub request_sha256: String,
    /// The serialized `CommandOutcome` (or error envelope) as stored.
    pub outcome_json: serde_json::Value,
    /// When it was stored.
    pub created_at: UtcMillis,
}

/// Result of [`check`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IdempotencyCheck {
    /// No record; execute the command.
    Miss,
    /// Same request seen before; return the stored outcome with `replayed = true`.
    Replay(StoredOutcome),
    /// Key reused with a different request hash.
    Conflict {
        /// Hash of the request originally stored under this key.
        stored_request_sha256: String,
    },
}

/// Hex SHA-256 of a request document (canonical JSON).
pub fn request_hash(request: &serde_json::Value) -> String {
    sha256_json(request)
}

/// Look up a record.
pub fn lookup(
    conn: &Connection,
    actor_id: &ActorId,
    key: &IdempotencyKey,
) -> StorageResult<Option<StoredOutcome>> {
    let row = conn
        .query_row(
            "SELECT request_sha256, outcome_json, created_at FROM idempotency_records WHERE actor_id = ?1 AND idempotency_key = ?2",
            params![actor_id.as_str(), key.as_str()],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?)),
        )
        .optional()?;
    match row {
        None => Ok(None),
        Some((request_sha256, outcome_json, created_at)) => Ok(Some(StoredOutcome {
            request_sha256,
            outcome_json: json_from_text(&outcome_json)?,
            created_at: UtcMillis::from_millis(created_at),
        })),
    }
}

/// Pipeline step 2: miss, replay or conflict.
pub fn check(
    conn: &Connection,
    actor_id: &ActorId,
    key: &IdempotencyKey,
    request_sha256: &str,
) -> StorageResult<IdempotencyCheck> {
    require_sha256("request_sha256", request_sha256)?;
    Ok(match lookup(conn, actor_id, key)? {
        None => IdempotencyCheck::Miss,
        Some(stored) if stored.request_sha256 == request_sha256 => IdempotencyCheck::Replay(stored),
        Some(stored) => IdempotencyCheck::Conflict {
            stored_request_sha256: stored.request_sha256,
        },
    })
}

/// Pipeline step 8: store the outcome. Storing the same `(actor, key, hash)`
/// twice is a no-op; a different hash under the same key is a `Conflict`.
pub fn store(
    conn: &Connection,
    actor_id: &ActorId,
    key: &IdempotencyKey,
    request_sha256: &str,
    outcome: &serde_json::Value,
    created_at: UtcMillis,
) -> StorageResult<()> {
    require_sha256("request_sha256", request_sha256)?;
    if let Some(existing) = lookup(conn, actor_id, key)? {
        if existing.request_sha256 == request_sha256 {
            return Ok(());
        }
        return Err(StorageError::Conflict(
            "idempotency key already used for a different request".to_string(),
        ));
    }
    let outcome_json =
        serde_json::to_string(outcome).map_err(|_| StorageError::Internal("outcome json"))?;
    conn.execute(
        "INSERT INTO idempotency_records (actor_id, idempotency_key, request_sha256, outcome_json, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![actor_id.as_str(), key.as_str(), request_sha256, outcome_json, created_at.as_millis()],
    )?;
    Ok(())
}

/// Delete records created before `cutoff`; returns the number removed.
pub fn purge_older_than(conn: &Connection, cutoff: UtcMillis) -> StorageResult<usize> {
    Ok(conn.execute(
        "DELETE FROM idempotency_records WHERE created_at < ?1",
        params![cutoff.as_millis()],
    )?)
}
