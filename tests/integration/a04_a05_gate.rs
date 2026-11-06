//! Gates A04 (finalized grade immutability) and A05 (competency replay and
//! correction), driven through `g9_app::dispatch` over the week-3 fixture.
//!
//! Registered as `[[test]] a04_a05_gate` of `crates/g9-evaluation`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use g9_evaluation::{scenarios, ScenarioReport};
use serde_json::json;

fn passed(r: &ScenarioReport) {
    assert!(r.passed, "{}", r.to_markdown());
}

#[test]
fn a04_finalized_grade_cannot_be_regraded_refinalized_reopened_or_deleted() {
    let r = scenarios::a04_finalized_grade_immutable().unwrap();
    passed(&r);
    for op in [
        "assessment.grade_provisional",
        "assessment.approve_final",
        "assessment.review_evidence",
        "deletion.request",
    ] {
        let steps = r.steps_for(op);
        assert_eq!(steps.len(), 1, "{op}");
        let s = steps[0];
        assert_eq!(
            (
                s.decision.as_str(),
                s.outcome_category.as_str(),
                s.error_code.as_deref()
            ),
            ("DENY", "UNAUTHORIZED", Some("policy.denied")),
            "{op}"
        );
        assert_eq!(s.state_after["state"], json!("APPROVED_FINAL"), "{op}");
        assert_eq!(s.state_after["final_decisions"], json!(1), "{op}");
        assert_eq!(s.state_after["grade_decisions"], json!(1), "{op}");
    }
}

#[test]
fn a04_model_advisory_grade_never_finalizes() {
    let r = scenarios::a04_model_advisory_cannot_finalize().unwrap();
    passed(&r);
    let graded = r.steps_for("assessment.grade_provisional");
    assert_eq!(graded[0].decision, "ALLOW");
    assert_eq!(graded[0].state_after["state"], json!("GRADED_PROVISIONAL"));
    let fin = r.steps_for("assessment.approve_final");
    assert_eq!(fin.len(), 1);
    assert_eq!(fin[0].outcome_category, "PRECONDITION_FAILED");
    assert_eq!(fin[0].decision, "NOT_EVALUATED");
    assert_eq!(fin[0].state_after["state"], json!("GRADED_PROVISIONAL"));
    assert_eq!(fin[0].state_after["final_decisions"], json!(0));
}

#[test]
fn a05_projection_replays_correction_supersedes_and_model_evidence_is_refused() {
    let r = scenarios::a05_competency_replay_and_correction().unwrap();
    passed(&r);
    let replay = r
        .steps
        .iter()
        .find(|s| s.op.starts_with("harness:replay derive_state"))
        .unwrap();
    assert_eq!(
        replay.state_after["forward"],
        replay.state_after["backward"]
    );

    let correction = r
        .steps
        .iter()
        .find(|s| s.op.contains("MANUAL_CORRECTION"))
        .unwrap();
    assert_eq!(correction.decision, "ACCEPTED");
    assert_eq!(
        correction.state_after["observations"]
            .as_array()
            .map(Vec::len),
        Some(4),
        "the superseded observation is retained"
    );

    let forged = r
        .steps
        .iter()
        .find(|s| s.op.contains("MODEL_OUTPUT provenance"))
        .unwrap();
    assert_eq!(
        (forged.decision.as_str(), forged.outcome_category.as_str()),
        ("REFUSED", "INVALID_INPUT")
    );
    assert_eq!(
        forged.state_after["observations"], correction.state_after["observations"],
        "the refused observation was not stored"
    );
    assert!(r
        .evidence_for("projection follows the correction")
        .is_some_and(|c| c.passed && c.actual == json!("GUIDED")));
}
