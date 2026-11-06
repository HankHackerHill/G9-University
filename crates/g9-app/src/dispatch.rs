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
