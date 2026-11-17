//! The IPC entry point the Tauri bridge calls for every command:
//! `dispatch(ctx, op, meta_json, payload_json) -> Result<Value, G9Error>`.
//!
//! Order: unknown operation → `INVALID_INPUT ipc.unknown_operation` (later-stage
//! areas answer `PRECONDITION_FAILED <area>.not_implemented`); metadata decoded
//! without an actor and assigned the backend operator (`ipc::parse_meta`); the
//! payload's serialized size checked against the operation's byte cap
//! ([`payload_cap`], `INVALID_INPUT ipc.payload_too_large`); the payload
//! validated against `schemas/ipc/<op>.request.schema.json` **before** it is
//! deserialized into a typed DTO; then the handler runs through the pipeline.
//! The success value is the serialized `CommandOutcome`.

use crate::context::AppContext;
use crate::{analytics, commands, executive, ipc, ops, research, schemas, teaching};
use g9_domain::G9Error;
use serde_json::Value;

/// Route one IPC call. With an operations log configured
/// ([`crate::AppContextBuilder::oplog`]) the call leaves one line there:
/// operation, request id, outcome, duration.
pub fn dispatch(
    ctx: &mut AppContext,
    op: &str,
    meta_json: Value,
    payload_json: Value,
) -> Result<Value, G9Error> {
    let call = begin(ctx, &meta_json);
    let result = route(ctx, op, meta_json, payload_json);
    end(ctx, call, op, &result);
    result
}

/// The name the operations log may write for `op`: the registry's own static
/// name of a routed or PROPOSED operation. What the caller sent is never
/// written when the registry does not know it.
fn logged_name(op: &str) -> &'static str {
    if let Some(spec) = ops::spec(op) {
        return spec.name;
    }
    [
        teaching::PROPOSED_OPERATIONS,
        analytics::PROPOSED_OPERATIONS,
        executive::PROPOSED_OPERATIONS,
        research::PROPOSED_OPERATIONS,
    ]
    .iter()
    .flat_map(|area| area.iter())
    .find(|name| **name == op)
    .copied()
    .unwrap_or(crate::oplog::UNKNOWN_OPERATION)
}

/// A call being timed for the operations log.
struct Call {
    log: std::sync::Arc<crate::oplog::OpLog>,
    request_id: Option<String>,
    started: u64,
}

/// Start timing a call when the context has an operations log. Without a log
/// no clock is read and nothing is copied.
fn begin(ctx: &AppContext, meta_json: &Value) -> Option<Call> {
    let log = ctx.svc.oplog.clone()?;
    Some(Call {
        log,
        request_id: meta_json
            .get("request_id")
            .and_then(Value::as_str)
            .map(str::to_string),
        started: ctx.svc.mono(),
    })
}

/// Record the call: the duration from the context's monotonic clock, the time
/// from its wall clock.
fn end(ctx: &AppContext, call: Option<Call>, op: &str, result: &Result<Value, G9Error>) {
    if let Some(call) = call {
        call.log.command(
            ctx.svc.now(),
            logged_name(op),
            call.request_id.as_deref(),
            result,
            ctx.svc.mono().saturating_sub(call.started),
        );
    }
}

fn route(
    ctx: &mut AppContext,
    op: &str,
    meta_json: Value,
    payload_json: Value,
) -> Result<Value, G9Error> {
    for area in [
        teaching::PROPOSED_OPERATIONS,
        analytics::PROPOSED_OPERATIONS,
        executive::PROPOSED_OPERATIONS,
        research::PROPOSED_OPERATIONS,
    ] {
        if area.contains(&op) {
            return Err(crate::not_implemented(op));
        }
    }
    let Some(spec) = ops::spec(op) else {
        return Err(G9Error::invalid_input(
            "ipc.unknown_operation",
            "unknown operation",
        ));
    };
    let handler = commands::handler_for(spec.name)
        .ok_or_else(|| G9Error::internal_redacted("ipc.handler_missing"))?;
    let meta = ipc::parse_meta(op, &meta_json, ctx.operator())?;
    refuse_oversized(spec.name, &payload_json)?;
    schemas::validate_request(op, &payload_json)?;
    refuse_unprintable_names(&payload_json)?;
    // The mode a handler evaluates is the backend's, derived here once for every operation.
    let meta = commands::effective_meta(ctx, spec, &meta)?;
    let outcome = handler(ctx, spec, &meta, &payload_json)?;
    serde_json::to_value(&outcome).map_err(|_| G9Error::internal_redacted("ipc.serialize"))
}

/// Byte cap of a serialized payload for every operation not named below (app-9).
/// The largest request of such an operation is a few tens of kilobytes (a 20,000
/// character task statement or review answer); operations that carry a file, a
/// note body or a curriculum manifest have their own caps.
pub const MAX_PAYLOAD_BYTES: usize = 1024 * 1024;

/// Byte cap for `evidence.attach_file`: the request schema's `content_base64`
/// limit (22,369,624 characters, a 16 MiB file) plus the file name, media type
/// and note id, with room to spare.
pub const MAX_ATTACHMENT_PAYLOAD_BYTES: usize = 23 * 1024 * 1024;

/// Byte cap for the operations that carry a note body (`notes.create`,
/// `notes.revise`, `assessment.submit` notes). The body may hold
/// `g9_evidence::MAX_NOTE_BYTES` (4 MiB) of UTF-8, and JSON writes a control
/// character as six bytes (`\u0001`): six times the body limit plus 1 MiB for the
/// other fields.
pub const MAX_NOTE_PAYLOAD_BYTES: usize = 6 * (g9_evidence::MAX_NOTE_BYTES as usize) + 1024 * 1024;

/// Byte cap per manifest for `curriculum.import` and `curriculum.validate`
/// (the manifest travels in the payload; the shipped draft is about 240 kB).
/// `curriculum.promote` carries two manifests and gets twice this.
pub const MAX_MANIFEST_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;

/// The largest serialized payload `op` accepts, in bytes (app-9).
pub fn payload_cap(op: &str) -> usize {
    match op {
        "evidence.attach_file" => MAX_ATTACHMENT_PAYLOAD_BYTES,
        "notes.create" | "notes.revise" | "assessment.submit" => MAX_NOTE_PAYLOAD_BYTES,
        "curriculum.import" | "curriculum.validate" => MAX_MANIFEST_PAYLOAD_BYTES,
        "curriculum.promote" => 2 * MAX_MANIFEST_PAYLOAD_BYTES,
        _ => MAX_PAYLOAD_BYTES,
    }
}

/// A writer that accepts at most `left` bytes and then fails: measures a
/// serialized payload without building it.
struct Budget {
    left: usize,
}

impl std::io::Write for Budget {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.left = self
            .left
            .checked_sub(buf.len())
            .ok_or(std::io::ErrorKind::FileTooLarge)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Refuse a payload whose serialized JSON exceeds the operation's byte cap,
/// before schema validation reads it (app-9). The refusal names the limit, never
/// the content.
fn refuse_oversized(op: &str, payload: &Value) -> Result<(), G9Error> {
    let cap = payload_cap(op);
    if serde_json::to_writer(&mut Budget { left: cap }, payload).is_err() {
        return Err(G9Error::invalid_input(
            "ipc.payload_too_large",
            "the request is larger than this operation accepts",
        )
        .with_details(serde_json::json!({ "limit_bytes": cap })));
    }
    Ok(())
}

/// Keys of a request whose value is a name: one line that lists, the native
/// approval confirmation and exports show as a fact.
const NAME_KEYS: [&str; 3] = ["title", "filename", "label"];

/// Refuse a request in which a name is blank or holds a hidden character
/// (`g9_domain::text::name_defect`). One check at the boundary, by key, so that
/// an operation added later is covered without its handler knowing the rule
/// (security audit 2026-11-13). The refusal names the field and the code point,
/// never the text.
fn refuse_unprintable_names(payload: &Value) -> Result<(), G9Error> {
    match payload {
        Value::Object(map) => {
            for (key, value) in map {
                if let (true, Value::String(text)) = (NAME_KEYS.contains(&key.as_str()), value) {
                    if let Some(reason) = g9_domain::text::name_defect(text) {
                        return Err(G9Error::invalid_input(
                            "input.name_not_printable",
                            "a title, file name or label needs visible text and no control, invisible or text-reordering characters",
                        )
                        .with_details(serde_json::json!({ "field": key, "reason": reason })));
                    }
                }
                refuse_unprintable_names(value)?;
            }
            Ok(())
        }
        Value::Array(items) => items.iter().try_for_each(refuse_unprintable_names),
        _ => Ok(()),
    }
}

/// What an approval grants, for a confirmation shown outside the webview
/// (ADR-0014). Every field comes from the stored request; nothing comes from
/// the caller's payload except the approval id used to look it up.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ApprovalSummary {
    /// Approval id.
    pub approval_id: String,
    /// Wire name of the operation the approval allows.
    pub operation: String,
    /// Resource kind (`ATTEMPT`, `NOTE`, ...).
    pub resource_kind: String,
    /// Resource id.
    pub resource_id: String,
    /// Revision the approval is bound to, when the resource exists.
    pub expected_revision: Option<u64>,
    /// When the request was made.
    pub requested_at: g9_domain::UtcMillis,
    /// When the request expires.
    pub expires_at: g9_domain::UtcMillis,
    /// What the record is, read from storage by the backend when the confirmation
    /// is raised (a note's stored title and classification, an event's time, the
    /// grade an attempt would make final), as one sanitised line. For a record
    /// that does not exist yet it says so. Never taken from the request.
    pub subject: String,
    /// What the approval grants, as policy decided it when the request was stored
    /// (`protected record write`, `data delete`, ...). Never taken from the request.
    pub grants: String,
    /// Area and mode the request was made in. The requester states these; they are
    /// one of a closed set of words and are shown as the context of the request,
    /// not as a fact about the record.
    pub scope: String,
    /// First 16 hexadecimal characters of the fingerprint of the exact values.
    pub fingerprint: String,
}

impl ApprovalSummary {
    /// The text the confirmation shows. One fact per line, each written by the
    /// backend; an identifier is labelled as one, because part of it can be chosen
    /// by whoever asked (a curriculum id, for example).
    pub fn text(&self) -> String {
        let revision = self
            .expected_revision
            .map_or_else(String::new, |r| format!(", revision {r}"));
        format!(
            "Approve: {}\nWhat: {}\nGrants: {}\nAsked in: {}\nRecord: {} with the id \"{}\"{}\nFingerprint of the exact values: {}…\n\nRequested {}; the approval expires {}.\nApproval {}",
            self.operation,
            self.subject,
            self.grants,
            self.scope,
            self.resource_kind,
            self.resource_id,
            revision,
            self.fingerprint,
            self.requested_at,
            self.expires_at,
            self.approval_id
        )
    }
}

/// Read what approval `id` grants from storage.
pub fn approval_summary(
    ctx: &AppContext,
    id: &g9_domain::ApprovalId,
) -> Result<ApprovalSummary, G9Error> {
    let row = g9_storage::approvals_store::get_request(ctx.db.conn(), id)?
        .ok_or_else(|| G9Error::not_found("approval.not_found", "approval not found"))?;
    let kind = serde_json::to_value(row.operation.resource.kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "RESOURCE".to_string());
    let name = ops::name_for_approval(row.operation.action, row.operation.resource.kind);
    let operation = if name == "unknown" {
        serde_json::to_value(row.operation.action)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "an operation".to_string())
    } else {
        name.to_string()
    };
    let word = |v: Result<Value, serde_json::Error>| {
        v.ok()
            .and_then(|v| v.as_str().map(|s| s.replace('_', " ").to_lowercase()))
            .unwrap_or_else(|| "unknown".to_string())
    };
    // What the record is comes from storage, never from the request: a page that
    // shows one note and asks about another cannot make this line agree with it.
    let subject = g9_storage::describe::describe_resource(
        ctx.db.conn(),
        row.operation.resource.kind,
        &row.operation.resource.id,
    )?
    .unwrap_or_else(|| {
        format!(
            "a new {} that is not stored yet; its content is fixed by the fingerprint below",
            kind.replace('_', " ").to_lowercase()
        )
    });
    let scope = format!(
        "the {} area, {} mode",
        word(serde_json::to_value(row.operation.scope.domain)),
        word(serde_json::to_value(row.operation.scope.mode))
    );
    let grants = row.required_capability.map_or_else(
        || "this one operation, no standing capability".to_string(),
        |c| word(serde_json::to_value(c)),
    );
    Ok(ApprovalSummary {
        approval_id: row.id.as_str().to_string(),
        operation,
        resource_kind: kind,
        // An id may hold characters chosen by the requester: one printable line.
        resource_id: g9_domain::text::one_line(&row.operation.resource.id, 160),
        expected_revision: row.expected_revision.map(|r| r.get()),
        requested_at: row.requested_at,
        expires_at: row.expires_at,
        subject,
        grants,
        scope,
        fingerprint: row.operation.parameters_sha256.chars().take(16).collect(),
    })
}

/// [`dispatch`], except that an `approvals.decide` APPROVE is first shown to
/// the operator through `confirm` and goes through only when it returns `true`
/// (ADR-0014, SEC-03). The desktop bridge passes a native dialog; a plain
/// [`dispatch`] of an APPROVE is refused while `AppConfig::native_confirmation`
/// is on. Every other operation, and a REJECT, is dispatched unchanged.
pub fn dispatch_confirming(
    ctx: &mut AppContext,
    op: &str,
    meta_json: Value,
    payload_json: Value,
    confirm: &mut dyn FnMut(&ApprovalSummary) -> bool,
) -> Result<Value, G9Error> {
    let approve = op == "approvals.decide"
        && payload_json.get("decision").and_then(Value::as_str) == Some("APPROVE");
    if !approve {
        return dispatch(ctx, op, meta_json, payload_json);
    }
    // One log line for the whole confirmed call, a declined confirmation included.
    let call = begin(ctx, &meta_json);
    let result = confirm_then_route(ctx, op, meta_json, payload_json, confirm);
    end(ctx, call, op, &result);
    result
}

fn confirm_then_route(
    ctx: &mut AppContext,
    op: &str,
    meta_json: Value,
    payload_json: Value,
    confirm: &mut dyn FnMut(&ApprovalSummary) -> bool,
) -> Result<Value, G9Error> {
    let id = payload_json
        .get("approval_id")
        .and_then(Value::as_str)
        .and_then(|s| g9_domain::ApprovalId::parse(s).ok())
        .ok_or_else(|| G9Error::invalid_input("ipc.payload_invalid", "approval_id is required"))?;
    let summary = approval_summary(ctx, &id)?;
    if !confirm(&summary) {
        // The operator's "no" is a decision about a stored request: it is recorded.
        crate::pipeline::record_declined_confirmation(ctx, &meta_json, &id);
        return Err(G9Error::unauthorized(
            "approval.not_confirmed",
            "the approval was not confirmed",
        ));
    }
    ctx.svc.native_confirmed = Some(id);
    let result = route(ctx, op, meta_json, payload_json);
    ctx.svc.native_confirmed = None;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_largest_attachment_the_schema_accepts_fits_under_its_cap() {
        let payload = json!({
            "filename": "f".repeat(255),
            "media_type": "application/pdf",
            "note_id": "n".repeat(128),
            "content_base64": "A".repeat(22_369_624),
        });
        assert!(refuse_oversized("evidence.attach_file", &payload).is_ok());
        let refused = refuse_oversized("notes.list", &payload).unwrap_err();
        assert_eq!(refused.code, "ipc.payload_too_large");
        assert_eq!(
            refused.details,
            Some(json!({ "limit_bytes": MAX_PAYLOAD_BYTES }))
        );
    }

    #[test]
    fn a_note_body_of_control_characters_at_its_byte_limit_fits_under_the_note_cap() {
        // Each U+0001 is one byte of UTF-8 and six bytes of JSON.
        let body = "\u{1}".repeat(usize::try_from(g9_evidence::MAX_NOTE_BYTES).unwrap());
        for op in ["notes.create", "notes.revise", "assessment.submit"] {
            let payload = json!({
                "title": "t".repeat(200),
                "body_markdown": body,
                "notes": "",
                "note_id": "n".repeat(128),
                "classification": "INTERNAL",
                "domain": "LEARNING",
                "evidence_refs": vec!["e".repeat(128); 100],
            });
            assert!(refuse_oversized(op, &payload).is_ok(), "{op}");
        }
    }

    #[test]
    fn the_shipped_curriculum_fits_under_the_manifest_cap() {
        let manifest: Value = serde_json::from_str(include_str!(
            "../../../data/curriculum/g9-ct9-480.draft.v1.json"
        ))
        .unwrap();
        let payload = json!({ "manifest": manifest, "provenance": "RECONCILED_CANDIDATE" });
        assert!(refuse_oversized("curriculum.import", &payload).is_ok());
    }

    #[test]
    fn every_operation_that_carries_a_manifest_gets_the_manifest_cap() {
        // curriculum.validate carries one manifest, curriculum.promote two
        // (candidate and original); neither may fall to the 1 MiB default.
        let manifest = "x".repeat(2 * MAX_PAYLOAD_BYTES);
        let one = json!({ "manifest": manifest });
        let two = json!({ "candidate_manifest": manifest, "original_manifest": manifest });
        for op in ["curriculum.import", "curriculum.validate"] {
            assert!(refuse_oversized(op, &one).is_ok(), "{op}");
        }
        assert!(refuse_oversized("curriculum.promote", &two).is_ok());
        assert_eq!(
            payload_cap("curriculum.promote"),
            2 * MAX_MANIFEST_PAYLOAD_BYTES
        );
    }

    #[test]
    fn the_cap_counts_serialized_bytes_exactly() {
        // `{"s":"…"}` is the string plus eight bytes.
        let at = json!({ "s": "x".repeat(MAX_PAYLOAD_BYTES - 8) });
        let over = json!({ "s": "x".repeat(MAX_PAYLOAD_BYTES - 7) });
        assert!(refuse_oversized("calendar.list", &at).is_ok());
        assert!(refuse_oversized("calendar.list", &over).is_err());
    }
}
