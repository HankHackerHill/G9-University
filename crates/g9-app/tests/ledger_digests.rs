#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The hashes of whole requests in the immutable ledgers are keyed (ADR-0021, security audit
//! 2026-11-13 residual).
//!
//! A plain SHA-256 of `{operation, payload}` confirms a guess: with the title known, a four-digit
//! body is recovered from the policy, approval or idempotency ledger in 10,000 tries by anyone who
//! holds the database file or one of its snapshots. The ledgers cannot be erased, so the hash must
//! not be computable without a secret that stays outside the database: the binding key.
//!
//! The probe below does what such an attacker does. It takes every 64-hex value stored anywhere in
//! the database (any table, any column, inside JSON text too), hashes every four-digit guess in the
//! two forms the pipeline uses, and looks for a match.

mod common;

use common::*;
use g9_ai_adapter::{ModelRef, ScriptedRuntime};
use g9_domain::hashing::sha256_json;
use g9_domain::{ApprovalId, ApprovalState, ErrorCategory, ModelId};
use g9_policy::DigestDomain;
use g9_storage::approvals_store::{self, NewApprovalRequest};
use g9_storage::repos::knowledge as knowledge_repo;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::Arc;

/// Every run of exactly 64 lowercase hex characters in any column of any table.
fn stored_hex64(conn: &rusqlite::Connection) -> BTreeSet<String> {
    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let mut found = BTreeSet::new();
    for table in tables {
        let columns: Vec<String> = conn
            .prepare(&format!("PRAGMA table_info(\"{table}\")"))
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for column in columns {
            let texts: Vec<String> = conn
                .prepare(&format!(
                    "SELECT CAST(\"{column}\" AS TEXT) FROM \"{table}\" WHERE \"{column}\" IS NOT NULL"
                ))
                .unwrap()
                .query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            for text in texts {
                collect_hex64(&text, &mut found);
            }
        }
    }
    found
}

fn collect_hex64(text: &str, into: &mut BTreeSet<String>) {
    let bytes = text.as_bytes();
    let is_hex = |b: u8| b.is_ascii_digit() || (b'a'..=b'f').contains(&b);
    let mut start = 0;
    while start < bytes.len() {
        if !is_hex(bytes[start]) {
            start += 1;
            continue;
        }
        let mut end = start;
        while end < bytes.len() && is_hex(bytes[end]) {
            end += 1;
        }
        if end - start == 64 {
            into.insert(text[start..end].to_string());
        }
        start = end;
    }
}

fn secret_payload(body: &str) -> Value {
    json!({ "title": "pin", "body_markdown": body, "classification": "SECRET", "domain": "LEARNING" })
}

/// The pre-image of the approval binding hash: `{operation, payload}`.
fn parameters_preimage(operation: &str, payload: &Value) -> Value {
    json!({ "operation": operation, "payload": payload })
}

/// The pre-image of the idempotency request hash, as the pipeline builds it for a request that
/// names no resource, no revision and no approval.
fn request_preimage(operation: &str, payload: &Value) -> Value {
    let (domain, mode) = default_scope(operation);
    json!({
        "operation": operation,
        "payload": payload,
        "resource": null,
        "scope": { "domain": domain, "mode": mode },
        "expected_revision": null,
        "approval_id": null,
    })
}

#[test]
fn no_ledger_holds_a_plain_sha256_of_a_request_that_carried_a_four_digit_secret() {
    let mut h = Harness::new();
    let created = h
        .call_with(
            "notes.create",
            secret_payload("4711"),
            &Opts::key("probe-key-1"),
        )
        .unwrap();
    let note_id = created["result"]["note_id"].as_str().unwrap().to_string();
    // The ledgers an approval writes to as well: policy decisions, approval requests and decisions.
    h.approved(
        "deletion.request",
        json!({ "resource": { "kind": "NOTE", "id": note_id }, "reason": "test" }),
        &Opts::default(),
    );

    let stored = stored_hex64(h.conn());
    assert!(
        stored.len() > 20,
        "the database holds {} hash-like values: the probe would prove nothing",
        stored.len()
    );

    // The attacker: every four-digit body, both forms of a whole-request hash.
    let mut recovered: Vec<String> = Vec::new();
    for guess in 0..10_000u32 {
        let payload = secret_payload(&format!("{guess:04}"));
        for (form, preimage) in [
            ("parameters", parameters_preimage("notes.create", &payload)),
            ("request", request_preimage("notes.create", &payload)),
        ] {
            if stored.contains(&sha256_json(&preimage)) {
                recovered.push(format!("{form} form: body {guess:04}"));
            }
        }
    }
    assert_eq!(
        recovered,
        Vec::<String>::new(),
        "a ledger holds a plain SHA-256 of a whole request: the secret body is recoverable from the database file"
    );
}

#[test]
fn the_ledgers_hold_the_keyed_digests_of_exactly_these_requests() {
    // The positive control of the probe above: the pre-images it builds are the real ones (a wrong
    // pre-image would make "nothing found" prove nothing), and what the ledgers hold instead is
    // the digest under the binding key, so an approval is still bound to its request.
    let mut h = Harness::new();
    let payload = secret_payload("4711");
    h.call_with("notes.create", payload.clone(), &Opts::key("probe-key-2"))
        .unwrap();
    let stored = stored_hex64(h.conn());
    let parameters = key()
        .digest_json(
            DigestDomain::Parameters,
            &parameters_preimage("notes.create", &payload),
        )
        .unwrap();
    let request = key()
        .digest_json(
            DigestDomain::Request,
            &request_preimage("notes.create", &payload),
        )
        .unwrap();
    assert!(
        stored.contains(&parameters),
        "policy_decisions.parameters_sha256 is not the keyed digest of {{operation, payload}}"
    );
    assert!(
        stored.contains(&request),
        "idempotency_records.request_sha256 is not the keyed digest of the request"
    );
    assert_ne!(parameters, request, "domains are separated");
    // Under another key the same requests have other digests: the database alone confirms nothing.
    let other = g9_policy::BindingKey::from_bytes([8u8; 32]);
    let elsewhere = other
        .digest_json(
            DigestDomain::Parameters,
            &parameters_preimage("notes.create", &payload),
        )
        .unwrap();
    assert!(!stored.contains(&elsewhere));
}

#[test]
fn the_digest_of_a_stored_replay_result_is_keyed() {
    // `notes.create` answers with the note (body included); the idempotency record keeps a
    // content-free replay record and a digest of the full result. A plain hash of it would
    // confirm a guess at the body from the record's other fields.
    let mut h = Harness::new();
    let created = h
        .call_with(
            "notes.create",
            secret_payload("4711"),
            &Opts::key("probe-key-3"),
        )
        .unwrap();
    let outcome: String = h
        .conn()
        .query_row(
            "SELECT outcome_json FROM idempotency_records WHERE idempotency_key = 'probe-key-3'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let stored: Value = serde_json::from_str(&outcome).unwrap();
    let recorded = stored["result"]["result_sha256"].as_str().unwrap();
    let result = &created["result"];
    assert_eq!(
        recorded,
        key().digest_json(DigestDomain::Result, result).unwrap(),
        "the stored result digest is not the keyed one"
    );
    assert_ne!(
        recorded,
        sha256_json(result),
        "a plain SHA-256 of the result is stored"
    );
    // A replay of the same request still returns the same outcome.
    let again = h
        .call_with(
            "notes.create",
            secret_payload("4711"),
            &Opts::key("probe-key-3"),
        )
        .unwrap();
    assert_eq!(again["replayed"], true);
    assert_eq!(again["result"]["note_id"], created["result"]["note_id"]);
}

#[test]
fn an_approval_made_before_the_digests_were_keyed_does_not_authorize_the_request_now() {
    // An upgrade changes what every approval is bound to (ADR-0021): an approval that an older
    // build stored with a plain SHA-256 of the request does not match the keyed digest this build
    // computes, so it is refused and left unconsumed; it expires unused. A new request works.
    let dir = tempfile::Builder::new().prefix("g9ld").tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let mut h = Harness::with(|s| {
        s.config.approved_roots = vec![root.clone()];
        s.config.export_dir = Some(root.clone());
    });
    let payload = json!({ "domains": ["LEARNING"], "format": "JSON", "include_private": false });

    // What this build stores for the request.
    let first = h.call("export.request", payload.clone()).unwrap_err();
    assert_eq!(first.category, ErrorCategory::ApprovalRequired);
    let row = approvals_store::get_request(h.conn(), first.approval_id.as_ref().unwrap())
        .unwrap()
        .unwrap();
    // The same descriptor, with the plain hash an older build stored in its place.
    let mut older = row.operation.clone();
    older.parameters_sha256 = sha256_json(&parameters_preimage("export.request", &payload));
    assert_ne!(older.parameters_sha256, row.operation.parameters_sha256);
    let old_id = ApprovalId::parse("apr-from-an-older-build").unwrap();
    approvals_store::insert_request(
        h.conn(),
        &NewApprovalRequest {
            id: old_id.clone(),
            operation: older,
            required_capability: row.required_capability,
            requested_by: row.requested_by.clone(),
            requested_at: row.requested_at,
            expires_at: row.expires_at,
            expected_revision: row.expected_revision,
        },
    )
    .unwrap();
    h.decide(old_id.as_str(), "APPROVE");

    let refused = h
        .call_with(
            "export.request",
            payload.clone(),
            &Opts::approval(old_id.as_str()),
        )
        .unwrap_err();
    assert_eq!(refused.code, "approval.operation_mismatch", "{refused:?}");
    let after = approvals_store::get_request(h.conn(), &old_id)
        .unwrap()
        .unwrap();
    assert_eq!(
        after.state,
        ApprovalState::Approved,
        "the approval was consumed"
    );

    // A request made and approved by this build goes through.
    let done = h.approved("export.request", payload, &Opts::default());
    assert_eq!(done["result"]["state"], "READY", "{done}");
}

/// A schema-valid T01 output (no recommended actions) carrying `risk`.
fn valid_brief_output(risk: &str) -> Value {
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
            "brief": { "day_id": "D15", "objectives": [], "due_reviews": [], "open_interventions": [], "risks": [risk] },
            "basis": { "evidence_refs": [], "unresolved_refs": [] }
        }
    })
}

/// The digests `skill_invocations` holds after one invocation of T01 on an installation whose
/// binding key is `[key_byte; 32]`: `(input_sha256, output_sha256)`.
fn skill_digests_under(key_byte: u8, input: &Value, out: &Value) -> (String, String) {
    let runtime = Arc::new(ScriptedRuntime::new(
        ModelRef::new(ModelId::parse("scripted-model").unwrap()),
        1_000_000,
    ));
    runtime.push_response(runtime.response(out.to_string(), Some(out.clone())));
    let registry = g9_skills::Registry::load(&skills_fixture_root()).unwrap();
    let rt: Arc<dyn g9_ai_adapter::InferenceRuntime> = runtime.clone();
    let mut h = Harness::with(|s| {
        s.runtime = Some(rt);
        s.skills = Some(registry);
        s.key = [key_byte; 32];
    });
    h.call(
        "skills.invoke",
        json!({ "skill_code": "T01", "input": input }),
    )
    .unwrap();
    let rows = knowledge_repo::list_invocations_for_skill(
        h.conn(),
        &g9_domain::SkillId::parse("g9-session-director").unwrap(),
        10,
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    (
        rows[0].input_sha256.clone(),
        rows[0].output_sha256.clone().unwrap(),
    )
}

#[test]
fn the_digests_of_a_skills_input_and_output_are_keyed() {
    // `skill_invocations` is an immutable ledger too (no delete, no update once terminal), and a
    // skill's input is operator content: the text of a note can be passed to it. A plain SHA-256 of
    // that input or of the model's output confirms a guess at it.
    let input =
        json!({ "day_id": "D15", "mode": "STUDY", "learner_context_ref": "snapshot:week03-day15" });
    let out = valid_brief_output("a risk");
    let (input_7, output_7) = skill_digests_under(7, &input, &out);

    // Not the plain hash of either.
    assert_ne!(
        input_7,
        sha256_json(&input),
        "the ledger holds a plain SHA-256 of the skill's input"
    );
    assert_ne!(
        output_7,
        sha256_json(&out),
        "the ledger holds a plain SHA-256 of the skill's output"
    );
    // The input digest is exactly the keyed digest under this installation's key.
    assert_eq!(
        input_7,
        key().digest_json(DigestDomain::SkillInput, &input).unwrap()
    );
    // The output digest depends on the key and on nothing else that changes between runs: the
    // same key gives the same value, another key another (a plain hash is the same in all three).
    let (input_7_again, output_7_again) = skill_digests_under(7, &input, &out);
    let (input_9, output_9) = skill_digests_under(9, &input, &out);
    assert_eq!((&input_7, &output_7), (&input_7_again, &output_7_again));
    assert_ne!(
        input_7, input_9,
        "the input digest does not depend on the key"
    );
    assert_ne!(
        output_7, output_9,
        "the output digest does not depend on the key"
    );
}
