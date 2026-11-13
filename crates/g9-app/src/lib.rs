#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
// `G9Error` is the closed IPC error contract (ADR-0013) and is returned by
// value through every handler; boxing it would change the bridge contract.
#![allow(clippy::result_large_err)]
//! # g9-app — application services and the typed IPC command pipeline
//!
//! The only crate that composes policy, storage, domain crates and skills
//! (ARCHITECTURE.md §2 rule 5). Every IPC operation enters through
//! [`dispatch::dispatch`] and every mutation passes [`pipeline::execute`]
//! (ARCHITECTURE.md §5, steps 1–9).
//!
//! | Module | Contract |
//! |---|---|
//! | [`ipc`] | wire `ClientCommandMeta` (no actor; ADR-0013) → domain `CommandMeta` with the backend-assigned operator |
//! | [`schemas`] | `schemas/ipc/*.json` embedded with `include_str!`, compiled once (`jsonschema` 0.60, no external refs) |
//! | [`ops`] | operation registry: schema version, mutating flag, policy verb/resource kind |
//! | [`dispatch`] | `dispatch(ctx, op, meta_json, payload_json)`: unknown op → `INVALID_INPUT`; payload schema check before deserialization |
//! | [`pipeline`] | idempotency → policy (+ `PolicyDecisionRecord`) → approval request / single-use consumption inside `BEGIN IMMEDIATE` → revision → mutation → events + hash-chained audit → commit + idempotency outcome |
//! | [`context`] | [`AppContext`]: database, policy engine, approval ledger, binding key, runtime, skills registry, clocks, operator |
//! | [`recovery`] | open-time: migrate only after a verified backup, audit-chain verification, open intervals → `UNCERTAIN` + `INTERRUPTED`, approval expiry |
//! | [`status`] | `system.status` (honest model/network/storage state) |
//! | [`commands`] | one module per domain |
//! | [`teaching`], [`analytics`], [`executive`], [`research`] | later-stage areas: PROPOSED, answer `PRECONDITION_FAILED <area>.not_implemented` |
//!
//! ## Authority
//!
//! The actor is assigned by the backend (never from IPC metadata). Model and
//! skill output is advisory data: `skills.invoke` returns recommended actions
//! and never executes them. Approval-gated operations (`curriculum.import`,
//! `curriculum.promote`, `calendar.apply`, `assessment.approve_final`,
//! `export.request`, `deletion.request`) are refused with `APPROVAL_REQUIRED`
//! and a persisted approval request until the operator approves and the
//! command is retried with that approval, which is consumed exactly once.
//!
//! ## Gaps in wave-1 APIs (worked around here, reported)
//!
//! * Backup manifests are read back from `backup_manifests`
//!   (`g9_storage::backup::read_manifest`/`list_manifests`): `backup.verify` and
//!   `backup.restore_test` work on backups created by an earlier process, and
//!   `system.status.last_backup` is the newest `RETAINED` manifest in the
//!   database. The operator label is not persisted (`label: null` after a
//!   restart).
//! * Restore into the live database is offline only: `g9 restore` (`apps/cli`,
//!   `g9_storage::restore::restore_over_live`) replaces the live file while the
//!   application is stopped and quarantines the old one. An in-app restore
//!   operation (a `DataDelete`-class approval bound to the backup id and the live
//!   revision) and restoring into a running application are PROPOSED; the
//!   `READ_ONLY` refusal says so (`details.live_restore`).
//! * Suspend detection (TEC-07) is a stop-time wall/monotonic comparison only;
//!   the heartbeat ledger needs a `g9-storage` table (PROPOSED).
//! * The `retention.due` v1.0 vocabulary has no `PROCEDURAL` kind (labelled
//!   `PRACTICAL_TRANSFER` with the warning `retention.due.procedural_label`).
//! * Idempotency records written before content-free replay records existed
//!   still hold their full outcome; erasure does not reach them (needs a
//!   `g9-storage` erasure/retention change).
//! * `g9_storage::audit` has no listing query: `audit.recent` lists rows
//!   appended by this process and verifies the whole chain.
//! * `g9_storage::deletion::execute_erasure` opens its own transaction: the
//!   erasure runs as a post-commit step of the approved command.
//! * `assessment_versions.difficulty` (nullable, since migration `0013`) is the
//!   author-declared difficulty: observations from a final grade are derived
//!   only for a version that declares one; otherwise `assessment.approve_final`
//!   warns `competency.observations_not_derived` and derives nothing. Every
//!   operator finalization is `SELF_ADJUDICATED` (the operator is the learner),
//!   so no derived observation validates in the MVP; an external reviewer or a
//!   deterministic checker is PROPOSED.
//! * `g9_policy::ApprovalLedger` has no record-level restore: the ledger is
//!   re-synchronised through its JSON snapshot form.
//! * Attachments cannot be quarantined (no quarantine columns): a file whose
//!   bytes do not match its declared type is refused, not held for review
//!   (PROPOSED). Backup verification checks that attachment files exist; hashing
//!   them during verification is PROPOSED (copies are hash-checked).
//! * `assessment.define` stores the baseline rubric and the storable kinds
//!   (blind practical, capstone, transfer check); custom criteria and the quiz
//!   and report kinds are PROPOSED.

pub mod analytics;
pub mod commands;
pub mod context;
pub mod dispatch;
pub mod executive;
pub mod ipc;
pub mod keyfile;
pub mod lock;
pub mod ops;
pub mod pipeline;
pub mod recovery;
pub mod research;
pub mod schemas;
pub mod status;
pub mod teaching;

// Unit tests that need a finalization by a reviewer who is not the learner (not
// reachable through `dispatch`: the operator is both) reuse the integration-test
// harness; `extern crate self` lets that file name this crate as `g9_app`.
#[cfg(test)]
extern crate self as g9_app;
#[cfg(test)]
#[path = "../tests/common/mod.rs"]
mod test_harness;

pub use context::{AppConfig, AppContext, AppContextBuilder, Transport};
pub use dispatch::{approval_summary, dispatch, dispatch_confirming, ApprovalSummary};
pub use g9_domain::{CommandOutcome, ErrorCategory, G9Error};
pub use recovery::RecoveryReport;

/// Storage options for the application's own database. `synchronous = FULL`
/// (owner decision 2026-11-13, TEC-12): every commit is fsynced before it is
/// acknowledged, so a power cut loses no committed command. The desktop bridge
/// and the CLI both open the database through this, so the choice lives in one
/// place (ARCHITECTURE.md §6 still says NORMAL: a reported deviation, ADR-0015).
pub fn database_options(app_version: impl Into<String>) -> g9_storage::OpenOptions {
    g9_storage::OpenOptions::new(app_version).synchronous(g9_storage::Synchronous::Full)
}

/// The explicit answer of a later-stage area (`<area>.not_implemented`).
pub fn not_implemented(op: &str) -> G9Error {
    let area = op.split('.').next().unwrap_or("app");
    G9Error::precondition_failed(
        format!("{area}.not_implemented"),
        "this operation belongs to a later stage and is PROPOSED; nothing was executed",
    )
}
