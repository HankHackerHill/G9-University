//! Invocation envelope construction and JSON-Schema validation of envelopes and
//! skill payloads (ARCHITECTURE.md §10 "Structured output", gate A11).
//!
//! Both runtime envelope schemas are embedded at compile time from
//! `schemas/skill-inputs/g9-invocation-envelope.schema.json` and
//! `schemas/skill-outputs/g9-output-envelope.schema.json`. Validation uses
//! `jsonschema` 0.60 with file/http resolution disabled; package schemas are
//! already checked to contain no `$ref`.
//!
//! Validation messages are built from the JSON-Schema keyword and schema-side
//! data only (property names, limits). The validated instance is never echoed,
//! so private payload content cannot leak into errors or traces.

use crate::package::SkillPackage;
use g9_domain::hashing::sha256_hex;
use g9_domain::skill::PolicyContext;
use g9_domain::{
    Actor, Classification, EvidenceId, IdempotencyKey, InvocationId, Scope,
    SkillInvocationEnvelope, TraceId,
};
use jsonschema::error::ValidationErrorKind;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// `schemas/skill-inputs/g9-invocation-envelope.schema.json` (verbatim).
pub const INVOCATION_ENVELOPE_SCHEMA_JSON: &str =
    include_str!("../../../schemas/skill-inputs/g9-invocation-envelope.schema.json");
/// `schemas/skill-outputs/g9-output-envelope.schema.json` (verbatim).
pub const OUTPUT_ENVELOPE_SCHEMA_JSON: &str =
    include_str!("../../../schemas/skill-outputs/g9-output-envelope.schema.json");

/// Schema version of `input_payload` / `result` envelopes produced by this crate.
pub const SCHEMA_VERSION: &str = "1.0";

/// One schema violation: JSON pointer into the instance plus a content-free message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// JSON pointer (`/input_payload/day_id`), `""` for the root.
    pub path: String,
    /// Keyword-based message; never contains instance values.
    pub message: String,
}

/// Envelope construction failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EnvelopeError {
    /// The embedded envelope schema is not valid JSON (build defect).
    #[error("embedded envelope schema is not valid JSON")]
    SchemaJson,
    /// The derived invocation id did not pass identifier validation (build defect).
    #[error("derived invocation id is not a valid identifier: {0}")]
    InvocationId(String),
    /// The envelope could not be serialized.
    #[error("envelope could not be serialized")]
    Serialize,
}

impl From<EnvelopeError> for g9_domain::G9Error {
    fn from(e: EnvelopeError) -> Self {
        g9_domain::G9Error::internal_redacted(format!(
            "skills.envelope.{}",
            match e {
                EnvelopeError::SchemaJson => "schema_json",
                EnvelopeError::InvocationId(_) => "invocation_id",
                EnvelopeError::Serialize => "serialize",
            }
        ))
    }
}

/// Parsed invocation envelope schema.
pub fn invocation_envelope_schema() -> Result<Value, EnvelopeError> {
    serde_json::from_str(INVOCATION_ENVELOPE_SCHEMA_JSON).map_err(|_| EnvelopeError::SchemaJson)
}

/// Parsed output envelope schema (with the generic `result` placeholder).
pub fn output_envelope_schema() -> Result<Value, EnvelopeError> {
    serde_json::from_str(OUTPUT_ENVELOPE_SCHEMA_JSON).map_err(|_| EnvelopeError::SchemaJson)
}

/// The output envelope schema with `properties.result` replaced by the skill's
/// `schemas/output.schema.json`. The embedded copy drops its own `$schema` and
/// `$id` so that it is a plain subschema of the envelope document.
pub fn compose_output_schema(result_schema: &Value) -> Result<Value, EnvelopeError> {
    let mut envelope = output_envelope_schema()?;
    let mut result = result_schema.clone();
    if let Value::Object(m) = &mut result {
        m.remove("$schema");
        m.remove("$id");
    }
    if let Some(props) = envelope
        .get_mut("properties")
        .and_then(Value::as_object_mut)
    {
        props.insert("result".to_string(), result);
    }
    Ok(envelope)
}

/// Inputs for [`build_invocation`]. A struct rather than ten positional
/// arguments so call sites stay readable and clippy-clean.
#[derive(Debug, Clone, PartialEq)]
pub struct InvocationRequest<'a> {
    /// The validated package being invoked (its budget and version are copied in).
    pub package: &'a SkillPackage,
    /// Who asks. `g9-app` sets this from the session (ADR-0013), never from IPC.
    pub actor: Actor,
    /// Operator intent, 1..=2000 chars.
    pub intent: String,
    /// Domain and mode the command was authorized for.
    pub scope: Scope,
    /// Classification of the payload.
    pub classification: Classification,
    /// Skill-specific payload (validated against `schemas/input.schema.json`).
    pub input_payload: Value,
    /// Evidence the skill may cite. Output refs outside this set are rejected.
    pub evidence_refs: Vec<EvidenceId>,
    /// Verified capabilities and recursion depth (0).
    pub policy_context: PolicyContext,
    /// Caller idempotency key.
    pub idempotency_key: IdempotencyKey,
    /// Trace id spanning the operator intent.
    pub trace_id: TraceId,
}

/// Deterministic invocation id: `inv-` + first 32 hex chars of
/// SHA-256(skill_id ‖ 0 ‖ actor_id ‖ 0 ‖ idempotency_key ‖ 0 ‖ trace_id).
/// A replayed command therefore yields the same invocation id (no randomness).
pub fn derive_invocation_id(
    skill_id: &str,
    actor_id: &str,
    idempotency_key: &str,
    trace_id: &str,
) -> Result<InvocationId, EnvelopeError> {
    let mut bytes = Vec::new();
    for (i, part) in [skill_id, actor_id, idempotency_key, trace_id]
        .iter()
        .enumerate()
    {
        if i > 0 {
            bytes.push(0);
        }
        bytes.extend_from_slice(part.as_bytes());
    }
    let hex = sha256_hex(&bytes);
    let short = hex.get(..32).unwrap_or(&hex);
    InvocationId::parse(&format!("inv-{short}")).map_err(|e| EnvelopeError::InvocationId(e.reason))
}

/// Build the invocation envelope for a package with the package's budget and
/// no tool ids (the orchestrator exposes no tools).
pub fn build_invocation(
    req: InvocationRequest<'_>,
) -> Result<SkillInvocationEnvelope, EnvelopeError> {
    let invocation_id = derive_invocation_id(
        req.package.name.as_str(),
        req.actor.id.as_str(),
        req.idempotency_key.as_str(),
        req.trace_id.as_str(),
    )?;
    Ok(SkillInvocationEnvelope {
        skill_id: req.package.name.clone(),
        skill_version: req.package.version.clone(),
        invocation_id,
        actor: req.actor,
        intent: req.intent,
        approved_scope: req.scope,
        data_classification: req.classification,
        input_schema_version: SCHEMA_VERSION.to_string(),
        input_payload: req.input_payload,
        evidence_refs: req.evidence_refs,
        policy_context: req.policy_context,
        resource_budget: req.package.budget,
        allowed_tool_ids: Vec::new(),
        idempotency_key: req.idempotency_key,
        expected_revision: None,
        trace_id: req.trace_id,
    })
}

/// Validate `instance` against `schema`; issues are reported with `prefix`
/// prepended to each instance path.
pub fn validate_against(
    schema: &Value,
    instance: &Value,
    prefix: &str,
) -> Result<(), Vec<ValidationIssue>> {
    let validator = match jsonschema::validator_for(schema) {
        Ok(v) => v,
        Err(e) => {
            return Err(vec![ValidationIssue {
                path: format!("{prefix}#schema"),
                message: format!("schema does not compile (`{}`)", e.kind().keyword()),
            }])
        }
    };
    let issues: Vec<ValidationIssue> = validator
        .iter_errors(instance)
        .map(|e| ValidationIssue {
            path: format!("{prefix}{}", e.instance_path()),
            message: describe(e.kind()),
        })
        .collect();
    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

fn describe(kind: &ValidationErrorKind) -> String {
    match kind {
        ValidationErrorKind::Required { property } => {
            format!("required property {property} is missing")
        }
        ValidationErrorKind::AdditionalProperties { unexpected } => {
            format!(
                "additional properties are not allowed: {}",
                unexpected.join(", ")
            )
        }
        ValidationErrorKind::Type { kind } => format!("wrong type (expected {kind:?})"),
        ValidationErrorKind::MinItems { limit } => format!("fewer than {limit} items"),
        ValidationErrorKind::MaxLength { limit } => format!("longer than {limit} characters"),
        ValidationErrorKind::MinLength { limit } => format!("shorter than {limit} characters"),
        ValidationErrorKind::Maximum { limit } => format!("greater than {limit}"),
        ValidationErrorKind::Minimum { limit } => format!("less than {limit}"),
        other => format!("`{}` constraint violated", other.keyword()),
    }
}

/// Validate an invocation envelope: the envelope document against the
/// invocation envelope schema, `input_payload` against the package's input
/// schema, and the skill id/version against the package.
pub fn validate_invocation(
    envelope: &SkillInvocationEnvelope,
    package: &SkillPackage,
) -> Result<(), Vec<ValidationIssue>> {
    let mut issues = Vec::new();
    if envelope.skill_id != package.name {
        issues.push(ValidationIssue {
            path: "/skill_id".into(),
            message: "does not name the package".into(),
        });
    }
    if envelope.skill_version != package.version {
        issues.push(ValidationIssue {
            path: "/skill_version".into(),
            message: "does not match the package version".into(),
        });
    }
    let doc = match serde_json::to_value(envelope) {
        Ok(v) => v,
        Err(_) => {
            issues.push(ValidationIssue {
                path: String::new(),
                message: "envelope could not be serialized".into(),
            });
            return Err(issues);
        }
    };
    match invocation_envelope_schema() {
        Ok(schema) => {
            if let Err(mut e) = validate_against(&schema, &doc, "") {
                issues.append(&mut e);
            }
        }
        Err(_) => issues.push(ValidationIssue {
            path: "#schema".into(),
            message: "embedded invocation envelope schema is unreadable".into(),
        }),
    }
    if let Err(mut e) = validate_against(
        &package.input_schema,
        &envelope.input_payload,
        "/input_payload",
    ) {
        issues.append(&mut e);
    }
    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

/// Validate a raw output document against the composed envelope + result schema.
pub fn validate_output(output: &Value, package: &SkillPackage) -> Result<(), Vec<ValidationIssue>> {
    let schema = compose_output_schema(&package.output_schema).map_err(|_| {
        vec![ValidationIssue {
            path: "#schema".into(),
            message: "embedded output envelope schema is unreadable".into(),
        }]
    })?;
    validate_against(&schema, output, "")
}

/// `path: message; ...` for the first `limit` issues.
pub fn summarize(issues: &[ValidationIssue], limit: usize) -> String {
    let mut parts: Vec<String> = issues
        .iter()
        .take(limit)
        .map(|i| {
            format!(
                "{}: {}",
                if i.path.is_empty() { "/" } else { &i.path },
                i.message
            )
        })
        .collect();
    if issues.len() > limit {
        parts.push(format!("(+{} more)", issues.len() - limit));
    }
    parts.join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn embedded_schemas_parse_and_compile() {
        let inv = invocation_envelope_schema().unwrap();
        let out = output_envelope_schema().unwrap();
        assert!(jsonschema::validator_for(&inv).is_ok());
        assert!(jsonschema::validator_for(&out).is_ok());
        assert_eq!(
            inv["$id"],
            "https://g9.local/schemas/skill-inputs/g9-invocation-envelope"
        );
    }

    #[test]
    fn compose_replaces_result_and_strips_ids() {
        let result = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://g9.local/skills/g9-x/output",
            "title": "r", "type": "object", "additionalProperties": false,
            "required": ["n"], "properties": { "n": { "type": "integer" } }
        });
        let composed = compose_output_schema(&result).unwrap();
        assert_eq!(composed["properties"]["result"]["required"], json!(["n"]));
        assert!(composed["properties"]["result"].get("$id").is_none());
        assert!(composed["properties"]["result"].get("$schema").is_none());
        assert_eq!(
            composed["$id"],
            "https://g9.local/schemas/skill-outputs/g9-output-envelope"
        );
    }

    #[test]
    fn issues_never_echo_instance_values() {
        let schema = json!({ "type": "object", "additionalProperties": false,
            "required": ["a"], "properties": { "a": { "type": "integer", "maximum": 3 } } });
        let secret = "hunter2-very-private";
        let err = validate_against(&schema, &json!({ "a": secret, "b": 1 }), "/x").unwrap_err();
        for i in &err {
            assert!(!i.message.contains(secret), "{i:?}");
            assert!(i.path.starts_with("/x"));
        }
        assert!(err.iter().any(|i| i.path == "/x/a"));
        let err = validate_against(&schema, &json!({ "a": 9 }), "").unwrap_err();
        assert_eq!(err[0].message, "greater than 3");
        let bad_schema = json!({ "type": "nonsense" });
        assert!(validate_against(&bad_schema, &json!(1), "").is_err());
    }

    #[test]
    fn invocation_id_is_deterministic_and_valid() {
        let a = derive_invocation_id("g9-x", "op", "k1", "t1").unwrap();
        let b = derive_invocation_id("g9-x", "op", "k1", "t1").unwrap();
        let c = derive_invocation_id("g9-x", "op", "k2", "t1").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.as_str().starts_with("inv-"));
        assert_eq!(a.as_str().len(), 36);
    }

    #[test]
    fn summarize_truncates() {
        let issues: Vec<ValidationIssue> = (0..7)
            .map(|i| ValidationIssue {
                path: format!("/p{i}"),
                message: "m".into(),
            })
            .collect();
        let s = summarize(&issues, 5);
        assert!(s.ends_with("(+2 more)"));
        assert_eq!(summarize(&[], 5), "");
    }
}
