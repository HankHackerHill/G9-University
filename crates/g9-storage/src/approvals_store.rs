//! Persistence of approval requests, approval decisions and policy decision
//! records as generic rows (M16 storage side; ARCHITECTURE.md §5 steps 3–4,
//! §11). Authority is decided in `g9-policy` and applied by `g9-app`; this
//! module only guarantees that what is stored follows the `ApprovalState`
//! machine, that revisions are checked, and that terminal rows stay immutable
//! (triggers in `0001_governance.sql`).

use crate::error::{StorageError, StorageResult};
use crate::sql::{
    actor_columns, actor_from_columns, enum_from_text, enum_to_text, json_from_text, opt_millis,
    parse_id, require_sha256,
};
use g9_domain::state::ApprovalEvent;
use g9_domain::{
    Actor, ActorId, ApprovalId, ApprovalState, Capability, Decision, OperationDescriptor,
    PolicyDecisionId, RequestId, ResourceRef, Revision, TraceId, UtcMillis,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

/// Input for [`insert_request`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewApprovalRequest {
    /// Approval id.
    pub id: ApprovalId,
    /// Exact operation the approval binds to.
    pub operation: OperationDescriptor,
    /// Capability the approval grants.
    pub required_capability: Option<Capability>,
    /// Requesting actor.
    pub requested_by: ActorId,
    /// When requested.
    pub requested_at: UtcMillis,
    /// Expiry instant (must be after `requested_at`).
    pub expires_at: UtcMillis,
    /// Resource revision the approval is bound to.
    pub expected_revision: Option<Revision>,
}

/// An `approval_requests` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequestRow {
    /// Approval id.
    pub id: ApprovalId,
    /// Bound operation.
    pub operation: OperationDescriptor,
    /// Capability granted on approval.
    pub required_capability: Option<Capability>,
    /// Machine state.
    pub state: ApprovalState,
    /// Requesting actor.
    pub requested_by: ActorId,
    /// When requested.
    pub requested_at: UtcMillis,
    /// Expiry instant.
    pub expires_at: UtcMillis,
    /// Bound resource revision.
    pub expected_revision: Option<Revision>,
    /// Hex HMAC written on approval.
    pub binding_mac: Option<String>,
    /// Deciding actor.
    pub decided_by: Option<ActorId>,
    /// When decided.
    pub decided_at: Option<UtcMillis>,
    /// When consumed.
    pub consumed_at: Option<UtcMillis>,
    /// Optimistic concurrency revision.
    pub revision: Revision,
}

/// A state change to apply with [`transition_request`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalUpdate {
    /// Machine event.
    pub event: ApprovalEvent,
    /// Actor deciding/consuming (required for Approve/Reject/Revoke/Consume).
    pub actor: Option<ActorId>,
    /// When it happened.
    pub at: UtcMillis,
    /// Binding MAC to store on `Approve` (hex HMAC-SHA256 from `g9-policy::binding`).
    pub binding_mac: Option<String>,
}

/// Decision kinds recorded in `approval_decisions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApprovalDecisionKind {
    /// Operator approved.
    Approved,
    /// Operator rejected.
    Rejected,
    /// Operator revoked an approval.
    Revoked,
}

/// An `approval_decisions` row (immutable).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalDecisionRow {
    /// Decision row id.
    pub id: String,
    /// Request decided.
    pub approval_id: ApprovalId,
    /// Decision kind.
    pub decision: ApprovalDecisionKind,
    /// Deciding actor.
    pub decided_by: ActorId,
    /// When decided.
    pub decided_at: UtcMillis,
    /// Optional reason.
    pub reason: Option<String>,
    /// Parameters hash at decision time.
    pub parameters_sha256: String,
    /// Bound revision at decision time.
    pub expected_revision: Option<Revision>,
    /// Hex HMAC bound to the operation.
    pub binding_mac: Option<String>,
}

/// A `policy_decisions` row (immutable, metadata only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDecisionRow {
    /// Decision id.
    pub id: PolicyDecisionId,
    /// When evaluated.
    pub decided_at: UtcMillis,
    /// Evaluated actor.
    pub actor: Actor,
    /// Evaluated operation.
    pub operation: OperationDescriptor,
    /// Engine decision.
    pub decision: Decision,
    /// Approval consulted, if any.
    pub approval_id: Option<ApprovalId>,
    /// Command request id.
    pub request_id: Option<RequestId>,
    /// Command trace id.
    pub trace_id: Option<TraceId>,
}

fn operation_json(op: &OperationDescriptor) -> StorageResult<String> {
    let value = serde_json::to_value(op).map_err(|_| StorageError::Internal("operation json"))?;
    Ok(g9_domain::hashing::canonical_json(&value))
}

/// Insert a `PENDING` request.
pub fn insert_request(
    conn: &Connection,
    req: &NewApprovalRequest,
) -> StorageResult<ApprovalRequestRow> {
    require_sha256("parameters_sha256", &req.operation.parameters_sha256)?;
    if req.expires_at <= req.requested_at {
        return Err(StorageError::InvalidInput(
            "expires_at must be after requested_at".to_string(),
        ));
    }
    conn.execute(
        "INSERT INTO approval_requests (id, operation_json, action, parameters_sha256, resource_kind, resource_id, scope_domain,
            scope_mode, required_capability, state, requested_by, requested_at, expires_at, expected_revision, binding_mac,
            decided_by, decided_at, consumed_at, revision, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'PENDING', ?10, ?11, ?12, ?13, NULL, NULL, NULL, NULL, 1, ?11, ?11)",
        params![
            req.id.as_str(),
            operation_json(&req.operation)?,
            enum_to_text(&req.operation.action)?,
            req.operation.parameters_sha256,
            enum_to_text(&req.operation.resource.kind)?,
            req.operation.resource.id,
            enum_to_text(&req.operation.scope.domain)?,
            enum_to_text(&req.operation.scope.mode)?,
            req.required_capability.as_ref().map(enum_to_text).transpose()?,
            req.requested_by.as_str(),
            req.requested_at.as_millis(),
            req.expires_at.as_millis(),
            req.expected_revision.map(|r| r.get() as i64),
        ],
    )?;
    get_request(conn, &req.id)?.ok_or(StorageError::NotFound("approval request"))
}

/// Read a request.
pub fn get_request(
    conn: &Connection,
    id: &ApprovalId,
) -> StorageResult<Option<ApprovalRequestRow>> {
    let row = conn
        .query_row(
            "SELECT operation_json, required_capability, state, requested_by, requested_at, expires_at, expected_revision,
                    binding_mac, decided_by, decided_at, consumed_at, revision
             FROM approval_requests WHERE id = ?1",
            params![id.as_str()],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, i64>(5)?,
                    r.get::<_, Option<i64>>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, Option<i64>>(9)?,
                    r.get::<_, Option<i64>>(10)?,
                    r.get::<_, i64>(11)?,
                ))
            },
        )
        .optional()?;
    let Some((
        operation_json,
        required_capability,
        state,
        requested_by,
        requested_at,
        expires_at,
        expected_revision,
        binding_mac,
        decided_by,
        decided_at,
        consumed_at,
        revision,
    )) = row
    else {
        return Ok(None);
    };
    let operation: OperationDescriptor =
        serde_json::from_value(json_from_text(&operation_json)?)
            .map_err(|e| StorageError::RowDecode(format!("operation_json: {e}")))?;
    Ok(Some(ApprovalRequestRow {
        id: id.clone(),
        operation,
        required_capability: required_capability
            .as_deref()
            .map(enum_from_text)
            .transpose()?,
        state: enum_from_text(&state)?,
        requested_by: parse_id(&requested_by)?,
        requested_at: UtcMillis::from_millis(requested_at),
        expires_at: UtcMillis::from_millis(expires_at),
        expected_revision: expected_revision.map(|r| Revision::new(r.max(0) as u64)),
        binding_mac,
        decided_by: decided_by.as_deref().map(parse_id).transpose()?,
        decided_at: opt_millis(decided_at),
        consumed_at: opt_millis(consumed_at),
        revision: Revision::new(revision.max(1) as u64),
    }))
}

/// Apply a state-machine event to a request with optimistic concurrency.
///
/// The edge is validated with `ApprovalState::transition`; the row is updated
/// only where `revision = expected_revision` (`STALE_REVISION` otherwise).
/// `Approve` stores `binding_mac`, `decided_by`, `decided_at`; `Reject`/`Revoke`
/// store the decider; `Consume` stores `consumed_at`; `Expire` is system-driven.
pub fn transition_request(
    conn: &Connection,
    id: &ApprovalId,
    expected_revision: Revision,
    update: &ApprovalUpdate,
) -> StorageResult<ApprovalRequestRow> {
    let current = get_request(conn, id)?.ok_or(StorageError::NotFound("approval request"))?;
    if current.revision != expected_revision {
        return Err(StorageError::StaleRevision {
            expected: Some(expected_revision),
            actual: current.revision,
        });
    }
    let next = current.state.transition(update.event)?;
    let needs_actor = matches!(
        update.event,
        ApprovalEvent::Approve
            | ApprovalEvent::Reject
            | ApprovalEvent::Revoke
            | ApprovalEvent::Consume
    );
    let actor = update.actor.as_ref().map(|a| a.as_str().to_string());
    if needs_actor && actor.is_none() {
        return Err(StorageError::InvalidInput(format!(
            "{:?} requires an actor",
            update.event
        )));
    }
    if let Some(mac) = &update.binding_mac {
        require_sha256("binding_mac", mac)?;
    }
    let (decided_by, decided_at, consumed_at, binding_mac) = match update.event {
        ApprovalEvent::Approve => (
            actor,
            Some(update.at.as_millis()),
            None,
            update.binding_mac.clone(),
        ),
        ApprovalEvent::Reject | ApprovalEvent::Revoke => (
            actor,
            Some(update.at.as_millis()),
            None,
            current.binding_mac.clone(),
        ),
        ApprovalEvent::Consume => (
            current.decided_by.as_ref().map(|a| a.as_str().to_string()),
            current.decided_at.map(UtcMillis::as_millis),
            Some(update.at.as_millis()),
            current.binding_mac.clone(),
        ),
        ApprovalEvent::Expire => (
            current.decided_by.as_ref().map(|a| a.as_str().to_string()),
            current.decided_at.map(UtcMillis::as_millis),
            None,
            current.binding_mac.clone(),
        ),
    };
    let changed = conn.execute(
        "UPDATE approval_requests SET state = ?2, decided_by = ?3, decided_at = ?4, consumed_at = ?5, binding_mac = ?6,
            revision = revision + 1, updated_at = ?7
         WHERE id = ?1 AND revision = ?8",
        params![
            id.as_str(),
            enum_to_text(&next)?,
            decided_by,
            decided_at,
            consumed_at,
            binding_mac,
            update.at.as_millis(),
            expected_revision.get() as i64,
        ],
    )?;
    if changed != 1 {
        return Err(StorageError::StaleRevision {
            expected: Some(expected_revision),
            actual: current.revision.next(),
        });
    }
    get_request(conn, id)?.ok_or(StorageError::NotFound("approval request"))
}

/// Expire every `PENDING`/`APPROVED` request whose `expires_at <= now`; returns their ids.
pub fn expire_due(conn: &Connection, now: UtcMillis) -> StorageResult<Vec<ApprovalId>> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM approval_requests WHERE state IN ('PENDING','APPROVED') AND expires_at <= ?1 ORDER BY expires_at, id",
        )?;
        let rows = stmt.query_map(params![now.as_millis()], |r| r.get::<_, String>(0))?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row?);
        }
        v
    };
    let mut expired = Vec::new();
    for id in ids {
        conn.execute(
            "UPDATE approval_requests SET state = 'EXPIRED', revision = revision + 1, updated_at = ?2
             WHERE id = ?1 AND state IN ('PENDING','APPROVED')",
            params![id, now.as_millis()],
        )?;
        expired.push(parse_id(&id)?);
    }
    Ok(expired)
}

/// Requests in a given state, oldest first.
pub fn list_by_state(
    conn: &Connection,
    state: ApprovalState,
) -> StorageResult<Vec<ApprovalRequestRow>> {
    let ids: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM approval_requests WHERE state = ?1 ORDER BY requested_at, id",
        )?;
        let rows = stmt.query_map(params![enum_to_text(&state)?], |r| r.get::<_, String>(0))?;
        let mut v = Vec::new();
        for row in rows {
            v.push(row?);
        }
        v
    };
    let mut out = Vec::new();
    for id in ids {
        if let Some(row) = get_request(conn, &parse_id(&id)?)? {
            out.push(row);
        }
    }
    Ok(out)
}

/// Insert an immutable decision row.
pub fn insert_decision(conn: &Connection, row: &ApprovalDecisionRow) -> StorageResult<()> {
    require_sha256("parameters_sha256", &row.parameters_sha256)?;
    if let Some(mac) = &row.binding_mac {
        require_sha256("binding_mac", mac)?;
    }
    conn.execute(
        "INSERT INTO approval_decisions (id, approval_id, decision, decided_by, decided_at, reason, parameters_sha256,
            expected_revision, binding_mac, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?5)",
        params![
            row.id,
            row.approval_id.as_str(),
            enum_to_text(&row.decision)?,
            row.decided_by.as_str(),
            row.decided_at.as_millis(),
            row.reason,
            row.parameters_sha256,
            row.expected_revision.map(|r| r.get() as i64),
            row.binding_mac,
        ],
    )?;
    Ok(())
}

/// Insert an immutable policy decision record.
pub fn insert_policy_decision(conn: &Connection, row: &PolicyDecisionRow) -> StorageResult<()> {
    require_sha256("parameters_sha256", &row.operation.parameters_sha256)?;
    let (actor_id, actor_kind, actor_skill_id) = actor_columns(&row.actor);
    let (decision, rule_id, reason, required_capability) = match &row.decision {
        Decision::Allow { rule_id } => ("ALLOW", rule_id.clone(), None, None),
        Decision::Deny { rule_id, reason } => ("DENY", rule_id.clone(), Some(reason.clone()), None),
        Decision::ApprovalRequired {
            rule_id,
            reason,
            required_capability,
        } => (
            "APPROVAL_REQUIRED",
            rule_id.clone(),
            Some(reason.clone()),
            Some(enum_to_text(required_capability)?),
        ),
    };
    conn.execute(
        "INSERT INTO policy_decisions (id, decided_at, actor_id, actor_kind, actor_skill_id, action, resource_kind, resource_id,
            scope_domain, scope_mode, parameters_sha256, decision, rule_id, reason, required_capability, approval_id,
            request_id, trace_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?2)",
        params![
            row.id.as_str(),
            row.decided_at.as_millis(),
            actor_id,
            actor_kind,
            actor_skill_id,
            enum_to_text(&row.operation.action)?,
            enum_to_text(&row.operation.resource.kind)?,
            row.operation.resource.id,
            enum_to_text(&row.operation.scope.domain)?,
            enum_to_text(&row.operation.scope.mode)?,
            row.operation.parameters_sha256,
            decision,
            rule_id,
            reason,
            required_capability,
            row.approval_id.as_ref().map(|a| a.as_str().to_string()),
            row.request_id.as_ref().map(|a| a.as_str().to_string()),
            row.trace_id.as_ref().map(|a| a.as_str().to_string()),
        ],
    )?;
    Ok(())
}

/// Policy decisions for one resource, newest first.
pub fn list_policy_decisions_for(
    conn: &Connection,
    resource: &ResourceRef,
    limit: u32,
) -> StorageResult<Vec<PolicyDecisionRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, decided_at, actor_id, actor_kind, actor_skill_id, action, resource_kind, resource_id, scope_domain, scope_mode,
                parameters_sha256, decision, rule_id, reason, required_capability, approval_id, request_id, trace_id
         FROM policy_decisions WHERE resource_kind = ?1 AND resource_id = ?2 ORDER BY decided_at DESC, id DESC LIMIT ?3",
    )?;
    let rows = stmt.query_map(
        params![enum_to_text(&resource.kind)?, resource.id, i64::from(limit)],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, String>(9)?,
                r.get::<_, String>(10)?,
                r.get::<_, String>(11)?,
                r.get::<_, String>(12)?,
                r.get::<_, Option<String>>(13)?,
                r.get::<_, Option<String>>(14)?,
                r.get::<_, Option<String>>(15)?,
                r.get::<_, Option<String>>(16)?,
                r.get::<_, Option<String>>(17)?,
            ))
        },
    )?;
    let mut out = Vec::new();
    for row in rows {
        let (
            id,
            decided_at,
            actor_id,
            actor_kind,
            actor_skill_id,
            action,
            resource_kind,
            resource_id,
            scope_domain,
            scope_mode,
            parameters_sha256,
            decision,
            rule_id,
            reason,
            required_capability,
            approval_id,
            request_id,
            trace_id,
        ) = row?;
        let decision = match decision.as_str() {
            "ALLOW" => Decision::Allow { rule_id },
            "DENY" => Decision::Deny {
                rule_id,
                reason: reason.unwrap_or_default(),
            },
            "APPROVAL_REQUIRED" => Decision::ApprovalRequired {
                rule_id,
                reason: reason.unwrap_or_default(),
                required_capability: enum_from_text(required_capability.as_deref().ok_or_else(
                    || StorageError::RowDecode("APPROVAL_REQUIRED without capability".into()),
                )?)?,
            },
            other => return Err(StorageError::RowDecode(format!("decision {other:?}"))),
        };
        out.push(PolicyDecisionRow {
            id: parse_id(&id)?,
            decided_at: UtcMillis::from_millis(decided_at),
            actor: actor_from_columns(&actor_id, &actor_kind, actor_skill_id.as_deref())?,
            operation: OperationDescriptor {
                action: enum_from_text(&action)?,
                resource: ResourceRef::new(enum_from_text(&resource_kind)?, resource_id),
                scope: g9_domain::Scope {
                    domain: enum_from_text(&scope_domain)?,
                    mode: enum_from_text(&scope_mode)?,
                },
                parameters_sha256,
            },
            decision,
            approval_id: approval_id.as_deref().map(parse_id).transpose()?,
            request_id: request_id.as_deref().map(parse_id).transpose()?,
            trace_id: trace_id.as_deref().map(parse_id).transpose()?,
        });
    }
    Ok(out)
}
