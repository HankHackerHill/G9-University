//! `retention.due` and `interventions.list` (read-only; M08, M12).
//!
//! The due queue is planned by `g9_retention::plan_reviews` under the daily
//! review budget (default 15 minutes, ARCHITECTURE.md §9); every due item is
//! read (no row limit) and items that do not fit are returned as `overflow`,
//! never dropped; an item whose scheduler state cannot be read is reported in
//! a warning. "Overdue" means due before local midnight (00:00:00) of `as_of`
//! in the operator zone. The per-item time estimate
//! is a scheduling parameter (ASSUMED 3 minutes, [`REVIEW_ITEM_ESTIMATE_MS`]),
//! not a measurement. Interventions are listed as stored; model-originated
//! proposals keep their `MODEL_OUTPUT` provenance.
//!
//! Reported gap: the v1.0 response vocabulary has no `PROCEDURAL` kind, so a
//! memory-scheduled `PROCEDURAL` review item is labelled `PRACTICAL_TRANSFER`
//! (it is NOT a `practical_transfer_checks` row); every such outcome carries
//! the warning `retention.due.procedural_label` until the response schema and
//! the UI add `PROCEDURAL` (PROPOSED, outside `g9-app`).

use super::{decode, rc, scope_of};
use crate::context::AppContext;
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, Facts, Mutation};
use g9_domain::{
    Classification, CommandMeta, CommandOutcome, DurationMs, G9Error, InterventionState,
    ResourceKind, ResourceRef, UtcMillis,
};
use g9_retention::plan_reviews;
use g9_storage::repos::retention::{self as repo, ReviewItemKind};
use serde::Deserialize;
use serde_json::{json, Value};

/// Default daily review budget (ARCHITECTURE.md §9: 15 minutes).
pub const DEFAULT_REVIEW_BUDGET_MS: i64 = 15 * 60_000;
/// Planning estimate per review item (ASSUMED scheduling parameter).
pub const REVIEW_ITEM_ESTIMATE_MS: i64 = 3 * 60_000;

#[derive(Debug, Deserialize)]
struct DueRequest {
    as_of: Option<UtcMillis>,
    budget_ms: Option<i64>,
}

/// `retention.due`.
pub fn due(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: DueRequest = decode(payload)?;
    let resource = ResourceRef::new(ResourceKind::ReviewItem, "due");
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
            let as_of = req.as_of.unwrap_or(env.now);
            let budget = DurationMs::new(req.budget_ms.unwrap_or(DEFAULT_REVIEW_BUDGET_MS))
                .map_err(|_| {
                    G9Error::invalid_input(
                        "retention.budget_invalid",
                        "budget must be non-negative",
                    )
                })?;
            let estimate = DurationMs::new(REVIEW_ITEM_ESTIMATE_MS)
                .map_err(|_| G9Error::internal_redacted("retention.estimate"))?;
            // Every due item (no row limit): overflow is reported, never dropped.
            let items = repo::list_due_items(env.conn, &scope, as_of, u32::MAX)?;
            let mut states = Vec::new();
            let mut stateless = Vec::new();
            for item in &items {
                match repo::item_state(env.conn, &item.id)? {
                    Some(s) => states.push(s),
                    None => stateless.push(item.id.as_str().to_string()),
                }
            }
            let plan = plan_reviews(&states, budget, estimate, as_of)
                .map_err(|e| G9Error::invalid_input("retention.plan_invalid", e.to_string()))?;
            let start_of_day = g9_calendar::to_civil(as_of, &env.svc.config.zone)
                .ok()
                .and_then(|c| {
                    let midnight = g9_domain::CivilDateTime {
                        hour: 0,
                        minute: 0,
                        second: 0,
                        ..c
                    };
                    g9_calendar::resolve_local(&midnight, &env.svc.config.zone).ok()
                })
                .map_or(as_of, |r| r.instant);
            let view = |id: &g9_domain::ReviewItemId| -> Option<(bool, Value)> {
                let item = items.iter().find(|i| &i.id == id)?;
                let kind = match item.kind {
                    ReviewItemKind::Declarative => "DECLARATIVE",
                    // The UI vocabulary has no PROCEDURAL kind (reported gap).
                    ReviewItemKind::Procedural => "PRACTICAL_TRANSFER",
                };
                Some((
                    item.due_at < start_of_day,
                    json!({
                        "review_item_id": item.id,
                        "competency_id": item.competency_id,
                        "objective_id": item.objective_id.as_deref().map(super::tail),
                        "prompt": item.prompt,
                        "kind": kind,
                        "due_at": item.due_at,
                        "last_success_at": item.last_success_at,
                        "offset_days": item.interval_days,
                        "estimated_ms": REVIEW_ITEM_ESTIMATE_MS,
                    }),
                ))
            };
            let mut due = Vec::new();
            let mut overdue = Vec::new();
            for id in &plan.scheduled {
                if let Some((late, v)) = view(id) {
                    if late {
                        overdue.push(v)
                    } else {
                        due.push(v)
                    }
                }
            }
            let overflow: Vec<Value> = plan
                .deferred_overflow
                .iter()
                .filter_map(|id| view(id).map(|v| v.1))
                .collect();
            let procedural = items
                .iter()
                .filter(|i| i.kind == ReviewItemKind::Procedural)
                .count();
            let mut mutation = Mutation::new(
                json!({ "as_of": as_of, "budget_ms": budget, "due": due, "overdue": overdue, "overflow": overflow }),
                None,
            );
            if procedural > 0 {
                mutation = mutation.with_warning(format!(
                    "retention.due.procedural_label: {procedural} PROCEDURAL review item(s) are memory-scheduled recall items labelled PRACTICAL_TRANSFER because the v1.0 response has no PROCEDURAL kind; they are not practical transfer checks"
                ));
            }
            if !stateless.is_empty() {
                mutation = mutation.with_warning(format!(
                    "retention.due.item_state_missing: no scheduler state for {}; not planned",
                    stateless.join(", ")
                ));
            }
            Ok(mutation)
        },
    )
}

#[derive(Debug, Deserialize)]
struct InterventionsRequest {
    state: Option<InterventionState>,
}

/// `interventions.list`.
pub fn interventions(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: InterventionsRequest = decode(payload)?;
    let resource = ResourceRef::new(ResourceKind::Intervention, "interventions");
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
            let interventions: Vec<Value> = repo::list_interventions(env.conn, &scope, req.state)?
                .into_iter()
                .map(|i| {
                    json!({
                        "intervention_id": i.id,
                        "state": i.state,
                        "hypothesis_id": i.hypothesis_id,
                        "statement": i.description,
                        "steps": i.steps,
                        "time_budget_ms": i.time_budget,
                        "acceptance": i.acceptance,
                        "provenance": i.origin_trust,
                        "created_at": i.created_at,
                        "revision": i.revision,
                    })
                })
                .collect();
            Ok(Mutation::new(
                json!({ "interventions": interventions }),
                None,
            ))
        },
    )
}
