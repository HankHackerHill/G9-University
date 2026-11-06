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

use crate::context::AppContext;
use crate::ops::OpSpec;
use g9_domain::{Classification, CommandMeta, CommandOutcome, DataDomain, G9Error};
use g9_policy::ResourceContext;
use g9_storage::repos::ReadScope;
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

/// Fresh row id `<prefix>-<uuidv7>` (time-ordered; ids never carry content).
pub(crate) fn new_id(prefix: &str) -> String {
    format!("{prefix}-{}", g9_domain::EventId::new_v7())
}
