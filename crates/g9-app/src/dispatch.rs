//! The IPC entry point the Tauri bridge calls for every command:
//! `dispatch(ctx, op, meta_json, payload_json) -> Result<Value, G9Error>`.
//!
//! Order: unknown operation → `INVALID_INPUT ipc.unknown_operation` (later-stage
//! areas answer `PRECONDITION_FAILED <area>.not_implemented`); metadata decoded
//! without an actor and assigned the backend operator (`ipc::parse_meta`); the
//! payload validated against `schemas/ipc/<op>.request.schema.json` **before**
//! it is deserialized into a typed DTO; then the handler runs through the
//! pipeline. The success value is the serialized `CommandOutcome`.

use crate::context::AppContext;
use crate::{analytics, commands, executive, ipc, ops, research, schemas, teaching};
use g9_domain::G9Error;
use serde_json::Value;

/// Route one IPC call.
pub fn dispatch(
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
    schemas::validate_request(op, &payload_json)?;
    let outcome = handler(ctx, spec, &meta, &payload_json)?;
    serde_json::to_value(&outcome).map_err(|_| G9Error::internal_redacted("ipc.serialize"))
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
}

impl ApprovalSummary {
    /// The text the confirmation shows.
    pub fn text(&self) -> String {
        let revision = self
            .expected_revision
            .map_or_else(String::new, |r| format!(" at revision {r}"));
        format!(
            "Approve {} on {} {}{}?\n\nRequested {}; the approval expires {}.\nApproval {}",
            self.operation,
            self.resource_kind,
            self.resource_id,
            revision,
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
    Ok(ApprovalSummary {
        approval_id: row.id.as_str().to_string(),
        operation,
        resource_kind: kind,
        resource_id: row.operation.resource.id.clone(),
        expected_revision: row.expected_revision.map(|r| r.get()),
        requested_at: row.requested_at,
        expires_at: row.expires_at,
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
    let id = payload_json
        .get("approval_id")
        .and_then(Value::as_str)
        .and_then(|s| g9_domain::ApprovalId::parse(s).ok())
        .ok_or_else(|| G9Error::invalid_input("ipc.payload_invalid", "approval_id is required"))?;
    let summary = approval_summary(ctx, &id)?;
    if !confirm(&summary) {
        return Err(G9Error::unauthorized(
            "approval.not_confirmed",
            "the approval was not confirmed",
        ));
    }
    ctx.svc.native_confirmed = Some(id);
    let result = dispatch(ctx, op, meta_json, payload_json);
    ctx.svc.native_confirmed = None;
    result
}
