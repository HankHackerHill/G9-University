//! Structured-output validation (A11; ARCHITECTURE.md §10 "invalid output is
//! rejected, never auto-repaired into authoritative data").
//!
//! Validation uses `jsonschema` 0.60 (draft 2020-12, no file/http resolution:
//! schemas must be self-contained). Every reported [`ValidationIssue`] carries
//! the JSON-pointer path of the offending instance node and a **masked**
//! message: instance values are replaced by a placeholder so that model output
//! or private content never leaks through an error detail (handoff §12
//! "Sensitive log leakage").

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;

/// The G9 output envelope schema, embedded at compile time from
/// `schemas/skill-outputs/g9-output-envelope.schema.json`.
pub const OUTPUT_ENVELOPE_SCHEMA_JSON: &str =
    include_str!("../../../schemas/skill-outputs/g9-output-envelope.schema.json");

/// One schema violation. `path` is a JSON pointer into the instance (`""` is
/// the root); `message` never contains instance values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationIssue {
    /// JSON pointer into the instance; empty for the root.
    pub path: String,
    /// Masked description of the violation (no instance values).
    pub message: String,
}

impl ValidationIssue {
    /// Build an issue at `path`.
    pub fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }

    /// Issue attributed to the schema itself (it failed to compile).
    fn schema(message: impl Into<String>) -> Self {
        Self::new("(schema)", message)
    }
}

/// Validate `instance` against `schema` (draft 2020-12 by default; the
/// schema's own `$schema` wins when present). Returns every violation found,
/// or a single `(schema)` issue when the schema itself cannot be compiled.
pub fn validate_structured(instance: &Value, schema: &Value) -> Result<(), Vec<ValidationIssue>> {
    let validator = match jsonschema::validator_for(schema) {
        Ok(v) => v,
        Err(e) => {
            return Err(vec![ValidationIssue::schema(format!(
                "schema is not a valid JSON Schema at {}: {}",
                e.instance_path(),
                e.masked()
            ))])
        }
    };
    let issues: Vec<ValidationIssue> = validator
        .iter_errors(instance)
        .map(|e| ValidationIssue::new(e.instance_path().as_str(), e.masked().to_string()))
        .collect();
    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

/// Parsed output envelope schema (parsed once). `None` only if the embedded
/// file is not valid JSON, which is a build defect reported as a `(schema)` issue.
pub fn output_envelope_schema() -> Option<&'static Value> {
    static SCHEMA: OnceLock<Option<Value>> = OnceLock::new();
    SCHEMA
        .get_or_init(|| serde_json::from_str(OUTPUT_ENVELOPE_SCHEMA_JSON).ok())
        .as_ref()
}

/// Compose a skill's `output.schema.json` into the envelope: the result is a
/// copy of `envelope` whose `properties.result` is replaced by `result_schema`.
/// Nothing else is touched, so `additionalProperties: false` and the
/// `required` list of the envelope stay in force.
pub fn compose_output_schema(envelope: &Value, result_schema: &Value) -> Value {
    let mut composed = envelope.clone();
    let props = match composed.get_mut("properties") {
        Some(Value::Object(map)) => map,
        _ => {
            if let Value::Object(root) = &mut composed {
                root.insert("properties".to_string(), Value::Object(Default::default()));
            }
            match composed.get_mut("properties") {
                Some(Value::Object(map)) => map,
                // `composed` is not an object at all; return it unchanged.
                _ => return composed,
            }
        }
    };
    props.insert("result".to_string(), result_schema.clone());
    composed
}

/// Validate a full skill output (`SkillOutputEnvelope` JSON plus `result`)
/// against the embedded envelope schema composed with `result_schema`.
pub fn validate_against_envelope(
    output: &Value,
    result_schema: &Value,
) -> Result<(), Vec<ValidationIssue>> {
    let Some(envelope) = output_envelope_schema() else {
        return Err(vec![ValidationIssue::schema(
            "embedded output envelope schema is not valid JSON",
        )]);
    };
    let composed = compose_output_schema(envelope, result_schema);
    validate_structured(output, &composed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn result_schema() -> Value {
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "https://g9.local/schemas/skills/test/output",
            "title": "test result",
            "type": "object",
            "additionalProperties": false,
            "required": ["lesson_title", "steps"],
            "properties": {
                "lesson_title": { "type": "string", "minLength": 1 },
                "steps": { "type": "array", "minItems": 1, "items": { "type": "string" } }
            }
        })
    }

    pub(crate) fn valid_output() -> Value {
        json!({
            "status": "COMPLETED",
            "result_schema_version": "1.0",
            "observations": [{ "statement": "learner completed D01", "evidence_refs": ["ev-1"] }],
            "inferences": [],
            "evidence_refs": ["ev-1"],
            "confidence_limits": ["based on one attempt"],
            "recommended_actions": [],
            "approval_requirements": [],
            "resource_usage": { "tokens": 10, "seconds": 1, "tool_calls": 0 },
            "validation_results": [],
            "errors": [],
            "result": { "lesson_title": "Ports", "steps": ["scan"] }
        })
    }

    #[test]
    fn envelope_schema_embeds_and_parses() {
        let s = output_envelope_schema().expect("embedded schema parses");
        assert_eq!(s["$schema"], "https://json-schema.org/draft/2020-12/schema");
        assert_eq!(s["additionalProperties"], false);
    }

    #[test]
    fn compose_replaces_only_result() {
        let env = output_envelope_schema().unwrap();
        let composed = compose_output_schema(env, &result_schema());
        assert_eq!(composed["properties"]["result"], result_schema());
        assert_eq!(
            composed["properties"]["status"],
            env["properties"]["status"]
        );
        assert_eq!(composed["required"], env["required"]);
        // Non-object envelope is returned unchanged.
        assert_eq!(compose_output_schema(&json!(1), &result_schema()), json!(1));
        // Envelope without `properties` gains one.
        let c = compose_output_schema(&json!({"type": "object"}), &json!({"type": "string"}));
        assert_eq!(c["properties"]["result"], json!({"type": "string"}));
    }

    #[test]
    fn invalid_output_reports_pointer_paths_and_masks_values() {
        let mut out = valid_output();
        out["result"]["steps"] = json!([]);
        out["result"]["lesson_title"] = json!("SECRET-FLAG-123");
        out["result"]["lesson_title"] = json!(42);
        out["status"] = json!("DONE");
        let issues = validate_against_envelope(&out, &result_schema()).unwrap_err();
        let paths: Vec<&str> = issues.iter().map(|i| i.path.as_str()).collect();
        assert!(paths.contains(&"/result/steps"), "{paths:?}");
        assert!(paths.contains(&"/result/lesson_title"), "{paths:?}");
        assert!(paths.contains(&"/status"), "{paths:?}");
        for i in &issues {
            assert!(!i.message.contains("42"), "value leaked: {}", i.message);
            assert!(!i.message.contains("DONE"), "value leaked: {}", i.message);
        }
    }

    #[test]
    fn additional_property_and_missing_required_are_rejected() {
        let mut out = valid_output();
        out["extra"] = json!(true);
        let issues = validate_against_envelope(&out, &result_schema()).unwrap_err();
        assert!(issues.iter().any(|i| i.path.is_empty()));
        let mut out = valid_output();
        out.as_object_mut().unwrap().remove("result");
        assert!(validate_against_envelope(&out, &result_schema()).is_err());
    }

    #[test]
    fn broken_schema_is_reported_not_panicked() {
        let bad = json!({"type": "not-a-type"});
        let err = validate_structured(&json!({}), &bad).unwrap_err();
        assert_eq!(err.len(), 1);
        assert_eq!(err[0].path, "(schema)");
    }

    #[test]
    fn observation_without_evidence_is_rejected_by_envelope() {
        let mut out = valid_output();
        out["observations"][0]["evidence_refs"] = json!([]);
        let issues = validate_against_envelope(&out, &result_schema()).unwrap_err();
        assert!(issues
            .iter()
            .any(|i| i.path == "/observations/0/evidence_refs"));
    }
}
