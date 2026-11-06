//! Gate A08 (backup / restore / recovery) and A03 (timer discontinuity on the
//! recovery path): populated file database → backup → verify → restore test →
//! crash with a write transaction open → reopen → recovery → audit chain.
//!
//! Registered as `[[test]] a08_recovery_drill` of `crates/g9-evaluation`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use g9_evaluation::recovery::{self, RUNNING_SESSION};
use g9_evaluation::ScenarioReport;
use serde_json::json;

fn passed(r: &ScenarioReport) {
    assert!(r.passed, "{}", r.to_markdown());
}

#[test]
fn a08_backup_verify_restore_crash_reopen_recover() {
    let r = recovery::recovery_drill().unwrap();
    passed(&r);

    // Backup lifecycle through the application.
    let create = r.steps_for("backup.create")[0];
    assert_eq!(
        (create.decision.as_str(), create.outcome_category.as_str()),
        ("ALLOW", "OK")
    );
    let verifies = r.steps_for("backup.verify");
    assert_eq!(verifies[0].decision, "ALLOW");
    let restore = r.steps_for("backup.restore_test")[0];
    assert_eq!(restore.decision, "ALLOW");
    assert_eq!(restore.state_after["scratch_entries"], json!(0));
    assert!(r
        .evidence_for("restore into a disposable copy passed; backup retained")
        .is_some_and(|c| c.passed));

    // The crash left an uncommitted write behind.
    let crash = r
        .steps
        .iter()
        .find(|s| s.op.starts_with("harness:crash"))
        .unwrap();
    assert_eq!(
        crash.state_after["uncommitted_row_visible_inside_tx"],
        json!(true)
    );

    // Recovery at reopen.
    let reopen = r
        .steps
        .iter()
        .find(|s| s.op.starts_with("harness:reopen"))
        .unwrap();
    assert_eq!(reopen.decision, "RECOVERED");
    assert_eq!(reopen.state_after["audit_chain_ok"], json!(true));
    assert_eq!(
        reopen.state_after["interrupted_sessions"],
        json!([RUNNING_SESSION])
    );
    assert_eq!(reopen.state_after["pending_migrations"], json!([]));
    assert_eq!(
        reopen.state_after["expired_approvals"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    for check in [
        "session INTERRUPTED; interval closed UNCERTAIN by recovery",
        "uncertain time is never credited (credited 0, uncertain > 0)",
        "the uncommitted row did not survive the crash",
        "committed learner data identical after the crash",
        "audit chain verifies after recovery (+1 interrupt row, +1 expiry row)",
    ] {
        assert!(
            r.evidence_for(check).is_some_and(|c| c.passed),
            "{check}\n{}",
            r.to_markdown()
        );
    }

    // The application keeps working after recovery and the chain still verifies.
    let after = r.steps_for("notes.create")[0];
    assert_eq!(after.decision, "ALLOW");
    assert_eq!(after.state_after["ok"], json!(true));
    let list = r.steps_for("session.list")[0];
    assert_eq!(list.outcome_category, "OK");
}
