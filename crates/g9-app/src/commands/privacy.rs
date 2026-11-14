//! Export and deletion requests (gate A10; ARCHITECTURE.md §6 "Deletion
//! semantics"; ADR-0002).
//!
//! Both are approval-gated (`Capability::DataExport` / `Capability::DataDelete`,
//! operator-only for deletion). Deletion of personal content runs
//! `g9_storage::deletion`: the approved command records a `deletion_requests`
//! row in the command transaction, then `execute_erasure` (which opens its own
//! `BEGIN IMMEDIATE`) redacts the content in place, replaces every stored digest
//! of that content, writes one tombstone per redacted row (its digest identifies
//! the tombstone and is never derived from the content: ADR-0017), invalidates
//! derived rows and removes the files of the erased attachments from the
//! attachment folder before it commits. If the erasure fails, the request stays
//! `REQUESTED` with its approval spent (fail closed: nothing is erased without a
//! fresh approval; a file already removed stays removed and the next erasure of
//! the record completes). Evidentiary records are never deleted (policy
//! `evidentiary.delete.deny`, storage `ON DELETE RESTRICT`).
//!
//! Export: the approved request writes `<exports>/<export_id>/` in the command
//! transaction (`g9_storage::export`: the selected domains, never a SECRET row,
//! PRIVATE rows only with `include_private`, erased rows left out, attachment bytes
//! copied and hash-checked, `manifest.json` last) and returns `READY` with the
//! folder. A failure writes nothing and rolls the command back, so the approval
//! is not spent. Attachments missing from the store are listed in the manifest
//! and reported as a warning.

use super::{decode, new_id, parse_id, rc, scope_of};
use crate::context::{AppContext, Services};
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation, PostCommit};
use g9_domain::{
    AttachmentId, Classification, CommandMeta, CommandOutcome, DataDomain, G9Error, NoteId,
    ResourceKind, ResourceRef,
};
use g9_storage::deletion::{self, DeletionState, DeletionSubjectKind, NewDeletionRequest};
use g9_storage::export::{self, ExportFormat, ExportHeader, ExportSelection};
use g9_storage::repos::evidence as evidence_repo;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct ExportRequest {
    domains: Vec<DataDomain>,
    format: ExportFormat,
    include_private: bool,
}

/// The attachment folder, when one is configured inside an approved root.
fn attachment_store_of(svc: &Services) -> Option<PathBuf> {
    let dir = svc.config.attachment_dir.clone()?;
    svc.approved_roots
        .as_ref()
        .is_some_and(|roots| roots.contains_directory(&dir))
        .then_some(dir)
}

fn export_dir(svc: &Services) -> Result<PathBuf, G9Error> {
    let dir = svc.config.export_dir.clone().ok_or_else(|| {
        G9Error::precondition_failed(
            "export.dir_not_configured",
            "no export directory is configured",
        )
    })?;
    if !svc
        .approved_roots
        .as_ref()
        .is_some_and(|roots| roots.contains_directory(&dir))
    {
        return Err(G9Error::precondition_failed(
            "export.dir_outside_approved_roots",
            "the export directory is not inside an approved root",
        ));
    }
    Ok(dir)
}

/// `export.request` (approval-gated: `DataExport`).
pub fn export_request(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: ExportRequest = decode(payload)?;
    let domains: BTreeSet<DataDomain> = req.domains.iter().copied().collect();
    let label: Vec<String> = domains
        .iter()
        .filter_map(|d| serde_json::to_value(d).ok())
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    // The resource names the selection, the privacy ceiling and the format,
    // so the native confirmation shows what leaves (the approval also binds
    // the exact payload).
    let ceiling = if req.include_private {
        "PRIVATE"
    } else {
        "INTERNAL"
    };
    let format_label = match req.format {
        ExportFormat::Json => "JSON",
        ExportFormat::Csv => "CSV",
    };
    let resource = ResourceRef::new(
        ResourceKind::Export,
        // Domains joined with `.`: the id must match the resource-id pattern
        // `[A-Za-z0-9._:-]` of the approvals schemas (a `+` made the pending
        // request unlistable and so unapprovable).
        format!("export:{}:{ceiling}:{format_label}", label.join(".")),
    );
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |_, _| {
            // The export directory is checked after policy (in the mutation), so
            // the approval request comes first and a refusal there rolls back
            // without spending the approval.
            let classification = if req.include_private {
                Classification::Private
            } else {
                Classification::Internal
            };
            Ok(Facts::new_record(rc(
                DataDomain::System,
                classification,
                false,
            )))
        },
        |env| {
            let dir = export_dir(env.svc)?;
            let export_id = new_id("export");
            let selection = ExportSelection {
                domains: domains.clone(),
                include_private: req.include_private,
            };
            let data = export::collect(env.conn, &selection)?;
            let attachment_root = env.svc.config.attachment_dir.clone().filter(|d| {
                env.svc
                    .approved_roots
                    .as_ref()
                    .is_some_and(|roots| roots.contains_directory(d))
            });
            let written = export::write(
                &dir,
                &ExportHeader {
                    export_id: export_id.clone(),
                    created_at: env.now,
                    app_version: env.svc.config.app_version.clone(),
                    selection,
                    format: req.format,
                },
                &data,
                attachment_root.as_deref(),
            )?;
            let missing = written.attachments.missing.len() + written.attachments.corrupt.len();
            let result = json!({
                "export_id": export_id,
                "state": "READY",
                "domains": label,
                "format": req.format,
                "requested_at": env.now,
                "location": written.dir.display().to_string(),
                "files": written.files,
                "rows": written.rows,
                "attachments": written.attachments.written + written.attachments.already_present,
                "manifest_sha256": written.manifest_sha256,
            });
            let mut m = Mutation::new(result, Some(g9_domain::Revision::INITIAL))
                .with_audit_resource(ResourceRef::new(ResourceKind::Export, export_id.as_str()))
                .with_event(EventSpec::system(
                    "export.requested",
                    json!({
                        "export_id": export_id,
                        "domains": label,
                        "format": req.format,
                        "include_private": req.include_private,
                        "approval_id": env.consumed.as_ref().map(|c| c.approval_id.clone()),
                    }),
                ))
                .with_event(EventSpec::system(
                    "export.written",
                    json!({
                        "export_id": export_id,
                        "domains": label,
                        "format": req.format,
                        "include_private": req.include_private,
                        "rows": written.rows,
                        "files": written.files,
                        "manifest_sha256": written.manifest_sha256,
                        "approval_id": env.consumed.as_ref().map(|c| c.approval_id.clone()),
                    }),
                ));
            if missing > 0 {
                m = m.with_warning(format!(
                    "export.attachments_missing: {missing} attachment file(s) were missing or damaged in the store and are listed in manifest.json"
                ));
            }
            Ok(m)
        },
    )
}

#[derive(Debug, Deserialize)]
struct DeletionRequest {
    resource: ResourceRef,
    reason: String,
}

fn subject_kind(kind: ResourceKind) -> Option<DeletionSubjectKind> {
    match kind {
        ResourceKind::Note => Some(DeletionSubjectKind::Note),
        ResourceKind::Attachment => Some(DeletionSubjectKind::Attachment),
        _ => None,
    }
}

const fn evidentiary(kind: ResourceKind) -> bool {
    matches!(
        kind,
        ResourceKind::Attempt
            | ResourceKind::GradeDecision
            | ResourceKind::Observation
            | ResourceKind::AuditMetadata
            | ResourceKind::Approval
    )
}

/// `deletion.request` (approval-gated: `DataDelete`).
pub fn deletion_request(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: DeletionRequest = decode(payload)?;
    let target = req.resource.clone();
    let scope = scope_of(meta);
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &target,
            payload,
        },
        |conn, _| {
            let kind = req.resource.kind;
            if evidentiary(kind) {
                // Let policy refuse it (`evidentiary.delete.deny`), logged as a DENY.
                return Ok(Facts::new_record(rc(
                    DataDomain::Learning,
                    Classification::Internal,
                    true,
                )));
            }
            match subject_kind(kind) {
                Some(DeletionSubjectKind::Note) => {
                    let id: NoteId = parse_id(&req.resource.id, "resource.id")?;
                    let note = evidence_repo::get_note(conn, &scope, &id)?
                        .ok_or_else(|| G9Error::not_found("note.not_found", "note not found"))?;
                    if note.erased {
                        return Err(G9Error::conflict(
                            "deletion.already_erased",
                            "the note is already erased",
                        ));
                    }
                    Ok(Facts::existing(
                        rc(note.data_domain, note.classification, false),
                        note.revision,
                    ))
                }
                Some(_) => {
                    let id: AttachmentId = parse_id(&req.resource.id, "resource.id")?;
                    let att =
                        evidence_repo::get_attachment(conn, &scope, &id)?.ok_or_else(|| {
                            G9Error::not_found("attachment.not_found", "attachment not found")
                        })?;
                    Ok(Facts::existing(
                        rc(att.data_domain, att.classification, false),
                        att.revision,
                    ))
                }
                None if kind == ResourceKind::StudySession => Err(G9Error::precondition_failed(
                    "deletion.study_session.not_implemented",
                    "erasure of study sessions is PROPOSED (the timer ledger is evidentiary)",
                )),
                None => Err(G9Error::precondition_failed(
                    "deletion.unsupported_subject",
                    "only notes and attachments can be erased through a deletion request",
                )),
            }
        },
        |env| {
            let subject = subject_kind(req.resource.kind).ok_or_else(|| {
                G9Error::precondition_failed(
                    "deletion.unsupported_subject",
                    "unsupported deletion subject",
                )
            })?;
            let approval_id = env.consumed.as_ref().map(|c| c.approval_id.clone());
            let request_id = new_id("del");
            let row = deletion::create_request(
                env.conn,
                &NewDeletionRequest {
                    id: request_id.clone(),
                    subject_kind: subject,
                    subject_id: req.resource.id.clone(),
                    requested_by: env.meta.actor.id.clone(),
                    requested_at: env.now,
                    reason: Some(req.reason.clone()),
                    approval_id,
                },
            )?;
            let resource = req.resource.clone();
            let requested_at = row.requested_at;
            let step: PostCommit = Box::new(move |post| {
                let executor = post.meta.actor.id.clone();
                // The approval covered the erasure of this record and its files: the bytes
                // leave the application's own folder with the rows (ADR-0017).
                let store = attachment_store_of(post.svc);
                let report = deletion::execute_erasure_in_store(
                    post.db.conn_mut(),
                    &request_id,
                    &executor,
                    post.now,
                    store.as_deref(),
                )?;
                let tombstone = report
                    .tombstones
                    .iter()
                    .find(|t| t.row_id == resource.id)
                    .or_else(|| report.tombstones.first())
                    .map(|t| t.tombstone_sha256.clone());
                let stored = deletion::get_request(post.db.conn(), &request_id)?
                    .ok_or_else(|| G9Error::internal_redacted("deletion.reload"))?;
                let state = match stored.state {
                    DeletionState::Requested => "REQUESTED",
                    DeletionState::Executed => "APPLIED",
                    DeletionState::Rejected => "REFUSED",
                };
                let mut m = Mutation::new(
                    json!({
                        "deletion_id": request_id,
                        "state": state,
                        "resource": resource,
                        "tombstone_sha256": tombstone,
                        "requested_at": requested_at,
                    }),
                    Some(stored.revision),
                )
                .with_event(EventSpec::system(
                    "deletion.executed",
                    json!({
                        "deletion_id": request_id,
                        "resource": resource,
                        "redacted_rows": report.redacted_rows.len(),
                        "tombstones": report.tombstones.len(),
                        "invalidated_derived": report.invalidated_derived.len(),
                    }),
                ));
                // Files remain only when this process was not given the folder they are in.
                if !report.attachment_storage_refs_to_purge.is_empty() {
                    m = m.with_warning(
                        "deletion.attachment_bytes_not_purged: the record is erased, but no attachment folder inside an approved root is configured, so its file was not removed from disk",
                    );
                }
                Ok(m)
            });
            Ok(Mutation::new(Value::Null, None).then(step))
        },
    )
}
