//! Typed command/error envelope (handoff §6 "Typed command/error envelope").
//!
//! Every IPC operation returns either a typed success result or a stable error
//! category. Secrets and Rust backtraces never cross this boundary.

use crate::actor::Actor;
use crate::ids::{ApprovalId, AuditRef, IdempotencyKey, RequestId, TraceId};
use crate::mode::InteractionMode;
use crate::policy::ResourceKind;
use crate::privacy::DataDomain;
use crate::revision::Revision;
use crate::time::UtcMillis;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Stable error categories. The set is closed; add a category only with a
/// schema version bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCategory {
    InvalidInput,
    Unauthorized,
    ApprovalRequired,
    StaleRevision,
    PreconditionFailed,
    NotFound,
    Conflict,
    ResourceUnavailable,
    Timeout,
    Cancelled,
    RetryableStorage,
    DataCorruption,
    Internal,
}

impl ErrorCategory {
    pub const ALL: [ErrorCategory; 13] = [
        ErrorCategory::InvalidInput,
        ErrorCategory::Unauthorized,
        ErrorCategory::ApprovalRequired,
        ErrorCategory::StaleRevision,
        ErrorCategory::PreconditionFailed,
        ErrorCategory::NotFound,
        ErrorCategory::Conflict,
        ErrorCategory::ResourceUnavailable,
        ErrorCategory::Timeout,
        ErrorCategory::Cancelled,
        ErrorCategory::RetryableStorage,
        ErrorCategory::DataCorruption,
        ErrorCategory::Internal,
    ];

    pub const fn is_retryable(self) -> bool {
        matches!(
            self,
            ErrorCategory::RetryableStorage
                | ErrorCategory::Timeout
                | ErrorCategory::ResourceUnavailable
        )
    }
}

/// The only error type that crosses the IPC boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct G9Error {
    pub category: ErrorCategory,
    /// Machine-readable code, e.g. `curriculum.invariant_violated`.
    pub code: String,
    /// Human-readable, secret-free message.
    pub message: String,
    /// Optional structured, secret-free details (validation errors, ids).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// Set when `category == ApprovalRequired` and an approval request was created.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<ApprovalId>,
}

impl G9Error {
    pub fn new(
        category: ErrorCategory,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            category,
            code: code.into(),
            message: message.into(),
            details: None,
            approval_id: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_approval(mut self, approval_id: ApprovalId) -> Self {
        self.approval_id = Some(approval_id);
        self
    }

    pub fn invalid_input(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::InvalidInput, code, message)
    }

    pub fn unauthorized(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Unauthorized, code, message)
    }

    pub fn approval_required(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::ApprovalRequired, code, message)
    }

    pub fn stale_revision(expected: Option<Revision>, actual: Revision) -> Self {
        Self::new(
            ErrorCategory::StaleRevision,
            "revision.stale",
            "the record changed since it was read; reload and retry",
        )
        .with_details(serde_json::json!({
            "expected": expected.map(Revision::get),
            "actual": actual.get(),
        }))
    }

    pub fn precondition_failed(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::PreconditionFailed, code, message)
    }

    pub fn not_found(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::NotFound, code, message)
    }

    pub fn conflict(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Conflict, code, message)
    }

    pub fn resource_unavailable(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::ResourceUnavailable, code, message)
    }

    pub fn timeout(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Timeout, code, message)
    }

    pub fn cancelled(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::Cancelled, code, message)
    }

    pub fn retryable_storage(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::RetryableStorage, code, message)
    }

    pub fn data_corruption(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(ErrorCategory::DataCorruption, code, message)
    }

    /// Internal error with a redacted message. The real cause must be logged
    /// locally (metadata-only) and never returned to the UI.
    pub fn internal_redacted(code: impl Into<String>) -> Self {
        Self::new(
            ErrorCategory::Internal,
            code,
            "internal error; see local operation log",
        )
    }
}

impl fmt::Display for G9Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?} [{}]: {}", self.category, self.code, self.message)
    }
}

impl std::error::Error for G9Error {}

/// Reference to a protected resource.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceRef {
    pub kind: ResourceKind,
    pub id: String,
}

impl ResourceRef {
    pub fn new(kind: ResourceKind, id: impl Into<String>) -> Self {
        Self {
            kind,
            id: id.into(),
        }
    }
}

/// Scope of a command: which privacy domain and which interaction mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Scope {
    pub domain: DataDomain,
    pub mode: InteractionMode,
}

/// Semantic version of a payload schema, e.g. `1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SchemaVersion {
    pub major: u16,
    pub minor: u16,
}

impl SchemaVersion {
    pub const V1: SchemaVersion = SchemaVersion { major: 1, minor: 0 };

    pub const fn new(major: u16, minor: u16) -> Self {
        Self { major, minor }
    }

    /// A reader at `self` can accept a payload at `other` if the major matches
    /// and the payload minor is not newer.
    pub const fn accepts(self, other: SchemaVersion) -> bool {
        self.major == other.major && other.minor <= self.minor
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Required metadata on every mutating command (handoff §6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandMeta {
    pub request_id: RequestId,
    pub actor: Actor,
    /// Operation name, e.g. `session.start`.
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<ResourceRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<Revision>,
    pub idempotency_key: IdempotencyKey,
    pub scope: Scope,
    pub payload_schema_version: SchemaVersion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<ApprovalId>,
    pub trace_id: TraceId,
    pub issued_at: UtcMillis,
}

/// Success envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandOutcome<T> {
    pub result: T,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<Revision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audit_ref: Option<AuditRef>,
    /// True when this response was replayed from the idempotency store.
    #[serde(default)]
    pub replayed: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl<T> CommandOutcome<T> {
    pub fn new(result: T) -> Self {
        Self {
            result,
            revision: None,
            audit_ref: None,
            replayed: false,
            warnings: Vec::new(),
        }
    }

    pub fn with_revision(mut self, revision: Revision) -> Self {
        self.revision = Some(revision);
        self
    }

    pub fn with_audit_ref(mut self, audit_ref: AuditRef) -> Self {
        self.audit_ref = Some(audit_ref);
        self
    }
}

pub type CommandResult<T> = Result<CommandOutcome<T>, G9Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_serializes_without_backtrace_or_secret_fields() {
        let e = G9Error::internal_redacted("storage.open");
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json["category"], "INTERNAL");
        assert!(json.get("backtrace").is_none());
        assert_eq!(json["message"], "internal error; see local operation log");
    }

    #[test]
    fn categories_are_exactly_thirteen_and_closed() {
        assert_eq!(ErrorCategory::ALL.len(), 13);
        assert!(ErrorCategory::RetryableStorage.is_retryable());
        assert!(!ErrorCategory::Unauthorized.is_retryable());
    }

    #[test]
    fn schema_version_compat() {
        assert!(SchemaVersion::new(1, 2).accepts(SchemaVersion::new(1, 0)));
        assert!(!SchemaVersion::new(1, 0).accepts(SchemaVersion::new(1, 1)));
        assert!(!SchemaVersion::new(2, 0).accepts(SchemaVersion::new(1, 0)));
    }
}
