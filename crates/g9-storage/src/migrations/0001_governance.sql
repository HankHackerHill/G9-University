-- 0001_governance: governance / system tables (ARCHITECTURE.md §6, handoff §7 "Governance").
--
-- Conventions used by every migration in this crate:
--   * every application table is STRICT; ids are TEXT; instants are INTEGER epoch milliseconds;
--     durations are INTEGER milliseconds; booleans are INTEGER 0/1 with a CHECK;
--     enums are TEXT with CHECK lists that mirror the g9-domain Rust enums (SCREAMING_SNAKE_CASE);
--     JSON columns carry a json_valid() CHECK; mutable rows have revision/created_at/updated_at.
--   * every foreign key states its deletion action explicitly, with a comment that justifies it.
--     RESTRICT on evidentiary rows; CASCADE only inside a curriculum version's structural tree;
--     SET NULL where the reference is informational.
--   * immutability is enforced at the storage level with BEFORE UPDATE/DELETE triggers that RAISE(ABORT)
--     with a message prefixed by "IMMUTABLE:" (mapped to PRECONDITION_FAILED by g9-storage).
--
-- `schema_migrations` is bootstrapped by the Migrator before this file runs (it must exist in order
-- to record the application of this very migration); its definition lives in migrations.rs.

CREATE TABLE app_settings (
  key            TEXT    PRIMARY KEY NOT NULL CHECK (length(key) BETWEEN 1 AND 128),
  value_json     TEXT    NOT NULL CHECK (json_valid(value_json)),
  data_domain    TEXT    NOT NULL DEFAULT 'SYSTEM'
                 CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification TEXT    NOT NULL DEFAULT 'INTERNAL'
                 CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision       INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL
) STRICT;

-- Hash-chained, metadata-only audit log (A04 at storage level; ARCHITECTURE §6 "Audit chain").
-- seq is the chain order; rows are never updated or deleted, so AUTOINCREMENT guarantees monotonic seq.
CREATE TABLE audit_metadata (
  seq            INTEGER PRIMARY KEY AUTOINCREMENT,
  id             TEXT    NOT NULL UNIQUE CHECK (length(id) BETWEEN 1 AND 128),
  prev_hash      TEXT    NOT NULL CHECK (length(prev_hash) = 64),
  row_hash       TEXT    NOT NULL UNIQUE CHECK (length(row_hash) = 64),
  occurred_at    INTEGER NOT NULL,
  actor_id       TEXT    NOT NULL,
  actor_kind     TEXT    NOT NULL CHECK (actor_kind IN ('OPERATOR','SYSTEM','SKILL','MODEL')),
  actor_skill_id TEXT,
  operation      TEXT    NOT NULL,
  resource_kind  TEXT    CHECK (resource_kind IS NULL OR resource_kind IN (
                   'CURRICULUM','CURRICULUM_VERSION','CALENDAR_EVENT','SCHEDULE_PROPOSAL','STUDY_SESSION',
                   'TIME_INTERVAL','NOTE','ATTACHMENT','EVIDENCE_ITEM','ASSESSMENT_DEFINITION','ATTEMPT',
                   'GRADE_DECISION','COMPETENCY','OBSERVATION','LEARNER_HYPOTHESIS','REVIEW_ITEM','INTERVENTION',
                   'EXPERIMENT','GOAL','PROJECT','MILESTONE','DECISION','RESEARCH_SOURCE','KNOWLEDGE_DOCUMENT',
                   'MODEL','SKILL','SKILL_INVOCATION','APPROVAL','BACKUP','POLICY','EXPORT','FILESYSTEM_PATH',
                   'NETWORK_HOST','PROCESS','APP_SETTING','AUDIT_METADATA')),
  resource_id    TEXT,
  decision       TEXT    NOT NULL CHECK (decision IN ('ALLOW','DENY','APPROVAL_REQUIRED','NOT_EVALUATED')),
  outcome        TEXT    NOT NULL CHECK (outcome IN ('SUCCESS','INVALID_INPUT','UNAUTHORIZED','APPROVAL_REQUIRED',
                   'STALE_REVISION','PRECONDITION_FAILED','NOT_FOUND','CONFLICT','RESOURCE_UNAVAILABLE','TIMEOUT',
                   'CANCELLED','RETRYABLE_STORAGE','DATA_CORRUPTION','INTERNAL')),
  request_id     TEXT    NOT NULL,
  trace_id       TEXT    NOT NULL,
  details_sha256 TEXT    CHECK (details_sha256 IS NULL OR length(details_sha256) = 64)
) STRICT;
CREATE INDEX idx_audit_metadata_occurred_at ON audit_metadata (occurred_at);
CREATE INDEX idx_audit_metadata_resource    ON audit_metadata (resource_kind, resource_id);
CREATE INDEX idx_audit_metadata_request     ON audit_metadata (request_id);
CREATE INDEX idx_audit_metadata_trace       ON audit_metadata (trace_id);
CREATE TRIGGER trg_audit_metadata_no_update BEFORE UPDATE ON audit_metadata
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: audit_metadata rows cannot be updated'); END;
CREATE TRIGGER trg_audit_metadata_no_delete BEFORE DELETE ON audit_metadata
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: audit_metadata rows cannot be deleted'); END;

-- Approval requests (M16). operation_json is the canonical OperationDescriptor; parameters_sha256 binds it.
CREATE TABLE approval_requests (
  id                  TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  operation_json      TEXT    NOT NULL CHECK (json_valid(operation_json)),
  action              TEXT    NOT NULL CHECK (action IN ('READ','CREATE','UPDATE','DELETE','EXPORT','IMPORT',
                        'PROMOTE_CANONICAL','PROPOSE','APPROVE','APPLY','FINALIZE','INVOKE','EXECUTE','CONNECT',
                        'LOAD','CORRECT')),
  parameters_sha256   TEXT    NOT NULL CHECK (length(parameters_sha256) = 64),
  resource_kind       TEXT    NOT NULL CHECK (resource_kind IN (
                        'CURRICULUM','CURRICULUM_VERSION','CALENDAR_EVENT','SCHEDULE_PROPOSAL','STUDY_SESSION',
                        'TIME_INTERVAL','NOTE','ATTACHMENT','EVIDENCE_ITEM','ASSESSMENT_DEFINITION','ATTEMPT',
                        'GRADE_DECISION','COMPETENCY','OBSERVATION','LEARNER_HYPOTHESIS','REVIEW_ITEM','INTERVENTION',
                        'EXPERIMENT','GOAL','PROJECT','MILESTONE','DECISION','RESEARCH_SOURCE','KNOWLEDGE_DOCUMENT',
                        'MODEL','SKILL','SKILL_INVOCATION','APPROVAL','BACKUP','POLICY','EXPORT','FILESYSTEM_PATH',
                        'NETWORK_HOST','PROCESS','APP_SETTING','AUDIT_METADATA')),
  resource_id         TEXT    NOT NULL,
  scope_domain        TEXT    NOT NULL CHECK (scope_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  scope_mode          TEXT    NOT NULL CHECK (scope_mode IN ('STUDY','LAB','EXAM','REVIEW','EXECUTIVE','RESEARCH','MAINTENANCE')),
  required_capability TEXT    CHECK (required_capability IS NULL OR required_capability IN (
                        'PROTECTED_RECORD_WRITE','GRADE_FINALIZE','SCHEDULE_APPLY','CURRICULUM_IMPORT','CURRICULUM_PROMOTE',
                        'SKILL_INVOKE','MODEL_INFERENCE','FILESYSTEM_READ','FILESYSTEM_WRITE','NETWORK_EGRESS','SHELL_EXEC',
                        'DATA_EXPORT','DATA_DELETE','POLICY_MODIFY','MODEL_LOAD','LAB_ORCHESTRATE','CROSS_DOMAIN_READ')),
  state               TEXT    NOT NULL CHECK (state IN ('PENDING','APPROVED','REJECTED','EXPIRED','CONSUMED','REVOKED')),
  requested_by        TEXT    NOT NULL,
  requested_at        INTEGER NOT NULL,
  expires_at          INTEGER NOT NULL CHECK (expires_at > requested_at),
  expected_revision   INTEGER CHECK (expected_revision IS NULL OR expected_revision >= 1),
  binding_mac         TEXT    CHECK (binding_mac IS NULL OR length(binding_mac) = 64),
  decided_by          TEXT,
  decided_at          INTEGER,
  consumed_at         INTEGER,
  revision            INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at          INTEGER NOT NULL,
  updated_at          INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_approval_requests_state      ON approval_requests (state, expires_at);
CREATE INDEX idx_approval_requests_resource   ON approval_requests (resource_kind, resource_id);
CREATE INDEX idx_approval_requests_requested  ON approval_requests (requested_at);
-- Terminal approval states are immutable (single-use tokens; replay must be detectable forever).
CREATE TRIGGER trg_approval_requests_terminal_no_update BEFORE UPDATE ON approval_requests
WHEN OLD.state IN ('CONSUMED','REJECTED','EXPIRED','REVOKED')
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: approval request in a terminal state cannot be updated'); END;
CREATE TRIGGER trg_approval_requests_terminal_no_delete BEFORE DELETE ON approval_requests
WHEN OLD.state IN ('CONSUMED','REJECTED','EXPIRED','REVOKED')
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: approval request in a terminal state cannot be deleted'); END;

-- Operator decisions on approval requests. Evidentiary: immutable; RESTRICT keeps the request row.
CREATE TABLE approval_decisions (
  id                TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  approval_id       TEXT    NOT NULL REFERENCES approval_requests (id)
                    ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: a decision is evidence about its request
  decision          TEXT    NOT NULL CHECK (decision IN ('APPROVED','REJECTED','REVOKED')),
  decided_by        TEXT    NOT NULL,
  decided_at        INTEGER NOT NULL,
  reason            TEXT,
  parameters_sha256 TEXT    NOT NULL CHECK (length(parameters_sha256) = 64),
  expected_revision INTEGER CHECK (expected_revision IS NULL OR expected_revision >= 1),
  binding_mac       TEXT    CHECK (binding_mac IS NULL OR length(binding_mac) = 64),
  created_at        INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_approval_decisions_approval ON approval_decisions (approval_id);
CREATE INDEX idx_approval_decisions_decided  ON approval_decisions (decided_at);
CREATE TRIGGER trg_approval_decisions_no_update BEFORE UPDATE ON approval_decisions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: approval_decisions rows cannot be updated'); END;
CREATE TRIGGER trg_approval_decisions_no_delete BEFORE DELETE ON approval_decisions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: approval_decisions rows cannot be deleted'); END;

-- Every policy evaluation (metadata only). Immutable.
CREATE TABLE policy_decisions (
  id                  TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  decided_at          INTEGER NOT NULL,
  actor_id            TEXT    NOT NULL,
  actor_kind          TEXT    NOT NULL CHECK (actor_kind IN ('OPERATOR','SYSTEM','SKILL','MODEL')),
  actor_skill_id      TEXT,
  action              TEXT    NOT NULL CHECK (action IN ('READ','CREATE','UPDATE','DELETE','EXPORT','IMPORT',
                        'PROMOTE_CANONICAL','PROPOSE','APPROVE','APPLY','FINALIZE','INVOKE','EXECUTE','CONNECT',
                        'LOAD','CORRECT')),
  resource_kind       TEXT    NOT NULL CHECK (resource_kind IN (
                        'CURRICULUM','CURRICULUM_VERSION','CALENDAR_EVENT','SCHEDULE_PROPOSAL','STUDY_SESSION',
                        'TIME_INTERVAL','NOTE','ATTACHMENT','EVIDENCE_ITEM','ASSESSMENT_DEFINITION','ATTEMPT',
                        'GRADE_DECISION','COMPETENCY','OBSERVATION','LEARNER_HYPOTHESIS','REVIEW_ITEM','INTERVENTION',
                        'EXPERIMENT','GOAL','PROJECT','MILESTONE','DECISION','RESEARCH_SOURCE','KNOWLEDGE_DOCUMENT',
                        'MODEL','SKILL','SKILL_INVOCATION','APPROVAL','BACKUP','POLICY','EXPORT','FILESYSTEM_PATH',
                        'NETWORK_HOST','PROCESS','APP_SETTING','AUDIT_METADATA')),
  resource_id         TEXT    NOT NULL,
  scope_domain        TEXT    NOT NULL CHECK (scope_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  scope_mode          TEXT    NOT NULL CHECK (scope_mode IN ('STUDY','LAB','EXAM','REVIEW','EXECUTIVE','RESEARCH','MAINTENANCE')),
  parameters_sha256   TEXT    NOT NULL CHECK (length(parameters_sha256) = 64),
  decision            TEXT    NOT NULL CHECK (decision IN ('ALLOW','DENY','APPROVAL_REQUIRED')),
  rule_id             TEXT    NOT NULL,
  reason              TEXT,
  required_capability TEXT    CHECK (required_capability IS NULL OR required_capability IN (
                        'PROTECTED_RECORD_WRITE','GRADE_FINALIZE','SCHEDULE_APPLY','CURRICULUM_IMPORT','CURRICULUM_PROMOTE',
                        'SKILL_INVOKE','MODEL_INFERENCE','FILESYSTEM_READ','FILESYSTEM_WRITE','NETWORK_EGRESS','SHELL_EXEC',
                        'DATA_EXPORT','DATA_DELETE','POLICY_MODIFY','MODEL_LOAD','LAB_ORCHESTRATE','CROSS_DOMAIN_READ')),
  approval_id         TEXT    REFERENCES approval_requests (id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: immutable row cannot be SET NULL
  request_id          TEXT,
  trace_id            TEXT,
  created_at          INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_policy_decisions_decided_at ON policy_decisions (decided_at);
CREATE INDEX idx_policy_decisions_resource   ON policy_decisions (resource_kind, resource_id);
CREATE INDEX idx_policy_decisions_approval   ON policy_decisions (approval_id);
CREATE TRIGGER trg_policy_decisions_no_update BEFORE UPDATE ON policy_decisions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: policy_decisions rows cannot be updated'); END;
CREATE TRIGGER trg_policy_decisions_no_delete BEFORE DELETE ON policy_decisions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: policy_decisions rows cannot be deleted'); END;

CREATE TABLE data_retention_rules (
  id             TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  table_name     TEXT    NOT NULL,
  data_domain    TEXT    NOT NULL CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification TEXT    CHECK (classification IS NULL OR classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  retention_days INTEGER NOT NULL CHECK (retention_days > 0),
  action         TEXT    NOT NULL CHECK (action IN ('PURGE','REDACT','ARCHIVE')),
  enabled        INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  revision       INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL
) STRICT;
CREATE UNIQUE INDEX idx_data_retention_rules_table ON data_retention_rules (table_name, data_domain);

-- Erasure workflow for personal content (A10). Requests are kept forever (tombstone anchor).
CREATE TABLE deletion_requests (
  id           TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  subject_kind TEXT    NOT NULL CHECK (subject_kind IN ('NOTE','ATTACHMENT','OPT_IN_REFLECTION','STUDY_SESSION')),
  subject_id   TEXT    NOT NULL,
  requested_by TEXT    NOT NULL,
  requested_at INTEGER NOT NULL,
  reason       TEXT,
  approval_id  TEXT    REFERENCES approval_requests (id)
               ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the approval token
  state        TEXT    NOT NULL CHECK (state IN ('REQUESTED','EXECUTED','REJECTED')),
  executed_by  TEXT,
  executed_at  INTEGER,
  report_json  TEXT    CHECK (report_json IS NULL OR json_valid(report_json)),
  revision     INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at   INTEGER NOT NULL,
  updated_at   INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_deletion_requests_subject ON deletion_requests (subject_kind, subject_id);
CREATE INDEX idx_deletion_requests_state   ON deletion_requests (state, requested_at);
CREATE TRIGGER trg_deletion_requests_terminal_no_update BEFORE UPDATE ON deletion_requests
WHEN OLD.state IN ('EXECUTED','REJECTED')
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: deletion request in a terminal state cannot be updated'); END;
CREATE TRIGGER trg_deletion_requests_no_delete BEFORE DELETE ON deletion_requests
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: deletion_requests rows cannot be deleted'); END;

-- One tombstone per erased row: keeps the pre-erasure content hash for audit continuity.
CREATE TABLE deletion_tombstones (
  id                    TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  deletion_request_id   TEXT    NOT NULL REFERENCES deletion_requests (id)
                        ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: tombstones are evidence of erasure
  table_name            TEXT    NOT NULL,
  row_id                TEXT    NOT NULL,
  content_sha256_before TEXT    NOT NULL CHECK (length(content_sha256_before) = 64),
  erased_at             INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_deletion_tombstones_row ON deletion_tombstones (table_name, row_id);
CREATE TRIGGER trg_deletion_tombstones_no_update BEFORE UPDATE ON deletion_tombstones
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: deletion_tombstones rows cannot be updated'); END;
CREATE TRIGGER trg_deletion_tombstones_no_delete BEFORE DELETE ON deletion_tombstones
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: deletion_tombstones rows cannot be deleted'); END;

-- Backup sets (M17). state mirrors g9_domain::BackupState.
CREATE TABLE backup_manifests (
  id                TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  created_at        INTEGER NOT NULL,
  app_version       TEXT    NOT NULL,
  schema_version    INTEGER NOT NULL CHECK (schema_version >= 0),
  page_count        INTEGER NOT NULL CHECK (page_count >= 0),
  sha256_of_file    TEXT    NOT NULL CHECK (length(sha256_of_file) = 64),
  state             TEXT    NOT NULL CHECK (state IN ('REQUESTED','SNAPSHOT_CREATED','CHECKED','RESTORE_TESTED',
                      'RETAINED','EXPIRED','FAILED')),
  path              TEXT,
  audit_anchor_seq  INTEGER,
  audit_anchor_hash TEXT    CHECK (audit_anchor_hash IS NULL OR length(audit_anchor_hash) = 64),
  verification_json TEXT    CHECK (verification_json IS NULL OR json_valid(verification_json)),
  restore_tested_at INTEGER,
  failure_reason    TEXT,
  revision          INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  updated_at        INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_backup_manifests_created_at ON backup_manifests (created_at);
CREATE INDEX idx_backup_manifests_state      ON backup_manifests (state);

-- Idempotent replay store (command pipeline step 2 / step 8).
CREATE TABLE idempotency_records (
  actor_id        TEXT    NOT NULL,
  idempotency_key TEXT    NOT NULL,
  request_sha256  TEXT    NOT NULL CHECK (length(request_sha256) = 64),
  outcome_json    TEXT    NOT NULL CHECK (json_valid(outcome_json)),
  created_at      INTEGER NOT NULL,
  PRIMARY KEY (actor_id, idempotency_key)
) STRICT;
CREATE INDEX idx_idempotency_records_created_at ON idempotency_records (created_at);

-- Significant-fact events (handoff §7). Immutable.
CREATE TABLE domain_events (
  seq            INTEGER PRIMARY KEY AUTOINCREMENT,
  id             TEXT    NOT NULL UNIQUE CHECK (length(id) BETWEEN 1 AND 128),
  kind           TEXT    NOT NULL,
  occurred_at    INTEGER NOT NULL,
  actor_id       TEXT    NOT NULL,
  actor_kind     TEXT    NOT NULL CHECK (actor_kind IN ('OPERATOR','SYSTEM','SKILL','MODEL')),
  actor_skill_id TEXT,
  trust          TEXT    NOT NULL CHECK (trust IN ('OPERATOR','SYSTEM_DETERMINISTIC','APPROVED_REFERENCE',
                   'IMPORTED_DOCUMENT','SKILL_PACKAGE','MODEL_OUTPUT','UNKNOWN')),
  schema_version TEXT    NOT NULL,
  correlation_id TEXT    NOT NULL,
  content_sha256 TEXT    CHECK (content_sha256 IS NULL OR length(content_sha256) = 64),
  payload_json   TEXT    NOT NULL CHECK (json_valid(payload_json)),
  payload_sha256 TEXT    NOT NULL CHECK (length(payload_sha256) = 64),
  data_domain    TEXT    NOT NULL CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification TEXT    NOT NULL CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET'))
) STRICT;
CREATE INDEX idx_domain_events_occurred_at ON domain_events (occurred_at);
CREATE INDEX idx_domain_events_kind        ON domain_events (kind, occurred_at);
CREATE INDEX idx_domain_events_correlation ON domain_events (correlation_id);
CREATE TRIGGER trg_domain_events_no_update BEFORE UPDATE ON domain_events
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: domain_events rows cannot be updated'); END;
CREATE TRIGGER trg_domain_events_no_delete BEFORE DELETE ON domain_events
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: domain_events rows cannot be deleted'); END;

-- Foreign-key columns not covered by an earlier index. SQLite evaluates ON DELETE RESTRICT / SET NULL on the
-- parent by scanning the child column, so every FK column carries a leading index (checked by
-- every_foreign_key_column_is_index_covered).
CREATE INDEX idx_deletion_requests_approval_id ON deletion_requests (approval_id);
CREATE INDEX idx_deletion_tombstones_deletion_request_id ON deletion_tombstones (deletion_request_id);
