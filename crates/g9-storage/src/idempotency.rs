//! Idempotent replay store (ARCHITECTURE.md §5 steps 2 and 8, §6 "Idempotency").
//!
//! Key: `(actor_id, idempotency_key)`. A hit with the same `request_sha256`
//! returns the stored outcome (the caller sets `CommandOutcome::replayed`);
//! a hit with a different hash is a `CONFLICT` (same key reused for a different
//! request). Records are kept for [`RETENTION_DAYS`] (30 days,
//! `docs/privacy/data-lifecycle.md`): `g9-app` recovery purges older ones at
//! open ([`purge_older_than`] with [`retention_cutoff`]). A stored result that
//! repeats a record which no longer exists is found with [`results_quoting`]
//! and replaced with [`replace_outcome`] (data inventory gap G1).

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

/// Days an idempotency record is kept (`docs/privacy/data-lifecycle.md`).
pub const RETENTION_DAYS: i64 = 30;

/// The instant before which records are past retention at `now`.
pub fn retention_cutoff(now: UtcMillis) -> UtcMillis {
    UtcMillis::from_millis(
        now.as_millis()
            .saturating_sub(RETENTION_DAYS.saturating_mul(86_400_000)),
    )
}

/// A stored outcome found by [`results_quoting`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuotingRecord {
    /// Actor of the record.
    pub actor_id: String,
    /// Idempotency key of the record.
    pub idempotency_key: String,
    /// The stored outcome.
    pub outcome_json: serde_json::Value,
}

/// Records whose stored `result` holds an object in which `id_key` is `id` and
/// `content_key` is present: the view of that record, with its content, kept
/// inside the result (a calendar event's `event_id` and `title`). Found by the
/// JSON structure, not by searching for text (the `instr` test only skips
/// records that cannot match). One pass over each stored result: the sibling
/// `content_key` is looked up by the path of the object that holds `id_key`
/// (a self-join of two `json_tree` scans was quadratic in the size of one
/// result). `content_key` is a key name chosen by the caller, never input.
pub fn results_quoting(
    conn: &Connection,
    id_key: &str,
    id: &str,
    content_key: &str,
) -> StorageResult<Vec<QuotingRecord>> {
    let mut stmt = conn.prepare(
        "SELECT r.actor_id, r.idempotency_key, r.outcome_json FROM idempotency_records r
          WHERE instr(r.outcome_json, ?2) > 0
            AND EXISTS (
            SELECT 1 FROM json_tree(r.outcome_json, '$.result') a
             WHERE a.key = ?1 AND a.type = 'text' AND a.atom = ?2
               AND json_type(r.outcome_json, a.path || '.\"' || ?3 || '\"') IS NOT NULL)
          ORDER BY r.created_at, r.actor_id, r.idempotency_key",
    )?;
    let rows = stmt.query_map(params![id_key, id, content_key], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (actor_id, idempotency_key, outcome) = row?;
        out.push(QuotingRecord {
            actor_id,
            idempotency_key,
            outcome_json: json_from_text(&outcome)?,
        });
    }
    Ok(out)
}

/// Replace the stored outcome of one record (same request hash, same time).
pub fn replace_outcome(
    conn: &Connection,
    actor_id: &str,
    idempotency_key: &str,
    outcome: &serde_json::Value,
) -> StorageResult<()> {
    let outcome_json =
        serde_json::to_string(outcome).map_err(|_| StorageError::Internal("outcome json"))?;
    let changed = conn.execute(
        "UPDATE idempotency_records SET outcome_json = ?3 WHERE actor_id = ?1 AND idempotency_key = ?2",
        params![actor_id, idempotency_key, outcome_json],
    )?;
    if changed == 1 {
        Ok(())
    } else {
        Err(StorageError::NotFound("idempotency record"))
    }
}

/// The purge cutoff at `now`: [`RETENTION_DAYS`] before the earlier of `now` and
/// the newest stored record. The wall clock alone cannot be trusted for elapsed
/// time: an open with the clock weeks ahead would otherwise purge fresh records,
/// and a retry with the same key after the clock was corrected would run the
/// command a second time. A record is therefore purged only when a command was
/// recorded at least [`RETENTION_DAYS`] after it (and the clock agrees), so it
/// can be kept up to [`RETENTION_DAYS`] past the last recorded command.
pub fn purge_cutoff(conn: &Connection, now: UtcMillis) -> StorageResult<UtcMillis> {
    let newest: Option<i64> =
        conn.query_row("SELECT MAX(created_at) FROM idempotency_records", [], |r| {
            r.get(0)
        })?;
    let anchor = newest.map_or(now.as_millis(), |n| n.min(now.as_millis()));
    Ok(retention_cutoff(UtcMillis::from_millis(anchor)))
}

/// Delete records created before `cutoff`; returns the number removed.
pub fn purge_older_than(conn: &Connection, cutoff: UtcMillis) -> StorageResult<usize> {
    Ok(conn.execute(
        "DELETE FROM idempotency_records WHERE created_at < ?1",
        params![cutoff.as_millis()],
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, OpenOptions};
    use serde_json::json;

    fn db() -> Database {
        Database::open_in_memory(OpenOptions::new("test").applied_at(UtcMillis::from_millis(1)))
            .unwrap()
    }

    fn put(db: &Database, key: &str, result: serde_json::Value, at: i64) {
        store(
            db.conn(),
            &ActorId::parse("operator-1").unwrap(),
            &IdempotencyKey::parse(key).unwrap(),
            &"a".repeat(64),
            &json!({ "result": result, "replayed": false }),
            UtcMillis::from_millis(at),
        )
        .unwrap();
    }

    #[test]
    fn results_quoting_finds_the_view_of_a_record_by_structure_only() {
        let db = db();
        put(
            &db,
            "k-create",
            json!({ "event_id": "ev-1", "title": "Dentist" }),
            1,
        );
        put(
            &db,
            "k-apply",
            json!({ "events": [{ "event_id": "ev-2" , "title": "Other" }, { "event_id": "ev-1", "title": "Dentist" }] }),
            2,
        );
        // Named but no content next to it; content of another event; the id as text elsewhere.
        put(
            &db,
            "k-delete",
            json!({ "event_id": "ev-1", "deleted": true }),
            3,
        );
        put(
            &db,
            "k-other",
            json!({ "event_id": "ev-2", "title": "ev-1" }),
            4,
        );
        put(&db, "k-text", json!({ "note": "ev-1", "title": "x" }), 5);

        let found = results_quoting(db.conn(), "event_id", "ev-1", "title").unwrap();
        let keys: Vec<&str> = found.iter().map(|r| r.idempotency_key.as_str()).collect();
        assert_eq!(keys, ["k-create", "k-apply"]);

        replace_outcome(
            db.conn(),
            "operator-1",
            "k-create",
            &json!({ "result": { "replaced": true }, "replayed": false }),
        )
        .unwrap();
        let keys: Vec<String> = results_quoting(db.conn(), "event_id", "ev-1", "title")
            .unwrap()
            .into_iter()
            .map(|r| r.idempotency_key)
            .collect();
        assert_eq!(keys, ["k-apply"]);
        assert!(matches!(
            replace_outcome(db.conn(), "operator-1", "k-missing", &json!({})),
            Err(StorageError::NotFound(_))
        ));
    }

    #[test]
    fn results_quoting_is_linear_in_the_size_of_a_stored_outcome() {
        // Final security review ipc-3: a self-join of two json_tree scans made
        // the search quadratic in how often one outcome repeats the id (about
        // 5 s at 4,000 repeats in a debug build). A restored snapshot can hold
        // such a row; deleting the event then froze the window.
        let db = db();
        let repeats: Vec<serde_json::Value> =
            (0..20_000).map(|_| json!({ "event_id": "ev-1" })).collect();
        put(&db, "k-big", json!({ "items": repeats }), 1);
        put(
            &db,
            "k-view",
            json!({ "event_id": "ev-1", "title": "Dentist" }),
            2,
        );
        let started = std::time::Instant::now();
        let found = results_quoting(db.conn(), "event_id", "ev-1", "title").unwrap();
        let took = started.elapsed();
        let keys: Vec<&str> = found.iter().map(|r| r.idempotency_key.as_str()).collect();
        assert_eq!(keys, ["k-view"]);
        assert!(took < std::time::Duration::from_secs(5), "took {took:?}");
    }

    #[test]
    fn retention_is_thirty_days_and_purge_removes_only_older_records() {
        let db = db();
        let day = 86_400_000;
        let now = UtcMillis::from_millis(40 * day);
        let cutoff = retention_cutoff(now);
        assert_eq!(cutoff.as_millis(), 10 * day);
        put(&db, "old", json!({}), 10 * day - 1);
        put(&db, "edge", json!({}), 10 * day);
        put(&db, "new", json!({}), 39 * day);
        assert_eq!(purge_older_than(db.conn(), cutoff).unwrap(), 1);
        let left: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM idempotency_records", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 2);
        assert_eq!(
            retention_cutoff(UtcMillis::from_millis(i64::MIN)).as_millis(),
            i64::MIN
        );
    }
}
