//! `audit.recent` (read-only). `chain_ok` is the result of recomputing the
//! whole hash chain (`AuditChain::verify_chain`). Entries are the audit rows
//! appended by this process (newest first), read back with `AuditChain::get`:
//! `g9-storage` exposes no audit listing query in wave 1, so rows written by an
//! earlier process are not listed (reported as a warning, never hidden).

use super::{decode, rc};
use crate::context::AppContext;
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, Facts, Mutation};
use g9_domain::{
    ActorKind, Classification, CommandMeta, CommandOutcome, DataDomain, G9Error, ResourceKind,
    ResourceRef, Revision, UtcMillis,
};
use g9_storage::audit::{AuditChain, AuditOutcome};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
struct RecentRequest {
    limit: Option<u32>,
    before: Option<UtcMillis>,
}

/// `audit.recent`.
pub fn recent(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: RecentRequest = decode(payload)?;
    let resource = ResourceRef::new(ResourceKind::AuditMetadata, "audit");
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |_, _| {
            Ok(Facts::existing(
                rc(DataDomain::System, Classification::Internal, true),
                Revision::INITIAL,
            ))
        },
        |env| {
            let limit = usize::try_from(req.limit.unwrap_or(50).min(500)).unwrap_or(50);
            let chain = AuditChain::verify_chain(env.conn)?;
            let mut entries = Vec::new();
            for audit_ref in env.svc.recent_audit.iter().rev() {
                if entries.len() >= limit {
                    break;
                }
                let Some((record, entry)) = AuditChain::get(env.conn, audit_ref)? else {
                    continue;
                };
                if req.before.is_some_and(|b| entry.occurred_at >= b) {
                    continue;
                }
                let actor_kind = match entry.actor.kind {
                    ActorKind::Operator => "OPERATOR",
                    ActorKind::System => "SYSTEM",
                    ActorKind::Skill { .. } => "SKILL",
                    ActorKind::Model => "MODEL",
                };
                entries.push(json!({
                    "id": record.audit_ref,
                    "prev_hash": record.prev_hash,
                    "row_hash": record.row_hash,
                    "occurred_at": entry.occurred_at,
                    "actor_id": entry.actor.id,
                    "actor_kind": actor_kind,
                    "operation": entry.operation,
                    "resource_kind": entry.resource.as_ref().map(|r| r.kind),
                    "resource_id": entry.resource.as_ref().map(|r| r.id.clone()),
                    "decision": entry.decision,
                    "outcome": if entry.outcome == AuditOutcome::Success { "OK" } else { "ERROR" },
                    "request_id": entry.request_id,
                    "trace_id": entry.trace_id,
                    "details_sha256": entry.details_sha256.unwrap_or_else(|| "0".repeat(64)),
                }));
            }
            let mut m = Mutation::new(json!({ "entries": entries, "chain_ok": chain.ok }), None);
            if u64::try_from(env.svc.recent_audit.len()).unwrap_or(u64::MAX) < chain.rows {
                m = m.with_warning(
                    "audit.recent.process_scoped: rows appended before this process started are verified (chain_ok) but not listed",
                );
            }
            Ok(m)
        },
    )
}
