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
//!
//! Attachment manifest (ADR-0001, runbook step 2): every verification and
//! restore test checks attachments under `AppConfig::attachment_dir` (when it
//! lies inside an approved root). A snapshot that holds attachment rows which
//! were not checked (no attachment directory configured) is never `CHECKED`:
//! the manifest moves to `FAILED` with reason `backup.attachments_not_checked`.
//! Hashing attachment bytes against `content_sha256` is a `g9-storage`
//! verification change (PROPOSED).
//!
//! `backup_manifests` is the source of truth. `backup.verify` and
//! `backup.restore_test` read the manifest (state, path, hash, anchor,
//! verification report) and its stored revision with
//! `g9_storage::backup::read_manifest`, so a backup created by an earlier
//! process, or by another front end on the same data directory, can be verified,
//! restore-tested and retained. An unknown id is `NOT_FOUND backup.not_found`; a
//! stored row that cannot be decoded is `DATA_CORRUPTION storage.row_decode`; a
//! recorded snapshot path that is relative or outside the approved roots is refused
//! (`UNAUTHORIZED backup.manifest_path_outside_approved_roots`: stored content is
//! data, never a licence to read an arbitrary file) before any state changes. The operator label is not
//! persisted: a manifest loaded from the database has `label: null`.
//!
//! The in-memory backup map (`Services::backups`) only remembers the operator
//! label of backups this process created and is updated only after the command
//! transaction committed (`Mutation::on_commit`), so a rolled-back command never
//! leaves it ahead of the database.

use super::{decode, parse_id, rc, to_value};
use crate::context::{AppContext, KnownBackup, Services};
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation};
use g9_domain::{
    BackupId, BackupState, Classification, CommandMeta, CommandOutcome, DataDomain, G9Error,
    ResourceKind, ResourceRef, Revision,
};
use g9_storage::backup::{
    self, AttachmentCheck, SnapshotRequest, VerificationReport, VerifyOptions,
};
use g9_storage::BackupManifest;
use rusqlite::Connection;
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
        .is_some_and(|roots| roots.contains_directory(dir))
    {
        return Err(G9Error::unauthorized(
            format!("backup.{what}_outside_approved_roots"),
            format!("the {what} directory is not inside an approved root"),
        ));
    }
    Ok(dir.clone())
}

/// Verification options for every backup check: the attachment directory,
/// when configured inside an approved root.
pub(crate) fn verify_options(svc: &Services) -> VerifyOptions {
    VerifyOptions {
        attachment_root: svc.config.attachment_dir.clone().filter(|dir| {
            svc.approved_roots
                .as_ref()
                .is_some_and(|roots| roots.contains_directory(dir))
        }),
    }
}

/// Reason recorded when attachment rows exist but were not checked.
pub(crate) const ATTACHMENTS_NOT_CHECKED: &str = "backup.attachments_not_checked";

/// Whether a report holds attachment rows whose files were never checked (an
/// unchecked manifest is not a passed check).
pub(crate) fn attachments_unchecked(report: &VerificationReport) -> bool {
    matches!(report.attachment_manifest, AttachmentCheck::NotChecked)
        && report.table_counts.get("attachments").copied().unwrap_or(0) > 0
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

/// Load a manifest from `backup_manifests` (size from the file's metadata, label
/// `None`, revision as stored so that the next `record_manifest` update lines up).
fn load_known(
    svc: &Services,
    conn: &Connection,
    id: &BackupId,
) -> Result<Option<KnownBackup>, G9Error> {
    let Some(stored) = backup::read_manifest(conn, id)? else {
        return Ok(None);
    };
    if let Some(path) = &stored.manifest.path {
        // `contains` is lexical and places a relative path under the primary root,
        // but the file operations that follow use the path as stored (relative to
        // the process directory), so a relative path would read an arbitrary file
        // there and drive the manifest to FAILED. Only an absolute path inside an
        // approved root is accepted.
        if !path.is_absolute()
            || !svc
                .approved_roots
                .as_ref()
                .is_some_and(|roots| roots.contains(path))
        {
            return Err(G9Error::unauthorized(
                "backup.manifest_path_outside_approved_roots",
                "the recorded snapshot path is not inside an approved root",
            ));
        }
    }
    let size_bytes = stored
        .manifest
        .path
        .as_ref()
        .and_then(|path| std::fs::metadata(path).ok())
        .map(|meta| meta.len());
    Ok(Some(KnownBackup {
        manifest: stored.manifest,
        revision: stored.revision,
        label: None,
        size_bytes,
    }))
}

/// The backup the command acts on: the database row is authoritative (state and
/// revision, also when another process advanced it); the in-memory entry of this
/// process only contributes the operator label. An entry without a row is used
/// as is.
fn resolve_known(
    svc: &Services,
    conn: &Connection,
    id: &BackupId,
) -> Result<Option<KnownBackup>, G9Error> {
    let remembered = svc.backups.get(id);
    match load_known(svc, conn, id)? {
        Some(mut loaded) => {
            if let Some(remembered) = remembered {
                loaded.label = remembered.label.clone();
            }
            Ok(Some(loaded))
        }
        None => Ok(remembered.cloned()),
    }
}

fn not_found() -> G9Error {
    G9Error::not_found("backup.not_found", "backup not found")
}

fn known_facts(svc: &Services, conn: &Connection, id: &BackupId) -> Result<Facts, G9Error> {
    let known = resolve_known(svc, conn, id)?.ok_or_else(not_found)?;
    Ok(Facts::existing(
        rc(
            DataDomain::System,
            Classification::Internal,
            known.manifest.state.is_terminal(),
        ),
        known.revision,
    ))
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
            let mut known = resolve_known(env.svc, env.conn, &id)?.ok_or_else(not_found)?;
            let options = verify_options(env.svc);
            let kind = match &scratch {
                None => {
                    if known.manifest.state != BackupState::SnapshotCreated {
                        return Err(illegal(known.manifest.state, "CHECK"));
                    }
                    // A failed or impossible verification drives the manifest to FAILED.
                    match backup::verify_and_advance(&mut known.manifest, &options) {
                        Ok(report) if attachments_unchecked(&report) => {
                            if known.manifest.state == BackupState::Checked {
                                known.manifest.fail(ATTACHMENTS_NOT_CHECKED)?;
                            }
                        }
                        Ok(_) => {}
                        Err(e) => {
                            if known.manifest.state == BackupState::SnapshotCreated {
                                known.manifest.fail(e.code())?;
                            }
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
            let key = id.clone();
            Ok(Mutation::new(result, Some(revision))
                .with_event(EventSpec::system(
                    kind,
                    json!({ "backup_id": id, "state": state }),
                ))
                // Only after the manifest row committed (no divergence on rollback).
                .on_commit(Box::new(move |svc| {
                    svc.backups.insert(key, known);
                })))
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
