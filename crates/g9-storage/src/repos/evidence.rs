//! Evidence repository (M05; migration `0005_evidence`, tables `notes`,
//! `note_versions`, `attachments`, `evidence_items`, `artifact_links`,
//! `source_provenance`, `content_classifications`).
//!
//! * Notes keep their body inline (`notes.body_markdown` is the current
//!   revision, `note_versions` the append-only history). [`revise_note`] appends
//!   a version and bumps the note; nothing ever UPDATEs `note_versions` except
//!   the A10 erasure workflow in [`crate::deletion`].
//! * Attachments are metadata rows over the content-addressed store.
//! * Evidence items point at notes, attachments or sessions and carry a
//!   provenance; they are invalidated, never deleted.
//! * `source_provenance` is an immutable per-record ledger of
//!   `g9_domain::Provenance`; `content_classifications` is the append-only
//!   classification history whose latest row is cached on the record.
//!
//! DDL ↔ wave-1 gaps: `evidence_items.kind` lacks `OBSERVATION` and spells
//! `ATTEMPT_SUBMISSION`/`EXTERNAL` ([`EvidenceItemKind`]); `attachments` has
//! no quarantine columns, so a quarantined `AttachmentRef` is refused until the
//! operator classifies it; `g9_evidence::Note` stores content out of line while
//! the DDL stores it inline — [`load_note`] bridges the two.

use super::{
    finish_update, from_json, from_ms, i64_bool, ms, opt_from_ms, opt_parse_id, query_all,
    query_opt, required, rev_from_i64, rev_i64, to_json, u32_from_i64, u64_from_i64, ReadScope,
    RepoError, RepoResult,
};
use crate::error::{StorageError, StorageResult};
use crate::sql::{enum_from_text, enum_to_text, parse_id, require_sha256};
use g9_domain::hashing::sha256_hex;
use g9_domain::{
    ActorId, AttachmentId, Classification, DataDomain, EvidenceId, NoteId, Provenance,
    ResourceKind, ResourceRef, Revision, SessionId, SourceKind, TrustClass, UtcMillis,
};
use g9_evidence::{AttachmentRef, EvidenceItem, EvidenceKind, Note, NoteVersion};
use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};

/// Note kind (mirrors the `notes.kind` CHECK list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NoteKind {
    /// Learning journal.
    Journal,
    /// Lab log.
    LabLog,
    /// Reflection.
    Reflection,
    /// Report.
    Report,
    /// Other.
    Other,
}

/// A `notes` row (current revision inline).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteRow {
    /// Note id.
    pub id: NoteId,
    /// Title of the current revision.
    pub title: String,
    /// Body of the current revision (private content; never placed in errors).
    pub body_markdown: String,
    /// Hex SHA-256 of `body_markdown` (set by the repository).
    pub body_sha256: String,
    /// Kind.
    pub kind: NoteKind,
    /// Linked session.
    pub session_id: Option<SessionId>,
    /// Linked objective row.
    pub objective_id: Option<String>,
    /// Source kind of the content.
    pub source_kind: SourceKind,
    /// Trust class of the content.
    pub trust: TrustClass,
    /// Privacy domain.
    pub data_domain: DataDomain,
    /// Classification (cached; history in `content_classifications`).
    pub classification: Classification,
    /// Erased through a deletion request.
    pub erased: bool,
    /// Erasure instant.
    pub erased_at: Option<UtcMillis>,
    /// Erasure request id.
    pub erasure_request_id: Option<String>,
    /// Revision.
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

/// A `note_versions` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteVersionRow {
    /// Row id.
    pub id: String,
    /// Note.
    pub note_id: NoteId,
    /// 1-based version number.
    pub version_no: u32,
    /// Title at this version.
    pub title: String,
    /// Body at this version (empty once erased).
    pub body_markdown: String,
    /// Hex SHA-256 of the body as captured (kept after erasure).
    pub body_sha256: String,
    /// Author.
    pub author_id: ActorId,
    /// Erased flag.
    pub erased: bool,
    /// Row creation.
    pub created_at: UtcMillis,
}

const NOTE_COLUMNS: &str = "id, title, body_markdown, body_sha256, kind, session_id, objective_id, source_kind, trust, data_domain,
    classification, erased, erased_at, erasure_request_id, revision, created_at, updated_at";

fn map_note(r: &Row<'_>) -> StorageResult<NoteRow> {
    Ok(NoteRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        title: r.get("title")?,
        body_markdown: r.get("body_markdown")?,
        body_sha256: r.get("body_sha256")?,
        kind: enum_from_text(&r.get::<_, String>("kind")?)?,
        session_id: opt_parse_id(r.get("session_id")?)?,
        objective_id: r.get("objective_id")?,
        source_kind: enum_from_text(&r.get::<_, String>("source_kind")?)?,
        trust: enum_from_text(&r.get::<_, String>("trust")?)?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        erased: i64_bool(r.get("erased")?)?,
        erased_at: opt_from_ms(r.get("erased_at")?),
        erasure_request_id: r.get("erasure_request_id")?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert a note with its first revision (`note_versions.version_no = 1`).
/// `body_sha256` is computed here; the row's value is ignored.
pub fn insert_note(
    conn: &Connection,
    row: &NoteRow,
    author: &ActorId,
    version_row_id: &str,
) -> RepoResult<Revision> {
    let sha = sha256_hex(row.body_markdown.as_bytes());
    conn.execute(
        &format!(
            "INSERT INTO notes ({NOTE_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, NULL, NULL, 1, ?12, ?12)"
        ),
        params![
            row.id.as_str(),
            row.title,
            row.body_markdown,
            sha,
            enum_to_text(&row.kind)?,
            row.session_id.as_ref().map(|s| s.as_str().to_string()),
            row.objective_id,
            enum_to_text(&row.source_kind)?,
            enum_to_text(&row.trust)?,
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    conn.execute(
        "INSERT INTO note_versions (id, note_id, version_no, title, body_markdown, body_sha256, author_id, erased, created_at)
         VALUES (?1, ?2, 1, ?3, ?4, ?5, ?6, 0, ?7)",
        params![
            version_row_id,
            row.id.as_str(),
            row.title,
            row.body_markdown,
            sha,
            author.as_str(),
            ms(row.created_at)
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One note, scope-checked.
pub fn get_note(conn: &Connection, scope: &ReadScope, id: &NoteId) -> RepoResult<Option<NoteRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {NOTE_COLUMNS} FROM notes WHERE id = ?1"),
        &[&id.as_str()],
        map_note,
    )?;
    if let Some(r) = &row {
        scope.check("notes", r.data_domain)?;
    }
    Ok(row)
}

/// Notes of a session, domain-filtered in SQL.
pub fn list_notes_for_session(
    conn: &Connection,
    scope: &ReadScope,
    session_id: &SessionId,
) -> RepoResult<Vec<NoteRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {NOTE_COLUMNS} FROM notes WHERE session_id = ?1 AND (?2 = 1 OR data_domain = ?3)
             ORDER BY created_at, id"
        ),
        &[
            &session_id.as_str(),
            &scope.sql_flag(),
            &scope.sql_domain()?,
        ],
        map_note,
    )
}

/// Notes created in `[from, to)`, domain-filtered in SQL.
pub fn list_notes_between(
    conn: &Connection,
    scope: &ReadScope,
    from: UtcMillis,
    to: UtcMillis,
) -> RepoResult<Vec<NoteRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {NOTE_COLUMNS} FROM notes WHERE created_at >= ?1 AND created_at < ?2 AND (?3 = 1 OR data_domain = ?4)
             ORDER BY created_at, id"
        ),
        &[&ms(from), &ms(to), &scope.sql_flag(), &scope.sql_domain()?],
        map_note,
    )
}

/// Append a revision: new `note_versions` row (`version_no = current + 1`) and
/// the note's current title/body under a revision check. Erased notes refuse
/// revisions. Returns `(version_no, revision)`.
#[allow(clippy::too_many_arguments)]
pub fn revise_note(
    conn: &Connection,
    id: &NoteId,
    expected_revision: Revision,
    title: &str,
    body_markdown: &str,
    author: &ActorId,
    version_row_id: &str,
    now: UtcMillis,
) -> RepoResult<(u32, Revision)> {
    let scope = ReadScope::cross_domain(DataDomain::Learning);
    let current = required("notes", get_note(conn, &scope, id)?)?;
    if current.revision != expected_revision {
        return Err(RepoError::Storage(StorageError::StaleRevision {
            expected: Some(expected_revision),
            actual: current.revision,
        }));
    }
    if current.erased {
        return Err(RepoError::Storage(StorageError::Precondition(
            "notes.erased: an erased note cannot be revised".to_string(),
        )));
    }
    let max_version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version_no), 0) FROM note_versions WHERE note_id = ?1",
        params![id.as_str()],
        |r| r.get(0),
    )?;
    let next = u32_from_i64(max_version, "version_no")?.saturating_add(1);
    let sha = sha256_hex(body_markdown.as_bytes());
    let changed = conn.execute(
        "UPDATE notes SET title = ?3, body_markdown = ?4, body_sha256 = ?5, revision = revision + 1, updated_at = ?6
         WHERE id = ?1 AND revision = ?2 AND erased = 0",
        params![
            id.as_str(),
            rev_i64(expected_revision),
            title,
            body_markdown,
            sha,
            ms(now)
        ],
    )?;
    let revision = finish_update(conn, "notes", "id", id.as_str(), expected_revision, changed)?;
    conn.execute(
        "INSERT INTO note_versions (id, note_id, version_no, title, body_markdown, body_sha256, author_id, erased, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8)",
        params![
            version_row_id,
            id.as_str(),
            i64::from(next),
            title,
            body_markdown,
            sha,
            author.as_str(),
            ms(now)
        ],
    )?;
    Ok((next, revision))
}

/// Revision history of a note, oldest first. The versions carry the note's
/// content, so the note's `data_domain` is checked against `scope` first
/// (`CrossDomainRefused` otherwise); an unknown note has no versions.
pub fn list_note_versions(
    conn: &Connection,
    scope: &ReadScope,
    note_id: &NoteId,
) -> RepoResult<Vec<NoteVersionRow>> {
    if !super::check_parent_domain(conn, scope, "notes", "id", note_id.as_str())? {
        return Ok(Vec::new());
    }
    query_all(
        conn,
        "SELECT id, note_id, version_no, title, body_markdown, body_sha256, author_id, erased, created_at
         FROM note_versions WHERE note_id = ?1 ORDER BY version_no",
        &[&note_id.as_str()],
        |r| {
            Ok(NoteVersionRow {
                id: r.get("id")?,
                note_id: parse_id(&r.get::<_, String>("note_id")?)?,
                version_no: u32_from_i64(r.get("version_no")?, "version_no")?,
                title: r.get("title")?,
                body_markdown: r.get("body_markdown")?,
                body_sha256: r.get("body_sha256")?,
                author_id: parse_id(&r.get::<_, String>("author_id")?)?,
                erased: i64_bool(r.get("erased")?)?,
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

fn content_relpath(sha: &str) -> String {
    if sha.len() >= 4 {
        format!("{}/{}/{}", &sha[0..2], &sha[2..4], sha)
    } else {
        sha.to_string()
    }
}

/// The evidence crate's `Note` view: metadata plus the revision ledger, with
/// content hashes as tombstones for erased revisions.
pub fn load_note(conn: &Connection, scope: &ReadScope, id: &NoteId) -> RepoResult<Option<Note>> {
    let Some(note) = get_note(conn, scope, id)? else {
        return Ok(None);
    };
    let versions = list_note_versions(conn, scope, id)?;
    let created_by = versions
        .first()
        .map(|v| v.author_id.clone())
        .ok_or(StorageError::RowDecode("note without versions".to_string()))?;
    let current_version = versions.last().map(|v| v.version_no).unwrap_or(1);
    let mapped = versions
        .into_iter()
        .map(|v| NoteVersion {
            version: v.version_no,
            content_sha256: v.body_sha256.clone(),
            byte_len: v.body_markdown.len() as u64,
            provenance: Provenance {
                source_kind: note.source_kind,
                source_ref: v.id.clone(),
                trust: note.trust,
                content_sha256: Some(v.body_sha256.clone()),
                recorded_at: v.created_at,
                recorded_by: v.author_id.clone(),
                version: Some(v.version_no.to_string()),
            },
            created_at: v.created_at,
            redacted: v.erased,
            content_ref: if v.erased {
                None
            } else {
                Some(content_relpath(&v.body_sha256))
            },
            redacted_at: if v.erased { note.erased_at } else { None },
            redacted_by: None,
        })
        .collect();
    Ok(Some(Note {
        id: note.id,
        data_domain: note.data_domain,
        classification: note.classification,
        created_by,
        created_at: note.created_at,
        current_version,
        versions: mapped,
    }))
}

/// An `attachments` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentRow {
    /// Attachment id.
    pub id: AttachmentId,
    /// Owning note, if any.
    pub note_id: Option<NoteId>,
    /// Original file name (private content).
    pub original_filename: String,
    /// Normalized media type.
    pub media_type: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Hex SHA-256 of the bytes.
    pub content_sha256: String,
    /// Content-addressed relative path under the attachment root.
    pub storage_ref: String,
    /// Source kind.
    pub source_kind: SourceKind,
    /// Trust class.
    pub trust: TrustClass,
    /// Privacy domain.
    pub data_domain: DataDomain,
    /// Classification.
    pub classification: Classification,
    /// Erased flag.
    pub erased: bool,
    /// Erasure instant.
    pub erased_at: Option<UtcMillis>,
    /// Erasure request id.
    pub erasure_request_id: Option<String>,
    /// Revision.
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

impl AttachmentRow {
    /// Build a row from a validated `AttachmentRef`. Quarantined references are
    /// refused: the DDL has no quarantine columns, so the operator must classify
    /// (`release_from_quarantine`) before the metadata row is written.
    pub fn from_ref(
        attachment: &AttachmentRef,
        original_filename: &str,
        note_id: Option<NoteId>,
        data_domain: DataDomain,
        now: UtcMillis,
    ) -> StorageResult<Self> {
        if attachment.quarantined {
            return Err(StorageError::NotImplemented(
                "evidence.attachment.quarantine",
            ));
        }
        Ok(Self {
            id: attachment.id.clone(),
            note_id,
            original_filename: original_filename.to_string(),
            media_type: attachment.media_type.as_str().to_string(),
            size_bytes: attachment.byte_len,
            content_sha256: attachment.sha256.clone(),
            storage_ref: attachment.stored_relpath.clone(),
            source_kind: attachment.provenance.source_kind,
            trust: attachment.provenance.trust,
            data_domain,
            classification: attachment.classification,
            erased: false,
            erased_at: None,
            erasure_request_id: None,
            revision: Revision::INITIAL,
            created_at: now,
            updated_at: now,
        })
    }
}

const ATTACHMENT_COLUMNS: &str = "id, note_id, original_filename, media_type, size_bytes, content_sha256, storage_ref, source_kind,
    trust, data_domain, classification, erased, erased_at, erasure_request_id, revision, created_at, updated_at";

fn map_attachment(r: &Row<'_>) -> StorageResult<AttachmentRow> {
    Ok(AttachmentRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        note_id: opt_parse_id(r.get("note_id")?)?,
        original_filename: r.get("original_filename")?,
        media_type: r.get("media_type")?,
        size_bytes: u64_from_i64(r.get("size_bytes")?, "size_bytes")?,
        content_sha256: r.get("content_sha256")?,
        storage_ref: r.get("storage_ref")?,
        source_kind: enum_from_text(&r.get::<_, String>("source_kind")?)?,
        trust: enum_from_text(&r.get::<_, String>("trust")?)?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        erased: i64_bool(r.get("erased")?)?,
        erased_at: opt_from_ms(r.get("erased_at")?),
        erasure_request_id: r.get("erasure_request_id")?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert an attachment metadata row.
pub fn insert_attachment(conn: &Connection, row: &AttachmentRow) -> RepoResult<Revision> {
    require_sha256("content_sha256", &row.content_sha256)?;
    conn.execute(
        &format!(
            "INSERT INTO attachments ({ATTACHMENT_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, NULL, NULL, 1, ?12, ?12)"
        ),
        params![
            row.id.as_str(),
            row.note_id.as_ref().map(|n| n.as_str().to_string()),
            row.original_filename,
            row.media_type,
            super::i64_from_u64(row.size_bytes, "size_bytes")?,
            row.content_sha256,
            row.storage_ref,
            enum_to_text(&row.source_kind)?,
            enum_to_text(&row.trust)?,
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One attachment, scope-checked.
pub fn get_attachment(
    conn: &Connection,
    scope: &ReadScope,
    id: &AttachmentId,
) -> RepoResult<Option<AttachmentRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {ATTACHMENT_COLUMNS} FROM attachments WHERE id = ?1"),
        &[&id.as_str()],
        map_attachment,
    )?;
    if let Some(r) = &row {
        scope.check("attachments", r.data_domain)?;
    }
    Ok(row)
}

/// Attachments of a note, domain-filtered in SQL.
pub fn list_attachments_for_note(
    conn: &Connection,
    scope: &ReadScope,
    note_id: &NoteId,
) -> RepoResult<Vec<AttachmentRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {ATTACHMENT_COLUMNS} FROM attachments WHERE note_id = ?1 AND (?2 = 1 OR data_domain = ?3)
             ORDER BY created_at, id"
        ),
        &[&note_id.as_str(), &scope.sql_flag(), &scope.sql_domain()?],
        map_attachment,
    )
}

/// Evidence item kind (mirrors the `evidence_items.kind` CHECK list).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceItemKind {
    /// A note.
    Note,
    /// An attachment.
    Attachment,
    /// A study session.
    Session,
    /// An attempt's submission (linked through `attempt_evidence`).
    AttemptSubmission,
    /// Deterministic checker output.
    CheckerOutput,
    /// Reviewer comment.
    ReviewerComment,
    /// External reference (approved source).
    External,
}

impl EvidenceItemKind {
    /// Map the evidence crate's kind; `OBSERVATION` has no column value (gap).
    pub fn from_domain(kind: EvidenceKind) -> StorageResult<Self> {
        Ok(match kind {
            EvidenceKind::Note => Self::Note,
            EvidenceKind::Attachment => Self::Attachment,
            EvidenceKind::Attempt => Self::AttemptSubmission,
            EvidenceKind::Session => Self::Session,
            EvidenceKind::ExternalReference => Self::External,
            EvidenceKind::Observation => {
                return Err(StorageError::NotImplemented("evidence.kind.observation"))
            }
        })
    }

    /// The evidence crate's kind, if representable.
    pub const fn to_domain(self) -> Option<EvidenceKind> {
        Some(match self {
            Self::Note => EvidenceKind::Note,
            Self::Attachment => EvidenceKind::Attachment,
            Self::Session => EvidenceKind::Session,
            Self::AttemptSubmission => EvidenceKind::Attempt,
            Self::External => EvidenceKind::ExternalReference,
            Self::CheckerOutput | Self::ReviewerComment => return None,
        })
    }
}

/// An `evidence_items` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceItemRow {
    /// Evidence id.
    pub id: EvidenceId,
    /// Kind.
    pub kind: EvidenceItemKind,
    /// Referenced note.
    pub note_id: Option<NoteId>,
    /// Referenced attachment.
    pub attachment_id: Option<AttachmentId>,
    /// Referenced session.
    pub session_id: Option<SessionId>,
    /// Operator-facing summary.
    pub summary: String,
    /// Hex SHA-256 of the referenced content.
    pub content_sha256: String,
    /// Trust class (equals `provenance.trust`).
    pub trust: TrustClass,
    /// Provenance.
    pub provenance: Provenance,
    /// Privacy domain.
    pub data_domain: DataDomain,
    /// Classification.
    pub classification: Classification,
    /// Invalidation instant (content erased or superseded).
    pub invalidated_at: Option<UtcMillis>,
    /// Invalidation reason.
    pub invalidation_reason: Option<String>,
    /// Revision.
    pub revision: Revision,
    /// Row creation.
    pub created_at: UtcMillis,
    /// Last change.
    pub updated_at: UtcMillis,
}

impl EvidenceItemRow {
    /// Build a row from a validated `EvidenceItem`; `content_sha256` is the hash
    /// of the referenced content (the item's provenance hash when present).
    pub fn from_domain(item: &EvidenceItem, content_sha256: &str) -> StorageResult<Self> {
        let kind = EvidenceItemKind::from_domain(item.kind)?;
        let (note_id, attachment_id, session_id) = match kind {
            EvidenceItemKind::Note => (Some(parse_id::<NoteId>(&item.ref_id)?), None, None),
            EvidenceItemKind::Attachment => {
                (None, Some(parse_id::<AttachmentId>(&item.ref_id)?), None)
            }
            EvidenceItemKind::Session => (None, None, Some(parse_id::<SessionId>(&item.ref_id)?)),
            _ => (None, None, None),
        };
        Ok(Self {
            id: item.id.clone(),
            kind,
            note_id,
            attachment_id,
            session_id,
            summary: item.summary.clone(),
            content_sha256: content_sha256.to_string(),
            trust: item.provenance.trust,
            provenance: item.provenance.clone(),
            data_domain: item.data_domain,
            classification: item.classification,
            invalidated_at: None,
            invalidation_reason: None,
            revision: Revision::INITIAL,
            created_at: item.created_at,
            updated_at: item.created_at,
        })
    }

    /// The evidence crate's view. `ref_id` overrides the referenced id for kinds
    /// without a column (`ATTEMPT_SUBMISSION` via `attempt_evidence`, `EXTERNAL`
    /// defaults to `provenance.source_ref`).
    pub fn to_domain(&self, ref_id: Option<&str>) -> StorageResult<EvidenceItem> {
        let kind = self
            .kind
            .to_domain()
            .ok_or(StorageError::NotImplemented("evidence.kind.unmapped"))?;
        let resolved = match (ref_id, self.kind) {
            (Some(r), _) => r.to_string(),
            (None, EvidenceItemKind::Note) => self
                .note_id
                .as_ref()
                .map(|n| n.as_str().to_string())
                .ok_or_else(|| {
                    StorageError::RowDecode("note evidence without note_id".to_string())
                })?,
            (None, EvidenceItemKind::Attachment) => self
                .attachment_id
                .as_ref()
                .map(|a| a.as_str().to_string())
                .ok_or_else(|| {
                    StorageError::RowDecode("attachment evidence without attachment_id".to_string())
                })?,
            (None, EvidenceItemKind::Session) => self
                .session_id
                .as_ref()
                .map(|s| s.as_str().to_string())
                .ok_or_else(|| {
                    StorageError::RowDecode("session evidence without session_id".to_string())
                })?,
            (None, EvidenceItemKind::External) => self.provenance.source_ref.clone(),
            (None, _) => {
                return Err(StorageError::InvalidInput(
                    "ref_id required for this evidence kind".to_string(),
                ))
            }
        };
        EvidenceItem::new(
            self.id.clone(),
            kind,
            resolved,
            self.data_domain,
            self.classification,
            self.provenance.clone(),
            self.summary.clone(),
            self.created_at,
        )
        .map_err(|e| StorageError::RowDecode(format!("evidence item: {e}")))
    }
}

const EVIDENCE_COLUMNS: &str = "id, kind, note_id, attachment_id, session_id, summary, content_sha256, trust, provenance_json,
    data_domain, classification, invalidated_at, invalidation_reason, revision, created_at, updated_at";

fn map_evidence(r: &Row<'_>) -> StorageResult<EvidenceItemRow> {
    Ok(EvidenceItemRow {
        id: parse_id(&r.get::<_, String>("id")?)?,
        kind: enum_from_text(&r.get::<_, String>("kind")?)?,
        note_id: opt_parse_id(r.get("note_id")?)?,
        attachment_id: opt_parse_id(r.get("attachment_id")?)?,
        session_id: opt_parse_id(r.get("session_id")?)?,
        summary: r.get("summary")?,
        content_sha256: r.get("content_sha256")?,
        trust: enum_from_text(&r.get::<_, String>("trust")?)?,
        provenance: from_json(&r.get::<_, String>("provenance_json")?, "provenance_json")?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        invalidated_at: opt_from_ms(r.get("invalidated_at")?),
        invalidation_reason: r.get("invalidation_reason")?,
        revision: rev_from_i64(r.get("revision")?),
        created_at: from_ms(r.get("created_at")?),
        updated_at: from_ms(r.get("updated_at")?),
    })
}

/// Insert an evidence item. `trust` must equal `provenance.trust`.
pub fn insert_evidence_item(conn: &Connection, row: &EvidenceItemRow) -> RepoResult<Revision> {
    require_sha256("content_sha256", &row.content_sha256)?;
    if row.trust != row.provenance.trust {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "evidence trust column must equal provenance.trust".to_string(),
        )));
    }
    conn.execute(
        &format!(
            "INSERT INTO evidence_items ({EVIDENCE_COLUMNS})
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL, NULL, 1, ?12, ?12)"
        ),
        params![
            row.id.as_str(),
            enum_to_text(&row.kind)?,
            row.note_id.as_ref().map(|n| n.as_str().to_string()),
            row.attachment_id.as_ref().map(|a| a.as_str().to_string()),
            row.session_id.as_ref().map(|s| s.as_str().to_string()),
            row.summary,
            row.content_sha256,
            enum_to_text(&row.trust)?,
            to_json(&row.provenance)?,
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.created_at),
        ],
    )?;
    Ok(Revision::INITIAL)
}

/// One evidence item, scope-checked.
pub fn get_evidence_item(
    conn: &Connection,
    scope: &ReadScope,
    id: &EvidenceId,
) -> RepoResult<Option<EvidenceItemRow>> {
    let row = query_opt(
        conn,
        &format!("SELECT {EVIDENCE_COLUMNS} FROM evidence_items WHERE id = ?1"),
        &[&id.as_str()],
        map_evidence,
    )?;
    if let Some(r) = &row {
        scope.check("evidence_items", r.data_domain)?;
    }
    Ok(row)
}

/// Evidence items of a session, domain-filtered in SQL.
pub fn list_evidence_for_session(
    conn: &Connection,
    scope: &ReadScope,
    session_id: &SessionId,
) -> RepoResult<Vec<EvidenceItemRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {EVIDENCE_COLUMNS} FROM evidence_items WHERE session_id = ?1 AND (?2 = 1 OR data_domain = ?3)
             ORDER BY created_at, id"
        ),
        &[&session_id.as_str(), &scope.sql_flag(), &scope.sql_domain()?],
        map_evidence,
    )
}

/// Evidence items pointing at a note, domain-filtered in SQL.
pub fn list_evidence_for_note(
    conn: &Connection,
    scope: &ReadScope,
    note_id: &NoteId,
) -> RepoResult<Vec<EvidenceItemRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {EVIDENCE_COLUMNS} FROM evidence_items WHERE note_id = ?1 AND (?2 = 1 OR data_domain = ?3)
             ORDER BY created_at, id"
        ),
        &[&note_id.as_str(), &scope.sql_flag(), &scope.sql_domain()?],
        map_evidence,
    )
}

/// Evidence items of a note and of the note's attachments (an attachment's
/// item references the attachment, not the note), domain-filtered in SQL.
pub fn list_note_and_attachment_evidence(
    conn: &Connection,
    scope: &ReadScope,
    note_id: &NoteId,
) -> RepoResult<Vec<EvidenceItemRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {EVIDENCE_COLUMNS} FROM evidence_items
             WHERE (note_id = ?1 OR attachment_id IN (SELECT id FROM attachments WHERE note_id = ?1))
               AND (?2 = 1 OR data_domain = ?3)
             ORDER BY created_at, id"
        ),
        &[&note_id.as_str(), &scope.sql_flag(), &scope.sql_domain()?],
        map_evidence,
    )
}

/// The newest evidence items in scope, domain-filtered in SQL.
pub fn list_recent_evidence(
    conn: &Connection,
    scope: &ReadScope,
    limit: u32,
) -> RepoResult<Vec<EvidenceItemRow>> {
    query_all(
        conn,
        &format!(
            "SELECT {EVIDENCE_COLUMNS} FROM evidence_items WHERE (?1 = 1 OR data_domain = ?2)
             ORDER BY created_at DESC, id DESC LIMIT ?3"
        ),
        &[&scope.sql_flag(), &scope.sql_domain()?, &i64::from(limit)],
        map_evidence,
    )
}

/// Mark an evidence item invalid (content erased, superseded ...). Items are never deleted.
pub fn invalidate_evidence_item(
    conn: &Connection,
    id: &EvidenceId,
    expected_revision: Revision,
    reason: &str,
    now: UtcMillis,
) -> RepoResult<Revision> {
    super::require_non_empty("reason", reason)?;
    let changed = conn.execute(
        "UPDATE evidence_items SET invalidated_at = ?3, invalidation_reason = ?4, revision = revision + 1, updated_at = ?3
         WHERE id = ?1 AND revision = ?2 AND invalidated_at IS NULL",
        params![id.as_str(), rev_i64(expected_revision), ms(now), reason],
    )?;
    finish_update(
        conn,
        "evidence_items",
        "id",
        id.as_str(),
        expected_revision,
        changed,
    )
}

/// Relation of an artifact link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LinkRelation {
    /// The evidence supports the target.
    Supports,
    /// The evidence was produced by the target.
    ProducedBy,
    /// The evidence references the target.
    References,
}

/// An `artifact_links` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactLinkRow {
    /// Row id.
    pub id: String,
    /// Evidence item.
    pub evidence_item_id: EvidenceId,
    /// Target record.
    pub target: ResourceRef,
    /// Relation.
    pub relation: LinkRelation,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Link an evidence item to a record.
pub fn insert_artifact_link(conn: &Connection, row: &ArtifactLinkRow) -> RepoResult<()> {
    conn.execute(
        "INSERT INTO artifact_links (id, evidence_item_id, target_kind, target_id, relation, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            row.id,
            row.evidence_item_id.as_str(),
            enum_to_text(&row.target.kind)?,
            row.target.id,
            enum_to_text(&row.relation)?,
            ms(row.created_at)
        ],
    )?;
    Ok(())
}

/// Links of an evidence item.
pub fn list_artifact_links(
    conn: &Connection,
    evidence_item_id: &EvidenceId,
) -> RepoResult<Vec<ArtifactLinkRow>> {
    query_all(
        conn,
        "SELECT id, evidence_item_id, target_kind, target_id, relation, created_at
         FROM artifact_links WHERE evidence_item_id = ?1 ORDER BY created_at, id",
        &[&evidence_item_id.as_str()],
        map_link,
    )
}

/// Evidence linked to a target record.
pub fn list_links_to(conn: &Connection, target: &ResourceRef) -> RepoResult<Vec<ArtifactLinkRow>> {
    query_all(
        conn,
        "SELECT id, evidence_item_id, target_kind, target_id, relation, created_at
         FROM artifact_links WHERE target_kind = ?1 AND target_id = ?2 ORDER BY created_at, id",
        &[&enum_to_text(&target.kind)?, &target.id],
        map_link,
    )
}

fn map_link(r: &Row<'_>) -> StorageResult<ArtifactLinkRow> {
    let kind: ResourceKind = enum_from_text(&r.get::<_, String>("target_kind")?)?;
    Ok(ArtifactLinkRow {
        id: r.get("id")?,
        evidence_item_id: parse_id(&r.get::<_, String>("evidence_item_id")?)?,
        target: ResourceRef::new(kind, r.get::<_, String>("target_id")?),
        relation: enum_from_text(&r.get::<_, String>("relation")?)?,
        created_at: from_ms(r.get("created_at")?),
    })
}

/// A `source_provenance` row (immutable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceRow {
    /// Row id.
    pub id: String,
    /// Table of the described record.
    pub record_table: String,
    /// Id of the described record.
    pub record_id: String,
    /// The provenance.
    pub provenance: Provenance,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Append a provenance record for any row (never updated or deleted; trigger-enforced).
pub fn append_provenance(
    conn: &Connection,
    id: &str,
    record_table: &str,
    record_id: &str,
    provenance: &Provenance,
    now: UtcMillis,
) -> RepoResult<()> {
    super::opt_sha256("content_sha256", provenance.content_sha256.as_deref())?;
    conn.execute(
        "INSERT INTO source_provenance (id, record_table, record_id, source_kind, source_ref, trust, content_sha256, version,
            recorded_by, recorded_at, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            id,
            record_table,
            record_id,
            enum_to_text(&provenance.source_kind)?,
            provenance.source_ref,
            enum_to_text(&provenance.trust)?,
            provenance.content_sha256,
            provenance.version,
            provenance.recorded_by.as_str(),
            ms(provenance.recorded_at),
            ms(now),
        ],
    )?;
    Ok(())
}

/// Provenance records of a row, oldest first.
pub fn list_provenance(
    conn: &Connection,
    record_table: &str,
    record_id: &str,
) -> RepoResult<Vec<ProvenanceRow>> {
    query_all(
        conn,
        "SELECT id, record_table, record_id, source_kind, source_ref, trust, content_sha256, version, recorded_by,
                recorded_at, created_at
         FROM source_provenance WHERE record_table = ?1 AND record_id = ?2 ORDER BY recorded_at, id",
        &[&record_table, &record_id],
        |r| {
            Ok(ProvenanceRow {
                id: r.get("id")?,
                record_table: r.get("record_table")?,
                record_id: r.get("record_id")?,
                provenance: Provenance {
                    source_kind: enum_from_text(&r.get::<_, String>("source_kind")?)?,
                    source_ref: r.get("source_ref")?,
                    trust: enum_from_text(&r.get::<_, String>("trust")?)?,
                    content_sha256: r.get("content_sha256")?,
                    recorded_at: from_ms(r.get("recorded_at")?),
                    recorded_by: parse_id(&r.get::<_, String>("recorded_by")?)?,
                    version: r.get("version")?,
                },
                created_at: from_ms(r.get("created_at")?),
            })
        },
    )
}

/// A `content_classifications` row (append-only history).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentClassificationRow {
    /// Row id.
    pub id: String,
    /// Table of the classified record.
    pub record_table: String,
    /// Id of the classified record.
    pub record_id: String,
    /// Domain assigned.
    pub data_domain: DataDomain,
    /// Classification assigned.
    pub classification: Classification,
    /// Classifying actor.
    pub classified_by: ActorId,
    /// When.
    pub classified_at: UtcMillis,
    /// Reason.
    pub reason: Option<String>,
    /// Row creation.
    pub created_at: UtcMillis,
}

/// Append a classification decision (history only; see [`reclassify`] to also
/// update the cached columns of the record).
pub fn append_classification(conn: &Connection, row: &ContentClassificationRow) -> RepoResult<()> {
    conn.execute(
        "INSERT INTO content_classifications (id, record_table, record_id, data_domain, classification, classified_by,
            classified_at, reason, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            row.id,
            row.record_table,
            row.record_id,
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            row.classified_by.as_str(),
            ms(row.classified_at),
            row.reason,
            ms(row.created_at),
        ],
    )?;
    Ok(())
}

fn map_classification(r: &Row<'_>) -> StorageResult<ContentClassificationRow> {
    Ok(ContentClassificationRow {
        id: r.get("id")?,
        record_table: r.get("record_table")?,
        record_id: r.get("record_id")?,
        data_domain: enum_from_text(&r.get::<_, String>("data_domain")?)?,
        classification: enum_from_text(&r.get::<_, String>("classification")?)?,
        classified_by: parse_id(&r.get::<_, String>("classified_by")?)?,
        classified_at: from_ms(r.get("classified_at")?),
        reason: r.get("reason")?,
        created_at: from_ms(r.get("created_at")?),
    })
}

/// Classification history of a record, oldest first.
pub fn list_classifications(
    conn: &Connection,
    record_table: &str,
    record_id: &str,
) -> RepoResult<Vec<ContentClassificationRow>> {
    query_all(
        conn,
        "SELECT id, record_table, record_id, data_domain, classification, classified_by, classified_at, reason, created_at
         FROM content_classifications WHERE record_table = ?1 AND record_id = ?2 ORDER BY classified_at, id",
        &[&record_table, &record_id],
        map_classification,
    )
}

/// Latest classification of a record, if any.
pub fn latest_classification(
    conn: &Connection,
    record_table: &str,
    record_id: &str,
) -> RepoResult<Option<ContentClassificationRow>> {
    query_opt(
        conn,
        "SELECT id, record_table, record_id, data_domain, classification, classified_by, classified_at, reason, created_at
         FROM content_classifications WHERE record_table = ?1 AND record_id = ?2
         ORDER BY classified_at DESC, id DESC LIMIT 1",
        &[&record_table, &record_id],
        map_classification,
    )
}

/// Tables whose `data_domain`/`classification` columns cache the latest classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassifiedTable {
    /// `notes`.
    Notes,
    /// `attachments`.
    Attachments,
    /// `evidence_items`.
    EvidenceItems,
}

impl ClassifiedTable {
    const fn name(self) -> &'static str {
        match self {
            Self::Notes => "notes",
            Self::Attachments => "attachments",
            Self::EvidenceItems => "evidence_items",
        }
    }
}

/// Record a classification decision and update the cached columns of the record
/// under a revision check. `row.record_table` must name `table`.
pub fn reclassify(
    conn: &Connection,
    table: ClassifiedTable,
    expected_revision: Revision,
    row: &ContentClassificationRow,
) -> RepoResult<Revision> {
    if row.record_table != table.name() {
        return Err(RepoError::Storage(StorageError::InvalidInput(
            "classification row names a different table".to_string(),
        )));
    }
    let sql = format!(
        "UPDATE {} SET data_domain = ?3, classification = ?4, revision = revision + 1, updated_at = ?5
         WHERE id = ?1 AND revision = ?2",
        table.name()
    );
    let changed = conn.execute(
        &sql,
        params![
            row.record_id,
            rev_i64(expected_revision),
            enum_to_text(&row.data_domain)?,
            enum_to_text(&row.classification)?,
            ms(row.classified_at)
        ],
    )?;
    let revision = finish_update(
        conn,
        table.name(),
        "id",
        &row.record_id,
        expected_revision,
        changed,
    )?;
    append_classification(conn, row)?;
    Ok(revision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::testutil::{assert_immutable, db, learning, operator_id, sha, t};

    fn note(id: &str, domain: DataDomain) -> NoteRow {
        NoteRow {
            id: NoteId::parse(id).unwrap(),
            title: "Lab log".into(),
            body_markdown: "# Lab log\nkernel boots via GRUB".into(),
            body_sha256: String::new(),
            kind: NoteKind::LabLog,
            session_id: None,
            objective_id: None,
            source_kind: SourceKind::OperatorInput,
            trust: TrustClass::Operator,
            data_domain: domain,
            classification: Classification::Private,
            erased: false,
            erased_at: None,
            erasure_request_id: None,
            revision: Revision::INITIAL,
            created_at: t(0),
            updated_at: t(0),
        }
    }

    fn provenance(kind: SourceKind, trust: TrustClass, sha: Option<String>) -> Provenance {
        Provenance {
            source_kind: kind,
            source_ref: "note-1".into(),
            trust,
            content_sha256: sha,
            recorded_at: t(0),
            recorded_by: operator_id(),
            version: None,
        }
    }

    #[test]
    fn note_create_revise_and_load_ledger() {
        let db = db();
        let conn = db.conn();
        let row = note("note-1", DataDomain::Learning);
        insert_note(conn, &row, &operator_id(), "nv-1").unwrap();
        let stored = get_note(conn, &learning(), &row.id).unwrap().unwrap();
        assert_eq!(stored.body_sha256, sha(&row.body_markdown));
        assert_eq!(stored.revision, Revision::INITIAL);

        assert!(matches!(
            revise_note(
                conn,
                &row.id,
                Revision::new(3),
                "t",
                "b",
                &operator_id(),
                "nv-x",
                t(1)
            ),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        let (version_no, revision) = revise_note(
            conn,
            &row.id,
            Revision::INITIAL,
            "Lab log (rev)",
            "# Lab log\nkernel boots via GRUB, then initramfs",
            &operator_id(),
            "nv-2",
            t(1),
        )
        .unwrap();
        assert_eq!(version_no, 2);
        assert_eq!(revision, Revision::new(2));
        let after = get_note(conn, &learning(), &row.id).unwrap().unwrap();
        assert_eq!(after.title, "Lab log (rev)");
        assert_eq!(
            after.body_sha256,
            sha("# Lab log\nkernel boots via GRUB, then initramfs")
        );
        let versions = list_note_versions(conn, &learning(), &row.id).unwrap();
        // the history carries the note's content: other domains are refused
        assert!(matches!(
            list_note_versions(conn, &ReadScope::domain(DataDomain::Personal), &row.id),
            Err(RepoError::CrossDomainRefused { table: "notes", .. })
        ));
        assert!(
            list_note_versions(conn, &learning(), &NoteId::parse("missing").unwrap())
                .unwrap()
                .is_empty()
        );
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].version_no, 1);
        assert_eq!(versions[0].body_markdown, row.body_markdown);
        assert_eq!(versions[1].version_no, 2);
        // the history is append-only: no rewrite, no delete (only the A10 erasure form may redact)
        assert_immutable(
            conn,
            "UPDATE note_versions SET body_markdown = 'rewritten' WHERE version_no = 1",
        );
        assert_immutable(conn, "UPDATE note_versions SET erased = 1, body_markdown = '', body_sha256 = 'x' WHERE version_no = 1");
        assert_immutable(conn, "DELETE FROM note_versions WHERE version_no = 1");
        assert_eq!(
            list_note_versions(conn, &learning(), &row.id).unwrap(),
            versions
        );

        let loaded = load_note(conn, &learning(), &row.id).unwrap().unwrap();
        assert_eq!(loaded.current_version, 2);
        assert_eq!(loaded.versions.len(), 2);
        assert_eq!(loaded.created_by, operator_id());
        assert_eq!(loaded.versions[0].content_sha256, sha(&row.body_markdown));
        assert!(!loaded.versions[0].redacted);
        assert_eq!(
            loaded.versions[1].content_ref.as_deref(),
            Some(content_relpath(&after.body_sha256).as_str())
        );
        assert!(loaded.verify_invariants().is_ok());
        assert_eq!(
            list_notes_between(conn, &learning(), t(0), t(1))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn note_reads_honour_scope() {
        let db = db();
        let conn = db.conn();
        let mut personal = note("note-p", DataDomain::Personal);
        personal.session_id = Some(SessionId::parse("sess-x").unwrap());
        // sessions FK: insert a session first
        conn.execute(
            "INSERT INTO study_sessions (id, state, mode, zone, created_at, updated_at) VALUES ('sess-x', 'PLANNED', 'STUDY', 'UTC', 1, 1)",
            [],
        )
        .unwrap();
        insert_note(conn, &personal, &operator_id(), "nv-p").unwrap();
        let mut learning_note = note("note-l", DataDomain::Learning);
        learning_note.session_id = Some(SessionId::parse("sess-x").unwrap());
        insert_note(conn, &learning_note, &operator_id(), "nv-l").unwrap();

        assert!(matches!(
            get_note(conn, &learning(), &personal.id),
            Err(RepoError::CrossDomainRefused { table: "notes", .. })
        ));
        assert!(load_note(conn, &learning(), &personal.id).is_err());
        let sess = SessionId::parse("sess-x").unwrap();
        assert_eq!(
            list_notes_for_session(conn, &learning(), &sess)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            list_notes_for_session(conn, &ReadScope::cross_domain(DataDomain::Learning), &sess)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn attachment_roundtrip_from_ref_and_quarantine_gap() {
        let db = db();
        let conn = db.conn();
        insert_note(
            conn,
            &note("note-1", DataDomain::Learning),
            &operator_id(),
            "nv-1",
        )
        .unwrap();
        let bytes = b"# markdown body";
        let reference = AttachmentRef::new(
            AttachmentId::parse("att-1").unwrap(),
            bytes,
            "text/markdown",
            Classification::Private,
            provenance(SourceKind::OperatorInput, TrustClass::Operator, None),
        )
        .unwrap();
        assert!(!reference.quarantined);
        let row = AttachmentRow::from_ref(
            &reference,
            "notes.md",
            Some(NoteId::parse("note-1").unwrap()),
            DataDomain::Learning,
            t(0),
        )
        .unwrap();
        insert_attachment(conn, &row).unwrap();
        let stored = get_attachment(conn, &learning(), &row.id).unwrap().unwrap();
        assert_eq!(stored, row);
        assert_eq!(stored.content_sha256, sha256_hex(bytes));
        assert_eq!(stored.storage_ref, content_relpath(&stored.content_sha256));
        assert_eq!(
            list_attachments_for_note(conn, &learning(), &NoteId::parse("note-1").unwrap())
                .unwrap()
                .len(),
            1
        );

        let quarantined = AttachmentRef::new(
            AttachmentId::parse("att-2").unwrap(),
            b"plain text declared as png",
            "image/png",
            Classification::Private,
            provenance(SourceKind::OperatorInput, TrustClass::Operator, None),
        )
        .unwrap();
        assert!(quarantined.quarantined);
        assert!(matches!(
            AttachmentRow::from_ref(&quarantined, "x.png", None, DataDomain::Learning, t(0)),
            Err(StorageError::NotImplemented(
                "evidence.attachment.quarantine"
            ))
        ));
    }

    #[test]
    fn evidence_items_roundtrip_invalidate_and_adapters() {
        let db = db();
        let conn = db.conn();
        insert_note(
            conn,
            &note("note-1", DataDomain::Learning),
            &operator_id(),
            "nv-1",
        )
        .unwrap();
        let body_sha = sha("# Lab log\nkernel boots via GRUB");
        let item = EvidenceItem::new(
            EvidenceId::parse("evi-1").unwrap(),
            EvidenceKind::Note,
            "note-1",
            DataDomain::Learning,
            Classification::Private,
            provenance(
                SourceKind::Note,
                TrustClass::Operator,
                Some(body_sha.clone()),
            ),
            "lab log for day 1",
            t(0),
        )
        .unwrap();
        let row = EvidenceItemRow::from_domain(&item, &body_sha).unwrap();
        assert_eq!(row.note_id.as_ref().map(|n| n.as_str()), Some("note-1"));
        insert_evidence_item(conn, &row).unwrap();
        let stored = get_evidence_item(conn, &learning(), &row.id)
            .unwrap()
            .unwrap();
        assert_eq!(stored, row);
        assert_eq!(stored.to_domain(None).unwrap(), item);
        assert_eq!(
            list_evidence_for_note(conn, &learning(), &NoteId::parse("note-1").unwrap())
                .unwrap()
                .len(),
            1
        );

        // trust must match provenance
        let mut inconsistent = row.clone();
        inconsistent.id = EvidenceId::parse("evi-bad").unwrap();
        inconsistent.trust = TrustClass::ModelOutput;
        assert!(matches!(
            insert_evidence_item(conn, &inconsistent),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));

        // OBSERVATION kind has no column value (gap)
        let obs = EvidenceItem::new(
            EvidenceId::parse("evi-o").unwrap(),
            EvidenceKind::Observation,
            "obs-1",
            DataDomain::Learning,
            Classification::Internal,
            provenance(
                SourceKind::SystemGenerated,
                TrustClass::SystemDeterministic,
                None,
            ),
            "observation",
            t(0),
        )
        .unwrap();
        assert!(matches!(
            EvidenceItemRow::from_domain(&obs, &sha("x")),
            Err(StorageError::NotImplemented("evidence.kind.observation"))
        ));

        // attempt-submission evidence needs a ref override to map back
        let attempt_item = EvidenceItem::new(
            EvidenceId::parse("evi-a").unwrap(),
            EvidenceKind::Attempt,
            "attempt-1",
            DataDomain::Learning,
            Classification::Private,
            provenance(SourceKind::AssessmentSubmission, TrustClass::Operator, None),
            "submission",
            t(0),
        )
        .unwrap();
        let attempt_row = EvidenceItemRow::from_domain(&attempt_item, &sha("submission")).unwrap();
        assert_eq!(attempt_row.kind, EvidenceItemKind::AttemptSubmission);
        assert!(attempt_row.to_domain(None).is_err());
        assert_eq!(
            attempt_row.to_domain(Some("attempt-1")).unwrap(),
            attempt_item
        );

        let link = ArtifactLinkRow {
            id: "al-1".into(),
            evidence_item_id: row.id.clone(),
            target: ResourceRef::new(ResourceKind::StudySession, "sess-1"),
            relation: LinkRelation::ProducedBy,
            created_at: t(1),
        };
        insert_artifact_link(conn, &link).unwrap();
        assert_eq!(
            list_artifact_links(conn, &row.id).unwrap(),
            vec![link.clone()]
        );
        assert_eq!(
            list_links_to(
                conn,
                &ResourceRef::new(ResourceKind::StudySession, "sess-1")
            )
            .unwrap(),
            vec![link]
        );

        assert!(matches!(
            invalidate_evidence_item(conn, &row.id, Revision::new(2), "x", t(5)),
            Err(RepoError::Storage(StorageError::StaleRevision { .. }))
        ));
        let r2 = invalidate_evidence_item(conn, &row.id, Revision::INITIAL, "note erased", t(5))
            .unwrap();
        let invalid = get_evidence_item(conn, &learning(), &row.id)
            .unwrap()
            .unwrap();
        assert_eq!(invalid.invalidated_at, Some(t(5)));
        assert_eq!(invalid.invalidation_reason.as_deref(), Some("note erased"));
        assert!(matches!(
            invalidate_evidence_item(conn, &row.id, r2, "again", t(6)),
            Err(RepoError::Storage(StorageError::Precondition(_)))
        ));
    }

    #[test]
    fn provenance_ledger_is_append_only_and_classification_history_tracks_latest() {
        let db = db();
        let conn = db.conn();
        let row = note("note-1", DataDomain::Learning);
        insert_note(conn, &row, &operator_id(), "nv-1").unwrap();
        let p = provenance(
            SourceKind::OperatorInput,
            TrustClass::Operator,
            Some(sha("x")),
        );
        append_provenance(conn, "prov-1", "notes", "note-1", &p, t(0)).unwrap();
        let listed = list_provenance(conn, "notes", "note-1").unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].provenance, p);
        assert_immutable(
            conn,
            "UPDATE source_provenance SET trust = 'UNKNOWN' WHERE id = 'prov-1'",
        );
        assert_immutable(conn, "DELETE FROM source_provenance WHERE id = 'prov-1'");
        assert!(matches!(
            append_provenance(
                conn,
                "prov-2",
                "notes",
                "note-1",
                &Provenance {
                    content_sha256: Some("short".into()),
                    ..p
                },
                t(0)
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));

        let first = ContentClassificationRow {
            id: "cc-1".into(),
            record_table: "notes".into(),
            record_id: "note-1".into(),
            data_domain: DataDomain::Learning,
            classification: Classification::Private,
            classified_by: operator_id(),
            classified_at: t(1),
            reason: None,
            created_at: t(1),
        };
        append_classification(conn, &first).unwrap();
        let second = ContentClassificationRow {
            id: "cc-2".into(),
            record_table: "notes".into(),
            record_id: "note-1".into(),
            data_domain: DataDomain::SecuritySensitive,
            classification: Classification::Secret,
            classified_by: operator_id(),
            classified_at: t(2),
            reason: Some("contains lab credentials".into()),
            created_at: t(2),
        };
        assert!(matches!(
            reclassify(
                conn,
                ClassifiedTable::Attachments,
                Revision::INITIAL,
                &second
            ),
            Err(RepoError::Storage(StorageError::InvalidInput(_)))
        ));
        let r2 = reclassify(conn, ClassifiedTable::Notes, Revision::INITIAL, &second).unwrap();
        assert_eq!(r2, Revision::new(2));
        assert_eq!(
            latest_classification(conn, "notes", "note-1")
                .unwrap()
                .unwrap(),
            second
        );
        assert_eq!(
            list_classifications(conn, "notes", "note-1").unwrap().len(),
            2
        );
        assert_immutable(
            conn,
            "UPDATE content_classifications SET classification = 'PUBLIC'",
        );
        assert_immutable(conn, "DELETE FROM content_classifications");
        let secure = ReadScope::domain(DataDomain::SecuritySensitive);
        let reclassified = get_note(conn, &secure, &row.id).unwrap().unwrap();
        assert_eq!(reclassified.classification, Classification::Secret);
        assert_eq!(reclassified.data_domain, DataDomain::SecuritySensitive);
        assert!(matches!(
            get_note(conn, &learning(), &row.id),
            Err(RepoError::CrossDomainRefused { .. })
        ));
    }
}
