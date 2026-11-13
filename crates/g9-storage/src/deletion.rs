//! Erasure workflow for personal content (gate A10; ARCHITECTURE.md §6
//! "Deletion semantics"; `docs/privacy/data-lifecycle.md` § Deletion model).
//!
//! 1. [`create_request`] records a `deletion_requests` row for one subject.
//!    Policy (`Capability::DataDelete`) and the approval are decided in
//!    `g9-policy`/`g9-app`; this module only persists the `approval_id` link.
//! 2. [`execute_erasure`] runs in one `BEGIN IMMEDIATE` transaction: content
//!    columns of the subject (and its personal children) are redacted in place,
//!    one `deletion_tombstones` row per redacted row keeps the pre-erasure
//!    content hash, derived rows that referenced the content receive an
//!    invalidation marker, and the request moves to `EXECUTED` with a report.
//!    Rows are never deleted: evidentiary references stay resolvable to a
//!    tombstone.
//! 3. After commit the WAL is checkpointed with TRUNCATE so that redacted
//!    content does not linger in the log (SEC-09; `secure_delete` handles free pages).
//!
//! Subjects implemented: `NOTE` (note + versions + attachments of the note),
//! `ATTACHMENT`, `OPT_IN_REFLECTION`. `STUDY_SESSION` erasure is PROPOSED and
//! returns `NotImplemented` (the timer ledger is evidentiary; which personal
//! fields of a session are erasable is an open product decision).

use crate::connection::checkpoint_truncate;
use crate::error::{StorageError, StorageResult};
use crate::sql::{enum_from_text, enum_to_text, json_from_text, opt_millis, parse_id};
use g9_domain::{ActorId, ApprovalId, Revision, UtcMillis};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use serde::{Deserialize, Serialize};

/// Redaction placeholder written into text columns.
pub const REDACTED: &str = "[REDACTED]";

/// What a deletion request targets (mirrors the `deletion_requests.subject_kind` CHECK).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DeletionSubjectKind {
    /// A `notes` row (plus its versions and attachments).
    Note,
    /// An `attachments` row.
    Attachment,
    /// An `opt_in_reflections` row.
    OptInReflection,
    /// A `study_sessions` row (erasure PROPOSED, not implemented).
    StudySession,
}

/// Request lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DeletionState {
    /// Created, awaiting execution or rejection.
    Requested,
    /// Erasure completed (terminal, immutable).
    Executed,
    /// Refused by the operator (terminal, immutable).
    Rejected,
}

/// Input for [`create_request`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewDeletionRequest {
    /// Request id.
    pub id: String,
    /// Subject kind.
    pub subject_kind: DeletionSubjectKind,
    /// Subject row id.
    pub subject_id: String,
    /// Requesting actor (the operator).
    pub requested_by: ActorId,
    /// When requested.
    pub requested_at: UtcMillis,
    /// Optional reason (free text; stored on the request, which is SYSTEM domain).
    pub reason: Option<String>,
    /// Approval bound to the request, when already granted.
    pub approval_id: Option<ApprovalId>,
}

/// A `deletion_requests` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeletionRequestRow {
    /// Request id.
    pub id: String,
    /// Subject kind.
    pub subject_kind: DeletionSubjectKind,
    /// Subject row id.
    pub subject_id: String,
    /// Requesting actor.
    pub requested_by: ActorId,
    /// When requested.
    pub requested_at: UtcMillis,
    /// Optional reason.
    pub reason: Option<String>,
    /// Linked approval token, if any.
    pub approval_id: Option<ApprovalId>,
    /// Lifecycle state.
    pub state: DeletionState,
    /// Actor that executed or rejected the request.
    pub executed_by: Option<ActorId>,
    /// When executed or rejected.
    pub executed_at: Option<UtcMillis>,
    /// Erasure report once executed.
    pub report: Option<ErasureReport>,
    /// Optimistic concurrency revision.
    pub revision: Revision,
}

/// One tombstone written during erasure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tombstone {
    /// Tombstone id (`<request>:<table>:<n>`).
    pub id: String,
    /// Redacted table.
    pub table_name: String,
    /// Redacted row id.
    pub row_id: String,
    /// Content hash before redaction.
    pub content_sha256_before: String,
    /// When redacted.
    pub erased_at: UtcMillis,
}

/// A `(table, row id)` pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowRef {
    /// Table name.
    pub table_name: String,
    /// Row id.
    pub row_id: String,
}

/// What an erasure did; stored as `deletion_requests.report_json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErasureReport {
    /// The executed request.
    pub request_id: String,
    /// Rows whose content columns were redacted.
    pub redacted_rows: Vec<RowRef>,
    /// Tombstones written (one per redacted row).
    pub tombstones: Vec<Tombstone>,
    /// Derived rows that received an invalidation marker.
    pub invalidated_derived: Vec<RowRef>,
    /// Content-addressed attachment paths whose bytes the application must remove
    /// from the approved attachment root (this crate performs no filesystem deletion).
    /// A path still referenced by another non-erased attachment (identical bytes)
    /// is never listed.
    pub attachment_storage_refs_to_purge: Vec<String>,
}

fn subject_exists(conn: &Connection, kind: DeletionSubjectKind, id: &str) -> StorageResult<bool> {
    let (sql, erasable) = match kind {
        DeletionSubjectKind::Note => ("SELECT erased FROM notes WHERE id = ?1", true),
        DeletionSubjectKind::Attachment => ("SELECT erased FROM attachments WHERE id = ?1", true),
        DeletionSubjectKind::OptInReflection => {
            ("SELECT erased FROM opt_in_reflections WHERE id = ?1", true)
        }
        DeletionSubjectKind::StudySession => ("SELECT 0 FROM study_sessions WHERE id = ?1", false),
    };
    let erased: Option<i64> = conn.query_row(sql, params![id], |r| r.get(0)).optional()?;
    match erased {
        None => Ok(false),
        Some(1) if erasable => Err(StorageError::Conflict(
            "subject is already erased".to_string(),
        )),
        Some(_) => Ok(true),
    }
}

/// Record a deletion request (state `REQUESTED`). The subject must exist and not be erased.
pub fn create_request(
    conn: &Connection,
    req: &NewDeletionRequest,
) -> StorageResult<DeletionRequestRow> {
    if !subject_exists(conn, req.subject_kind, &req.subject_id)? {
        return Err(StorageError::NotFound("deletion subject"));
    }
    conn.execute(
        "INSERT INTO deletion_requests (id, subject_kind, subject_id, requested_by, requested_at, reason, approval_id, state,
            revision, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'REQUESTED', 1, ?5, ?5)",
        params![
            req.id,
            enum_to_text(&req.subject_kind)?,
            req.subject_id,
            req.requested_by.as_str(),
            req.requested_at.as_millis(),
            req.reason,
            req.approval_id.as_ref().map(|a| a.as_str().to_string()),
        ],
    )?;
    get_request(conn, &req.id)?.ok_or(StorageError::NotFound("deletion request"))
}

/// Read a request.
pub fn get_request(conn: &Connection, id: &str) -> StorageResult<Option<DeletionRequestRow>> {
    let row = conn
        .query_row(
            "SELECT id, subject_kind, subject_id, requested_by, requested_at, reason, approval_id, state, executed_by,
                    executed_at, report_json, revision
             FROM deletion_requests WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, Option<String>>(5)?,
                    r.get::<_, Option<String>>(6)?,
                    r.get::<_, String>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, Option<i64>>(9)?,
                    r.get::<_, Option<String>>(10)?,
                    r.get::<_, i64>(11)?,
                ))
            },
        )
        .optional()?;
    let Some((
        id,
        subject_kind,
        subject_id,
        requested_by,
        requested_at,
        reason,
        approval_id,
        state,
        executed_by,
        executed_at,
        report_json,
        revision,
    )) = row
    else {
        return Ok(None);
    };
    let report = match report_json {
        Some(j) => Some(
            serde_json::from_value(json_from_text(&j)?)
                .map_err(|e| StorageError::RowDecode(format!("report_json: {e}")))?,
        ),
        None => None,
    };
    Ok(Some(DeletionRequestRow {
        id,
        subject_kind: enum_from_text(&subject_kind)?,
        subject_id,
        requested_by: parse_id(&requested_by)?,
        requested_at: UtcMillis::from_millis(requested_at),
        reason,
        approval_id: approval_id.as_deref().map(parse_id).transpose()?,
        state: enum_from_text(&state)?,
        executed_by: executed_by.as_deref().map(parse_id).transpose()?,
        executed_at: opt_millis(executed_at),
        report,
        revision: Revision::new(revision.max(1) as u64),
    }))
}

/// Reject a pending request (terminal; the row becomes immutable).
pub fn reject_request(
    conn: &Connection,
    id: &str,
    by: &ActorId,
    now: UtcMillis,
) -> StorageResult<DeletionRequestRow> {
    let changed = conn.execute(
        "UPDATE deletion_requests SET state = 'REJECTED', executed_by = ?2, executed_at = ?3, revision = revision + 1,
            updated_at = ?3
         WHERE id = ?1 AND state = 'REQUESTED'",
        params![id, by.as_str(), now.as_millis()],
    )?;
    if changed == 0 {
        return Err(StorageError::Precondition(
            "deletion request is not in REQUESTED state".to_string(),
        ));
    }
    get_request(conn, id)?.ok_or(StorageError::NotFound("deletion request"))
}

struct Eraser<'a> {
    tx: &'a Transaction<'a>,
    request_id: &'a str,
    now: UtcMillis,
    report: ErasureReport,
    tombstone_seq: u32,
}

impl Eraser<'_> {
    fn tombstone(&mut self, table: &str, row_id: &str, sha_before: &str) -> StorageResult<()> {
        self.tombstone_seq += 1;
        let id = format!("{}:{}:{}", self.request_id, table, self.tombstone_seq);
        self.tx.execute(
            "INSERT INTO deletion_tombstones (id, deletion_request_id, table_name, row_id, content_sha256_before, erased_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, self.request_id, table, row_id, sha_before, self.now.as_millis()],
        )?;
        self.report.tombstones.push(Tombstone {
            id,
            table_name: table.to_string(),
            row_id: row_id.to_string(),
            content_sha256_before: sha_before.to_string(),
            erased_at: self.now,
        });
        self.report.redacted_rows.push(RowRef {
            table_name: table.to_string(),
            row_id: row_id.to_string(),
        });
        Ok(())
    }

    fn invalidate_evidence(&mut self, column: &str, id: &str) -> StorageResult<()> {
        // `column` is one of our own column names, never input.
        let sql = format!(
            "SELECT id FROM evidence_items WHERE {column} = ?1 AND invalidated_at IS NULL ORDER BY id"
        );
        let ids: Vec<String> = {
            let mut stmt = self.tx.prepare(&sql)?;
            let rows = stmt.query_map(params![id], |r| r.get::<_, String>(0))?;
            let mut v = Vec::new();
            for row in rows {
                v.push(row?);
            }
            v
        };
        let reason = format!("deletion_request:{}", self.request_id);
        for eid in ids {
            // The summary repeats erased content (a note title, a file name):
            // it is redacted with the item, never left readable.
            self.tx.execute(
                "UPDATE evidence_items SET invalidated_at = ?2, invalidation_reason = ?3, summary = ?4,
                     revision = revision + 1, updated_at = ?2
                 WHERE id = ?1",
                params![eid, self.now.as_millis(), reason, REDACTED],
            )?;
            self.report.invalidated_derived.push(RowRef {
                table_name: "evidence_items".to_string(),
                row_id: eid.clone(),
            });
            self.invalidate_hypotheses_referencing(&eid)?;
        }
        Ok(())
    }

    fn invalidate_hypotheses_referencing(&mut self, reference: &str) -> StorageResult<()> {
        let ids: Vec<String> = {
            let mut stmt = self.tx.prepare(
                "SELECT id FROM learner_hypotheses
                 WHERE invalidated_at IS NULL
                   AND (EXISTS (SELECT 1 FROM json_each(supporting_refs) WHERE value = ?1)
                     OR EXISTS (SELECT 1 FROM json_each(contradicting_refs) WHERE value = ?1))
                 ORDER BY id",
            )?;
            let rows = stmt.query_map(params![reference], |r| r.get::<_, String>(0))?;
            let mut v = Vec::new();
            for row in rows {
                v.push(row?);
            }
            v
        };
        let reason = format!("deletion_request:{}", self.request_id);
        for hid in ids {
            self.tx.execute(
                "UPDATE learner_hypotheses SET invalidated_at = ?2, invalidation_reason = ?3, revision = revision + 1,
                    updated_at = ?2
                 WHERE id = ?1",
                params![hid, self.now.as_millis(), reason],
            )?;
            self.report.invalidated_derived.push(RowRef {
                table_name: "learner_hypotheses".to_string(),
                row_id: hid,
            });
        }
        Ok(())
    }

    fn erase_attachment(&mut self, attachment_id: &str) -> StorageResult<()> {
        let row: Option<(String, String, i64)> = self
            .tx
            .query_row(
                "SELECT content_sha256, storage_ref, erased FROM attachments WHERE id = ?1",
                params![attachment_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((sha, storage_ref, erased)) = row else {
            return Err(StorageError::NotFound("attachment"));
        };
        if erased == 1 {
            return Ok(());
        }
        self.tx.execute(
            "UPDATE attachments SET original_filename = ?2, storage_ref = '', erased = 1, erased_at = ?3,
                erasure_request_id = ?4, revision = revision + 1, updated_at = ?3
             WHERE id = ?1",
            params![attachment_id, REDACTED, self.now.as_millis(), self.request_id],
        )?;
        self.tombstone("attachments", attachment_id, &sha)?;
        // Paths are content-addressed, so identical bytes share one path: the path
        // is purged only when no other non-erased attachment still uses it (the
        // last erasure of those bytes lists it).
        if !storage_ref.is_empty() {
            let shared: bool = self.tx.query_row(
                "SELECT EXISTS (SELECT 1 FROM attachments WHERE storage_ref = ?1 AND erased = 0 AND id <> ?2)",
                params![storage_ref, attachment_id],
                |r| r.get(0),
            )?;
            if !shared {
                self.report
                    .attachment_storage_refs_to_purge
                    .push(storage_ref);
            }
        }
        self.invalidate_evidence("attachment_id", attachment_id)?;
        self.invalidate_hypotheses_referencing(attachment_id)
    }

    fn erase_note(&mut self, note_id: &str) -> StorageResult<()> {
        let row: Option<(String, i64)> = self
            .tx
            .query_row(
                "SELECT body_sha256, erased FROM notes WHERE id = ?1",
                params![note_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((sha, erased)) = row else {
            return Err(StorageError::NotFound("note"));
        };
        if erased == 1 {
            return Ok(());
        }
        self.tx.execute(
            "UPDATE notes SET title = ?2, body_markdown = '', erased = 1, erased_at = ?3, erasure_request_id = ?4,
                revision = revision + 1, updated_at = ?3
             WHERE id = ?1",
            params![note_id, REDACTED, self.now.as_millis(), self.request_id],
        )?;
        self.tombstone("notes", note_id, &sha)?;

        let versions: Vec<(String, String)> = {
            let mut stmt = self
                .tx
                .prepare("SELECT id, body_sha256 FROM note_versions WHERE note_id = ?1 AND erased = 0 ORDER BY version_no")?;
            let rows = stmt.query_map(params![note_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?;
            let mut v = Vec::new();
            for row in rows {
                v.push(row?);
            }
            v
        };
        for (vid, vsha) in versions {
            self.tx.execute(
                "UPDATE note_versions SET title = ?2, body_markdown = '', erased = 1 WHERE id = ?1",
                params![vid, REDACTED],
            )?;
            self.tombstone("note_versions", &vid, &vsha)?;
        }

        let attachments: Vec<String> = {
            let mut stmt = self.tx.prepare(
                "SELECT id FROM attachments WHERE note_id = ?1 AND erased = 0 ORDER BY id",
            )?;
            let rows = stmt.query_map(params![note_id], |r| r.get::<_, String>(0))?;
            let mut v = Vec::new();
            for row in rows {
                v.push(row?);
            }
            v
        };
        for aid in attachments {
            self.erase_attachment(&aid)?;
        }
        self.invalidate_evidence("note_id", note_id)?;
        self.invalidate_hypotheses_referencing(note_id)
    }

    fn erase_reflection(&mut self, id: &str) -> StorageResult<()> {
        let row: Option<(String, i64)> = self
            .tx
            .query_row(
                "SELECT body_sha256, erased FROM opt_in_reflections WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let Some((sha, erased)) = row else {
            return Err(StorageError::NotFound("opt-in reflection"));
        };
        if erased == 1 {
            return Ok(());
        }
        self.tx.execute(
            "UPDATE opt_in_reflections SET body = '', erased = 1, erased_at = ?2, erasure_request_id = ?3,
                revision = revision + 1, updated_at = ?2
             WHERE id = ?1",
            params![id, self.now.as_millis(), self.request_id],
        )?;
        self.tombstone("opt_in_reflections", id, &sha)
    }
}

/// Execute an approved deletion request in one transaction and checkpoint the WAL afterwards.
pub fn execute_erasure(
    conn: &mut Connection,
    request_id: &str,
    executed_by: &ActorId,
    now: UtcMillis,
) -> StorageResult<ErasureReport> {
    let request =
        get_request(conn, request_id)?.ok_or(StorageError::NotFound("deletion request"))?;
    if request.state != DeletionState::Requested {
        return Err(StorageError::Precondition(
            "deletion request is not in REQUESTED state".to_string(),
        ));
    }
    if request.subject_kind == DeletionSubjectKind::StudySession {
        return Err(StorageError::NotImplemented("deletion.study_session"));
    }
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let report = {
        let mut eraser = Eraser {
            tx: &tx,
            request_id,
            now,
            report: ErasureReport {
                request_id: request_id.to_string(),
                redacted_rows: Vec::new(),
                tombstones: Vec::new(),
                invalidated_derived: Vec::new(),
                attachment_storage_refs_to_purge: Vec::new(),
            },
            tombstone_seq: 0,
        };
        match request.subject_kind {
            DeletionSubjectKind::Note => eraser.erase_note(&request.subject_id)?,
            DeletionSubjectKind::Attachment => eraser.erase_attachment(&request.subject_id)?,
            DeletionSubjectKind::OptInReflection => eraser.erase_reflection(&request.subject_id)?,
            DeletionSubjectKind::StudySession => {
                return Err(StorageError::NotImplemented("deletion.study_session"))
            }
        }
        eraser.report
    };
    let report_json = serde_json::to_string(&report)
        .map_err(|_| StorageError::Internal("erasure report json"))?;
    let changed = tx.execute(
        "UPDATE deletion_requests SET state = 'EXECUTED', executed_by = ?2, executed_at = ?3, report_json = ?4,
            revision = revision + 1, updated_at = ?3
         WHERE id = ?1 AND state = 'REQUESTED' AND revision = ?5",
        params![
            request_id,
            executed_by.as_str(),
            now.as_millis(),
            report_json,
            request.revision.get() as i64
        ],
    )?;
    if changed != 1 {
        return Err(StorageError::StaleRevision {
            expected: Some(request.revision),
            actual: request.revision.next(),
        });
    }
    tx.commit()?;
    // Redacted content must not linger in the WAL (SEC-09). Ignored for non-WAL/in-memory databases.
    let _ = checkpoint_truncate(conn);
    Ok(report)
}
