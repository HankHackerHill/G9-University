//! Gate A01 (ARCHITECTURE.md §14): curriculum invariants and provenance,
//! driven end to end through `g9_app::dispatch` by `g9-evaluation`.
//!
//! Registered as `[[test]] a01_gate` of `crates/g9-evaluation`; run with
//! `cargo test -p g9-evaluation --test a01_gate`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use g9_evaluation::{scenarios, ScenarioReport, ScenarioStep};
use serde_json::json;

fn passed(r: &ScenarioReport) {
    assert!(r.passed, "{}", r.to_markdown());
}

fn decision_and_outcome(s: &ScenarioStep) -> (&str, &str, Option<&str>) {
    (
        s.decision.as_str(),
        s.outcome_category.as_str(),
        s.error_code.as_deref(),
    )
}

#[test]
fn a01_curriculum_invariants_and_provenance_end_to_end() {
    let r = scenarios::a01_curriculum_import_gate().unwrap();
    passed(&r);
    let imports = r.steps_for("curriculum.import");
    assert_eq!(imports.len(), 5, "{}", r.to_markdown());

    // An 11-week manifest: refused by the invariant check, nothing stored.
    assert_eq!(
        decision_and_outcome(imports[0]),
        (
            "NOT_EVALUATED",
            "INVALID_INPUT",
            Some("curriculum.invariant_violated")
        )
    );
    assert_eq!(imports[0].state_after["versions"], json!([]));
    assert_eq!(imports[0].state_after["approvals"], json!({}));

    // A payload claiming APPROVED_CANONICAL never reaches the pipeline.
    assert_eq!(
        decision_and_outcome(imports[1]),
        ("NO_AUDIT_ROW", "INVALID_INPUT", Some("ipc.payload_invalid"))
    );

    // The seed without approval: APPROVAL_REQUIRED, one PENDING request, nothing stored.
    assert_eq!(
        decision_and_outcome(imports[2]),
        (
            "APPROVAL_REQUIRED",
            "APPROVAL_REQUIRED",
            Some("policy.approval_required")
        )
    );
    assert_eq!(imports[2].state_after["versions"], json!([]));
    assert_eq!(imports[2].state_after["approvals"], json!({ "PENDING": 1 }));

    // With the operator's approval: stored as PROPOSED_SEED, not current, approval consumed.
    assert_eq!(decision_and_outcome(imports[3]), ("ALLOW", "OK", None));
    let versions = imports[3].state_after["versions"].as_array().unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0]["provenance"], json!("PROPOSED_SEED"));
    assert_eq!(versions[0]["is_current"], json!(false));
    assert_eq!(
        imports[3].state_after["approvals"],
        json!({ "CONSUMED": 1 })
    );

    // Re-import with the spent approval: refused; still exactly one version.
    assert_eq!(
        decision_and_outcome(imports[4]),
        (
            "NOT_EVALUATED",
            "CONFLICT",
            Some("curriculum.version_exists")
        )
    );
    assert_eq!(
        imports[4].state_after["versions"].as_array().map(Vec::len),
        Some(1)
    );
}
