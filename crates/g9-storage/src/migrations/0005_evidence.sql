-- 0005_evidence: notes, attachments, evidence items, provenance and classification (M05, ARCHITECTURE §6 privacy).
-- Personal content supports erasure through deletion_requests: content columns are redacted in place,
-- `erased` is set and a deletion_tombstones row keeps the pre-erasure hash. Rows are never deleted.

CREATE TABLE notes (
  id                 TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  title              TEXT    NOT NULL,
  body_markdown      TEXT    NOT NULL,
  body_sha256        TEXT    NOT NULL CHECK (length(body_sha256) = 64),
  kind               TEXT    NOT NULL CHECK (kind IN ('JOURNAL','LAB_LOG','REFLECTION','REPORT','OTHER')),
  session_id         TEXT    REFERENCES study_sessions (id)
                     ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link
  objective_id       TEXT    REFERENCES daily_objectives (id)
                     ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link
  source_kind        TEXT    NOT NULL CHECK (source_kind IN ('OPERATOR_INPUT','SYSTEM_GENERATED','CURRICULUM_MANIFEST',
                       'NOTE','ATTACHMENT','REFERENCE_DOCUMENT','MODEL_RESPONSE','SKILL_PACKAGE','ASSESSMENT_SUBMISSION',
                       'EXTERNAL_FEED')),
  trust              TEXT    NOT NULL CHECK (trust IN ('OPERATOR','SYSTEM_DETERMINISTIC','APPROVED_REFERENCE',
                       'IMPORTED_DOCUMENT','SKILL_PACKAGE','MODEL_OUTPUT','UNKNOWN')),
  data_domain        TEXT    NOT NULL DEFAULT 'LEARNING'
                     CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification     TEXT    NOT NULL DEFAULT 'PRIVATE'
                     CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  erased             INTEGER NOT NULL DEFAULT 0 CHECK (erased IN (0, 1)),
  erased_at          INTEGER,
  erasure_request_id TEXT    REFERENCES deletion_requests (id)
                     ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: the erasure anchor must survive
  revision           INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at         INTEGER NOT NULL,
  updated_at         INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_notes_session   ON notes (session_id);
CREATE INDEX idx_notes_objective ON notes (objective_id);
CREATE INDEX idx_notes_created   ON notes (created_at);

CREATE TABLE note_versions (
  id            TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  note_id       TEXT    NOT NULL REFERENCES notes (id)
                ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: version history is evidence; erasure redacts
  version_no    INTEGER NOT NULL CHECK (version_no >= 1),
  title         TEXT    NOT NULL,
  body_markdown TEXT    NOT NULL,
  body_sha256   TEXT    NOT NULL CHECK (length(body_sha256) = 64),
  author_id     TEXT    NOT NULL,
  erased        INTEGER NOT NULL DEFAULT 0 CHECK (erased IN (0, 1)),
  created_at    INTEGER NOT NULL,
  UNIQUE (note_id, version_no)
) STRICT;
CREATE INDEX idx_note_versions_note ON note_versions (note_id);

CREATE TABLE attachments (
  id                 TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  note_id            TEXT    REFERENCES notes (id)
                     ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: an attachment may stand alone
  original_filename  TEXT    NOT NULL,
  media_type         TEXT    NOT NULL,
  size_bytes         INTEGER NOT NULL CHECK (size_bytes >= 0),
  content_sha256     TEXT    NOT NULL CHECK (length(content_sha256) = 64),
  storage_ref        TEXT    NOT NULL,
  source_kind        TEXT    NOT NULL CHECK (source_kind IN ('OPERATOR_INPUT','SYSTEM_GENERATED','CURRICULUM_MANIFEST',
                       'NOTE','ATTACHMENT','REFERENCE_DOCUMENT','MODEL_RESPONSE','SKILL_PACKAGE','ASSESSMENT_SUBMISSION',
                       'EXTERNAL_FEED')),
  trust              TEXT    NOT NULL CHECK (trust IN ('OPERATOR','SYSTEM_DETERMINISTIC','APPROVED_REFERENCE',
                       'IMPORTED_DOCUMENT','SKILL_PACKAGE','MODEL_OUTPUT','UNKNOWN')),
  data_domain        TEXT    NOT NULL DEFAULT 'LEARNING'
                     CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification     TEXT    NOT NULL DEFAULT 'PRIVATE'
                     CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  erased             INTEGER NOT NULL DEFAULT 0 CHECK (erased IN (0, 1)),
  erased_at          INTEGER,
  erasure_request_id TEXT    REFERENCES deletion_requests (id)
                     ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: the erasure anchor must survive
  revision           INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at         INTEGER NOT NULL,
  updated_at         INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_attachments_note    ON attachments (note_id);
CREATE INDEX idx_attachments_sha     ON attachments (content_sha256);
CREATE INDEX idx_attachments_created ON attachments (created_at);

CREATE TABLE evidence_items (
  id                  TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  kind                TEXT    NOT NULL CHECK (kind IN ('NOTE','ATTACHMENT','SESSION','ATTEMPT_SUBMISSION','CHECKER_OUTPUT',
                        'REVIEWER_COMMENT','EXTERNAL')),
  note_id             TEXT    REFERENCES notes (id)
                      ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: evidence keeps its hash even if the note goes
  attachment_id       TEXT    REFERENCES attachments (id)
                      ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: same
  session_id          TEXT    REFERENCES study_sessions (id)
                      ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link
  summary             TEXT    NOT NULL,
  content_sha256      TEXT    NOT NULL CHECK (length(content_sha256) = 64),
  trust               TEXT    NOT NULL CHECK (trust IN ('OPERATOR','SYSTEM_DETERMINISTIC','APPROVED_REFERENCE',
                        'IMPORTED_DOCUMENT','SKILL_PACKAGE','MODEL_OUTPUT','UNKNOWN')),
  provenance_json     TEXT    NOT NULL CHECK (json_valid(provenance_json)),
  data_domain         TEXT    NOT NULL DEFAULT 'LEARNING'
                      CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification      TEXT    NOT NULL DEFAULT 'PRIVATE'
                      CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  invalidated_at      INTEGER,
  invalidation_reason TEXT,
  revision            INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at          INTEGER NOT NULL,
  updated_at          INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_evidence_items_note       ON evidence_items (note_id);
CREATE INDEX idx_evidence_items_attachment ON evidence_items (attachment_id);
CREATE INDEX idx_evidence_items_session    ON evidence_items (session_id);
CREATE INDEX idx_evidence_items_created    ON evidence_items (created_at);

CREATE TABLE artifact_links (
  id               TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  evidence_item_id TEXT    NOT NULL REFERENCES evidence_items (id)
                   ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: links are part of the evidence graph
  target_kind      TEXT    NOT NULL CHECK (target_kind IN (
                     'CURRICULUM','CURRICULUM_VERSION','CALENDAR_EVENT','SCHEDULE_PROPOSAL','STUDY_SESSION',
                     'TIME_INTERVAL','NOTE','ATTACHMENT','EVIDENCE_ITEM','ASSESSMENT_DEFINITION','ATTEMPT',
                     'GRADE_DECISION','COMPETENCY','OBSERVATION','LEARNER_HYPOTHESIS','REVIEW_ITEM','INTERVENTION',
                     'EXPERIMENT','GOAL','PROJECT','MILESTONE','DECISION','RESEARCH_SOURCE','KNOWLEDGE_DOCUMENT',
                     'MODEL','SKILL','SKILL_INVOCATION','APPROVAL','BACKUP','POLICY','EXPORT','FILESYSTEM_PATH',
                     'NETWORK_HOST','PROCESS','APP_SETTING','AUDIT_METADATA')),
  target_id        TEXT    NOT NULL,
  relation         TEXT    NOT NULL CHECK (relation IN ('SUPPORTS','PRODUCED_BY','REFERENCES')),
  created_at       INTEGER NOT NULL,
  UNIQUE (evidence_item_id, target_kind, target_id, relation)
) STRICT;
CREATE INDEX idx_artifact_links_target ON artifact_links (target_kind, target_id);

-- Provenance records for any row (g9_domain::Provenance). Append-only.
CREATE TABLE source_provenance (
  id             TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  record_table   TEXT    NOT NULL,
  record_id      TEXT    NOT NULL,
  source_kind    TEXT    NOT NULL CHECK (source_kind IN ('OPERATOR_INPUT','SYSTEM_GENERATED','CURRICULUM_MANIFEST',
                   'NOTE','ATTACHMENT','REFERENCE_DOCUMENT','MODEL_RESPONSE','SKILL_PACKAGE','ASSESSMENT_SUBMISSION',
                   'EXTERNAL_FEED')),
  source_ref     TEXT    NOT NULL,
  trust          TEXT    NOT NULL CHECK (trust IN ('OPERATOR','SYSTEM_DETERMINISTIC','APPROVED_REFERENCE',
                   'IMPORTED_DOCUMENT','SKILL_PACKAGE','MODEL_OUTPUT','UNKNOWN')),
  content_sha256 TEXT    CHECK (content_sha256 IS NULL OR length(content_sha256) = 64),
  version        TEXT,
  recorded_by    TEXT    NOT NULL,
  recorded_at    INTEGER NOT NULL,
  created_at     INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_source_provenance_record ON source_provenance (record_table, record_id);
CREATE TRIGGER trg_source_provenance_no_update BEFORE UPDATE ON source_provenance
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: source_provenance rows cannot be updated'); END;
CREATE TRIGGER trg_source_provenance_no_delete BEFORE DELETE ON source_provenance
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: source_provenance rows cannot be deleted'); END;

-- Classification history for any row; the latest row wins; the column on the row is the cached value.
CREATE TABLE content_classifications (
  id             TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  record_table   TEXT    NOT NULL,
  record_id      TEXT    NOT NULL,
  data_domain    TEXT    NOT NULL CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification TEXT    NOT NULL CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  classified_by  TEXT    NOT NULL,
  classified_at  INTEGER NOT NULL,
  reason         TEXT,
  created_at     INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_content_classifications_record ON content_classifications (record_table, record_id, classified_at);
