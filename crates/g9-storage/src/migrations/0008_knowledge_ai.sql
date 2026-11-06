-- 0008_knowledge_ai: research sources, knowledge documents/chunks/claims, model and skill registries,
-- skill invocations and evaluations (M10/M11/M15; ARCHITECTURE §10, handoff §7 "Knowledge/AI", §14).
-- Rules enforced here:
--   * source_registry.state mirrors g9_domain::SourceState; trust is APPROVED_REFERENCE only in that state.
--   * knowledge content is data (trust IMPORTED_DOCUMENT / APPROVED_REFERENCE), never OPERATOR/SYSTEM.
--   * claim_versions separate "source claim" from "validated truth" (handoff §14); versions are immutable.
--   * model_registry rows carry a checksum; a model is loadable only when state = 'APPROVED' (operator-only).
--   * skill_invocations are metadata only: hashes, counts, latency, validation outcome — never raw prompts.

CREATE TABLE source_registry (
  id                 TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  domain_or_resource TEXT    NOT NULL,
  resource_class     TEXT    NOT NULL CHECK (resource_class IN ('STANDARD','DOCUMENTATION','DATASET','TOOL','COURSE','OTHER')),
  allowed_data       TEXT    NOT NULL,
  frequency          TEXT    NOT NULL,
  license            TEXT    NOT NULL,
  version            TEXT    NOT NULL,
  cache_policy       TEXT    NOT NULL DEFAULT 'LOCAL_ONLY' CHECK (cache_policy IN ('LOCAL_ONLY','NO_CACHE')),
  state              TEXT    NOT NULL CHECK (state IN ('UNTRUSTED','QUARANTINED','EVALUATED','APPROVED_REFERENCE','SUPERSEDED','REVOKED')),
  trust              TEXT    NOT NULL CHECK (trust IN ('APPROVED_REFERENCE','IMPORTED_DOCUMENT','UNKNOWN')),
  reviewed_by        TEXT,
  reviewed_at        INTEGER,
  superseded_by      TEXT    REFERENCES source_registry (id)
                     ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: supersession chain is history
  data_domain        TEXT    NOT NULL DEFAULT 'LEARNING'
                     CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification     TEXT    NOT NULL DEFAULT 'INTERNAL'
                     CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision           INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at         INTEGER NOT NULL,
  updated_at         INTEGER NOT NULL,
  -- trust is derived from the state machine (g9-evidence::sources)
  CHECK ((state = 'APPROVED_REFERENCE') = (trust = 'APPROVED_REFERENCE')),
  CHECK (state <> 'APPROVED_REFERENCE' OR (reviewed_by IS NOT NULL AND reviewed_at IS NOT NULL)),
  CHECK (id <> superseded_by)
) STRICT;
CREATE INDEX idx_source_registry_state ON source_registry (state);

CREATE TABLE knowledge_documents (
  id             TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  source_id      TEXT    NOT NULL REFERENCES source_registry (id)
                 ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: documents remain available for reproducibility (§14)
  title          TEXT    NOT NULL,
  uri            TEXT,
  version        TEXT    NOT NULL,
  media_type     TEXT    NOT NULL,
  byte_len       INTEGER NOT NULL CHECK (byte_len >= 0),
  content_sha256 TEXT    NOT NULL CHECK (length(content_sha256) = 64),
  storage_ref    TEXT,
  fetched_at     INTEGER NOT NULL,
  fetched_by     TEXT    NOT NULL,
  review_state   TEXT    NOT NULL DEFAULT 'QUARANTINED' CHECK (review_state IN ('QUARANTINED','REVIEWED','APPROVED','SUPERSEDED','REVOKED')),
  trust          TEXT    NOT NULL CHECK (trust IN ('APPROVED_REFERENCE','IMPORTED_DOCUMENT','UNKNOWN')),
  data_domain    TEXT    NOT NULL DEFAULT 'LEARNING'
                 CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification TEXT    NOT NULL DEFAULT 'INTERNAL'
                 CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision       INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL,
  UNIQUE (source_id, content_sha256),
  CHECK (review_state <> 'APPROVED' OR trust = 'APPROVED_REFERENCE')
) STRICT;
CREATE INDEX idx_knowledge_documents_source  ON knowledge_documents (source_id);
CREATE INDEX idx_knowledge_documents_fetched ON knowledge_documents (fetched_at);
CREATE INDEX idx_knowledge_documents_sha     ON knowledge_documents (content_sha256);

-- Inspectable chunks cited by retrieval answers (handoff §14). Immutable: a changed document is a new document.
CREATE TABLE knowledge_chunks (
  id             TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  document_id    TEXT    NOT NULL REFERENCES knowledge_documents (id)
                 ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: claims cite chunks
  ordinal        INTEGER NOT NULL CHECK (ordinal >= 0),
  content        TEXT    NOT NULL,
  content_sha256 TEXT    NOT NULL CHECK (length(content_sha256) = 64),
  byte_len       INTEGER NOT NULL CHECK (byte_len >= 0),
  token_estimate INTEGER CHECK (token_estimate IS NULL OR token_estimate >= 0),
  locator        TEXT,
  created_at     INTEGER NOT NULL,
  UNIQUE (document_id, ordinal)
) STRICT;
CREATE INDEX idx_knowledge_chunks_document ON knowledge_chunks (document_id, ordinal);
CREATE TRIGGER trg_knowledge_chunks_no_update BEFORE UPDATE ON knowledge_chunks
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: knowledge_chunks rows cannot be updated; ingest a new document version'); END;

CREATE TABLE knowledge_claims (
  id                 TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  topic              TEXT    NOT NULL,
  current_version_no INTEGER NOT NULL DEFAULT 1 CHECK (current_version_no >= 1),
  status             TEXT    NOT NULL DEFAULT 'SOURCE_CLAIM' CHECK (status IN ('SOURCE_CLAIM','VALIDATED','DISPUTED','RETIRED')),
  data_domain        TEXT    NOT NULL DEFAULT 'LEARNING'
                     CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification     TEXT    NOT NULL DEFAULT 'INTERNAL'
                     CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision           INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at         INTEGER NOT NULL,
  updated_at         INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_knowledge_claims_status ON knowledge_claims (status, topic);

-- Each version states the claim text, the chunk that supports it and whether an operator validated it.
CREATE TABLE claim_versions (
  id           TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  claim_id     TEXT    NOT NULL REFERENCES knowledge_claims (id)
               ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: versions are history
  version_no   INTEGER NOT NULL CHECK (version_no >= 1),
  statement    TEXT    NOT NULL,
  chunk_id     TEXT    REFERENCES knowledge_chunks (id)
               ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: the citation must stay inspectable
  trust        TEXT    NOT NULL CHECK (trust IN ('OPERATOR','APPROVED_REFERENCE','IMPORTED_DOCUMENT','MODEL_OUTPUT','UNKNOWN')),
  validated_by TEXT,
  validated_at INTEGER,
  created_by   TEXT    NOT NULL,
  created_at   INTEGER NOT NULL,
  UNIQUE (claim_id, version_no),
  -- only an operator validation turns a source claim into validated truth
  CHECK ((validated_by IS NULL) = (validated_at IS NULL))
) STRICT;
CREATE INDEX idx_claim_versions_claim ON claim_versions (claim_id, version_no);
CREATE INDEX idx_claim_versions_chunk ON claim_versions (chunk_id);
CREATE TRIGGER trg_claim_versions_no_update BEFORE UPDATE ON claim_versions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: claim_versions rows cannot be updated; insert a new version'); END;
CREATE TRIGGER trg_claim_versions_no_delete BEFORE DELETE ON claim_versions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: claim_versions rows cannot be deleted'); END;

-- Local model registry (ARCHITECTURE §10). Loading requires Capability::ModelLoad and a checksum match.
CREATE TABLE model_registry (
  model_id           TEXT    PRIMARY KEY NOT NULL CHECK (length(model_id) BETWEEN 1 AND 128),
  display_name       TEXT    NOT NULL,
  architecture       TEXT    NOT NULL,
  weights_source     TEXT    NOT NULL,
  license            TEXT    NOT NULL,
  quantization       TEXT    NOT NULL,
  checksum           TEXT    NOT NULL CHECK (length(checksum) = 64),
  context_length     INTEGER NOT NULL CHECK (context_length > 0),
  tokenizer_template TEXT    NOT NULL,
  runtime_version    TEXT    NOT NULL,
  state              TEXT    NOT NULL DEFAULT 'REGISTERED' CHECK (state IN ('REGISTERED','APPROVED','DISABLED')),
  approved_by        TEXT,
  approved_at        INTEGER,
  approval_id        TEXT    REFERENCES approval_requests (id)
                     ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the approval token
  data_domain        TEXT    NOT NULL DEFAULT 'SYSTEM'
                     CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification     TEXT    NOT NULL DEFAULT 'INTERNAL'
                     CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision           INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at         INTEGER NOT NULL,
  updated_at         INTEGER NOT NULL,
  CHECK (state <> 'APPROVED' OR (approved_by IS NOT NULL AND approved_at IS NOT NULL))
) STRICT;
CREATE INDEX idx_model_registry_state ON model_registry (state);

-- Benchmark runs per docs/evaluations/model-benchmark-protocol.md. Immutable.
CREATE TABLE model_evals (
  id               TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  model_id         TEXT    NOT NULL REFERENCES model_registry (model_id)
                   ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evaluation evidence
  protocol_version TEXT    NOT NULL,
  eval_set         TEXT    NOT NULL,
  metrics_json     TEXT    NOT NULL CHECK (json_valid(metrics_json)),
  report_sha256    TEXT    CHECK (report_sha256 IS NULL OR length(report_sha256) = 64),
  run_by           TEXT    NOT NULL,
  run_at           INTEGER NOT NULL,
  created_at       INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_model_evals_model ON model_evals (model_id, run_at);
CREATE TRIGGER trg_model_evals_no_update BEFORE UPDATE ON model_evals
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: model_evals rows cannot be updated'); END;
CREATE TRIGGER trg_model_evals_no_delete BEFORE DELETE ON model_evals
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: model_evals rows cannot be deleted'); END;

-- The 37 logical skills (E01–E14, T01–T08, L01–L06, P01–P06, K01–K03). skill_id is the Agent Skills name.
CREATE TABLE skill_registry (
  skill_id              TEXT    PRIMARY KEY NOT NULL CHECK (length(skill_id) BETWEEN 1 AND 64),
  code                  TEXT    NOT NULL UNIQUE CHECK (code GLOB '[ETLPK][0-9][0-9]'),
  family                TEXT    NOT NULL CHECK (family IN ('ENGINEERING','TEACHING','LEARNING','PERFORMANCE','KNOWLEDGE')),
  title                 TEXT    NOT NULL,
  package_dir           TEXT    NOT NULL,
  current_version_label TEXT,
  state                 TEXT    NOT NULL DEFAULT 'REGISTERED' CHECK (state IN ('REGISTERED','VALIDATED','PROMOTED','DISABLED')),
  revision              INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at            INTEGER NOT NULL,
  updated_at            INTEGER NOT NULL,
  CHECK (family = CASE substr(code, 1, 1)
                    WHEN 'E' THEN 'ENGINEERING' WHEN 'T' THEN 'TEACHING' WHEN 'L' THEN 'LEARNING'
                    WHEN 'P' THEN 'PERFORMANCE' ELSE 'KNOWLEDGE' END)
) STRICT;

-- Package hashes per version (supply-chain rule: a mismatched hash is refused at runtime). Immutable.
CREATE TABLE skill_versions (
  id                   TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  skill_id             TEXT    NOT NULL REFERENCES skill_registry (skill_id)
                       ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: invocations reference versions
  version_label        TEXT    NOT NULL,
  package_sha256       TEXT    NOT NULL CHECK (length(package_sha256) = 64),
  skill_md_sha256      TEXT    NOT NULL CHECK (length(skill_md_sha256) = 64),
  input_schema_sha256  TEXT    NOT NULL CHECK (length(input_schema_sha256) = 64),
  output_schema_sha256 TEXT    NOT NULL CHECK (length(output_schema_sha256) = 64),
  budget_json          TEXT    NOT NULL CHECK (json_valid(budget_json)),
  allowed_tool_ids     TEXT    NOT NULL DEFAULT '[]' CHECK (json_valid(allowed_tool_ids) AND json_type(allowed_tool_ids) = 'array'),
  registered_by        TEXT    NOT NULL,
  registered_at        INTEGER NOT NULL,
  created_at           INTEGER NOT NULL,
  UNIQUE (skill_id, version_label)
) STRICT;
CREATE INDEX idx_skill_versions_skill ON skill_versions (skill_id);
CREATE TRIGGER trg_skill_versions_no_update BEFORE UPDATE ON skill_versions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: skill_versions rows cannot be updated; register a new version'); END;
CREATE TRIGGER trg_skill_versions_no_delete BEFORE DELETE ON skill_versions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: skill_versions rows cannot be deleted'); END;

-- Per-invocation record (ARCHITECTURE §10 "Context assembly"): metadata only, no raw prompts or outputs.
CREATE TABLE skill_invocations (
  id                  TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  skill_id            TEXT    NOT NULL REFERENCES skill_registry (skill_id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: invocation history is evidence
  skill_version_id    TEXT    NOT NULL REFERENCES skill_versions (id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: same
  actor_id            TEXT    NOT NULL,
  actor_kind          TEXT    NOT NULL CHECK (actor_kind IN ('OPERATOR','SYSTEM','SKILL','MODEL')),
  intent              TEXT    NOT NULL,
  scope_domain        TEXT    NOT NULL CHECK (scope_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  scope_mode          TEXT    NOT NULL CHECK (scope_mode IN ('STUDY','LAB','EXAM','REVIEW','EXECUTIVE','RESEARCH','MAINTENANCE')),
  data_classification TEXT    NOT NULL CHECK (data_classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  state               TEXT    NOT NULL CHECK (state IN ('REQUESTED','PREFLIGHT','AUTHORIZED','RUNNING','VALIDATING','COMPLETED',
                        'ERROR','DENIED','TIMED_OUT','CANCELLED')),
  model_id            TEXT    REFERENCES model_registry (model_id)
                      ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational; the record keeps model_id_label
  model_id_label      TEXT,
  template_version    TEXT,
  input_sha256        TEXT    NOT NULL CHECK (length(input_sha256) = 64),
  output_sha256       TEXT    CHECK (output_sha256 IS NULL OR length(output_sha256) = 64),
  output_status       TEXT    CHECK (output_status IS NULL OR output_status IN ('COMPLETED','ABSTAINED','ERROR')),
  validation_outcome  TEXT    NOT NULL DEFAULT 'NOT_RUN' CHECK (validation_outcome IN ('NOT_RUN','VALID','INVALID')),
  tokens_used         INTEGER NOT NULL DEFAULT 0 CHECK (tokens_used >= 0),
  seconds_used        INTEGER NOT NULL DEFAULT 0 CHECK (seconds_used >= 0),
  tool_calls_used     INTEGER NOT NULL DEFAULT 0 CHECK (tool_calls_used >= 0),
  latency_ms          INTEGER CHECK (latency_ms IS NULL OR latency_ms >= 0),
  policy_decision_id  TEXT    REFERENCES policy_decisions (id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: the preflight decision is evidence
  approval_id         TEXT    REFERENCES approval_requests (id)
                      ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the token
  idempotency_key     TEXT    NOT NULL,
  trace_id            TEXT    NOT NULL,
  error_category      TEXT    CHECK (error_category IS NULL OR error_category IN ('INVALID_INPUT','UNAUTHORIZED','APPROVAL_REQUIRED',
                        'STALE_REVISION','PRECONDITION_FAILED','NOT_FOUND','CONFLICT','RESOURCE_UNAVAILABLE','TIMEOUT',
                        'CANCELLED','RETRYABLE_STORAGE','DATA_CORRUPTION','INTERNAL')),
  requested_at        INTEGER NOT NULL,
  finished_at         INTEGER CHECK (finished_at IS NULL OR finished_at >= requested_at),
  revision            INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at          INTEGER NOT NULL,
  updated_at          INTEGER NOT NULL,
  UNIQUE (actor_id, idempotency_key),
  -- a COMPLETED invocation has a validated output; anything else never carries a VALID outcome
  CHECK (state <> 'COMPLETED' OR (output_sha256 IS NOT NULL AND validation_outcome = 'VALID')),
  -- no skill/model actor can invoke in EXAM mode with SECRET data
  CHECK (NOT (scope_mode = 'EXAM' AND data_classification = 'SECRET'))
) STRICT;
CREATE INDEX idx_skill_invocations_skill     ON skill_invocations (skill_id, requested_at);
CREATE INDEX idx_skill_invocations_state     ON skill_invocations (state);
CREATE INDEX idx_skill_invocations_trace     ON skill_invocations (trace_id);
CREATE INDEX idx_skill_invocations_requested ON skill_invocations (requested_at);
-- terminal invocation states are immutable (no retry loops inside one invocation)
CREATE TRIGGER trg_skill_invocations_terminal_no_update BEFORE UPDATE ON skill_invocations
WHEN OLD.state IN ('COMPLETED','ERROR','DENIED','TIMED_OUT','CANCELLED')
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: skill invocation in a terminal state cannot be updated'); END;
CREATE TRIGGER trg_skill_invocations_no_delete BEFORE DELETE ON skill_invocations
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: skill_invocations rows cannot be deleted (retention sweep is a separate approved command)'); END;

-- Three-pass evaluation results per skill version (handoff Appendix B). Immutable.
CREATE TABLE skill_evals (
  id               TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  skill_version_id TEXT    NOT NULL REFERENCES skill_versions (id)
                   ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: promotion evidence
  pass_kind        TEXT    NOT NULL CHECK (pass_kind IN ('POSITIVE','NEGATIVE','ADVERSARIAL','ACCEPTANCE')),
  total_cases      INTEGER NOT NULL CHECK (total_cases >= 0),
  passed_cases     INTEGER NOT NULL CHECK (passed_cases >= 0 AND passed_cases <= total_cases),
  failed_cases     INTEGER NOT NULL CHECK (failed_cases >= 0 AND passed_cases + failed_cases = total_cases),
  report_sha256    TEXT    CHECK (report_sha256 IS NULL OR length(report_sha256) = 64),
  run_by           TEXT    NOT NULL,
  run_at           INTEGER NOT NULL,
  created_at       INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_skill_evals_version ON skill_evals (skill_version_id, run_at);
CREATE TRIGGER trg_skill_evals_no_update BEFORE UPDATE ON skill_evals
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: skill_evals rows cannot be updated'); END;
CREATE TRIGGER trg_skill_evals_no_delete BEFORE DELETE ON skill_evals
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: skill_evals rows cannot be deleted'); END;

-- Foreign-key columns not covered by an earlier index. SQLite evaluates ON DELETE RESTRICT / SET NULL on the
-- parent by scanning the child column, so every FK column carries a leading index (checked by
-- every_foreign_key_column_is_index_covered).
CREATE INDEX idx_source_registry_superseded_by ON source_registry (superseded_by);
CREATE INDEX idx_model_registry_approval_id ON model_registry (approval_id);
CREATE INDEX idx_skill_invocations_skill_version_id ON skill_invocations (skill_version_id);
CREATE INDEX idx_skill_invocations_model_id ON skill_invocations (model_id);
CREATE INDEX idx_skill_invocations_policy_decision_id ON skill_invocations (policy_decision_id);
CREATE INDEX idx_skill_invocations_approval_id ON skill_invocations (approval_id);
