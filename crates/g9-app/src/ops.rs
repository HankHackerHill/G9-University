//! Operation registry: one entry per IPC operation in `schemas/ipc`, mirroring
//! `apps/desktop/src/ipc/operations.ts` (`OPERATIONS`). The registry fixes, per
//! operation, the payload schema version the handler accepts (§5 step 1),
//! whether the command mutates (read-only queries skip §5 steps 4–9) and the
//! policy verb/resource kind the backend evaluates. The UI's copy is advisory;
//! this table is the one the backend enforces.

use g9_domain::{Action, ResourceKind, SchemaVersion};

/// Static description of one IPC operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpSpec {
    /// Wire name, e.g. `session.start`.
    pub name: &'static str,
    /// Payload schema version the handler reads (major must match, payload minor ≤ this minor).
    pub schema_version: SchemaVersion,
    /// `false` for read-only queries.
    pub mutating: bool,
    /// Policy verb.
    pub action: Action,
    /// Policy resource kind.
    pub resource_kind: ResourceKind,
}

const fn op(
    name: &'static str,
    mutating: bool,
    action: Action,
    resource_kind: ResourceKind,
) -> OpSpec {
    OpSpec {
        name,
        schema_version: SchemaVersion::V1,
        mutating,
        action,
        resource_kind,
    }
}

/// Every routed IPC operation (35: the 34 UI operations plus `curriculum.promote`).
pub const OPERATIONS: &[OpSpec] = &[
    op("system.status", false, Action::Read, ResourceKind::AppSetting),
    op(
        "curriculum.validate",
        false,
        Action::Read,
        ResourceKind::Curriculum,
    ),
    op(
        "curriculum.import",
        true,
        Action::Import,
        ResourceKind::CurriculumVersion,
    ),
    op(
        "curriculum.promote",
        true,
        Action::PromoteCanonical,
        ResourceKind::CurriculumVersion,
    ),
    op(
        "curriculum.daily_plan",
        false,
        Action::Read,
        ResourceKind::CurriculumVersion,
    ),
    op(
        "calendar.list",
        false,
        Action::Read,
        ResourceKind::CalendarEvent,
    ),
    op(
        "calendar.propose",
        true,
        Action::Propose,
        ResourceKind::ScheduleProposal,
    ),
    op(
        "calendar.apply",
        true,
        Action::Apply,
        ResourceKind::ScheduleProposal,
    ),
    op(
        "session.list",
        false,
        Action::Read,
        ResourceKind::StudySession,
    ),
    op(
        "session.prepare",
        true,
        Action::Update,
        ResourceKind::StudySession,
    ),
    op(
        "session.start",
        true,
        Action::Update,
        ResourceKind::StudySession,
    ),
    op(
        "session.pause",
        true,
        Action::Update,
        ResourceKind::StudySession,
    ),
    op(
        "session.resume",
        true,
        Action::Update,
        ResourceKind::StudySession,
    ),
    op(
        "session.complete",
        true,
        Action::Update,
        ResourceKind::StudySession,
    ),
    op(
        "session.correct_time",
        true,
        Action::Correct,
        ResourceKind::TimeInterval,
    ),
    op("notes.create", true, Action::Create, ResourceKind::Note),
    op("notes.revise", true, Action::Update, ResourceKind::Note),
    op("assessment.list", false, Action::Read, ResourceKind::Attempt),
    op("assessment.begin", true, Action::Update, ResourceKind::Attempt),
    op("assessment.submit", true, Action::Update, ResourceKind::Attempt),
    op(
        "assessment.review_evidence",
        true,
        Action::Update,
        ResourceKind::Attempt,
    ),
    op(
        "assessment.grade_provisional",
        true,
        Action::Update,
        ResourceKind::GradeDecision,
    ),
    op(
        "assessment.approve_final",
        true,
        Action::Finalize,
        ResourceKind::GradeDecision,
    ),
    op(
        "competency.projection",
        false,
        Action::Read,
        ResourceKind::Competency,
    ),
    op("retention.due", false, Action::Read, ResourceKind::ReviewItem),
    op(
        "interventions.list",
        false,
        Action::Read,
        ResourceKind::Intervention,
    ),
    op("approvals.list", false, Action::Read, ResourceKind::Approval),
    op("approvals.decide", true, Action::Approve, ResourceKind::Approval),
    op(
        "audit.recent",
        false,
        Action::Read,
        ResourceKind::AuditMetadata,
    ),
    op("backup.create", true, Action::Create, ResourceKind::Backup),
    op("backup.verify", true, Action::Update, ResourceKind::Backup),
    op("backup.restore_test", true, Action::Update, ResourceKind::Backup),
    op("skills.invoke", true, Action::Invoke, ResourceKind::Skill),
    op("export.request", true, Action::Export, ResourceKind::Export),
    op("deletion.request", true, Action::Delete, ResourceKind::Note),
];

/// Look up an operation by wire name.
pub fn spec(name: &str) -> Option<&'static OpSpec> {
    OPERATIONS.iter().find(|s| s.name == name)
}

/// Operation name for an approval's bound `(action, resource kind)`; used to
/// label approval requests for the UI (the `approval_requests` table stores the
/// descriptor, not the wire name). Every approval-gated operation has a unique pair.
pub fn name_for_approval(action: Action, kind: ResourceKind) -> &'static str {
    match (action, kind) {
        (Action::Import, ResourceKind::CurriculumVersion) => "curriculum.import",
        (Action::PromoteCanonical, ResourceKind::CurriculumVersion) => "curriculum.promote",
        (Action::Apply, ResourceKind::ScheduleProposal) => "calendar.apply",
        (Action::Finalize, ResourceKind::GradeDecision) => "assessment.approve_final",
        (Action::Export, _) => "export.request",
        (Action::Delete, _) => "deletion.request",
        _ => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn names_are_unique_and_match_schema_files() {
        let names: BTreeSet<&str> = OPERATIONS.iter().map(|s| s.name).collect();
        assert_eq!(names.len(), OPERATIONS.len());
        assert_eq!(OPERATIONS.len(), 35);
        for s in OPERATIONS {
            assert!(
                crate::schemas::request_schema_text(s.name).is_some(),
                "{}",
                s.name
            );
            assert!(
                crate::schemas::response_schema_text(s.name).is_some(),
                "{}",
                s.name
            );
        }
    }
}
