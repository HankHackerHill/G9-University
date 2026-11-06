//! Read-only backend state probes used as `state_after` and in checks.
//!
//! Every probe reads through the `g9-storage` repositories (no SQL here) and
//! returns metadata only: hashes, states, revisions and counts — never a note
//! body or other private content.

use crate::harness::Harness;
use g9_domain::hashing::sha256_hex;
use g9_domain::{
    ApprovalId, ApprovalState, AttemptId, CompetencyId, DataDomain, Decision, G9Error, NoteId,
    ResourceRef, SessionId, SkillId, UtcMillis,
};
use g9_storage::approvals_store;
use g9_storage::events;
use g9_storage::repos::assessment as assessment_repo;
use g9_storage::repos::calendar as calendar_repo;
use g9_storage::repos::competency as competency_repo;
use g9_storage::repos::evidence as evidence_repo;
use g9_storage::repos::knowledge as knowledge_repo;
use g9_storage::repos::sessions as sessions_repo;
use g9_storage::repos::ReadScope;
use g9_storage::AuditChain;
use serde_json::{json, Value};

/// Cross-domain scope used by the harness to *inspect* rows (never passed to the app).
pub fn inspect(domain: DataDomain) -> ReadScope {
    ReadScope::cross_domain(domain)
}

fn id<T: std::str::FromStr>(s: &str) -> Result<T, G9Error> {
    s.parse()
        .map_err(|_| G9Error::invalid_input("probe.id_invalid", "probe id is not valid"))
}

/// Stored approval request (state, binding, consumption).
pub fn approval(h: &Harness, approval_id: &str) -> Result<Value, G9Error> {
    let Ok(aid) = ApprovalId::parse(approval_id) else {
        return Ok(json!({ "exists": false }));
    };
    Ok(match approvals_store::get_request(h.db()?.conn(), &aid)? {
        None => json!({ "exists": false }),
        Some(r) => json!({
            "exists": true,
            "state": r.state,
            "action": r.operation.action,
            "resource": r.operation.resource,
            "parameters_sha256": r.operation.parameters_sha256,
            "required_capability": r.required_capability,
            "requested_by": r.requested_by,
            "expected_revision": r.expected_revision,
            "has_binding_mac": r.binding_mac.is_some(),
            "consumed": r.consumed_at.is_some(),
        }),
    })
}

/// Stored approval state, if the request exists.
pub fn approval_state(h: &Harness, approval_id: &str) -> Result<Option<ApprovalState>, G9Error> {
    let Ok(aid) = ApprovalId::parse(approval_id) else {
        return Ok(None);
    };
    Ok(approvals_store::get_request(h.db()?.conn(), &aid)?.map(|r| r.state))
}

/// Number of approval requests in `state`.
pub fn approvals_in(h: &Harness, state: ApprovalState) -> Result<usize, G9Error> {
    Ok(approvals_store::list_by_state(h.db()?.conn(), state)?.len())
}

/// Count of approval requests per state.
pub fn approval_counts(h: &Harness) -> Result<Value, G9Error> {
    let mut out = serde_json::Map::new();
    for s in ApprovalState::ALL {
        let n = approvals_in(h, *s)?;
        if n > 0 {
            out.insert(
                serde_json::to_value(s)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default(),
                json!(n),
            );
        }
    }
    Ok(Value::Object(out))
}

/// Note metadata: revision, erasure, hash of the current body, version count.
pub fn note(h: &Harness, note_id: &str) -> Result<Value, G9Error> {
    let nid: NoteId = id(note_id)?;
    let conn = h.db()?.conn();
    let scope = inspect(DataDomain::Learning);
    Ok(match evidence_repo::get_note(conn, &scope, &nid)? {
        None => json!({ "exists": false }),
        Some(n) => {
            let versions = evidence_repo::list_note_versions(conn, &scope, &nid)?;
            json!({
                "exists": true,
                "revision": n.revision,
                "erased": n.erased,
                "domain": n.data_domain,
                "classification": n.classification,
                "body_sha256": sha256_hex(n.body_markdown.as_bytes()),
                "version_count": versions.len(),
                "erased_versions": versions.iter().filter(|v| v.erased).count(),
            })
        }
    })
}

/// SHA-256 of the stored note body (for "stored verbatim" / "redacted" checks).
pub fn note_body_sha256(h: &Harness, note_id: &str) -> Result<Option<String>, G9Error> {
    let nid: NoteId = id(note_id)?;
    Ok(
        evidence_repo::get_note(h.db()?.conn(), &inspect(DataDomain::Learning), &nid)?
            .map(|n| sha256_hex(n.body_markdown.as_bytes())),
    )
}

/// Attempt state, revision and grade decisions.
pub fn attempt(h: &Harness, attempt_id: &str) -> Result<Value, G9Error> {
    let aid: AttemptId = id(attempt_id)?;
    let conn = h.db()?.conn();
    Ok(
        match assessment_repo::get_attempt(conn, &inspect(DataDomain::Learning), &aid)? {
            None => json!({ "exists": false }),
            Some(a) => {
                let decisions = assessment_repo::list_grade_decisions(conn, &aid)?;
                json!({
                    "exists": true,
                    "state": a.state,
                    "revision": a.revision,
                    "grade_decisions": decisions.len(),
                    "final_decisions": decisions.iter().filter(|d| d.is_final).count(),
                    "final_passed": decisions.iter().find(|d| d.is_final).map(|d| d.passed),
                    "final_approval": decisions.iter().find(|d| d.is_final).and_then(|d| d.approval_id.clone()),
                })
            }
        },
    )
}

/// Session state, credited/uncertain time and interval continuity.
pub fn session(h: &Harness, session_id: &str) -> Result<Value, G9Error> {
    let sid: SessionId = id(session_id)?;
    let conn = h.db()?.conn();
    Ok(
        match sessions_repo::get_session(conn, &inspect(DataDomain::Learning), &sid)? {
            None => json!({ "exists": false }),
            Some(s) => {
                let intervals = sessions_repo::list_intervals(conn, &sid)?;
                json!({
                    "exists": true,
                    "state": s.state,
                    "domain": s.data_domain,
                    "credited_ms": s.credited_ms.as_millis(),
                    "uncertain_ms": s.uncertain_ms.as_millis(),
                    "revision": s.revision,
                    "intervals": intervals.iter().map(|i| json!({
                        "continuity": i.record.continuity,
                        "closed_by": i.closed_by,
                        "open": i.record.end_wall.is_none(),
                    })).collect::<Vec<_>>(),
                })
            }
        },
    )
}

/// Invocation metadata rows of one skill, as `g9-storage` lists them
/// (`requested_at` descending, ties by id). Assert on counts, not positions,
/// when several invocations share an instant.
pub fn invocations(h: &Harness, skill_name: &str) -> Result<Vec<Value>, G9Error> {
    let sid = SkillId::parse(skill_name)
        .map_err(|_| G9Error::invalid_input("probe.id_invalid", "skill id is not valid"))?;
    Ok(
        knowledge_repo::list_invocations_for_skill(h.db()?.conn(), &sid, 100)?
            .into_iter()
            .map(|r| {
                json!({
                    "state": r.state,
                    "actor_id": r.actor_id,
                    "has_output_hash": r.output_sha256.is_some(),
                    "validation": r.validation_outcome,
                    "error_category": r.error_category,
                })
            })
            .collect(),
    )
}

/// Domain event kinds in ledger order.
pub fn event_kinds(h: &Harness) -> Result<Vec<String>, G9Error> {
    Ok(events::list_since(h.db()?.conn(), 0, 10_000)?
        .into_iter()
        .map(|e| e.event.header.kind)
        .collect())
}

/// Whether any domain event payload contains `needle` (content-leak check).
pub fn events_contain(h: &Harness, needle: &str) -> Result<bool, G9Error> {
    Ok(events::list_since(h.db()?.conn(), 0, 10_000)?
        .iter()
        .any(|e| e.event.payload.to_string().contains(needle)))
}

/// Audit chain verification.
pub fn audit(h: &Harness) -> Result<Value, G9Error> {
    let r = AuditChain::verify_chain(h.db()?.conn())?;
    Ok(json!({ "rows": r.rows, "ok": r.ok, "first_bad_seq": r.first_bad_seq }))
}

/// Calendar events stored over all time (any domain).
pub fn calendar_event_count(h: &Harness) -> Result<usize, G9Error> {
    Ok(calendar_repo::list_events_between(
        h.db()?.conn(),
        &inspect(DataDomain::Learning),
        UtcMillis::from_millis(0),
        UtcMillis::from_millis(i64::MAX / 2),
    )?
    .len())
}

/// Observation ids of a competency (including superseded ones).
pub fn observation_ids(h: &Harness, competency_id: &str) -> Result<Vec<String>, G9Error> {
    let cid: CompetencyId = id(competency_id)?;
    Ok(competency_repo::list_observations(h.db()?.conn(), &cid)?
        .into_iter()
        .map(|o| o.id.as_str().to_string())
        .collect())
}

/// Newest persisted policy decision for a resource: `(kind, rule_id, reason)`.
pub fn latest_policy_decision(
    h: &Harness,
    resource: &ResourceRef,
) -> Result<Option<(String, String, String)>, G9Error> {
    let rows = approvals_store::list_policy_decisions_for(h.db()?.conn(), resource, 1)?;
    Ok(rows.into_iter().next().map(|r| match r.decision {
        Decision::Allow { rule_id } => ("ALLOW".to_string(), rule_id, String::new()),
        Decision::Deny { rule_id, reason } => ("DENY".to_string(), rule_id, reason),
        Decision::ApprovalRequired {
            rule_id, reason, ..
        } => ("APPROVAL_REQUIRED".to_string(), rule_id, reason),
    }))
}

/// Number of persisted policy decisions for a resource (bounded at 1000).
pub fn policy_decision_count(h: &Harness, resource: &ResourceRef) -> Result<usize, G9Error> {
    Ok(approvals_store::list_policy_decisions_for(h.db()?.conn(), resource, 1_000)?.len())
}

/// Stored versions of the `g9-ct9-480` curriculum (provenance, current flag, approval).
pub fn curriculum_versions(h: &Harness) -> Result<Vec<Value>, G9Error> {
    let cid = g9_domain::CurriculumId::parse("g9-ct9-480")
        .map_err(|_| G9Error::internal_redacted("probe.curriculum_id"))?;
    Ok(
        g9_storage::repos::curriculum::list_versions(h.db()?.conn(), &cid)?
            .into_iter()
            .map(|v| {
                json!({
                    "id": v.id,
                    "provenance": v.provenance,
                    "is_current": v.is_current,
                    "approval_id": v.approval_id,
                })
            })
            .collect(),
    )
}
