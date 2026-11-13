//! Notes (M05). Note revisions are append-only (`note_versions`); the body is
//! private content and never enters an error, event or audit row (only its
//! SHA-256 does). Imported/model text in a note is data, never instructions.

use super::{creation_id, decode, new_id, parse_id, rc, scope_of, to_value};
use crate::commands::curriculum::active_version;
use crate::context::AppContext;
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation};
use g9_domain::hashing::sha256_hex;
use g9_domain::{
    Classification, CommandMeta, CommandOutcome, DataDomain, G9Error, NoteId, ObjectiveId,
    ResourceKind, ResourceRef, Revision, SourceKind, TrustClass,
};
use g9_evidence::MAX_NOTE_BYTES;
use g9_storage::repos::curriculum as curriculum_repo;
use g9_storage::repos::evidence::{self as repo, NoteKind, NoteRow};
use g9_storage::repos::ReadScope;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{json, Value};

fn check_body(body: &str) -> Result<(), G9Error> {
    if u64::try_from(body.len()).unwrap_or(u64::MAX) > MAX_NOTE_BYTES {
        return Err(G9Error::invalid_input(
            "note.too_large",
            "note body exceeds the maximum size",
        ));
    }
    Ok(())
}

fn view(
    conn: &Connection,
    row: &NoteRow,
    objective_code: Option<String>,
) -> Result<Value, G9Error> {
    let versions =
        repo::list_note_versions(conn, &ReadScope::cross_domain(row.data_domain), &row.id)?;
    to_value(&json!({
        "note_id": row.id,
        "title": row.title,
        "body_markdown": row.body_markdown,
        "content_sha256": sha256_hex(row.body_markdown.as_bytes()),
        "classification": row.classification,
        "domain": row.data_domain,
        "objective_id": objective_code,
        // Note-level evidence links are not persisted in wave 1 (see crate docs, Gaps).
        "evidence_refs": [],
        "version_count": versions.len(),
        "created_at": row.created_at,
        "updated_at": row.updated_at,
        "revision": row.revision,
    }))
}

/// Content-free idempotency record of a note result (A10, ADR-0002): the note
/// title and body are personal content and are never kept in
/// `idempotency_records`; a replay rebuilds the view from the stored note, so
/// an erased note replays erased.
fn replay_record(id: &NoteId) -> Value {
    json!({ "note_id": id })
}

/// Rebuild the note view for an idempotent replay (see [`replay_record`]).
pub(crate) fn replay_view(
    conn: &Connection,
    _svc: &crate::context::Services,
    record: &Value,
) -> Result<(Value, Vec<String>), G9Error> {
    let id: NoteId = record
        .get("note_id")
        .and_then(Value::as_str)
        .ok_or_else(|| G9Error::internal_redacted("idempotency.replay_record"))
        .and_then(|s| parse_id(s, "note_id"))?;
    let row = repo::get_note(conn, &ReadScope::cross_domain(DataDomain::Learning), &id)?
        .ok_or_else(|| G9Error::not_found("note.not_found", "note not found"))?;
    let code = objective_code(row.objective_id.as_deref());
    Ok((
        view(conn, &row, code)?,
        vec![
            "idempotency.replay_rebuilt: note content is not retained in idempotency records; the result shows the note as currently stored".to_string(),
        ],
    ))
}

/// UI objective code of a stored `daily_objectives.id` (`<version>:<code>`).
fn objective_code(objective_row: Option<&str>) -> Option<String> {
    objective_row.map(|row_id| super::tail(row_id).to_string())
}

#[derive(Debug, Deserialize)]
struct CreateRequest {
    title: String,
    body_markdown: String,
    classification: Classification,
    domain: DataDomain,
    objective_id: Option<String>,
    #[serde(default)]
    evidence_refs: Vec<String>,
}

/// `notes.create`.
pub fn create(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: CreateRequest = decode(payload)?;
    let note_id: NoteId = parse_id(
        &creation_id(ctx, spec, meta, || NoteId::new_v7().as_str().to_string())?,
        "note_id",
    )?;
    let resource = ResourceRef::new(ResourceKind::Note, note_id.as_str());
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |_, _| {
            check_body(&req.body_markdown)?;
            Ok(Facts::new_record(rc(req.domain, req.classification, false)))
        },
        |env| {
            let objective_row = match &req.objective_id {
                None => None,
                Some(code) => {
                    let code: ObjectiveId = parse_id(code, "objective_id")?;
                    let version = active_version(env.conn, env.svc)?.ok_or_else(|| {
                        G9Error::not_found(
                            "curriculum.not_imported",
                            "no curriculum version is imported",
                        )
                    })?;
                    let row = curriculum_repo::objective_by_code(env.conn, &version.id, &code)?
                        .ok_or_else(|| {
                            G9Error::not_found("objective.not_found", "objective not found")
                        })?;
                    Some(row.id)
                }
            };
            let row = NoteRow {
                id: note_id.clone(),
                title: req.title.clone(),
                body_markdown: req.body_markdown.clone(),
                body_sha256: String::new(),
                kind: NoteKind::Journal,
                session_id: None,
                objective_id: objective_row.clone(),
                source_kind: SourceKind::OperatorInput,
                trust: TrustClass::Operator,
                data_domain: req.domain,
                classification: req.classification,
                erased: false,
                erased_at: None,
                erasure_request_id: None,
                revision: Revision::INITIAL,
                created_at: env.now,
                updated_at: env.now,
            };
            let revision = repo::insert_note(env.conn, &row, &env.meta.actor.id, &new_id("nv"))?;
            let stored = repo::get_note(env.conn, &ReadScope::cross_domain(req.domain), &note_id)?
                .ok_or_else(|| G9Error::internal_redacted("note.reload"))?;
            let code = objective_code(objective_row.as_deref());
            let mut m = Mutation::new(view(env.conn, &stored, code)?, Some(revision))
                .with_event(EventSpec {
                    domain: req.domain,
                    content_sha256: Some(sha256_hex(req.body_markdown.as_bytes())),
                    ..EventSpec::learning("note.created", json!({ "note_id": note_id }))
                })
                .replay_with(replay_record(&note_id));
            if !req.evidence_refs.is_empty() {
                m = m.with_warning(
                    "notes.evidence_refs_not_persisted: note-level evidence links are PROPOSED (wave-1 storage gap)",
                );
            }
            Ok(m)
        },
    )
}

#[derive(Debug, Deserialize)]
struct ReviseRequest {
    note_id: String,
    title: Option<String>,
    body_markdown: String,
}

/// `notes.revise` (new `note_versions` row; earlier revisions are kept).
pub fn revise(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: ReviseRequest = decode(payload)?;
    let id: NoteId = parse_id(&req.note_id, "note_id")?;
    let resource = ResourceRef::new(ResourceKind::Note, id.as_str());
    let scope = scope_of(meta);
    let facts_id = id.clone();
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |conn, _| {
            check_body(&req.body_markdown)?;
            let row = repo::get_note(conn, &scope, &facts_id)?
                .ok_or_else(|| G9Error::not_found("note.not_found", "note not found"))?;
            Ok(Facts::existing(
                rc(row.data_domain, row.classification, row.erased),
                row.revision,
            ))
        },
        |env| {
            let row = repo::get_note(env.conn, &scope, &id)?
                .ok_or_else(|| G9Error::not_found("note.not_found", "note not found"))?;
            let title = req.title.clone().unwrap_or_else(|| row.title.clone());
            let (version_no, revision) = repo::revise_note(
                env.conn,
                &id,
                row.revision,
                &title,
                &req.body_markdown,
                &env.meta.actor.id,
                &new_id("nv"),
                env.now,
            )?;
            let stored = repo::get_note(env.conn, &ReadScope::cross_domain(row.data_domain), &id)?
                .ok_or_else(|| G9Error::internal_redacted("note.reload"))?;
            let code = objective_code(stored.objective_id.as_deref());
            Ok(
                Mutation::new(view(env.conn, &stored, code)?, Some(revision))
                    .with_event(EventSpec {
                        domain: row.data_domain,
                        content_sha256: Some(sha256_hex(req.body_markdown.as_bytes())),
                        ..EventSpec::learning(
                            "note.revised",
                            json!({ "note_id": id, "version_no": version_no }),
                        )
                    })
                    .replay_with(replay_record(&id)),
            )
        },
    )
}
