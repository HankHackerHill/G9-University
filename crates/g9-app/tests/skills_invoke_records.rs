#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! `skills.invoke` records (2026-11-10 review):
//!
//! * app-2: an invocation that fails commits its `skill_invocations` row under the client's
//!   idempotency key. A retry with the same key re-ran the handler and hit the
//!   `UNIQUE (actor_id, idempotency_key)` constraint (CONFLICT `storage.unique_violation`, with
//!   SQLite table names in the message). The retry now answers the recorded outcome and writes
//!   nothing.
//! * storage-repos-4: a package whose files changed while its `g9-version` stayed the same was
//!   recorded against the stored `skill_versions` row of the old files. The invocation is now
//!   refused before the model runs (`PRECONDITION_FAILED skills.version_hash_mismatch`).

mod common;

use common::*;
use g9_ai_adapter::{ModelRef, ScriptedRuntime};
use g9_domain::{ErrorCategory, ModelId};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

fn t01_payload() -> Value {
    json!({ "skill_code": "T01", "input": { "day_id": "D15", "mode": "STUDY", "learner_context_ref": "snapshot:week03-day15" } })
}

fn count(h: &Harness, sql: &str) -> i64 {
    h.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

fn invocations_with_key(h: &Harness, key: &str) -> i64 {
    h.conn()
        .query_row(
            "SELECT COUNT(*) FROM skill_invocations WHERE idempotency_key = ?1",
            [key],
            |r| r.get(0),
        )
        .unwrap()
}

/// Everything a replay must leave untouched.
fn ledger_counts(h: &Harness) -> (i64, i64, i64, i64) {
    (
        count(h, "SELECT COUNT(*) FROM skill_invocations"),
        count(h, "SELECT COUNT(*) FROM audit_metadata"),
        count(h, "SELECT COUNT(*) FROM policy_decisions"),
        count(h, "SELECT COUNT(*) FROM domain_events"),
    )
}

#[test]
fn a_retry_after_a_committed_failure_answers_the_recorded_error_and_leaves_one_row() {
    let registry = g9_skills::Registry::load(&skills_fixture_root()).unwrap();
    // No runtime is configured (NullRuntime): every invocation is a committed failure.
    let mut h = Harness::with(|s| s.skills = Some(registry));

    let first = h
        .call_with("skills.invoke", t01_payload(), &Opts::key("k"))
        .unwrap_err();
    assert_eq!(
        first.category,
        ErrorCategory::ResourceUnavailable,
        "{first:?}"
    );
    assert_eq!(invocations_with_key(&h, "k"), 1);
    let before = ledger_counts(&h);

    let second = h
        .call_with("skills.invoke", t01_payload(), &Opts::key("k"))
        .unwrap_err();
    assert_ne!(second.code, "storage.unique_violation", "{second:?}");
    assert_eq!(second.category, first.category, "{second:?}");
    assert_eq!(second.code, first.code, "{second:?}");
    assert_eq!(second.details, first.details, "{second:?}");
    assert_eq!(second.approval_id, first.approval_id);
    assert!(
        !second.message.to_lowercase().contains("unique"),
        "{second:?}"
    );
    assert_eq!(invocations_with_key(&h, "k"), 1, "exactly one row");
    assert_eq!(
        ledger_counts(&h),
        before,
        "a replay runs nothing and writes nothing"
    );

    // The same key for a different request is still a conflict.
    let other = json!({ "skill_code": "T01", "input": { "day_id": "D16", "mode": "STUDY", "learner_context_ref": "snapshot:week03-day16" } });
    let reused = h
        .call_with("skills.invoke", other, &Opts::key("k"))
        .unwrap_err();
    assert_eq!(reused.category, ErrorCategory::Conflict, "{reused:?}");
    assert_eq!(reused.code, "idempotency.key_reused");
    assert_eq!(invocations_with_key(&h, "k"), 1);
}

#[test]
fn a_key_that_names_an_invocation_without_an_idempotency_record_is_refused_cleanly() {
    // A database written before committed failures were replayable (or whose idempotency
    // records were purged) holds the invocation row and no record of its outcome.
    let registry = g9_skills::Registry::load(&skills_fixture_root()).unwrap();
    let mut h = Harness::with(|s| s.skills = Some(registry));
    h.call_with("skills.invoke", t01_payload(), &Opts::key("legacy"))
        .unwrap_err();
    h.conn()
        .execute("DELETE FROM idempotency_records", [])
        .unwrap();
    let invocation: String = h
        .conn()
        .query_row(
            "SELECT id FROM skill_invocations WHERE idempotency_key = 'legacy'",
            [],
            |r| r.get(0),
        )
        .unwrap();

    let retry = h
        .call_with("skills.invoke", t01_payload(), &Opts::key("legacy"))
        .unwrap_err();
    assert_eq!(retry.category, ErrorCategory::Conflict, "{retry:?}");
    assert_eq!(
        retry.code, "skills.invocation_already_recorded",
        "{retry:?}"
    );
    assert!(
        !retry.message.contains("skill_invocations") && !retry.message.contains("UNIQUE"),
        "no SQLite text: {retry:?}"
    );
    assert_eq!(
        retry.details.as_ref().map(|d| d["invocation_id"].clone()),
        Some(json!(invocation))
    );
    assert_eq!(invocations_with_key(&h, "legacy"), 1);
}

#[test]
fn the_recorded_failure_keeps_no_text_derived_from_the_input() {
    let registry = g9_skills::Registry::load(&skills_fixture_root()).unwrap();
    let mut h = Harness::with(|s| s.skills = Some(registry));
    // A property the input schema does not allow: the preflight names it in its message.
    let payload = json!({ "skill_code": "T01", "input": { "day_id": "D15", "mode": "STUDY", "learner_context_ref": "snapshot:week03-day15", "PLUGHSENTINEL": 1 } });
    let first = h
        .call_with("skills.invoke", payload.clone(), &Opts::key("p"))
        .unwrap_err();
    assert_eq!(first.code, "skills.preflight_failed", "{first:?}");
    assert!(first.message.contains("PLUGHSENTINEL"), "{first:?}");

    let stored: Vec<String> = {
        let mut stmt = h
            .conn()
            .prepare("SELECT outcome_json FROM idempotency_records")
            .unwrap();
        stmt.query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert!(
        stored.iter().all(|o| !o.contains("PLUGHSENTINEL")),
        "{stored:?}"
    );

    let replay = h
        .call_with("skills.invoke", payload, &Opts::key("p"))
        .unwrap_err();
    assert_eq!(replay.code, first.code);
    assert_eq!(replay.category, first.category);
    assert_eq!(replay.details, first.details);
    assert!(!replay.message.contains("PLUGHSENTINEL"), "{replay:?}");
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}

/// A schema-valid T01 output with no recommended actions.
fn t01_output() -> Value {
    json!({
        "status": "COMPLETED",
        "result_schema_version": "1.0",
        "observations": [],
        "inferences": [],
        "evidence_refs": [],
        "confidence_limits": ["fixture"],
        "recommended_actions": [],
        "approval_requirements": [],
        "resource_usage": { "tokens": 10, "seconds": 1, "tool_calls": 0 },
        "validation_results": [],
        "errors": [],
        "result": {
            "brief": { "day_id": "D15", "objectives": [], "due_reviews": [], "open_interventions": [], "risks": [] },
            "basis": { "evidence_refs": [], "unresolved_refs": [] }
        }
    })
}

#[test]
fn a_package_changed_without_a_version_bump_is_refused_and_not_recorded_against_the_old_row() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("skills");
    copy_dir(&skills_fixture_root(), &root);
    // Pinned registries, as the desktop application loads them: a release
    // whose pins changed while the package kept its version.
    let original = pinned(&root);
    let mut h = Harness::with(|s| s.skills = Some(original.clone()));
    // The first invocation registers `g9-session-director@0.1.0` (no model: a committed failure).
    h.call_with("skills.invoke", t01_payload(), &Opts::key("before-edit"))
        .unwrap_err();
    let version = "g9-session-director@0.1.0";
    let stored_hash: String = h
        .conn()
        .query_row(
            "SELECT package_sha256 FROM skill_versions WHERE id = ?1",
            [version],
            |r| r.get(0),
        )
        .unwrap();
    let name = g9_domain::SkillId::parse("g9-session-director").unwrap();
    assert_eq!(original.hashes().get(&name), Some(&stored_hash));

    // A release edits SKILL.md and keeps `g9-version: "0.1.0"`.
    let skill_md = root.join("g9-session-director/SKILL.md");
    let mut text = std::fs::read_to_string(&skill_md).unwrap();
    assert!(text.contains("g9-version: \"0.1.0\""));
    text.push_str("\nAn added paragraph that changes what the model is told.\n");
    std::fs::write(&skill_md, text).unwrap();
    let edited = pinned(&root);
    let edited_pkg = edited.get(&name).unwrap();
    assert_eq!(edited_pkg.version, "0.1.0");
    assert_ne!(edited_pkg.package_sha256, stored_hash);

    let runtime = Arc::new(ScriptedRuntime::new(
        ModelRef::new(ModelId::parse("scripted-model").unwrap()),
        1_000_000,
    ));
    let out = t01_output();
    runtime.push_response(runtime.response(out.to_string(), Some(out)));
    let rt: Arc<dyn g9_ai_adapter::InferenceRuntime> = runtime.clone();
    let db = h.ctx.take().unwrap().into_database();
    let mut h = Harness::with_database(db, |s| {
        s.skills = Some(edited);
        s.runtime = Some(rt);
    })
    .unwrap();
    h.continue_ids_from(1_000);
    let invocations = count(&h, "SELECT COUNT(*) FROM skill_invocations");

    let refused = h
        .call_with("skills.invoke", t01_payload(), &Opts::key("after-edit"))
        .unwrap_err();
    assert_eq!(
        refused.category,
        ErrorCategory::PreconditionFailed,
        "{refused:?}"
    );
    assert_eq!(refused.code, "skills.version_hash_mismatch");
    assert_eq!(runtime.remaining(), 1, "the model was not called");
    assert_eq!(
        count(&h, "SELECT COUNT(*) FROM skill_invocations"),
        invocations,
        "nothing is recorded against the stale version row"
    );
    let rows: Vec<(String, String)> = {
        let mut stmt = h
            .conn()
            .prepare("SELECT id, package_sha256 FROM skill_versions")
            .unwrap();
        stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(rows, vec![(version.to_string(), stored_hash)]);
}

/// The registry under `root`, loaded against its own hashes as pins.
fn pinned(root: &Path) -> g9_skills::Registry {
    let pins = g9_skills::Registry::load(root).unwrap().hashes();
    let registry = g9_skills::Registry::load_pinned(root, &pins).unwrap();
    assert!(registry.is_pinned());
    registry
}

#[test]
fn an_unpinned_first_registration_does_not_block_the_pinned_package() {
    // Final security review authz-2: the first invocation registered the
    // version row from whatever registry was loaded. An edited package invoked
    // once through the unpinned CLI became the immutable row
    // `g9-session-director@0.1.0`, and the pinned package of the same version
    // was refused with skills.version_hash_mismatch from then on.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("skills");
    copy_dir(&skills_fixture_root(), &root);
    let skill_md = root.join("g9-session-director/SKILL.md");
    let mut text = std::fs::read_to_string(&skill_md).unwrap();
    text.push_str("\nAn edited paragraph.\n");
    std::fs::write(&skill_md, text).unwrap();
    let edited = g9_skills::Registry::load(&root).unwrap();
    assert!(!edited.is_pinned());
    let mut h = Harness::with(|s| s.skills = Some(edited));
    h.call_with("skills.invoke", t01_payload(), &Opts::key("edited"))
        .unwrap_err();

    let db = h.ctx.take().unwrap().into_database();
    let genuine = pinned(&skills_fixture_root());
    let mut h = Harness::with_database(db, |s| s.skills = Some(genuine)).unwrap();
    h.continue_ids_from(1_000);
    let second = h
        .call_with("skills.invoke", t01_payload(), &Opts::key("genuine"))
        .unwrap_err();
    assert_ne!(second.code, "skills.version_hash_mismatch", "{second:?}");
    assert_eq!(
        second.category,
        ErrorCategory::ResourceUnavailable,
        "{second:?}"
    );
    let ids: Vec<String> = {
        let mut stmt = h
            .conn()
            .prepare("SELECT id FROM skill_versions ORDER BY id")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(ids.len(), 2, "{ids:?}");
    assert!(
        ids.contains(&"g9-session-director@0.1.0".to_string()),
        "{ids:?}"
    );
    assert!(
        ids.iter()
            .any(|i| i.starts_with("g9-session-director@0.1.0+")),
        "{ids:?}"
    );
}

#[test]
fn an_unchanged_package_keeps_using_its_version_row() {
    let registry = g9_skills::Registry::load(&skills_fixture_root()).unwrap();
    let mut h = Harness::with(|s| s.skills = Some(registry));
    h.call_with("skills.invoke", t01_payload(), &Opts::key("one"))
        .unwrap_err();
    h.restart();
    let second = h
        .call_with("skills.invoke", t01_payload(), &Opts::key("two"))
        .unwrap_err();
    assert_eq!(
        second.category,
        ErrorCategory::ResourceUnavailable,
        "{second:?}"
    );
    assert_eq!(count(&h, "SELECT COUNT(*) FROM skill_versions"), 1);
    assert_eq!(count(&h, "SELECT COUNT(*) FROM skill_invocations"), 2);
}
