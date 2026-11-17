//! Open-time recovery (ARCHITECTURE.md §6 "Migrations", §7 "Timer ledger",
//! §11 approval expiry; gate A03; runbook `docs/runbooks/backup-restore.md`).
//!
//! Sequence, run once by [`crate::context::AppContextBuilder::open`]:
//!
//! 1. **Migrations.** `g9-storage` migrates a fresh database on open and opens
//!    an existing database with pending migrations *unmigrated*. Here, pending
//!    migrations are applied only after a verified backup: Online Backup API
//!    snapshot into the configured backup directory → verification on the copy
//!    (attachment files copied to `<backup_dir>/attachments` and hashed
//!    there) → (file databases)
//!    `Migrator::dry_run_on_copy` in the scratch directory → `migrate_pending`.
//!    Without a backup directory, without an approved scratch directory for a
//!    file database (the dry run is mandatory, runbook Migration step 2), or
//!    when any step fails, the database stays unmigrated and every mutating
//!    command is refused (`DATA_CORRUPTION recovery.integrity_failed` with
//!    reason `pending_migrations`).
//! 2. **Audit chain.** `AuditChain::verify_chain`; a broken chain blocks
//!    mutations (tamper evidence must not be written over).
//! 3. **Open intervals.** Every interval still open is closed `UNCERTAIN`
//!    (no monotonic end; never credited without an operator correction), its
//!    session moves to `INTERRUPTED` through the session state machine, a
//!    `session.interrupted` event and an audit row are written — all by the
//!    system actor after a policy evaluation that is persisted.
//! 4. **Approval expiry sweep.** `PENDING`/`APPROVED` requests past their
//!    expiry move to `EXPIRED` (audited).
//! 5. **Idempotency retention.** `idempotency_records` older than
//!    `g9_storage::idempotency::RETENTION_DAYS` (30 days,
//!    `docs/privacy/data-lifecycle.md`) are deleted; when any were, one audit row
//!    records the count (data inventory gap G1: the documented retention was never
//!    enforced, and the stored results repeat operator text). Like steps 3 and 4,
//!    it runs only when writes are allowed, in the same transaction.

// Step 0 (before the steps above): the schema itself is compared with the
// embedded migrations at the recorded version; a difference blocks every write,
// recovery's own included (`schema_drift`).

use crate::context::{KnownBackup, Services};
use crate::pipeline::{audit_entry, insert_decision};
use g9_calendar::{Continuity, SessionEventRow};
use g9_domain::state::SessionEvent;
use g9_domain::{
    Action, ApprovalId, BackupId, Classification, CommandMeta, DataDomain, EventHeader, EventId,
    G9Error, IdempotencyKey, InteractionMode, OperationDescriptor, RequestId, ResourceKind,
    ResourceRef, Revision, SchemaVersion, Scope, SessionId, TraceId, UtcMillis,
};
use g9_policy::{DigestDomain, ResourceContext};
use g9_storage::approvals_store;
use g9_storage::audit::{AuditChain, AuditDecision, AuditOutcome};
use g9_storage::backup::{self, SnapshotRequest};
use g9_storage::events::{self, DomainEvent};
use g9_storage::idempotency;
use g9_storage::repos::sessions::{self, ClosedBy};
use g9_storage::repos::ReadScope;
use g9_storage::{Database, Migrator};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// What recovery did (exposed through `AppContext::recovery_report` and `system.status`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryReport {
    /// Migration versions applied during this open.
    pub migrated: Vec<u32>,
    /// Migration versions still pending (mutations are refused while non-empty).
    pub pending_migrations: Vec<u32>,
    /// Why pending migrations were not applied, when they were not.
    pub migration_blocked_reason: Option<String>,
    /// Backup taken before migrating.
    pub pre_migration_backup: Option<BackupId>,
    /// Audit chain verification result.
    pub audit_chain_ok: bool,
    /// Rows examined by the chain verification.
    pub audit_rows: u64,
    /// First failing chain position, when broken.
    pub audit_first_bad_seq: Option<i64>,
    /// Sessions moved to `INTERRUPTED`.
    pub interrupted_sessions: Vec<SessionId>,
    /// Intervals closed `UNCERTAIN`.
    pub closed_intervals: Vec<String>,
    /// Approvals moved to `EXPIRED`.
    pub expired_approvals: Vec<ApprovalId>,
    /// Schema objects that differ from what the embedded migrations produce at the
    /// recorded version (`g9_storage::migrations::verify_schema`); empty when the
    /// schema matches. Mutations are refused while non-empty.
    #[serde(default)]
    pub schema_differences: Vec<String>,
    /// Idempotency records deleted at open because they were past retention.
    #[serde(default)]
    pub purged_idempotency_records: u64,
}

impl RecoveryReport {
    /// Why mutations must be refused, if they must.
    pub fn write_block_reason(&self) -> Option<&'static str> {
        if !self.schema_differences.is_empty() {
            Some("schema_drift")
        } else if !self.audit_chain_ok {
            Some("audit_chain_broken")
        } else if !self.pending_migrations.is_empty() {
            Some("pending_migrations")
        } else {
            None
        }
    }
}

/// Run the recovery sequence.
pub fn run(db: &mut Database, svc: &mut Services) -> Result<RecoveryReport, G9Error> {
    let mut report = RecoveryReport {
        migrated: db
            .migration_report()
            .map(|r| r.applied.iter().map(|m| m.version).collect())
            .unwrap_or_default(),
        ..RecoveryReport::default()
    };

    // Before anything is written: a schema that is not exactly the one the
    // migrations produce may carry triggers that would run inside recovery's own
    // writes (and every later command). Report it and write nothing.
    let schema = g9_storage::migrations::verify_schema(db.conn())?;
    if !schema.ok() {
        report.schema_differences = schema.differences;
        let chain = AuditChain::verify_chain(db.conn())?;
        report.audit_chain_ok = chain.ok;
        report.audit_rows = chain.rows;
        report.audit_first_bad_seq = chain.first_bad_seq;
        tracing::error!(
            version = schema.version,
            differences = report.schema_differences.len(),
            "schema differs from the embedded migrations; mutations are refused"
        );
        return Ok(report);
    }

    migrate_after_backup(db, svc, &mut report)?;

    let chain = AuditChain::verify_chain(db.conn())?;
    report.audit_chain_ok = chain.ok;
    report.audit_rows = chain.rows;
    report.audit_first_bad_seq = chain.first_bad_seq;
    if !chain.ok {
        tracing::error!(
            first_bad_seq = ?chain.first_bad_seq,
            "audit chain verification failed; mutations are refused"
        );
    }
    if report.write_block_reason().is_some() {
        // Nothing may be written over a broken chain or an unmigrated schema.
        return Ok(report);
    }
    recover_intervals_and_approvals(db, svc, &mut report)?;
    Ok(report)
}

fn migrate_after_backup(
    db: &mut Database,
    svc: &mut Services,
    report: &mut RecoveryReport,
) -> Result<(), G9Error> {
    let pending = db.pending_migrations().to_vec();
    if pending.is_empty() {
        return Ok(());
    }
    report.pending_migrations = pending;
    let Some(dir) = svc.config.backup_dir.clone() else {
        report.migration_blocked_reason = Some(
            "no backup directory is configured; migrations need a verified backup first".into(),
        );
        return Ok(());
    };
    if !svc
        .approved_roots
        .as_ref()
        .is_some_and(|roots| roots.contains_directory(&dir))
    {
        report.migration_blocked_reason =
            Some("backup directory is not inside an approved root".into());
        return Ok(());
    }
    // A file database is migrated only after a dry run on a copy, which needs
    // an approved scratch directory; checked before any snapshot is taken.
    let file_db = db.path().map(std::path::Path::to_path_buf);
    let scratch = match (&file_db, svc.config.scratch_dir.clone()) {
        (None, _) => None,
        (Some(_), None) => {
            report.migration_blocked_reason = Some(
                "no scratch directory is configured; the migration dry run on a copy is required"
                    .into(),
            );
            return Ok(());
        }
        (Some(_), Some(scratch)) => {
            if !svc
                .approved_roots
                .as_ref()
                .is_some_and(|roots| roots.contains_directory(&scratch))
            {
                report.migration_blocked_reason =
                    Some("scratch directory is not inside an approved root".into());
                return Ok(());
            }
            Some(scratch)
        }
    };
    // The backup refuses to start below the free-space threshold (STOR-4),
    // before any file is written; the database stays unmigrated.
    if let Err(e) = backup::check_free_space(db.conn(), &dir, svc.config.free_space) {
        report.migration_blocked_reason =
            Some(format!("pre-migration backup refused: {}", e.code()));
        return Ok(());
    }
    let now = svc.now();
    let backup_id = BackupId::new_v7();
    let dest = dir.join(format!("{}.pre-migration.sqlite", backup_id.as_str()));
    let mut manifest = match backup::create_snapshot(
        db.conn(),
        &dest,
        &SnapshotRequest {
            backup_id: backup_id.clone(),
            requested_at: now,
            app_version: svc.config.app_version.clone(),
        },
    ) {
        Ok(m) => m,
        Err(e) => {
            report.migration_blocked_reason =
                Some(format!("pre-migration backup failed: {}", e.code()));
            return Ok(());
        }
    };
    // The attachment files travel with this backup as with `backup.create`:
    // verification hashes them in `<backup_dir>/attachments`, the store a restore
    // copies them from (STOR-2). A file that is neither intact in the live store
    // nor already intact there fails the verification below.
    // From here the snapshot exists. A pre-migration backup that is refused is
    // not recorded, so its file would be an orphan copy of every note: it is
    // removed on each refusal below.
    if let Err(e) = crate::commands::backup::copy_attachments(db.conn(), svc, &dir) {
        report.migration_blocked_reason = Some(format!(
            "pre-migration backup could not copy the attachment files: {}",
            e.code
        ));
        discard_unrecorded_snapshot(&dest);
        return Ok(());
    }
    let options = crate::commands::backup::verify_options(svc, &manifest);
    let verified = backup::verify_and_advance(&mut manifest, &options);
    if !matches!(&verified, Ok(r) if r.ok()) {
        report.migration_blocked_reason = Some("pre-migration backup failed verification".into());
        discard_unrecorded_snapshot(&dest);
        return Ok(());
    }
    if matches!(&verified, Ok(r) if crate::commands::backup::attachments_unchecked(r)) {
        report.migration_blocked_reason = Some(
            "pre-migration backup holds attachments that were not checked (no attachment directory is configured)"
                .into(),
        );
        discard_unrecorded_snapshot(&dest);
        return Ok(());
    }
    if let (Some(path), Some(scratch)) = (file_db, scratch) {
        let migrator = Migrator::new(svc.config.app_version.clone(), now);
        // The rehearsal copy is disposable whatever the outcome: a copy left
        // behind would only be replaced by the next rehearsal (storage-core-3).
        match migrator.dry_run_on_copy(&path, &scratch) {
            Ok(dry) if dry.integrity_ok && dry.foreign_key_violations == 0 => {
                discard_rehearsal_copy(&dry.copy_path);
            }
            Ok(dry) => {
                discard_rehearsal_copy(&dry.copy_path);
                report.migration_blocked_reason = Some(format!(
                    "migration dry run on a copy failed its checks (integrity {}, foreign-key violations {})",
                    if dry.integrity_ok { "ok" } else { "failed" },
                    dry.foreign_key_violations
                ));
                discard_unrecorded_snapshot(&dest);
                return Ok(());
            }
            Err(e) => {
                report.migration_blocked_reason =
                    Some(format!("migration dry run on a copy failed: {}", e.code()));
                discard_unrecorded_snapshot(&dest);
                return Ok(());
            }
        }
    }
    backup::record_manifest(db.conn(), &manifest, now)?;
    let applied = db.migrate_pending()?;
    report
        .migrated
        .extend(applied.applied.iter().map(|m| m.version));
    report.pending_migrations = db.pending_migrations().to_vec();
    report.pre_migration_backup = Some(backup_id.clone());
    let size_bytes = std::fs::metadata(&dest).ok().map(|m| m.len());
    svc.backups.insert(
        backup_id,
        KnownBackup {
            manifest,
            revision: Revision::INITIAL,
            label: Some("pre-migration".into()),
            size_bytes,
        },
    );
    Ok(())
}

fn system_meta(svc: &Services, now: UtcMillis) -> CommandMeta {
    CommandMeta {
        request_id: RequestId::new_v7(),
        actor: svc.system.clone(),
        operation: "recovery.open".to_string(),
        resource: None,
        expected_revision: None,
        idempotency_key: IdempotencyKey::new_v7(),
        scope: Scope {
            domain: DataDomain::System,
            mode: InteractionMode::Maintenance,
        },
        payload_schema_version: SchemaVersion::V1,
        approval_id: None,
        trace_id: TraceId::new_v7(),
        issued_at: now,
    }
}

fn recover_intervals_and_approvals(
    db: &mut Database,
    svc: &mut Services,
    report: &mut RecoveryReport,
) -> Result<(), G9Error> {
    let now = svc.now();
    let meta = system_meta(svc, now);
    let tx = db.begin_immediate()?;
    let open = sessions::open_intervals(&tx)?;
    for interval in open {
        let session_id = interval.record.session_id.clone();
        let Some(session) = sessions::get_session(
            &tx,
            &ReadScope::cross_domain(DataDomain::Learning),
            &session_id,
        )?
        else {
            continue;
        };
        let op = OperationDescriptor {
            action: Action::Update,
            resource: ResourceRef::new(ResourceKind::StudySession, session_id.as_str()),
            scope: Scope {
                domain: session.data_domain,
                mode: InteractionMode::Maintenance,
            },
            // Keyed like every other parameters digest in the ledgers (ADR-0021).
            parameters_sha256: svc
                .binding_key
                .digest_json(
                    DigestDomain::Parameters,
                    &json!({
                        "operation": "recovery.interrupt_session",
                        "session_id": session_id,
                        "interval_id": interval.id,
                    }),
                )
                .map_err(|_| G9Error::internal_redacted("digest.key_rejected"))?,
        };
        let ctx = ResourceContext::new(session.data_domain, session.classification, false);
        let evaluation = svc.policy.evaluate(&svc.system, &op, &ctx);
        insert_decision(&tx, &evaluation.record, &meta, None)?;
        if !evaluation.is_allow() {
            tracing::error!(session = %session_id, "policy refused recovery of an open interval");
            continue;
        }
        let end = if now < interval.record.start_wall {
            interval.record.start_wall
        } else {
            now
        };
        sessions::close_interval(
            &tx,
            &interval.id,
            interval.revision,
            end,
            None,
            Continuity::Uncertain,
            ClosedBy::Recovery,
            now,
        )?;
        report.closed_intervals.push(interval.id.clone());
        let mut revision = session.revision;
        if session.state.can(SessionEvent::Interrupt) {
            let seq = u32::try_from(sessions::list_session_events(&tx, &session_id)?.len())
                .unwrap_or(u32::MAX);
            revision = sessions::transition_session(
                &tx,
                &format!("{}:recovery:{}", session_id.as_str(), interval.id),
                &SessionEventRow {
                    seq,
                    session_id: session_id.clone(),
                    event: SessionEvent::Interrupt,
                    from: session.state,
                    to: session
                        .state
                        .transition(SessionEvent::Interrupt)
                        .map_err(|e| {
                            G9Error::precondition_failed(
                                "session.illegal_transition",
                                e.to_string(),
                            )
                        })?,
                    actor: svc.system.clone(),
                    occurred_at: now,
                },
                Some(
                    &json!({ "reason": "open interval at process start", "interval_id": interval.id }),
                ),
                revision,
            )?;
            report.interrupted_sessions.push(session_id.clone());
        }
        let ledger = crate::commands::sessions::ledger_for(&tx, svc, &session_id)?;
        let credit = ledger.credited(&session_id);
        sessions::update_session_credit(
            &tx,
            &session_id,
            revision,
            credit.total_credited_ms,
            credit.uncertain_ms,
            now,
        )?;
        events::append_event(
            &tx,
            &DomainEvent {
                header: EventHeader {
                    event_id: EventId::new_v7(),
                    kind: "session.interrupted".to_string(),
                    occurred_at: now,
                    actor: svc.system.clone(),
                    trust: svc.system.trust(),
                    schema_version: SchemaVersion::V1,
                    correlation_id: meta.trace_id.clone(),
                    content_sha256: None,
                },
                payload: json!({
                    "session_id": session_id,
                    "interval_id": interval.id,
                    "continuity": "UNCERTAIN",
                    "reason": "open interval at process start",
                }),
                data_domain: DataDomain::Learning,
                classification: Classification::Internal,
            },
        )?;
        let audit = AuditChain::append(
            &tx,
            &audit_entry(
                &meta,
                now,
                "recovery.interrupt_session",
                Some(ResourceRef::new(
                    ResourceKind::StudySession,
                    session_id.as_str(),
                )),
                AuditDecision::Allow,
                AuditOutcome::Success,
                &json!({ "interval_id": interval.id, "rule_id": evaluation.record.rule_id }),
            ),
        )?;
        svc.note_audit(audit.audit_ref);
    }
    for id in approvals_store::expire_due(&tx, now)? {
        let audit = AuditChain::append(
            &tx,
            &audit_entry(
                &meta,
                now,
                "recovery.approval_expired",
                Some(ResourceRef::new(ResourceKind::Approval, id.as_str())),
                AuditDecision::NotEvaluated,
                AuditOutcome::Success,
                &json!({ "approval_id": id }),
            ),
        )?;
        svc.note_audit(audit.audit_ref);
        report.expired_approvals.push(id);
    }
    let cutoff = idempotency::retention_cutoff(now);
    let purged = idempotency::purge_older_than(&tx, cutoff)?;
    if purged > 0 {
        let audit = AuditChain::append(
            &tx,
            &audit_entry(
                &meta,
                now,
                "recovery.idempotency_purged",
                None,
                AuditDecision::NotEvaluated,
                AuditOutcome::Success,
                &json!({ "purged": purged, "cutoff": cutoff }),
            ),
        )?;
        svc.note_audit(audit.audit_ref);
        report.purged_idempotency_records = u64::try_from(purged).unwrap_or(u64::MAX);
    }
    tx.commit()
        .map_err(|e| G9Error::from(g9_storage::StorageError::from(e)))?;
    Ok(())
}

/// Remove a migration rehearsal copy; a failure is logged, never fatal.
/// Remove a pre-migration snapshot that was refused and never recorded (no
/// manifest row names it, so nothing could restore from it).
fn discard_unrecorded_snapshot(snapshot: &std::path::Path) {
    if let Err(e) = g9_storage::migrations::remove_scratch_copy(snapshot) {
        tracing::warn!(
            code = e.code(),
            "refused pre-migration snapshot was not removed"
        );
    }
}

fn discard_rehearsal_copy(copy: &std::path::Path) {
    if let Err(e) = g9_storage::migrations::remove_scratch_copy(copy) {
        tracing::warn!(code = e.code(), "migration dry-run copy was not removed");
    }
}
