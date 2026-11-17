#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! app-6: what a refusal that storage would have raised says through IPC.
//!
//! SQLite's own constraint text names tables and columns ("UNIQUE constraint
//! failed: competencies.code"); it reached the webview as the error message.
//! The cases a caller can provoke are now answered before storage with a typed
//! code (an unknown evidence id is NOT_FOUND, a duplicate code is CONFLICT), and
//! no serialized error names a table, a constraint or `UNIQUE`.

mod common;

use common::*;
use g9_domain::{ErrorCategory, G9Error};
use serde_json::json;

/// Every table of the schema, read from the live database.
fn table_names(h: &Harness) -> Vec<String> {
    let mut stmt = h
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
        .unwrap();
    stmt.query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// The serialized error holds no table name (as a whole word), no
/// "constraint" and no "UNIQUE".
fn assert_names_no_schema(h: &Harness, err: &G9Error) {
    let text = serde_json::to_string(err).unwrap();
    assert!(
        !text.to_lowercase().contains("constraint"),
        "constraint text in {text}"
    );
    assert!(!text.contains("UNIQUE"), "UNIQUE in {text}");
    let words: Vec<&str> = text
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .collect();
    for table in table_names(h) {
        assert!(
            !words.contains(&table.as_str()),
            "table {table} named in {text}"
        );
    }
}

fn count(h: &Harness, sql: &str) -> i64 {
    h.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn a_submission_citing_an_unknown_evidence_id_is_not_found_and_names_no_table() {
    let mut h = Harness::new();
    let assessment = seed_blind_practical(&h, 1, Some(3));
    let attempt = h
        .call(
            "assessment.begin",
            json!({ "assessment_id": assessment.as_str() }),
        )
        .unwrap()["result"]["attempt_id"]
        .as_str()
        .unwrap()
        .to_string();

    let refused = h
        .call(
            "assessment.submit",
            json!({ "attempt_id": attempt, "evidence_refs": ["nope"] }),
        )
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::NotFound, "{refused:?}");
    assert_eq!(refused.code, "evidence.not_found");
    assert_names_no_schema(&h, &refused);

    // Nothing was linked and the attempt is still open.
    assert_eq!(count(&h, "SELECT COUNT(*) FROM attempt_evidence"), 0);
    let state: String = h
        .conn()
        .query_row(
            "SELECT state FROM attempts WHERE id = ?1",
            [attempt.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(state, "STARTED");
}

#[test]
fn a_duplicate_competency_code_is_a_typed_conflict_that_names_no_table() {
    let mut h = Harness::new();
    h.call(
        "competency.create",
        json!({ "code": "DUP", "title": "First" }),
    )
    .unwrap();
    let refused = h
        .call(
            "competency.create",
            json!({ "code": "DUP", "title": "Second" }),
        )
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::Conflict, "{refused:?}");
    assert_eq!(refused.code, "competency.code_exists");
    assert_names_no_schema(&h, &refused);
    assert_eq!(
        count(&h, "SELECT COUNT(*) FROM competencies WHERE code = 'DUP'"),
        1
    );
}

#[test]
fn a_duplicate_assessment_code_is_a_typed_conflict_that_names_no_table() {
    let mut h = Harness::new();
    let competency = h
        .call(
            "competency.create",
            json!({ "code": "WCP", "title": "Web cache poisoning" }),
        )
        .unwrap()["result"]["competency_id"]
        .as_str()
        .unwrap()
        .to_string();
    let define = |title: &str| {
        json!({
            "code": "WCP-BLIND",
            "title": title,
            "kind": "BLIND_PRACTICAL",
            "competency_id": competency,
            "task_statement": "Poison the cache of the lab proxy.",
            "difficulty": 3,
        })
    };
    h.call("assessment.define", define("First")).unwrap();
    let refused = h.call("assessment.define", define("Second")).unwrap_err();
    assert_eq!(refused.category, ErrorCategory::Conflict, "{refused:?}");
    assert_eq!(refused.code, "assessment.code_exists");
    assert_names_no_schema(&h, &refused);
    assert_eq!(
        count(
            &h,
            "SELECT COUNT(*) FROM assessment_definitions WHERE code = 'WCP-BLIND'"
        ),
        1
    );
}

#[test]
fn a_submission_citing_evidence_of_another_domain_is_not_found() {
    let mut h = Harness::new();
    let assessment = seed_blind_practical(&h, 1, Some(3));
    let personal = personal_evidence(&h, "evi-personal-1");
    let attempt = h
        .call(
            "assessment.begin",
            json!({ "assessment_id": assessment.as_str() }),
        )
        .unwrap()["result"]["attempt_id"]
        .as_str()
        .unwrap()
        .to_string();
    let refused = h
        .call(
            "assessment.submit",
            json!({ "attempt_id": attempt, "evidence_refs": [personal] }),
        )
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::NotFound, "{refused:?}");
    assert_eq!(refused.code, "evidence.not_found");
    assert_names_no_schema(&h, &refused);
    assert_eq!(count(&h, "SELECT COUNT(*) FROM attempt_evidence"), 0);
}
