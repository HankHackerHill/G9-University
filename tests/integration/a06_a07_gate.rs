//! Gates A06 (policy denies unapproved actions) and A07 (a malicious note or
//! skill cannot grant privilege), driven through `g9_app::dispatch`.
//!
//! The A07 claims reuse the threat-model scenarios ST01 (note with embedded
//! instructions), ST02 (tampered skill package) and ST04 (skill output asking
//! for EXECUTE/CONNECT); the full ST01–ST12 catalogue runs in
//! `tests/adversarial/st_adversarial_catalogue.rs`.
//!
//! Registered as `[[test]] a06_a07_gate` of `crates/g9-evaluation`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use g9_evaluation::{adversarial, scenarios, ScenarioReport};
use serde_json::json;

fn passed(r: &ScenarioReport) {
    assert!(r.passed, "{}", r.to_markdown());
}

#[test]
fn a06_every_gated_operation_requires_a_bound_approval_and_mutates_nothing() {
    let r = scenarios::a06_gated_operations_require_approval().unwrap();
    passed(&r);
    let gated = [
        "export.request",
        "deletion.request",
        "assessment.approve_final",
        "curriculum.import",
        "calendar.apply",
    ];
    for (i, op) in gated.iter().enumerate() {
        let s = r.steps_for(op)[0];
        assert_eq!(s.decision, "APPROVAL_REQUIRED", "{op}");
        assert_eq!(s.outcome_category, "APPROVAL_REQUIRED", "{op}");
        // One more PENDING request per refused command; the fixture's one
        // CONSUMED approval is untouched.
        assert_eq!(s.state_after["PENDING"], json!(i + 1), "{op}");
        assert_eq!(s.state_after["CONSUMED"], json!(1), "{op}");
    }
    for check in [
        "note not erased",
        "attempt still GRADED_PROVISIONAL",
        "no curriculum version stored",
        "no calendar event created",
        "no domain event appended by refused commands",
    ] {
        assert!(
            r.evidence_for(check).is_some_and(|c| c.passed),
            "{check}\n{}",
            r.to_markdown()
        );
    }
}

#[test]
fn a06_ui_actor_is_refused_and_approvals_bind_to_parameters() {
    let r = scenarios::a06_identity_binding_and_evidentiary_denial().unwrap();
    passed(&r);
    let creates = r.steps_for("notes.create");
    assert_eq!(creates.len(), 3);
    let rows_before = &creates[0].state_after["rows"];
    for s in creates {
        assert_eq!(s.error_code.as_deref(), Some("meta.actor_not_accepted"));
        assert_eq!(s.decision, "NO_AUDIT_ROW");
        assert_eq!(&s.state_after["rows"], rows_before, "nothing persisted");
    }
    let exports = r.steps_for("export.request");
    assert_eq!(exports[0].decision, "DENY");
    // The ceiling is part of the export resource id, so an approval for the
    // INTERNAL selection presented for PRIVATE is another resource.
    assert_eq!(
        exports[0].error_code.as_deref(),
        Some("approval.resource_mismatch")
    );
    assert_eq!(exports[0].state_after["state"], json!("APPROVED"));
    assert_eq!(exports[1].decision, "ALLOW");
    assert_eq!(exports[1].state_after["state"], json!("CONSUMED"));
    let delete = r.steps_for("deletion.request")[0];
    assert_eq!(delete.decision, "DENY");
    assert_eq!(delete.state_after["exists"], json!(true));
}

#[test]
fn a06_pipeline_order_idempotency_before_policy_and_revision_check() {
    let r = scenarios::a06_pipeline_order_idempotency_and_revision().unwrap();
    passed(&r);
    let creates = r.steps_for("notes.create");
    assert_eq!(creates.len(), 3);
    assert_eq!(creates[1].outcome_category, "OK");
    assert_eq!(creates[1].state_after["note_created_events"], json!(1));
    assert_eq!(
        creates[2].error_code.as_deref(),
        Some("idempotency.key_reused")
    );
    assert_eq!(creates[2].state_after["note_created_events"], json!(1));
    let exports = r.steps_for("export.request");
    assert_eq!(exports[1].outcome_category, "OK", "replay, not a DENY");
    assert_eq!(exports[1].state_after["exports"]["export_events"], json!(1));
    assert_eq!(
        exports[1].state_after["approval"]["state"],
        json!("CONSUMED")
    );
    let revises = r.steps_for("notes.revise");
    assert_eq!(revises[0].outcome_category, "STALE_REVISION");
    assert_eq!(revises[0].state_after["revision"], json!(1));
    assert_eq!(revises[1].outcome_category, "OK");
    assert_eq!(revises[1].state_after["revision"], json!(2));
}

#[test]
fn a07_note_with_embedded_instructions_grants_nothing() {
    let r = adversarial::st01_malicious_note().unwrap();
    passed(&r);
    let stored = r.steps_for("notes.create")[0];
    assert_eq!(stored.decision, "ALLOW");
    assert_eq!(stored.state_after["approvals"], json!({}));
    let invoke = r.steps_for("skills.invoke")[0];
    assert_eq!(invoke.error_code.as_deref(), Some("skills.output_rejected"));
    assert_eq!(invoke.state_after["calendar_events"], json!(0));
    assert_eq!(
        invoke.state_after["invocations"][0]["state"],
        json!("ERROR")
    );
}

#[test]
fn a07_tampered_skill_package_is_refused_and_cannot_be_invoked() {
    let r = adversarial::st02_tampered_skill_package().unwrap();
    passed(&r);
    let refused: Vec<_> = r
        .steps
        .iter()
        .filter(|s| s.op.starts_with("registry:") && s.decision == "REFUSED")
        .collect();
    assert_eq!(refused.len(), 2, "{}", r.to_markdown());
    assert_eq!(
        refused[0].error_code.as_deref(),
        Some("skills.registry.invalid_packages")
    );
    assert_eq!(
        refused[1].error_code.as_deref(),
        Some("skills.registry.hash_mismatch")
    );
    let invoke = r.steps_for("skills.invoke")[0];
    assert_eq!(invoke.outcome_category, "RESOURCE_UNAVAILABLE");
    assert_eq!(invoke.state_after["invocations"], json!([]));
}

#[test]
fn a07_skill_output_requesting_execute_or_connect_is_rejected() {
    let r = adversarial::st04_skill_output_requests_execute().unwrap();
    passed(&r);
    for s in r.steps_for("skills.invoke") {
        assert_eq!(s.decision, "ALLOW");
        assert_eq!(s.error_code.as_deref(), Some("skills.output_rejected"));
        assert_eq!(s.state_after["approvals"], json!({}));
        assert!(s.state_after["invocations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["state"] == json!("ERROR") && i["has_output_hash"] == json!(false)));
    }
}
