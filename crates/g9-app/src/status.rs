//! `system.status` (read-only). Every field reports observed state; nothing is
//! assumed: the model is `available: false, reason: NO_RUNTIME_CONFIGURED`
//! whenever the configured runtime has no loaded model (the default
//! `NullRuntime`), network egress is reported disabled because the mentor
//! runtime has no network capability at all, and storage is `READ_ONLY` when
//! recovery refused writes (broken audit chain or unapplied migrations).
//!
//! `last_backup` covers backups created or advanced by this process
//! (`g9-storage` has no manifest reader yet). When it is `null` but
//! `backup_manifests` holds rows from an earlier process, the outcome carries
//! the warning `system.status.last_backup_process_scoped` with their count,
//! so "no backup" is never reported while backups are on record.

use crate::commands::{rc, to_value};
use crate::context::AppContext;
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, Facts, Mutation};
use g9_domain::{
    ApprovalState, Classification, CommandMeta, CommandOutcome, DataDomain, G9Error, ResourceKind,
    ResourceRef, Revision, SchemaVersion,
};
use g9_storage::approvals_store;
use g9_storage::{migrations, Migrator};
use serde_json::{json, Value};

/// Handle `system.status`.
pub fn handle(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let resource = ResourceRef::new(ResourceKind::AppSetting, "system.status");
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
                rc(DataDomain::System, Classification::Internal, false),
                Revision::INITIAL,
            ))
        },
        |env| {
            let svc = &*env.svc;
            let info = svc.runtime.describe();
            let model = match info.model {
                Some(model) => json!({ "available": true, "model_id": model.model_id }),
                None => json!({ "available": false, "reason": "NO_RUNTIME_CONFIGURED" }),
            };
            let pending = approvals_store::list_by_state(env.conn, ApprovalState::Pending)?
                .iter()
                .filter(|r| r.expires_at > env.now)
                .count();
            let migrations_applied = Migrator::applied(env.conn)?.len();
            let storage_state = match svc.recovery.write_block_reason() {
                Some(_) => "READ_ONLY",
                None => "READY",
            };
            let last_backup = svc
                .backups
                .iter()
                .max_by_key(|(id, b)| (b.manifest.created_at, (*id).clone()))
                .map(|(id, b)| {
                    json!({ "backup_id": id, "state": b.manifest.state, "created_at": b.manifest.created_at })
                });
            let status = json!({
                "app_version": svc.config.app_version,
                "schema_version": SchemaVersion::V1,
                "storage": {
                    "state": storage_state,
                    "migrations_applied": migrations_applied,
                    "wal": svc.storage_wal,
                },
                "model": model,
                "network": { "egress_allowed": false },
                "clock": {
                    "now": env.now,
                    "zone": svc.config.zone.as_str(),
                    "monotonic_ok": svc.monotonic_ok(),
                },
                "transport": svc.config.transport.as_str(),
                "pending_approvals": pending,
                "last_backup": last_backup,
            });
            let mut mutation = Mutation::new(to_value(&status)?, None);
            if svc.backups.is_empty() {
                let recorded = migrations::table_counts(env.conn)?
                    .get("backup_manifests")
                    .copied()
                    .unwrap_or(0);
                if recorded > 0 {
                    mutation = mutation.with_warning(format!(
                        "system.status.last_backup_process_scoped: {recorded} backup manifest(s) recorded by an earlier process are not read (no manifest reader); last_backup covers this process only"
                    ));
                }
            }
            Ok(mutation)
        },
    )
}
