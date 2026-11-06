-- 0006_assessment: definitions, versions with sealed keys, rubrics, attempts, grading and exposure (M07, A04).
-- Every row here is evidentiary: all FKs RESTRICT; criterion_results are immutable; attempts freeze at
-- APPROVED_FINAL; grade_decisions freeze when is_final = 1; corrections are superseding rows.

CREATE TABLE assessment_definitions (
  id              TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  code            TEXT    NOT NULL UNIQUE,
  title           TEXT    NOT NULL,
  kind            TEXT    NOT NULL CHECK (kind IN ('BLIND_PRACTICAL','QUIZ','REPORT','CAPSTONE','TRANSFER_CHECK')),
  objective_id    TEXT    REFERENCES daily_objectives (id)
                  ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the curriculum
  training_day_id TEXT    REFERENCES training_days (id)
                  ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the curriculum
  competency_id   TEXT    REFERENCES competencies (id)
                  ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: an assessed competency cannot vanish
  mode            TEXT    NOT NULL DEFAULT 'EXAM' CHECK (mode IN ('STUDY','LAB','EXAM','REVIEW','EXECUTIVE','RESEARCH','MAINTENANCE')),
  data_domain     TEXT    NOT NULL DEFAULT 'LEARNING'
                  CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification  TEXT    NOT NULL DEFAULT 'INTERNAL'
                  CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision        INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_assessment_definitions_objective  ON assessment_definitions (objective_id);
CREATE INDEX idx_assessment_definitions_competency ON assessment_definitions (competency_id);

-- sealed_key_ref is a content-addressed reference to the answer key (classification SECRET); the key
-- bytes never live in this table and never reach EXAM-mode model context.
CREATE TABLE assessment_versions (
  id                 TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  definition_id      TEXT    NOT NULL REFERENCES assessment_definitions (id)
                     ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: versions are history
  version_label      TEXT    NOT NULL,
  task_statement     TEXT    NOT NULL,
  task_sha256        TEXT    NOT NULL CHECK (length(task_sha256) = 64),
  sealed_key_ref     TEXT,
  sealed_key_sha256  TEXT    CHECK (sealed_key_sha256 IS NULL OR length(sealed_key_sha256) = 64),
  key_classification TEXT    NOT NULL DEFAULT 'SECRET' CHECK (key_classification = 'SECRET'),
  pass_threshold     INTEGER NOT NULL DEFAULT 80 CHECK (pass_threshold BETWEEN 0 AND 100),
  created_by         TEXT    NOT NULL,
  created_at         INTEGER NOT NULL,
  UNIQUE (definition_id, version_label)
) STRICT;
CREATE INDEX idx_assessment_versions_definition ON assessment_versions (definition_id);

CREATE TABLE rubrics (
  id                    TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  assessment_version_id TEXT    NOT NULL REFERENCES assessment_versions (id)
                        ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: grading history depends on it
  title                 TEXT    NOT NULL,
  version_label         TEXT    NOT NULL,
  total_weight          INTEGER NOT NULL CHECK (total_weight = 100),
  created_at            INTEGER NOT NULL,
  UNIQUE (assessment_version_id, version_label)
) STRICT;
CREATE INDEX idx_rubrics_version ON rubrics (assessment_version_id);

CREATE TABLE rubric_criteria (
  id                TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  rubric_id         TEXT    NOT NULL REFERENCES rubrics (id)
                    ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: criterion_results reference criteria
  code              TEXT    NOT NULL,
  title             TEXT    NOT NULL,
  ordinal           INTEGER NOT NULL CHECK (ordinal >= 1),
  weight            INTEGER NOT NULL CHECK (weight BETWEEN 0 AND 100),
  machine_checkable INTEGER NOT NULL DEFAULT 0 CHECK (machine_checkable IN (0, 1)),
  critical_gate     INTEGER NOT NULL DEFAULT 0 CHECK (critical_gate IN (0, 1)),
  created_at        INTEGER NOT NULL,
  UNIQUE (rubric_id, code),
  UNIQUE (rubric_id, ordinal)
) STRICT;

CREATE TABLE attempts (
  id                    TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  assessment_version_id TEXT    NOT NULL REFERENCES assessment_versions (id)
                        ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: attempts are evidence
  session_id            TEXT    REFERENCES study_sessions (id)
                        ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link
  retake_of             TEXT    REFERENCES attempts (id)
                        ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: a retake links to its original forever
  state                 TEXT    NOT NULL CHECK (state IN ('AVAILABLE','STARTED','SUBMITTED','EVIDENCE_REVIEW',
                          'GRADED_PROVISIONAL','APPROVED_FINAL','VOIDED')),
  mode                  TEXT    NOT NULL DEFAULT 'EXAM' CHECK (mode IN ('STUDY','LAB','EXAM','REVIEW','EXECUTIVE','RESEARCH','MAINTENANCE')),
  assistance_level      TEXT    NOT NULL DEFAULT 'UNAIDED' CHECK (assistance_level IN ('UNAIDED','PROMPTED','HINTED','GUIDED','SHOWN')),
  hint_count            INTEGER NOT NULL DEFAULT 0 CHECK (hint_count >= 0),
  started_at            INTEGER,
  submitted_at          INTEGER,
  finalized_at          INTEGER,
  submission_sha256     TEXT    CHECK (submission_sha256 IS NULL OR length(submission_sha256) = 64),
  data_domain           TEXT    NOT NULL DEFAULT 'LEARNING'
                        CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification        TEXT    NOT NULL DEFAULT 'PRIVATE'
                        CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision              INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at            INTEGER NOT NULL,
  updated_at            INTEGER NOT NULL,
  CHECK (id <> retake_of),
  CHECK (state <> 'APPROVED_FINAL' OR finalized_at IS NOT NULL)
) STRICT;
CREATE INDEX idx_attempts_version   ON attempts (assessment_version_id);
CREATE INDEX idx_attempts_session   ON attempts (session_id);
CREATE INDEX idx_attempts_retake_of ON attempts (retake_of);
CREATE INDEX idx_attempts_state     ON attempts (state);
CREATE INDEX idx_attempts_started   ON attempts (started_at);
-- A04: a finalized attempt has no outgoing edges; a retake is a new row.
CREATE TRIGGER trg_attempts_final_no_update BEFORE UPDATE ON attempts
WHEN OLD.state = 'APPROVED_FINAL'
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: finalized attempt cannot be updated; create a retake attempt'); END;
CREATE TRIGGER trg_attempts_no_delete BEFORE DELETE ON attempts
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: attempts rows cannot be deleted'); END;

CREATE TABLE attempt_evidence (
  attempt_id       TEXT    NOT NULL REFERENCES attempts (id)
                   ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidence binding
  evidence_item_id TEXT    NOT NULL REFERENCES evidence_items (id)
                   ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidence binding
  role             TEXT    NOT NULL CHECK (role IN ('SUBMISSION','SUPPORTING','REVIEWER')),
  added_at         INTEGER NOT NULL,
  PRIMARY KEY (attempt_id, evidence_item_id)
) STRICT;
CREATE INDEX idx_attempt_evidence_item ON attempt_evidence (evidence_item_id);

-- Per-criterion scores. Append-only; corrections supersede.
CREATE TABLE criterion_results (
  id            TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  attempt_id    TEXT    NOT NULL REFERENCES attempts (id)
                ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary
  criterion_id  TEXT    NOT NULL REFERENCES rubric_criteria (id)
                ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary
  score         INTEGER NOT NULL CHECK (score BETWEEN 0 AND 100),
  source        TEXT    NOT NULL CHECK (source IN ('DETERMINISTIC_CHECKER','HUMAN_REVIEWER','MODEL_ADVISORY')),
  evidence_refs TEXT    NOT NULL DEFAULT '[]' CHECK (json_valid(evidence_refs) AND json_type(evidence_refs) = 'array'),
  rationale     TEXT,
  scored_by     TEXT    NOT NULL,
  scored_at     INTEGER NOT NULL,
  supersedes    TEXT    REFERENCES criterion_results (id)
                ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: supersession chain is evidence
  created_at    INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_criterion_results_attempt   ON criterion_results (attempt_id);
CREATE INDEX idx_criterion_results_criterion ON criterion_results (criterion_id);
CREATE INDEX idx_criterion_results_scored_at ON criterion_results (scored_at);
CREATE TRIGGER trg_criterion_results_no_update BEFORE UPDATE ON criterion_results
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: criterion_results rows cannot be updated'); END;
CREATE TRIGGER trg_criterion_results_no_delete BEFORE DELETE ON criterion_results
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: criterion_results rows cannot be deleted'); END;

CREATE TABLE grade_decisions (
  id                      TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  attempt_id              TEXT    NOT NULL REFERENCES attempts (id)
                          ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary
  total_score             INTEGER NOT NULL CHECK (total_score BETWEEN 0 AND 100),
  passed                  INTEGER NOT NULL CHECK (passed IN (0, 1)),
  is_final                INTEGER NOT NULL DEFAULT 0 CHECK (is_final IN (0, 1)),
  decided_by              TEXT    NOT NULL,
  decided_at              INTEGER NOT NULL,
  approval_id             TEXT    REFERENCES approval_requests (id)
                          ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: a final grade keeps its approval
  rationale               TEXT,
  criterion_results_sha256 TEXT   CHECK (criterion_results_sha256 IS NULL OR length(criterion_results_sha256) = 64),
  supersedes              TEXT    REFERENCES grade_decisions (id)
                          ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: supersession chain is evidence
  revision                INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at              INTEGER NOT NULL,
  updated_at              INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_grade_decisions_attempt ON grade_decisions (attempt_id);
CREATE INDEX idx_grade_decisions_decided ON grade_decisions (decided_at);
-- A04: a final grade is immutable; a correction is a new superseding row.
CREATE TRIGGER trg_grade_decisions_final_no_update BEFORE UPDATE ON grade_decisions
WHEN OLD.is_final = 1
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: final grade decision cannot be updated; insert a superseding decision'); END;
CREATE TRIGGER trg_grade_decisions_no_delete BEFORE DELETE ON grade_decisions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: grade_decisions rows cannot be deleted'); END;

CREATE TABLE retake_links (
  original_attempt_id TEXT    NOT NULL REFERENCES attempts (id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary
  retake_attempt_id   TEXT    NOT NULL UNIQUE REFERENCES attempts (id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary
  reason              TEXT    NOT NULL,
  created_by          TEXT    NOT NULL,
  created_at          INTEGER NOT NULL,
  PRIMARY KEY (original_attempt_id, retake_attempt_id),
  CHECK (original_attempt_id <> retake_attempt_id)
) STRICT;

-- Every exposure of task/key/rubric content (sealed key rule, handoff §9 "Exam integrity"). Immutable.
CREATE TABLE exposure_log (
  id                    TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  assessment_version_id TEXT    NOT NULL REFERENCES assessment_versions (id)
                        ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary
  attempt_id            TEXT    REFERENCES attempts (id)
                        ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary
  exposed_to            TEXT    NOT NULL CHECK (exposed_to IN ('OPERATOR','MODEL','SKILL','EXPORT')),
  actor_id              TEXT    NOT NULL,
  mode                  TEXT    NOT NULL CHECK (mode IN ('STUDY','LAB','EXAM','REVIEW','EXECUTIVE','RESEARCH','MAINTENANCE')),
  content_kind          TEXT    NOT NULL CHECK (content_kind IN ('TASK','KEY','RUBRIC')),
  content_sha256        TEXT    NOT NULL CHECK (length(content_sha256) = 64),
  reason                TEXT,
  exposed_at            INTEGER NOT NULL,
  created_at            INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_exposure_log_version ON exposure_log (assessment_version_id, exposed_at);
CREATE INDEX idx_exposure_log_attempt ON exposure_log (attempt_id);
CREATE TRIGGER trg_exposure_log_no_update BEFORE UPDATE ON exposure_log
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: exposure_log rows cannot be updated'); END;
CREATE TRIGGER trg_exposure_log_no_delete BEFORE DELETE ON exposure_log
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: exposure_log rows cannot be deleted'); END;

-- Foreign-key columns not covered by an earlier index. SQLite evaluates ON DELETE RESTRICT / SET NULL on the
-- parent by scanning the child column, so every FK column carries a leading index (checked by
-- every_foreign_key_column_is_index_covered).
CREATE INDEX idx_assessment_definitions_training_day_id ON assessment_definitions (training_day_id);
CREATE INDEX idx_criterion_results_supersedes ON criterion_results (supersedes);
CREATE INDEX idx_grade_decisions_supersedes ON grade_decisions (supersedes);
CREATE INDEX idx_grade_decisions_approval_id ON grade_decisions (approval_id);
