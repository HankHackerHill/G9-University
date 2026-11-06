//! Backup commands (M17; ARCHITECTURE.md §6 "Backup", "Restore"; ADR-0001; gate A08).
//!
//! The `BackupState` machine is driven only through `g9_storage::backup`:
//! `backup.create` takes an Online Backup API snapshot (Requested →
//! SnapshotCreated) *after* the command transaction committed (the snapshot
//! reads the live connection, so it must not run inside an open write
//! transaction); `backup.verify` checks the copy (→ Checked or → Failed);
//! `backup.restore_test` restores into a disposable copy under the scratch
//! directory, verifies it (→ RestoreTested) and retains the set (→ Retained),
//! then removes the disposable copy. Every state is persisted with
//! `record_manifest`. Backup and scratch directories must lie inside an
//! approved root. A failed verification or restore test is reported as the
//! `FAILED` state (the command itself succeeded in establishing that).

use super::{decode, parse_id, rc, to_value};
use crate::context::{AppContext, KnownBackup, Services};
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation};
use g9_domain::{
    BackupId, BackupState, Classification, CommandMeta, CommandOutcome, DataDomain, G9Error,
    ResourceKind, ResourceRef, Revision,
};
use g9_storage::backup::{self, SnapshotRequest, VerifyOptions};
use g9_storage::BackupManifest;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

const PLACEHOLDER_SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn view(id: &BackupId, known: &KnownBackup) -> Result<Value, G9Error> {
    let m = &known.manifest;
    let integrity = match (&m.verification, m.state) {
        (_, BackupState::Failed) => "FAILED",
        (Some(report), _) if report.ok() => "OK",
        (Some(_), _) => "FAILED",
        (None, _) => "UNKNOWN",
    };
    let sha = (m.sha256_of_file != PLACEHOLDER_SHA).then(|| m.sha256_of_file.clone());
    to_value(&json!({
        "backup_id": id,
        "state": m.state,
        "label": known.label,
        "created_at": m.created_at,
        "size_bytes": known.size_bytes,
        "integrity": integrity,
        "restore_tested_at": m.restore_tested_at,
        "manifest_sha256": sha,
        "revision": known.revision,
    }))
}

/// A configured host directory that lies inside an approved root.
fn approved_dir(svc: &Services, dir: Option<&PathBuf>, what: &str) -> Result<PathBuf, G9Error> {
    let dir = dir.ok_or_else(|| {
        G9Error::resource_unavailable(
            format!("backup.{what}_not_configured"),
            format!("no {what} directory is configured"),
        )
    })?;
    if !svc
        .approved_roots
        .as_ref()
        .is_some_and(|roots| roots.contains(dir))
    {
        return Err(G9Error::unauthorized(
            format!("backup.{what}_outside_approved_roots"),
            format!("the {what} directory is not inside an approved root"),
        ));
    }
    Ok(dir.clone())
}

#[derive(Debug, Deserialize)]
struct CreateRequest {
    label: Option<String>,
}

/// `backup.create`.
pub fn create(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: CreateRequest = decode(payload)?;
    let id = BackupId::new_v7();
    let resource = ResourceRef::new(ResourceKind::Backup, id.as_str());
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |_, svc| {
            approved_dir(svc, svc.config.backup_dir.as_ref(), "backup")?;
            Ok(Facts::new_record(rc(
                DataDomain::System,
                Classification::Internal,
                false,
            )))
        },
        move |env| {
            let dir = approved_dir(env.svc, env.svc.config.backup_dir.as_ref(), "backup")?;
            let label = req.label;
            let step: pipeline::PostCommit =
                Box::new(move |post| {
                    let request = SnapshotRequest {
                        backup_id: id.clone(),
                        requested_at: post.now,
                        app_version: post.svc.config.app_version.clone(),
                    };
                    let dest = dir.join(format!("{}.sqlite", id.as_str()));
                    let manifest = match backup::create_snapshot(post.db.conn(), &dest, &request) {
                        Ok(m) => m,
                        Err(e) => {
                            let mut failed = BackupManifest::requested(&request);
                            failed.fail(e.code())?;
                            backup::record_manifest(post.db.conn(), &failed, post.now)?;
                            post.svc.backups.insert(
                                id.clone(),
                                KnownBackup {
                                    manifest: failed,
                                    revision: Revision::INITIAL,
                                    label,
                                    size_bytes: None,
                                },
                            );
                            return Err(e.into());
                        }
                    };
                    backup::record_manifest(post.db.conn(), &manifest, post.now)?;
                    let known = KnownBackup {
                        size_bytes: std::fs::metadata(&dest).ok().map(|m| m.len()),
                        manifest,
                        revision: Revision::INITIAL,
                        label,
                    };
                    let result = view(&id, &known)?;
                    post.svc.backups.insert(id.clone(), known);
                    Ok(Mutation::new(result, Some(Revision::INITIAL)).with_event(
                        EventSpec::system("backup.snapshot_created", json!({ "backup_id": id })),
                    ))
                });
            Ok(Mutation::new(Value::Null, None).then(step))
        },
    )
}

#[derive(Debug, Deserialize)]
struct BackupRequest {
    backup_id: String,
}

fn known_facts(
    svc: &Services,
    conn: &rusqlite::Connection,
    id: &BackupId,
) -> Result<Facts, G9Error> {
    match svc.backups.get(id) {
        Some(known) => Ok(Facts::existing(
            rc(
                DataDomain::System,
                Classification::Internal,
                known.manifest.state.is_terminal(),
            ),
            known.revision,
        )),
        None => {
            if backup::manifest_state(conn, id)?.is_some() {
                Err(G9Error::precondition_failed(
                    "backup.manifest_unavailable",
                    "this backup was recorded by an earlier process; g9-storage exposes no manifest reader yet",
                ))
            } else {
                Err(G9Error::not_found("backup.not_found", "backup not found"))
            }
        }
    }
}

fn advance(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
    restore: bool,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: BackupRequest = decode(payload)?;
    let id: BackupId = parse_id(&req.backup_id, "backup_id")?;
    let resource = ResourceRef::new(ResourceKind::Backup, id.as_str());
    let facts_id = id.clone();
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        move |conn, svc| known_facts(svc, conn, &facts_id),
        move |env| {
            let scratch = if restore {
                Some(approved_dir(
                    env.svc,
                    env.svc.config.scratch_dir.as_ref(),
                    "scratch",
                )?)
            } else {
                None
            };
            let mut known = env
                .svc
                .backups
                .get(&id)
                .cloned()
                .ok_or_else(|| G9Error::not_found("backup.not_found", "backup not found"))?;
            let options = VerifyOptions::default();
            let kind = match &scratch {
                None => {
                    if known.manifest.state != BackupState::SnapshotCreated {
                        return Err(illegal(known.manifest.state, "CHECK"));
                    }
                    // A failed or impossible verification drives the manifest to FAILED.
                    if let Err(e) = backup::verify_and_advance(&mut known.manifest, &options) {
                        if known.manifest.state == BackupState::SnapshotCreated {
                            known.manifest.fail(e.code())?;
                        }
                    }
                    "backup.verified"
                }
                Some(dir) => {
                    if known.manifest.state != BackupState::Checked {
                        return Err(illegal(known.manifest.state, "TEST_RESTORE"));
                    }
                    match backup::restore_test(&mut known.manifest, dir, &options, env.now) {
                        Ok(copy) => {
                            known.manifest.retain()?;
                            if let Err(e) = std::fs::remove_file(&copy) {
                                tracing::warn!(error = %e.kind(), "restore-test copy was not removed");
                            }
                        }
                        Err(e) => {
                            if known.manifest.state == BackupState::Checked {
                                known.manifest.fail(e.code())?;
                            }
                        }
                    }
                    "backup.restore_tested"
                }
            };
            backup::record_manifest(env.conn, &known.manifest, env.now)?;
            known.revision = known.revision.next();
            let result = view(&id, &known)?;
            let state = known.manifest.state;
            let revision = known.revision;
            env.svc.backups.insert(id.clone(), known);
            Ok(
                Mutation::new(result, Some(revision)).with_event(EventSpec::system(
                    kind,
                    json!({ "backup_id": id, "state": state }),
                )),
            )
        },
    )
}

fn illegal(from: BackupState, event: &str) -> G9Error {
    G9Error::precondition_failed(
        "backup.illegal_transition",
        format!("backup: event {event} is not allowed in state {from}"),
    )
    .with_details(json!({ "machine": "backup", "from": from, "event": event }))
}

/// `backup.verify` (SnapshotCreated → Checked | Failed).
pub fn verify(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    advance(ctx, spec, meta, payload, false)
}

/// `backup.restore_test` (Checked → RestoreTested → Retained | Failed).
pub fn restore_test(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    advance(ctx, spec, meta, payload, true)
}
