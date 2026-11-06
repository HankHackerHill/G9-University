//! M12 Adaptive Teacher (`crates/g9-app::teaching`). PROPOSED (Stage 05/06):
//! lesson, hint and intervention *proposals* produced by T/L skills and
//! persisted as `PROPOSED` records for operator review. Until then every
//! operation answers `PRECONDITION_FAILED teaching.not_implemented`; teaching
//! output that exists today flows only through `skills.invoke` (advisory).

/// Operation names reserved for this area (none is routed to a handler yet).
pub const PROPOSED_OPERATIONS: &[&str] = &[
    "teaching.propose_lesson",
    "teaching.propose_hint",
    "teaching.propose_intervention",
    "teaching.review_proposal",
];
