//! M13 Analytics (`crates/g9-app::analytics`). PROPOSED (Stage 07): reproducible
//! read models over storage (KPI definitions in
//! `docs/evaluations/kpi-definitions.md`). No metric is computed or reported
//! until its query exists and is tested; every operation answers
//! `PRECONDITION_FAILED analytics.not_implemented`.

/// Operation names reserved for this area.
pub const PROPOSED_OPERATIONS: &[&str] = &[
    "analytics.kpi_report",
    "analytics.recall_probe_report",
    "analytics.calibration_report",
];
