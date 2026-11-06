#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Shared helpers for the integration tests: fixture paths, a loaded valid
//! package, envelope and output builders. Everything is deterministic.

use g9_domain::skill::PolicyContext;
use g9_domain::{
    Actor, ActorId, Capability, Classification, DataDomain, EvidenceId, IdempotencyKey,
    InteractionMode, Scope, SkillInvocationEnvelope, TraceId, UtcMillis,
};
use g9_policy::FixedClock;
use g9_skills::{
    build_invocation, InferenceBackend, InvocationRequest, Orchestrator, PolicyPreflight, Registry,
    SkillPackage,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

pub fn valid_root() -> PathBuf {
    fixtures().join("valid")
}

pub fn valid_pkg_dir() -> PathBuf {
    valid_root().join("g9-session-director")
}

/// The single package directory inside `tests/fixtures/invalid/<variant>/`.
pub fn invalid_pkg_dir(variant: &str) -> PathBuf {
    let root = fixtures().join("invalid").join(variant);
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("{}: {e}", root.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    assert_eq!(
        dirs.len(),
        1,
        "variant {variant} must hold exactly one package dir"
    );
    dirs.remove(0)
}

pub fn invalid_root(variant: &str) -> PathBuf {
    fixtures().join("invalid").join(variant)
}

pub fn registry() -> Registry {
    Registry::load(&valid_root()).expect("valid fixture registry loads")
}

pub fn package() -> SkillPackage {
    g9_skills::load_package(&valid_pkg_dir()).expect("valid fixture loads")
}

pub fn operator() -> Actor {
    Actor::operator(ActorId::parse("operator-1").unwrap())
}

pub fn now() -> UtcMillis {
    UtcMillis::from_millis(1_762_400_000_000)
}

pub fn scope(mode: InteractionMode) -> Scope {
    Scope {
        domain: DataDomain::Learning,
        mode,
    }
}

pub fn evidence(ids: &[&str]) -> Vec<EvidenceId> {
    ids.iter().map(|s| EvidenceId::parse(s).unwrap()).collect()
}

pub fn payload() -> Value {
    json!({ "day_id": "D15", "mode": "STUDY", "learner_context_ref": "snapshot:week03-day15" })
}

pub fn envelope_with(
    pkg: &SkillPackage,
    actor: Actor,
    mode: InteractionMode,
    payload: Value,
) -> SkillInvocationEnvelope {
    build_invocation(InvocationRequest {
        package: pkg,
        actor,
        intent: "Start my day".into(),
        scope: scope(mode),
        classification: Classification::Private,
        input_payload: payload,
        evidence_refs: evidence(&["ev:obj-D15", "ev:review-7"]),
        policy_context: PolicyContext {
            mode,
            granted_capabilities: vec![Capability::SkillInvoke, Capability::ModelInference],
            recursion_depth: 0,
        },
        idempotency_key: IdempotencyKey::parse("idem-1").unwrap(),
        trace_id: TraceId::parse("trace-1").unwrap(),
    })
    .expect("envelope builds")
}

pub fn envelope(pkg: &SkillPackage) -> SkillInvocationEnvelope {
    envelope_with(pkg, operator(), InteractionMode::Study, payload())
}

/// A fully valid output document for the fixture skill.
pub fn good_output() -> Value {
    json!({
        "status": "COMPLETED",
        "result_schema_version": "1.0",
        "observations": [
            { "statement": "D15 has two objectives", "evidence_refs": ["ev:obj-D15"] }
        ],
        "inferences": [],
        "evidence_refs": ["ev:obj-D15", "ev:review-7"],
        "confidence_limits": ["no interventions resolved"],
        "recommended_actions": [
            { "action": "PROPOSE", "resource": { "kind": "INTERVENTION", "id": "int-1" },
              "rationale": "review item 7 is overdue", "evidence_refs": ["ev:review-7"], "parameters": {} }
        ],
        "approval_requirements": [],
        "resource_usage": { "tokens": 900, "seconds": 3, "tool_calls": 0 },
        "validation_results": [],
        "errors": [],
        "result": {
            "brief": { "day_id": "D15", "objectives": ["D15-O1", "D15-O2"], "due_reviews": ["review-7"],
                       "open_interventions": [], "risks": [] },
            "basis": { "evidence_refs": ["ev:obj-D15", "ev:review-7"], "unresolved_refs": [] }
        }
    })
}

pub fn response(output: Value) -> g9_skills::BackendResponse {
    g9_skills::BackendResponse {
        text: output.to_string(),
        structured: Some(output),
        usage: g9_skills::BackendUsage {
            prompt_tokens: 600,
            output_tokens: 300,
            total_ms: 2_500,
        },
        finish: g9_skills::BackendFinish::Stop,
        model_id: None,
    }
}

pub fn orchestrator(
    policy: Box<dyn PolicyPreflight>,
    backend: Box<dyn InferenceBackend>,
) -> Orchestrator {
    Orchestrator::new(registry(), policy, backend, Box::new(FixedClock(now())))
}
