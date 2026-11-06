//! Append-only `domain_events` (handoff §7 "event histories for significant
//! facts"). Every event carries the typed [`EventHeader`] (actor, trust,
//! schema version, correlation id, optional content hash), a JSON payload and
//! the payload's SHA-256. Rows are immutable by trigger.
//!
//! The event store is metadata-plus-structured-facts, not a content store:
//! `Classification::Secret` payloads are refused (sealed keys, credentials and
//! raw prompts never enter the ledger).

use crate::error::{StorageError, StorageResult};
use crate::sql::{
    actor_columns, actor_from_columns, enum_from_text, enum_to_text, json_from_text, parse_id,
};
use g9_domain::hashing::sha256_json;
use g9_domain::{Classification, DataDomain, EventHeader, EventId, SchemaVersion, UtcMillis};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

/// A significant-fact event to append.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainEvent {
    /// Typed header from `g9-domain`.
    pub header: EventHeader,
    /// Structured payload (facts and ids, never raw content).
    pub payload: serde_json::Value,
    /// Privacy domain of the payload.
    pub data_domain: DataDomain,
    /// Classification of the payload (`Secret` is refused).
    pub classification: Classification,
}

/// Result of [`append_event`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRecord {
    /// Ledger position.
    pub seq: i64,
    /// Event id.
    pub event_id: EventId,
    /// Hex SHA-256 of the canonical payload.
    pub payload_sha256: String,
}

/// A stored event read back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredEvent {
    /// Ledger position.
    pub seq: i64,
    /// The event.
    pub event: DomainEvent,
    /// Hex SHA-256 of the canonical payload as stored.
    pub payload_sha256: String,
}

/// Append an event. Returns `InvalidInput` for `Secret` payloads.
pub fn append_event(conn: &Connection, event: &DomainEvent) -> StorageResult<EventRecord> {
    if event.classification == Classification::Secret {
        return Err(StorageError::InvalidInput(
            "domain_events never store SECRET payloads".to_string(),
        ));
    }
    if let Some(h) = &event.header.content_sha256 {
        crate::sql::require_sha256("content_sha256", h)?;
    }
    let (actor_id, actor_kind, actor_skill_id) = actor_columns(&event.header.actor);
    let payload_json = serde_json::to_string(&event.payload)
        .map_err(|_| StorageError::Internal("payload json"))?;
    let payload_sha256 = sha256_json(&event.payload);
    conn.execute(
        "INSERT INTO domain_events (id, kind, occurred_at, actor_id, actor_kind, actor_skill_id, trust, schema_version,
            correlation_id, content_sha256, payload_json, payload_sha256, data_domain, classification)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            event.header.event_id.as_str(),
            event.header.kind,
            event.header.occurred_at.as_millis(),
            actor_id,
            actor_kind,
            actor_skill_id,
            enum_to_text(&event.header.trust)?,
            event.header.schema_version.to_string(),
            event.header.correlation_id.as_str(),
            event.header.content_sha256,
            payload_json,
            payload_sha256,
            enum_to_text(&event.data_domain)?,
            enum_to_text(&event.classification)?,
        ],
    )?;
    Ok(EventRecord {
        seq: conn.last_insert_rowid(),
        event_id: event.header.event_id.clone(),
        payload_sha256,
    })
}

/// Events with `seq > after_seq`, oldest first, at most `limit`.
pub fn list_since(
    conn: &Connection,
    after_seq: i64,
    limit: u32,
) -> StorageResult<Vec<StoredEvent>> {
    let mut stmt = conn.prepare(
        "SELECT seq, id, kind, occurred_at, actor_id, actor_kind, actor_skill_id, trust, schema_version, correlation_id,
                content_sha256, payload_json, payload_sha256, data_domain, classification
         FROM domain_events WHERE seq > ?1 ORDER BY seq LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![after_seq, i64::from(limit)], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, String>(7)?,
            r.get::<_, String>(8)?,
            r.get::<_, String>(9)?,
            r.get::<_, Option<String>>(10)?,
            r.get::<_, String>(11)?,
            r.get::<_, String>(12)?,
            r.get::<_, String>(13)?,
            r.get::<_, String>(14)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (
            seq,
            id,
            kind,
            occurred_at,
            actor_id,
            actor_kind,
            actor_skill_id,
            trust,
            schema_version,
            correlation_id,
            content_sha256,
            payload_json,
            payload_sha256,
            data_domain,
            classification,
        ) = row?;
        out.push(StoredEvent {
            seq,
            event: DomainEvent {
                header: EventHeader {
                    event_id: parse_id(&id)?,
                    kind,
                    occurred_at: UtcMillis::from_millis(occurred_at),
                    actor: actor_from_columns(&actor_id, &actor_kind, actor_skill_id.as_deref())?,
                    trust: enum_from_text(&trust)?,
                    schema_version: parse_schema_version(&schema_version)?,
                    correlation_id: parse_id(&correlation_id)?,
                    content_sha256,
                },
                payload: json_from_text(&payload_json)?,
                data_domain: enum_from_text(&data_domain)?,
                classification: enum_from_text(&classification)?,
            },
            payload_sha256,
        });
    }
    Ok(out)
}

fn parse_schema_version(text: &str) -> StorageResult<SchemaVersion> {
    let (major, minor) = text
        .split_once('.')
        .ok_or_else(|| StorageError::RowDecode(format!("schema_version {text:?}")))?;
    let major = major
        .parse::<u16>()
        .map_err(|_| StorageError::RowDecode(format!("schema_version {text:?}")))?;
    let minor = minor
        .parse::<u16>()
        .map_err(|_| StorageError::RowDecode(format!("schema_version {text:?}")))?;
    Ok(SchemaVersion::new(major, minor))
}
