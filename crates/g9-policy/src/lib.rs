#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! # g9-policy — M16 Authorization
//!
//! Independent, deny-by-default authorization engine (ARCHITECTURE.md §5 steps
//! 3–4 and §11; handoff §12 "Operating approvals" and the "Policy bypass/TOCTOU"
//! row of the threat-control matrix). Pure: no I/O, no randomness, no wall clock
//! unless the caller injects one ([`engine::WallClock`]).
//!
//! | Module | Contract |
//! |---|---|
//! | [`rules`] | [`PolicyRule`] data table, [`RuleSet::builtin_v1`], structural validation, deterministic selection |
//! | [`engine`] | [`Engine::evaluate`] → [`Evaluation`]; hard invariants no table can override; [`Engine::authorize_with_approval`] (§5 step 3–4) |
//! | [`approvals`] | [`ApprovalRequest`] / [`ApprovalRecord`] / [`ApprovalLedger`]: request → approve → single-use `verify_and_consume` |
//! | [`binding`] | `binding_mac = HMAC-SHA256(install_secret, canonical_json(operation) ‖ expected_revision ‖ approval_id)`, constant-time verify |
//! | [`record`] | Metadata-only [`PolicyDecisionRecord`] persisted by `g9-app` as `policy_decisions` |
//! | [`error`] | [`PolicyError`] → `G9Error` (Unauthorized / ApprovalRequired / StaleRevision / PreconditionFailed / …) |
//!
//! ## Authority model
//!
//! * Default decision is `Deny`; a rule must positively match.
//! * Skill and model actors are proposal-only: they may `Read` non-secret
//!   records inside their scope domain and `Propose` on proposal resources.
//!   They never receive a write verb, a native host action, or an
//!   [`Capability::operator_only`](g9_domain::Capability::operator_only)
//!   capability — not from any rule table, not with any approval.
//! * `Execute`, `Connect`, `Load`, host-path writes, policy edits, finalization,
//!   promotion, import, deletion and export require a fresh per-operation
//!   approval even for the operator.
//! * Approvals bind to the exact operation hash, resource, scope and expected
//!   revision; they expire (15 min default), are single-use, and every refused
//!   consumption yields a `DENY` record with an `approval.*` reason.
//! * Expiry fails closed (SEC-02): a wall clock earlier than the request or
//!   decision instant expires the approval, and so does consumption under a
//!   [`RuntimeEpoch`] (boot id + process-start id) other than the one the
//!   request was issued in, when the caller supplies one.
//! * Finalized records are immutable for everyone; corrections supersede.
//!
//! ## What this crate does not do
//!
//! It does not persist anything, does not read the OS keyring or CSPRNG (the
//! [`BindingKey`] bytes are supplied by `g9-app`), does not read the boot id or
//! mint a process-start id (the [`RuntimeEpoch`] is supplied by `g9-app`), and
//! does not decide *which* resource context applies — `g9-app` reads the
//! stored row and supplies a [`ResourceContext`].

pub mod approvals;
pub mod binding;
pub mod engine;
pub mod error;
pub mod record;
pub mod rules;

pub use approvals::{
    ApprovalError, ApprovalLedger, ApprovalRecord, ApprovalRequest, ConsumeIntent,
    ConsumedApproval, RuntimeEpoch, DEFAULT_APPROVAL_TTL,
};
pub use binding::{sign, verify, BindingError, BindingKey, BINDING_KEY_LEN, MAC_HEX_LEN};
pub use engine::{
    ApprovalGate, Authorized, Clock, Engine, Evaluation, FixedClock, SteppingClock, WallClock,
};
pub use error::PolicyError;
pub use record::{DecisionKind, PolicyDecisionRecord};
pub use rules::{
    ActorKindMatcher, Effect, PolicyRule, ResourceContext, RuleCondition, RuleSet, RuleSetError,
    BUILTIN_V1_VERSION, DEFAULT_DENY_REASON, DEFAULT_DENY_RULE_ID,
};
