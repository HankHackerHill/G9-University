//! Embedded IPC JSON Schemas (`schemas/ipc`, draft 2020-12) and their
//! compiled validators (ARCHITECTURE.md §5 step 1; ADR-0013).
//!
//! Every request payload is validated against `<op>.request.schema.json`
//! **before** it is deserialized into a typed DTO; command metadata is validated
//! against `command-meta.schema.json` (which has no `actor` property, so a
//! UI-supplied actor is rejected by `additionalProperties: false`). Responses
//! are validated against `<op>.response.schema.json` before they cross the
//! boundary, so a handler can never emit a shape the UI was not built against.
//!
//! The schemas are compiled once per process with `jsonschema` 0.60
//! (`default-features = false`: no file or HTTP `$ref` resolution). Error
//! messages are masked: instance values never appear in a `G9Error`.

use g9_domain::G9Error;
use jsonschema::Validator;
use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// `command-meta.schema.json` (ClientCommandMeta: no actor).
pub const COMMAND_META_SCHEMA: &str = include_str!("../../../schemas/ipc/command-meta.schema.json");
/// `command-outcome.schema.json`.
pub const COMMAND_OUTCOME_SCHEMA: &str =
    include_str!("../../../schemas/ipc/command-outcome.schema.json");
/// `g9-error.schema.json`.
pub const G9_ERROR_SCHEMA: &str = include_str!("../../../schemas/ipc/g9-error.schema.json");

/// `(request, response)` schema texts for an operation.
fn texts(op: &str) -> Option<(&'static str, &'static str)> {
    Some(match op {
        "evidence.list" => (
            include_str!("../../../schemas/ipc/evidence.list.request.schema.json"),
            include_str!("../../../schemas/ipc/evidence.list.response.schema.json"),
        ),
        "evidence.from_note" => (
            include_str!("../../../schemas/ipc/evidence.from_note.request.schema.json"),
            include_str!("../../../schemas/ipc/evidence.from_note.response.schema.json"),
        ),
        "evidence.attach_file" => (
            include_str!("../../../schemas/ipc/evidence.attach_file.request.schema.json"),
            include_str!("../../../schemas/ipc/evidence.attach_file.response.schema.json"),
        ),
        "assessment.define" => (
            include_str!("../../../schemas/ipc/assessment.define.request.schema.json"),
            include_str!("../../../schemas/ipc/assessment.define.response.schema.json"),
        ),
        "competency.create" => (
            include_str!("../../../schemas/ipc/competency.create.request.schema.json"),
            include_str!("../../../schemas/ipc/competency.create.response.schema.json"),
        ),
        "retention.record_review" => (
            include_str!("../../../schemas/ipc/retention.record_review.request.schema.json"),
            include_str!("../../../schemas/ipc/retention.record_review.response.schema.json"),
        ),
        "retention.create_item" => (
            include_str!("../../../schemas/ipc/retention.create_item.request.schema.json"),
            include_str!("../../../schemas/ipc/retention.create_item.response.schema.json"),
        ),
        "calendar.delete_event" => (
            include_str!("../../../schemas/ipc/calendar.delete_event.request.schema.json"),
            include_str!("../../../schemas/ipc/calendar.delete_event.response.schema.json"),
        ),
        "calendar.update_event" => (
            include_str!("../../../schemas/ipc/calendar.update_event.request.schema.json"),
            include_str!("../../../schemas/ipc/calendar.update_event.response.schema.json"),
        ),
        "calendar.create_event" => (
            include_str!("../../../schemas/ipc/calendar.create_event.request.schema.json"),
            include_str!("../../../schemas/ipc/calendar.create_event.response.schema.json"),
        ),
        "session.create" => (
            include_str!("../../../schemas/ipc/session.create.request.schema.json"),
            include_str!("../../../schemas/ipc/session.create.response.schema.json"),
        ),
        "approvals.decide" => (
            include_str!("../../../schemas/ipc/approvals.decide.request.schema.json"),
            include_str!("../../../schemas/ipc/approvals.decide.response.schema.json"),
        ),
        "approvals.list" => (
            include_str!("../../../schemas/ipc/approvals.list.request.schema.json"),
            include_str!("../../../schemas/ipc/approvals.list.response.schema.json"),
        ),
        "assessment.approve_final" => (
            include_str!("../../../schemas/ipc/assessment.approve_final.request.schema.json"),
            include_str!("../../../schemas/ipc/assessment.approve_final.response.schema.json"),
        ),
        "assessment.begin" => (
            include_str!("../../../schemas/ipc/assessment.begin.request.schema.json"),
            include_str!("../../../schemas/ipc/assessment.begin.response.schema.json"),
        ),
        "assessment.grade_provisional" => (
            include_str!("../../../schemas/ipc/assessment.grade_provisional.request.schema.json"),
            include_str!("../../../schemas/ipc/assessment.grade_provisional.response.schema.json"),
        ),
        "assessment.list" => (
            include_str!("../../../schemas/ipc/assessment.list.request.schema.json"),
            include_str!("../../../schemas/ipc/assessment.list.response.schema.json"),
        ),
        "assessment.review_evidence" => (
            include_str!("../../../schemas/ipc/assessment.review_evidence.request.schema.json"),
            include_str!("../../../schemas/ipc/assessment.review_evidence.response.schema.json"),
        ),
        "assessment.submit" => (
            include_str!("../../../schemas/ipc/assessment.submit.request.schema.json"),
            include_str!("../../../schemas/ipc/assessment.submit.response.schema.json"),
        ),
        "audit.recent" => (
            include_str!("../../../schemas/ipc/audit.recent.request.schema.json"),
            include_str!("../../../schemas/ipc/audit.recent.response.schema.json"),
        ),
        "backup.create" => (
            include_str!("../../../schemas/ipc/backup.create.request.schema.json"),
            include_str!("../../../schemas/ipc/backup.create.response.schema.json"),
        ),
        "backup.restore_test" => (
            include_str!("../../../schemas/ipc/backup.restore_test.request.schema.json"),
            include_str!("../../../schemas/ipc/backup.restore_test.response.schema.json"),
        ),
        "backup.verify" => (
            include_str!("../../../schemas/ipc/backup.verify.request.schema.json"),
            include_str!("../../../schemas/ipc/backup.verify.response.schema.json"),
        ),
        "calendar.apply" => (
            include_str!("../../../schemas/ipc/calendar.apply.request.schema.json"),
            include_str!("../../../schemas/ipc/calendar.apply.response.schema.json"),
        ),
        "calendar.list" => (
            include_str!("../../../schemas/ipc/calendar.list.request.schema.json"),
            include_str!("../../../schemas/ipc/calendar.list.response.schema.json"),
        ),
        "calendar.propose" => (
            include_str!("../../../schemas/ipc/calendar.propose.request.schema.json"),
            include_str!("../../../schemas/ipc/calendar.propose.response.schema.json"),
        ),
        "competency.projection" => (
            include_str!("../../../schemas/ipc/competency.projection.request.schema.json"),
            include_str!("../../../schemas/ipc/competency.projection.response.schema.json"),
        ),
        "curriculum.daily_plan" => (
            include_str!("../../../schemas/ipc/curriculum.daily_plan.request.schema.json"),
            include_str!("../../../schemas/ipc/curriculum.daily_plan.response.schema.json"),
        ),
        "curriculum.import" => (
            include_str!("../../../schemas/ipc/curriculum.import.request.schema.json"),
            include_str!("../../../schemas/ipc/curriculum.import.response.schema.json"),
        ),
        "curriculum.promote" => (
            include_str!("../../../schemas/ipc/curriculum.promote.request.schema.json"),
            include_str!("../../../schemas/ipc/curriculum.promote.response.schema.json"),
        ),
        "curriculum.validate" => (
            include_str!("../../../schemas/ipc/curriculum.validate.request.schema.json"),
            include_str!("../../../schemas/ipc/curriculum.validate.response.schema.json"),
        ),
        "deletion.request" => (
            include_str!("../../../schemas/ipc/deletion.request.request.schema.json"),
            include_str!("../../../schemas/ipc/deletion.request.response.schema.json"),
        ),
        "export.request" => (
            include_str!("../../../schemas/ipc/export.request.request.schema.json"),
            include_str!("../../../schemas/ipc/export.request.response.schema.json"),
        ),
        "interventions.list" => (
            include_str!("../../../schemas/ipc/interventions.list.request.schema.json"),
            include_str!("../../../schemas/ipc/interventions.list.response.schema.json"),
        ),
        "notes.create" => (
            include_str!("../../../schemas/ipc/notes.create.request.schema.json"),
            include_str!("../../../schemas/ipc/notes.create.response.schema.json"),
        ),
        "notes.revise" => (
            include_str!("../../../schemas/ipc/notes.revise.request.schema.json"),
            include_str!("../../../schemas/ipc/notes.revise.response.schema.json"),
        ),
        "retention.due" => (
            include_str!("../../../schemas/ipc/retention.due.request.schema.json"),
            include_str!("../../../schemas/ipc/retention.due.response.schema.json"),
        ),
        "session.complete" => (
            include_str!("../../../schemas/ipc/session.complete.request.schema.json"),
            include_str!("../../../schemas/ipc/session.complete.response.schema.json"),
        ),
        "session.correct_time" => (
            include_str!("../../../schemas/ipc/session.correct_time.request.schema.json"),
            include_str!("../../../schemas/ipc/session.correct_time.response.schema.json"),
        ),
        "session.list" => (
            include_str!("../../../schemas/ipc/session.list.request.schema.json"),
            include_str!("../../../schemas/ipc/session.list.response.schema.json"),
        ),
        "session.pause" => (
            include_str!("../../../schemas/ipc/session.pause.request.schema.json"),
            include_str!("../../../schemas/ipc/session.pause.response.schema.json"),
        ),
        "session.prepare" => (
            include_str!("../../../schemas/ipc/session.prepare.request.schema.json"),
            include_str!("../../../schemas/ipc/session.prepare.response.schema.json"),
        ),
        "session.resume" => (
            include_str!("../../../schemas/ipc/session.resume.request.schema.json"),
            include_str!("../../../schemas/ipc/session.resume.response.schema.json"),
        ),
        "session.start" => (
            include_str!("../../../schemas/ipc/session.start.request.schema.json"),
            include_str!("../../../schemas/ipc/session.start.response.schema.json"),
        ),
        "skills.invoke" => (
            include_str!("../../../schemas/ipc/skills.invoke.request.schema.json"),
            include_str!("../../../schemas/ipc/skills.invoke.response.schema.json"),
        ),
        "system.status" => (
            include_str!("../../../schemas/ipc/system.status.request.schema.json"),
            include_str!("../../../schemas/ipc/system.status.response.schema.json"),
        ),
        _ => return None,
    })
}

/// Raw request schema text for `op`.
pub fn request_schema_text(op: &str) -> Option<&'static str> {
    texts(op).map(|t| t.0)
}

/// Raw response schema text for `op`.
pub fn response_schema_text(op: &str) -> Option<&'static str> {
    texts(op).map(|t| t.1)
}

/// Which side of an operation a schema describes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Side {
    Request,
    Response,
    Meta,
    Outcome,
    Error,
}

type Compiled = BTreeMap<(String, Side), Result<Validator, String>>;

fn registry() -> &'static Compiled {
    static REGISTRY: OnceLock<Compiled> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        let mut out = Compiled::new();
        let mut add = |name: &str, side: Side, text: &str| {
            let compiled = serde_json::from_str::<Value>(text)
                .map_err(|e| format!("schema is not JSON: line {}", e.line()))
                .and_then(|v| {
                    jsonschema::validator_for(&v)
                        .map_err(|e| format!("schema invalid: {}", e.masked()))
                });
            out.insert((name.to_string(), side), compiled);
        };
        add("command-meta", Side::Meta, COMMAND_META_SCHEMA);
        add("command-outcome", Side::Outcome, COMMAND_OUTCOME_SCHEMA);
        add("g9-error", Side::Error, G9_ERROR_SCHEMA);
        for spec in crate::ops::OPERATIONS {
            if let Some((req, resp)) = texts(spec.name) {
                add(spec.name, Side::Request, req);
                add(spec.name, Side::Response, resp);
            }
        }
        out
    })
}

/// One masked schema violation: JSON pointer into the instance plus a message
/// that never contains instance values.
fn violations(validator: &Validator, instance: &Value) -> Vec<Value> {
    validator
        .iter_errors(instance)
        .take(20)
        .map(|e| {
            serde_json::json!({
                "path": e.instance_path().as_str(),
                "message": e.masked().to_string(),
            })
        })
        .collect()
}

fn check(name: &str, side: Side, instance: &Value, code: &str, what: &str) -> Result<(), G9Error> {
    let Some(entry) = registry().get(&(name.to_string(), side)) else {
        return Err(G9Error::invalid_input(
            "ipc.unknown_operation",
            "unknown operation",
        ));
    };
    let validator = match entry {
        Ok(v) => v,
        Err(reason) => {
            tracing::error!(schema = name, %reason, "embedded IPC schema failed to compile");
            return Err(G9Error::internal_redacted("ipc.schema_unavailable"));
        }
    };
    let found = violations(validator, instance);
    if found.is_empty() {
        Ok(())
    } else {
        Err(
            G9Error::invalid_input(code, format!("{what} does not match its schema"))
                .with_details(serde_json::json!({ "violations": found })),
        )
    }
}

/// §5 step 1: validate a request payload against `<op>.request.schema.json`.
pub fn validate_request(op: &str, payload: &Value) -> Result<(), G9Error> {
    check(op, Side::Request, payload, "ipc.payload_invalid", "payload")
}

/// Validate a handler result against `<op>.response.schema.json`. A failure is a
/// backend defect: it is logged and reported as a redacted `INTERNAL` error.
pub fn validate_response(op: &str, result: &Value) -> Result<(), G9Error> {
    match check(op, Side::Response, result, "ipc.response_invalid", "result") {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::error!(operation = op, details = ?e.details, "handler result violates its response schema");
            Err(G9Error::internal_redacted("ipc.response_schema_violation"))
        }
    }
}

/// Validate wire metadata against `command-meta.schema.json`.
pub fn validate_meta(meta: &Value) -> Result<(), G9Error> {
    check(
        "command-meta",
        Side::Meta,
        meta,
        "meta.invalid",
        "command metadata",
    )
}

/// Validate a success envelope against `command-outcome.schema.json` (tests, bridge).
pub fn validate_outcome(outcome: &Value) -> Result<(), G9Error> {
    check(
        "command-outcome",
        Side::Outcome,
        outcome,
        "ipc.outcome_invalid",
        "outcome",
    )
}

/// Validate an error against `g9-error.schema.json` (tests, bridge).
pub fn validate_error(error: &Value) -> Result<(), G9Error> {
    check("g9-error", Side::Error, error, "ipc.error_invalid", "error")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_schema_compiles() {
        for ((name, side), compiled) in registry() {
            assert!(
                compiled.is_ok(),
                "{name} {side:?}: {:?}",
                compiled.as_ref().err()
            );
        }
        assert_eq!(registry().len(), 3 + 2 * crate::ops::OPERATIONS.len());
    }

    #[test]
    fn violations_are_masked() {
        let err = validate_request(
            "notes.create",
            &serde_json::json!({ "title": "t", "body_markdown": "SECRET-TEXT", "classification": "NOPE-SECRET", "domain": "LEARNING" }),
        )
        .unwrap_err();
        let text = serde_json::to_string(&err).unwrap();
        assert!(!text.contains("NOPE-SECRET"), "{text}");
        assert!(!text.contains("SECRET-TEXT"), "{text}");
        assert_eq!(err.code, "ipc.payload_invalid");
    }
}
