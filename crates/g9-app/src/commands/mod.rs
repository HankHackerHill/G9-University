//! Command handlers, one module per domain. Every handler builds the
//! backend-derived [`ResourceRef`], a `facts` closure that reads the stored
//! resource inside the transaction, and a handler closure; both run through
//! [`crate::pipeline::execute`]. Handlers compose the wave-1 crates (pure
//! domain transitions) with the `g9-storage` repositories; no SQL is written
//! here.

pub mod approvals;
pub mod assessment;
pub mod audit;
pub mod backup;
pub mod calendar;
pub mod competency;
pub mod curriculum;
pub mod notes;
pub mod privacy;
pub mod retention;
pub mod sessions;
pub mod skills;

use crate::context::{AppContext, Services};
use crate::ops::OpSpec;
use g9_domain::{
    AssessmentAttemptState, Classification, CommandMeta, CommandOutcome, DataDomain, G9Error,
    InteractionMode,
};
use g9_policy::ResourceContext;
use g9_storage::repos::assessment as assessment_repo;
use g9_storage::repos::ReadScope;
use rusqlite::Connection;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// Signature shared by every operation handler.
pub type Handler = fn(
    &mut AppContext,
    &'static OpSpec,
    &CommandMeta,
    &Value,
) -> Result<CommandOutcome<Value>, G9Error>;

/// Route an operation to its handler.
pub fn handler_for(name: &str) -> Option<Handler> {
    Some(match name {
        "system.status" => crate::status::handle,
        "curriculum.validate" => curriculum::validate,
        "curriculum.import" => curriculum::import,
        "curriculum.promote" => curriculum::promote,
        "curriculum.daily_plan" => curriculum::daily_plan,
        "calendar.list" => calendar::list,
        "calendar.propose" => calendar::propose,
        "calendar.apply" => calendar::apply,
        "session.list" => sessions::list,
        "session.prepare" => sessions::prepare,
        "session.start" => sessions::start,
        "session.pause" => sessions::pause,
        "session.resume" => sessions::resume,
        "session.complete" => sessions::complete,
        "session.correct_time" => sessions::correct_time,
        "notes.create" => notes::create,
        "notes.revise" => notes::revise,
        "assessment.list" => assessment::list,
        "assessment.begin" => assessment::begin,
        "assessment.submit" => assessment::submit,
        "assessment.review_evidence" => assessment::review_evidence,
        "assessment.grade_provisional" => assessment::grade_provisional,
        "assessment.approve_final" => assessment::approve_final,
        "competency.projection" => competency::projection,
        "retention.due" => retention::due,
        "interventions.list" => retention::interventions,
        "approvals.list" => approvals::list,
        "approvals.decide" => approvals::decide,
        "audit.recent" => audit::recent,
        "backup.create" => backup::create,
        "backup.verify" => backup::verify,
        "backup.restore_test" => backup::restore_test,
        "skills.invoke" => skills::invoke,
        "export.request" => privacy::export_request,
        "deletion.request" => privacy::deletion_request,
        _ => return None,
    })
}

/// Rebuild a content-bearing result on an idempotent replay from its stored
/// content-free record (`pipeline::REPLAY_RECORD_KEY`). Returns the result
/// and the warnings to add to the replayed outcome.
pub(crate) fn rebuild_replay(
    op: &str,
    conn: &Connection,
    svc: &Services,
    record: &Value,
) -> Result<(Value, Vec<String>), G9Error> {
    match op {
        "notes.create" | "notes.revise" => notes::replay_view(conn, svc, record),
        "skills.invoke" => Ok(skills::replay_view(record)),
        _ => Err(G9Error::internal_redacted("idempotency.replay_unsupported")),
    }
}

/// The open EXAM-mode attempt, if any (an attempt in `STARTED` whose mode is
/// `EXAM`). Backend state, never client metadata (TEC-25, SEC-10).
pub(crate) fn open_exam_attempt(conn: &Connection) -> Result<Option<String>, G9Error> {
    let open = assessment_repo::list_attempts_by_state(
        conn,
        &ReadScope::cross_domain(DataDomain::Learning),
        AssessmentAttemptState::Started,
    )?;
    Ok(open
        .into_iter()
        .find(|a| a.mode == InteractionMode::Exam)
        .map(|a| a.id.as_str().to_string()))
}

/// Command metadata with the backend-derived interaction mode (TEC-25,
/// SEC-10): while an EXAM-mode attempt is `STARTED` the command is evaluated
/// in `EXAM`, whatever mode the client declared. The client's mode can never
/// widen the backend-derived one.
pub(crate) fn with_backend_mode(
    ctx: &AppContext,
    meta: &CommandMeta,
) -> Result<CommandMeta, G9Error> {
    let mut effective = meta.clone();
    if open_exam_attempt(ctx.db.conn())?.is_some() {
        effective.scope.mode = InteractionMode::Exam;
    }
    Ok(effective)
}

/// Deserialize a payload that already passed its JSON Schema.
pub(crate) fn decode<T: DeserializeOwned>(payload: &Value) -> Result<T, G9Error> {
    serde_json::from_value(payload.clone())
        .map_err(|_| G9Error::invalid_input("ipc.payload_invalid", "payload could not be decoded"))
}

/// Serialize a view.
pub(crate) fn to_value<T: Serialize>(view: &T) -> Result<Value, G9Error> {
    serde_json::to_value(view).map_err(|_| G9Error::internal_redacted("ipc.serialize"))
}

/// Resource context for policy.
pub(crate) const fn rc(
    domain: DataDomain,
    classification: Classification,
    finalized: bool,
) -> ResourceContext {
    ResourceContext::new(domain, classification, finalized)
}

/// Read scope of the command (cross-domain reads are never granted implicitly).
pub(crate) fn scope_of(meta: &CommandMeta) -> ReadScope {
    ReadScope::from_scope(&meta.scope, false)
}

/// Parse a typed id from a payload string (already pattern-checked by the schema).
pub(crate) fn parse_id<T: std::str::FromStr>(s: &str, what: &str) -> Result<T, G9Error> {
    s.parse().map_err(|_| {
        G9Error::invalid_input(
            "ipc.id_invalid",
            format!("{what} is not a valid identifier"),
        )
    })
}

/// Last `:`-separated segment of a storage row id (`<version>:D01` → `D01`).
pub(crate) fn tail(id: &str) -> &str {
    id.rsplit_once(':').map_or(id, |(_, t)| t)
}

/// The id a creating command works on (app-1). A retry that carries an
/// approval stored for this same action on this same resource kind reuses the
/// id the approval was granted for, so the approved resource matches; every
/// other call mints a fresh id. The id grants nothing by itself: the pipeline
/// still verifies the whole binding (parameters hash, scope, revision, MAC)
/// before anything is written.
pub(crate) fn creation_id(
    ctx: &AppContext,
    spec: &OpSpec,
    meta: &CommandMeta,
    mint: impl FnOnce() -> String,
) -> Result<String, G9Error> {
    if let Some(approval) = &meta.approval_id {
        if let Some(row) = g9_storage::approvals_store::get_request(ctx.db.conn(), approval)? {
            if row.operation.action == spec.action
                && row.operation.resource.kind == spec.resource_kind
            {
                return Ok(row.operation.resource.id);
            }
        }
    }
    Ok(mint())
}

/// Fresh row id `<prefix>-<uuidv7>` (time-ordered; ids never carry content).
pub(crate) fn new_id(prefix: &str) -> String {
    format!("{prefix}-{}", g9_domain::EventId::new_v7())
}
