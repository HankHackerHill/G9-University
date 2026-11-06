#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
// `G9Error` is the closed error contract returned by value through g9-app;
// boxing it here would diverge from the bridge contract.
#![allow(clippy::result_large_err)]
//! # g9-evaluation — evaluation harness (skills E12 mentor evaluation / E13 adversarial assurance)
//!
//! Offline, deterministic gate and adversarial scenarios that drive the real
//! application through [`g9_app::dispatch`] and assert both the decision the
//! backend recorded and the final backend state (handoff §3 Audit B).
//!
//! | Module | Contract |
//! |---|---|
//! | [`harness`] | [`Harness`]: `AppContext` over in-memory SQLite, fixed clock, fixed binding key, [`g9_ai_adapter::ScriptedRuntime`] (or `NullRuntime`), the real `.agents/skills` registry when present |
//! | [`fixtures`] | `data/fixtures/*.json` loader (schema-validated before deserialization) and the learner-history seeder |
//! | [`report`] | [`ScenarioReport`] (steps with recorded decision, outcome and state; evidence checks; computed `passed`) and the Markdown renderers |
//! | [`scenarios`] | gate scenarios A01, A04, A05, A06, A09, A10, A11 |
//! | [`adversarial`] | threat-model catalogue ST01–ST12 (`docs/threat-model/threat-model.md` §6) |
//! | [`recovery`] | A08 drill: populated DB → backup → verify → restore test → crash mid-transaction → reopen → recovery → audit chain |
//!
//! ## Honesty rules
//!
//! * `passed` is computed from evidence checks; a scenario without checks fails.
//! * Steps that cannot go through `dispatch` (no IPC operation exists) are
//!   labelled with their boundary (`registry:`, `storage:`, `orchestrator:`,
//!   `harness:`) and the gap is written into the report notes.
//! * No model is run; replies are scripted. No network, no wall clock, no randomness
//!   that affects an assertion (generated UUIDv7 ids are never asserted on).
//!
//! ## Gaps in wave-1 APIs found while building the harness
//!
//! See [`WAVE1_GAPS`]; each is also attached as a note to the scenario that hit it.

pub mod adversarial;
pub mod fixtures;
pub mod harness;
pub mod recovery;
pub mod report;
pub mod scenarios;

pub use harness::{
    category_name, default_scope, CallOpts, Harness, HarnessOptions, RegistryStatus,
    RuntimeChoice, StepOutcome, T0,
};
pub use report::{render_gate_evidence, EvidenceCheck, ScenarioReport, ScenarioStep};

/// Wave-1 API gaps observed by the harness (reported, worked around or scoped out).
pub const WAVE1_GAPS: &[&str] = &[
    "g9-app notes.create does not run g9_evidence::classify_text nor persist content_classifications: an instruction-like note is stored as data (correct) but its InstructionLike flag is not recorded (ST01).",
    "No IPC cancellation operation: skills.invoke always runs with a fresh CancelToken, so late-response discard (ST12) is exercised at the orchestrator boundary with g9-app's own PolicyAdapter/RuntimeBackend.",
    "No IPC path creates study sessions, competencies, assessment definitions or observations (FILL_CURRICULUM and catalogue import are PROPOSED): fixtures insert them through the g9-storage repositories.",
    "g9-app RuntimeBackend maps a structured-output schema failure (InferenceError::InvalidOutput) to BackendError::Runtime, so a schema-invalid model reply surfaces as INTERNAL skills.backend.* instead of PRECONDITION_FAILED skills.output_rejected (A11).",
    "No model-load IPC operation: modified model weights (ST06) are checked at the g9_ai_adapter::verify_checksum boundary only; dependency bans are CI (cargo deny) and NOT RUN here.",
    "g9-app backup.verify/restore_test only know backups created by the same process (no manifest reader in g9-storage).",
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Run every scenario, render `docs/evaluations/gate-evidence.md`, and write
    /// it only when `G9_WRITE_EVIDENCE=1`.
    #[test]
    fn gate_evidence_document() {
        let mut reports = scenarios::run_all().unwrap();
        reports.extend(adversarial::run_all().unwrap());
        reports.push(recovery::recovery_drill().unwrap());
        let doc = render_gate_evidence(&reports);
        for r in &reports {
            assert!(r.passed, "{}", r.to_markdown());
        }
        assert!(doc.contains("| A08 |"));
        if std::env::var("G9_WRITE_EVIDENCE").as_deref() == Ok("1") {
            let path = harness::repo_root()
                .join("docs")
                .join("evaluations")
                .join("gate-evidence.md");
            std::fs::write(&path, doc).unwrap();
        }
    }
}
