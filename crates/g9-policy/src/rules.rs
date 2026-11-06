//! Policy rules as data (ARCHITECTURE.md §11; handoff §12).
//!
//! A [`RuleSet`] is an ordered table of [`PolicyRule`]s. Each rule matches on
//! the actor kind, the action, the resource kind, the interaction mode, the
//! resource's privacy domain and optional [`RuleCondition`]s evaluated against
//! the [`ResourceContext`] supplied by the application layer. The engine
//! (`crate::engine`) selects the winning rule deterministically:
//!
//! 1. only matching rules are considered;
//! 2. the highest `priority` wins;
//! 3. on a tie the most restrictive effect wins (`Deny` > `ApprovalRequired` > `Allow`);
//! 4. on a full tie the lexicographically smallest rule id wins.
//!
//! If no rule matches, the decision is `Deny` (`DEFAULT_DENY_RULE_ID`).
//!
//! Matching semantics: an **empty** `resource_kinds`, `modes` or `domains` list
//! means "any". `actor_kinds` and `actions` must be explicit (validated by
//! [`RuleSet::validate`]) so that a rule can never silently widen to every actor
//! or every verb.
//!
//! The built-in table ([`RuleSet::builtin_v1`]) is the versioned default. Edits
//! require `Capability::PolicyModify` through an approved MAINTENANCE change set
//! (ARCHITECTURE.md §11); the engine additionally enforces hard invariants that
//! no table can override (see `crate::engine`).

use g9_domain::{
    Action, ActorKind, Capability, Classification, DataDomain, Decision, InteractionMode,
    OperationDescriptor, ResourceKind, Scope,
};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeSet;

/// Rule id recorded when no rule matched (deny by default).
pub const DEFAULT_DENY_RULE_ID: &str = "default.deny";

/// Reason recorded when no rule matched.
pub const DEFAULT_DENY_REASON: &str = "no policy rule allows this operation (deny by default)";

/// Version label of the built-in rule table.
pub const BUILTIN_V1_VERSION: &str = "builtin-v1";

/// Which actor kinds a rule applies to. A `Skill` matcher matches every skill
/// regardless of its `SkillId`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActorKindMatcher {
    /// The single human operator.
    Operator,
    /// Deterministic backend routine.
    System,
    /// Any skill package actor.
    Skill,
    /// The local language model.
    Model,
}

impl ActorKindMatcher {
    /// Every actor kind.
    pub const ALL: [ActorKindMatcher; 4] = [
        ActorKindMatcher::Operator,
        ActorKindMatcher::System,
        ActorKindMatcher::Skill,
        ActorKindMatcher::Model,
    ];
    /// Proposal-only actors (never authoritative).
    pub const SKILL_OR_MODEL: [ActorKindMatcher; 2] = [ActorKindMatcher::Skill, ActorKindMatcher::Model];
    /// Everyone except the operator.
    pub const NON_OPERATOR: [ActorKindMatcher; 3] =
        [ActorKindMatcher::System, ActorKindMatcher::Skill, ActorKindMatcher::Model];

    /// Whether this matcher applies to the given actor kind.
    pub fn matches(self, kind: &ActorKind) -> bool {
        matches!(
            (self, kind),
            (ActorKindMatcher::Operator, ActorKind::Operator)
                | (ActorKindMatcher::System, ActorKind::System)
                | (ActorKindMatcher::Skill, ActorKind::Skill { .. })
                | (ActorKindMatcher::Model, ActorKind::Model)
        )
    }

    /// The matcher corresponding to a concrete actor kind.
    pub fn from_kind(kind: &ActorKind) -> Self {
        match kind {
            ActorKind::Operator => ActorKindMatcher::Operator,
            ActorKind::System => ActorKindMatcher::System,
            ActorKind::Skill { .. } => ActorKindMatcher::Skill,
            ActorKind::Model => ActorKindMatcher::Model,
        }
    }

    /// Whether the matcher is the operator.
    pub const fn is_operator(self) -> bool {
        matches!(self, ActorKindMatcher::Operator)
    }
}

/// Effect of a matching rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "capability", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Effect {
    /// The operation may proceed without a separate approval.
    Allow,
    /// The operation is refused.
    Deny,
    /// The operation may proceed only with a consumed approval that grants the
    /// named capability for this exact operation.
    ApprovalRequired(Capability),
}

impl Effect {
    /// Ordering used for tie-breaks: higher is more restrictive.
    pub const fn restrictiveness(self) -> u8 {
        match self {
            Effect::Allow => 0,
            Effect::ApprovalRequired(_) => 1,
            Effect::Deny => 2,
        }
    }

    /// The capability an approval must grant, if any.
    pub const fn required_capability(self) -> Option<Capability> {
        match self {
            Effect::ApprovalRequired(c) => Some(c),
            Effect::Allow | Effect::Deny => None,
        }
    }
}

/// Facts about the targeted resource that the application layer supplies from
/// the stored row (or, for `Create`, from the record about to be written).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceContext {
    /// Privacy domain of the resource itself (not of the command scope).
    pub domain: DataDomain,
    /// Classification of the resource content.
    pub classification: Classification,
    /// Whether the resource is in a terminal/finalized state (approved grade,
    /// consumed approval, audit row, applied proposal). Finalized records are
    /// immutable; corrections are superseding records.
    pub finalized: bool,
    /// Whether the resource's *owning* aggregate lives in the command scope
    /// domain. Lets the application flag a resource whose own row domain matches
    /// the scope but whose owner (e.g. an attachment on a Business note read
    /// from a Learning scope) does not. Defaults to `true`.
    pub owner_domain_matches_scope: bool,
}

impl ResourceContext {
    /// Context for a resource with `owner_domain_matches_scope = true`.
    pub const fn new(domain: DataDomain, classification: Classification, finalized: bool) -> Self {
        Self {
            domain,
            classification,
            finalized,
            owner_domain_matches_scope: true,
        }
    }

    /// Override the owner-domain flag.
    pub const fn with_owner_domain_matches_scope(mut self, value: bool) -> Self {
        self.owner_domain_matches_scope = value;
        self
    }

    /// True when the resource is inside the command scope's domain.
    pub fn is_in_scope(&self, scope: &Scope) -> bool {
        self.domain == scope.domain && self.owner_domain_matches_scope
    }
}

/// Additional predicate on the resource context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "condition", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RuleCondition {
    /// Classification is at least `level` (`Public < Internal < Private < Secret`).
    ClassificationAtLeast {
        /// Lower bound (inclusive).
        level: Classification,
    },
    /// Classification is at most `level`.
    ClassificationAtMost {
        /// Upper bound (inclusive).
        level: Classification,
    },
    /// The resource's finalized flag equals `value`.
    Finalized {
        /// Expected flag.
        value: bool,
    },
    /// Whether the command scope domain matches the resource (`true`) or the
    /// read/write is cross-domain (`false`).
    ScopeMatchesResource {
        /// Expected in-scope flag.
        value: bool,
    },
}

impl RuleCondition {
    /// Evaluate the predicate.
    pub fn holds(self, scope: &Scope, ctx: &ResourceContext) -> bool {
        match self {
            RuleCondition::ClassificationAtLeast { level } => ctx.classification >= level,
            RuleCondition::ClassificationAtMost { level } => ctx.classification <= level,
            RuleCondition::Finalized { value } => ctx.finalized == value,
            RuleCondition::ScopeMatchesResource { value } => ctx.is_in_scope(scope) == value,
        }
    }
}

/// One authorization rule. Rules are data: they serialize to JSON for the
/// policy documentation and can be loaded from a versioned table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Unique, stable identifier (recorded in every decision record).
    pub id: String,
    /// What happens when the rule wins.
    pub effect: Effect,
    /// Actor kinds the rule applies to (must be non-empty).
    pub actor_kinds: Vec<ActorKindMatcher>,
    /// Actions the rule applies to (must be non-empty).
    pub actions: Vec<Action>,
    /// Resource kinds; empty means any.
    pub resource_kinds: Vec<ResourceKind>,
    /// Interaction modes; empty means any.
    pub modes: Vec<InteractionMode>,
    /// Resource domains; empty means any.
    pub domains: Vec<DataDomain>,
    /// Extra predicates on the resource context; all must hold.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<RuleCondition>,
    /// Higher wins. Ties are broken by restrictiveness, then by id.
    pub priority: u16,
    /// Short human-readable justification (becomes the decision reason).
    pub rationale: Cow<'static, str>,
}

impl PolicyRule {
    /// Whether the rule applies to the actor kind, operation and context.
    pub fn matches(&self, actor_kind: &ActorKind, op: &OperationDescriptor, ctx: &ResourceContext) -> bool {
        self.actor_kinds.iter().any(|m| m.matches(actor_kind))
            && self.actions.contains(&op.action)
            && (self.resource_kinds.is_empty() || self.resource_kinds.contains(&op.resource.kind))
            && (self.modes.is_empty() || self.modes.contains(&op.scope.mode))
            && (self.domains.is_empty() || self.domains.contains(&ctx.domain))
            && self.conditions.iter().all(|c| c.holds(&op.scope, ctx))
    }

    /// The decision this rule produces when it wins.
    pub fn decision(&self) -> Decision {
        match self.effect {
            Effect::Allow => Decision::Allow {
                rule_id: self.id.clone(),
            },
            Effect::Deny => Decision::Deny {
                rule_id: self.id.clone(),
                reason: self.rationale.to_string(),
            },
            Effect::ApprovalRequired(capability) => Decision::ApprovalRequired {
                rule_id: self.id.clone(),
                reason: self.rationale.to_string(),
                required_capability: capability,
            },
        }
    }

    /// Whether the rule applies to at least one non-operator actor kind.
    pub fn applies_to_non_operator(&self) -> bool {
        self.actor_kinds.iter().any(|m| !m.is_operator())
    }
}

/// Structural problems with a rule table.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RuleSetError {
    /// The table is empty.
    #[error("rule set is empty")]
    Empty,
    /// Two rules share an id.
    #[error("duplicate rule id {0}")]
    DuplicateRuleId(String),
    /// A rule has no actor kinds (it would match nobody or, if treated as a
    /// wildcard, everybody).
    #[error("rule {0} has no actor kinds")]
    EmptyActorKinds(String),
    /// A rule has no actions.
    #[error("rule {0} has no actions")]
    EmptyActions(String),
    /// A rule would grant an operator-only capability to a non-operator actor.
    #[error("rule {rule_id} grants operator-only capability {capability:?} to a non-operator actor")]
    OperatorOnlyCapabilityForNonOperator {
        /// Offending rule.
        rule_id: String,
        /// Offending capability.
        capability: Capability,
    },
    /// A rule would allow a native action (`Execute`, `Connect`, `Load`) without approval.
    #[error("rule {rule_id} allows native action {action:?} without approval")]
    NativeActionAllowed {
        /// Offending rule.
        rule_id: String,
        /// Offending action.
        action: Action,
    },
    /// A rule would grant a non-operator actor a native action.
    #[error("rule {rule_id} grants native action {action:?} to a non-operator actor")]
    NativeActionForNonOperator {
        /// Offending rule.
        rule_id: String,
        /// Offending action.
        action: Action,
    },
    /// The JSON form could not be parsed.
    #[error("rule set JSON is malformed: {0}")]
    Malformed(String),
}

/// Versioned table of rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleSet {
    /// Table version label, e.g. `builtin-v1`.
    pub version: String,
    /// Rules in table order (order is irrelevant for selection).
    pub rules: Vec<PolicyRule>,
}

/// Actions that are not `Read` and not `Propose` (everything a proposal-only
/// actor must never do).
pub const WRITE_ACTIONS: [Action; 14] = [
    Action::Create,
    Action::Update,
    Action::Delete,
    Action::Export,
    Action::Import,
    Action::PromoteCanonical,
    Action::Approve,
    Action::Apply,
    Action::Finalize,
    Action::Invoke,
    Action::Execute,
    Action::Connect,
    Action::Load,
    Action::Correct,
];

/// Actions that mutate authoritative state of a record.
pub const MUTATING_ACTIONS: [Action; 9] = [
    Action::Create,
    Action::Update,
    Action::Delete,
    Action::Import,
    Action::PromoteCanonical,
    Action::Approve,
    Action::Apply,
    Action::Finalize,
    Action::Correct,
];

/// Every action (kept in sync with `g9_domain::Action`; checked by a test).
pub const ALL_ACTIONS: [Action; 16] = [
    Action::Read,
    Action::Create,
    Action::Update,
    Action::Delete,
    Action::Export,
    Action::Import,
    Action::PromoteCanonical,
    Action::Propose,
    Action::Approve,
    Action::Apply,
    Action::Finalize,
    Action::Invoke,
    Action::Execute,
    Action::Connect,
    Action::Load,
    Action::Correct,
];

/// Native host actions that always need a fresh per-operation approval.
pub const NATIVE_ACTIONS: [Action; 3] = [Action::Execute, Action::Connect, Action::Load];

/// Native host resource kinds.
pub const NATIVE_RESOURCES: [ResourceKind; 3] =
    [ResourceKind::FilesystemPath, ResourceKind::Process, ResourceKind::NetworkHost];

/// Resource kinds that are self-modification targets.
pub const SELF_MODIFICATION_RESOURCES: [ResourceKind; 2] = [ResourceKind::Policy, ResourceKind::AppSetting];

/// Resource kinds that carry proposals (model/skill output lands here).
pub const PROPOSAL_RESOURCES: [ResourceKind; 5] = [
    ResourceKind::ScheduleProposal,
    ResourceKind::Intervention,
    ResourceKind::LearnerHypothesis,
    ResourceKind::Experiment,
    ResourceKind::KnowledgeDocument,
];

/// Teaching content: never proposed by skills/model in EXAM mode.
pub const TEACHING_RESOURCES: [ResourceKind; 2] = [ResourceKind::Intervention, ResourceKind::KnowledgeDocument];

/// Evidentiary records (storage uses `ON DELETE RESTRICT`).
pub const EVIDENTIARY_RESOURCES: [ResourceKind; 5] = [
    ResourceKind::Attempt,
    ResourceKind::GradeDecision,
    ResourceKind::Observation,
    ResourceKind::AuditMetadata,
    ResourceKind::Approval,
];

const MODES_EXCEPT_MAINTENANCE: [InteractionMode; 6] = [
    InteractionMode::Study,
    InteractionMode::Lab,
    InteractionMode::Exam,
    InteractionMode::Review,
    InteractionMode::Executive,
    InteractionMode::Research,
];

const MODES_EXCEPT_EXAM: [InteractionMode; 6] = [
    InteractionMode::Study,
    InteractionMode::Lab,
    InteractionMode::Review,
    InteractionMode::Executive,
    InteractionMode::Research,
    InteractionMode::Maintenance,
];

const ANY_KIND: [ResourceKind; 0] = [];
const ANY_MODE: [InteractionMode; 0] = [];
const ANY_DOMAIN: [DataDomain; 0] = [];
const NO_CONDITIONS: [RuleCondition; 0] = [];

#[allow(clippy::too_many_arguments)]
fn rule(
    id: &'static str,
    priority: u16,
    effect: Effect,
    actor_kinds: &[ActorKindMatcher],
    actions: &[Action],
    resource_kinds: &[ResourceKind],
    modes: &[InteractionMode],
    domains: &[DataDomain],
    conditions: &[RuleCondition],
    rationale: &'static str,
) -> PolicyRule {
    PolicyRule {
        id: id.to_string(),
        effect,
        actor_kinds: actor_kinds.to_vec(),
        actions: actions.to_vec(),
        resource_kinds: resource_kinds.to_vec(),
        modes: modes.to_vec(),
        domains: domains.to_vec(),
        conditions: conditions.to_vec(),
        priority,
        rationale: Cow::Borrowed(rationale),
    }
}

impl RuleSet {
    /// The versioned built-in table (`builtin-v1`). Deny by default; see the
    /// module documentation and `docs/threat-model/` for the rationale of each row.
    pub fn builtin_v1() -> RuleSet {
        use ActorKindMatcher as A;
        use Capability as C;
        use ResourceKind as R;
        let op = &[A::Operator];
        let sys = &[A::System];
        let sm = &A::SKILL_OR_MODEL;
        let all = &A::ALL;
        let non_op = &A::NON_OPERATOR;
        let rules = vec![
            // ---- 1000: immutability of finalized records (everyone) ----
            rule(
                "immutable.finalized.deny",
                1000,
                Effect::Deny,
                all,
                &[Action::Update, Action::Delete, Action::Finalize],
                &ANY_KIND,
                &ANY_MODE,
                &ANY_DOMAIN,
                &[RuleCondition::Finalized { value: true }],
                "finalized records are immutable; corrections are superseding records",
            ),
            // ---- 950: evidentiary integrity, self-modification, native resources ----
            rule(
                "evidentiary.delete.deny",
                950,
                Effect::Deny,
                all,
                &[Action::Delete],
                &EVIDENTIARY_RESOURCES,
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "evidentiary records are never deleted (storage ON DELETE RESTRICT)",
            ),
            rule(
                "audit.immutable.deny",
                950,
                Effect::Deny,
                all,
                &[Action::Update, Action::Correct, Action::Apply, Action::Finalize],
                &[R::AuditMetadata],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "audit metadata is append-only and hash-chained",
            ),
            rule(
                "selfmod.non_operator.deny",
                950,
                Effect::Deny,
                non_op,
                &MUTATING_ACTIONS,
                &SELF_MODIFICATION_RESOURCES,
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "only the operator may change policy or settings (Capability::PolicyModify is operator-only)",
            ),
            rule(
                "sm.native.deny",
                950,
                Effect::Deny,
                sm,
                &ALL_ACTIONS,
                &NATIVE_RESOURCES,
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "no shell, network or filesystem capability for skills or the model",
            ),
            rule(
                "sm.approval.deny",
                950,
                Effect::Deny,
                sm,
                &ALL_ACTIONS,
                &[R::Approval],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "a skill or model cannot read, create or decide approvals",
            ),
            // ---- 900: self-modification is MAINTENANCE-only with approval ----
            rule(
                "selfmod.runtime.deny",
                900,
                Effect::Deny,
                op,
                &MUTATING_ACTIONS,
                &SELF_MODIFICATION_RESOURCES,
                &MODES_EXCEPT_MAINTENANCE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "policy and settings change only through an approved MAINTENANCE change set",
            ),
            rule(
                "selfmod.maintenance.approval",
                900,
                Effect::ApprovalRequired(C::PolicyModify),
                op,
                &[Action::Create, Action::Update, Action::Delete],
                &SELF_MODIFICATION_RESOURCES,
                &[InteractionMode::Maintenance],
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "policy and settings changes require an approved MAINTENANCE change set",
            ),
            // ---- 850/800: exam integrity and secrets for proposal-only actors ----
            rule(
                "exam.sm.read.sealed.deny",
                850,
                Effect::Deny,
                sm,
                &[Action::Read],
                &[R::AssessmentDefinition],
                &[InteractionMode::Exam],
                &ANY_DOMAIN,
                &[RuleCondition::ClassificationAtLeast {
                    level: Classification::Secret,
                }],
                "sealed answer keys are never exposed to the model during an exam",
            ),
            rule(
                "sm.read.secret.deny",
                800,
                Effect::Deny,
                sm,
                &[Action::Read],
                &ANY_KIND,
                &ANY_MODE,
                &ANY_DOMAIN,
                &[RuleCondition::ClassificationAtLeast {
                    level: Classification::Secret,
                }],
                "secret-classified content never reaches a skill or the model",
            ),
            rule(
                "exam.sm.propose.teaching.deny",
                800,
                Effect::Deny,
                sm,
                &[Action::Propose],
                &TEACHING_RESOURCES,
                &[InteractionMode::Exam],
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "no hints, lessons or interventions may be proposed in EXAM mode",
            ),
            // ---- 600: cross-domain reads need CrossDomainRead ----
            rule(
                "sm.read.cross_domain.approval",
                600,
                Effect::ApprovalRequired(C::CrossDomainRead),
                sm,
                &[Action::Read],
                &ANY_KIND,
                &ANY_MODE,
                &ANY_DOMAIN,
                &[RuleCondition::ScopeMatchesResource { value: false }],
                "reading outside the invocation's scope domain requires CrossDomainRead",
            ),
            // ---- 500: skills and the model never write ----
            rule(
                "sm.write.deny",
                500,
                Effect::Deny,
                sm,
                &WRITE_ACTIONS,
                &ANY_KIND,
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "skills and the model are proposal-only actors; they never write authoritative state",
            ),
            // ---- 300: approval and audit bookkeeping ----
            rule(
                "approval.decide.allow",
                300,
                Effect::Allow,
                op,
                &[Action::Create, Action::Update, Action::Approve],
                &[R::Approval],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "the operator's approval decision is the trusted authorization path",
            ),
            rule(
                "approval.system.allow",
                300,
                Effect::Allow,
                sys,
                &[Action::Create, Action::Update],
                &[R::Approval],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "the command pipeline creates and expires approval requests",
            ),
            rule(
                "audit.append.allow",
                300,
                Effect::Allow,
                &[A::Operator, A::System],
                &[Action::Create],
                &[R::AuditMetadata],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "audit rows are appended by the pipeline for every command",
            ),
            // ---- 250: host filesystem access always needs approval ----
            rule(
                "operator.fs.read.approval",
                250,
                Effect::ApprovalRequired(C::FilesystemRead),
                op,
                &[Action::Read],
                &[R::FilesystemPath],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "reading a host path requires FilesystemRead for that exact path",
            ),
            rule(
                "operator.fs.write.approval",
                250,
                Effect::ApprovalRequired(C::FilesystemWrite),
                op,
                &[Action::Create, Action::Update, Action::Delete],
                &[R::FilesystemPath],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "writing a host path requires FilesystemWrite for that exact path",
            ),
            // ---- 200: operator actions that need a per-operation approval ----
            rule(
                "operator.security_sensitive.write.approval",
                200,
                Effect::ApprovalRequired(C::ProtectedRecordWrite),
                op,
                &[Action::Create, Action::Update],
                &ANY_KIND,
                &ANY_MODE,
                &[DataDomain::SecuritySensitive],
                &[RuleCondition::Finalized { value: false }],
                "security-sensitive records change only with an explicit approval",
            ),
            rule(
                "operator.apply.schedule.approval",
                200,
                Effect::ApprovalRequired(C::ScheduleApply),
                op,
                &[Action::Apply],
                &[R::ScheduleProposal, R::CalendarEvent],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "applying a schedule proposal to the calendar requires ScheduleApply",
            ),
            rule(
                "operator.apply.record.approval",
                200,
                Effect::ApprovalRequired(C::ProtectedRecordWrite),
                op,
                &[Action::Apply],
                &[R::Intervention, R::LearnerHypothesis, R::Experiment, R::KnowledgeDocument],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "applying a proposal to authoritative state requires ProtectedRecordWrite",
            ),
            rule(
                "operator.finalize.grade.approval",
                200,
                Effect::ApprovalRequired(C::GradeFinalize),
                op,
                &[Action::Finalize],
                &[R::Attempt, R::GradeDecision],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "finalizing a grade or attempt requires GradeFinalize",
            ),
            rule(
                "operator.finalize.record.approval",
                200,
                Effect::ApprovalRequired(C::ProtectedRecordWrite),
                op,
                &[Action::Finalize],
                &[R::StudySession, R::Intervention, R::Experiment, R::Decision, R::Milestone, R::Project, R::Goal],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "finalizing an official record requires ProtectedRecordWrite",
            ),
            rule(
                "operator.promote.approval",
                200,
                Effect::ApprovalRequired(C::CurriculumPromote),
                op,
                &[Action::PromoteCanonical],
                &[R::Curriculum, R::CurriculumVersion],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "promoting a curriculum version to approved canonical requires CurriculumPromote",
            ),
            rule(
                "operator.import.curriculum.approval",
                200,
                Effect::ApprovalRequired(C::CurriculumImport),
                op,
                &[Action::Import],
                &[R::Curriculum, R::CurriculumVersion],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "importing a curriculum manifest requires CurriculumImport",
            ),
            rule(
                "operator.import.document.approval",
                200,
                Effect::ApprovalRequired(C::FilesystemRead),
                op,
                &[Action::Import],
                &[R::Note, R::Attachment, R::EvidenceItem, R::ResearchSource, R::KnowledgeDocument],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "importing a document from the host requires FilesystemRead",
            ),
            rule(
                "operator.approve.curriculum.approval",
                200,
                Effect::ApprovalRequired(C::CurriculumPromote),
                op,
                &[Action::Approve],
                &[R::Curriculum, R::CurriculumVersion],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "approving a curriculum version requires CurriculumPromote",
            ),
            rule(
                "operator.approve.grade.approval",
                200,
                Effect::ApprovalRequired(C::GradeFinalize),
                op,
                &[Action::Approve],
                &[R::Attempt, R::GradeDecision],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "approving a grade requires GradeFinalize",
            ),
            rule(
                "operator.approve.record.approval",
                200,
                Effect::ApprovalRequired(C::ProtectedRecordWrite),
                op,
                &[Action::Approve],
                &[
                    R::ScheduleProposal,
                    R::Intervention,
                    R::LearnerHypothesis,
                    R::Experiment,
                    R::KnowledgeDocument,
                    R::ResearchSource,
                ],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "approving a proposal or source requires ProtectedRecordWrite",
            ),
            rule(
                "operator.delete.approval",
                200,
                Effect::ApprovalRequired(C::DataDelete),
                op,
                &[Action::Delete],
                &ANY_KIND,
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "deletion requires DataDelete for the exact record",
            ),
            rule(
                "operator.export.approval",
                200,
                Effect::ApprovalRequired(C::DataExport),
                op,
                &[Action::Export],
                &ANY_KIND,
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "export requires DataExport for the exact selection",
            ),
            rule(
                "operator.execute.approval",
                200,
                Effect::ApprovalRequired(C::ShellExec),
                op,
                &[Action::Execute],
                &[R::Process],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "executing a native process requires a fresh ShellExec approval",
            ),
            rule(
                "operator.connect.approval",
                200,
                Effect::ApprovalRequired(C::NetworkEgress),
                op,
                &[Action::Connect],
                &[R::NetworkHost],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "opening a network connection requires a fresh NetworkEgress approval",
            ),
            rule(
                "operator.load.approval",
                200,
                Effect::ApprovalRequired(C::ModelLoad),
                op,
                &[Action::Load],
                &[R::Model],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "loading a model artifact requires ModelLoad and a checksum match",
            ),
            rule(
                "operator.correct.grade.approval",
                200,
                Effect::ApprovalRequired(C::GradeFinalize),
                op,
                &[Action::Correct],
                &[R::GradeDecision, R::Attempt],
                &MODES_EXCEPT_EXAM,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "a superseding grade correction is a new final decision and requires GradeFinalize",
            ),
            // ---- 100: ordinary operator, system and proposal-only permissions ----
            rule(
                "operator.read.allow",
                100,
                Effect::Allow,
                op,
                &[Action::Read],
                &ANY_KIND,
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "the operator may read every record in every mode",
            ),
            rule(
                "operator.write.learning.allow",
                100,
                Effect::Allow,
                op,
                &[Action::Create, Action::Update],
                &ANY_KIND,
                &[
                    InteractionMode::Study,
                    InteractionMode::Lab,
                    InteractionMode::Exam,
                    InteractionMode::Review,
                    InteractionMode::Research,
                ],
                &[DataDomain::Learning],
                &[RuleCondition::Finalized { value: false }],
                "learning records are written in learning modes",
            ),
            rule(
                "operator.write.business.allow",
                100,
                Effect::Allow,
                op,
                &[Action::Create, Action::Update],
                &ANY_KIND,
                &[InteractionMode::Executive, InteractionMode::Review],
                &[DataDomain::Business],
                &[RuleCondition::Finalized { value: false }],
                "business records are written in EXECUTIVE and REVIEW modes",
            ),
            rule(
                "operator.write.personal.allow",
                100,
                Effect::Allow,
                op,
                &[Action::Create, Action::Update],
                &ANY_KIND,
                &[InteractionMode::Study, InteractionMode::Review, InteractionMode::Executive],
                &[DataDomain::Personal],
                &[RuleCondition::Finalized { value: false }],
                "personal records are written in STUDY, REVIEW and EXECUTIVE modes",
            ),
            rule(
                "operator.write.system.allow",
                100,
                Effect::Allow,
                op,
                &[Action::Create, Action::Update],
                &ANY_KIND,
                &[InteractionMode::Maintenance],
                &[DataDomain::System],
                &[RuleCondition::Finalized { value: false }],
                "system records are written in MAINTENANCE mode",
            ),
            rule(
                "operator.propose.allow",
                100,
                Effect::Allow,
                op,
                &[Action::Propose],
                &PROPOSAL_RESOURCES,
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "the operator may author proposals",
            ),
            rule(
                "operator.correct.allow",
                100,
                Effect::Allow,
                op,
                &[Action::Correct],
                &[R::Observation, R::Competency, R::TimeInterval, R::StudySession, R::LearnerHypothesis],
                &MODES_EXCEPT_EXAM,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "corrections are superseding records; none during an exam",
            ),
            rule(
                "operator.invoke.allow",
                100,
                Effect::Allow,
                op,
                &[Action::Invoke],
                &[R::Skill, R::Model],
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "the operator may invoke registered skills and the local model",
            ),
            rule(
                "system.read.allow",
                100,
                Effect::Allow,
                sys,
                &[Action::Read],
                &ANY_KIND,
                &ANY_MODE,
                &ANY_DOMAIN,
                &NO_CONDITIONS,
                "deterministic routines read what they need",
            ),
            rule(
                "system.routine.allow",
                100,
                Effect::Allow,
                sys,
                &[Action::Create, Action::Update],
                &[
                    R::Backup,
                    R::TimeInterval,
                    R::StudySession,
                    R::ReviewItem,
                    R::Observation,
                    R::Competency,
                    R::SkillInvocation,
                ],
                &ANY_MODE,
                &[DataDomain::Learning, DataDomain::System],
                &[RuleCondition::Finalized { value: false }],
                "scheduled deterministic routines (backup, timer recovery, retention, derivation)",
            ),
            rule(
                "sm.read.allow",
                100,
                Effect::Allow,
                sm,
                &[Action::Read],
                &ANY_KIND,
                &ANY_MODE,
                &ANY_DOMAIN,
                &[
                    RuleCondition::ClassificationAtMost {
                        level: Classification::Private,
                    },
                    RuleCondition::ScopeMatchesResource { value: true },
                ],
                "skills and the model read non-secret records inside their scope domain",
            ),
            rule(
                "sm.propose.allow",
                100,
                Effect::Allow,
                sm,
                &[Action::Propose],
                &PROPOSAL_RESOURCES,
                &ANY_MODE,
                &ANY_DOMAIN,
                &[RuleCondition::ScopeMatchesResource { value: true }],
                "skills and the model may propose inside their scope domain; proposals never apply state",
            ),
        ];
        RuleSet {
            version: BUILTIN_V1_VERSION.to_string(),
            rules,
        }
    }

    /// Structural validation: unique ids, explicit actors/actions, no
    /// operator-only capability for non-operators, no unapproved native action.
    pub fn validate(&self) -> Result<(), RuleSetError> {
        if self.rules.is_empty() {
            return Err(RuleSetError::Empty);
        }
        let mut ids = BTreeSet::new();
        for r in &self.rules {
            if !ids.insert(r.id.as_str()) {
                return Err(RuleSetError::DuplicateRuleId(r.id.clone()));
            }
            if r.actor_kinds.is_empty() {
                return Err(RuleSetError::EmptyActorKinds(r.id.clone()));
            }
            if r.actions.is_empty() {
                return Err(RuleSetError::EmptyActions(r.id.clone()));
            }
            if let Effect::ApprovalRequired(capability) = r.effect {
                if capability.operator_only() && r.applies_to_non_operator() {
                    return Err(RuleSetError::OperatorOnlyCapabilityForNonOperator {
                        rule_id: r.id.clone(),
                        capability,
                    });
                }
            }
            for action in &r.actions {
                if !NATIVE_ACTIONS.contains(action) {
                    continue;
                }
                match r.effect {
                    Effect::Allow => {
                        return Err(RuleSetError::NativeActionAllowed {
                            rule_id: r.id.clone(),
                            action: *action,
                        })
                    }
                    Effect::ApprovalRequired(_) if r.applies_to_non_operator() => {
                        return Err(RuleSetError::NativeActionForNonOperator {
                            rule_id: r.id.clone(),
                            action: *action,
                        })
                    }
                    Effect::ApprovalRequired(_) | Effect::Deny => {}
                }
            }
        }
        Ok(())
    }

    /// All rules that match, in no particular order.
    pub fn matching<'a>(
        &'a self,
        actor_kind: &ActorKind,
        op: &OperationDescriptor,
        ctx: &ResourceContext,
    ) -> Vec<&'a PolicyRule> {
        self.rules.iter().filter(|r| r.matches(actor_kind, op, ctx)).collect()
    }

    /// The winning rule (highest priority, then most restrictive, then smallest id).
    pub fn select<'a>(
        &'a self,
        actor_kind: &ActorKind,
        op: &OperationDescriptor,
        ctx: &ResourceContext,
    ) -> Option<&'a PolicyRule> {
        self.matching(actor_kind, op, ctx).into_iter().min_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| b.effect.restrictiveness().cmp(&a.effect.restrictiveness()))
                .then_with(|| a.id.cmp(&b.id))
        })
    }

    /// Ids of every rule, in table order.
    pub fn rule_ids(&self) -> Vec<&str> {
        self.rules.iter().map(|r| r.id.as_str()).collect()
    }

    /// Look up a rule by id.
    pub fn get(&self, id: &str) -> Option<&PolicyRule> {
        self.rules.iter().find(|r| r.id == id)
    }

    /// Pretty JSON rendering (used by the policy documentation).
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Parse and validate a JSON rendering.
    pub fn from_json(json: &str) -> Result<RuleSet, RuleSetError> {
        let set: RuleSet = serde_json::from_str(json).map_err(|e| RuleSetError::Malformed(e.to_string()))?;
        set.validate()?;
        Ok(set)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::ResourceRef;

    fn op(action: Action, kind: ResourceKind, mode: InteractionMode, domain: DataDomain) -> OperationDescriptor {
        OperationDescriptor {
            action,
            resource: ResourceRef::new(kind, "r1"),
            scope: Scope { domain, mode },
            parameters_sha256: "00".repeat(32),
        }
    }

    #[test]
    fn builtin_v1_validates_and_has_unique_ids() {
        let set = RuleSet::builtin_v1();
        set.validate().unwrap();
        let ids = set.rule_ids();
        let unique: BTreeSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len());
        assert_eq!(set.version, BUILTIN_V1_VERSION);
    }

    #[test]
    fn builtin_v1_serializes_to_json_and_round_trips() {
        let set = RuleSet::builtin_v1();
        let json = set.to_json_pretty().unwrap();
        assert!(json.contains("\"immutable.finalized.deny\""));
        assert!(json.contains("\"APPROVAL_REQUIRED\""));
        let back = RuleSet::from_json(&json).unwrap();
        assert_eq!(back, set);
    }

    #[test]
    fn validate_rejects_duplicates_and_empty_lists() {
        let mut set = RuleSet::builtin_v1();
        let dup = set.rules[0].clone();
        set.rules.push(dup);
        assert!(matches!(set.validate(), Err(RuleSetError::DuplicateRuleId(_))));

        let mut set = RuleSet::builtin_v1();
        set.rules[0].actor_kinds.clear();
        assert!(matches!(set.validate(), Err(RuleSetError::EmptyActorKinds(_))));

        let mut set = RuleSet::builtin_v1();
        set.rules[0].actions.clear();
        assert!(matches!(set.validate(), Err(RuleSetError::EmptyActions(_))));

        let empty = RuleSet {
            version: "x".into(),
            rules: vec![],
        };
        assert_eq!(empty.validate(), Err(RuleSetError::Empty));
    }

    #[test]
    fn validate_rejects_operator_only_capability_for_skill() {
        let mut set = RuleSet::builtin_v1();
        set.rules.push(rule(
            "evil.skill.shell",
            5000,
            Effect::ApprovalRequired(Capability::ShellExec),
            &[ActorKindMatcher::Skill],
            &[Action::Execute],
            &ANY_KIND,
            &ANY_MODE,
            &ANY_DOMAIN,
            &NO_CONDITIONS,
            "evil",
        ));
        assert!(matches!(
            set.validate(),
            Err(RuleSetError::OperatorOnlyCapabilityForNonOperator { .. })
        ));
    }

    #[test]
    fn validate_rejects_unapproved_native_action() {
        let mut set = RuleSet::builtin_v1();
        set.rules.push(rule(
            "evil.operator.exec",
            5000,
            Effect::Allow,
            &[ActorKindMatcher::Operator],
            &[Action::Execute],
            &ANY_KIND,
            &ANY_MODE,
            &ANY_DOMAIN,
            &NO_CONDITIONS,
            "evil",
        ));
        assert!(matches!(set.validate(), Err(RuleSetError::NativeActionAllowed { .. })));

        let mut set = RuleSet::builtin_v1();
        set.rules.push(rule(
            "evil.skill.connect",
            5000,
            Effect::ApprovalRequired(Capability::CrossDomainRead),
            &[ActorKindMatcher::Skill],
            &[Action::Connect],
            &ANY_KIND,
            &ANY_MODE,
            &ANY_DOMAIN,
            &NO_CONDITIONS,
            "evil",
        ));
        assert!(matches!(set.validate(), Err(RuleSetError::NativeActionForNonOperator { .. })));
    }

    #[test]
    fn select_prefers_priority_then_restrictiveness_then_id() {
        let set = RuleSet {
            version: "t".into(),
            rules: vec![
                rule(
                    "b.allow",
                    10,
                    Effect::Allow,
                    &[ActorKindMatcher::Operator],
                    &[Action::Read],
                    &ANY_KIND,
                    &ANY_MODE,
                    &ANY_DOMAIN,
                    &NO_CONDITIONS,
                    "",
                ),
                rule(
                    "a.deny",
                    10,
                    Effect::Deny,
                    &[ActorKindMatcher::Operator],
                    &[Action::Read],
                    &ANY_KIND,
                    &ANY_MODE,
                    &ANY_DOMAIN,
                    &NO_CONDITIONS,
                    "",
                ),
                rule(
                    "c.allow",
                    20,
                    Effect::Allow,
                    &[ActorKindMatcher::Operator],
                    &[Action::Read],
                    &[ResourceKind::Note],
                    &ANY_MODE,
                    &ANY_DOMAIN,
                    &NO_CONDITIONS,
                    "",
                ),
            ],
        };
        let ctx = ResourceContext::new(DataDomain::Learning, Classification::Internal, false);
        let note = op(Action::Read, ResourceKind::Note, InteractionMode::Study, DataDomain::Learning);
        assert_eq!(set.select(&ActorKind::Operator, &note, &ctx).unwrap().id, "c.allow");
        let goal = op(Action::Read, ResourceKind::Goal, InteractionMode::Study, DataDomain::Learning);
        assert_eq!(set.select(&ActorKind::Operator, &goal, &ctx).unwrap().id, "a.deny");
        assert!(set.select(&ActorKind::Model, &goal, &ctx).is_none());
    }

    #[test]
    fn conditions_evaluate_against_context() {
        let scope = Scope {
            domain: DataDomain::Learning,
            mode: InteractionMode::Study,
        };
        let ctx = ResourceContext::new(DataDomain::Business, Classification::Private, true);
        assert!(RuleCondition::ClassificationAtLeast {
            level: Classification::Private
        }
        .holds(&scope, &ctx));
        assert!(!RuleCondition::ClassificationAtLeast {
            level: Classification::Secret
        }
        .holds(&scope, &ctx));
        assert!(RuleCondition::ClassificationAtMost {
            level: Classification::Private
        }
        .holds(&scope, &ctx));
        assert!(RuleCondition::Finalized { value: true }.holds(&scope, &ctx));
        assert!(RuleCondition::ScopeMatchesResource { value: false }.holds(&scope, &ctx));
        let in_scope = ResourceContext::new(DataDomain::Learning, Classification::Public, false);
        assert!(RuleCondition::ScopeMatchesResource { value: true }.holds(&scope, &in_scope));
        let owner_mismatch = in_scope.with_owner_domain_matches_scope(false);
        assert!(RuleCondition::ScopeMatchesResource { value: false }.holds(&scope, &owner_mismatch));
    }

    #[test]
    fn action_constants_cover_the_domain_enum() {
        let distinct: BTreeSet<String> = ALL_ACTIONS.iter().map(|a| format!("{a:?}")).collect();
        assert_eq!(distinct.len(), 16, "g9_domain::Action has 16 variants");
        let writes: BTreeSet<String> = WRITE_ACTIONS.iter().map(|a| format!("{a:?}")).collect();
        assert!(!writes.contains("Read"));
        assert!(WRITE_ACTIONS.iter().all(|a| *a != Action::Propose && *a != Action::Read));
        assert_eq!(WRITE_ACTIONS.len() + 2, ALL_ACTIONS.len());
    }
}
