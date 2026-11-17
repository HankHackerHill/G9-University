//! `system.status` (read-only). Every field reports observed state; nothing is
//! assumed: the model is `available: false, reason: NO_RUNTIME_CONFIGURED`
//! whenever the configured runtime has no loaded model (the default
//! `NullRuntime`), network egress is reported disabled because the mentor
//! runtime has no network capability at all, and storage is `READ_ONLY` when
//! recovery refused writes (broken audit chain or unapplied migrations).
//!
//! `last_backup` is the newest `RETAINED` manifest in `backup_manifests` (read
//! with `g9_storage::backup::list_manifests`, so it survives restarts and covers
//! backups made by other processes on the same data directory); it is `null`
//! when no manifest is `RETAINED`. A backup that was created but not yet
//! verified and restore-tested is not a usable backup and is not reported. When
//! the manifests cannot be read (a stored value does not decode) `last_backup` is
//! `null` and the outcome carries the warning
//! `system.status.last_backup_unreadable` with the storage error code, so an
//! unreadable ledger is never reported as "no backup".
//!
//! An approved deletion whose erasure did not complete (the process stopped
//! between `deletion.request`'s two commits, or the erasure failed) is reported
//! with the warning `deletion.erasure_incomplete` and the request ids while the
//! content is still stored (`crate::recovery` step 5); it disappears once the
//! content is erased by a fresh approved request. It is read live, except over
//! an unmigrated or drifted schema, where the tables may not be the ones the
//! query reads; a failed read is the warning
//! `deletion.erasure_incomplete_unreadable`, never "nothing pending".

use crate::commands::{rc, to_value};
use crate::context::AppContext;
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, Facts, Mutation};
use g9_domain::{
    ApprovalState, BackupState, Classification, CommandMeta, CommandOutcome, DataDomain, G9Error,
    ResourceKind, ResourceRef, Revision, SchemaVersion,
};
use g9_storage::approvals_store;
use g9_storage::{backup, deletion, Migrator};
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
            let (last_backup, backup_warning) = match backup::list_manifests(env.conn) {
                Ok(manifests) => (
                    // Newest first: the first RETAINED manifest is the last backup.
                    manifests
                        .into_iter()
                        .find(|stored| stored.manifest.state == BackupState::Retained)
                        .map(|stored| {
                            json!({
                                "backup_id": stored.manifest.backup_id,
                                "state": stored.manifest.state,
                                "created_at": stored.manifest.created_at,
                            })
                        }),
                    None,
                ),
                Err(e) => (
                    None,
                    Some(format!(
                        "system.status.last_backup_unreadable: backup manifests could not be read ({}); last_backup is not known",
                        e.code()
                    )),
                ),
            };
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
            if let Some(warning) = backup_warning {
                mutation = mutation.with_warning(warning);
            }
            if let Some(warning) = deletion_warning(env.conn, svc) {
                mutation = mutation.with_warning(warning);
            }
            Ok(mutation)
        },
    )
}

/// The `deletion.erasure_incomplete` warning (module docs), or `None` when nothing is pending.
fn deletion_warning(conn: &rusqlite::Connection, svc: &crate::context::Services) -> Option<String> {
    let schema_known =
        svc.recovery.pending_migrations.is_empty() && svc.recovery.schema_differences.is_empty();
    if !schema_known {
        return None;
    }
    match deletion::list_approved_unexecuted(conn) {
        Ok(pending) if pending.is_empty() => None,
        Ok(pending) => {
            let ids: Vec<String> = pending.into_iter().map(|r| r.id).collect();
            Some(format!(
                "deletion.erasure_incomplete: {} approved deletion request(s) were recorded but their erasure did not complete, so the content is still stored ({}); request the deletion again to erase it (a new approval is needed)",
                ids.len(),
                ids.join(", ")
            ))
        }
        Err(e) => Some(format!(
            "deletion.erasure_incomplete_unreadable: pending deletion requests could not be read ({}); whether an approved erasure is incomplete is not known",
            e.code()
        )),
    }
}
