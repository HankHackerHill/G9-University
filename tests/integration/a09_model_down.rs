//! Gate A09 (model-down operation): with `NullRuntime` every deterministic
//! command works through `g9_app::dispatch` and skills fail closed.
//!
//! Registered as `[[test]] a09_model_down` of `crates/g9-evaluation`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use g9_evaluation::{scenarios, ScenarioReport};
use serde_json::json;

fn passed(r: &ScenarioReport) {
    assert!(r.passed, "{}", r.to_markdown());
}

#[test]
fn a09_deterministic_commands_work_and_skills_fail_closed_with_the_model_down() {
    let r = scenarios::a09_model_down().unwrap();
    passed(&r);
    for s in r.steps.iter().filter(|s| s.op != "skills.invoke") {
        assert_eq!(s.outcome_category, "OK", "{} with the model down", s.op);
    }
    let invoke = r.steps_for("skills.invoke");
    assert_eq!(invoke.len(), 1);
    assert_eq!(invoke[0].decision, "ALLOW");
    assert_eq!(invoke[0].outcome_category, "RESOURCE_UNAVAILABLE");
    let invocations = invoke[0].state_after["invocations"].as_array().unwrap();
    assert_eq!(invocations.len(), 1);
    assert_eq!(invocations[0]["state"], json!("ERROR"));
    assert_eq!(invocations[0]["actor_id"], json!("operator-1"));
    assert_eq!(invocations[0]["has_output_hash"], json!(false));
    assert!(r
        .evidence_for("status reports the model unavailable")
        .is_some_and(|c| c.passed));
    assert!(r
        .evidence_for("competency state unchanged by the model being down")
        .is_some_and(|c| c.passed));
}

#[test]
fn a09_harness_model_down_option_builds_the_null_runtime() {
    let h =
        g9_evaluation::Harness::with_options(g9_evaluation::HarnessOptions::model_down()).unwrap();
    assert!(h.scripted().is_none());
    assert!(h.script_reply(&json!({})).is_err());
}
