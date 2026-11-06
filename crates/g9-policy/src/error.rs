//! Crate error type and its mapping to the closed `G9Error` category set.

use crate::approvals::ApprovalError;
use crate::record::PolicyDecisionRecord;
use crate::rules::RuleSetError;
use g9_domain::{Capability, ErrorCategory, G9Error};

/// Errors produced by the engine and the authorization flow. Every variant
/// that results from an evaluation carries the metadata-only decision record
/// so that `g9-app` can persist a `policy_decisions` row before returning.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PolicyError {
    /// The operation is refused.
    #[error("denied by rule {}: {}", record.rule_id, record.reason)]
    Denied {
        /// The `DENY` record.
        record: Box<PolicyDecisionRecord>,
    },
    /// The operation needs an approval granting `required_capability`.
    #[error("approval required (capability {required_capability:?}) by rule {}", record.rule_id)]
    ApprovalRequired {
        /// The `APPROVAL_REQUIRED` record.
        record: Box<PolicyDecisionRecord>,
        /// Capability the approval must grant.
        required_capability: Capability,
    },
    /// An approval was supplied but could not be consumed for this operation.
    #[error("approval rejected: {source}")]
    Approval {
        /// Why the approval was rejected.
        source: ApprovalError,
        /// The `DENY` record logged for the failed consumption.
        record: Box<PolicyDecisionRecord>,
    },
    /// The rule table is structurally invalid.
    #[error("invalid rule set: {0}")]
    InvalidRuleSet(#[from] RuleSetError),
}

impl PolicyError {
    /// The decision record to persist, if the error came from an evaluation.
    pub fn record(&self) -> Option<&PolicyDecisionRecord> {
        match self {
            PolicyError::Denied { record }
            | PolicyError::ApprovalRequired { record, .. }
            | PolicyError::Approval { record, .. } => Some(record),
            PolicyError::InvalidRuleSet(_) => None,
        }
    }
}

impl From<PolicyError> for G9Error {
    fn from(e: PolicyError) -> Self {
        match e {
            PolicyError::Denied { record } => G9Error::unauthorized("policy.denied", record.reason.clone()).with_details(
                serde_json::json!({ "rule_id": record.rule_id, "action": record.action, "resource_kind": record.resource_kind }),
            ),
            PolicyError::ApprovalRequired {
                record,
                required_capability,
            } => G9Error::approval_required("policy.approval_required", record.reason.clone()).with_details(serde_json::json!({
                "rule_id": record.rule_id,
                "required_capability": required_capability,
                "action": record.action,
                "resource_kind": record.resource_kind,
            })),
            PolicyError::Approval { source, .. } => source.into(),
            PolicyError::InvalidRuleSet(err) => err.into(),
        }
    }
}

impl From<RuleSetError> for G9Error {
    fn from(e: RuleSetError) -> Self {
        G9Error::precondition_failed("policy.rule_set_invalid", e.to_string())
    }
}

impl From<ApprovalError> for G9Error {
    fn from(e: ApprovalError) -> Self {
        let code = e.code();
        let message = e.to_string();
        match e {
            ApprovalError::NotFound => G9Error::not_found(code, message),
            ApprovalError::NotApproved { .. } | ApprovalError::Expired => {
                G9Error::approval_required(code, message)
            }
            ApprovalError::OperationMismatch
            | ApprovalError::ResourceMismatch
            | ApprovalError::AlreadyConsumed
            | ApprovalError::MacInvalid
            | ApprovalError::CapabilityMismatch { .. }
            | ApprovalError::RequesterMismatch
            | ApprovalError::DeciderNotOperator
            | ApprovalError::OperatorOnlyCapability { .. } => G9Error::unauthorized(code, message),
            ApprovalError::RevisionMismatch { expected, bound } => {
                G9Error::new(ErrorCategory::StaleRevision, code, message).with_details(
                    serde_json::json!({
                        "expected": expected.map(g9_domain::Revision::get),
                        "bound": bound.map(g9_domain::Revision::get),
                    }),
                )
            }
            ApprovalError::AlreadyExists => G9Error::conflict(code, message),
            ApprovalError::InvalidTransition { .. } => G9Error::precondition_failed(code, message),
            ApprovalError::InvalidExpiry | ApprovalError::Binding(_) => {
                G9Error::invalid_input(code, message)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::{ApprovalState, Revision};

    #[test]
    fn approval_errors_map_to_closed_categories() {
        let cases: Vec<(ApprovalError, ErrorCategory)> = vec![
            (ApprovalError::NotFound, ErrorCategory::NotFound),
            (
                ApprovalError::NotApproved {
                    state: ApprovalState::Pending,
                },
                ErrorCategory::ApprovalRequired,
            ),
            (ApprovalError::Expired, ErrorCategory::ApprovalRequired),
            (
                ApprovalError::OperationMismatch,
                ErrorCategory::Unauthorized,
            ),
            (ApprovalError::ResourceMismatch, ErrorCategory::Unauthorized),
            (ApprovalError::AlreadyConsumed, ErrorCategory::Unauthorized),
            (ApprovalError::MacInvalid, ErrorCategory::Unauthorized),
            (
                ApprovalError::RequesterMismatch,
                ErrorCategory::Unauthorized,
            ),
            (
                ApprovalError::CapabilityMismatch {
                    required: Capability::DataExport,
                    granted: Capability::DataDelete,
                },
                ErrorCategory::Unauthorized,
            ),
            (
                ApprovalError::DeciderNotOperator,
                ErrorCategory::Unauthorized,
            ),
            (
                ApprovalError::OperatorOnlyCapability {
                    capability: Capability::ShellExec,
                },
                ErrorCategory::Unauthorized,
            ),
            (
                ApprovalError::RevisionMismatch {
                    expected: Some(Revision::new(2)),
                    bound: Some(Revision::new(1)),
                },
                ErrorCategory::StaleRevision,
            ),
            (ApprovalError::AlreadyExists, ErrorCategory::Conflict),
            (ApprovalError::InvalidExpiry, ErrorCategory::InvalidInput),
        ];
        for (err, cat) in cases {
            let g: G9Error = err.clone().into();
            assert_eq!(g.category, cat, "{err:?}");
            assert!(g.code.starts_with("approval."));
            assert!(g.details.map(|d| d.to_string()).unwrap_or_default().len() < 512);
        }
    }
}
