//! The command pipeline (ARCHITECTURE.md §5). Every IPC operation runs through
//! [`execute`]; there is no other path to a mutation.
//!
//! Mutating commands, in order, inside one `BEGIN IMMEDIATE` transaction:
//!
//! 1. **Schema version** — `payload_schema_version` must be accepted by the
//!    operation's [`SchemaVersion`] (the payload itself was validated against its
//!    JSON Schema by `dispatch` before deserialization) → `INVALID_INPUT`.
//! 2. **Idempotency** — `(actor_id, idempotency_key)`: same request hash →
//!    stored outcome with `replayed = true`; different hash → `CONFLICT`.
//! 3. **Policy** — the handler's `facts` closure reads the stored resource
//!    (domain, classification, finalized, revision) inside the transaction;
//!    [`g9_policy::Engine`] evaluates `OperationDescriptor { action, resource,
//!    scope, parameters_sha256 }`. `Deny` → `UNAUTHORIZED`; `ApprovalRequired`
//!    without an approval → a persisted `ApprovalRequest` and
//!    `APPROVAL_REQUIRED` carrying its id.
//! 4. **Approval consumption** — a presented approval is re-read from
//!    `approval_requests` inside the transaction and passed through
//!    `Engine::authorize_with_approval` → `ApprovalLedger::verify_and_consume`
//!    (state, expiry, requester, capability, resource, operation hash, bound
//!    revision, constant-time HMAC); the row moves to `CONSUMED` in the same
//!    transaction (single use, TOCTOU-safe).
//! 5. **Revision** — `expected_revision` against the stored revision → `STALE_REVISION`.
//! 6. **Mutation** — the handler (pure domain transition + repository writes).
//! 7. **Events + audit** — significant `domain_events`; one hash-chained
//!    `audit_metadata` row (metadata only); the `policy_decisions` row.
//! 8. **Commit** — the idempotency outcome is stored inside the same
//!    transaction, so a crash between commit and reply is replayable. A
//!    result that carries personal or model content (note title/body, skill
//!    output) is never stored: the handler supplies a content-free
//!    [`Mutation::replay_record`] (ids only) and a replay rebuilds the result
//!    from current storage (`commands::rebuild_replay`), so an approved
//!    erasure (A10, ADR-0002) leaves no copy in `idempotency_records`.
//! 9. **Return** `CommandOutcome { result, revision, audit_ref, replayed }`.
//!
//! In-memory service state that mirrors a row the handler wrote (the backup
//! map) is changed only by [`Mutation::on_commit`], after the commit succeeded.
//!
//! Any failure after step 2 rolls the transaction back (nothing the handler
//! wrote survives, an approval is not spent) and a second, small transaction
//! records what must survive a refusal: the policy decision row, the approval
//! request (step 3), an expired approval's terminal state, and an audit row
//! whose outcome is the error category. Read-only queries run step 3 only;
//! every read evaluation (allowed or refused) is persisted as a
//! metadata-only `policy_decisions` row (ARCHITECTURE.md §11, ADR-0003).
//!
//! Approval binding (ADR-0003, SEC-02): an approval request is bound to the
//! revision the resource has when it is requested — the stored revision read in
//! step 3, never a value the client chooses — and a request whose
//! `expected_revision` differs from it is refused STALE_REVISION before any
//! approval exists. Consumption binds by the same rule, so a resource changed
//! between grant and use (e.g. a regrade after the approval) refuses the stale
//! approval whatever revision the retry claims. (Binding the client's value let
//! an approval be requested "for revision r+2" of a failing attempt and consumed
//! after a regrade the operator never saw: security audit 2026-11-12.) For a
//! resource that does not exist yet, the client's `expected_revision` (if any)
//! is bound.

use crate::context::{AppContext, Services};
use crate::ops::OpSpec;
use crate::schemas;
use g9_domain::hashing::sha256_json;
use g9_domain::{
    Actor, ApprovalId, Classification, CommandMeta, CommandOutcome, DataDomain, Decision,
    EventHeader, EventId, G9Error, OperationDescriptor, PolicyDecisionId, ResourceRef, Revision,
    SchemaVersion, UtcMillis,
};
use g9_policy::{
    ApprovalError, ApprovalGate, ConsumedApproval, DecisionKind, PolicyDecisionRecord, PolicyError,
    ResourceContext,
};
use g9_storage::approvals_store::{self, ApprovalUpdate, NewApprovalRequest, PolicyDecisionRow};
use g9_storage::audit::{AuditChain, AuditDecision, AuditEntry, AuditOutcome};
use g9_storage::events::{self, DomainEvent};
use g9_storage::idempotency::{self, IdempotencyCheck};
use g9_storage::Database;
use rusqlite::Connection;
use serde_json::{json, Value};

/// The command as the pipeline sees it.
#[derive(Debug, Clone, Copy)]
pub struct Command<'a> {
    /// Operation registry entry.
    pub spec: &'static OpSpec,
    /// The protected resource, derived by the backend from the payload.
    pub resource: &'a ResourceRef,
    /// The validated request payload (hashed for idempotency and approval binding).
    pub payload: &'a Value,
}

/// Stored facts about the targeted resource, read inside the transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Facts {
    /// Policy context (domain, classification, finalized).
    pub context: ResourceContext,
    /// Stored revision, `None` when the command creates the resource.
    pub current_revision: Option<Revision>,
}

impl Facts {
    /// Facts for an existing record.
    pub const fn existing(context: ResourceContext, revision: Revision) -> Self {
        Self {
            context,
            current_revision: Some(revision),
        }
    }

    /// Facts for a record about to be created.
    pub const fn new_record(context: ResourceContext) -> Self {
        Self {
            context,
            current_revision: None,
        }
    }
}

/// A significant-fact event to append with the command.
#[derive(Debug, Clone, PartialEq)]
pub struct EventSpec {
    /// Event kind, e.g. `session.started`.
    pub kind: String,
    /// Structured facts and ids (never raw content).
    pub payload: Value,
    /// Privacy domain of the payload.
    pub domain: DataDomain,
    /// Classification of the payload.
    pub classification: Classification,
    /// Hash of immutable content the event refers to, when any.
    pub content_sha256: Option<String>,
}

impl EventSpec {
    /// A learning-domain, internal-classification event.
    pub fn learning(kind: impl Into<String>, payload: Value) -> Self {
        Self {
            kind: kind.into(),
            payload,
            domain: DataDomain::Learning,
            classification: Classification::Internal,
            content_sha256: None,
        }
    }

    /// A system-domain, internal-classification event.
    pub fn system(kind: impl Into<String>, payload: Value) -> Self {
        Self {
            domain: DataDomain::System,
            ..Self::learning(kind, payload)
        }
    }
}

/// Work to run after the main transaction committed (operations whose storage
/// primitive opens its own transaction or reads the live file through the
/// Online Backup API). Its result replaces the handler result.
pub type PostCommit = Box<dyn FnOnce(&mut PostEnv<'_>) -> Result<Mutation, G9Error>>;

/// In-memory service update applied only after the command transaction
/// committed (never after a rollback).
pub type OnCommit = Box<dyn FnOnce(&mut Services)>;

/// Key of the content-free record stored in `idempotency_records` in place of
/// a content-bearing result (see [`Mutation::replay_record`]).
pub const REPLAY_RECORD_KEY: &str = "g9_replay_record";

/// What a handler produced.
pub struct Mutation {
    /// Operation result (validated against the response schema before commit).
    pub result: Value,
    /// New revision of the primary resource.
    pub revision: Option<Revision>,
    /// Resource to record in the audit row when it differs from the command
    /// resource (e.g. the id of a newly created record).
    pub audit_resource: Option<ResourceRef>,
    /// Significant events.
    pub events: Vec<EventSpec>,
    /// Secret-free warnings returned with the outcome.
    pub warnings: Vec<String>,
    /// Follow-up step after commit.
    pub post_commit: Option<PostCommit>,
    /// Commit the transaction (metadata that must survive, e.g. a skill
    /// invocation record) but report this error to the caller. The audit row
    /// carries the error category and no idempotency outcome is stored.
    pub commit_error: Option<G9Error>,
    /// Content-free replacement for `result` in the idempotency record (ids
    /// only). Set by handlers whose result carries personal or model content;
    /// a replay rebuilds the result from current storage.
    pub replay_record: Option<Value>,
    /// In-memory service update applied after a successful commit.
    pub on_commit: Option<OnCommit>,
}

impl std::fmt::Debug for Mutation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mutation")
            .field("revision", &self.revision)
            .field("audit_resource", &self.audit_resource)
            .field("events", &self.events.len())
            .field("warnings", &self.warnings)
            .field("post_commit", &self.post_commit.is_some())
            .field("replay_record", &self.replay_record.is_some())
            .field("on_commit", &self.on_commit.is_some())
            .finish_non_exhaustive()
    }
}

impl Mutation {
    /// A result with a revision and nothing else.
    pub fn new(result: Value, revision: Option<Revision>) -> Self {
        Self {
            result,
            revision,
            audit_resource: None,
            events: Vec::new(),
            warnings: Vec::new(),
            post_commit: None,
            commit_error: None,
            replay_record: None,
            on_commit: None,
        }
    }

    /// Commit, then report `error` (see [`Mutation::commit_error`]).
    pub fn failed(error: G9Error) -> Self {
        Self {
            commit_error: Some(error),
            ..Self::new(Value::Null, None)
        }
    }

    /// Add a significant event.
    #[must_use]
    pub fn with_event(mut self, event: EventSpec) -> Self {
        self.events.push(event);
        self
    }

    /// Record a different resource in the audit row.
    #[must_use]
    pub fn with_audit_resource(mut self, resource: ResourceRef) -> Self {
        self.audit_resource = Some(resource);
        self
    }

    /// Add a warning.
    #[must_use]
    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    /// Schedule a post-commit step.
    #[must_use]
    pub fn then(mut self, step: PostCommit) -> Self {
        self.post_commit = Some(step);
        self
    }

    /// Store `record` (content-free ids) instead of the result in the
    /// idempotency record; a replay rebuilds the result from storage.
    #[must_use]
    pub fn replay_with(mut self, record: Value) -> Self {
        self.replay_record = Some(record);
        self
    }

    /// Apply `update` to the services after the transaction committed.
    #[must_use]
    pub fn on_commit(mut self, update: OnCommit) -> Self {
        self.on_commit = Some(update);
        self
    }
}

/// The outcome as stored in `idempotency_records`: verbatim, or with the
/// result replaced by the handler's content-free replay record plus the
/// SHA-256 of the full result.
fn stored_outcome(
    outcome: &CommandOutcome<Value>,
    replay_record: Option<&Value>,
) -> Result<Value, G9Error> {
    let mut stored = outcome.clone();
    if let Some(record) = replay_record {
        stored.result = json!({
            REPLAY_RECORD_KEY: record,
            "result_sha256": sha256_json(&outcome.result),
        });
    }
    serde_json::to_value(&stored).map_err(|_| G9Error::internal_redacted("idempotency.serialize"))
}

/// What a handler sees: the transaction connection and the services.
pub struct Env<'a> {
    /// The command transaction (or the plain connection for reads).
    pub conn: &'a Connection,
    /// Every service except the database.
    pub svc: &'a mut Services,
    /// The command metadata (actor already backend-assigned).
    pub meta: &'a CommandMeta,
    /// Pipeline clock reading for this command.
    pub now: UtcMillis,
    /// The approval spent by this command, if any.
    pub consumed: Option<ConsumedApproval>,
}

impl std::fmt::Debug for Env<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Env")
            .field("meta", &self.meta.operation)
            .field("now", &self.now)
            .field("consumed", &self.consumed)
            .finish_non_exhaustive()
    }
}

impl Env<'_> {
    /// The backend-assigned actor of this command.
    pub fn actor(&self) -> &Actor {
        &self.meta.actor
    }
}

/// What a post-commit step sees.
pub struct PostEnv<'a> {
    /// The database (no transaction open).
    pub db: &'a mut Database,
    /// Services.
    pub svc: &'a mut Services,
    /// Command metadata.
    pub meta: &'a CommandMeta,
    /// Pipeline clock reading.
    pub now: UtcMillis,
}

impl std::fmt::Debug for PostEnv<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PostEnv")
            .field("meta", &self.meta.operation)
            .field("now", &self.now)
            .finish_non_exhaustive()
    }
}

/// Hash binding an approval to the exact operation parameters:
/// SHA-256 of canonical `{"operation": <name>, "payload": <payload>}`.
pub fn parameters_sha256(operation: &str, payload: &Value) -> String {
    sha256_json(&json!({ "operation": operation, "payload": payload }))
}

/// Idempotency request hash: the operation, payload and every meta field that
/// changes what the command does.
fn request_sha256(meta: &CommandMeta, cmd: &Command<'_>) -> String {
    sha256_json(&json!({
        "operation": cmd.spec.name,
        "payload": cmd.payload,
        // The client-supplied resource only: backend-generated ids (new
        // records) must not make a retry look like a different request.
        "resource": meta.resource,
        "scope": meta.scope,
        "expected_revision": meta.expected_revision,
        "approval_id": meta.approval_id,
    }))
}

/// §5 step 1 checks that do not need storage.
fn precheck(meta: &CommandMeta, cmd: &Command<'_>) -> Result<(), G9Error> {
    if meta.operation != cmd.spec.name {
        return Err(G9Error::invalid_input(
            "meta.operation_mismatch",
            "meta.operation does not match the invoked operation",
        ));
    }
    if !cmd.spec.schema_version.accepts(meta.payload_schema_version) {
        return Err(G9Error::invalid_input(
            "ipc.schema_version_rejected",
            "payload schema version is not accepted by this operation",
        )
        .with_details(json!({ "accepted": schema_label(cmd.spec.schema_version) })));
    }
    if let Some(given) = &meta.resource {
        if given != cmd.resource {
            return Err(G9Error::invalid_input(
                "meta.resource_mismatch",
                "meta.resource does not match the resource named by the payload",
            ));
        }
    }
    Ok(())
}

fn schema_label(v: SchemaVersion) -> String {
    format!("{}.{}", v.major, v.minor)
}

fn descriptor(meta: &CommandMeta, cmd: &Command<'_>) -> OperationDescriptor {
    OperationDescriptor {
        action: cmd.spec.action,
        resource: cmd.resource.clone(),
        scope: meta.scope,
        parameters_sha256: parameters_sha256(cmd.spec.name, cmd.payload),
    }
}

/// Run one command through the pipeline.
///
/// `facts` reads the stored resource (inside the transaction for mutating
/// commands); `handler` performs the mutation (or the read) after policy,
/// approval and revision checks passed.
pub fn execute<F, H>(
    ctx: &mut AppContext,
    meta: &CommandMeta,
    cmd: Command<'_>,
    facts: F,
    handler: H,
) -> Result<CommandOutcome<Value>, G9Error>
where
    F: FnOnce(&Connection, &Services) -> Result<Facts, G9Error>,
    H: FnOnce(&mut Env<'_>) -> Result<Mutation, G9Error>,
{
    precheck(meta, &cmd)?;
    if meta.actor != ctx.svc.operator {
        // The bridge always assigns the operator; anything else is a defect.
        return Err(G9Error::unauthorized(
            "meta.actor_not_backend_assigned",
            "command actor must be the backend-assigned operator",
        ));
    }
    if cmd.spec.mutating {
        if let Some(reason) = ctx.svc.recovery.write_block_reason() {
            // There is no in-app restore (restoring into the running database is
            // PROPOSED). An offline restore exists (`g9 restore`, which replaces
            // the database file while the application is closed); say exactly
            // that and no more.
            return Err(G9Error::data_corruption(
                "recovery.integrity_failed",
                "integrity verification failed at open; mutations are refused. In-app restore is not implemented in this release (PROPOSED); close the application and restore a verified backup offline with `g9 restore` (docs/runbooks/backup-restore.md)",
            )
            .with_details(json!({
                "reason": reason,
                "live_restore": "NOT_IMPLEMENTED",
                "offline_restore": "g9 restore",
            })));
        }
        execute_mutating(ctx, meta, cmd, facts, handler)
    } else {
        execute_read(ctx, meta, cmd, facts, handler)
    }
}

fn execute_read<F, H>(
    ctx: &mut AppContext,
    meta: &CommandMeta,
    cmd: Command<'_>,
    facts: F,
    handler: H,
) -> Result<CommandOutcome<Value>, G9Error>
where
    F: FnOnce(&Connection, &Services) -> Result<Facts, G9Error>,
    H: FnOnce(&mut Env<'_>) -> Result<Mutation, G9Error>,
{
    let AppContext { db, svc } = ctx;
    let now = svc.now();
    let found = facts(db.conn(), svc)?;
    let op = descriptor(meta, &cmd);
    let evaluation = svc.policy.evaluate(&meta.actor, &op, &found.context);
    match &evaluation.decision {
        Decision::Allow { .. } => {
            // ARCHITECTURE.md §11 / ADR-0003: every evaluation is persisted,
            // allowed reads included (metadata only, before the read runs).
            let tx = db.begin_immediate()?;
            insert_decision(&tx, &evaluation.record, meta, None)?;
            tx.commit().map_err(storage_err)?;
        }
        Decision::Deny { .. } | Decision::ApprovalRequired { .. } => {
            let error: G9Error = match evaluation.decision.clone() {
                Decision::ApprovalRequired {
                    required_capability,
                    ..
                } => PolicyError::ApprovalRequired {
                    record: Box::new(evaluation.record.clone()),
                    required_capability,
                }
                .into(),
                _ => PolicyError::Denied {
                    record: Box::new(evaluation.record.clone()),
                }
                .into(),
            };
            let tx = db.begin_immediate()?;
            insert_decision(&tx, &evaluation.record, meta, None)?;
            tx.commit().map_err(storage_err)?;
            return Err(error);
        }
    }
    let mut env = Env {
        conn: db.conn(),
        svc,
        meta,
        now,
        consumed: None,
    };
    let mutation = handler(&mut env)?;
    schemas::validate_response(cmd.spec.name, &mutation.result)?;
    let mut outcome = CommandOutcome::new(mutation.result);
    outcome.revision = mutation.revision;
    outcome.warnings = mutation.warnings;
    Ok(outcome)
}

/// Everything the failure transaction must record.
struct Refusal {
    error: G9Error,
    decision: AuditDecision,
    record: Option<PolicyDecisionRecord>,
    new_approval: Option<g9_policy::ApprovalRequest>,
    touched_approval: Option<ApprovalId>,
    expire_approvals: bool,
}

impl Refusal {
    fn not_evaluated(error: G9Error) -> Self {
        Self {
            error,
            decision: AuditDecision::NotEvaluated,
            record: None,
            new_approval: None,
            touched_approval: None,
            expire_approvals: false,
        }
    }

    fn after_allow(
        error: G9Error,
        record: PolicyDecisionRecord,
        approval: Option<ApprovalId>,
    ) -> Self {
        // The transaction that would have spent the approval is rolled back:
        // the decision row must not claim a consumption that did not persist.
        let record = match &approval {
            Some(id) => PolicyDecisionRecord {
                reason: format!(
                    "approval {id} verified; consumption rolled back because the command failed ({})",
                    error.code
                ),
                ..record
            },
            None => record,
        };
        Self {
            error,
            decision: AuditDecision::Allow,
            record: Some(record),
            new_approval: None,
            touched_approval: approval,
            expire_approvals: false,
        }
    }
}

fn execute_mutating<F, H>(
    ctx: &mut AppContext,
    meta: &CommandMeta,
    cmd: Command<'_>,
    facts: F,
    handler: H,
) -> Result<CommandOutcome<Value>, G9Error>
where
    F: FnOnce(&Connection, &Services) -> Result<Facts, G9Error>,
    H: FnOnce(&mut Env<'_>) -> Result<Mutation, G9Error>,
{
    let now = ctx.svc.now();
    let request_hash = request_sha256(meta, &cmd);
    let op = descriptor(meta, &cmd);
    let result = {
        let AppContext { db, svc } = &mut *ctx;
        run_main_transaction(db, svc, meta, &cmd, &op, &request_hash, now, facts, handler)
    };
    match result {
        Ok(Step::Replayed(outcome)) => Ok(outcome),
        Ok(Step::CommittedFailure(error)) => Err(*error),
        Ok(Step::Committed { outcome, post }) => match post {
            None => Ok(outcome),
            Some(boxed) => {
                let (step, base) = *boxed;
                run_post_commit(ctx, meta, &cmd, &request_hash, now, step, base)
            }
        },
        Err(refusal) => Err(record_refusal(ctx, meta, &cmd, now, refusal)),
    }
}

enum Step {
    Replayed(CommandOutcome<Value>),
    CommittedFailure(Box<G9Error>),
    Committed {
        outcome: CommandOutcome<Value>,
        post: Option<Box<(PostCommit, PostBase)>>,
    },
}

/// What the post-commit step's audit row needs from the main transaction.
struct PostBase {
    record: PolicyDecisionRecord,
    audit_resource: ResourceRef,
    /// Classification of the resource (the floor of every event's).
    floor: Classification,
}

#[allow(clippy::too_many_arguments)]
fn run_main_transaction<F, H>(
    db: &mut Database,
    svc: &mut Services,
    meta: &CommandMeta,
    cmd: &Command<'_>,
    op: &OperationDescriptor,
    request_hash: &str,
    now: UtcMillis,
    facts: F,
    handler: H,
) -> Result<Step, Refusal>
where
    F: FnOnce(&Connection, &Services) -> Result<Facts, G9Error>,
    H: FnOnce(&mut Env<'_>) -> Result<Mutation, G9Error>,
{
    let tx = db
        .begin_immediate()
        .map_err(|e| Refusal::not_evaluated(e.into()))?;

    // Step 2: idempotency.
    match idempotency::check(&tx, &meta.actor.id, &meta.idempotency_key, request_hash)
        .map_err(|e| Refusal::not_evaluated(e.into()))?
    {
        IdempotencyCheck::Replay(stored) => {
            let mut outcome: CommandOutcome<Value> = serde_json::from_value(stored.outcome_json)
                .map_err(|_| {
                    Refusal::not_evaluated(G9Error::data_corruption(
                        "idempotency.outcome_unreadable",
                        "the stored outcome for this idempotency key cannot be read",
                    ))
                })?;
            if let Some(record) = outcome.result.get(REPLAY_RECORD_KEY).cloned() {
                // Content-bearing result: rebuilt from current storage (an
                // erased note replays as erased); nothing is written.
                let (result, warnings) =
                    crate::commands::rebuild_replay(cmd.spec.name, &tx, svc, &record)
                        .map_err(Refusal::not_evaluated)?;
                schemas::validate_response(cmd.spec.name, &result)
                    .map_err(Refusal::not_evaluated)?;
                outcome.result = result;
                outcome.warnings.extend(warnings);
            }
            drop(tx);
            outcome.replayed = true;
            return Ok(Step::Replayed(outcome));
        }
        IdempotencyCheck::Conflict { .. } => {
            drop(tx);
            return Err(Refusal::not_evaluated(G9Error::conflict(
                "idempotency.key_reused",
                "idempotency key was already used for a different request",
            )));
        }
        IdempotencyCheck::Miss => {}
    }

    // Step 3 (facts read inside the transaction).
    let found = facts(&tx, svc).map_err(Refusal::not_evaluated)?;
    // The revision an approval binds to (ADR-0003, SEC-02): the stored one
    // whenever the resource exists, so the operator approves the state they can
    // see. Used identically when the approval is requested and when it is
    // consumed, so a change to the resource in between refuses the approval
    // (`approval.revision_mismatch`), whatever the client sends.
    let bound_revision = found.current_revision.or(meta.expected_revision);

    // Steps 3–4: policy and approval consumption.
    let (record, consumed) = match &meta.approval_id {
        None => {
            let evaluation = svc.policy.evaluate(&meta.actor, op, &found.context);
            match evaluation.decision.clone() {
                Decision::Allow { .. } => (evaluation.record, None),
                Decision::Deny { .. } => {
                    let error = PolicyError::Denied {
                        record: Box::new(evaluation.record.clone()),
                    }
                    .into();
                    return Err(Refusal {
                        error,
                        decision: AuditDecision::Deny,
                        record: Some(evaluation.record),
                        new_approval: None,
                        touched_approval: None,
                        expire_approvals: false,
                    });
                }
                Decision::ApprovalRequired {
                    required_capability,
                    ..
                } => {
                    // A request made from a stale (or invented) view of the
                    // resource gets no approval at all: STALE_REVISION, recorded
                    // with the decision that asked for an approval.
                    if let (Some(expected), Some(actual)) =
                        (meta.expected_revision, found.current_revision)
                    {
                        if expected != actual {
                            return Err(Refusal {
                                error: G9Error::stale_revision(Some(expected), actual),
                                decision: AuditDecision::ApprovalRequired,
                                record: Some(evaluation.record),
                                new_approval: None,
                                touched_approval: None,
                                expire_approvals: false,
                            });
                        }
                    }
                    let id = ApprovalId::new_v7();
                    let request = evaluation
                        .approval_request(id.clone(), &meta.actor, op, bound_revision)
                        .unwrap_or(Err(ApprovalError::InvalidExpiry))
                        .map_err(|e| Refusal::not_evaluated(e.into()))?;
                    let error: G9Error = PolicyError::ApprovalRequired {
                        record: Box::new(evaluation.record.clone()),
                        required_capability,
                    }
                    .into();
                    let error = error.with_approval(id.clone()).with_details(json!({
                        "required_capability": required_capability,
                        "parameters_sha256": op.parameters_sha256,
                        "expires_at": request.expires_at,
                        "rule_id": evaluation.record.rule_id,
                    }));
                    return Err(Refusal {
                        error,
                        decision: AuditDecision::ApprovalRequired,
                        record: Some(evaluation.record.with_approval_id(id)),
                        new_approval: Some(request),
                        touched_approval: None,
                        expire_approvals: false,
                    });
                }
            }
        }
        Some(approval_id) => {
            let operator = svc.operator.clone();
            svc.approvals
                .refresh(&tx, approval_id, &operator)
                .map_err(Refusal::not_evaluated)?;
            let stored = approvals_store::get_request(&tx, approval_id)
                .map_err(|e| Refusal::not_evaluated(e.into()))?;
            let gate = ApprovalGate {
                ledger: svc.approvals.ledger_mut(),
                key: &svc.binding_key,
            };
            match svc.policy.authorize_with_approval(
                &meta.actor,
                op,
                &found.context,
                Some(approval_id),
                bound_revision,
                gate,
            ) {
                Ok(authorized) => {
                    if let (Some(consumed), Some(row)) = (&authorized.consumed, &stored) {
                        approvals_store::transition_request(
                            &tx,
                            &consumed.approval_id,
                            row.revision,
                            &ApprovalUpdate {
                                event: g9_domain::state::ApprovalEvent::Consume,
                                actor: Some(meta.actor.id.clone()),
                                at: consumed.consumed_at,
                                binding_mac: None,
                            },
                        )
                        .map_err(|e| {
                            Refusal::after_allow(
                                e.into(),
                                authorized.evaluation.record.clone(),
                                Some(approval_id.clone()),
                            )
                        })?;
                    }
                    (authorized.evaluation.record, authorized.consumed)
                }
                Err(e) => {
                    let expired = matches!(
                        &e,
                        PolicyError::Approval {
                            source: ApprovalError::Expired,
                            ..
                        }
                    );
                    let record = e.record().cloned().map(|r| {
                        if stored.is_none() {
                            // The presented id does not exist: never link a
                            // dangling approval id from the decision row.
                            PolicyDecisionRecord {
                                approval_id: None,
                                ..r
                            }
                        } else {
                            r
                        }
                    });
                    let decision = match record.as_ref().map(|r| r.decision) {
                        Some(DecisionKind::ApprovalRequired) => AuditDecision::ApprovalRequired,
                        Some(DecisionKind::Allow) => AuditDecision::Allow,
                        _ => AuditDecision::Deny,
                    };
                    return Err(Refusal {
                        error: e.into(),
                        decision,
                        record,
                        new_approval: None,
                        touched_approval: Some(approval_id.clone()),
                        expire_approvals: expired,
                    });
                }
            }
        }
    };
    let touched = consumed.as_ref().map(|c| c.approval_id.clone());

    // Step 5: revision.
    if let (Some(expected), Some(actual)) = (meta.expected_revision, found.current_revision) {
        if expected != actual {
            return Err(Refusal::after_allow(
                G9Error::stale_revision(Some(expected), actual),
                record,
                touched,
            ));
        }
    }

    // Step 6: mutation.
    let mut env = Env {
        conn: &tx,
        svc: &mut *svc,
        meta,
        now,
        consumed,
    };
    let mut mutation = match handler(&mut env) {
        Ok(m) => m,
        Err(e) => return Err(Refusal::after_allow(e, record, touched)),
    };
    let audit_resource = mutation
        .audit_resource
        .clone()
        .unwrap_or_else(|| cmd.resource.clone());
    if let Some(error) = mutation.commit_error.take() {
        let committed = (|| -> Result<(), G9Error> {
            append_events(
                &tx,
                meta,
                now,
                &mutation.events,
                found.context.classification,
            )?;
            insert_decision(&tx, &record, meta, touched.as_ref())?;
            let audit = AuditChain::append(
                &tx,
                &audit_entry(
                    meta,
                    now,
                    cmd.spec.name,
                    Some(audit_resource.clone()),
                    AuditDecision::Allow,
                    AuditOutcome::from(error.category),
                    &json!({ "request_sha256": request_hash, "code": error.code, "rule_id": record.rule_id }),
                ),
            )?;
            svc.note_audit(audit.audit_ref);
            Ok(())
        })();
        if let Err(e) = committed {
            return Err(Refusal::after_allow(e, record, touched));
        }
        if let Err(e) = tx.commit() {
            return Err(Refusal::after_allow(storage_err(e), record, touched));
        }
        if let Some(update) = mutation.on_commit.take() {
            update(svc);
        }
        return Ok(Step::CommittedFailure(Box::new(error)));
    }
    let post = mutation.post_commit.take();
    if post.is_none() {
        if let Err(e) = schemas::validate_response(cmd.spec.name, &mutation.result) {
            return Err(Refusal::after_allow(e, record, touched));
        }
    }

    // Step 7: events, decision, audit.
    let finish = (|| -> Result<CommandOutcome<Value>, G9Error> {
        append_events(
            &tx,
            meta,
            now,
            &mutation.events,
            found.context.classification,
        )?;
        insert_decision(&tx, &record, meta, touched.as_ref())?;
        let audit = AuditChain::append(
            &tx,
            &audit_entry(
                meta,
                now,
                cmd.spec.name,
                Some(audit_resource.clone()),
                AuditDecision::Allow,
                AuditOutcome::Success,
                &json!({
                    "request_sha256": request_hash,
                    "parameters_sha256": record.parameters_sha256,
                    "rule_id": record.rule_id,
                    "approval_id": touched,
                    "revision": mutation.revision,
                    "events": mutation.events.iter().map(|e| e.kind.clone()).collect::<Vec<_>>(),
                }),
            ),
        )?;
        svc.note_audit(audit.audit_ref.clone());
        let mut outcome = CommandOutcome::new(mutation.result.clone());
        outcome.revision = mutation.revision;
        outcome.audit_ref = Some(audit.audit_ref);
        outcome.warnings = mutation.warnings.clone();
        if post.is_none() {
            // Step 8: the idempotency outcome commits with the mutation
            // (content-bearing results are stored as their replay record).
            let stored = stored_outcome(&outcome, mutation.replay_record.as_ref())?;
            idempotency::store(
                &tx,
                &meta.actor.id,
                &meta.idempotency_key,
                request_hash,
                &stored,
                now,
            )?;
        }
        Ok(outcome)
    })();
    let outcome = match finish {
        Ok(o) => o,
        Err(e) => return Err(Refusal::after_allow(e, record, touched)),
    };
    if let Err(e) = tx.commit() {
        return Err(Refusal::after_allow(storage_err(e), record, touched));
    }
    if let Some(update) = mutation.on_commit.take() {
        update(svc);
    }
    Ok(Step::Committed {
        outcome,
        post: post.map(|p| {
            Box::new((
                p,
                PostBase {
                    record,
                    audit_resource,
                    floor: found.context.classification,
                },
            ))
        }),
    })
}

fn run_post_commit(
    ctx: &mut AppContext,
    meta: &CommandMeta,
    cmd: &Command<'_>,
    request_hash: &str,
    now: UtcMillis,
    step: PostCommit,
    base: PostBase,
) -> Result<CommandOutcome<Value>, G9Error> {
    let AppContext { db, svc } = ctx;
    let operation = format!("{}.post_commit", cmd.spec.name);
    let produced = {
        let mut env = PostEnv { db, svc, meta, now };
        step(&mut env)
    };
    let produced = produced.and_then(|m| {
        schemas::validate_response(cmd.spec.name, &m.result)?;
        Ok(m)
    });
    let tx = db.begin_immediate()?;
    match produced {
        Ok(mutation) => {
            append_events(&tx, meta, now, &mutation.events, base.floor)?;
            let audit = AuditChain::append(
                &tx,
                &audit_entry(
                    meta,
                    now,
                    &operation,
                    Some(
                        mutation
                            .audit_resource
                            .clone()
                            .unwrap_or(base.audit_resource),
                    ),
                    AuditDecision::Allow,
                    AuditOutcome::Success,
                    &json!({
                        "request_sha256": request_hash,
                        "rule_id": base.record.rule_id,
                        "revision": mutation.revision,
                    }),
                ),
            )?;
            svc.note_audit(audit.audit_ref.clone());
            let mut outcome = CommandOutcome::new(mutation.result);
            outcome.revision = mutation.revision;
            outcome.audit_ref = Some(audit.audit_ref);
            outcome.warnings = mutation.warnings;
            let stored = stored_outcome(&outcome, mutation.replay_record.as_ref())?;
            idempotency::store(
                &tx,
                &meta.actor.id,
                &meta.idempotency_key,
                request_hash,
                &stored,
                now,
            )?;
            tx.commit().map_err(storage_err)?;
            Ok(outcome)
        }
        Err(error) => {
            let audit = AuditChain::append(
                &tx,
                &audit_entry(
                    meta,
                    now,
                    &operation,
                    Some(base.audit_resource),
                    AuditDecision::Allow,
                    AuditOutcome::from(error.category),
                    &json!({ "request_sha256": request_hash, "code": error.code }),
                ),
            )?;
            tx.commit().map_err(storage_err)?;
            svc.note_audit(audit.audit_ref);
            Err(error)
        }
    }
}

/// The failure transaction: what must survive a refusal.
fn record_refusal(
    ctx: &mut AppContext,
    meta: &CommandMeta,
    cmd: &Command<'_>,
    now: UtcMillis,
    refusal: Refusal,
) -> G9Error {
    let AppContext { db, svc } = ctx;
    let operator = svc.operator.clone();
    let result = (|| -> Result<(), G9Error> {
        let tx = db.begin_immediate()?;
        if refusal.expire_approvals {
            approvals_store::expire_due(&tx, now)?;
        }
        if let Some(request) = &refusal.new_approval {
            approvals_store::insert_request(
                &tx,
                &NewApprovalRequest {
                    id: request.id.clone(),
                    operation: request.operation.clone(),
                    required_capability: Some(request.required_capability),
                    requested_by: request.requested_by.id.clone(),
                    requested_at: request.requested_at,
                    expires_at: request.expires_at,
                    expected_revision: request.expected_revision,
                },
            )?;
        }
        if let Some(record) = &refusal.record {
            insert_decision(&tx, record, meta, record.approval_id.as_ref())?;
        }
        let audit = AuditChain::append(
            &tx,
            &audit_entry(
                meta,
                now,
                cmd.spec.name,
                Some(cmd.resource.clone()),
                refusal.decision,
                AuditOutcome::from(refusal.error.category),
                &json!({
                    "code": refusal.error.code,
                    "rule_id": refusal.record.as_ref().map(|r| r.rule_id.clone()),
                    "approval_id": refusal.error.approval_id.clone().or(refusal.touched_approval.clone()),
                }),
            ),
        )?;
        tx.commit().map_err(storage_err)?;
        svc.note_audit(audit.audit_ref);
        Ok(())
    })();
    // Keep the ledger equal to the database after the rollback/refusal.
    if cmd.resource.kind == g9_domain::ResourceKind::Approval {
        if let Ok(id) = ApprovalId::parse(&cmd.resource.id) {
            if let Err(e) = svc.approvals.refresh(db.conn(), &id, &operator) {
                tracing::error!(code = %e.code, "approval ledger resync failed");
            }
        }
    }
    if let Some(id) = &refusal.touched_approval {
        if let Err(e) = svc.approvals.refresh(db.conn(), id, &operator) {
            tracing::error!(code = %e.code, "approval ledger resync failed");
        }
    }
    match result {
        Ok(()) => {
            if let Some(request) = refusal.new_approval {
                if let Err(e) = svc.approvals.insert_pending(request) {
                    tracing::error!(code = %e.code, "approval ledger insert failed");
                }
            }
            refusal.error
        }
        Err(storage) => {
            tracing::error!(code = %storage.code, original = %refusal.error.code, "failed to record a refused command");
            if refusal.new_approval.is_some() {
                // Without the persisted request the approval id is useless, and the storage error says
                // nothing the caller may see. The refused attempt must still leave its decision and its
                // audit row (security audit 2026-11-12), so they are recorded without the id, in a
                // transaction of their own.
                let error = G9Error::new(
                    storage.category,
                    "approval.request_not_stored",
                    "the approval request could not be stored, so nothing can be approved for this attempt",
                )
                .with_details(json!({ "cause": storage.code }));
                record_unstored_request_refusal(ctx, meta, cmd, now, &refusal, &error);
                error
            } else {
                refusal.error
            }
        }
    }
}

/// The second, smaller failure transaction: the decision and the audit row of a refusal whose
/// approval request could not be stored. Both are written without the approval id (it names no
/// stored request); a failure here is logged and changes nothing the caller sees.
fn record_unstored_request_refusal(
    ctx: &mut AppContext,
    meta: &CommandMeta,
    cmd: &Command<'_>,
    now: UtcMillis,
    refusal: &Refusal,
    error: &G9Error,
) {
    let AppContext { db, svc } = ctx;
    let result = (|| -> Result<(), G9Error> {
        let tx = db.begin_immediate()?;
        if let Some(record) = &refusal.record {
            insert_decision(&tx, record, meta, None)?;
        }
        let audit = AuditChain::append(
            &tx,
            &audit_entry(
                meta,
                now,
                cmd.spec.name,
                Some(cmd.resource.clone()),
                refusal.decision,
                AuditOutcome::from(error.category),
                &json!({
                    "code": error.code,
                    "rule_id": refusal.record.as_ref().map(|r| r.rule_id.clone()),
                    "approval_id": Value::Null,
                }),
            ),
        )?;
        tx.commit().map_err(storage_err)?;
        svc.note_audit(audit.audit_ref);
        Ok(())
    })();
    if let Err(e) = result {
        tracing::error!(code = %e.code, "failed to record a refusal whose approval request was not stored");
    }
}

/// A declined native confirmation (ADR-0014): the operator was shown what an approval grants and
/// said no. It is a decision about a stored request, so it leaves one metadata-only audit row
/// (`approvals.decide`, `NOT_EVALUATED` because no rule decided anything, `UNAUTHORIZED`, the
/// approval id and the refusal code hashed as details) and changes nothing else: the request
/// stays pending until it expires, is rejected or is approved later.
///
/// Nothing is written when the database is read-only, and the row is skipped when the metadata
/// is not a valid command metadata (the request is refused as such later). A failure to write
/// is logged and changes nothing the caller sees: the refusal is the answer either way.
pub(crate) fn record_declined_confirmation(
    ctx: &mut AppContext,
    meta_json: &Value,
    approval_id: &ApprovalId,
) {
    if ctx.svc.recovery.write_block_reason().is_some() {
        return;
    }
    let Ok(meta) = crate::ipc::parse_meta("approvals.decide", meta_json, ctx.operator()) else {
        return;
    };
    let now = ctx.svc.now();
    let AppContext { db, svc } = ctx;
    let result = (|| -> Result<(), G9Error> {
        let tx = db.begin_immediate()?;
        let audit = AuditChain::append(
            &tx,
            &audit_entry(
                &meta,
                now,
                "approvals.decide",
                Some(ResourceRef::new(
                    g9_domain::ResourceKind::Approval,
                    approval_id.as_str(),
                )),
                AuditDecision::NotEvaluated,
                AuditOutcome::Unauthorized,
                &json!({
                    "code": "approval.not_confirmed",
                    "approval_id": approval_id.as_str(),
                }),
            ),
        )?;
        tx.commit().map_err(storage_err)?;
        svc.note_audit(audit.audit_ref);
        Ok(())
    })();
    if let Err(e) = result {
        tracing::error!(code = %e.code, "failed to record a declined confirmation");
    }
}

fn storage_err(e: rusqlite::Error) -> G9Error {
    G9Error::from(g9_storage::StorageError::from(e))
}

/// Append the command's events. An event is never classified below the
/// resource it is about (`floor`, the policy facts' classification): events
/// reach exports and backups, so an event about a PRIVATE attempt or note is
/// PRIVATE. `domain_events` never stores SECRET (storage refuses it), so an
/// event about SECRET content is PRIVATE and carries no content hash (the hash
/// of a short secret is the secret).
fn append_events(
    conn: &Connection,
    meta: &CommandMeta,
    now: UtcMillis,
    specs: &[EventSpec],
    floor: Classification,
) -> Result<(), G9Error> {
    for spec in specs {
        let raised = spec.classification.max(floor);
        let (classification, content_sha256) = if raised == Classification::Secret {
            (Classification::Private, None)
        } else {
            (raised, spec.content_sha256.clone())
        };
        events::append_event(
            conn,
            &DomainEvent {
                header: EventHeader {
                    event_id: EventId::new_v7(),
                    kind: spec.kind.clone(),
                    occurred_at: now,
                    actor: meta.actor.clone(),
                    trust: meta.actor.trust(),
                    schema_version: SchemaVersion::V1,
                    correlation_id: meta.trace_id.clone(),
                    content_sha256,
                },
                payload: spec.payload.clone(),
                data_domain: spec.domain,
                classification,
            },
        )?;
    }
    Ok(())
}

/// Persist a metadata-only `policy_decisions` row.
pub(crate) fn insert_decision(
    conn: &Connection,
    record: &PolicyDecisionRecord,
    meta: &CommandMeta,
    approval_id: Option<&ApprovalId>,
) -> Result<(), G9Error> {
    let decision = match record.decision {
        DecisionKind::Allow => Decision::Allow {
            rule_id: record.rule_id.clone(),
        },
        DecisionKind::Deny => Decision::Deny {
            rule_id: record.rule_id.clone(),
            reason: record.reason.clone(),
        },
        DecisionKind::ApprovalRequired => match record.required_capability {
            Some(required_capability) => Decision::ApprovalRequired {
                rule_id: record.rule_id.clone(),
                reason: record.reason.clone(),
                required_capability,
            },
            None => Decision::Deny {
                rule_id: record.rule_id.clone(),
                reason: "approval required without a capability (refused)".to_string(),
            },
        },
    };
    approvals_store::insert_policy_decision(
        conn,
        &PolicyDecisionRow {
            id: PolicyDecisionId::new_v7(),
            decided_at: record.evaluated_at,
            actor: Actor {
                id: record.actor_id.clone(),
                kind: record.actor_kind.clone(),
            },
            operation: OperationDescriptor {
                action: record.action,
                resource: ResourceRef::new(record.resource_kind, record.resource_id.clone()),
                scope: g9_domain::Scope {
                    domain: record.scope_domain,
                    mode: record.mode,
                },
                parameters_sha256: record.parameters_sha256.clone(),
            },
            decision,
            approval_id: approval_id.cloned(),
            request_id: Some(meta.request_id.clone()),
            trace_id: Some(meta.trace_id.clone()),
        },
    )?;
    Ok(())
}

pub(crate) fn audit_entry(
    meta: &CommandMeta,
    now: UtcMillis,
    operation: &str,
    resource: Option<ResourceRef>,
    decision: AuditDecision,
    outcome: AuditOutcome,
    details: &Value,
) -> AuditEntry {
    AuditEntry {
        occurred_at: now,
        actor: meta.actor.clone(),
        operation: operation.to_string(),
        resource,
        decision,
        outcome,
        request_id: meta.request_id.clone(),
        trace_id: meta.trace_id.clone(),
        details_sha256: Some(AuditEntry::details_hash(details)),
    }
}
