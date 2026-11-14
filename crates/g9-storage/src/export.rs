//! Data export for `export.request` (gate A10; privacy model "export").
//!
//! [`collect`] reads the rows a selection covers and [`write`] writes them to
//! `<exports>/<export_id>/`:
//!
//! | File | Content |
//! |---|---|
//! | `data.json` (format JSON) | every exported table: column names and rows |
//! | `<table>.csv` (format CSV) | one RFC 4180 file per table, header row first |
//! | `attachments/ab/cd/<sha256>` | the bytes of every exported attachment, hash-checked |
//! | `manifest.json` (written last) | selection, row counts, every file with its SHA-256 and size |
//!
//! # Selection
//!
//! Every table of the schema is assigned exactly once in [`TABLES`] (a test
//! fails when a migration adds a table that is not): a table with a
//! `data_domain` column is filtered by domain, never exports a `SECRET` row
//! and, without `include_private`, keeps only `PUBLIC` and `INTERNAL` rows; a child table follows its owning
//! row (a session's intervals, a note's versions, an attempt's grades); a
//! table without row-level domain belongs to one fixed domain (the governance
//! tables to `SYSTEM`); `idempotency_records` (stored command responses kept
//! for replay) is never exported. Erased rows are left out. A row that also
//! references an attempt, episode, intervention or chunk ([`LINKS`]) leaves
//! only when that row does, and assessment task statements are withheld
//! ([`WITHHELD`], listed in the manifest).
//!
//! # Writing
//!
//! The export is assembled in a hidden `.<export_id>.partial` directory, every
//! file opened `create_new`, mode 0600, and flushed; the directory is renamed
//! into place and its parent flushed, so a power cut leaves either no export
//! or a complete one. An existing export is never overwritten. CSV text cells
//! that start with `=`, `+`, `-`, `@`, tab or carriage return are prefixed
//! with `'` so a spreadsheet does not evaluate imported content as a formula
//! (recorded in the manifest).

use crate::attachment_store::{self, CopyReport};
use crate::backup::sync_dir;
use crate::error::{StorageError, StorageResult};
use g9_domain::{DataDomain, UtcMillis};
use rusqlite::types::ValueRef;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Which rows an export covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportSelection {
    /// Privacy domains to export.
    pub domains: BTreeSet<DataDomain>,
    /// Include rows classified `PRIVATE`. `SECRET` rows are never exported.
    pub include_private: bool,
}

/// File format of the table data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExportFormat {
    /// One `data.json`.
    Json,
    /// One `<table>.csv` per table.
    Csv,
}

/// How a table is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// Own `data_domain` and `classification` columns (and `erased` when set).
    Own { erased: bool },
    /// Rows whose `column` references an exported row of `parent` (by `key`).
    Child {
        column: &'static str,
        parent: &'static str,
        key: &'static str,
        erased: bool,
    },
    /// No row-level domain: the whole table belongs to one domain.
    Fixed(DataDomain),
    /// Never exported.
    Excluded(&'static str),
}

const fn own() -> Scope {
    Scope::Own { erased: false }
}

const fn child(column: &'static str, parent: &'static str) -> Scope {
    Scope::Child {
        column,
        parent,
        key: "id",
        erased: false,
    }
}

const SYSTEM: Scope = Scope::Fixed(DataDomain::System);
const LEARNING: Scope = Scope::Fixed(DataDomain::Learning);

/// Every table of the schema, in export order (parents before children).
const TABLES: &[(&str, Scope)] = &[
    // Migration ledger (written by the migration runner).
    ("schema_migrations", SYSTEM),
    // Governance (0001).
    ("app_settings", own()),
    ("audit_metadata", SYSTEM),
    ("approval_requests", SYSTEM),
    ("approval_decisions", SYSTEM),
    ("policy_decisions", SYSTEM),
    ("data_retention_rules", own()),
    ("deletion_requests", SYSTEM),
    ("deletion_tombstones", SYSTEM),
    ("backup_manifests", SYSTEM),
    (
        "idempotency_records",
        Scope::Excluded("stored command responses kept for replay, not operator data"),
    ),
    ("domain_events", own()),
    // Curriculum (0002).
    ("curricula", own()),
    ("curriculum_versions", child("curriculum_id", "curricula")),
    ("program_plan", child("curriculum_id", "curricula")),
    (
        "weeks",
        child("curriculum_version_id", "curriculum_versions"),
    ),
    (
        "training_days",
        child("curriculum_version_id", "curriculum_versions"),
    ),
    (
        "daily_objectives",
        child("curriculum_version_id", "curriculum_versions"),
    ),
    ("study_blocks", child("training_day_id", "training_days")),
    ("resource_references", own()),
    (
        "deliverable_definitions",
        child("training_day_id", "training_days"),
    ),
    // Competency (0003).
    ("competencies", own()),
    (
        "competency_versions",
        child("competency_id", "competencies"),
    ),
    (
        "competency_edges",
        child("from_competency_id", "competencies"),
    ),
    (
        "objective_competencies",
        child("competency_id", "competencies"),
    ),
    ("learner_hypotheses", own()),
    (
        "competency_state_snapshots",
        child("competency_id", "competencies"),
    ),
    // Calendar and sessions (0004).
    ("calendar_events", own()),
    (
        "recurrence_rules",
        child("calendar_event_id", "calendar_events"),
    ),
    ("schedule_proposals", LEARNING),
    (
        "schedule_decisions",
        child("proposal_id", "schedule_proposals"),
    ),
    ("study_sessions", own()),
    ("session_events", child("session_id", "study_sessions")),
    ("time_intervals", child("session_id", "study_sessions")),
    (
        "manual_time_corrections",
        child("session_id", "study_sessions"),
    ),
    // Evidence (0005).
    ("notes", Scope::Own { erased: true }),
    (
        "note_versions",
        Scope::Child {
            column: "note_id",
            parent: "notes",
            key: "id",
            erased: true,
        },
    ),
    ("attachments", Scope::Own { erased: true }),
    ("evidence_items", own()),
    (
        "artifact_links",
        child("evidence_item_id", "evidence_items"),
    ),
    ("source_provenance", SYSTEM),
    ("content_classifications", own()),
    // Assessment (0006).
    ("assessment_definitions", own()),
    (
        "assessment_versions",
        child("definition_id", "assessment_definitions"),
    ),
    (
        "rubrics",
        child("assessment_version_id", "assessment_versions"),
    ),
    ("rubric_criteria", child("rubric_id", "rubrics")),
    ("attempts", own()),
    ("attempt_evidence", child("attempt_id", "attempts")),
    ("criterion_results", child("attempt_id", "attempts")),
    ("grade_decisions", child("attempt_id", "attempts")),
    ("retake_links", child("retake_attempt_id", "attempts")),
    (
        "exposure_log",
        child("assessment_version_id", "assessment_versions"),
    ),
    // Retention and learning model (0007).
    ("review_items", own()),
    ("review_attempts", child("review_item_id", "review_items")),
    (
        "practical_transfer_checks",
        child("competency_id", "competencies"),
    ),
    ("learning_episodes", own()),
    (
        "competency_observations",
        child("competency_id", "competencies"),
    ),
    ("interventions", own()),
    (
        "intervention_outcomes",
        child("intervention_id", "interventions"),
    ),
    ("method_experiments", LEARNING),
    ("calibration_observations", LEARNING),
    // Knowledge and AI (0008).
    ("source_registry", own()),
    ("knowledge_documents", own()),
    (
        "knowledge_chunks",
        child("document_id", "knowledge_documents"),
    ),
    ("knowledge_claims", own()),
    ("claim_versions", child("claim_id", "knowledge_claims")),
    ("model_registry", own()),
    (
        "model_evals",
        Scope::Child {
            column: "model_id",
            parent: "model_registry",
            key: "model_id",
            erased: false,
        },
    ),
    ("skill_registry", SYSTEM),
    ("skill_versions", SYSTEM),
    ("skill_invocations", SYSTEM),
    ("skill_evals", SYSTEM),
    // Business (0009).
    ("goals", own()),
    ("projects", own()),
    ("milestones", own()),
    (
        "milestone_dependencies",
        child("from_milestone_id", "milestones"),
    ),
    ("commitments", own()),
    ("execution_observations", own()),
    ("decisions", own()),
    ("decision_outcomes", child("decision_id", "decisions")),
    ("opt_in_reflections", Scope::Own { erased: true }),
    // Grade observations (0013).
    (
        "competency_observation_evidence",
        child("observation_id", "competency_observations"),
    ),
];

/// Nullable references that must point at an exported row or be NULL, on top
/// of the table's scope: a row tied to a PRIVATE attempt, an excluded
/// intervention or document chunk leaves only together with that row
/// (`(table, column, parent)`, the parent keyed by `id`).
const LINKS: &[(&str, &str, &str)] = &[
    ("competency_observations", "attempt_id", "attempts"),
    ("competency_observations", "episode_id", "learning_episodes"),
    (
        "competency_observation_evidence",
        "decision_id",
        "grade_decisions",
    ),
    ("practical_transfer_checks", "attempt_id", "attempts"),
    ("exposure_log", "attempt_id", "attempts"),
    ("method_experiments", "intervention_id", "interventions"),
    ("calibration_observations", "attempt_id", "attempts"),
    (
        "calibration_observations",
        "review_attempt_id",
        "review_attempts",
    ),
    ("claim_versions", "chunk_id", "knowledge_chunks"),
];

/// Columns never exported (`(table, column, reason)`; listed in the manifest).
const WITHHELD: &[(&str, &str, &str)] = &[
    (
        "assessment_versions",
        "task_statement",
        "blind assessment integrity: an exported task is an exposure the exposure log never records",
    ),
    // ADR-0017: a digest of content is content. A record of an erasure, and the
    // immutable ledgers, leave without anything derived from what a row held.
    (
        "deletion_tombstones",
        "content_sha256_before",
        "a tombstone identifies an erased row; rows written before ADR-0017 hold a digest of the erased content",
    ),
    (
        "deletion_requests",
        "report_json",
        "erasure reports written before ADR-0017 hold digests of the erased content",
    ),
    (
        "deletion_requests",
        "reason",
        "free text the operator wrote about content that was then erased",
    ),
    (
        "domain_events",
        "content_sha256",
        "events are immutable: a digest of a record's content there outlives the record",
    ),
    (
        "source_provenance",
        "content_sha256",
        "the provenance ledger is immutable: a digest of a record's content there outlives the record",
    ),
    // The hash of a whole request ({operation, payload}) is a guess-confirmation oracle: a short
    // secret in the payload is recovered in as many tries as it has values when the rest of the
    // payload is guessable. The ledgers keep these hashes (they are immutable and bind approvals);
    // an export carries none of them, nor the chain values a guess could be checked against.
    (
        "policy_decisions",
        "parameters_sha256",
        "the hash of a whole request confirms a guessed payload, secret content included",
    ),
    (
        "approval_requests",
        "parameters_sha256",
        "the hash of a whole request confirms a guessed payload, secret content included",
    ),
    (
        "approval_requests",
        "operation_json",
        "the descriptor repeats the parameters hash; its other fields are columns of the table",
    ),
    (
        "approval_decisions",
        "parameters_sha256",
        "the hash of a whole request confirms a guessed payload, secret content included",
    ),
    (
        "audit_metadata",
        "details_sha256",
        "the hash of the details holds the request hash",
    ),
    (
        "audit_metadata",
        "prev_hash",
        "chain values would confirm a guessed request hash; the chain is checked in the database",
    ),
    (
        "audit_metadata",
        "row_hash",
        "chain values would confirm a guessed request hash; the chain is checked in the database",
    ),
    (
        "approval_requests",
        "binding_mac",
        "the approval MAC is checked on this machine and never leaves it",
    ),
    (
        "approval_decisions",
        "binding_mac",
        "the approval MAC is checked on this machine and never leaves it",
    ),
];

const fn domain_sql(d: DataDomain) -> &'static str {
    match d {
        DataDomain::Learning => "'LEARNING'",
        DataDomain::Business => "'BUSINESS'",
        DataDomain::Personal => "'PERSONAL'",
        DataDomain::SecuritySensitive => "'SECURITY_SENSITIVE'",
        DataDomain::System => "'SYSTEM'",
    }
}

fn scope_of(table: &str) -> StorageResult<Scope> {
    TABLES
        .iter()
        .find(|(name, _)| *name == table)
        .map(|(_, scope)| *scope)
        .ok_or(StorageError::Internal("export.table_unassigned"))
}

/// The `WHERE` clause selecting the exported rows of `table`, or `None` when
/// the selection exports none of it. Built from static names only.
fn filter(table: &str, sel: &ExportSelection, depth: usize) -> StorageResult<Option<String>> {
    if depth > 8 {
        return Err(StorageError::Internal("export.parent_chain_too_deep"));
    }
    let erased_clause = |erased: bool| if erased { " AND erased = 0" } else { "" };
    let base = match scope_of(table)? {
        Scope::Excluded(_) => None,
        Scope::Fixed(domain) => sel.domains.contains(&domain).then(|| "1 = 1".to_string()),
        Scope::Own { erased } => {
            if sel.domains.is_empty() {
                return Ok(None);
            }
            let domains: Vec<&str> = sel.domains.iter().map(|d| domain_sql(*d)).collect();
            // SECRET never leaves (docs/privacy/data-lifecycle.md "Exports"):
            // PRIVATE only with `include_private`.
            let private = if sel.include_private {
                " AND classification IN ('PUBLIC', 'INTERNAL', 'PRIVATE')"
            } else {
                " AND classification IN ('PUBLIC', 'INTERNAL')"
            };
            Some(format!(
                "data_domain IN ({}){private}{}",
                domains.join(", "),
                erased_clause(erased)
            ))
        }
        Scope::Child {
            column,
            parent,
            key,
            erased,
        } => filter(parent, sel, depth + 1)?.map(|parent_filter| {
            format!(
                "{column} IN (SELECT {key} FROM {parent} WHERE {parent_filter}){}",
                erased_clause(erased)
            )
        }),
    };
    let Some(mut clause) = base else {
        return Ok(None);
    };
    for (_, column, parent) in LINKS.iter().filter(|(t, _, _)| *t == table) {
        match filter(parent, sel, depth + 1)? {
            Some(parent_filter) => clause.push_str(&format!(
                " AND ({column} IS NULL OR {column} IN (SELECT id FROM {parent} WHERE {parent_filter}))"
            )),
            None => clause.push_str(&format!(" AND {column} IS NULL")),
        }
    }
    Ok(Some(clause))
}

/// One exported table.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExportTable {
    /// Table name.
    pub name: String,
    /// Column names in table order.
    pub columns: Vec<String>,
    /// Rows; values as stored (integers, reals, text, `null`).
    pub rows: Vec<Vec<Value>>,
}

/// The rows of one export.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExportData {
    /// Exported tables in [`TABLES`] order (tables with no selected rows included, empty).
    pub tables: Vec<ExportTable>,
}

impl ExportData {
    /// Total number of rows.
    pub fn row_count(&self) -> u64 {
        self.tables.iter().map(|t| t.rows.len() as u64).sum()
    }

    /// Distinct content digests of the exported attachment rows.
    pub fn attachment_digests(&self) -> Vec<String> {
        let mut out = BTreeSet::new();
        for table in self.tables.iter().filter(|t| t.name == "attachments") {
            if let Some(col) = table.columns.iter().position(|c| c == "content_sha256") {
                for row in &table.rows {
                    if let Some(Value::String(sha)) = row.get(col) {
                        if attachment_store::is_sha256_hex(sha) {
                            out.insert(sha.clone());
                        }
                    }
                }
            }
        }
        out.into_iter().collect()
    }
}

fn to_json(value: ValueRef<'_>) -> StorageResult<Value> {
    Ok(match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => json!(i),
        ValueRef::Real(f) => serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number),
        ValueRef::Text(t) => Value::String(
            std::str::from_utf8(t)
                .map_err(|_| StorageError::RowDecode("text column is not UTF-8".to_string()))?
                .to_string(),
        ),
        ValueRef::Blob(b) => json!({ "hex": hex::encode(b) }),
    })
}

/// Read every row the selection covers, in one consistent read (call inside
/// the command transaction).
pub fn collect(conn: &Connection, sel: &ExportSelection) -> StorageResult<ExportData> {
    let mut tables = Vec::new();
    for (name, _) in TABLES {
        let Some(clause) = filter(name, sel, 0)? else {
            continue;
        };
        let mut stmt = conn.prepare(&format!(
            "SELECT * FROM {name} WHERE {clause} ORDER BY rowid"
        ))?;
        let columns: Vec<String> = stmt.column_names().map(str::to_string).collect();
        let width = columns.len();
        let withheld: Vec<usize> = columns
            .iter()
            .enumerate()
            .filter(|(_, c)| WITHHELD.iter().any(|(t, wc, _)| t == name && wc == *c))
            .map(|(i, _)| i)
            .collect();
        let mut rows = Vec::new();
        let mut cursor = stmt.query([])?;
        while let Some(row) = cursor.next()? {
            let mut values = Vec::with_capacity(width);
            for i in 0..width {
                values.push(if withheld.contains(&i) {
                    Value::Null
                } else {
                    to_json(row.get_ref(i)?)?
                });
            }
            rows.push(values);
        }
        tables.push(ExportTable {
            name: (*name).to_string(),
            columns,
            rows,
        });
    }
    Ok(ExportData { tables })
}

/// Who and what an export file records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportHeader {
    /// Export id; names the directory (`[A-Za-z0-9._-]`, at most 128 bytes, no leading dot).
    pub export_id: String,
    /// Instant of the export.
    pub created_at: UtcMillis,
    /// Application version.
    pub app_version: String,
    /// The selection.
    pub selection: ExportSelection,
    /// Data format.
    pub format: ExportFormat,
}

/// A written export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportWritten {
    /// `<exports>/<export_id>` (canonical).
    pub dir: PathBuf,
    /// SHA-256 of `manifest.json`.
    pub manifest_sha256: String,
    /// Number of files written, the manifest and attachments included.
    pub files: usize,
    /// Rows exported.
    pub rows: u64,
    /// Attachment copy result.
    pub attachments: CopyReport,
}

fn valid_export_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

fn new_dir(path: &Path) -> StorageResult<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(path)
        .map_err(|e| StorageError::io("create an export directory", &e))
}

/// Write `bytes` to a new file `dir/name` (create_new, 0600, flushed).
fn new_file(dir: &Path, name: &str, bytes: &[u8]) -> StorageResult<(String, u64)> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(dir.join(name))
        .map_err(|e| StorageError::io("create an export file", &e))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| StorageError::io("write an export file", &e))?;
    Ok((hex::encode(Sha256::digest(bytes)), bytes.len() as u64))
}

fn csv_cell(value: &Value, out: &mut String) {
    let text: Cow<'_, str> = match value {
        Value::Null => Cow::Borrowed(""),
        Value::String(s) if s.starts_with(['=', '+', '-', '@', '\t', '\r']) => {
            Cow::Owned(format!("'{s}"))
        }
        Value::String(s) => Cow::Borrowed(s.as_str()),
        other => Cow::Owned(other.to_string()),
    };
    if text.contains([',', '"', '\n', '\r']) {
        out.push('"');
        out.push_str(&text.replace('"', "\"\""));
        out.push('"');
    } else {
        out.push_str(&text);
    }
}

fn csv_table(table: &ExportTable) -> String {
    let mut out = String::new();
    let header: Vec<Value> = table.columns.iter().map(|c| json!(c)).collect();
    for row in std::iter::once(&header).chain(table.rows.iter()) {
        for (i, value) in row.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            csv_cell(value, &mut out);
        }
        out.push_str("\r\n");
    }
    out
}

fn domain_name(d: DataDomain) -> &'static str {
    domain_sql(d).trim_matches('\'')
}

/// Write `data` to `<exports_dir>/<export_id>/`, copying the bytes of every
/// exported attachment from `attachment_root` (hash-checked). Attachments that
/// are missing or damaged are listed in the manifest, not fatal.
pub fn write(
    exports_dir: &Path,
    header: &ExportHeader,
    data: &ExportData,
    attachment_root: Option<&Path>,
) -> StorageResult<ExportWritten> {
    if !valid_export_id(&header.export_id) {
        return Err(StorageError::InvalidInput(
            "export id must be [A-Za-z0-9._-], at most 128 bytes, no leading dot".to_string(),
        ));
    }
    let root = fs::canonicalize(exports_dir)
        .map_err(|e| StorageError::io("open the export directory", &e))?;
    let target = root.join(&header.export_id);
    if fs::symlink_metadata(&target).is_ok() {
        return Err(StorageError::Conflict(
            "an export with this id already exists".to_string(),
        ));
    }
    let partial = root.join(format!(".{}.partial", header.export_id));
    match fs::symlink_metadata(&partial) {
        Ok(meta) if meta.file_type().is_dir() => fs::remove_dir_all(&partial)
            .map_err(|e| StorageError::io("remove a stale partial export", &e))?,
        Ok(_) => {
            return Err(StorageError::Conflict(
                "a non-directory occupies the partial export path".to_string(),
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(StorageError::io("inspect a partial export", &e)),
    }
    new_dir(&partial)?;
    let assembled = assemble(&partial, header, data, attachment_root);
    let (manifest_sha256, files, attachments) = match assembled {
        Ok(done) => done,
        Err(e) => {
            let _ = fs::remove_dir_all(&partial);
            return Err(e);
        }
    };
    let placed = sync_dir(&partial)
        .and_then(|()| {
            fs::rename(&partial, &target)
                .map_err(|e| StorageError::io("place the export directory", &e))
        })
        .and_then(|()| sync_dir(&root));
    if let Err(e) = placed {
        let _ = fs::remove_dir_all(&partial);
        return Err(e);
    }
    Ok(ExportWritten {
        dir: target,
        manifest_sha256,
        files,
        rows: data.row_count(),
        attachments,
    })
}

fn assemble(
    dir: &Path,
    header: &ExportHeader,
    data: &ExportData,
    attachment_root: Option<&Path>,
) -> StorageResult<(String, usize, CopyReport)> {
    let mut files: Vec<Value> = Vec::new();
    match header.format {
        ExportFormat::Json => {
            let mut tables = Map::new();
            for t in &data.tables {
                tables.insert(
                    t.name.clone(),
                    json!({ "columns": t.columns, "rows": t.rows }),
                );
            }
            let bytes = serde_json::to_vec_pretty(&json!({ "tables": tables }))
                .map_err(|_| StorageError::Internal("export.json_encode"))?;
            let (sha, len) = new_file(dir, "data.json", &bytes)?;
            files.push(json!({ "path": "data.json", "sha256": sha, "bytes": len }));
        }
        ExportFormat::Csv => {
            for t in &data.tables {
                let name = format!("{}.csv", t.name);
                let (sha, len) = new_file(dir, &name, csv_table(t).as_bytes())?;
                files.push(json!({ "path": name, "sha256": sha, "bytes": len }));
            }
        }
    }
    let digests = data.attachment_digests();
    let attachments = if digests.is_empty() {
        CopyReport::default()
    } else {
        match attachment_root {
            Some(from) => {
                let to = dir.join("attachments");
                let report = attachment_store::copy_all(from, &to, &digests)?;
                for sha in &digests {
                    if report.missing.contains(sha) || report.corrupt.contains(sha) {
                        continue;
                    }
                    let rel = attachment_store::relpath(sha)?;
                    let len = fs::metadata(to.join(&rel))
                        .map_err(|e| StorageError::io("read an exported attachment", &e))?
                        .len();
                    files.push(json!({ "path": format!("attachments/{rel}"), "sha256": sha, "bytes": len }));
                }
                report
            }
            None => CopyReport {
                missing: digests,
                ..CopyReport::default()
            },
        }
    };
    let manifest = json!({
        "format": "g9-export",
        "version": 1,
        "export_id": header.export_id,
        "created_at": header.created_at.as_millis(),
        "app_version": header.app_version,
        "domains": header.selection.domains.iter().map(|d| domain_name(*d)).collect::<Vec<_>>(),
        "include_private": header.selection.include_private,
        "data_format": header.format,
        "csv_formula_guard": header.format == ExportFormat::Csv,
        "withheld": WITHHELD
            .iter()
            .filter(|(t, _, _)| data.tables.iter().any(|x| x.name == *t))
            .map(|(t, c, why)| json!({ "table": t, "column": c, "reason": why }))
            .collect::<Vec<_>>(),
        "tables": data.tables.iter().map(|t| json!({ "name": t.name, "rows": t.rows.len() })).collect::<Vec<_>>(),
        "files": files,
        "attachments": {
            "copied": attachments.written + attachments.already_present,
            "missing": attachments.missing,
            "damaged": attachments.corrupt,
        },
    });
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|_| StorageError::Internal("export.manifest_encode"))?;
    let (manifest_sha, _) = new_file(dir, "manifest.json", &bytes)?;
    let count = files.len() + 1;
    Ok((manifest_sha, count, attachments))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Database, OpenOptions};

    fn db() -> Database {
        Database::open_in_memory(OpenOptions::new("test").applied_at(UtcMillis::from_millis(1)))
            .unwrap()
    }

    fn all() -> ExportSelection {
        ExportSelection {
            domains: [
                DataDomain::Learning,
                DataDomain::Business,
                DataDomain::Personal,
                DataDomain::SecuritySensitive,
                DataDomain::System,
            ]
            .into_iter()
            .collect(),
            include_private: true,
        }
    }

    #[test]
    fn every_table_of_the_schema_is_assigned_exactly_once() {
        let db = db();
        let mut stmt = db
            .conn()
            .prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            )
            .unwrap();
        let schema: BTreeSet<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let assigned: Vec<&str> = TABLES.iter().map(|(n, _)| *n).collect();
        let unique: BTreeSet<String> = assigned.iter().map(|s| s.to_string()).collect();
        assert_eq!(unique.len(), assigned.len(), "a table is assigned twice");
        assert_eq!(unique, schema, "every table must be assigned");
        // Each child names a parent that comes earlier; so does every link.
        let position = |table: &str| TABLES.iter().position(|(n, _)| *n == table).unwrap();
        for (i, (name, scope)) in TABLES.iter().enumerate() {
            if let Scope::Child { parent, .. } = scope {
                assert!(
                    position(parent) < i,
                    "{name}: parent {parent} must come first"
                );
            }
        }
        for (table, column, parent) in LINKS {
            assert!(
                position(parent) < position(table),
                "{table}.{column}: {parent} first"
            );
        }
        for (table, _, _) in WITHHELD {
            position(table);
        }
    }

    #[test]
    fn every_filter_is_valid_sql_on_the_migrated_schema() {
        let db = db();
        let data = collect(db.conn(), &all()).unwrap();
        assert_eq!(
            data.tables.len(),
            TABLES.len() - 1,
            "all but the excluded table"
        );
        let none = collect(
            db.conn(),
            &ExportSelection {
                domains: BTreeSet::new(),
                include_private: false,
            },
        )
        .unwrap();
        assert!(none.tables.is_empty());
    }

    #[test]
    fn csv_cells_are_quoted_and_formulas_are_neutralized() {
        let mut out = String::new();
        csv_cell(&json!("a,\"b\"\nc"), &mut out);
        assert_eq!(out, "\"a,\"\"b\"\"\nc\"");
        let mut out = String::new();
        csv_cell(&json!("=HYPERLINK(\"x\")"), &mut out);
        assert_eq!(out, "\"'=HYPERLINK(\"\"x\"\")\"");
        let mut out = String::new();
        csv_cell(&json!(-5), &mut out);
        assert_eq!(out, "-5", "numbers are not text and are left alone");
        let mut out = String::new();
        csv_cell(&Value::Null, &mut out);
        assert_eq!(out, "");
    }

    #[test]
    fn an_export_is_written_once_and_never_overwritten() {
        let db = db();
        let dir = tempfile::tempdir().unwrap();
        let header = ExportHeader {
            export_id: "export-1".to_string(),
            created_at: UtcMillis::from_millis(5),
            app_version: "test".to_string(),
            selection: all(),
            format: ExportFormat::Csv,
        };
        let data = collect(db.conn(), &all()).unwrap();
        let written = write(dir.path(), &header, &data, None).unwrap();
        assert!(written.dir.join("manifest.json").is_file());
        assert!(written.dir.join("notes.csv").is_file());
        assert!(!dir.path().join(".export-1.partial").exists());
        assert!(matches!(
            write(dir.path(), &header, &data, None),
            Err(StorageError::Conflict(_))
        ));
        let bad = ExportHeader {
            export_id: "../x".to_string(),
            ..header
        };
        assert!(matches!(
            write(dir.path(), &bad, &data, None),
            Err(StorageError::InvalidInput(_))
        ));
    }
}
