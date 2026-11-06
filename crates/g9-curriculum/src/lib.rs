#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! # g9-curriculum — M02 Curriculum (ARCHITECTURE.md §3, §4; handoff §8; gate A01)
//!
//! Pure, I/O-free curriculum logic over the manifest format v1.0.0:
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`manifest`] | serde types (`deny_unknown_fields`) for the manifest format; the seed at `data/curriculum/g9-ct9-480.seed.v0.json` is the reference instance |
//! | [`validate`](mod@validate) | every importer proof obligation (12 × 6 × 3 = 216 objectives, 420/300 minutes, 2,400/week, 28,800 total, ids, blocks, deliverables, resources, provenance); reports, never corrects |
//! | [`checksum`] | SHA-256 over canonical JSON (`g9_domain::hashing`) — the identity approvals bind to |
//! | [`reconcile`](mod@reconcile) | day-by-day diff of a candidate against the operator-supplied original approved plan, with a human-readable report and the explicit approval request |
//! | [`provenance`] | the promotion gate: `PROPOSED_SEED → RECONCILED_CANDIDATE → APPROVED_CANONICAL`; a seed is never canonical by itself (C01) |
//! | [`plan`] | read models: the plan for one day and chrono-free calendar offsets for the program |
//! | [`error`] | `CurriculumError` → `G9Error` (closed category set) |
//!
//! Integrity rules honoured here: imported manifests are data, not instructions;
//! validation blocks import on any invariant or provenance failure; promotion is
//! a command-level operator approval applied by `g9-app`, which this crate can
//! only precondition. There is no storage, network or filesystem access.

pub mod checksum;
pub mod error;
pub mod manifest;
pub mod plan;
pub mod provenance;
pub mod reconcile;
pub mod validate;

pub use checksum::{json_checksum, json_text_checksum, manifest_checksum, Checksum};
pub use error::CurriculumError;
pub use manifest::{
    Block, BlockSpec, Day, Manifest, Objective, Reconciliation, Resource, ResourceKind, Structure,
    Week, MANIFEST_FORMAT_VERSION,
};
pub use plan::{
    daily_plan, program_timeline, CivilDate, DailyPlan, PlannedBlock, PlannedObjective,
    ProgramTimeline, TimelineDay,
};
pub use provenance::{can_promote, check_transition, provenance_label, PromotionBlocked};
pub use reconcile::{
    reconcile, DayDiff, DayDiffStatus, ReconciliationBinding, ReconciliationReport,
    ReconciliationSummary,
};
pub use validate::{
    codes, validate, ManifestError, ManifestWarning, ValidationReport, SUPPORTED_SCHEMA_VERSION,
};

/// Parse manifest JSON text. Parse errors report position only, never content.
pub fn parse_manifest(text: &str) -> Result<Manifest, CurriculumError> {
    serde_json::from_str(text).map_err(|e| CurriculumError::Parse(checksum::parse_message(&e)))
}

/// Parse and validate in one step. Returns the manifest with its report when
/// valid; `Err(CurriculumError::Invalid)` carries the full report otherwise.
pub fn load_manifest(text: &str) -> Result<(Manifest, ValidationReport), CurriculumError> {
    let manifest = parse_manifest(text)?;
    let report = validate(&manifest).into_result()?;
    Ok((manifest, report))
}
