//! Approval requests, operator decisions and single-use consumption
//! (ARCHITECTURE.md §5 step 4 and §11; handoff §12 "Operating approvals" and the
//! "Policy bypass/TOCTOU" row of the threat-control matrix).
//!
//! Life cycle (the [`g9_domain::ApprovalState`] machine):
//!
//! ```text
//! request() ──► Pending ──approve()──► Approved ──verify_and_consume()──► Consumed
//!                 │                       │
//!                 ├──reject()──► Rejected ├──revoke()──► Revoked
//!                 └──expire_due()─► Expired ◄──expire_due()/lazy expiry──┘
//! ```
//!
//! An approval binds to **one exact operation**: the canonical JSON of the
//! [`OperationDescriptor`] (action, resource, scope, parameters hash), the
//! `expected_revision` the operator saw, and the approval id, all covered by the
//! HMAC in [`crate::binding`]. [`ApprovalLedger::verify_and_consume`] checks every
//! binding, then moves the record to `Consumed` in the same call, so that a
//! second use, a use against another resource, a stale revision or a forged MAC
//! is refused with a specific [`ApprovalError`] variant. `g9-app` runs that call
//! inside the command's `BEGIN IMMEDIATE` transaction (TOCTOU-safe).
//!
//! The ledger is in-memory and serializable so that `g9-app` can persist
//! snapshots; the binding key is **never** part of a snapshot.

use crate::binding::{self, ct_str_eq, BindingError, BindingKey};
use g9_domain::hashing::sha256_json;
use g9_domain::state::ApprovalEvent;
use g9_domain::{
    Actor, ActorId, ApprovalId, ApprovalState, Capability, DurationMs, OperationDescriptor,
    Revision, UtcMillis,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Default validity window of an approval (ARCHITECTURE.md §11: 15 minutes).
pub const DEFAULT_APPROVAL_TTL: DurationMs = DurationMs::from_minutes_u32(15);

/// Every way an approval can fail to be created, decided or consumed. Each
/// variant has a stable machine code ([`ApprovalError::code`]) that becomes the
/// `policy_decisions` rule id when a consumption is refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ApprovalError {
    /// No approval with this id exists in the ledger.
    #[error("approval not found")]
    NotFound,
    /// The approval is not in the `Approved` state (pending, rejected, expired, revoked).
    #[error("approval is {state}, not APPROVED")]
    NotApproved {
        /// Current state.
        state: ApprovalState,
    },
    /// The approval's validity window has passed.
    #[error("approval has expired")]
    Expired,
    /// The operation (action, scope or parameters hash) differs from the bound one.
    #[error("approval is bound to a different operation")]
    OperationMismatch,
    /// The resource differs from the bound one.
    #[error("approval is bound to a different resource")]
    ResourceMismatch,
    /// The expected revision differs from the one the operator approved.
    #[error(
        "approval is bound to expected revision {bound:?}; the operation carries {expected:?}"
    )]
    RevisionMismatch {
        /// Revision carried by the operation being authorized.
        expected: Option<Revision>,
        /// Revision the approval was bound to.
        bound: Option<Revision>,
    },
    /// The approval was already spent (single use).
    #[error("approval was already consumed")]
    AlreadyConsumed,
    /// The binding MAC is missing, malformed or does not verify.
    #[error("approval binding MAC is invalid")]
    MacInvalid,
    /// The approval grants a different capability than the rule requires.
    #[error("approval grants {granted:?} but the operation requires {required:?}")]
    CapabilityMismatch {
        /// Capability the winning rule requires.
        required: Capability,
        /// Capability the approval was requested for.
        granted: Capability,
    },
    /// Only the actor that requested the approval may consume it.
    #[error("approval may only be consumed by the actor that requested it")]
    RequesterMismatch,
    /// Only the operator can approve, reject or revoke.
    #[error("only the operator can decide an approval")]
    DeciderNotOperator,
    /// An operator-only capability was requested by, or would be granted to, a non-operator.
    #[error("capability {capability:?} is operator-only and cannot be requested by or granted to a non-operator actor")]
    OperatorOnlyCapability {
        /// Offending capability.
        capability: Capability,
    },
    /// A request with this id already exists.
    #[error("an approval with this id already exists")]
    AlreadyExists,
    /// The state machine refused the event.
    #[error("approval event {event:?} is not allowed in state {from}")]
    InvalidTransition {
        /// State before the event.
        from: ApprovalState,
        /// Refused event.
        event: ApprovalEvent,
    },
    /// `expires_at` is not after `requested_at`, or the TTL overflowed.
    #[error("approval expiry must be after the request time")]
    InvalidExpiry,
    /// The MAC could not be computed.
    #[error("approval binding failed: {0}")]
    Binding(#[from] BindingError),
}

impl ApprovalError {
    /// Stable machine code, e.g. `approval.already_consumed`.
    pub const fn code(&self) -> &'static str {
        match self {
            ApprovalError::NotFound => "approval.not_found",
            ApprovalError::NotApproved { .. } => "approval.not_approved",
            ApprovalError::Expired => "approval.expired",
            ApprovalError::OperationMismatch => "approval.operation_mismatch",
            ApprovalError::ResourceMismatch => "approval.resource_mismatch",
            ApprovalError::RevisionMismatch { .. } => "approval.revision_mismatch",
            ApprovalError::AlreadyConsumed => "approval.already_consumed",
            ApprovalError::MacInvalid => "approval.mac_invalid",
            ApprovalError::CapabilityMismatch { .. } => "approval.capability_mismatch",
            ApprovalError::RequesterMismatch => "approval.requester_mismatch",
            ApprovalError::DeciderNotOperator => "approval.decider_not_operator",
            ApprovalError::OperatorOnlyCapability { .. } => "approval.operator_only_capability",
            ApprovalError::AlreadyExists => "approval.already_exists",
            ApprovalError::InvalidTransition { .. } => "approval.invalid_transition",
            ApprovalError::InvalidExpiry => "approval.invalid_expiry",
            ApprovalError::Binding(_) => "approval.binding_failed",
        }
    }
}

/// A request for the operator to authorize one exact operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Approval id (becomes `CommandMeta::approval_id` on the retry).
    pub id: ApprovalId,
    /// The exact operation; approvals bind to its canonical JSON.
    pub operation: OperationDescriptor,
    /// The actor whose command triggered the request. Only this actor may
    /// consume the approval; a non-operator may never request an operator-only
    /// capability.
    pub requested_by: Actor,
    /// When the request was created (engine clock).
    pub requested_at: UtcMillis,
    /// Hard expiry; the approval is unusable at or after this instant.
    pub expires_at: UtcMillis,
    /// Revision of the resource the operator will be looking at when deciding.
    /// Must equal the retry's `expected_revision`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<Revision>,
    /// Capability the winning policy rule demands.
    pub required_capability: Capability,
    /// Secret-free, operator-facing justification (the rule rationale).
    pub reason: String,
}

impl ApprovalRequest {
    /// Build a request that expires [`DEFAULT_APPROVAL_TTL`] after `requested_at`.
    pub fn new(
        id: ApprovalId,
        operation: OperationDescriptor,
        requested_by: Actor,
        requested_at: UtcMillis,
        expected_revision: Option<Revision>,
        required_capability: Capability,
        reason: impl Into<String>,
    ) -> Result<Self, ApprovalError> {
        let expires_at = requested_at
            .checked_add(DEFAULT_APPROVAL_TTL)
            .map_err(|_| ApprovalError::InvalidExpiry)?;
        let request = Self {
            id,
            operation,
            requested_by,
            requested_at,
            expires_at,
            expected_revision,
            required_capability,
            reason: reason.into(),
        };
        request.validate()?;
        Ok(request)
    }

    /// Replace the validity window with `ttl` after `requested_at`.
    pub fn with_ttl(self, ttl: DurationMs) -> Result<Self, ApprovalError> {
        let expires_at = self
            .requested_at
            .checked_add(ttl)
            .map_err(|_| ApprovalError::InvalidExpiry)?;
        self.with_expires_at(expires_at)
    }

    /// Replace the hard expiry (must be strictly after `requested_at`).
    pub fn with_expires_at(mut self, expires_at: UtcMillis) -> Result<Self, ApprovalError> {
        self.expires_at = expires_at;
        self.validate()?;
        Ok(self)
    }

    /// Structural checks: expiry after request time; operator-only capability
    /// only for the operator.
    pub fn validate(&self) -> Result<(), ApprovalError> {
        if self.expires_at <= self.requested_at {
            return Err(ApprovalError::InvalidExpiry);
        }
        if self.required_capability.operator_only() && !self.requested_by.is_operator() {
            return Err(ApprovalError::OperatorOnlyCapability {
                capability: self.required_capability,
            });
        }
        Ok(())
    }

    /// Whether the request is expired at `now` (inclusive bound).
    pub fn is_expired_at(&self, now: UtcMillis) -> bool {
        now >= self.expires_at
    }
}

/// The request plus everything that happened to it. Serializable (snapshot);
/// the binding MAC is included because it cannot be forged without the key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRecord {
    /// The original request (immutable once created).
    pub request: ApprovalRequest,
    /// Current state.
    pub state: ApprovalState,
    /// Operator that approved or rejected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<ActorId>,
    /// When the approve/reject decision was made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<UtcMillis>,
    /// Optional secret-free note from the deciding/revoking operator.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision_note: Option<String>,
    /// Hex HMAC-SHA256 computed at approval time (see [`crate::binding`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binding_mac: Option<String>,
    /// Operator that revoked an approved request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_by: Option<ActorId>,
    /// When the approval was revoked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<UtcMillis>,
    /// When the approval was marked expired.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expired_at: Option<UtcMillis>,
    /// Actor that spent the approval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumed_by: Option<ActorId>,
    /// When the approval was spent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumed_at: Option<UtcMillis>,
}

impl ApprovalRecord {
    fn pending(request: ApprovalRequest) -> Self {
        Self {
            request,
            state: ApprovalState::Pending,
            decided_by: None,
            decided_at: None,
            decision_note: None,
            binding_mac: None,
            revoked_by: None,
            revoked_at: None,
            expired_at: None,
            consumed_by: None,
            consumed_at: None,
        }
    }

    /// Apply a state-machine event or refuse it.
    fn apply(&mut self, event: ApprovalEvent) -> Result<(), ApprovalError> {
        let next = self
            .state
            .transition(event)
            .map_err(|_| ApprovalError::InvalidTransition {
                from: self.state,
                event,
            })?;
        self.state = next;
        Ok(())
    }

    /// Mark expired if the window has passed; returns `true` when it did.
    fn expire_if_due(&mut self, now: UtcMillis) -> bool {
        if self.request.is_expired_at(now) && self.apply(ApprovalEvent::Expire).is_ok() {
            self.expired_at = Some(now);
            return true;
        }
        false
    }

    /// Whether the record can still be consumed at `now`.
    pub fn is_usable_at(&self, now: UtcMillis) -> bool {
        self.state == ApprovalState::Approved && !self.request.is_expired_at(now)
    }
}

/// Proof that an approval was verified and spent for one operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumedApproval {
    /// The spent approval.
    pub approval_id: ApprovalId,
    /// The operation it authorized.
    pub operation: OperationDescriptor,
    /// The capability it granted.
    pub granted_capability: Capability,
    /// Revision bound at approval time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<Revision>,
    /// Who requested it.
    pub requested_by: ActorId,
    /// Which operator approved it.
    pub decided_by: ActorId,
    /// When it was approved.
    pub decided_at: UtcMillis,
    /// Who spent it.
    pub consumed_by: ActorId,
    /// When it was spent.
    pub consumed_at: UtcMillis,
}

/// Everything the ledger needs to decide whether an approval may be spent.
#[derive(Debug, Clone, Copy)]
pub struct ConsumeIntent<'a> {
    /// The operation about to execute.
    pub operation: &'a OperationDescriptor,
    /// The revision the command carries (`CommandMeta::expected_revision`).
    pub expected_revision: Option<Revision>,
    /// The capability the winning rule demands.
    pub required_capability: Capability,
    /// The actor executing the command.
    pub consumer: &'a Actor,
    /// Engine clock reading.
    pub now: UtcMillis,
}

/// In-memory ledger of approval records keyed by id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalLedger {
    records: BTreeMap<ApprovalId, ApprovalRecord>,
}

impl ApprovalLedger {
    /// Empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a pending request. Refuses duplicates, invalid expiry and
    /// operator-only capabilities requested by a non-operator.
    pub fn request(&mut self, request: ApprovalRequest) -> Result<&ApprovalRecord, ApprovalError> {
        request.validate()?;
        if self.records.contains_key(&request.id) {
            return Err(ApprovalError::AlreadyExists);
        }
        let id = request.id.clone();
        self.records
            .insert(id.clone(), ApprovalRecord::pending(request));
        self.records.get(&id).ok_or(ApprovalError::NotFound)
    }

    /// Operator approves a pending request; computes and stores the binding MAC.
    pub fn approve(
        &mut self,
        id: &ApprovalId,
        decided_by: &Actor,
        now: UtcMillis,
        key: &BindingKey,
    ) -> Result<&ApprovalRecord, ApprovalError> {
        if !decided_by.is_operator() {
            return Err(ApprovalError::DeciderNotOperator);
        }
        let record = self.records.get_mut(id).ok_or(ApprovalError::NotFound)?;
        if record.state != ApprovalState::Pending {
            return Err(ApprovalError::InvalidTransition {
                from: record.state,
                event: ApprovalEvent::Approve,
            });
        }
        if record.expire_if_due(now) {
            return Err(ApprovalError::Expired);
        }
        let capability = record.request.required_capability;
        if capability.operator_only() && !record.request.requested_by.is_operator() {
            return Err(ApprovalError::OperatorOnlyCapability { capability });
        }
        let mac = binding::sign(
            key,
            &record.request.operation,
            id,
            record.request.expected_revision,
        )?;
        record.apply(ApprovalEvent::Approve)?;
        record.decided_by = Some(decided_by.id.clone());
        record.decided_at = Some(now);
        record.binding_mac = Some(mac);
        Ok(record)
    }

    /// Operator rejects a pending request.
    pub fn reject(
        &mut self,
        id: &ApprovalId,
        decided_by: &Actor,
        now: UtcMillis,
        note: impl Into<String>,
    ) -> Result<&ApprovalRecord, ApprovalError> {
        if !decided_by.is_operator() {
            return Err(ApprovalError::DeciderNotOperator);
        }
        let record = self.records.get_mut(id).ok_or(ApprovalError::NotFound)?;
        record.apply(ApprovalEvent::Reject)?;
        record.decided_by = Some(decided_by.id.clone());
        record.decided_at = Some(now);
        record.decision_note = Some(note.into());
        Ok(record)
    }

    /// Operator withdraws an approval that has not been consumed.
    pub fn revoke(
        &mut self,
        id: &ApprovalId,
        revoked_by: &Actor,
        now: UtcMillis,
        note: impl Into<String>,
    ) -> Result<&ApprovalRecord, ApprovalError> {
        if !revoked_by.is_operator() {
            return Err(ApprovalError::DeciderNotOperator);
        }
        let record = self.records.get_mut(id).ok_or(ApprovalError::NotFound)?;
        record.apply(ApprovalEvent::Revoke)?;
        record.revoked_by = Some(revoked_by.id.clone());
        record.revoked_at = Some(now);
        record.decision_note = Some(note.into());
        Ok(record)
    }

    /// Move every pending or approved record whose window has passed to
    /// `Expired`; returns the ids that changed.
    pub fn expire_due(&mut self, now: UtcMillis) -> Vec<ApprovalId> {
        let mut expired = Vec::new();
        for (id, record) in &mut self.records {
            if record.expire_if_due(now) {
                expired.push(id.clone());
            }
        }
        expired
    }

    /// Verify every binding and spend the approval in one step.
    ///
    /// Check order (first failure wins): `NotFound`, `AlreadyConsumed`,
    /// `NotApproved`, `Expired`, `RequesterMismatch`, `OperatorOnlyCapability`,
    /// `CapabilityMismatch`, `ResourceMismatch`, `OperationMismatch`,
    /// `RevisionMismatch`, `MacInvalid`. Hash and id comparisons are
    /// constant-time. On success the record is `Consumed` and cannot be spent again.
    pub fn verify_and_consume(
        &mut self,
        id: &ApprovalId,
        intent: &ConsumeIntent<'_>,
        key: &BindingKey,
    ) -> Result<ConsumedApproval, ApprovalError> {
        let record = self.records.get_mut(id).ok_or(ApprovalError::NotFound)?;
        match record.state {
            ApprovalState::Consumed => return Err(ApprovalError::AlreadyConsumed),
            ApprovalState::Approved => {}
            other => return Err(ApprovalError::NotApproved { state: other }),
        }
        if record.expire_if_due(intent.now) {
            return Err(ApprovalError::Expired);
        }
        let request = &record.request;
        if intent.consumer.id != request.requested_by.id {
            return Err(ApprovalError::RequesterMismatch);
        }
        if intent.required_capability.operator_only() && !intent.consumer.is_operator() {
            return Err(ApprovalError::OperatorOnlyCapability {
                capability: intent.required_capability,
            });
        }
        if request.required_capability != intent.required_capability {
            return Err(ApprovalError::CapabilityMismatch {
                required: intent.required_capability,
                granted: request.required_capability,
            });
        }
        let bound = &request.operation.resource;
        let given = &intent.operation.resource;
        if bound.kind != given.kind || !ct_str_eq(&bound.id, &given.id) {
            return Err(ApprovalError::ResourceMismatch);
        }
        if !operations_equal(&request.operation, intent.operation) {
            return Err(ApprovalError::OperationMismatch);
        }
        if request.expected_revision != intent.expected_revision {
            return Err(ApprovalError::RevisionMismatch {
                expected: intent.expected_revision,
                bound: request.expected_revision,
            });
        }
        let mac = record
            .binding_mac
            .as_deref()
            .ok_or(ApprovalError::MacInvalid)?;
        if !binding::verify(key, intent.operation, id, intent.expected_revision, mac) {
            return Err(ApprovalError::MacInvalid);
        }
        let (decided_by, decided_at) = match (&record.decided_by, record.decided_at) {
            (Some(by), Some(at)) => (by.clone(), at),
            // An approved record always carries its decision; treat a snapshot
            // that lost it as unverifiable rather than guessing.
            _ => return Err(ApprovalError::MacInvalid),
        };
        record.apply(ApprovalEvent::Consume)?;
        record.consumed_by = Some(intent.consumer.id.clone());
        record.consumed_at = Some(intent.now);
        Ok(ConsumedApproval {
            approval_id: id.clone(),
            operation: intent.operation.clone(),
            granted_capability: record.request.required_capability,
            expected_revision: record.request.expected_revision,
            requested_by: record.request.requested_by.id.clone(),
            decided_by,
            decided_at,
            consumed_by: intent.consumer.id.clone(),
            consumed_at: intent.now,
        })
    }

    /// Look up a record.
    pub fn get(&self, id: &ApprovalId) -> Option<&ApprovalRecord> {
        self.records.get(id)
    }

    /// All records in id order.
    pub fn records(&self) -> impl Iterator<Item = &ApprovalRecord> {
        self.records.values()
    }

    /// Number of records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the ledger holds no records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// JSON snapshot (no key material).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Restore from a JSON snapshot.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Constant-time equality of two operation descriptors via their canonical
/// JSON hash plus a direct constant-time compare of the parameters hash.
fn operations_equal(a: &OperationDescriptor, b: &OperationDescriptor) -> bool {
    let (Ok(va), Ok(vb)) = (serde_json::to_value(a), serde_json::to_value(b)) else {
        return false;
    };
    let hash_equal = ct_str_eq(&sha256_json(&va), &sha256_json(&vb));
    let params_equal = ct_str_eq(&a.parameters_sha256, &b.parameters_sha256);
    hash_equal & params_equal
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::{
        Action, DataDomain, InteractionMode, ResourceKind, ResourceRef, Scope, SkillId,
    };

    fn key() -> BindingKey {
        BindingKey::from_bytes([7u8; 32])
    }

    fn operator() -> Actor {
        Actor::operator(ActorId::parse("op").unwrap())
    }

    fn skill() -> Actor {
        Actor::skill(
            ActorId::parse("skill-1").unwrap(),
            SkillId::parse("g9-session-director").unwrap(),
        )
    }

    fn op(action: Action, kind: ResourceKind, id: &str, params: &str) -> OperationDescriptor {
        OperationDescriptor {
            action,
            resource: ResourceRef::new(kind, id),
            scope: Scope {
                domain: DataDomain::Learning,
                mode: InteractionMode::Review,
            },
            parameters_sha256: params.repeat(32),
        }
    }

    fn request(
        id: &str,
        actor: Actor,
        cap: Capability,
        operation: OperationDescriptor,
    ) -> ApprovalRequest {
        ApprovalRequest::new(
            ApprovalId::parse(id).unwrap(),
            operation,
            actor,
            UtcMillis::from_millis(1_000),
            Some(Revision::new(3)),
            cap,
            "test",
        )
        .unwrap()
    }

    fn approved_ledger() -> (ApprovalLedger, ApprovalId, OperationDescriptor) {
        let operation = op(Action::Finalize, ResourceKind::GradeDecision, "g1", "aa");
        let req = request(
            "apr-1",
            operator(),
            Capability::GradeFinalize,
            operation.clone(),
        );
        let id = req.id.clone();
        let mut ledger = ApprovalLedger::new();
        ledger.request(req).unwrap();
        ledger
            .approve(&id, &operator(), UtcMillis::from_millis(2_000), &key())
            .unwrap();
        (ledger, id, operation)
    }

    fn intent<'a>(
        operation: &'a OperationDescriptor,
        consumer: &'a Actor,
        now: i64,
    ) -> ConsumeIntent<'a> {
        ConsumeIntent {
            operation,
            expected_revision: Some(Revision::new(3)),
            required_capability: Capability::GradeFinalize,
            consumer,
            now: UtcMillis::from_millis(now),
        }
    }

    #[test]
    fn default_ttl_is_fifteen_minutes() {
        let req = request(
            "apr-1",
            operator(),
            Capability::DataExport,
            op(Action::Export, ResourceKind::Note, "n", "aa"),
        );
        assert_eq!(req.expires_at, UtcMillis::from_millis(1_000 + 15 * 60_000));
        assert!(req.is_expired_at(req.expires_at));
        assert!(!req.is_expired_at(UtcMillis::from_millis(req.expires_at.as_millis() - 1)));
        assert_eq!(
            req.clone()
                .with_expires_at(UtcMillis::from_millis(1_000))
                .unwrap_err(),
            ApprovalError::InvalidExpiry
        );
        assert_eq!(
            req.with_ttl(DurationMs::from_seconds_u32(30))
                .unwrap()
                .expires_at,
            UtcMillis::from_millis(31_000)
        );
    }

    #[test]
    fn happy_path_request_approve_consume() {
        let (mut ledger, id, operation) = approved_ledger();
        let rec = ledger.get(&id).unwrap();
        assert_eq!(rec.state, ApprovalState::Approved);
        assert_eq!(rec.binding_mac.as_ref().map(String::len), Some(64));
        let consumer = operator();
        let consumed = ledger
            .verify_and_consume(&id, &intent(&operation, &consumer, 3_000), &key())
            .unwrap();
        assert_eq!(consumed.granted_capability, Capability::GradeFinalize);
        assert_eq!(consumed.consumed_at, UtcMillis::from_millis(3_000));
        assert_eq!(consumed.decided_at, UtcMillis::from_millis(2_000));
        let rec = ledger.get(&id).unwrap();
        assert_eq!(rec.state, ApprovalState::Consumed);
        assert_eq!(rec.consumed_by, Some(consumer.id.clone()));
    }

    #[test]
    fn second_consumption_is_already_consumed() {
        let (mut ledger, id, operation) = approved_ledger();
        let consumer = operator();
        ledger
            .verify_and_consume(&id, &intent(&operation, &consumer, 3_000), &key())
            .unwrap();
        assert_eq!(
            ledger.verify_and_consume(&id, &intent(&operation, &consumer, 3_001), &key()),
            Err(ApprovalError::AlreadyConsumed)
        );
    }

    #[test]
    fn pending_rejected_revoked_are_not_approved() {
        let operation = op(Action::Finalize, ResourceKind::GradeDecision, "g1", "aa");
        let consumer = operator();
        let mut ledger = ApprovalLedger::new();
        ledger
            .request(request(
                "p",
                operator(),
                Capability::GradeFinalize,
                operation.clone(),
            ))
            .unwrap();
        let p = ApprovalId::parse("p").unwrap();
        assert_eq!(
            ledger.verify_and_consume(&p, &intent(&operation, &consumer, 1_500), &key()),
            Err(ApprovalError::NotApproved {
                state: ApprovalState::Pending
            })
        );
        ledger
            .reject(&p, &operator(), UtcMillis::from_millis(1_600), "no")
            .unwrap();
        assert_eq!(
            ledger.verify_and_consume(&p, &intent(&operation, &consumer, 1_700), &key()),
            Err(ApprovalError::NotApproved {
                state: ApprovalState::Rejected
            })
        );
        assert_eq!(
            ledger
                .approve(&p, &operator(), UtcMillis::from_millis(1_800), &key())
                .unwrap_err(),
            ApprovalError::InvalidTransition {
                from: ApprovalState::Rejected,
                event: ApprovalEvent::Approve
            }
        );
        let (mut ledger, id, operation) = approved_ledger();
        ledger
            .revoke(
                &id,
                &operator(),
                UtcMillis::from_millis(2_500),
                "changed my mind",
            )
            .unwrap();
        assert_eq!(
            ledger.verify_and_consume(&id, &intent(&operation, &consumer, 2_600), &key()),
            Err(ApprovalError::NotApproved {
                state: ApprovalState::Revoked
            })
        );
    }

    #[test]
    fn only_operator_decides() {
        let operation = op(Action::Finalize, ResourceKind::GradeDecision, "g1", "aa");
        let mut ledger = ApprovalLedger::new();
        ledger
            .request(request(
                "p",
                operator(),
                Capability::GradeFinalize,
                operation,
            ))
            .unwrap();
        let p = ApprovalId::parse("p").unwrap();
        let model = Actor::model(ActorId::parse("model").unwrap());
        assert_eq!(
            ledger
                .approve(&p, &model, UtcMillis::from_millis(1_500), &key())
                .unwrap_err(),
            ApprovalError::DeciderNotOperator
        );
        assert_eq!(
            ledger
                .reject(&p, &skill(), UtcMillis::from_millis(1_500), "x")
                .unwrap_err(),
            ApprovalError::DeciderNotOperator
        );
        assert_eq!(ledger.get(&p).unwrap().state, ApprovalState::Pending);
    }

    #[test]
    fn skill_cannot_request_operator_only_capability() {
        let operation = op(Action::Execute, ResourceKind::Process, "proc", "aa");
        let err = ApprovalRequest::new(
            ApprovalId::parse("x").unwrap(),
            operation,
            skill(),
            UtcMillis::from_millis(1),
            None,
            Capability::ShellExec,
            "evil",
        )
        .unwrap_err();
        assert_eq!(
            err,
            ApprovalError::OperatorOnlyCapability {
                capability: Capability::ShellExec
            }
        );
        // A request smuggled in through a snapshot is still refused at approval time.
        let smuggled = ApprovalRequest {
            id: ApprovalId::parse("x").unwrap(),
            operation: op(Action::Connect, ResourceKind::NetworkHost, "h", "aa"),
            requested_by: skill(),
            requested_at: UtcMillis::from_millis(1),
            expires_at: UtcMillis::from_millis(1_000_000),
            expected_revision: None,
            required_capability: Capability::NetworkEgress,
            reason: "evil".into(),
        };
        let mut ledger = ApprovalLedger::new();
        assert!(matches!(
            ledger.request(smuggled.clone()),
            Err(ApprovalError::OperatorOnlyCapability { .. })
        ));
        let json = serde_json::to_string(&ApprovalLedger {
            records: [(
                smuggled.id.clone(),
                ApprovalRecord::pending(smuggled.clone()),
            )]
            .into_iter()
            .collect(),
        })
        .unwrap();
        let mut ledger = ApprovalLedger::from_json(&json).unwrap();
        assert_eq!(
            ledger
                .approve(&smuggled.id, &operator(), UtcMillis::from_millis(2), &key())
                .unwrap_err(),
            ApprovalError::OperatorOnlyCapability {
                capability: Capability::NetworkEgress
            }
        );
    }

    #[test]
    fn duplicate_ids_are_refused() {
        let operation = op(Action::Finalize, ResourceKind::GradeDecision, "g1", "aa");
        let mut ledger = ApprovalLedger::new();
        ledger
            .request(request(
                "p",
                operator(),
                Capability::GradeFinalize,
                operation.clone(),
            ))
            .unwrap();
        assert_eq!(
            ledger
                .request(request(
                    "p",
                    operator(),
                    Capability::GradeFinalize,
                    operation
                ))
                .unwrap_err(),
            ApprovalError::AlreadyExists
        );
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn expire_due_sweeps_pending_and_approved() {
        let operation = op(Action::Finalize, ResourceKind::GradeDecision, "g1", "aa");
        let mut ledger = ApprovalLedger::new();
        ledger
            .request(request(
                "pending",
                operator(),
                Capability::GradeFinalize,
                operation.clone(),
            ))
            .unwrap();
        ledger
            .request(request(
                "approved",
                operator(),
                Capability::GradeFinalize,
                operation.clone(),
            ))
            .unwrap();
        ledger
            .request(
                request("fresh", operator(), Capability::GradeFinalize, operation)
                    .with_expires_at(UtcMillis::from_millis(10_000_000))
                    .unwrap(),
            )
            .unwrap();
        let approved = ApprovalId::parse("approved").unwrap();
        ledger
            .approve(
                &approved,
                &operator(),
                UtcMillis::from_millis(2_000),
                &key(),
            )
            .unwrap();
        let expired = ledger.expire_due(UtcMillis::from_millis(1_000 + 15 * 60_000));
        assert_eq!(expired.len(), 2);
        assert_eq!(ledger.get(&approved).unwrap().state, ApprovalState::Expired);
        assert_eq!(
            ledger
                .get(&ApprovalId::parse("pending").unwrap())
                .unwrap()
                .state,
            ApprovalState::Expired
        );
        assert_eq!(
            ledger
                .get(&ApprovalId::parse("fresh").unwrap())
                .unwrap()
                .state,
            ApprovalState::Pending
        );
        // Sweeping again changes nothing.
        assert!(ledger
            .expire_due(UtcMillis::from_millis(1_000 + 15 * 60_000))
            .is_empty());
    }

    #[test]
    fn snapshot_round_trip_preserves_state_and_mac() {
        let (ledger, id, _) = approved_ledger();
        let json = ledger.to_json().unwrap();
        assert!(!json.contains("BindingKey"));
        let back = ApprovalLedger::from_json(&json).unwrap();
        assert_eq!(back, ledger);
        assert_eq!(
            back.get(&id).unwrap().binding_mac,
            ledger.get(&id).unwrap().binding_mac
        );
    }

    #[test]
    fn missing_decision_metadata_in_snapshot_is_unverifiable() {
        let (ledger, id, operation) = approved_ledger();
        let mut json: serde_json::Value = serde_json::from_str(&ledger.to_json().unwrap()).unwrap();
        json["records"][id.as_str()]
            .as_object_mut()
            .unwrap()
            .remove("decided_by");
        let mut tampered = ApprovalLedger::from_json(&json.to_string()).unwrap();
        let consumer = operator();
        assert_eq!(
            tampered.verify_and_consume(&id, &intent(&operation, &consumer, 3_000), &key()),
            Err(ApprovalError::MacInvalid)
        );
    }

    #[test]
    fn error_codes_are_stable_and_prefixed() {
        let all = [
            ApprovalError::NotFound,
            ApprovalError::NotApproved {
                state: ApprovalState::Pending,
            },
            ApprovalError::Expired,
            ApprovalError::OperationMismatch,
            ApprovalError::ResourceMismatch,
            ApprovalError::RevisionMismatch {
                expected: None,
                bound: None,
            },
            ApprovalError::AlreadyConsumed,
            ApprovalError::MacInvalid,
            ApprovalError::CapabilityMismatch {
                required: Capability::DataExport,
                granted: Capability::DataDelete,
            },
            ApprovalError::RequesterMismatch,
            ApprovalError::DeciderNotOperator,
            ApprovalError::OperatorOnlyCapability {
                capability: Capability::ShellExec,
            },
            ApprovalError::AlreadyExists,
            ApprovalError::InvalidTransition {
                from: ApprovalState::Pending,
                event: ApprovalEvent::Consume,
            },
            ApprovalError::InvalidExpiry,
            ApprovalError::Binding(BindingError::KeyLength),
        ];
        let codes: std::collections::BTreeSet<&str> = all.iter().map(ApprovalError::code).collect();
        assert_eq!(codes.len(), all.len(), "codes are unique");
        assert!(codes.iter().all(|c| c.starts_with("approval.")));
    }
}
