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
//! Attachment check (ADR-0001, runbook step 2; STOR-2): every verification and
//! restore test hashes the file of every attachment the snapshot references in
//! `<backup_dir>/attachments`, the store `g9 restore` copies the files back
//! from (`g9_storage::backup::VerifyOptions::attachment_root`). A missing or
//! damaged file fails the backup (`FAILED`), reported by code and count in the
//! stored verification report (`backup.attachments_missing`,
//! `backup.attachments_damaged`). The live store (`AppConfig::attachment_dir`)
//! is only the source the files are copied from: a restore never reads it and
//! replaces a damaged file there, so it does not decide whether a backup
//! restores and is not consulted. A snapshot that holds attachment rows which
//! were not checked (no attachment directory configured) is never `CHECKED`:
//! the manifest moves to `FAILED` with reason `backup.attachments_not_checked`.
//!
//! Free space (STOR-4; runbook "Disk-full and power-fail"): `backup.create`
//! refuses to start, before anything is written, when the backup directory's
//! filesystem has less than `g9_storage::backup::BACKUP_FREE_SPACE_FACTOR` ×
//! the database size free (`RETRYABLE_STORAGE backup.insufficient_space`, with
//! `required_bytes` and `available_bytes` in the details). The reading comes
//! from `AppConfig::free_space`. Where the platform cannot tell, the backup
//! starts and says so (warning `backup.free_space_not_checked`).
//!
//! Attachment bytes: `backup.create` copies the bytes of every attachment the
//! database references into `<backup_dir>/attachments/ab/cd/<sha256>`
//! (`g9_storage::attachment_store::copy_all`: hash-checked while copying,
//! durable, identical bytes stored once for all backups). A missing or damaged
//! source file, or a failed copy, is a warning on the created backup; the
//! snapshot itself is kept. `g9 copy-backup` carries the referenced files to
//! the other drive and `g9 restore` puts them back into the live store.
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
use g9_storage::{attachment_store, BackupManifest};
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

/// Whether `dir` lies inside an approved root.
fn approved(svc: &Services, dir: &std::path::Path) -> bool {
    svc.approved_roots
        .as_ref()
        .is_some_and(|roots| roots.contains_directory(dir))
}

/// The live attachment store, when configured inside an approved root: the
/// source the referenced files are copied from.
fn live_attachment_root(svc: &Services) -> Option<PathBuf> {
    svc.config
        .attachment_dir
        .clone()
        .filter(|dir| approved(svc, dir))
}

/// Verification options for every backup check: the backup attachment store
/// `<backup_dir>/attachments` (the store a restore copies the files from), when
/// an attachment directory and the backup directory are configured inside
/// approved roots. Without an attachment directory nothing was ever copied
/// there, and the check is reported as not done (`NotChecked`).
pub(crate) fn verify_options(svc: &Services, manifest: &BackupManifest) -> VerifyOptions {
    // The store beside the snapshot is the one `g9 restore` copies from
    // (`<snapshot dir>/attachments`), which is the configured backup directory
    // only while the snapshot was written there. A snapshot without a path, or
    // whose store is not inside an approved root, gets no store: its
    // attachments are reported unchecked, never silently passed.
    let attachment_root = live_attachment_root(svc).and_then(|_| {
        manifest
            .path
            .as_ref()
            .and_then(|snapshot| snapshot.parent())
            .map(|dir| dir.join(BACKUP_ATTACHMENTS_DIR))
            .filter(|store| approved(svc, store))
    });
    VerifyOptions { attachment_root }
}

/// Directory beside the snapshots that holds the attachment bytes every
/// backup references (content-addressed, shared by all backups).
pub const BACKUP_ATTACHMENTS_DIR: &str = "attachments";

/// Copy the bytes of every attachment the database references from the live
/// store into `<backup_dir>/attachments`, hash-checked. `Ok(None)` when no
/// attachment directory is configured.
pub(crate) fn copy_attachments(
    conn: &rusqlite::Connection,
    svc: &Services,
    backup_dir: &std::path::Path,
) -> Result<Option<attachment_store::CopyReport>, G9Error> {
    let Some(root) = live_attachment_root(svc) else {
        return Ok(None);
    };
    let digests = attachment_store::referenced_digests(conn)?;
    if digests.is_empty() {
        return Ok(Some(attachment_store::CopyReport::default()));
    }
    Ok(Some(attachment_store::copy_all(
        &root,
        &backup_dir.join(BACKUP_ATTACHMENTS_DIR),
        &digests,
    )?))
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
            // Before anything is written: a refusal here rolls the command back
            // and records it (STOR-4).
            let space = backup::check_free_space(env.conn, &dir, env.svc.config.free_space)?;
            let label = req.label;
            let step: pipeline::PostCommit = Box::new(move |post| {
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
                let attachments = copy_attachments(post.db.conn(), post.svc, &dir);
                let known = KnownBackup {
                    size_bytes: std::fs::metadata(&dest).ok().map(|m| m.len()),
                    manifest,
                    revision: Revision::INITIAL,
                    label,
                };
                let result = view(&id, &known)?;
                post.svc.backups.insert(id.clone(), known);
                let copied = attachments
                    .as_ref()
                    .ok()
                    .and_then(Option::as_ref)
                    .map(|r| r.written + r.already_present);
                let mut m =
                    Mutation::new(result, Some(Revision::INITIAL)).with_event(EventSpec::system(
                        "backup.snapshot_created",
                        json!({ "backup_id": id, "attachments_copied": copied }),
                    ));
                if space.available_bytes.is_none() {
                    m = m.with_warning(
                        "backup.free_space_not_checked: the free space of the backup directory cannot be read on this platform; the backup started without the free-space check",
                    );
                }
                match attachments {
                    Ok(Some(r)) if !r.complete() => {
                        m = m.with_warning(format!(
                                "backup.attachments_incomplete: {} attachment file(s) missing and {} damaged in the store were not copied",
                                r.missing.len(),
                                r.corrupt.len()
                            ));
                    }
                    Ok(_) => {}
                    Err(e) => {
                        m = m.with_warning(format!(
                                "backup.attachments_not_copied: {}; the snapshot was written, the attachment files were not",
                                e.code
                            ));
                    }
                }
                Ok(m)
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
            let options = verify_options(env.svc, &known.manifest);
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
                            if let Err(e) = g9_storage::migrations::remove_scratch_copy(&copy) {
                                tracing::warn!(
                                    code = e.code(),
                                    "restore-test copy was not removed"
                                );
                            }
                        }
                        // Storage fails the manifest when the failure concerns the
                        // backup's content. A manifest still CHECKED means an
                        // environmental failure (scratch directory, disk, I/O): the
                        // command fails and the backup stays usable (storage-core-2).
                        Err(e) if known.manifest.state == BackupState::Checked => {
                            return Err(G9Error::from(e));
                        }
                        Err(_) => {}
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

/// `backup.list` (read-only): every backup recorded in the database, newest first, with the
/// operator label of those this process created. A recorded path outside the approved roots is
/// not followed (stored content is data, never a licence to read a file): such an entry is left
/// out and counted in `unlisted`.
pub fn list(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let resource = ResourceRef::new(ResourceKind::Backup, "list");
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |_, _| {
            Ok(Facts::new_record(rc(
                DataDomain::System,
                Classification::Internal,
                false,
            )))
        },
        |env| {
            let mut backups = Vec::new();
            let mut unlisted = 0u32;
            for stored in backup::list_manifests(env.conn)? {
                let id = stored.manifest.backup_id.clone();
                match resolve_known(env.svc, env.conn, &id) {
                    Ok(Some(known)) => backups.push(view(&id, &known)?),
                    Ok(None) => {}
                    Err(e) if e.code == "backup.manifest_path_outside_approved_roots" => {
                        unlisted += 1;
                    }
                    Err(e) => return Err(e),
                }
            }
            let mut m = Mutation::new(json!({ "backups": backups, "unlisted": unlisted }), None);
            if unlisted > 0 {
                m = m.with_warning(
                    "backup.manifest_path_outside_approved_roots: a recorded snapshot lies outside the approved roots and is not listed",
                );
            }
            Ok(m)
        },
    )
}
