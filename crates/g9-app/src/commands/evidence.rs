//! Evidence the operator attaches (M05): files and notes that become evidence
//! items, which `assessment.submit` takes as `evidence_refs`.
//!
//! # `evidence.attach_file`
//!
//! The bytes arrive base64 in the payload: the desktop UI reads the file the
//! operator picked in the system file chooser, so the backend never opens a
//! host path (no filesystem read capability; deny by default). The backend:
//! 1. decodes and checks the bytes with `g9_evidence::AttachmentRef::new`:
//!    executables, archives and packet captures are refused by signature
//!    (SEC-13); a declared type the bytes do not match is refused (the schema
//!    has no quarantine columns, so a quarantined attachment cannot be stored;
//!    PROPOSED);
//! 2. writes them to the content-addressed store under
//!    `AppConfig::attachment_dir` (`g9_storage::attachment_store::put`: durable,
//!    never outside the store, identical bytes stored once);
//! 3. records an `attachments` row and an `ATTACHMENT` evidence item in the
//!    command transaction. The bytes are written before the transaction
//!    commits; if the commit fails, the file stays unreferenced (harmless:
//!    content-addressed, and no row points at it).
//!
//! The attachment takes the data domain and classification of its note, so a
//! file on a security-sensitive note needs an approval like the note did
//! (`operator.security_sensitive.write.approval`). Without a note it is
//! LEARNING data. Its trust class is `IMPORTED_DOCUMENT`: data, never
//! instructions, never parsed (PDF included).
//!
//! # `evidence.from_note`, `evidence.list`
//!
//! A typed note becomes a `NOTE` evidence item; `evidence.list` lists items
//! (newest first, or those of one note) for the submission form.

use super::{creation_id, decode, parse_id, rc, scope_of};
use crate::context::AppContext;
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation};
use base64::Engine as _;
use g9_domain::hashing::sha256_hex;
use g9_domain::{
    AttachmentId, Classification, CommandMeta, CommandOutcome, DataDomain, EvidenceId, G9Error,
    NoteId, Provenance, ResourceKind, ResourceRef, SourceKind, TrustClass,
};
use g9_evidence::{AttachmentRef, EvidenceError, EvidenceItem, EvidenceKind};
use g9_storage::attachment_store;
use g9_storage::repos::evidence::{self as repo, AttachmentRow, EvidenceItemRow};
use g9_storage::ReadScope;
use serde::Deserialize;
use serde_json::{json, Value};

/// Largest file accepted through the IPC (decoded bytes). The store itself
/// accepts up to `g9_evidence::MAX_ATTACHMENT_BYTES`; the IPC carries the
/// bytes base64 inside one JSON message, so the cap here is lower.
pub const MAX_UPLOAD_BYTES: usize = 16 * 1024 * 1024;

/// Longest original file name kept (bytes).
const MAX_FILENAME_BYTES: usize = 255;

#[derive(Debug, Deserialize)]
struct AttachRequest {
    note_id: Option<String>,
    filename: String,
    media_type: String,
    content_base64: String,
    classification: Option<Classification>,
}

/// The original name, reduced to a plain file name: no separators, control
/// characters or dot-only names (private content; stored, never used as a path).
fn clean_filename(name: &str) -> Result<String, G9Error> {
    let trimmed = name.trim();
    let bad = trimmed.is_empty()
        || trimmed.len() > MAX_FILENAME_BYTES
        || trimmed
            .chars()
            .any(|c| c.is_control() || c == '/' || c == '\\')
        || trimmed.chars().all(|c| c == '.');
    if bad {
        return Err(G9Error::invalid_input(
            "attachment.filename_invalid",
            "the file name must be 1 to 255 bytes with no path separators or control characters",
        ));
    }
    Ok(trimmed.to_string())
}

fn decode_bytes(content: &str) -> Result<Vec<u8>, G9Error> {
    // 4 base64 characters carry 3 bytes; refuse before allocating.
    if content.len() / 4 * 3 > MAX_UPLOAD_BYTES + 3 {
        return Err(too_large());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(content.as_bytes())
        .map_err(|_| {
            G9Error::invalid_input(
                "attachment.content_invalid",
                "content_base64 is not valid base64 (standard alphabet, padded)",
            )
        })?;
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(too_large());
    }
    Ok(bytes)
}

fn too_large() -> G9Error {
    G9Error::invalid_input(
        "attachment.too_large",
        format!("a file attached through the app is at most {MAX_UPLOAD_BYTES} bytes"),
    )
}

fn attachment_error(e: EvidenceError) -> G9Error {
    match e {
        EvidenceError::EmptyContent => {
            G9Error::invalid_input("attachment.empty", "the file is empty")
        }
        EvidenceError::ContentTooLarge { .. } => too_large(),
        EvidenceError::ContentRefused { detected, .. } => G9Error::invalid_input(
            "attachment.refused",
            format!(
                "{detected}: executables, archives and packet captures are not stored; attach a written report instead"
            ),
        ),
        EvidenceError::MediaTypeInvalid { reason } => {
            G9Error::invalid_input("attachment.media_type_invalid", reason)
        }
        _ => G9Error::internal_redacted("attachment.validate"),
    }
}

/// The owning note's domain and classification (scope-checked).
struct Owner {
    note_id: Option<NoteId>,
    domain: DataDomain,
    classification: Classification,
}

fn owner(
    conn: &rusqlite::Connection,
    scope: &ReadScope,
    req: &AttachRequest,
) -> Result<Owner, G9Error> {
    match &req.note_id {
        Some(id) => {
            let id: NoteId = parse_id(id, "note_id")?;
            let note = repo::get_note(conn, scope, &id)?
                .ok_or_else(|| G9Error::not_found("note.not_found", "note not found"))?;
            if note.erased {
                return Err(G9Error::conflict(
                    "note.erased",
                    "the note is erased; nothing can be attached to it",
                ));
            }
            Ok(Owner {
                note_id: Some(id),
                domain: note.data_domain,
                classification: note.classification,
            })
        }
        None => Ok(Owner {
            note_id: None,
            domain: DataDomain::Learning,
            classification: req.classification.unwrap_or(Classification::Internal),
        }),
    }
}

fn attachment_root(env_svc: &crate::context::Services) -> Result<std::path::PathBuf, G9Error> {
    let dir = env_svc.config.attachment_dir.clone().ok_or_else(|| {
        G9Error::precondition_failed(
            "attachment.store_not_configured",
            "no attachment directory is configured",
        )
    })?;
    let inside = env_svc
        .approved_roots
        .as_ref()
        .is_some_and(|roots| roots.contains_directory(&dir));
    if !inside {
        return Err(G9Error::precondition_failed(
            "attachment.store_outside_approved_roots",
            "the attachment directory is not inside an approved root",
        ));
    }
    Ok(dir)
}

/// `evidence.attach_file`.
pub fn attach_file(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: AttachRequest = decode(payload)?;
    let filename = clean_filename(&req.filename)?;
    let bytes = decode_bytes(&req.content_base64)?;
    let attachment_id: AttachmentId = parse_id(
        &creation_id(ctx, spec, meta, || {
            AttachmentId::new_v7().as_str().to_string()
        })?,
        "attachment_id",
    )?;
    let resource = ResourceRef::new(ResourceKind::Attachment, attachment_id.as_str());
    let scope = scope_of(meta);
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |conn, svc| {
            attachment_root(svc)?;
            let o = owner(conn, &scope, &req)?;
            Ok(Facts::new_record(rc(o.domain, o.classification, false)))
        },
        |env| {
            let root = attachment_root(env.svc)?;
            let o = owner(env.conn, &scope, &req)?;
            let provenance = Provenance {
                source_kind: SourceKind::Attachment,
                source_ref: "ui:attach".to_string(),
                trust: TrustClass::ImportedDocument,
                content_sha256: None,
                recorded_at: env.now,
                recorded_by: env.meta.actor.id.clone(),
                version: None,
            };
            let reference = AttachmentRef::new(
                attachment_id.clone(),
                &bytes,
                &req.media_type,
                o.classification,
                provenance,
            )
            .map_err(attachment_error)?;
            if reference.quarantined {
                return Err(G9Error::invalid_input(
                    "attachment.content_mismatch",
                    format!(
                        "the file's bytes do not match {}; quarantine is not stored yet (PROPOSED), so it was not attached",
                        reference.media_type
                    ),
                ));
            }
            let stored = attachment_store::put(&root, &reference.sha256, &bytes)?;
            let row = AttachmentRow::from_ref(
                &reference,
                &filename,
                o.note_id.clone(),
                o.domain,
                env.now,
            )?;
            let revision = repo::insert_attachment(env.conn, &row)?;
            let item = EvidenceItem::new(
                EvidenceId::new_v7(),
                EvidenceKind::Attachment,
                attachment_id.as_str(),
                o.domain,
                o.classification,
                reference.provenance.clone(),
                filename.clone(),
                env.now,
            )
            .map_err(|_| G9Error::internal_redacted("attachment.evidence"))?;
            repo::insert_evidence_item(
                env.conn,
                &EvidenceItemRow::from_domain(&item, &reference.sha256)?,
            )?;
            let result = json!({
                "attachment_id": attachment_id,
                "evidence_id": item.id,
                "note_id": o.note_id,
                "filename": filename,
                "media_type": reference.media_type.as_str(),
                "size_bytes": reference.byte_len,
                "sha256": reference.sha256,
                "already_stored": stored == attachment_store::Stored::AlreadyPresent,
            });
            Ok(Mutation::new(result, Some(revision)).with_event(EventSpec {
                domain: o.domain,
                content_sha256: Some(reference.sha256.clone()),
                ..EventSpec::learning(
                    "attachment.added",
                    json!({ "attachment_id": attachment_id, "evidence_id": item.id, "note_id": o.note_id }),
                )
            }))
        },
    )
}

#[derive(Debug, Deserialize)]
struct FromNoteRequest {
    note_id: String,
}

/// `evidence.from_note`: a typed note becomes a `NOTE` evidence item.
pub fn from_note(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: FromNoteRequest = decode(payload)?;
    let note_id: NoteId = parse_id(&req.note_id, "note_id")?;
    let evidence_id: EvidenceId = parse_id(
        &creation_id(ctx, spec, meta, || {
            EvidenceId::new_v7().as_str().to_string()
        })?,
        "evidence_id",
    )?;
    let resource = ResourceRef::new(ResourceKind::EvidenceItem, evidence_id.as_str());
    let scope = scope_of(meta);
    let load = move |conn: &rusqlite::Connection| {
        let note = repo::get_note(conn, &scope, &note_id)?
            .ok_or_else(|| G9Error::not_found("note.not_found", "note not found"))?;
        if note.erased {
            return Err(G9Error::conflict("note.erased", "the note is erased"));
        }
        Ok(note)
    };
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |conn, _| {
            let note = load(conn)?;
            Ok(Facts::new_record(rc(
                note.data_domain,
                note.classification,
                false,
            )))
        },
        |env| {
            let note = load(env.conn)?;
            let sha = sha256_hex(note.body_markdown.as_bytes());
            let provenance = Provenance {
                source_kind: SourceKind::Note,
                source_ref: format!("note:{}", note.id.as_str()),
                trust: note.trust,
                content_sha256: Some(sha.clone()),
                recorded_at: env.now,
                recorded_by: env.meta.actor.id.clone(),
                version: None,
            };
            let summary: String = if note.title.trim().is_empty() {
                "Note".to_string()
            } else {
                note.title.chars().take(500).collect()
            };
            let item = EvidenceItem::new(
                evidence_id.clone(),
                EvidenceKind::Note,
                note.id.as_str(),
                note.data_domain,
                note.classification,
                provenance,
                summary.clone(),
                env.now,
            )
            .map_err(|_| G9Error::internal_redacted("evidence.from_note"))?;
            let revision =
                repo::insert_evidence_item(env.conn, &EvidenceItemRow::from_domain(&item, &sha)?)?;
            Ok(Mutation::new(
                json!({
                    "evidence_id": evidence_id,
                    "kind": "NOTE",
                    "note_id": note.id,
                    "summary": summary,
                }),
                Some(revision),
            )
            .with_event(EventSpec {
                domain: note.data_domain,
                ..EventSpec::learning(
                    "evidence.added",
                    json!({ "evidence_id": evidence_id, "note_id": note.id }),
                )
            }))
        },
    )
}

#[derive(Debug, Deserialize)]
struct ListRequest {
    note_id: Option<String>,
    limit: Option<u32>,
}

fn item_view(
    conn: &rusqlite::Connection,
    scope: &ReadScope,
    row: &EvidenceItemRow,
) -> Result<Value, G9Error> {
    let attachment = match &row.attachment_id {
        Some(id) => repo::get_attachment(conn, scope, id)?,
        None => None,
    };
    let kind =
        serde_json::to_value(row.kind).map_err(|_| G9Error::internal_redacted("evidence.kind"))?;
    Ok(json!({
        "evidence_id": row.id,
        "kind": kind,
        "note_id": row.note_id,
        "attachment_id": row.attachment_id,
        "summary": row.summary,
        "sha256": row.content_sha256,
        "filename": attachment.as_ref().filter(|a| !a.erased).map(|a| a.original_filename.clone()),
        "media_type": attachment.as_ref().map(|a| a.media_type.clone()),
        "size_bytes": attachment.as_ref().map(|a| a.size_bytes),
        "created_at": row.created_at,
        "invalidated": row.invalidated_at.is_some(),
    }))
}

/// `evidence.list`: evidence items of one note, or the newest items in scope.
pub fn list(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: ListRequest = decode(payload)?;
    let note: Option<NoteId> = req
        .note_id
        .as_deref()
        .map(|n| parse_id(n, "note_id"))
        .transpose()?;
    let limit = req.limit.unwrap_or(100).clamp(1, 500);
    let resource = ResourceRef::new(ResourceKind::EvidenceItem, "list");
    let scope = scope_of(meta);
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
                meta.scope.domain,
                Classification::Internal,
                false,
            )))
        },
        |env| {
            let rows = match &note {
                Some(id) => repo::list_note_and_attachment_evidence(env.conn, &scope, id)?,
                None => repo::list_recent_evidence(env.conn, &scope, limit)?,
            };
            let items = rows
                .iter()
                .take(limit as usize)
                .map(|r| item_view(env.conn, &scope, r))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Mutation::new(json!({ "items": items }), None))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_plain_names() {
        assert_eq!(clean_filename(" report.pdf ").unwrap(), "report.pdf");
        for bad in ["", "   ", "..", ".", "a/b", "a\\b", "a\u{0}b", "a\nb"] {
            assert!(clean_filename(bad).is_err(), "{bad:?}");
        }
        assert!(clean_filename(&"x".repeat(256)).is_err());
    }

    #[test]
    fn base64_is_strict_and_capped() {
        assert_eq!(decode_bytes("aGk=").unwrap(), b"hi");
        assert_eq!(
            decode_bytes("aGk").unwrap_err().code,
            "attachment.content_invalid"
        );
        assert_eq!(
            decode_bytes("a$k=").unwrap_err().code,
            "attachment.content_invalid"
        );
        let over = "A".repeat((MAX_UPLOAD_BYTES / 3 + 2) * 4);
        assert_eq!(
            decode_bytes(&over).unwrap_err().code,
            "attachment.too_large"
        );
    }
}
