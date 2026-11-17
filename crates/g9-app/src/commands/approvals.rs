//! `approvals.list` and `approvals.decide` (M16; ARCHITECTURE.md §11; ADR-0003).
//!
//! A decision is an operator command like any other: it passes the pipeline
//! (`Action::Approve` on `ResourceKind::Approval`, rule `approval.decide.allow`),
//! the ledger computes the binding MAC with the per-installation key on
//! approve, and the `approval_requests` row plus an immutable
//! `approval_decisions` row are written in the command transaction. The native
//! confirmation dialog raised from Rust (ADR-0014, SEC-03) is PROPOSED for
//! Stage 03 and is not part of this handler.
//!
//! A decision refused because the request expired (its window has passed, or
//! the wall clock reads earlier than the request instant: SEC-02, fail closed)
//! moves the request to `EXPIRED` and commits that with the refusal
//! ([`Mutation::failed`]): the caller gets the same `approval.expired` error,
//! and a wall clock corrected or stepped back afterwards cannot revive the
//! request.

use super::{decode, rc, to_value};
use crate::context::AppContext;
use crate::ops::{self, OpSpec};
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation};
use g9_domain::state::ApprovalEvent;
use g9_domain::{
    Actor, ApprovalId, ApprovalState, Classification, CommandMeta, CommandOutcome, DataDomain,
    G9Error, ResourceKind, ResourceRef, Revision, UtcMillis,
};
use g9_policy::ApprovalError;
use g9_storage::approvals_store::{
    self, ApprovalDecisionKind, ApprovalDecisionRow, ApprovalRequestRow, ApprovalUpdate,
};
use serde::Deserialize;
use serde_json::{json, Value};

/// Effective state at `now`: a pending/approved request past its expiry is
/// reported `EXPIRED` even before the sweep persisted it.
fn effective_state(row: &ApprovalRequestRow, now: UtcMillis) -> ApprovalState {
    match row.state {
        ApprovalState::Pending | ApprovalState::Approved if now >= row.expires_at => {
            ApprovalState::Expired
        }
        s => s,
    }
}

/// UI view of an approval request (`ApprovalRequest` in `operations.ts`).
pub(crate) fn view(row: &ApprovalRequestRow, now: UtcMillis) -> Value {
    let name = ops::name_for_approval(row.operation.action, row.operation.resource.kind);
    let capability = row
        .required_capability
        .map(|c| serde_json::to_value(c).unwrap_or(Value::Null));
    let summary = format!(
        "{name}: {} {} (requires {})",
        serde_json::to_value(row.operation.resource.kind)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default(),
        row.operation.resource.id,
        capability
            .as_ref()
            .and_then(|v| v.as_str())
            .unwrap_or("no capability"),
    );
    json!({
        "approval_id": row.id,
        "state": effective_state(row, now),
        "operation_name": name,
        "operation": row.operation,
        "requested_by": row.requested_by,
        "requested_at": row.requested_at,
        "expires_at": row.expires_at,
        "expected_revision": row.expected_revision,
        "decided_at": row.decided_at,
        "consumed_at": row.consumed_at,
        "summary": summary,
        "revision": row.revision,
    })
}

#[derive(Debug, Deserialize)]
struct ListRequest {
    state: Option<ApprovalState>,
}

/// `approvals.list`.
pub fn list(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: ListRequest = decode(payload)?;
    let resource = ResourceRef::new(ResourceKind::Approval, "approvals");
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |_, _| {
            Ok(Facts::existing(
                rc(DataDomain::System, Classification::Internal, false),
                Revision::INITIAL,
            ))
        },
        |env| {
            let mut rows = Vec::new();
            for state in ApprovalState::ALL {
                rows.extend(approvals_store::list_by_state(env.conn, *state)?);
            }
            let mut views: Vec<(UtcMillis, String, Value)> = rows
                .iter()
                .filter(|r| req.state.is_none_or(|s| effective_state(r, env.now) == s))
                .map(|r| (r.requested_at, r.id.as_str().to_string(), view(r, env.now)))
                .collect();
            views.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
            let approvals: Vec<Value> = views.into_iter().map(|v| v.2).collect();
            Ok(Mutation::new(json!({ "approvals": approvals }), None))
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum Verdict {
    Approve,
    Reject,
}

#[derive(Debug, Deserialize)]
struct DecideRequest {
    approval_id: String,
    decision: Verdict,
}

/// `approvals.decide`.
pub fn decide(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: DecideRequest = decode(payload)?;
    let id: ApprovalId = super::parse_id(&req.approval_id, "approval_id")?;
    let resource = ResourceRef::new(ResourceKind::Approval, id.as_str());
    let facts_id = id.clone();
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        move |conn, _| {
            let row = approvals_store::get_request(conn, &facts_id)?
                .ok_or_else(|| G9Error::not_found("approval.not_found", "approval not found"))?;
            Ok(Facts::existing(
                rc(
                    DataDomain::System,
                    Classification::Internal,
                    row.state.is_terminal(),
                ),
                row.revision,
            ))
        },
        move |env| {
            // ADR-0014, SEC-03: the webview's request is a proposal; the grant is
            // the operator's confirmation outside it. Rejecting needs none.
            if matches!(req.decision, Verdict::Approve)
                && env.svc.config.native_confirmation
                && env.svc.native_confirmed.as_ref() != Some(&id)
            {
                return Err(G9Error::unauthorized(
                    "approval.native_confirmation_required",
                    "an approval is granted only in the confirmation dialog the application shows",
                ));
            }
            let operator = env.svc.operator.clone();
            env.svc.approvals.refresh(env.conn, &id, &operator)?;
            let row = approvals_store::get_request(env.conn, &id)?
                .ok_or_else(|| G9Error::not_found("approval.not_found", "approval not found"))?;
            if effective_state(&row, env.now) == ApprovalState::Expired {
                return refuse_expired(
                    env,
                    &id,
                    &operator,
                    G9Error::precondition_failed(
                        "approval.expired",
                        "the approval request expired before a decision was made",
                    ),
                );
            }
            let (event, kind, mac) = match req.decision {
                Verdict::Approve => {
                    // The key is borrowed, never copied (core-11): the ledger and the key are
                    // separate fields of the services.
                    let svc = &mut *env.svc;
                    let approved = svc
                        .approvals
                        .ledger_mut()
                        .approve(&id, &operator, env.now, &svc.binding_key)
                        .map(|record| record.binding_mac.clone());
                    let mac = match approved {
                        Ok(mac) => mac,
                        // The wall clock reads earlier than the request instant (SEC-02).
                        Err(ApprovalError::Expired) => {
                            return refuse_expired(
                                env,
                                &id,
                                &operator,
                                ApprovalError::Expired.into(),
                            );
                        }
                        Err(e) => return Err(e.into()),
                    };
                    (ApprovalEvent::Approve, ApprovalDecisionKind::Approved, mac)
                }
                Verdict::Reject => {
                    env.svc.approvals.ledger_mut().reject(
                        &id,
                        &operator,
                        env.now,
                        "rejected by the operator",
                    )?;
                    (ApprovalEvent::Reject, ApprovalDecisionKind::Rejected, None)
                }
            };
            let updated = approvals_store::transition_request(
                env.conn,
                &id,
                row.revision,
                &ApprovalUpdate {
                    event,
                    actor: Some(operator.id.clone()),
                    at: env.now,
                    binding_mac: mac.clone(),
                },
            )?;
            approvals_store::insert_decision(
                env.conn,
                &ApprovalDecisionRow {
                    id: format!("{}:decision", id.as_str()),
                    approval_id: id.clone(),
                    decision: kind,
                    decided_by: operator.id.clone(),
                    decided_at: env.now,
                    reason: None,
                    parameters_sha256: row.operation.parameters_sha256.clone(),
                    expected_revision: row.expected_revision,
                    binding_mac: mac,
                },
            )?;
            env.svc.approvals.refresh(env.conn, &id, &operator)?;
            Ok(Mutation::new(to_value(&view(&updated, env.now))?, Some(updated.revision))
                .with_event(EventSpec::system(
                    "approval.decided",
                    json!({ "approval_id": id, "decision": kind, "operation": ops::name_for_approval(row.operation.action, row.operation.resource.kind) }),
                )))
        },
    )
}

/// Refuse a decision on an expired request and persist the expiry with the
/// refusal (core-7, SEC-02): the row moves to `EXPIRED` in the command
/// transaction, which commits with the decision and audit rows of the refusal
/// and answers `error` (module docs). The ledger is reloaded from the row.
fn refuse_expired(
    env: &mut pipeline::Env<'_>,
    id: &ApprovalId,
    operator: &Actor,
    error: G9Error,
) -> Result<Mutation, G9Error> {
    pipeline::expire_presented(env.conn, id, env.now)?;
    env.svc.approvals.refresh(env.conn, id, operator)?;
    Ok(Mutation::failed(error))
}
