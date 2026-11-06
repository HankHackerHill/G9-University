//! Metadata-only decision records (ARCHITECTURE.md §11: "Every evaluation
//! produces a `PolicyDecisionRecord` (metadata only) persisted by `g9-app`").
//!
//! A record never contains operation parameters, prompts or private content —
//! only identifiers, enums, the rule id, the reason text of the rule and the
//! SHA-256 of the parameters that the approval (if any) was bound to.

use crate::rules::ResourceContext;
use g9_domain::{
    Action, Actor, ActorId, ActorKind, Capability, Classification, DataDomain, Decision, InteractionMode,
    OperationDescriptor, ResourceKind, UtcMillis,
};
use serde::{Deserialize, Serialize};

/// The three possible outcomes, without payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DecisionKind {
    /// Proceed.
    Allow,
    /// Refuse.
    Deny,
    /// Proceed only with a consumed approval.
    ApprovalRequired,
}

impl DecisionKind {
    /// Classify a domain decision.
    pub fn of(decision: &Decision) -> Self {
        match decision {
            Decision::Allow { .. } => DecisionKind::Allow,
            Decision::Deny { .. } => DecisionKind::Deny,
            Decision::ApprovalRequired { .. } => DecisionKind::ApprovalRequired,
        }
    }
}

/// Rule id carried by a decision.
pub fn decision_rule_id(decision: &Decision) -> &str {
    match decision {
        Decision::Allow { rule_id } | Decision::Deny { rule_id, .. } | Decision::ApprovalRequired { rule_id, .. } => {
            rule_id
        }
    }
}

/// Capability required by a decision, if any.
pub fn decision_capability(decision: &Decision) -> Option<Capability> {
    match decision {
        Decision::ApprovalRequired {
            required_capability, ..
        } => Some(*required_capability),
        Decision::Allow { .. } | Decision::Deny { .. } => None,
    }
}

/// One row of the `policy_decisions` log. Serializable; metadata only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecisionRecord {
    /// Requesting actor id.
    pub actor_id: ActorId,
    /// Requesting actor kind (includes the skill id for skill actors).
    pub actor_kind: ActorKind,
    /// Requested verb.
    pub action: Action,
    /// Targeted resource kind.
    pub resource_kind: ResourceKind,
    /// Targeted resource id.
    pub resource_id: String,
    /// Interaction mode of the command scope.
    pub mode: InteractionMode,
    /// Privacy domain of the command scope.
    pub scope_domain: DataDomain,
    /// Privacy domain of the resource itself.
    pub resource_domain: DataDomain,
    /// Classification of the resource.
    pub classification: Classification,
    /// Whether the resource was finalized at evaluation time.
    pub finalized: bool,
    /// Winning rule id (or `default.deny` / an `invariant.*` id / an `approval.*` id).
    pub rule_id: String,
    /// Outcome.
    pub decision: DecisionKind,
    /// Capability an approval must grant (only for `ApprovalRequired`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_capability: Option<Capability>,
    /// Secret-free reason text.
    pub reason: String,
    /// When the evaluation happened (engine clock).
    pub evaluated_at: UtcMillis,
    /// Hex SHA-256 of the canonical operation parameters.
    pub parameters_sha256: String,
}

impl PolicyDecisionRecord {
    /// Build a record from an evaluation.
    pub fn new(
        actor: &Actor,
        op: &OperationDescriptor,
        ctx: &ResourceContext,
        decision: &Decision,
        reason: impl Into<String>,
        evaluated_at: UtcMillis,
    ) -> Self {
        Self {
            actor_id: actor.id.clone(),
            actor_kind: actor.kind.clone(),
            action: op.action,
            resource_kind: op.resource.kind,
            resource_id: op.resource.id.clone(),
            mode: op.scope.mode,
            scope_domain: op.scope.domain,
            resource_domain: ctx.domain,
            classification: ctx.classification,
            finalized: ctx.finalized,
            rule_id: decision_rule_id(decision).to_string(),
            decision: DecisionKind::of(decision),
            required_capability: decision_capability(decision),
            reason: reason.into(),
            evaluated_at,
            parameters_sha256: op.parameters_sha256.clone(),
        }
    }

    /// A `Deny` variant of this record with a different rule id and reason
    /// (used when an approval check fails after the rule evaluation).
    pub fn denied(&self, rule_id: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            rule_id: rule_id.into(),
            decision: DecisionKind::Deny,
            required_capability: None,
            reason: reason.into(),
            ..self.clone()
        }
    }

    /// True for an `Allow` row.
    pub fn is_allow(&self) -> bool {
        self.decision == DecisionKind::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::{ResourceRef, Scope};

    #[test]
    fn record_is_metadata_only_and_serializable() {
        let actor = Actor::model(ActorId::parse("model").unwrap());
        let op = OperationDescriptor {
            action: Action::Read,
            resource: ResourceRef::new(ResourceKind::Note, "n1"),
            scope: Scope {
                domain: DataDomain::Learning,
                mode: InteractionMode::Study,
            },
            parameters_sha256: "ab".repeat(32),
        };
        let ctx = ResourceContext::new(DataDomain::Learning, Classification::Private, false);
        let decision = Decision::Deny {
            rule_id: "x".into(),
            reason: "why".into(),
        };
        let rec = PolicyDecisionRecord::new(&actor, &op, &ctx, &decision, "why", UtcMillis::from_millis(5));
        let json = serde_json::to_value(&rec).unwrap();
        assert_eq!(json["decision"], "DENY");
        assert_eq!(json["actor_kind"]["kind"], "MODEL");
        assert!(json.get("parameters").is_none());
        assert!(json.get("required_capability").is_none());
        let back: PolicyDecisionRecord = serde_json::from_value(json).unwrap();
        assert_eq!(back, rec);
        let d = rec.denied("approval.expired", "expired");
        assert_eq!(d.decision, DecisionKind::Deny);
        assert_eq!(d.rule_id, "approval.expired");
    }
}
