//! M15 Research (`crates/g9-app::research`). PROPOSED (Stage 09): approved
//! research sources over `g9_evidence::sources` (`SourceState` machine).
//! Network egress is off by default and operator-only with a fresh approval;
//! no fetch exists in the mentor runtime. Every operation answers
//! `PRECONDITION_FAILED research.not_implemented`.

/// Operation names reserved for this area.
pub const PROPOSED_OPERATIONS: &[&str] = &[
    "research.source_register",
    "research.source_evaluate",
    "research.source_approve",
    "research.source_fetch",
];
