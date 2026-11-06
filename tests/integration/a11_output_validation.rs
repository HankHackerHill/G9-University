//! Gate A11 (output validation): skill output is schema-validated and only
//! ever advisory; invalid input is refused before the model; malformed IPC
//! payloads are refused before deserialization.
//!
//! Registered as `[[test]] a11_output_validation` of `crates/g9-evaluation`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use g9_evaluation::{scenarios, ScenarioReport};
use serde_json::json;

fn passed(r: &ScenarioReport) {
    assert!(r.passed, "{}", r.to_markdown());
}

#[test]
fn a11_valid_output_is_advisory_and_invalid_output_is_rejected() {
    let r = scenarios::a11_output_validation().unwrap();
    passed(&r);
    let invokes = r.steps_for("skills.invoke");
    assert_eq!(invokes.len(), 4);
    for s in &invokes {
        assert_eq!(s.decision, "ALLOW", "policy allowed the invocation itself");
        assert_eq!(s.state_after["approvals"], json!({}), "no approval created");
    }
    // Valid reply: completed, advisory only.
    assert_eq!(invokes[0].outcome_category, "OK");
    let count = |s: &g9_evaluation::ScenarioStep, key: &str, value: serde_json::Value| {
        s.state_after["invocations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| i[key] == value)
            .count()
    };
    // Schema-invalid and non-JSON replies: never completed, no output hash;
    // each adds one ERROR row and the only output hash stays the valid one.
    for (n, s) in invokes[1..3].iter().enumerate() {
        assert_ne!(s.outcome_category, "OK");
        assert_eq!(count(s, "state", json!("ERROR")), n + 1);
        assert_eq!(count(s, "has_output_hash", json!(true)), 1);
    }
    // Input violating the package's input schema: refused before the model.
    assert_eq!(
        invokes[3].error_code.as_deref(),
        Some("skills.preflight_failed")
    );
    let completed: Vec<_> = invokes[3].state_after["invocations"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["state"] == json!("COMPLETED"))
        .collect();
    assert_eq!(completed.len(), 1, "only the valid reply completed");
    // IPC payload violating its request schema: nothing persisted.
    let bad = r.steps_for("notes.create")[0];
    assert_eq!(bad.error_code.as_deref(), Some("ipc.payload_invalid"));
    assert_eq!(bad.decision, "NO_AUDIT_ROW");
}

#[test]
fn a11_budget_and_cancellation_boundaries() {
    let st08 = g9_evaluation::adversarial::st08_budget_refusals().unwrap();
    passed(&st08);
    let codes: Vec<_> = st08
        .steps_for("skills.invoke")
        .iter()
        .map(|s| s.error_code.clone())
        .collect();
    assert_eq!(
        codes,
        vec![
            Some("skills.backend.budget".to_string()),
            Some("skills.output_rejected".to_string())
        ]
    );
    let st12 = g9_evaluation::adversarial::st12_cancellation_discards_late_response().unwrap();
    passed(&st12);
    let cancelled = st12
        .steps
        .iter()
        .find(|s| s.op.contains("cancelled while the model was generating"))
        .unwrap();
    assert_eq!(cancelled.state_after["state"], json!("CANCELLED"));
    assert_eq!(cancelled.state_after["has_output"], json!(false));
}
