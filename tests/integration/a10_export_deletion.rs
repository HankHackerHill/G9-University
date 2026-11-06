//! Gate A10 (export / deletion propagation), driven through
//! `g9_app::dispatch` over the week-3 fixture.
//!
//! Registered as `[[test]] a10_export_deletion` of `crates/g9-evaluation`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use g9_evaluation::{scenarios, ScenarioReport};
use serde_json::json;

fn passed(r: &ScenarioReport) {
    assert!(r.passed, "{}", r.to_markdown());
}

#[test]
fn a10_approved_deletion_redacts_note_and_versions_with_tombstone() {
    let r = scenarios::a10_deletion_propagation().unwrap();
    passed(&r);
    let deletions = r.steps_for("deletion.request");
    assert_eq!(deletions.len(), 3);
    // Without approval: nothing erased.
    assert_eq!(deletions[0].decision, "APPROVAL_REQUIRED");
    assert_eq!(deletions[0].state_after["erased"], json!(false));
    // With approval: erased, every version erased, body redacted.
    assert_eq!(deletions[1].decision, "ALLOW");
    assert_eq!(deletions[1].state_after["erased"], json!(true));
    assert_eq!(
        deletions[1].state_after["erased_versions"],
        deletions[1].state_after["version_count"]
    );
    assert_ne!(
        deletions[1].state_after["body_sha256"],
        deletions[0].state_after["body_sha256"]
    );
    // Repeat: refused; state unchanged.
    assert_eq!(
        deletions[2].error_code.as_deref(),
        Some("deletion.already_erased")
    );
    assert_eq!(deletions[2].state_after, deletions[1].state_after);
    let revise = r.steps_for("notes.revise")[0];
    assert_ne!(revise.outcome_category, "OK");
    assert_eq!(revise.state_after, deletions[1].state_after);
    assert!(r
        .evidence_for("no event carries the erased content")
        .is_some_and(|c| c.passed));
}

#[test]
fn a10_export_requires_an_approval_bound_to_the_selection_and_is_recorded_once() {
    let r = scenarios::a10_export_request().unwrap();
    passed(&r);
    let exports = r.steps_for("export.request");
    assert_eq!(exports.len(), 3);
    assert_eq!(exports[0].decision, "APPROVAL_REQUIRED");
    assert_eq!(exports[0].state_after["export_events"], json!(0));
    assert_eq!(exports[1].decision, "ALLOW");
    assert_eq!(exports[1].state_after["export_events"], json!(1));
    assert_eq!(exports[2].decision, "DENY");
    assert_eq!(
        exports[2].error_code.as_deref(),
        Some("approval.already_consumed")
    );
    assert_eq!(exports[2].state_after["export_events"], json!(1));
}
