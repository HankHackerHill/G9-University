//! The only types the Tauri bridge needs (ADR-0013, Addendum A.8).
//!
//! * [`ClientCommandMeta`] is the wire metadata the UI sends: it has **no
//!   actor**. [`parse_meta`] refuses any incoming meta that carries an `actor`
//!   key (whatever its value), validates the rest against
//!   `schemas/ipc/command-meta.schema.json`, and builds the domain
//!   [`CommandMeta`] with the backend-assigned operator actor.
//! * Errors cross the boundary as [`G9Error`] unchanged (closed 13 categories,
//!   secret-free messages).

use crate::schemas;
use g9_domain::{
    Actor, ApprovalId, CommandMeta, G9Error, IdempotencyKey, RequestId, ResourceRef, Revision,
    SchemaVersion, Scope, TraceId, UtcMillis,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use g9_domain::{CommandOutcome, ErrorCategory};

/// Wire metadata as sent by the UI (`ClientCommandMeta` in `apps/desktop/src/ipc/types.ts`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientCommandMeta {
    /// Per-call request id.
    pub request_id: RequestId,
    /// Operation name; must equal the invoked operation.
    pub operation: String,
    /// Optional resource reference; when present it must equal the resource the
    /// backend derives from the payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<ResourceRef>,
    /// Optimistic-concurrency revision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<Revision>,
    /// Idempotency key (replays return the stored outcome).
    pub idempotency_key: IdempotencyKey,
    /// Privacy domain and interaction mode.
    pub scope: Scope,
    /// Payload schema version.
    pub payload_schema_version: SchemaVersion,
    /// Approval presented for an approval-gated retry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<ApprovalId>,
    /// Trace id spanning the operator intent.
    pub trace_id: TraceId,
    /// Client instant (informational; the backend clock decides).
    pub issued_at: UtcMillis,
}

impl ClientCommandMeta {
    /// Attach the backend-assigned actor.
    pub fn into_command_meta(self, actor: Actor) -> CommandMeta {
        CommandMeta {
            request_id: self.request_id,
            actor,
            operation: self.operation,
            resource: self.resource,
            expected_revision: self.expected_revision,
            idempotency_key: self.idempotency_key,
            scope: self.scope,
            payload_schema_version: self.payload_schema_version,
            approval_id: self.approval_id,
            trace_id: self.trace_id,
            issued_at: self.issued_at,
        }
    }
}

/// Decode wire metadata for `op` and assign `operator` as the actor.
///
/// Order: an `actor` key anywhere at the top level is refused with
/// `INVALID_INPUT meta.actor_not_accepted` (the UI can never set identity);
/// then the schema check; then typed decoding; then `meta.operation == op`.
pub fn parse_meta(op: &str, meta: &Value, operator: &Actor) -> Result<CommandMeta, G9Error> {
    let Some(map) = meta.as_object() else {
        return Err(G9Error::invalid_input(
            "meta.not_object",
            "command metadata must be a JSON object",
        ));
    };
    if map.contains_key("actor") {
        return Err(G9Error::invalid_input(
            "meta.actor_not_accepted",
            "command metadata must not carry an actor; the backend assigns it",
        ));
    }
    schemas::validate_meta(meta)?;
    let client: ClientCommandMeta = serde_json::from_value(meta.clone())
        .map_err(|_| G9Error::invalid_input("meta.invalid", "command metadata is malformed"))?;
    if client.operation != op {
        return Err(G9Error::invalid_input(
            "meta.operation_mismatch",
            "meta.operation does not match the invoked operation",
        ));
    }
    Ok(client.into_command_meta(operator.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::ActorId;
    use serde_json::json;

    fn wire() -> Value {
        json!({
            "request_id": "r1", "operation": "session.start", "idempotency_key": "k1",
            "scope": { "domain": "LEARNING", "mode": "STUDY" },
            "payload_schema_version": { "major": 1, "minor": 0 },
            "trace_id": "t1", "issued_at": 5
        })
    }

    #[test]
    fn meta_gets_backend_actor() {
        let op = Actor::operator(ActorId::parse("op").unwrap());
        let meta = parse_meta("session.start", &wire(), &op).unwrap();
        assert_eq!(meta.actor, op);
    }

    #[test]
    fn actor_key_is_refused_even_when_it_matches() {
        let op = Actor::operator(ActorId::parse("op").unwrap());
        let mut w = wire();
        w["actor"] = serde_json::to_value(&op).unwrap();
        let err = parse_meta("session.start", &w, &op).unwrap_err();
        assert_eq!(err.code, "meta.actor_not_accepted");
        assert_eq!(err.category, ErrorCategory::InvalidInput);
    }

    #[test]
    fn operation_mismatch_is_refused() {
        let op = Actor::operator(ActorId::parse("op").unwrap());
        let err = parse_meta("session.pause", &wire(), &op).unwrap_err();
        assert_eq!(err.code, "meta.operation_mismatch");
    }
}
