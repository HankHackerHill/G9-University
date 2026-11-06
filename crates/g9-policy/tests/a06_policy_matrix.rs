//! Gate A06 — policy denies unapproved actions (ARCHITECTURE.md §14).
//!
//! Table-driven matrix over (actor kind, action, resource kind, mode, scope
//! domain, resource domain, classification, finalized) → expected decision and
//! winning rule id, plus exhaustive sweeps for the properties that must hold
//! for every combination.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::*;
use g9_domain::{
    Action, Actor, Capability, Classification, DataDomain, InteractionMode, ResourceKind,
};
use g9_policy::rules::{ALL_ACTIONS, NATIVE_ACTIONS, WRITE_ACTIONS};
use g9_policy::{DecisionKind, Engine, FixedClock, RuleSet, DEFAULT_DENY_RULE_ID};
use std::collections::BTreeSet;

use Action as Ac;
use Classification as Cl;
use DataDomain as D;
use DecisionKind as K;
use InteractionMode as M;
use ResourceKind as R;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Who {
    Op,
    Sys,
    Skill,
    Model,
}

impl Who {
    fn actor(self) -> Actor {
        match self {
            Who::Op => operator(),
            Who::Sys => system(),
            Who::Skill => skill(),
            Who::Model => model(),
        }
    }
}

#[derive(Debug, Clone)]
struct Row {
    who: Who,
    action: Action,
    kind: ResourceKind,
    mode: InteractionMode,
    scope_domain: DataDomain,
    resource_domain: DataDomain,
    classification: Classification,
    finalized: bool,
    owner_matches: bool,
    expect: DecisionKind,
    rule: &'static str,
    capability: Option<Capability>,
}

#[allow(clippy::too_many_arguments)]
const fn row(
    who: Who,
    action: Action,
    kind: ResourceKind,
    mode: InteractionMode,
    domain: DataDomain,
    classification: Classification,
    finalized: bool,
    expect: DecisionKind,
    rule: &'static str,
) -> Row {
    Row {
        who,
        action,
        kind,
        mode,
        scope_domain: domain,
        resource_domain: domain,
        classification,
        finalized,
        owner_matches: true,
        expect,
        rule,
        capability: None,
    }
}

impl Row {
    const fn cap(mut self, c: Capability) -> Self {
        self.capability = Some(c);
        self
    }

    /// Resource lives in `resource_domain` while the command scope is `scope_domain`.
    const fn cross(mut self, resource_domain: DataDomain) -> Self {
        self.resource_domain = resource_domain;
        self
    }

    const fn owner_mismatch(mut self) -> Self {
        self.owner_matches = false;
        self
    }
}

/// The A06 matrix. Every builtin-v1 rule and the default deny must appear as
/// a winning rule at least once (asserted below).
fn matrix() -> Vec<Row> {
    use Capability as C;
    use Who::*;
    vec![
        // ---------------- operator: reads ----------------
        row(
            Op,
            Ac::Read,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "operator.read.allow",
        ),
        row(
            Op,
            Ac::Read,
            R::Goal,
            M::Exam,
            D::Business,
            Cl::Private,
            false,
            K::Allow,
            "operator.read.allow",
        ),
        row(
            Op,
            Ac::Read,
            R::AssessmentDefinition,
            M::Exam,
            D::Learning,
            Cl::Secret,
            false,
            K::Allow,
            "operator.read.allow",
        ),
        row(
            Op,
            Ac::Read,
            R::AuditMetadata,
            M::Maintenance,
            D::System,
            Cl::Internal,
            true,
            K::Allow,
            "operator.read.allow",
        ),
        row(
            Op,
            Ac::Read,
            R::FilesystemPath,
            M::Study,
            D::System,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.fs.read.approval",
        )
        .cap(C::FilesystemRead),
        // ---------------- operator: host writes ----------------
        row(
            Op,
            Ac::Create,
            R::FilesystemPath,
            M::Study,
            D::System,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.fs.write.approval",
        )
        .cap(C::FilesystemWrite),
        row(
            Op,
            Ac::Delete,
            R::FilesystemPath,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.fs.write.approval",
        )
        .cap(C::FilesystemWrite),
        // ---------------- operator: ordinary writes per domain/mode ----------------
        row(
            Op,
            Ac::Create,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "operator.write.learning.allow",
        ),
        row(
            Op,
            Ac::Update,
            R::Attempt,
            M::Exam,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "operator.write.learning.allow",
        ),
        row(
            Op,
            Ac::Update,
            R::Competency,
            M::Research,
            D::Learning,
            Cl::Public,
            false,
            K::Allow,
            "operator.write.learning.allow",
        ),
        row(
            Op,
            Ac::Create,
            R::Goal,
            M::Executive,
            D::Business,
            Cl::Internal,
            false,
            K::Allow,
            "operator.write.business.allow",
        ),
        row(
            Op,
            Ac::Update,
            R::Project,
            M::Review,
            D::Business,
            Cl::Private,
            false,
            K::Allow,
            "operator.write.business.allow",
        ),
        row(
            Op,
            Ac::Create,
            R::Goal,
            M::Study,
            D::Business,
            Cl::Internal,
            false,
            K::Deny,
            DEFAULT_DENY_RULE_ID,
        ),
        row(
            Op,
            Ac::Update,
            R::Note,
            M::Review,
            D::Personal,
            Cl::Private,
            false,
            K::Allow,
            "operator.write.personal.allow",
        ),
        row(
            Op,
            Ac::Create,
            R::Note,
            M::Lab,
            D::Personal,
            Cl::Private,
            false,
            K::Deny,
            DEFAULT_DENY_RULE_ID,
        ),
        row(
            Op,
            Ac::Update,
            R::Backup,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::Allow,
            "operator.write.system.allow",
        ),
        row(
            Op,
            Ac::Update,
            R::Backup,
            M::Study,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            DEFAULT_DENY_RULE_ID,
        ),
        row(
            Op,
            Ac::Create,
            R::Note,
            M::Study,
            D::SecuritySensitive,
            Cl::Private,
            false,
            K::ApprovalRequired,
            "operator.security_sensitive.write.approval",
        )
        .cap(C::ProtectedRecordWrite),
        // ---------------- immutability / evidentiary / audit (everyone) ----------------
        row(
            Op,
            Ac::Update,
            R::GradeDecision,
            M::Review,
            D::Learning,
            Cl::Internal,
            true,
            K::Deny,
            "immutable.finalized.deny",
        ),
        row(
            Op,
            Ac::Delete,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Internal,
            true,
            K::Deny,
            "immutable.finalized.deny",
        ),
        row(
            Op,
            Ac::Finalize,
            R::Attempt,
            M::Review,
            D::Learning,
            Cl::Internal,
            true,
            K::Deny,
            "immutable.finalized.deny",
        ),
        row(
            Sys,
            Ac::Update,
            R::Observation,
            M::Study,
            D::Learning,
            Cl::Internal,
            true,
            K::Deny,
            "immutable.finalized.deny",
        ),
        row(
            Skill,
            Ac::Update,
            R::GradeDecision,
            M::Review,
            D::Learning,
            Cl::Internal,
            true,
            K::Deny,
            "immutable.finalized.deny",
        ),
        row(
            Op,
            Ac::Delete,
            R::Attempt,
            M::Review,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "evidentiary.delete.deny",
        ),
        row(
            Op,
            Ac::Delete,
            R::AuditMetadata,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            "evidentiary.delete.deny",
        ),
        row(
            Model,
            Ac::Delete,
            R::Attempt,
            M::Review,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "evidentiary.delete.deny",
        ),
        row(
            Op,
            Ac::Update,
            R::AuditMetadata,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            "audit.immutable.deny",
        ),
        row(
            Op,
            Ac::Create,
            R::AuditMetadata,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "audit.append.allow",
        ),
        row(
            Sys,
            Ac::Create,
            R::AuditMetadata,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "audit.append.allow",
        ),
        // ---------------- self-modification ----------------
        row(
            Op,
            Ac::Update,
            R::Policy,
            M::Study,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            "selfmod.runtime.deny",
        ),
        row(
            Op,
            Ac::Update,
            R::AppSetting,
            M::Executive,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            "selfmod.runtime.deny",
        ),
        row(
            Op,
            Ac::Update,
            R::Policy,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "selfmod.maintenance.approval",
        )
        .cap(C::PolicyModify),
        row(
            Op,
            Ac::Create,
            R::AppSetting,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "selfmod.maintenance.approval",
        )
        .cap(C::PolicyModify),
        row(
            Sys,
            Ac::Update,
            R::AppSetting,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            "selfmod.non_operator.deny",
        ),
        row(
            Skill,
            Ac::Update,
            R::Policy,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            "selfmod.non_operator.deny",
        ),
        row(
            Model,
            Ac::Create,
            R::AppSetting,
            M::Study,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            "selfmod.non_operator.deny",
        ),
        // ---------------- approvals bookkeeping ----------------
        row(
            Op,
            Ac::Approve,
            R::Approval,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "approval.decide.allow",
        ),
        row(
            Op,
            Ac::Update,
            R::Approval,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::Allow,
            "approval.decide.allow",
        ),
        row(
            Sys,
            Ac::Create,
            R::Approval,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "approval.system.allow",
        ),
        row(
            Model,
            Ac::Read,
            R::Approval,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "sm.approval.deny",
        ),
        row(
            Skill,
            Ac::Approve,
            R::Approval,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "sm.approval.deny",
        ),
        // ---------------- operator: approval-gated verbs ----------------
        row(
            Op,
            Ac::Apply,
            R::ScheduleProposal,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.apply.schedule.approval",
        )
        .cap(C::ScheduleApply),
        row(
            Op,
            Ac::Apply,
            R::CalendarEvent,
            M::Executive,
            D::Business,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.apply.schedule.approval",
        )
        .cap(C::ScheduleApply),
        row(
            Op,
            Ac::Apply,
            R::Intervention,
            M::Review,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.apply.record.approval",
        )
        .cap(C::ProtectedRecordWrite),
        row(
            Op,
            Ac::Apply,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            DEFAULT_DENY_RULE_ID,
        ),
        row(
            Op,
            Ac::Finalize,
            R::GradeDecision,
            M::Review,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.finalize.grade.approval",
        )
        .cap(C::GradeFinalize),
        row(
            Op,
            Ac::Finalize,
            R::Attempt,
            M::Exam,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.finalize.grade.approval",
        )
        .cap(C::GradeFinalize),
        row(
            Op,
            Ac::Finalize,
            R::StudySession,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.finalize.record.approval",
        )
        .cap(C::ProtectedRecordWrite),
        row(
            Op,
            Ac::Finalize,
            R::Decision,
            M::Executive,
            D::Business,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.finalize.record.approval",
        )
        .cap(C::ProtectedRecordWrite),
        row(
            Op,
            Ac::PromoteCanonical,
            R::CurriculumVersion,
            M::Maintenance,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.promote.approval",
        )
        .cap(C::CurriculumPromote),
        row(
            Op,
            Ac::Import,
            R::Curriculum,
            M::Maintenance,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.import.curriculum.approval",
        )
        .cap(C::CurriculumImport),
        row(
            Op,
            Ac::Import,
            R::Note,
            M::Research,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.import.document.approval",
        )
        .cap(C::FilesystemRead),
        row(
            Op,
            Ac::Approve,
            R::CurriculumVersion,
            M::Maintenance,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.approve.curriculum.approval",
        )
        .cap(C::CurriculumPromote),
        row(
            Op,
            Ac::Approve,
            R::GradeDecision,
            M::Review,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.approve.grade.approval",
        )
        .cap(C::GradeFinalize),
        row(
            Op,
            Ac::Approve,
            R::ScheduleProposal,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.approve.record.approval",
        )
        .cap(C::ProtectedRecordWrite),
        row(
            Op,
            Ac::Approve,
            R::ResearchSource,
            M::Research,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.approve.record.approval",
        )
        .cap(C::ProtectedRecordWrite),
        row(
            Op,
            Ac::Delete,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.delete.approval",
        )
        .cap(C::DataDelete),
        row(
            Op,
            Ac::Delete,
            R::Goal,
            M::Executive,
            D::Business,
            Cl::Private,
            false,
            K::ApprovalRequired,
            "operator.delete.approval",
        )
        .cap(C::DataDelete),
        row(
            Op,
            Ac::Export,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.export.approval",
        )
        .cap(C::DataExport),
        row(
            Op,
            Ac::Export,
            R::Export,
            M::Executive,
            D::Business,
            Cl::Private,
            false,
            K::ApprovalRequired,
            "operator.export.approval",
        )
        .cap(C::DataExport),
        row(
            Op,
            Ac::Execute,
            R::Process,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.execute.approval",
        )
        .cap(C::ShellExec),
        row(
            Op,
            Ac::Execute,
            R::Process,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.execute.approval",
        )
        .cap(C::ShellExec),
        row(
            Op,
            Ac::Execute,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            DEFAULT_DENY_RULE_ID,
        ),
        row(
            Op,
            Ac::Connect,
            R::NetworkHost,
            M::Research,
            D::System,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.connect.approval",
        )
        .cap(C::NetworkEgress),
        row(
            Op,
            Ac::Load,
            R::Model,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "operator.load.approval",
        )
        .cap(C::ModelLoad),
        row(
            Op,
            Ac::Correct,
            R::GradeDecision,
            M::Review,
            D::Learning,
            Cl::Internal,
            true,
            K::ApprovalRequired,
            "operator.correct.grade.approval",
        )
        .cap(C::GradeFinalize),
        row(
            Op,
            Ac::Correct,
            R::Observation,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "operator.correct.allow",
        ),
        row(
            Op,
            Ac::Correct,
            R::TimeInterval,
            M::Review,
            D::Learning,
            Cl::Internal,
            true,
            K::Allow,
            "operator.correct.allow",
        ),
        row(
            Op,
            Ac::Correct,
            R::Observation,
            M::Exam,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            DEFAULT_DENY_RULE_ID,
        ),
        row(
            Op,
            Ac::Propose,
            R::ScheduleProposal,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "operator.propose.allow",
        ),
        row(
            Op,
            Ac::Invoke,
            R::Skill,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "operator.invoke.allow",
        ),
        row(
            Op,
            Ac::Invoke,
            R::Model,
            M::Exam,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "operator.invoke.allow",
        ),
        row(
            Op,
            Ac::Invoke,
            R::Process,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            DEFAULT_DENY_RULE_ID,
        ),
        // ---------------- system routines ----------------
        row(
            Sys,
            Ac::Read,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Private,
            false,
            K::Allow,
            "system.read.allow",
        ),
        row(
            Sys,
            Ac::Update,
            R::Backup,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::Allow,
            "system.routine.allow",
        ),
        row(
            Sys,
            Ac::Create,
            R::TimeInterval,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "system.routine.allow",
        ),
        row(
            Sys,
            Ac::Create,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            DEFAULT_DENY_RULE_ID,
        ),
        row(
            Sys,
            Ac::Execute,
            R::Process,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            DEFAULT_DENY_RULE_ID,
        ),
        row(
            Sys,
            Ac::Propose,
            R::Intervention,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            DEFAULT_DENY_RULE_ID,
        ),
        row(
            Sys,
            Ac::Delete,
            R::Note,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            DEFAULT_DENY_RULE_ID,
        ),
        // ---------------- skills and the model: reads ----------------
        row(
            Skill,
            Ac::Read,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "sm.read.allow",
        ),
        row(
            Model,
            Ac::Read,
            R::Attempt,
            M::Exam,
            D::Learning,
            Cl::Private,
            false,
            K::Allow,
            "sm.read.allow",
        ),
        row(
            Model,
            Ac::Read,
            R::Goal,
            M::Executive,
            D::Business,
            Cl::Internal,
            false,
            K::Allow,
            "sm.read.allow",
        ),
        row(
            Skill,
            Ac::Read,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Secret,
            false,
            K::Deny,
            "sm.read.secret.deny",
        ),
        row(
            Model,
            Ac::Read,
            R::AssessmentDefinition,
            M::Study,
            D::Learning,
            Cl::Secret,
            false,
            K::Deny,
            "sm.read.secret.deny",
        ),
        row(
            Model,
            Ac::Read,
            R::AssessmentDefinition,
            M::Exam,
            D::Learning,
            Cl::Secret,
            false,
            K::Deny,
            "exam.sm.read.sealed.deny",
        ),
        row(
            Skill,
            Ac::Read,
            R::AssessmentDefinition,
            M::Exam,
            D::Learning,
            Cl::Secret,
            false,
            K::Deny,
            "exam.sm.read.sealed.deny",
        ),
        row(
            Model,
            Ac::Read,
            R::AssessmentDefinition,
            M::Exam,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "sm.read.allow",
        ),
        row(
            Skill,
            Ac::Read,
            R::Goal,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "sm.read.cross_domain.approval",
        )
        .cross(D::Business)
        .cap(C::CrossDomainRead),
        row(
            Model,
            Ac::Read,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Private,
            false,
            K::ApprovalRequired,
            "sm.read.cross_domain.approval",
        )
        .cross(D::Personal)
        .cap(C::CrossDomainRead),
        row(
            Model,
            Ac::Read,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Secret,
            false,
            K::Deny,
            "sm.read.secret.deny",
        )
        .cross(D::Personal),
        row(
            Skill,
            Ac::Read,
            R::Attachment,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::ApprovalRequired,
            "sm.read.cross_domain.approval",
        )
        .owner_mismatch()
        .cap(C::CrossDomainRead),
        // ---------------- skills and the model: proposals ----------------
        row(
            Skill,
            Ac::Propose,
            R::ScheduleProposal,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "sm.propose.allow",
        ),
        row(
            Model,
            Ac::Propose,
            R::LearnerHypothesis,
            M::Review,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "sm.propose.allow",
        ),
        row(
            Model,
            Ac::Propose,
            R::Experiment,
            M::Lab,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "sm.propose.allow",
        ),
        row(
            Model,
            Ac::Propose,
            R::ScheduleProposal,
            M::Exam,
            D::Learning,
            Cl::Internal,
            false,
            K::Allow,
            "sm.propose.allow",
        ),
        row(
            Model,
            Ac::Propose,
            R::Intervention,
            M::Exam,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "exam.sm.propose.teaching.deny",
        ),
        row(
            Skill,
            Ac::Propose,
            R::KnowledgeDocument,
            M::Exam,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "exam.sm.propose.teaching.deny",
        ),
        row(
            Skill,
            Ac::Propose,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            DEFAULT_DENY_RULE_ID,
        ),
        row(
            Model,
            Ac::Propose,
            R::ScheduleProposal,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            DEFAULT_DENY_RULE_ID,
        )
        .cross(D::Business),
        // ---------------- skills and the model: every write verb is denied ----------------
        row(
            Skill,
            Ac::Create,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "sm.write.deny",
        ),
        row(
            Model,
            Ac::Update,
            R::ScheduleProposal,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "sm.write.deny",
        ),
        row(
            Skill,
            Ac::Delete,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "sm.write.deny",
        ),
        row(
            Model,
            Ac::Export,
            R::Note,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "sm.write.deny",
        ),
        row(
            Skill,
            Ac::Invoke,
            R::Skill,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "sm.write.deny",
        ),
        row(
            Model,
            Ac::Apply,
            R::ScheduleProposal,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "sm.write.deny",
        ),
        row(
            Skill,
            Ac::Finalize,
            R::GradeDecision,
            M::Review,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "sm.write.deny",
        ),
        row(
            Model,
            Ac::Approve,
            R::GradeDecision,
            M::Review,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "sm.write.deny",
        ),
        row(
            Skill,
            Ac::Import,
            R::Curriculum,
            M::Maintenance,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "sm.write.deny",
        ),
        row(
            Model,
            Ac::Load,
            R::Model,
            M::Maintenance,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            "sm.write.deny",
        ),
        row(
            Model,
            Ac::Correct,
            R::Observation,
            M::Study,
            D::Learning,
            Cl::Internal,
            false,
            K::Deny,
            "sm.write.deny",
        ),
        // ---------------- skills and the model: native host resources ----------------
        row(
            Skill,
            Ac::Execute,
            R::Process,
            M::Study,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            "sm.native.deny",
        ),
        row(
            Model,
            Ac::Connect,
            R::NetworkHost,
            M::Research,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            "sm.native.deny",
        ),
        row(
            Skill,
            Ac::Read,
            R::FilesystemPath,
            M::Study,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            "sm.native.deny",
        ),
        row(
            Model,
            Ac::Create,
            R::FilesystemPath,
            M::Study,
            D::System,
            Cl::Internal,
            false,
            K::Deny,
            "sm.native.deny",
        ),
    ]
}

fn evaluate(engine: &Engine, r: &Row) -> g9_policy::Evaluation {
    let operation = op(r.action, r.kind, r.mode, r.scope_domain);
    let context = ctx(r.resource_domain, r.classification, r.finalized)
        .with_owner_domain_matches_scope(r.owner_matches);
    engine.evaluate(&r.who.actor(), &operation, &context)
}

#[test]
fn a06_matrix_has_at_least_sixty_rows() {
    assert!(matrix().len() >= 60, "matrix has {} rows", matrix().len());
}

#[test]
fn a06_matrix_decisions_and_rule_ids() {
    let engine = engine();
    for (i, r) in matrix().iter().enumerate() {
        let eval = evaluate(&engine, r);
        assert_eq!(
            eval.kind(),
            r.expect,
            "row {i}: {r:?} → {:?}",
            eval.decision
        );
        assert_eq!(
            eval.rule_id(),
            r.rule,
            "row {i}: {r:?} → {:?}",
            eval.decision
        );
        assert_eq!(eval.required_capability(), r.capability, "row {i}: {r:?}");
        assert_eq!(eval.record.evaluated_at, T0);
        assert_eq!(eval.record.decision, r.expect);
        assert_eq!(eval.record.resource_domain, r.resource_domain);
        assert_eq!(eval.record.scope_domain, r.scope_domain);
    }
}

#[test]
fn a06_matrix_covers_every_builtin_rule_and_default_deny() {
    let engine = engine();
    let winning: BTreeSet<String> = matrix()
        .iter()
        .map(|r| evaluate(&engine, r).rule_id().to_string())
        .collect();
    let mut expected: BTreeSet<String> = engine
        .rules()
        .rule_ids()
        .into_iter()
        .map(str::to_string)
        .collect();
    expected.insert(DEFAULT_DENY_RULE_ID.to_string());
    let missing: Vec<&String> = expected.difference(&winning).collect();
    assert!(
        missing.is_empty(),
        "rules never exercised by the matrix: {missing:?}"
    );
    let unknown: Vec<&String> = winning.difference(&expected).collect();
    assert!(
        unknown.is_empty(),
        "matrix expects rule ids that do not exist: {unknown:?}"
    );
}

#[test]
fn a06_every_rule_has_unique_id() {
    let set = RuleSet::builtin_v1();
    let ids = set.rule_ids();
    let unique: BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(ids.len(), unique.len(), "duplicate rule ids in builtin-v1");
    assert!(
        !unique.contains(DEFAULT_DENY_RULE_ID),
        "default deny is not a table row"
    );
    assert!(ids.iter().all(|id| id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c == '.' || c == '_')));
    set.validate().unwrap();
}

#[test]
fn a06_builtin_rules_serialize_to_json() {
    let set = RuleSet::builtin_v1();
    let json = set.to_json_pretty().unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["version"], "builtin-v1");
    let rules = value["rules"].as_array().unwrap();
    assert_eq!(rules.len(), set.rules.len());
    for (rule, rendered) in set.rules.iter().zip(rules) {
        assert_eq!(rendered["id"], rule.id);
        assert!(rendered["rationale"]
            .as_str()
            .map(|s| !s.is_empty())
            .unwrap_or(false));
        assert!(rendered["effect"]["kind"].is_string());
    }
    let back = RuleSet::from_json(&json).unwrap();
    assert_eq!(back, set);
}

#[test]
fn a06_deny_by_default_when_no_rule_matches() {
    let mut set = RuleSet::builtin_v1();
    set.rules.retain(|r| r.id == "operator.read.allow");
    let engine = Engine::new(set, Box::new(FixedClock(T0))).unwrap();
    let eval = engine.evaluate(
        &operator(),
        &op(Ac::Create, R::Note, M::Study, D::Learning),
        &ctx(D::Learning, Cl::Internal, false),
    );
    assert_eq!(eval.kind(), K::Deny);
    assert_eq!(eval.rule_id(), DEFAULT_DENY_RULE_ID);
    assert_eq!(eval.record.reason, g9_policy::DEFAULT_DENY_REASON);
}

#[test]
fn a06_all_kinds_fixture_is_complete() {
    // Serialize each kind and make sure the fixture has no duplicates; if the
    // domain enum grows, the count assertion below forces this file to follow.
    let names: BTreeSet<String> = ALL_KINDS
        .iter()
        .map(|k| serde_json::to_string(k).unwrap())
        .collect();
    assert_eq!(names.len(), ALL_KINDS.len());
    assert_eq!(
        ALL_KINDS.len(),
        36,
        "g9_domain::ResourceKind has 36 variants"
    );
    assert_eq!(ALL_ACTIONS.len(), 16);
}

#[test]
fn a06_skill_and_model_never_allowed_to_write_exhaustive() {
    let engine = engine();
    let mut evaluated = 0usize;
    for actor in [skill(), model()] {
        for action in WRITE_ACTIONS {
            for kind in ALL_KINDS {
                for mode in M::ALL {
                    for domain in ALL_DOMAINS {
                        for finalized in [false, true] {
                            let eval = engine.evaluate(
                                &actor,
                                &op(action, kind, mode, domain),
                                &ctx(domain, Cl::Internal, finalized),
                            );
                            evaluated += 1;
                            assert_eq!(
                                eval.kind(),
                                K::Deny,
                                "{actor:?} {action:?} {kind:?} {mode:?} {domain:?}"
                            );
                        }
                    }
                }
            }
        }
    }
    assert_eq!(
        evaluated,
        2 * WRITE_ACTIONS.len() * ALL_KINDS.len() * M::ALL.len() * ALL_DOMAINS.len() * 2
    );
}

#[test]
fn a06_native_actions_never_plain_allow_exhaustive() {
    let engine = engine();
    for actor in [operator(), system(), skill(), model()] {
        for action in NATIVE_ACTIONS {
            for kind in ALL_KINDS {
                for mode in M::ALL {
                    for domain in ALL_DOMAINS {
                        let eval = engine.evaluate(
                            &actor,
                            &op(action, kind, mode, domain),
                            &ctx(domain, Cl::Internal, false),
                        );
                        assert_ne!(
                            eval.kind(),
                            K::Allow,
                            "{actor:?} {action:?} {kind:?} {mode:?} {domain:?}"
                        );
                        if !actor.is_operator() {
                            assert_eq!(
                                eval.kind(),
                                K::Deny,
                                "{actor:?} {action:?} {kind:?} {mode:?} {domain:?}"
                            );
                        }
                    }
                }
            }
        }
        // Host-path writes are never plain Allow either.
        for action in [Ac::Create, Ac::Update, Ac::Delete] {
            for mode in M::ALL {
                let eval = engine.evaluate(
                    &actor,
                    &op(action, R::FilesystemPath, mode, D::System),
                    &ctx(D::System, Cl::Internal, false),
                );
                assert_ne!(
                    eval.kind(),
                    K::Allow,
                    "{actor:?} {action:?} FilesystemPath {mode:?}"
                );
            }
        }
    }
}

#[test]
fn a06_operator_only_capabilities_never_required_of_non_operators_exhaustive() {
    let engine = engine();
    let mut approval_rows = 0usize;
    for actor in [system(), skill(), model()] {
        for action in ALL_ACTIONS {
            for kind in ALL_KINDS {
                for mode in M::ALL {
                    for scope_domain in ALL_DOMAINS {
                        for resource_domain in ALL_DOMAINS {
                            for classification in [Cl::Internal, Cl::Secret] {
                                let eval = engine.evaluate(
                                    &actor,
                                    &op(action, kind, mode, scope_domain),
                                    &ctx(resource_domain, classification, false),
                                );
                                if let Some(cap) = eval.required_capability() {
                                    approval_rows += 1;
                                    assert!(
                                        !cap.operator_only(),
                                        "{actor:?} {action:?} {kind:?} {mode:?} requires operator-only {cap:?}"
                                    );
                                    assert_eq!(
                                        cap,
                                        Capability::CrossDomainRead,
                                        "the only non-operator approval is CrossDomainRead"
                                    );
                                    assert_eq!(action, Ac::Read);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        approval_rows > 0,
        "cross-domain reads must surface as ApprovalRequired"
    );
}

#[test]
fn a06_secret_content_never_reaches_skill_or_model_exhaustive() {
    let engine = engine();
    for actor in [skill(), model()] {
        for kind in ALL_KINDS {
            for mode in M::ALL {
                for scope_domain in ALL_DOMAINS {
                    for resource_domain in ALL_DOMAINS {
                        let eval = engine.evaluate(
                            &actor,
                            &op(Ac::Read, kind, mode, scope_domain),
                            &ctx(resource_domain, Cl::Secret, false),
                        );
                        assert_eq!(
                            eval.kind(),
                            K::Deny,
                            "{actor:?} Read {kind:?} {mode:?} secret"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn a06_finalized_records_never_mutated_exhaustive() {
    let engine = engine();
    for actor in [operator(), system(), skill(), model()] {
        for action in [Ac::Update, Ac::Delete, Ac::Finalize] {
            for kind in ALL_KINDS {
                for mode in M::ALL {
                    for domain in ALL_DOMAINS {
                        for classification in ALL_CLASSIFICATIONS {
                            let eval = engine.evaluate(
                                &actor,
                                &op(action, kind, mode, domain),
                                &ctx(domain, classification, true),
                            );
                            assert_eq!(
                                eval.kind(),
                                K::Deny,
                                "{actor:?} {action:?} {kind:?} finalized"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a06_self_modification_only_in_maintenance_with_approval_exhaustive() {
    let engine = engine();
    for actor in [operator(), system(), skill(), model()] {
        for action in g9_policy::rules::MUTATING_ACTIONS {
            for kind in g9_policy::rules::SELF_MODIFICATION_RESOURCES {
                for mode in M::ALL {
                    for domain in ALL_DOMAINS {
                        let eval = engine.evaluate(
                            &actor,
                            &op(action, kind, mode, domain),
                            &ctx(domain, Cl::Internal, false),
                        );
                        if mode != M::Maintenance || !actor.is_operator() {
                            assert_eq!(
                                eval.kind(),
                                K::Deny,
                                "{actor:?} {action:?} {kind:?} {mode:?}"
                            );
                        } else {
                            assert_ne!(
                                eval.kind(),
                                K::Allow,
                                "{actor:?} {action:?} {kind:?} {mode:?}"
                            );
                            if eval.kind() == K::ApprovalRequired {
                                assert_eq!(
                                    eval.required_capability(),
                                    Some(Capability::PolicyModify)
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn a06_exam_mode_blocks_model_teaching_proposals_and_sealed_keys() {
    let engine = engine();
    for actor in [skill(), model()] {
        for kind in g9_policy::rules::TEACHING_RESOURCES {
            let eval = engine.evaluate(
                &actor,
                &op(Ac::Propose, kind, M::Exam, D::Learning),
                &ctx(D::Learning, Cl::Internal, false),
            );
            assert_eq!(eval.kind(), K::Deny);
            assert_eq!(eval.rule_id(), "exam.sm.propose.teaching.deny");
            // The same proposal outside EXAM is allowed.
            let eval = engine.evaluate(
                &actor,
                &op(Ac::Propose, kind, M::Study, D::Learning),
                &ctx(D::Learning, Cl::Internal, false),
            );
            assert_eq!(eval.kind(), K::Allow);
        }
        let eval = engine.evaluate(
            &actor,
            &op(Ac::Read, R::AssessmentDefinition, M::Exam, D::Learning),
            &ctx(D::Learning, Cl::Secret, false),
        );
        assert_eq!(eval.rule_id(), "exam.sm.read.sealed.deny");
    }
}

#[test]
fn a06_decision_records_are_metadata_only() {
    let engine = engine();
    for r in matrix() {
        let eval = evaluate(&engine, &r);
        let json = serde_json::to_value(&eval.record).unwrap();
        let obj = json.as_object().unwrap();
        for forbidden in [
            "parameters",
            "payload",
            "content",
            "prompt",
            "secret",
            "key",
            "mac",
        ] {
            assert!(
                !obj.contains_key(forbidden),
                "{forbidden} leaked into decision record"
            );
        }
        assert_eq!(obj["parameters_sha256"].as_str().unwrap().len(), 64);
        assert!(obj["reason"].as_str().unwrap().len() < 200);
    }
}
