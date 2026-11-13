//! Threat-model adversarial catalogue ST01–ST12
//! (`docs/threat-model/threat-model.md` §6), each scenario run through the
//! `g9-evaluation` harness. Every test asserts the decision the backend
//! recorded and the backend state read back afterwards.
//!
//! Registered as `[[test]] st_adversarial_catalogue` of `crates/g9-evaluation`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use g9_evaluation::adversarial::CATALOGUE;
use g9_evaluation::{ScenarioReport, ScenarioStep};
use serde_json::json;

/// Run one scenario and check what every scenario must carry: a PASS computed
/// from evidence, a decision and an outcome on every step, and a decision
/// check for every dispatched step.
fn run(id: &str) -> ScenarioReport {
    let (_, scenario) = CATALOGUE
        .iter()
        .find(|(st, _)| *st == id)
        .unwrap_or_else(|| panic!("{id} is not in the catalogue"));
    let r = scenario().unwrap();
    assert_eq!(r.id, id);
    assert!(r.passed, "{}", r.to_markdown());
    assert!(!r.steps.is_empty(), "{id}: no steps");
    for s in &r.steps {
        assert!(
            !s.decision.is_empty() && !s.outcome_category.is_empty(),
            "{id}: {s:?}"
        );
    }
    let dispatched = r.steps.iter().filter(|s| s.request_id.is_some()).count();
    let decision_checks = r
        .evidence
        .iter()
        .filter(|c| c.check.ends_with(": recorded decision"))
        .count();
    if dispatched > 0 {
        assert!(decision_checks > 0, "{id}: no recorded-decision check");
    }
    r
}

fn step<'a>(r: &'a ScenarioReport, op_prefix: &str, nth: usize) -> &'a ScenarioStep {
    r.steps
        .iter()
        .filter(|s| s.op.starts_with(op_prefix))
        .nth(nth)
        .unwrap_or_else(|| panic!("{}: no step {op_prefix} #{nth}\n{}", r.id, r.to_markdown()))
}

fn verdict(s: &ScenarioStep) -> (&str, &str, Option<&str>) {
    (
        s.decision.as_str(),
        s.outcome_category.as_str(),
        s.error_code.as_deref(),
    )
}

#[test]
fn catalogue_covers_every_threat_model_scenario() {
    let doc = std::fs::read_to_string(
        g9_evaluation::harness::repo_root()
            .join("docs")
            .join("threat-model")
            .join("threat-model.md"),
    )
    .unwrap();
    let in_doc: Vec<String> = doc
        .lines()
        .filter_map(|l| l.strip_prefix("| ST"))
        .filter_map(|rest| rest.get(..2))
        .filter(|n| n.chars().all(|c| c.is_ascii_digit()))
        .map(|n| format!("ST{n}"))
        .collect();
    let in_catalogue: Vec<String> = CATALOGUE.iter().map(|(id, _)| (*id).to_string()).collect();
    assert_eq!(in_doc.len(), 12, "threat model §6 lists ST01–ST12");
    assert_eq!(in_doc, in_catalogue);
}

#[test]
fn st01_note_with_embedded_instructions_is_data() {
    let r = run("ST01");
    assert_eq!(verdict(step(&r, "notes.create", 0)), ("ALLOW", "OK", None));
    let invoke = step(&r, "skills.invoke", 0);
    assert_eq!(
        verdict(invoke),
        (
            "ALLOW",
            "PRECONDITION_FAILED",
            Some("skills.output_rejected")
        )
    );
    assert_eq!(invoke.state_after["calendar_events"], json!(0));
}

#[test]
fn st02_tampered_skill_package_is_refused() {
    let r = run("ST02");
    assert_eq!(
        verdict(step(&r, "registry:load (SKILL.md", 0)),
        (
            "REFUSED",
            "PRECONDITION_FAILED",
            Some("skills.registry.invalid_packages")
        )
    );
    assert_eq!(
        verdict(step(&r, "registry:load_pinned", 0)),
        (
            "REFUSED",
            "PRECONDITION_FAILED",
            Some("skills.registry.hash_mismatch")
        )
    );
    assert_eq!(
        step(&r, "skills.invoke", 0).state_after["invocations"],
        json!([])
    );
}

#[test]
fn st03_forged_model_provenance_observation_is_rejected() {
    let r = run("ST03");
    let forged = step(&r, "storage:append_observation", 0);
    assert_eq!(
        (forged.decision.as_str(), forged.outcome_category.as_str()),
        ("REFUSED", "INVALID_INPUT")
    );
    assert!(!forged.state_after["observations"]
        .as_array()
        .unwrap()
        .contains(&json!("obs-forged-validated")));
    assert_eq!(
        step(&r, "skills.invoke", 0).state_after["state"],
        json!("APPROVED_FINAL")
    );
}

#[test]
fn st04_skill_output_requesting_execute_is_rejected() {
    let r = run("ST04");
    for nth in 0..2 {
        let s = step(&r, "skills.invoke", nth);
        assert_eq!(
            verdict(s),
            (
                "ALLOW",
                "PRECONDITION_FAILED",
                Some("skills.output_rejected")
            )
        );
        assert_eq!(s.state_after["approvals"], json!({}));
    }
}

#[test]
fn st05_private_export_requires_approval() {
    let r = run("ST05");
    let first = step(&r, "export.request", 0);
    assert_eq!(
        verdict(first),
        (
            "APPROVAL_REQUIRED",
            "APPROVAL_REQUIRED",
            Some("policy.approval_required")
        )
    );
    assert_eq!(first.state_after["export_events"], json!(0));
    let rejected = step(&r, "export.request", 2);
    assert_eq!(rejected.decision, "DENY");
    assert_eq!(
        rejected.error_code.as_deref(),
        Some("approval.not_approved")
    );
    assert_eq!(rejected.state_after["export_events"], json!(0));
}

#[test]
fn st06_modified_model_artifact_is_refused() {
    let r = run("ST06");
    assert_eq!(
        verdict(step(&r, "ai-adapter:verify_checksum (one bit flipped)", 0)),
        (
            "REFUSED",
            "PRECONDITION_FAILED",
            Some("ai.model.checksum_mismatch")
        )
    );
    assert_eq!(
        verdict(step(&r, "model.load", 0)),
        (
            "NO_AUDIT_ROW",
            "INVALID_INPUT",
            Some("ipc.unknown_operation")
        )
    );
}

#[test]
fn st07_markup_payloads_are_stored_as_data() {
    let r = run("ST07");
    for nth in 0..4 {
        assert_eq!(
            verdict(step(&r, "notes.create", nth)),
            ("ALLOW", "OK", None)
        );
    }
}

#[test]
fn st08_oversized_context_and_over_budget_reply_are_refused() {
    let r = run("ST08");
    assert_eq!(
        verdict(step(&r, "skills.invoke", 0)),
        (
            "ALLOW",
            "PRECONDITION_FAILED",
            Some("skills.backend.budget")
        )
    );
    assert_eq!(
        verdict(step(&r, "skills.invoke", 1)),
        (
            "ALLOW",
            "PRECONDITION_FAILED",
            Some("skills.output_rejected")
        )
    );
}

#[test]
fn st09_replayed_stale_and_forged_approvals_are_denied_with_reasons() {
    let r = run("ST09");
    let codes: Vec<Option<&str>> = r
        .steps
        .iter()
        .filter(|s| s.request_id.is_some())
        .map(|s| s.error_code.as_deref())
        .collect();
    assert_eq!(
        codes,
        vec![
            Some("approval.already_consumed"),
            Some("approval.resource_mismatch"),
            Some("approval.expired"),
            Some("approval.not_found"),
            // The approval binds to the stored revision (ADR-0003 addendum
            // 2026-11-13): refused at verification whatever revision the retry
            // claims, so both stale retries now give the same code.
            Some("approval.revision_mismatch"),
            Some("approval.revision_mismatch"),
            Some("approval.mac_invalid"),
        ]
    );
    let last = step(&r, "deletion.request", 3);
    assert_eq!(last.decision, "DENY");
    assert_eq!(
        last.state_after,
        json!({ "a_erased": false, "b_erased": false })
    );
}

#[test]
fn st10_corrupted_backup_fails_verification_and_restore_is_refused() {
    let r = run("ST10");
    assert_eq!(
        verdict(step(&r, "backup.restore_test", 0)),
        ("DENY", "UNAUTHORIZED", Some("policy.denied"))
    );
    assert!(r
        .evidence_for("corrupted snapshot driven to FAILED with integrity FAILED")
        .is_some_and(|c| c.passed));
    assert!(r
        .evidence_for("substituted snapshot FAILED (hash differs from the manifest)")
        .is_some_and(|c| c.passed));
}

#[test]
fn st11_cross_domain_access_is_denied() {
    let r = run("ST11");
    let revise = step(&r, "notes.revise", 0);
    assert_eq!(
        verdict(revise),
        (
            "NOT_EVALUATED",
            "UNAUTHORIZED",
            Some("storage.cross_domain_read")
        )
    );
    assert_eq!(revise.state_after["domain"], json!("PERSONAL"));
    assert_eq!(revise.state_after["revision"], json!(1));
    let prepare = step(&r, "session.prepare", 0);
    assert_eq!(prepare.state_after["state"], json!("PLANNED"));
}

#[test]
fn st12_late_response_after_cancellation_is_discarded() {
    let r = run("ST12");
    let s = step(&r, "orchestrator:invoke T01 (cancelled", 0);
    assert_eq!(s.outcome_category, "CANCELLED");
    assert_eq!(s.state_after["state"], json!("CANCELLED"));
    assert_eq!(s.state_after["has_output"], json!(false));
    assert_eq!(s.state_after["validated"], json!(false));
}
