//! M14 Goals/Execution (`crates/g9-app::executive`). PROPOSED (Stage 08):
//! goals, projects, milestones and decisions in the `DataDomain::Business`
//! domain over `g9_storage::repos::business`. Every operation answers
//! `PRECONDITION_FAILED executive.not_implemented`.

/// Operation names reserved for this area.
pub const PROPOSED_OPERATIONS: &[&str] = &[
    "executive.goal_create",
    "executive.goal_list",
    "executive.decision_record",
    "executive.milestone_update",
];
