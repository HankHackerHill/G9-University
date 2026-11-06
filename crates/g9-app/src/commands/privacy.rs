//! Export and deletion requests (gate A10; ARCHITECTURE.md §6 "Deletion
//! semantics"; ADR-0002).
//!
//! Both are approval-gated (`Capability::DataExport` / `Capability::DataDelete`,
//! operator-only for deletion). Deletion of personal content runs
//! `g9_storage::deletion`: the approved command records a `deletion_requests`
//! row in the command transaction, then `execute_erasure` (which opens its own
//! `BEGIN IMMEDIATE`) redacts the content in place, writes one tombstone per
//! redacted row with the pre-erasure hash and invalidates derived rows. If the
//! erasure fails, the request stays `REQUESTED` with its approval spent (fail
//! closed: nothing is erased without a fresh approval). Evidentiary records are
//! never deleted (policy `evidentiary.delete.deny`, storage `ON DELETE RESTRICT`).
//!
//! Export: the approved request is recorded as a `export.requested` event and
//! returned `REQUESTED`. Writing the export file is PROPOSED: `g9-storage`
//! offers only whole-database `VACUUM INTO`, which cannot honour the domain
//! selection or `include_private = false`, so no file is produced.

use super::{decode, new_id, parse_id, rc, scope_of};
use crate::context::AppContext;
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation, PostCommit};
use g9_domain::{
    AttachmentId, Classification, CommandMeta, CommandOutcome, DataDomain, G9Error, NoteId,
    ResourceKind, ResourceRef,
};
use g9_storage::deletion::{self, DeletionState, DeletionSubjectKind, NewDeletionRequest};
use g9_storage::repos::evidence as evidence_repo;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
struct ExportRequest {
    domains: Vec<DataDomain>,
    format: String,
    include_private: bool,
}

/// `export.request` (approval-gated: `DataExport`).
pub fn export_request(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: ExportRequest = decode(payload)?;
    let mut domains = req.domains.clone();
    domains.sort();
    domains.dedup();
    let label: Vec<String> = domains
        .iter()
        .filter_map(|d| serde_json::to_value(d).ok())
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    let resource = ResourceRef::new(ResourceKind::Export, format!("export:{}", label.join("+")));
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |_, _| {
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
            let export_id = new_id("export");
            let result = json!({
                "export_id": export_id,
                "state": "REQUESTED",
                "domains": req.domains,
                "format": req.format,
                "requested_at": env.now,
            });
            Ok(Mutation::new(result, Some(g9_domain::Revision::INITIAL))
                .with_audit_resource(ResourceRef::new(ResourceKind::Export, export_id.as_str()))
                .with_event(EventSpec::system(
                    "export.requested",
                    json!({
                        "export_id": export_id,
                        "domains": req.domains,
                        "format": req.format,
                        "include_private": req.include_private,
                        "approval_id": env.consumed.as_ref().map(|c| c.approval_id.clone()),
                    }),
                ))
                .with_warning("export.execution_not_implemented: the per-domain export writer is PROPOSED; no file was produced"))
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
                let report = deletion::execute_erasure(
                    post.db.conn_mut(),
                    &request_id,
                    &executor,
                    post.now,
                )?;
                let tombstone = report
                    .tombstones
                    .iter()
                    .find(|t| t.row_id == resource.id)
                    .or_else(|| report.tombstones.first())
                    .map(|t| t.content_sha256_before.clone());
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
                if !report.attachment_storage_refs_to_purge.is_empty() {
                    m = m.with_warning(
                        "deletion.attachment_bytes_not_purged: removing attachment files needs a FilesystemWrite approval; PROPOSED",
                    );
                }
                Ok(m)
            });
            Ok(Mutation::new(Value::Null, None).then(step))
        },
    )
}
