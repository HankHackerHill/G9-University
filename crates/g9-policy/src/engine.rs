//! The evaluation engine (ARCHITECTURE.md §5 step 3–4, §11).
//!
//! `Engine::evaluate` is pure given its clock: it selects the winning
//! [`PolicyRule`](crate::rules::PolicyRule) from the [`RuleSet`], falls back to
//! `Deny` when nothing matches, and then applies the **hard invariants** below,
//! which cap whatever the table says. A rule table can only ever *narrow* the
//! invariants, never widen them:
//!
//! | Invariant id | Ceiling |
//! |---|---|
//! | `invariant.immutable_finalized` | `Update`/`Delete`/`Finalize` on a finalized record is denied for everyone |
//! | `invariant.proposal_only_actor` | skill and model actors never get a write verb (anything but `Read`/`Propose`) |
//! | `invariant.secret_not_for_model` | skill and model actors never read `Secret` content |
//! | `invariant.native_non_operator` | non-operators never `Execute`/`Connect`/`Load`; skills/model never touch host paths, processes or hosts |
//! | `invariant.native_requires_approval` | `Execute`/`Connect`/`Load` and host-path writes are never plain `Allow`, even for the operator |
//! | `invariant.operator_only_capability` | `Capability::operator_only()` is never required of (i.e. grantable to) a non-operator |
//! | `invariant.self_modification_mode` | `Policy`/`AppSetting` mutate only in `MAINTENANCE` |
//! | `invariant.self_modification_approval` | `Policy`/`AppSetting` mutations are never plain `Allow` and never for non-operators |
//!
//! Every evaluation yields a metadata-only [`PolicyDecisionRecord`] that
//! `g9-app` persists as a `policy_decisions` row.

use crate::approvals::{
    ApprovalLedger, ApprovalRequest, ConsumeIntent, ConsumedApproval, RuntimeEpoch,
};
use crate::binding::BindingKey;
use crate::error::PolicyError;
use crate::record::{DecisionKind, PolicyDecisionRecord};
use crate::rules::{
    ResourceContext, RuleSet, RuleSetError, DEFAULT_DENY_REASON, DEFAULT_DENY_RULE_ID,
    MUTATING_ACTIONS, NATIVE_ACTIONS, NATIVE_RESOURCES, SELF_MODIFICATION_RESOURCES, WRITE_ACTIONS,
};
use g9_domain::{
    Action, Actor, ActorKind, ApprovalId, Capability, Classification, Decision, DurationMs,
    InteractionMode, OperationDescriptor, ResourceKind, Revision, UtcMillis,
};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::atomic::{AtomicI64, Ordering};

/// Invariant rule ids (see the module docs).
pub const INVARIANT_IMMUTABLE_FINALIZED: &str = "invariant.immutable_finalized";
/// Skill/model actors never write.
pub const INVARIANT_PROPOSAL_ONLY_ACTOR: &str = "invariant.proposal_only_actor";
/// Skill/model actors never read secrets.
pub const INVARIANT_SECRET_NOT_FOR_MODEL: &str = "invariant.secret_not_for_model";
/// Non-operators never get native host actions.
pub const INVARIANT_NATIVE_NON_OPERATOR: &str = "invariant.native_non_operator";
/// Native host actions always need a fresh approval.
pub const INVARIANT_NATIVE_REQUIRES_APPROVAL: &str = "invariant.native_requires_approval";
/// Operator-only capabilities are never required of a non-operator.
pub const INVARIANT_OPERATOR_ONLY_CAPABILITY: &str = "invariant.operator_only_capability";
/// Self-modification is MAINTENANCE-only.
pub const INVARIANT_SELF_MODIFICATION_MODE: &str = "invariant.self_modification_mode";
/// Self-modification always needs an operator approval.
pub const INVARIANT_SELF_MODIFICATION_APPROVAL: &str = "invariant.self_modification_approval";

/// Source of "now" for the engine. Tests use [`FixedClock`] or
/// [`SteppingClock`]; `g9-app` supplies [`WallClock`].
pub trait Clock: Send + Sync {
    /// Current instant.
    fn now(&self) -> UtcMillis;
}

impl<F> Clock for F
where
    F: Fn() -> UtcMillis + Send + Sync,
{
    fn now(&self) -> UtcMillis {
        self()
    }
}

/// A clock frozen at one instant (fixtures).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedClock(pub UtcMillis);

impl Clock for FixedClock {
    fn now(&self) -> UtcMillis {
        self.0
    }
}

/// A clock that tests advance explicitly (no sleeping on wall clock).
#[derive(Debug, Default)]
pub struct SteppingClock {
    ms: AtomicI64,
}

impl SteppingClock {
    /// Start at `start`.
    pub fn new(start: UtcMillis) -> Self {
        Self {
            ms: AtomicI64::new(start.as_millis()),
        }
    }

    /// Jump to `t`.
    pub fn set(&self, t: UtcMillis) {
        self.ms.store(t.as_millis(), Ordering::SeqCst);
    }

    /// Move forward by `d` (saturating).
    pub fn advance(&self, d: DurationMs) {
        self.ms
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                Some(v.saturating_add(d.as_millis()))
            })
            .ok();
    }
}

impl Clock for SteppingClock {
    fn now(&self) -> UtcMillis {
        UtcMillis::from_millis(self.ms.load(Ordering::SeqCst))
    }
}

/// The process wall clock. Only for `g9-app`; never in tests.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WallClock;

impl Clock for WallClock {
    fn now(&self) -> UtcMillis {
        UtcMillis::now()
    }
}

/// Result of one evaluation: the decision plus the row to persist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evaluation {
    /// What the caller must do.
    pub decision: Decision,
    /// Metadata-only log row.
    pub record: PolicyDecisionRecord,
}

impl Evaluation {
    /// Decision without payload.
    pub fn kind(&self) -> DecisionKind {
        DecisionKind::of(&self.decision)
    }

    /// True for `Allow`.
    pub fn is_allow(&self) -> bool {
        self.decision.is_allow()
    }

    /// Winning rule id.
    pub fn rule_id(&self) -> &str {
        &self.record.rule_id
    }

    /// Capability an approval must grant, for `ApprovalRequired`.
    pub fn required_capability(&self) -> Option<Capability> {
        self.record.required_capability
    }

    /// Build the approval request `g9-app` should create for an
    /// `ApprovalRequired` evaluation; `None` for any other decision.
    pub fn approval_request(
        &self,
        id: ApprovalId,
        actor: &Actor,
        op: &OperationDescriptor,
        expected_revision: Option<Revision>,
    ) -> Option<Result<ApprovalRequest, crate::approvals::ApprovalError>> {
        let capability = self.required_capability()?;
        Some(ApprovalRequest::new(
            id,
            op.clone(),
            actor.clone(),
            self.record.evaluated_at,
            expected_revision,
            capability,
            self.record.reason.clone(),
        ))
    }
}

/// An authorized operation: either plainly allowed or allowed through a
/// consumed approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Authorized {
    /// The evaluation (its record is the `ALLOW` row to persist).
    pub evaluation: Evaluation,
    /// The approval that was spent, when one was needed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumed: Option<ConsumedApproval>,
}

/// The ledger and the key the engine needs to spend an approval.
#[derive(Debug)]
pub struct ApprovalGate<'a> {
    /// Where approvals live (persisted by `g9-app`).
    pub ledger: &'a mut ApprovalLedger,
    /// Per-installation binding key (backend process only).
    pub key: &'a BindingKey,
}

/// The authorization engine.
pub struct Engine {
    rules: RuleSet,
    clock: Box<dyn Clock>,
    /// The running process instance approvals must have been issued in (SEC-02).
    runtime_epoch: Option<RuntimeEpoch>,
}

impl fmt::Debug for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Engine")
            .field("rules.version", &self.rules.version)
            .field("rules.len", &self.rules.rules.len())
            .field("runtime_epoch", &self.runtime_epoch.is_some())
            .finish_non_exhaustive()
    }
}

impl Engine {
    /// Build an engine over a validated rule table.
    pub fn new(rules: RuleSet, clock: Box<dyn Clock>) -> Result<Self, RuleSetError> {
        rules.validate()?;
        Ok(Self {
            rules,
            clock,
            runtime_epoch: None,
        })
    }

    /// Engine over the built-in `builtin-v1` table (validated by this crate's tests).
    pub fn builtin_v1(clock: Box<dyn Clock>) -> Self {
        Self {
            rules: RuleSet::builtin_v1(),
            clock,
            runtime_epoch: None,
        }
    }

    /// Bind approval consumption to the running process instance (SEC-02,
    /// `docs/audit/defect-register.md` D.2): [`Self::authorize_with_approval`]
    /// treats an approval that was not issued in `epoch`
    /// ([`ApprovalLedger::request_in_epoch`]) as EXPIRED. Without an epoch only
    /// the wall-clock rules apply.
    #[must_use]
    pub fn with_runtime_epoch(mut self, epoch: RuntimeEpoch) -> Self {
        self.runtime_epoch = Some(epoch);
        self
    }

    /// The runtime epoch approvals are bound to, if configured.
    pub fn runtime_epoch(&self) -> Option<&RuntimeEpoch> {
        self.runtime_epoch.as_ref()
    }

    /// The active rule table.
    pub fn rules(&self) -> &RuleSet {
        &self.rules
    }

    /// Current engine time.
    pub fn now(&self) -> UtcMillis {
        self.clock.now()
    }

    /// Evaluate one operation. Never fails; a missing rule is `Deny`.
    pub fn evaluate(
        &self,
        actor: &Actor,
        op: &OperationDescriptor,
        ctx: &ResourceContext,
    ) -> Evaluation {
        let now = self.clock.now();
        let table = match self.rules.select(&actor.kind, op, ctx) {
            Some(rule) => rule.decision(),
            None => Decision::Deny {
                rule_id: DEFAULT_DENY_RULE_ID.to_string(),
                reason: DEFAULT_DENY_REASON.to_string(),
            },
        };
        let decision = if matches!(table, Decision::Deny { .. }) {
            table
        } else {
            match invariant_violation(actor, op, ctx, &table) {
                Some((rule_id, reason)) => Decision::Deny {
                    rule_id: rule_id.to_string(),
                    reason: reason.to_string(),
                },
                None => table,
            }
        };
        let reason = match &decision {
            Decision::Allow { rule_id } => self
                .rules
                .get(rule_id)
                .map(|r| r.rationale.to_string())
                .unwrap_or_default(),
            Decision::Deny { reason, .. } | Decision::ApprovalRequired { reason, .. } => {
                reason.clone()
            }
        };
        let record = PolicyDecisionRecord::new(actor, op, ctx, &decision, reason, now);
        Evaluation { decision, record }
    }

    /// Evaluate and turn anything but `Allow` into a [`PolicyError`].
    pub fn authorize(
        &self,
        actor: &Actor,
        op: &OperationDescriptor,
        ctx: &ResourceContext,
    ) -> Result<Evaluation, PolicyError> {
        let evaluation = self.evaluate(actor, op, ctx);
        match &evaluation.decision {
            Decision::Allow { .. } => Ok(evaluation),
            Decision::Deny { .. } => Err(PolicyError::Denied {
                record: Box::new(evaluation.record),
            }),
            Decision::ApprovalRequired {
                required_capability,
                ..
            } => Err(PolicyError::ApprovalRequired {
                required_capability: *required_capability,
                record: Box::new(evaluation.record),
            }),
        }
    }

    /// The full §5 step 3–4 flow: evaluate; `Deny` is final even when an
    /// approval is presented; `ApprovalRequired` needs `approval_id`, which is
    /// verified against this exact operation and `expected_revision` and
    /// consumed (single use). A presented approval is ignored for a plain
    /// `Allow` (nothing to spend). Any consumption failure yields a `DENY`
    /// record whose rule id is the `approval.*` code.
    pub fn authorize_with_approval(
        &self,
        actor: &Actor,
        op: &OperationDescriptor,
        ctx: &ResourceContext,
        approval_id: Option<&ApprovalId>,
        expected_revision: Option<Revision>,
        gate: ApprovalGate<'_>,
    ) -> Result<Authorized, PolicyError> {
        let evaluation = self.evaluate(actor, op, ctx);
        let required_capability = match &evaluation.decision {
            Decision::Allow { .. } => {
                return Ok(Authorized {
                    evaluation,
                    consumed: None,
                })
            }
            Decision::Deny { .. } => {
                let record = match approval_id {
                    Some(id) => evaluation.record.with_approval_id(id.clone()),
                    None => evaluation.record,
                };
                return Err(PolicyError::Denied {
                    record: Box::new(record),
                });
            }
            Decision::ApprovalRequired {
                required_capability,
                ..
            } => *required_capability,
        };
        let Some(id) = approval_id else {
            return Err(PolicyError::ApprovalRequired {
                required_capability,
                record: Box::new(evaluation.record),
            });
        };
        let intent = ConsumeIntent {
            operation: op,
            expected_revision,
            required_capability,
            consumer: actor,
            now: evaluation.record.evaluated_at,
            runtime_epoch: self.runtime_epoch.as_ref(),
        };
        match gate.ledger.verify_and_consume(id, &intent, gate.key) {
            Ok(consumed) => {
                let record = evaluation.record.allowed_by_approval(id.clone());
                let decision = Decision::Allow {
                    rule_id: record.rule_id.clone(),
                };
                Ok(Authorized {
                    evaluation: Evaluation { decision, record },
                    consumed: Some(consumed),
                })
            }
            Err(source) => {
                let record = evaluation
                    .record
                    .denied(source.code(), source.to_string())
                    .with_approval_id(id.clone());
                Err(PolicyError::Approval {
                    source,
                    record: Box::new(record),
                })
            }
        }
    }
}

fn is_proposal_only(kind: &ActorKind) -> bool {
    matches!(kind, ActorKind::Skill { .. } | ActorKind::Model)
}

/// The hard invariants. Returns the invariant id and reason when the table
/// decision (`Allow` or `ApprovalRequired`) would exceed a ceiling.
fn invariant_violation(
    actor: &Actor,
    op: &OperationDescriptor,
    ctx: &ResourceContext,
    table: &Decision,
) -> Option<(&'static str, &'static str)> {
    let action = op.action;
    let kind = op.resource.kind;
    let proposal_only = is_proposal_only(&actor.kind);
    let operator = actor.is_operator();
    let allow = matches!(table, Decision::Allow { .. });

    if ctx.finalized && matches!(action, Action::Update | Action::Delete | Action::Finalize) {
        return Some((
            INVARIANT_IMMUTABLE_FINALIZED,
            "finalized records are immutable; corrections are superseding records",
        ));
    }
    if proposal_only && WRITE_ACTIONS.contains(&action) {
        return Some((
            INVARIANT_PROPOSAL_ONLY_ACTOR,
            "skills and the model are proposal-only actors and never write authoritative state",
        ));
    }
    if proposal_only && action == Action::Read && ctx.classification == Classification::Secret {
        return Some((
            INVARIANT_SECRET_NOT_FOR_MODEL,
            "secret-classified content never reaches a skill or the model",
        ));
    }
    if (!operator && NATIVE_ACTIONS.contains(&action))
        || (proposal_only && NATIVE_RESOURCES.contains(&kind))
    {
        return Some((
            INVARIANT_NATIVE_NON_OPERATOR,
            "only the operator may execute, connect, load or touch host resources, and only with approval",
        ));
    }
    let host_write = kind == ResourceKind::FilesystemPath && MUTATING_ACTIONS.contains(&action);
    if allow && (NATIVE_ACTIONS.contains(&action) || host_write) {
        return Some((
            INVARIANT_NATIVE_REQUIRES_APPROVAL,
            "execute, connect, load and host writes always require a fresh per-operation approval",
        ));
    }
    if let Decision::ApprovalRequired {
        required_capability,
        ..
    } = table
    {
        if required_capability.operator_only() && !operator {
            return Some((
                INVARIANT_OPERATOR_ONLY_CAPABILITY,
                "operator-only capabilities can never be granted to a non-operator actor",
            ));
        }
    }
    if SELF_MODIFICATION_RESOURCES.contains(&kind) && MUTATING_ACTIONS.contains(&action) {
        if op.scope.mode != InteractionMode::Maintenance {
            return Some((
                INVARIANT_SELF_MODIFICATION_MODE,
                "policy and settings change only in MAINTENANCE mode",
            ));
        }
        if allow || !operator {
            return Some((
                INVARIANT_SELF_MODIFICATION_APPROVAL,
                "policy and settings change only through an operator-approved change set",
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{ActorKindMatcher, Effect, PolicyRule};
    use g9_domain::{ActorId, DataDomain, ResourceRef, Scope, SkillId};
    use std::borrow::Cow;

    fn operator() -> Actor {
        Actor::operator(ActorId::parse("op").unwrap())
    }

    fn skill() -> Actor {
        Actor::skill(
            ActorId::parse("skill-1").unwrap(),
            SkillId::parse("g9-session-director").unwrap(),
        )
    }

    fn model() -> Actor {
        Actor::model(ActorId::parse("model").unwrap())
    }

    fn system() -> Actor {
        Actor::system(ActorId::parse("system").unwrap())
    }

    fn op(
        action: Action,
        kind: ResourceKind,
        mode: InteractionMode,
        domain: DataDomain,
    ) -> OperationDescriptor {
        OperationDescriptor {
            action,
            resource: ResourceRef::new(kind, "r1"),
            scope: Scope { domain, mode },
            parameters_sha256: "00".repeat(32),
        }
    }

    fn ctx(domain: DataDomain, classification: Classification, finalized: bool) -> ResourceContext {
        ResourceContext::new(domain, classification, finalized)
    }

    fn engine() -> Engine {
        Engine::builtin_v1(Box::new(FixedClock(UtcMillis::from_millis(
            1_700_000_000_000,
        ))))
    }

    /// A deliberately broken table that allows everything to everyone.
    fn permissive_engine() -> Engine {
        let rules = RuleSet {
            version: "permissive-test".into(),
            rules: vec![PolicyRule {
                id: "evil.allow_all".into(),
                effect: Effect::Allow,
                actor_kinds: ActorKindMatcher::ALL.to_vec(),
                actions: crate::rules::ALL_ACTIONS.to_vec(),
                resource_kinds: vec![],
                modes: vec![],
                domains: vec![],
                conditions: vec![],
                priority: 9_999,
                rationale: Cow::Borrowed("evil"),
            }],
        };
        // Bypass validation on purpose: the invariants must hold even then.
        Engine {
            rules,
            clock: Box::new(FixedClock(UtcMillis::from_millis(1))),
            runtime_epoch: None,
        }
    }

    #[test]
    fn builtin_v1_constructs_through_new_and_validates() {
        let e = Engine::new(
            RuleSet::builtin_v1(),
            Box::new(FixedClock(UtcMillis::UNIX_EPOCH)),
        )
        .unwrap();
        assert_eq!(e.rules().version, crate::rules::BUILTIN_V1_VERSION);
        assert_eq!(e.now(), UtcMillis::UNIX_EPOCH);
        assert!(format!("{e:?}").contains("builtin-v1"));
        let empty = RuleSet {
            version: "x".into(),
            rules: vec![],
        };
        assert!(matches!(
            Engine::new(empty, Box::new(FixedClock(UtcMillis::UNIX_EPOCH))),
            Err(RuleSetError::Empty)
        ));
    }

    #[test]
    fn clocks_are_deterministic() {
        let stepping = SteppingClock::new(UtcMillis::from_millis(10));
        stepping.advance(DurationMs::from_seconds_u32(5));
        assert_eq!(stepping.now(), UtcMillis::from_millis(5_010));
        stepping.set(UtcMillis::from_millis(1));
        assert_eq!(stepping.now(), UtcMillis::from_millis(1));
        let closure = || UtcMillis::from_millis(42);
        let e = Engine::builtin_v1(Box::new(closure));
        assert_eq!(e.now(), UtcMillis::from_millis(42));
        let rec = e
            .evaluate(
                &operator(),
                &op(
                    Action::Read,
                    ResourceKind::Note,
                    InteractionMode::Study,
                    DataDomain::Learning,
                ),
                &ctx(DataDomain::Learning, Classification::Internal, false),
            )
            .record;
        assert_eq!(rec.evaluated_at, UtcMillis::from_millis(42));
    }

    #[test]
    fn permissive_table_cannot_override_invariants() {
        let e = permissive_engine();
        let cases: Vec<(Actor, OperationDescriptor, ResourceContext, &str)> = vec![
            (
                operator(),
                op(
                    Action::Update,
                    ResourceKind::GradeDecision,
                    InteractionMode::Review,
                    DataDomain::Learning,
                ),
                ctx(DataDomain::Learning, Classification::Internal, true),
                INVARIANT_IMMUTABLE_FINALIZED,
            ),
            (
                system(),
                op(
                    Action::Delete,
                    ResourceKind::Note,
                    InteractionMode::Maintenance,
                    DataDomain::System,
                ),
                ctx(DataDomain::System, Classification::Internal, true),
                INVARIANT_IMMUTABLE_FINALIZED,
            ),
            (
                skill(),
                op(
                    Action::Create,
                    ResourceKind::Note,
                    InteractionMode::Study,
                    DataDomain::Learning,
                ),
                ctx(DataDomain::Learning, Classification::Internal, false),
                INVARIANT_PROPOSAL_ONLY_ACTOR,
            ),
            (
                model(),
                op(
                    Action::Invoke,
                    ResourceKind::Skill,
                    InteractionMode::Study,
                    DataDomain::Learning,
                ),
                ctx(DataDomain::Learning, Classification::Internal, false),
                INVARIANT_PROPOSAL_ONLY_ACTOR,
            ),
            (
                model(),
                op(
                    Action::Read,
                    ResourceKind::AssessmentDefinition,
                    InteractionMode::Exam,
                    DataDomain::Learning,
                ),
                ctx(DataDomain::Learning, Classification::Secret, false),
                INVARIANT_SECRET_NOT_FOR_MODEL,
            ),
            (
                skill(),
                op(
                    Action::Read,
                    ResourceKind::FilesystemPath,
                    InteractionMode::Study,
                    DataDomain::System,
                ),
                ctx(DataDomain::System, Classification::Internal, false),
                INVARIANT_NATIVE_NON_OPERATOR,
            ),
            (
                system(),
                op(
                    Action::Execute,
                    ResourceKind::Process,
                    InteractionMode::Maintenance,
                    DataDomain::System,
                ),
                ctx(DataDomain::System, Classification::Internal, false),
                INVARIANT_NATIVE_NON_OPERATOR,
            ),
            (
                operator(),
                op(
                    Action::Execute,
                    ResourceKind::Process,
                    InteractionMode::Maintenance,
                    DataDomain::System,
                ),
                ctx(DataDomain::System, Classification::Internal, false),
                INVARIANT_NATIVE_REQUIRES_APPROVAL,
            ),
            (
                operator(),
                op(
                    Action::Connect,
                    ResourceKind::NetworkHost,
                    InteractionMode::Research,
                    DataDomain::System,
                ),
                ctx(DataDomain::System, Classification::Internal, false),
                INVARIANT_NATIVE_REQUIRES_APPROVAL,
            ),
            (
                operator(),
                op(
                    Action::Load,
                    ResourceKind::Model,
                    InteractionMode::Maintenance,
                    DataDomain::System,
                ),
                ctx(DataDomain::System, Classification::Internal, false),
                INVARIANT_NATIVE_REQUIRES_APPROVAL,
            ),
            (
                operator(),
                op(
                    Action::Create,
                    ResourceKind::FilesystemPath,
                    InteractionMode::Study,
                    DataDomain::System,
                ),
                ctx(DataDomain::System, Classification::Internal, false),
                INVARIANT_NATIVE_REQUIRES_APPROVAL,
            ),
            (
                operator(),
                op(
                    Action::Update,
                    ResourceKind::Policy,
                    InteractionMode::Study,
                    DataDomain::System,
                ),
                ctx(DataDomain::System, Classification::Internal, false),
                INVARIANT_SELF_MODIFICATION_MODE,
            ),
            (
                operator(),
                op(
                    Action::Update,
                    ResourceKind::AppSetting,
                    InteractionMode::Maintenance,
                    DataDomain::System,
                ),
                ctx(DataDomain::System, Classification::Internal, false),
                INVARIANT_SELF_MODIFICATION_APPROVAL,
            ),
            (
                system(),
                op(
                    Action::Update,
                    ResourceKind::Policy,
                    InteractionMode::Maintenance,
                    DataDomain::System,
                ),
                ctx(DataDomain::System, Classification::Internal, false),
                INVARIANT_SELF_MODIFICATION_APPROVAL,
            ),
        ];
        for (actor, operation, context, expected) in cases {
            let eval = e.evaluate(&actor, &operation, &context);
            assert_eq!(eval.kind(), DecisionKind::Deny, "{actor:?} {operation:?}");
            assert_eq!(eval.rule_id(), expected, "{actor:?} {operation:?}");
        }
        // The permissive table still allows what the invariants do not forbid.
        let eval = e.evaluate(
            &operator(),
            &op(
                Action::Read,
                ResourceKind::Note,
                InteractionMode::Study,
                DataDomain::Learning,
            ),
            &ctx(DataDomain::Learning, Classification::Internal, false),
        );
        assert!(eval.is_allow());
        assert_eq!(eval.rule_id(), "evil.allow_all");
    }

    #[test]
    fn operator_only_capability_required_of_skill_is_denied() {
        let rules = RuleSet {
            version: "t".into(),
            rules: vec![PolicyRule {
                id: "evil.skill.export".into(),
                effect: Effect::ApprovalRequired(Capability::DataDelete),
                actor_kinds: vec![ActorKindMatcher::Skill],
                actions: vec![Action::Read],
                resource_kinds: vec![],
                modes: vec![],
                domains: vec![],
                conditions: vec![],
                priority: 9_999,
                rationale: Cow::Borrowed("evil"),
            }],
        };
        assert!(matches!(
            Engine::new(rules.clone(), Box::new(FixedClock(UtcMillis::UNIX_EPOCH))),
            Err(RuleSetError::OperatorOnlyCapabilityForNonOperator { .. })
        ));
        let e = Engine {
            rules,
            clock: Box::new(FixedClock(UtcMillis::UNIX_EPOCH)),
            runtime_epoch: None,
        };
        let eval = e.evaluate(
            &skill(),
            &op(
                Action::Read,
                ResourceKind::Note,
                InteractionMode::Study,
                DataDomain::Learning,
            ),
            &ctx(DataDomain::Learning, Classification::Internal, false),
        );
        assert_eq!(eval.kind(), DecisionKind::Deny);
        assert_eq!(eval.rule_id(), INVARIANT_OPERATOR_ONLY_CAPABILITY);
    }

    #[test]
    fn table_deny_is_reported_with_its_own_rule_id() {
        let e = engine();
        let eval = e.evaluate(
            &operator(),
            &op(
                Action::Update,
                ResourceKind::GradeDecision,
                InteractionMode::Review,
                DataDomain::Learning,
            ),
            &ctx(DataDomain::Learning, Classification::Internal, true),
        );
        assert_eq!(eval.rule_id(), "immutable.finalized.deny");
        assert_eq!(
            eval.record.reason,
            "finalized records are immutable; corrections are superseding records"
        );
        let eval = e.evaluate(
            &system(),
            &op(
                Action::Propose,
                ResourceKind::Intervention,
                InteractionMode::Study,
                DataDomain::Learning,
            ),
            &ctx(DataDomain::Learning, Classification::Internal, false),
        );
        assert_eq!(eval.rule_id(), DEFAULT_DENY_RULE_ID);
        assert_eq!(eval.record.reason, DEFAULT_DENY_REASON);
    }

    #[test]
    fn authorize_maps_decisions_to_errors_with_records() {
        let e = engine();
        let read = op(
            Action::Read,
            ResourceKind::Note,
            InteractionMode::Study,
            DataDomain::Learning,
        );
        let c = ctx(DataDomain::Learning, Classification::Internal, false);
        let ok = e.authorize(&operator(), &read, &c).unwrap();
        assert!(ok.is_allow());
        assert_eq!(
            ok.record.reason,
            "the operator may read every record in every mode"
        );

        let write = op(
            Action::Create,
            ResourceKind::Note,
            InteractionMode::Study,
            DataDomain::Learning,
        );
        let err = e.authorize(&model(), &write, &c).unwrap_err();
        assert!(matches!(err, PolicyError::Denied { .. }));
        assert_eq!(err.record().unwrap().rule_id, "sm.write.deny");
        let g: g9_domain::G9Error = err.into();
        assert_eq!(g.category, g9_domain::ErrorCategory::Unauthorized);

        let export = op(
            Action::Export,
            ResourceKind::Note,
            InteractionMode::Study,
            DataDomain::Learning,
        );
        let err = e.authorize(&operator(), &export, &c).unwrap_err();
        match &err {
            PolicyError::ApprovalRequired {
                required_capability,
                record,
            } => {
                assert_eq!(*required_capability, Capability::DataExport);
                assert_eq!(record.decision, DecisionKind::ApprovalRequired);
                assert_eq!(record.required_capability, Some(Capability::DataExport));
            }
            other => panic!("unexpected {other:?}"),
        }
        let g: g9_domain::G9Error = err.into();
        assert_eq!(g.category, g9_domain::ErrorCategory::ApprovalRequired);
    }

    #[test]
    fn evaluation_builds_the_approval_request() {
        let e = engine();
        let export = op(
            Action::Export,
            ResourceKind::Note,
            InteractionMode::Study,
            DataDomain::Learning,
        );
        let c = ctx(DataDomain::Learning, Classification::Internal, false);
        let eval = e.evaluate(&operator(), &export, &c);
        let req = eval
            .approval_request(
                ApprovalId::parse("apr-1").unwrap(),
                &operator(),
                &export,
                Some(Revision::new(2)),
            )
            .unwrap()
            .unwrap();
        assert_eq!(req.required_capability, Capability::DataExport);
        assert_eq!(req.requested_at, e.now());
        assert_eq!(
            req.expires_at,
            e.now()
                .checked_add(crate::approvals::DEFAULT_APPROVAL_TTL)
                .unwrap()
        );
        assert_eq!(
            req.reason,
            "export requires DataExport for the exact selection"
        );
        let allow = e.evaluate(
            &operator(),
            &op(
                Action::Read,
                ResourceKind::Note,
                InteractionMode::Study,
                DataDomain::Learning,
            ),
            &c,
        );
        assert!(allow
            .approval_request(
                ApprovalId::parse("apr-2").unwrap(),
                &operator(),
                &export,
                None
            )
            .is_none());
    }

    #[test]
    fn authorize_with_approval_full_flow() {
        let e = engine();
        let key = BindingKey::from_bytes([3u8; 32]);
        let mut ledger = ApprovalLedger::new();
        let export = op(
            Action::Export,
            ResourceKind::Note,
            InteractionMode::Study,
            DataDomain::Learning,
        );
        let c = ctx(DataDomain::Learning, Classification::Internal, false);
        let actor = operator();

        // 1. no approval → APPROVAL_REQUIRED
        let err = e
            .authorize_with_approval(
                &actor,
                &export,
                &c,
                None,
                Some(Revision::new(1)),
                ApprovalGate {
                    ledger: &mut ledger,
                    key: &key,
                },
            )
            .unwrap_err();
        let eval = match &err {
            PolicyError::ApprovalRequired { record, .. } => Evaluation {
                decision: Decision::ApprovalRequired {
                    rule_id: record.rule_id.clone(),
                    reason: record.reason.clone(),
                    required_capability: Capability::DataExport,
                },
                record: (**record).clone(),
            },
            other => panic!("unexpected {other:?}"),
        };

        // 2. request + approve
        let id = ApprovalId::parse("apr-1").unwrap();
        let req = eval
            .approval_request(id.clone(), &actor, &export, Some(Revision::new(1)))
            .unwrap()
            .unwrap();
        ledger.request(req).unwrap();
        ledger.approve(&id, &actor, e.now(), &key).unwrap();

        // 3. retry with the approval → ALLOW through the approval, consumed once
        let authorized = e
            .authorize_with_approval(
                &actor,
                &export,
                &c,
                Some(&id),
                Some(Revision::new(1)),
                ApprovalGate {
                    ledger: &mut ledger,
                    key: &key,
                },
            )
            .unwrap();
        assert!(authorized.evaluation.is_allow());
        assert_eq!(authorized.evaluation.rule_id(), "operator.export.approval");
        assert_eq!(authorized.evaluation.record.approval_id, Some(id.clone()));
        assert_eq!(
            authorized.evaluation.record.required_capability,
            Some(Capability::DataExport)
        );
        assert_eq!(
            authorized.consumed.as_ref().unwrap().granted_capability,
            Capability::DataExport
        );
        assert_eq!(
            ledger.get(&id).unwrap().state,
            g9_domain::ApprovalState::Consumed
        );

        // 4. replay → DENY row with approval.already_consumed
        let err = e
            .authorize_with_approval(
                &actor,
                &export,
                &c,
                Some(&id),
                Some(Revision::new(1)),
                ApprovalGate {
                    ledger: &mut ledger,
                    key: &key,
                },
            )
            .unwrap_err();
        match &err {
            PolicyError::Approval { source, record } => {
                assert_eq!(*source, crate::approvals::ApprovalError::AlreadyConsumed);
                assert_eq!(record.decision, DecisionKind::Deny);
                assert_eq!(record.rule_id, "approval.already_consumed");
                assert_eq!(record.approval_id, Some(id.clone()));
            }
            other => panic!("unexpected {other:?}"),
        }
        let g: g9_domain::G9Error = err.into();
        assert_eq!(g.category, g9_domain::ErrorCategory::Unauthorized);
    }

    #[test]
    fn approval_presented_for_denied_or_allowed_operation_is_not_spent() {
        let e = engine();
        let key = BindingKey::from_bytes([3u8; 32]);
        let mut ledger = ApprovalLedger::new();
        let actor = operator();
        let export = op(
            Action::Export,
            ResourceKind::Note,
            InteractionMode::Study,
            DataDomain::Learning,
        );
        let c = ctx(DataDomain::Learning, Classification::Internal, false);
        let id = ApprovalId::parse("apr-1").unwrap();
        ledger
            .request(
                ApprovalRequest::new(
                    id.clone(),
                    export,
                    actor.clone(),
                    e.now(),
                    None,
                    Capability::DataExport,
                    "x",
                )
                .unwrap(),
            )
            .unwrap();
        ledger.approve(&id, &actor, e.now(), &key).unwrap();

        // Denied operation: approval irrelevant and untouched.
        let finalized = op(
            Action::Update,
            ResourceKind::GradeDecision,
            InteractionMode::Review,
            DataDomain::Learning,
        );
        let err = e
            .authorize_with_approval(
                &actor,
                &finalized,
                &ctx(DataDomain::Learning, Classification::Internal, true),
                Some(&id),
                None,
                ApprovalGate {
                    ledger: &mut ledger,
                    key: &key,
                },
            )
            .unwrap_err();
        assert!(matches!(err, PolicyError::Denied { .. }));
        assert_eq!(err.record().unwrap().approval_id, Some(id.clone()));
        assert_eq!(
            ledger.get(&id).unwrap().state,
            g9_domain::ApprovalState::Approved
        );

        // Allowed operation: approval ignored and untouched.
        let read = op(
            Action::Read,
            ResourceKind::Note,
            InteractionMode::Study,
            DataDomain::Learning,
        );
        let ok = e
            .authorize_with_approval(
                &actor,
                &read,
                &c,
                Some(&id),
                None,
                ApprovalGate {
                    ledger: &mut ledger,
                    key: &key,
                },
            )
            .unwrap();
        assert!(ok.consumed.is_none());
        assert_eq!(
            ledger.get(&id).unwrap().state,
            g9_domain::ApprovalState::Approved
        );
    }
}
