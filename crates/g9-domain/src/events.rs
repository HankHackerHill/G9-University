//! Header for significant-fact events (handoff §7 "event histories for
//! significant facts, not event sourcing for every UI pixel").

use crate::actor::Actor;
use crate::envelope::SchemaVersion;
use crate::ids::{EventId, TraceId};
use crate::time::UtcMillis;
use crate::trust::TrustClass;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventHeader {
    pub event_id: EventId,
    /// Event kind, e.g. `assessment.attempt.finalized`.
    pub kind: String,
    pub occurred_at: UtcMillis,
    pub actor: Actor,
    pub trust: TrustClass,
    pub schema_version: SchemaVersion,
    pub correlation_id: TraceId,
    /// Hex SHA-256 of the immutable event content, when applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_sha256: Option<String>,
}
