#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! POL-2 (`docs/release/path-to-10.md` §5.3, RTM R-005): registry completeness.
//!
//! Every operation in `g9_app::ops::OPERATIONS` is driven through `g9_app::dispatch` with a
//! minimal valid payload, and the rows the pipeline leaves are read back by request id. The
//! tests iterate the registry itself, so an operation added without a handler, a policy rule,
//! a payload fixture or a client registry entry fails here.
//!
//! What is asserted, per operation (the rules are those of `ARCHITECTURE.md` §5 and §11 as
//! `crates/g9-app/src/pipeline.rs` implements them):
//!
//! * **Registry parity**: a handler (`commands::handler_for`), a payload fixture
//!   ([`payload`]), and an entry in the client registry `apps/desktop/src/ipc/operations.ts`
//!   with the same action, resource kind and mutating flag. The client entry supplies the
//!   declared scope (default domain and mode) and whether an approval is expected.
//! * **A policy rule**: on success exactly one operator `policy_decisions` row for the request,
//!   `ALLOW`, by a rule of the built-in table that is not `default.deny`, in the declared scope.
//! * **Audit on success**: a mutating operation appends exactly one hash-chained
//!   `audit_metadata` row (`ALLOW`, `SUCCESS`, operation = its name), and a second one named
//!   `<op>.post_commit` when its work runs after the commit (backup.create, deletion.request);
//!   the outcome's `audit_ref` names the last of them. A read-only operation appends no audit
//!   row (§5: reads skip steps 4 to 9) and its outcome carries no `audit_ref`.
//! * **Refusal rows**: an approval-expected operation refuses without an approval with
//!   `APPROVAL_REQUIRED` and an approval id, a `policy_decisions` row `APPROVAL_REQUIRED`, an
//!   audit row `APPROVAL_REQUIRED`/`APPROVAL_REQUIRED` and a `PENDING` approval request bound to
//!   its action and resource kind. Every other mutating operation that the policy can refuse is
//!   refused in a mode the built-in table denies, leaving a `policy_decisions` row `DENY` and an
//!   audit row `DENY`/`UNAUTHORIZED`. Every mutating operation refused before evaluation (an
//!   idempotency key reused for another request) leaves an audit row
//!   `NOT_EVALUATED`/`CONFLICT` and no decision row, because nothing was evaluated. Reads cannot
//!   be refused by policy for the operator (`operator.read.allow` has no condition), which is
//!   asserted against the engine for every mode and domain.
//! * **Derived interaction mode**: while an EXAM attempt is open, every mutating operation that
//!   is not exam-exempt is evaluated in EXAM whatever mode was declared; reads and the
//!   exam-exempt backup operations are evaluated as declared (`commands::effective_meta`).

mod common;

use common::*;
use g9_ai_adapter::{ModelRef, ScriptedRuntime};
use g9_app::ops::{OpSpec, OPERATIONS};
use g9_domain::{
    Action, ActorId, ApprovalId, ApprovalState, AssessmentAttemptState, DataDomain, Decision,
    ErrorCategory, G9Error, InteractionMode, ModelId, OperationDescriptor, ResourceRef, Scope,
    SessionId, UtcMillis,
};
use g9_policy::{Engine, FixedClock, ResourceContext, RuleSet, DEFAULT_DENY_RULE_ID};
use g9_storage::repos::ReadScope;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

const DAY: i64 = 86_400_000;

/// Operations whose work runs after the command transaction committed, so that a success
/// leaves a second audit row `<op>.post_commit` (`pipeline::run_post_commit`).
const POST_COMMIT_OPS: [&str; 2] = ["backup.create", "deletion.request"];

/// Operations evaluated as declared while an EXAM attempt is open (`OpSpec::exam_exempt`).
const EXAM_EXEMPT_OPS: [&str; 3] = ["backup.create", "backup.verify", "backup.restore_test"];

/// Mutating operations without an approval gate that the policy never refuses to the operator
/// in the state their fixture needs, with the reason. Their refusal is shown by the
/// not-evaluated path (idempotency conflict) only.
const NEVER_DENIED_TO_OPERATOR: [(&str, &str); 4] = [
    (
        "calendar.propose",
        "operator.propose.allow: a proposal is allowed in every mode and domain",
    ),
    (
        "approvals.decide",
        "approval.decide.allow: deciding an approval is allowed in every mode and domain",
    ),
    (
        "skills.invoke",
        "operator.invoke.allow: invoking a skill is allowed in every mode and domain (the skill's own modes are checked by the orchestrator)",
    ),
    (
        "assessment.submit",
        "its attempt is the open EXAM attempt, so it is always evaluated in EXAM, where learning writes are allowed",
    ),
];

const ALL_MODES: [InteractionMode; 7] = [
    InteractionMode::Executive,
    InteractionMode::Maintenance,
    InteractionMode::Research,
    InteractionMode::Lab,
    InteractionMode::Review,
    InteractionMode::Study,
    InteractionMode::Exam,
];

const ALL_DOMAINS: [DataDomain; 5] = [
    DataDomain::Learning,
    DataDomain::Business,
    DataDomain::Personal,
    DataDomain::SecuritySensitive,
    DataDomain::System,
];

// ---------------------------------------------------------------------------
// The client registry (apps/desktop/src/ipc/operations.ts)
// ---------------------------------------------------------------------------

const UI_OPERATIONS_TS: &str = include_str!("../../../apps/desktop/src/ipc/operations.ts");

/// One entry of the client's `OPERATIONS` table.
#[derive(Debug, Clone, PartialEq, Eq)]
struct UiSpec {
    action: String,
    kind: String,
    mutating: bool,
    domain: String,
    mode: String,
    approval_expected: bool,
}

/// Read `OPERATIONS` of `operations.ts`: `'<name>': spec('<name>', '<ACTION>', '<KIND>', { ... })`
/// with the defaults of its `spec` function (mutating, LEARNING, STUDY, no approval).
fn ui_registry() -> BTreeMap<String, UiSpec> {
    let start = UI_OPERATIONS_TS
        .find("export const OPERATIONS")
        .expect("operations.ts declares `export const OPERATIONS`");
    let tail = &UI_OPERATIONS_TS[start..];
    let end = tail.find("\n};").expect("the OPERATIONS table is closed");
    let body: String = tail[..end]
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let mut out = BTreeMap::new();
    let mut rest = body.as_str();
    while let Some(at) = rest.find("': spec(") {
        let name_start = rest[..at].rfind('\'').expect("quoted name") + 1;
        let name = rest[name_start..at].to_string();
        let args_start = at + "': spec(".len();
        let mut depth = 1usize;
        let mut args_end = None;
        for (i, c) in rest[args_start..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        args_end = Some(args_start + i);
                        break;
                    }
                }
                _ => {}
            }
        }
        let args_end = args_end.expect("spec( is closed");
        let args = &rest[args_start..args_end];
        let quoted: Vec<&str> = args.split('\'').skip(1).step_by(2).collect();
        assert!(quoted.len() >= 3, "{name}: spec(name, action, kind)");
        assert_eq!(quoted[0], name, "the key and the spec name agree");
        let option = |key: &str| -> Option<String> {
            let marker = format!("{key}: ");
            args.find(&marker).map(|p| {
                args[p + marker.len()..]
                    .split([',', '}', '\n'])
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_matches('\'')
                    .to_string()
            })
        };
        let spec = UiSpec {
            action: quoted[1].to_string(),
            kind: quoted[2].to_string(),
            mutating: option("mutating").is_none_or(|v| v != "false"),
            domain: option("defaultDomain").unwrap_or_else(|| "LEARNING".into()),
            mode: option("defaultMode").unwrap_or_else(|| "STUDY".into()),
            approval_expected: option("approvalExpected").is_some_and(|v| v == "true"),
        };
        assert!(
            out.insert(name.clone(), spec).is_none(),
            "{name} listed twice"
        );
        rest = &rest[args_end..];
    }
    out
}

/// The client entry of `name`; an operation the client does not list fails here by name.
fn ui_spec<'a>(ui: &'a BTreeMap<String, UiSpec>, name: &str) -> &'a UiSpec {
    ui.get(name)
        .unwrap_or_else(|| panic!("{name}: not in apps/desktop/src/ipc/operations.ts"))
}

fn wire<T: serde::Serialize>(v: T) -> String {
    serde_json::to_value(v)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

fn from_wire<T: serde::de::DeserializeOwned>(s: &str) -> T {
    serde_json::from_value(json!(s)).unwrap()
}

// ---------------------------------------------------------------------------
// Harness and rows
// ---------------------------------------------------------------------------

/// A valid T01 brief: no recommended action, so `skills.invoke` completes.
fn brief_output() -> Value {
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

/// The desktop layout (attachments, backups, scratch and exports are approved roots), the
/// fixture skill packages and a scripted model with one valid answer.
fn harness() -> Harness {
    let dir = tempfile::Builder::new().prefix("g9pol2").tempdir().unwrap();
    let root = dir.path().to_path_buf();
    for sub in ["attachments", "backups", "scratch", "exports"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    let runtime = Arc::new(ScriptedRuntime::new(
        ModelRef::new(ModelId::parse("scripted-model").unwrap()),
        1_000_000,
    ));
    let out = brief_output();
    runtime.push_response(runtime.response(out.to_string(), Some(out)));
    let rt: Arc<dyn g9_ai_adapter::InferenceRuntime> = runtime;
    let registry = g9_skills::Registry::load(&skills_fixture_root()).unwrap();
    let mut h = Harness::with(move |s| {
        s.config.approved_roots = ["attachments", "backups", "scratch", "exports"]
            .iter()
            .map(|d| root.join(d))
            .collect();
        s.config.attachment_dir = Some(root.join("attachments"));
        s.config.backup_dir = Some(root.join("backups"));
        s.config.scratch_dir = Some(root.join("scratch"));
        s.config.export_dir = Some(root.join("exports"));
        s.runtime = Some(rt);
        s.skills = Some(registry);
    });
    h.dir = Some(dir);
    h
}

/// One dispatched call and the request id its rows carry.
struct Call {
    request_id: String,
    out: Result<Value, G9Error>,
}

fn call(
    h: &mut Harness,
    op: &str,
    payload: &Value,
    scope: (&str, &str),
    key: &str,
    approval: Option<&str>,
) -> Call {
    let mut meta = h.meta(
        op,
        &Opts {
            idempotency_key: Some(key.to_string()),
            approval_id: approval.map(str::to_string),
            ..Opts::default()
        },
    );
    meta["scope"] = json!({ "domain": scope.0, "mode": scope.1 });
    let request_id = meta["request_id"].as_str().unwrap().to_string();
    let out = h.raw(op, meta, payload.clone());
    Call { request_id, out }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DecisionRow {
    actor_kind: String,
    action: String,
    resource_kind: String,
    scope_domain: String,
    scope_mode: String,
    decision: String,
    rule_id: String,
    required_capability: Option<String>,
    approval_id: Option<String>,
}

/// The operator's `policy_decisions` rows of one request for this operation's action and kind.
fn decisions(h: &Harness, spec: &OpSpec, request_id: &str) -> Vec<DecisionRow> {
    let mut stmt = h
        .conn()
        .prepare(
            "SELECT actor_kind, action, resource_kind, scope_domain, scope_mode, decision, rule_id,
                    required_capability, approval_id
               FROM policy_decisions WHERE request_id = ?1 ORDER BY decided_at, rowid",
        )
        .unwrap();
    let rows = stmt
        .query_map([request_id], |r| {
            Ok(DecisionRow {
                actor_kind: r.get(0)?,
                action: r.get(1)?,
                resource_kind: r.get(2)?,
                scope_domain: r.get(3)?,
                scope_mode: r.get(4)?,
                decision: r.get(5)?,
                rule_id: r.get(6)?,
                required_capability: r.get(7)?,
                approval_id: r.get(8)?,
            })
        })
        .unwrap()
        .map(Result::unwrap)
        .collect::<Vec<_>>();
    rows.into_iter()
        .filter(|d| {
            d.actor_kind == "OPERATOR"
                && d.action == wire(spec.action)
                && d.resource_kind == wire(spec.resource_kind)
        })
        .collect()
}

/// Every `policy_decisions` row of one request, whoever the actor.
fn all_decisions(h: &Harness, request_id: &str) -> i64 {
    h.conn()
        .query_row(
            "SELECT COUNT(*) FROM policy_decisions WHERE request_id = ?1",
            [request_id],
            |r| r.get(0),
        )
        .unwrap()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AuditRow {
    id: String,
    operation: String,
    decision: String,
    outcome: String,
}

fn audits(h: &Harness, request_id: &str) -> Vec<AuditRow> {
    let mut stmt = h
        .conn()
        .prepare(
            "SELECT id, operation, decision, outcome FROM audit_metadata WHERE request_id = ?1 ORDER BY seq",
        )
        .unwrap();
    stmt.query_map([request_id], |r| {
        Ok(AuditRow {
            id: r.get(0)?,
            operation: r.get(1)?,
            decision: r.get(2)?,
            outcome: r.get(3)?,
        })
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

fn audit_shape(rows: &[AuditRow]) -> Vec<(String, String, String)> {
    rows.iter()
        .map(|a| (a.operation.clone(), a.decision.clone(), a.outcome.clone()))
        .collect()
}

fn engine() -> Engine {
    Engine::builtin_v1(Box::new(FixedClock(UtcMillis::from_millis(T0))))
}

/// The engine's decision for the operator on `spec` in `mode` and `domain`, for a resource of
/// that domain that is INTERNAL and not finalized.
fn predicted(spec: &OpSpec, mode: InteractionMode, domain: DataDomain) -> Decision {
    let operator = g9_domain::Actor::operator(ActorId::parse("operator-1").unwrap());
    let descriptor = OperationDescriptor {
        action: spec.action,
        resource: ResourceRef::new(spec.resource_kind, "pol2-resource"),
        scope: Scope { domain, mode },
        parameters_sha256: "0".repeat(64),
    };
    let ctx = ResourceContext::new(domain, g9_domain::Classification::Internal, false);
    engine().evaluate(&operator, &descriptor, &ctx).decision
}

/// Whether an EXAM-mode attempt is `STARTED` (the backend's derivation rule, TEC-25).
fn exam_open(h: &Harness) -> bool {
    g9_storage::repos::assessment::list_attempts_by_state(
        h.conn(),
        &ReadScope::cross_domain(DataDomain::Learning),
        AssessmentAttemptState::Started,
    )
    .unwrap()
    .iter()
    .any(|a| a.mode == InteractionMode::Exam)
}

// ---------------------------------------------------------------------------
// Fixture builders (state a payload needs, built through dispatch where possible)
// ---------------------------------------------------------------------------

fn result_str(v: &Value, key: &str) -> String {
    v["result"][key]
        .as_str()
        .unwrap_or_else(|| panic!("{key} in {v}"))
        .to_string()
}

fn note(h: &mut Harness) -> String {
    let created = h
        .call(
            "notes.create",
            json!({ "title": "Registry note", "body_markdown": "Lab write-up.", "classification": "INTERNAL", "domain": "LEARNING" }),
        )
        .unwrap();
    result_str(&created, "note_id")
}

fn import_seed(h: &mut Harness) {
    h.approved(
        "curriculum.import",
        json!({ "manifest": seed_manifest(), "provenance": "PROPOSED_SEED" }),
        &Opts::default(),
    );
}

/// The seed imported and the program started on Monday 2026-11-23.
fn program_started(h: &mut Harness) {
    import_seed(h);
    h.call(
        "curriculum.set_program_start",
        json!({ "date": "2026-11-23" }),
    )
    .unwrap();
}

/// The seed with a reconciliation block that matches it (a RECONCILED_CANDIDATE).
fn reconciled_candidate(original: &Value) -> Value {
    let mut candidate = original.clone();
    candidate["version"] = json!("0.2.0-reconciled");
    candidate["provenance"] = json!("RECONCILED_CANDIDATE");
    let original_m: g9_curriculum::Manifest = serde_json::from_value(original.clone()).unwrap();
    candidate["reconciliation"] = json!({
        "original_checksum": "0".repeat(64),
        "diff_checksum": "0".repeat(64),
        "approved_by": "operator-1",
        "approved_at": T0,
    });
    let cand_m: g9_curriculum::Manifest = serde_json::from_value(candidate.clone()).unwrap();
    let report = g9_curriculum::reconcile(&original_m, &cand_m).unwrap();
    candidate["reconciliation"]["original_checksum"] = json!(report.original_checksum.as_str());
    candidate["reconciliation"]["diff_checksum"] = json!(report.diff_checksum.as_str());
    candidate
}

const SESSION: &str = "pol2-session";

fn session_in(h: &mut Harness, steps: &[&str]) -> Value {
    insert_planned_session(h, SESSION);
    for step in steps {
        if *step == "advance" {
            h.advance(10 * 60_000);
        } else {
            h.call(step, json!({ "session_id": SESSION })).unwrap();
        }
    }
    json!({ "session_id": SESSION })
}

fn competency(h: &mut Harness, code: &str) -> String {
    let created = h
        .call(
            "competency.create",
            json!({ "code": code, "title": "Registry competency", "validity_days": 120 }),
        )
        .unwrap();
    result_str(&created, "competency_id")
}

fn define_payload(competency_id: &str, code: &str) -> Value {
    json!({
        "code": code,
        "title": "Registry practical",
        "kind": "BLIND_PRACTICAL",
        "competency_id": competency_id,
        "task_statement": "Recover the service from the supplied snapshot.",
        "difficulty": 3
    })
}

fn assessment(h: &mut Harness) -> String {
    let cid = competency(h, "POL2C");
    let defined = h
        .call("assessment.define", define_payload(&cid, "POL2-A"))
        .unwrap();
    result_str(&defined, "assessment_id")
}

/// An attempt of an authored practical driven through `steps`.
fn attempt(h: &mut Harness, steps: &[&str]) -> String {
    let def = assessment(h);
    let begun = h
        .call("assessment.begin", json!({ "assessment_id": def }))
        .unwrap();
    let id = result_str(&begun, "attempt_id");
    for step in steps {
        let payload = match *step {
            "assessment.submit" => json!({ "attempt_id": id, "evidence_refs": [] }),
            "assessment.grade_provisional" => grade_payload(&id),
            _ => json!({ "attempt_id": id }),
        };
        h.call(step, payload).unwrap();
    }
    id
}

fn grade_payload(attempt_id: &str) -> Value {
    let criteria: Vec<Value> = g9_assessment::Rubric::baseline()
        .criteria()
        .iter()
        .map(|c| json!({ "criterion_code": c.code, "score": 4, "source": "HUMAN_REVIEWER", "evidence_refs": [] }))
        .collect();
    json!({ "attempt_id": attempt_id, "criteria": criteria })
}

fn backup(h: &mut Harness, steps: &[&str]) -> Value {
    let created = h.call("backup.create", json!({})).unwrap();
    let id = result_str(&created, "backup_id");
    for step in steps {
        h.call(step, json!({ "backup_id": id })).unwrap();
    }
    json!({ "backup_id": id })
}

fn event(h: &mut Harness) -> String {
    let start = T0 + 2 * 3_600_000;
    let created = h
        .call(
            "calendar.create_event",
            json!({ "title": "Registry event", "kind": "APPOINTMENT", "start": start, "end": start + 3_600_000 }),
        )
        .unwrap();
    result_str(&created, "event_id")
}

/// The minimal valid payload of `op`, after building the state it needs. One row per
/// operation; `None` for an operation without a fixture (a registry entry added without one
/// fails `every_registered_operation_has_a_handler_a_payload_fixture_and_a_client_entry`).
fn payload(op: &str, h: &mut Harness) -> Option<Value> {
    Some(match op {
        // ---- reads ----
        "system.status" => json!({}),
        "curriculum.validate" => json!({ "manifest": seed_manifest() }),
        "curriculum.daily_plan" => {
            program_started(h);
            json!({ "date": "2026-11-24" })
        }
        "curriculum.program" => json!({}),
        "calendar.list" => json!({ "from": T0, "to": T0 + DAY }),
        "session.list" => json!({}),
        "assessment.list" => json!({}),
        "competency.projection" => json!({}),
        "retention.due" => json!({}),
        "interventions.list" => json!({}),
        "approvals.list" => json!({}),
        "audit.recent" => json!({}),
        "evidence.list" => json!({}),
        "notes.list" => json!({}),
        "notes.get" => json!({ "note_id": note(h) }),
        "backup.list" => json!({}),
        // ---- curriculum ----
        "curriculum.import" => {
            json!({ "manifest": seed_manifest(), "provenance": "PROPOSED_SEED" })
        }
        "curriculum.promote" => {
            let original = seed_manifest();
            let candidate = reconciled_candidate(&original);
            let imported = h.approved(
                "curriculum.import",
                json!({ "manifest": candidate, "provenance": "RECONCILED_CANDIDATE" }),
                &Opts::default(),
            );
            json!({
                "curriculum_version_id": result_str(&imported, "curriculum_version_id"),
                "candidate_manifest": candidate,
                "original_manifest": original,
            })
        }
        "curriculum.set_program_start" => {
            import_seed(h);
            json!({ "date": "2026-11-23" })
        }
        "session.plan_day" => {
            program_started(h);
            json!({ "date": "2026-11-24" })
        }
        // ---- calendar ----
        "calendar.propose" => json!({ "from": T0, "to": T0 + DAY, "intent": "ADD_REVIEWS" }),
        "calendar.apply" => {
            let proposed = h
                .call(
                    "calendar.propose",
                    json!({ "from": T0, "to": T0 + DAY, "intent": "ADD_REVIEWS" }),
                )
                .unwrap();
            json!({ "proposal_id": result_str(&proposed, "proposal_id") })
        }
        "calendar.create_event" => {
            let start = T0 + 3 * 3_600_000;
            json!({ "title": "Registry appointment", "kind": "APPOINTMENT", "start": start, "end": start + 3_600_000 })
        }
        "calendar.update_event" => {
            json!({ "event_id": event(h), "title": "Registry event (moved)" })
        }
        "calendar.delete_event" => json!({ "event_id": event(h) }),
        // ---- sessions ----
        "session.create" => json!({ "planned_minutes": 30 }),
        "session.prepare" => session_in(h, &[]),
        "session.start" => session_in(h, &["session.prepare"]),
        "session.pause" => session_in(h, &["session.prepare", "session.start", "advance"]),
        "session.resume" => session_in(
            h,
            &[
                "session.prepare",
                "session.start",
                "advance",
                "session.pause",
            ],
        ),
        "session.complete" => session_in(h, &["session.prepare", "session.start", "advance"]),
        "session.correct_time" => {
            session_in(h, &["session.prepare", "session.start", "advance"]);
            // A restart leaves the open interval UNCERTAIN; only those can be corrected.
            h.restart();
            let intervals = g9_storage::repos::sessions::list_intervals(
                h.conn(),
                &SessionId::parse(SESSION).unwrap(),
            )
            .unwrap();
            json!({
                "session_id": SESSION,
                "interval_id": intervals[0].id,
                "credited_ms": 60_000,
                "reason": "verified work before the restart",
            })
        }
        // ---- notes and evidence ----
        "notes.create" => {
            json!({ "title": "Registry note", "body_markdown": "Body.", "classification": "INTERNAL", "domain": "LEARNING" })
        }
        "notes.revise" => json!({ "note_id": note(h), "body_markdown": "Revised body." }),
        "evidence.attach_file" => {
            json!({ "filename": "registry.txt", "media_type": "text/plain", "content_base64": "cmVnaXN0cnkgZmlsZQ==" })
        }
        "evidence.from_note" => json!({ "note_id": note(h) }),
        // ---- assessment ----
        "assessment.define" => {
            let cid = competency(h, "POL2D");
            define_payload(&cid, "POL2-D")
        }
        "assessment.begin" => json!({ "assessment_id": assessment(h) }),
        "assessment.submit" => {
            json!({ "attempt_id": attempt(h, &[]), "evidence_refs": [] })
        }
        "assessment.review_evidence" => {
            json!({ "attempt_id": attempt(h, &["assessment.submit"]) })
        }
        "assessment.grade_provisional" => grade_payload(&attempt(
            h,
            &["assessment.submit", "assessment.review_evidence"],
        )),
        "assessment.approve_final" => json!({
            "attempt_id": attempt(
                h,
                &["assessment.submit", "assessment.review_evidence", "assessment.grade_provisional"],
            )
        }),
        // ---- competency and retention ----
        "competency.create" => json!({ "code": "POL2N", "title": "Registry competency" }),
        "competency.import_pack" => json!({}),
        "retention.create_item" => {
            json!({ "prompt": "What does HSTS prevent?", "answer": "A downgrade to plain HTTP." })
        }
        "retention.record_review" => {
            let created = h
                .call(
                    "retention.create_item",
                    json!({ "prompt": "Name the TLS 1.3 key exchange.", "answer": "(EC)DHE" }),
                )
                .unwrap();
            json!({ "review_item_id": result_str(&created, "review_item_id"), "outcome": "SUCCESS" })
        }
        // ---- approvals, audit, backup, skills, privacy ----
        "approvals.decide" => {
            let refused = h
                .call(
                    "export.request",
                    json!({ "domains": ["LEARNING"], "format": "JSON", "include_private": false }),
                )
                .unwrap_err();
            let id = refused.approval_id.expect("an approval id");
            json!({ "approval_id": id.as_str(), "decision": "REJECT" })
        }
        "backup.create" => json!({ "label": "registry" }),
        "backup.verify" => backup(h, &[]),
        "backup.restore_test" => backup(h, &["backup.verify"]),
        "skills.invoke" => json!({
            "skill_code": "T01",
            "input": { "day_id": "D15", "mode": "STUDY", "learner_context_ref": "snapshot:week03-day15" }
        }),
        "export.request" => {
            json!({ "domains": ["LEARNING"], "format": "JSON", "include_private": false })
        }
        "deletion.request" => {
            json!({ "resource": { "kind": "NOTE", "id": note(h) }, "reason": "registry fixture" })
        }
        _ => return None,
    })
}

/// A started EXAM attempt (the seeded blind practical of the shared harness).
fn open_exam(h: &mut Harness) {
    let def = seed_blind_practical(h, 1, Some(3));
    let begun = h
        .call("assessment.begin", json!({ "assessment_id": def.as_str() }))
        .unwrap();
    assert_eq!(begun["result"]["mode"], "EXAM");
    assert!(exam_open(h));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn every_registered_operation_has_a_handler_a_payload_fixture_and_a_client_entry() {
    let ui = ui_registry();
    let mut problems = Vec::new();
    for spec in OPERATIONS {
        if g9_app::commands::handler_for(spec.name).is_none() {
            problems.push(format!("{}: no handler", spec.name));
        }
        if payload(spec.name, &mut harness()).is_none() {
            problems.push(format!("{}: no payload fixture", spec.name));
        }
        match ui.get(spec.name) {
            None => problems.push(format!("{}: not in operations.ts", spec.name)),
            Some(u) => {
                let backend = (wire(spec.action), wire(spec.resource_kind), spec.mutating);
                let client = (u.action.clone(), u.kind.clone(), u.mutating);
                if backend != client {
                    problems.push(format!(
                        "{}: backend {backend:?} but operations.ts {client:?}",
                        spec.name
                    ));
                }
                // The shared harness declares the same default scope as the client.
                let harness_scope = default_scope(spec.name);
                if (harness_scope.0, harness_scope.1) != (u.domain.as_str(), u.mode.as_str()) {
                    problems.push(format!(
                        "{}: harness scope {harness_scope:?}, operations.ts ({}, {})",
                        spec.name, u.domain, u.mode
                    ));
                }
            }
        }
    }
    let names: BTreeSet<&str> = OPERATIONS.iter().map(|s| s.name).collect();
    for name in ui.keys() {
        if !names.contains(name.as_str()) {
            problems.push(format!("{name}: in operations.ts but not routed"));
        }
    }
    // An operation the client expects to be gated maps back to itself from its approval
    // (`ops::name_for_approval`, which labels pending approvals for the confirmation).
    for spec in OPERATIONS {
        if ui.get(spec.name).is_some_and(|u| u.approval_expected) {
            let label = g9_app::ops::name_for_approval(spec.action, spec.resource_kind);
            if label != spec.name {
                problems.push(format!("{}: its approvals are labelled {label}", spec.name));
            }
        }
    }
    let exempt: BTreeSet<&str> = OPERATIONS
        .iter()
        .filter(|s| s.exam_exempt)
        .map(|s| s.name)
        .collect();
    if exempt != EXAM_EXEMPT_OPS.into_iter().collect() {
        problems.push(format!("exam-exempt operations changed: {exempt:?}"));
    }
    assert!(problems.is_empty(), "{problems:#?}");
    assert_eq!(ui.len(), OPERATIONS.len());
}

/// Reads are never refused by policy to the operator, in any mode or domain: the reason a
/// read leaves no refusal row (its success row is asserted below).
#[test]
fn no_read_is_refused_to_the_operator_by_policy_in_any_mode_or_domain() {
    let rules = RuleSet::builtin_v1();
    let builtin: BTreeSet<&str> = rules.rule_ids().into_iter().collect();
    for spec in OPERATIONS.iter().filter(|s| !s.mutating) {
        assert_eq!(spec.action, Action::Read, "{}", spec.name);
        for mode in ALL_MODES {
            for domain in ALL_DOMAINS {
                match predicted(spec, mode, domain) {
                    Decision::Allow { rule_id } => {
                        assert_ne!(rule_id, DEFAULT_DENY_RULE_ID);
                        assert!(builtin.contains(rule_id.as_str()), "{rule_id}");
                    }
                    other => panic!("{} in {mode:?}/{domain:?}: {other:?}", spec.name),
                }
            }
        }
    }
}

/// The per-operation pipeline contract: rule, audit on success, rows on refusal, approval gate.
#[test]
fn every_operation_passes_the_pipeline_with_its_rule_audit_and_refusal_rows() {
    let ui = ui_registry();
    let rules = RuleSet::builtin_v1();
    let builtin: BTreeSet<&str> = rules.rule_ids().into_iter().collect();
    let mut problems: Vec<String> = Vec::new();
    let mut never_denied = BTreeSet::new();
    let mut approval_gated = BTreeSet::new();
    let mut post_commit = BTreeSet::new();
    let mut rules_seen = BTreeSet::new();
    for spec in OPERATIONS {
        let name = spec.name;
        let u = ui_spec(&ui, name);
        let declared = (u.domain.as_str(), u.mode.as_str());
        let declared_domain: DataDomain = from_wire(&u.domain);
        let mut h = harness();
        let payload = payload(name, &mut h).unwrap_or_else(|| panic!("{name}: no payload fixture"));
        let mut fail = |what: String| problems.push(format!("{name}: {what}"));

        // ---- 1. a policy refusal, where the policy can refuse ----
        if spec.mutating && !u.approval_expected {
            let effective = |m: InteractionMode| {
                if !spec.exam_exempt && exam_open(&h) {
                    InteractionMode::Exam
                } else {
                    m
                }
            };
            let deny = ALL_MODES.into_iter().find_map(|m| {
                match predicted(spec, effective(m), declared_domain) {
                    Decision::Deny { rule_id, .. } => Some((m, rule_id)),
                    _ => None,
                }
            });
            match deny {
                None => {
                    never_denied.insert(name);
                }
                Some((mode, rule)) => {
                    let audit_before = h.audit_rows();
                    let pending_before = approvals_pending(&h);
                    let c = call(
                        &mut h,
                        name,
                        &payload,
                        (declared.0, &wire(mode)),
                        &format!("pol2-{name}-deny"),
                        None,
                    );
                    match &c.out {
                        Err(e) if e.category == ErrorCategory::Unauthorized => {}
                        other => fail(format!("refusal in {mode:?} expected, got {other:?}")),
                    }
                    let d = decisions(&h, spec, &c.request_id);
                    if d.len() != 1 || d[0].decision != "DENY" || d[0].rule_id != rule {
                        fail(format!(
                            "refusal decision rows {d:?}, expected one DENY by {rule}"
                        ));
                    }
                    let a = audit_shape(&audits(&h, &c.request_id));
                    if a != [(name.to_string(), "DENY".into(), "UNAUTHORIZED".into())] {
                        fail(format!("refusal audit rows {a:?}"));
                    }
                    if h.audit_rows() != audit_before + 1 {
                        fail("a refusal appended more than its one audit row".into());
                    }
                    if approvals_pending(&h) != pending_before {
                        fail("a DENY created an approval request".into());
                    }
                }
            }
        }

        // ---- 2. the approval gate ----
        let ok_key = format!("pol2-{name}-ok");
        let mut approval: Option<String> = None;
        if u.approval_expected {
            approval_gated.insert(name);
            let c = call(&mut h, name, &payload, declared, &ok_key, None);
            match &c.out {
                Err(e) if e.category == ErrorCategory::ApprovalRequired => match &e.approval_id {
                    Some(id) => approval = Some(id.as_str().to_string()),
                    None => fail("APPROVAL_REQUIRED without an approval id".into()),
                },
                other => fail(format!("APPROVAL_REQUIRED expected, got {other:?}")),
            }
            let d = decisions(&h, spec, &c.request_id);
            if d.len() != 1
                || d[0].decision != "APPROVAL_REQUIRED"
                || d[0].required_capability.is_none()
                || d[0].approval_id != approval
                || !builtin.contains(d[0].rule_id.as_str())
                || (d[0].scope_domain.as_str(), d[0].scope_mode.as_str()) != declared
            {
                fail(format!("approval decision rows {d:?}"));
            }
            let a = audit_shape(&audits(&h, &c.request_id));
            if a != [(
                name.to_string(),
                "APPROVAL_REQUIRED".into(),
                "APPROVAL_REQUIRED".into(),
            )] {
                fail(format!("approval audit rows {a:?}"));
            }
            if let Some(id) = &approval {
                let row = g9_storage::approvals_store::get_request(
                    h.conn(),
                    &ApprovalId::parse(id).unwrap(),
                )
                .unwrap();
                match row {
                    Some(r)
                        if r.state == ApprovalState::Pending
                            && r.operation.action == spec.action
                            && r.operation.resource.kind == spec.resource_kind => {}
                    other => fail(format!("approval request {other:?}")),
                }
                h.decide(id, "APPROVE");
            }
        }

        // ---- 3. success ----
        let audit_before = h.audit_rows();
        let c = call(
            &mut h,
            name,
            &payload,
            declared,
            &ok_key,
            approval.as_deref(),
        );
        let outcome = match &c.out {
            Ok(v) => v.clone(),
            Err(e) => {
                fail(format!("success expected, got {e:?}"));
                continue;
            }
        };
        let d = decisions(&h, spec, &c.request_id);
        match d.as_slice() {
            [row] => {
                if row.decision != "ALLOW"
                    || row.rule_id == DEFAULT_DENY_RULE_ID
                    || !builtin.contains(row.rule_id.as_str())
                    || (row.scope_domain.as_str(), row.scope_mode.as_str()) != declared
                    || row.approval_id != approval
                {
                    fail(format!("success decision row {row:?}"));
                }
                rules_seen.insert(row.rule_id.clone());
            }
            other => fail(format!("one operator decision row expected, got {other:?}")),
        }
        let a = audits(&h, &c.request_id);
        if spec.mutating {
            let mut expected = vec![(name.to_string(), "ALLOW".to_string(), "SUCCESS".to_string())];
            if a.len() == 2 {
                post_commit.insert(name);
                expected.push((
                    format!("{name}.post_commit"),
                    "ALLOW".into(),
                    "SUCCESS".into(),
                ));
            }
            if audit_shape(&a) != expected {
                fail(format!("success audit rows {:?}", audit_shape(&a)));
            }
            if outcome["audit_ref"].as_str() != a.last().map(|r| r.id.as_str()) {
                fail(format!(
                    "audit_ref {} does not name the last audit row",
                    outcome["audit_ref"]
                ));
            }
            if h.audit_rows() != audit_before + a.len() as u64 {
                fail("the success appended audit rows of another request".into());
            }
        } else {
            if !a.is_empty() || h.audit_rows() != audit_before {
                fail(format!("a read appended audit rows {:?}", audit_shape(&a)));
            }
            if !outcome["audit_ref"].is_null() {
                fail(format!(
                    "a read returned audit_ref {}",
                    outcome["audit_ref"]
                ));
            }
        }
        // ---- 4. a refusal before evaluation ----
        if spec.mutating {
            // The same key for another request: another declared domain (the domain is never
            // derived; a changed mode can be derived back to the stored one while an exam
            // is open, which is the same request and replays).
            let other_domain = if u.domain == "SYSTEM" {
                "LEARNING"
            } else {
                "SYSTEM"
            };
            let c = call(
                &mut h,
                name,
                &payload,
                (other_domain, declared.1),
                &ok_key,
                None,
            );
            match &c.out {
                Err(e) if e.code == "idempotency.key_reused" => {}
                other => fail(format!("idempotency conflict expected, got {other:?}")),
            }
            if all_decisions(&h, &c.request_id) != 0 {
                fail("a refusal before evaluation wrote a decision row".into());
            }
            let a = audit_shape(&audits(&h, &c.request_id));
            if a != [(name.to_string(), "NOT_EVALUATED".into(), "CONFLICT".into())] {
                fail(format!("not-evaluated audit rows {a:?}"));
            }
        }
    }
    let expected_never: BTreeSet<&str> = NEVER_DENIED_TO_OPERATOR.iter().map(|(n, _)| *n).collect();
    if never_denied != expected_never {
        problems.push(format!(
            "operations the policy never refuses to the operator: {never_denied:?}, documented {expected_never:?}"
        ));
    }
    if post_commit != POST_COMMIT_OPS.into_iter().collect() {
        problems.push(format!(
            "operations with a post-commit audit row: {post_commit:?}"
        ));
    }
    let gated_client: BTreeSet<&str> = ui
        .iter()
        .filter(|(_, u)| u.approval_expected)
        .map(|(n, _)| n.as_str())
        .collect();
    assert_eq!(approval_gated, gated_client);
    assert!(problems.is_empty(), "{problems:#?}");
    // Every success was decided by a rule of the table: list them for the record.
    assert!(rules_seen.len() >= 5, "{rules_seen:?}");
}

/// Derived interaction mode (TEC-25, SEC-10): with an EXAM attempt open, every mutating
/// operation that is not exam-exempt is evaluated in EXAM whatever mode the client declared;
/// reads and the backup operations are evaluated in the declared mode.
#[test]
fn every_mutating_operation_is_evaluated_in_exam_while_an_exam_is_open() {
    let ui = ui_registry();
    let mut problems = Vec::new();
    for spec in OPERATIONS {
        let name = spec.name;
        let u = ui_spec(&ui, name);
        // A declared mode that is never EXAM, so that the derivation is visible.
        let declared_mode = if u.mode == "EXAM" {
            "STUDY"
        } else {
            u.mode.as_str()
        };
        let mut h = harness();
        let payload = payload(name, &mut h).unwrap_or_else(|| panic!("{name}: no payload fixture"));
        open_exam(&mut h);
        let c = call(
            &mut h,
            name,
            &payload,
            (u.domain.as_str(), declared_mode),
            &format!("pol2-{name}-exam"),
            None,
        );
        let expected = if spec.mutating && !spec.exam_exempt {
            "EXAM"
        } else {
            declared_mode
        };
        let d = decisions(&h, spec, &c.request_id);
        match d.first() {
            Some(row) if row.scope_mode == expected => {}
            Some(row) => problems.push(format!(
                "{name}: evaluated in {} (declared {declared_mode}), expected {expected}",
                row.scope_mode
            )),
            None => problems.push(format!(
                "{name}: not evaluated with an exam open ({:?})",
                c.out.as_ref().err().map(|e| e.code.clone())
            )),
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
}

fn approvals_pending(h: &Harness) -> usize {
    g9_storage::approvals_store::list_by_state(h.conn(), ApprovalState::Pending)
        .unwrap()
        .len()
}
