//! Policy preflight boundary (ARCHITECTURE.md §10 "policy preflight
//! (`Capability::SkillInvoke`, mode)", §11). The orchestrator never decides
//! authority itself: it builds an exact [`OperationDescriptor`] for
//! `Action::Invoke` on `ResourceKind::Skill` and asks the engine.
//!
//! [`PolicyPreflight`] is implemented directly for [`g9_policy::Engine`]; the
//! trait exists so tests can inject fixed decisions ([`StaticPolicy`]).

use g9_domain::hashing::sha256_json;
use g9_domain::{
    Action, Actor, Decision, OperationDescriptor, ResourceKind, ResourceRef,
    SkillInvocationEnvelope,
};
use g9_policy::ResourceContext;

/// Deny-by-default preflight decision for one invocation.
pub trait PolicyPreflight: Send + Sync {
    /// Evaluate `op` for `actor` against the resource facts in `ctx`.
    fn preflight(&self, actor: &Actor, op: &OperationDescriptor, ctx: &ResourceContext)
        -> Decision;
}

impl PolicyPreflight for g9_policy::Engine {
    fn preflight(
        &self,
        actor: &Actor,
        op: &OperationDescriptor,
        ctx: &ResourceContext,
    ) -> Decision {
        self.evaluate(actor, op, ctx).decision
    }
}

/// Test double returning one fixed decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticPolicy {
    decision: Decision,
}

impl StaticPolicy {
    /// Always return `decision`.
    pub fn new(decision: Decision) -> Self {
        Self { decision }
    }

    /// `Allow` with rule id `static.allow`.
    pub fn allow() -> Self {
        Self::new(Decision::Allow {
            rule_id: "static.allow".into(),
        })
    }

    /// `Deny` with the given reason.
    pub fn deny(reason: impl Into<String>) -> Self {
        Self::new(Decision::Deny {
            rule_id: "static.deny".into(),
            reason: reason.into(),
        })
    }
}

impl PolicyPreflight for StaticPolicy {
    fn preflight(
        &self,
        _actor: &Actor,
        _op: &OperationDescriptor,
        _ctx: &ResourceContext,
    ) -> Decision {
        self.decision.clone()
    }
}

/// The exact operation an invocation represents: `Invoke` on the skill, in the
/// envelope's approved scope, bound to the SHA-256 of the canonical envelope
/// JSON (so an approval, if ever required, binds to this exact input).
pub fn invoke_descriptor(
    envelope: &SkillInvocationEnvelope,
) -> Result<OperationDescriptor, serde_json::Error> {
    let value = serde_json::to_value(envelope)?;
    Ok(OperationDescriptor {
        action: Action::Invoke,
        resource: ResourceRef::new(ResourceKind::Skill, envelope.skill_id.as_str()),
        scope: envelope.approved_scope,
        parameters_sha256: sha256_json(&value),
    })
}

/// Resource facts for the invocation: the skill operates on the scope's domain
/// with the payload's classification; a skill package is never "finalized".
pub fn invoke_context(envelope: &SkillInvocationEnvelope) -> ResourceContext {
    ResourceContext::new(
        envelope.approved_scope.domain,
        envelope.data_classification,
        false,
    )
}

/// Rule id carried by any decision.
pub fn decision_rule_id(decision: &Decision) -> &str {
    match decision {
        Decision::Allow { rule_id }
        | Decision::Deny { rule_id, .. }
        | Decision::ApprovalRequired { rule_id, .. } => rule_id,
    }
}
