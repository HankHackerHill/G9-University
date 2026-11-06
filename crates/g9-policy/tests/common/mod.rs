//! Shared fixtures for the g9-policy gate tests. Deterministic: fixed clocks,
//! fixed key bytes, no I/O.
#![allow(dead_code)]

use g9_domain::{
    Action, Actor, ActorId, Classification, DataDomain, InteractionMode, OperationDescriptor,
    ResourceKind, ResourceRef, Scope, SkillId, UtcMillis,
};
use g9_policy::{BindingKey, Engine, FixedClock, ResourceContext};

/// 2025-11-05T00:00:00.000Z.
pub const T0: UtcMillis = UtcMillis::from_millis(1_762_300_800_000);

pub fn key() -> BindingKey {
    BindingKey::from_bytes([0x5a; 32])
}

pub fn other_key() -> BindingKey {
    BindingKey::from_bytes([0xa5; 32])
}

pub fn operator() -> Actor {
    Actor::operator(ActorId::parse("operator-1").unwrap())
}

pub fn system() -> Actor {
    Actor::system(ActorId::parse("system").unwrap())
}

pub fn skill() -> Actor {
    Actor::skill(
        ActorId::parse("skill-g9-session-director").unwrap(),
        SkillId::parse("g9-session-director").unwrap(),
    )
}

pub fn model() -> Actor {
    Actor::model(ActorId::parse("model-local").unwrap())
}

pub fn engine_at(t: UtcMillis) -> Engine {
    Engine::builtin_v1(Box::new(FixedClock(t)))
}

pub fn engine() -> Engine {
    engine_at(T0)
}

pub fn op(
    action: Action,
    kind: ResourceKind,
    mode: InteractionMode,
    domain: DataDomain,
) -> OperationDescriptor {
    op_with(action, kind, "res-1", mode, domain, "00")
}

pub fn op_with(
    action: Action,
    kind: ResourceKind,
    id: &str,
    mode: InteractionMode,
    domain: DataDomain,
    params_byte: &str,
) -> OperationDescriptor {
    OperationDescriptor {
        action,
        resource: ResourceRef::new(kind, id),
        scope: Scope { domain, mode },
        parameters_sha256: params_byte.repeat(32),
    }
}

pub fn ctx(domain: DataDomain, classification: Classification, finalized: bool) -> ResourceContext {
    ResourceContext::new(domain, classification, finalized)
}

/// Every `ResourceKind` (36). Kept in sync with `g9_domain::policy::ResourceKind`
/// by the `all_kinds_is_complete` check in the A06 suite.
pub const ALL_KINDS: [ResourceKind; 36] = [
    ResourceKind::Curriculum,
    ResourceKind::CurriculumVersion,
    ResourceKind::CalendarEvent,
    ResourceKind::ScheduleProposal,
    ResourceKind::StudySession,
    ResourceKind::TimeInterval,
    ResourceKind::Note,
    ResourceKind::Attachment,
    ResourceKind::EvidenceItem,
    ResourceKind::AssessmentDefinition,
    ResourceKind::Attempt,
    ResourceKind::GradeDecision,
    ResourceKind::Competency,
    ResourceKind::Observation,
    ResourceKind::LearnerHypothesis,
    ResourceKind::ReviewItem,
    ResourceKind::Intervention,
    ResourceKind::Experiment,
    ResourceKind::Goal,
    ResourceKind::Project,
    ResourceKind::Milestone,
    ResourceKind::Decision,
    ResourceKind::ResearchSource,
    ResourceKind::KnowledgeDocument,
    ResourceKind::Model,
    ResourceKind::Skill,
    ResourceKind::SkillInvocation,
    ResourceKind::Approval,
    ResourceKind::Backup,
    ResourceKind::Policy,
    ResourceKind::Export,
    ResourceKind::FilesystemPath,
    ResourceKind::NetworkHost,
    ResourceKind::Process,
    ResourceKind::AppSetting,
    ResourceKind::AuditMetadata,
];

pub const ALL_DOMAINS: [DataDomain; 5] = [
    DataDomain::Learning,
    DataDomain::Business,
    DataDomain::Personal,
    DataDomain::SecuritySensitive,
    DataDomain::System,
];

pub const ALL_CLASSIFICATIONS: [Classification; 4] = [
    Classification::Public,
    Classification::Internal,
    Classification::Private,
    Classification::Secret,
];
