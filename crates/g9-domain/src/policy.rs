//! Policy vocabulary (handoff §12, M16). The engine lives in `g9-policy`; the
//! types live here so that application services, skills and storage share one
//! definition of actions, resources, capabilities and decisions.

use crate::envelope::{ResourceRef, Scope};
use serde::{Deserialize, Serialize};

/// Kinds of protected resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceKind {
    Curriculum,
    CurriculumVersion,
    CalendarEvent,
    ScheduleProposal,
    StudySession,
    TimeInterval,
    Note,
    Attachment,
    EvidenceItem,
    AssessmentDefinition,
    Attempt,
    GradeDecision,
    Competency,
    Observation,
    LearnerHypothesis,
    ReviewItem,
    Intervention,
    Experiment,
    Goal,
    Project,
    Milestone,
    Decision,
    ResearchSource,
    KnowledgeDocument,
    Model,
    Skill,
    SkillInvocation,
    Approval,
    Backup,
    Policy,
    Export,
    FilesystemPath,
    NetworkHost,
    Process,
    AppSetting,
    AuditMetadata,
}

/// Generic verbs. Specific semantics come from the (action, resource kind) pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Action {
    Read,
    Create,
    Update,
    Delete,
    Export,
    Import,
    /// Promote a proposed/reconciled curriculum version to approved canonical.
    PromoteCanonical,
    /// Produce a proposal (schedule, intervention, correction). Never applies state.
    Propose,
    Approve,
    /// Apply an approved proposal to authoritative state.
    Apply,
    /// Finalize an official record (grade decision, assessment attempt).
    Finalize,
    /// Invoke a skill / inference.
    Invoke,
    /// Execute a native process or shell command.
    Execute,
    /// Open a network connection.
    Connect,
    /// Load a model artifact into the runtime.
    Load,
    /// Correct a derived state by adding a superseding record (never in-place mutation).
    Correct,
}

/// Capabilities that an approval may grant. Deny-by-default: absence means denied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Capability {
    ProtectedRecordWrite,
    GradeFinalize,
    ScheduleApply,
    CurriculumImport,
    CurriculumPromote,
    SkillInvoke,
    ModelInference,
    FilesystemRead,
    FilesystemWrite,
    NetworkEgress,
    ShellExec,
    DataExport,
    DataDelete,
    PolicyModify,
    ModelLoad,
    LabOrchestrate,
    CrossDomainRead,
}

impl Capability {
    /// Capabilities that can never be granted to a skill or model actor, even with
    /// an approval. Only the operator may hold them, and only per exact operation.
    pub const fn operator_only(self) -> bool {
        matches!(
            self,
            Capability::ShellExec
                | Capability::NetworkEgress
                | Capability::FilesystemWrite
                | Capability::PolicyModify
                | Capability::GradeFinalize
                | Capability::CurriculumPromote
                | Capability::DataDelete
                | Capability::ModelLoad
                | Capability::LabOrchestrate
        )
    }
}

/// Exact description of an operation for policy evaluation and approval binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationDescriptor {
    pub action: Action,
    pub resource: ResourceRef,
    pub scope: Scope,
    /// Hex SHA-256 of the canonical JSON of the full operation parameters.
    /// Approvals bind to this hash; a changed parameter invalidates the approval.
    pub parameters_sha256: String,
}

/// Outcome of a policy evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Decision {
    Allow {
        rule_id: String,
    },
    Deny {
        rule_id: String,
        reason: String,
    },
    ApprovalRequired {
        rule_id: String,
        reason: String,
        required_capability: Capability,
    },
}

impl Decision {
    pub fn is_allow(&self) -> bool {
        matches!(self, Decision::Allow { .. })
    }
}
