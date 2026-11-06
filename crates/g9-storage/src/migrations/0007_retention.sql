-- 0007_retention: retention queue, learning episodes, interventions, experiments, calibration (M08/M12, §9)
-- and competency_observations (M06), created here because it references attempts (0006) and learning_episodes.

CREATE TABLE review_items (
  id              TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  kind            TEXT    NOT NULL CHECK (kind IN ('DECLARATIVE','PROCEDURAL')),
  competency_id   TEXT    REFERENCES competencies (id)
                  ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: review history belongs to the competency
  objective_id    TEXT    REFERENCES daily_objectives (id)
                  ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the curriculum
  prompt          TEXT    NOT NULL,
  answer_ref      TEXT,
  scheduler       TEXT    NOT NULL DEFAULT 'BASELINE' CHECK (scheduler IN ('BASELINE','FSRS')),
  due_at          INTEGER NOT NULL,
  last_success_at INTEGER,
  stage           INTEGER NOT NULL DEFAULT 0 CHECK (stage >= 0),
  interval_days   INTEGER NOT NULL DEFAULT 1 CHECK (interval_days >= 0),
  fsrs_state_json TEXT    CHECK (fsrs_state_json IS NULL OR json_valid(fsrs_state_json)),
  state           TEXT    NOT NULL DEFAULT 'ACTIVE' CHECK (state IN ('ACTIVE','SUSPENDED','RETIRED')),
  data_domain     TEXT    NOT NULL DEFAULT 'LEARNING'
                  CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification  TEXT    NOT NULL DEFAULT 'INTERNAL'
                  CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision        INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  -- FSRS is item-level and declarative-only (ARCHITECTURE §9)
  CHECK (scheduler <> 'FSRS' OR kind = 'DECLARATIVE')
) STRICT;
CREATE INDEX idx_review_items_due        ON review_items (state, due_at);
CREATE INDEX idx_review_items_competency ON review_items (competency_id);

CREATE TABLE review_attempts (
  id             TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  review_item_id TEXT    NOT NULL REFERENCES review_items (id)
                 ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: recall history is evidence
  session_id     TEXT    REFERENCES study_sessions (id)
                 ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link
  attempted_at   INTEGER NOT NULL,
  outcome        TEXT    NOT NULL CHECK (outcome IN ('SUCCESS','FAILURE','SKIPPED')),
  response_ms    INTEGER CHECK (response_ms IS NULL OR response_ms >= 0),
  confidence     REAL    CHECK (confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)),
  rating         INTEGER CHECK (rating IS NULL OR rating BETWEEN 1 AND 4),
  created_at     INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_review_attempts_item ON review_attempts (review_item_id, attempted_at);
CREATE TRIGGER trg_review_attempts_no_update BEFORE UPDATE ON review_attempts
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: review_attempts rows cannot be updated'); END;
CREATE TRIGGER trg_review_attempts_no_delete BEFORE DELETE ON review_attempts
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: review_attempts rows cannot be deleted'); END;

CREATE TABLE practical_transfer_checks (
  id            TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  competency_id TEXT    NOT NULL REFERENCES competencies (id)
                ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: scheduled by the competency's validity window
  due_at        INTEGER NOT NULL,
  scheduled_by  TEXT    NOT NULL CHECK (scheduled_by IN ('VALIDITY_WINDOW','OPERATOR')),
  state         TEXT    NOT NULL DEFAULT 'DUE' CHECK (state IN ('DUE','SCHEDULED','COMPLETED','WAIVED')),
  attempt_id    TEXT    REFERENCES attempts (id)
                ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link (attempts are never deleted anyway)
  completed_at  INTEGER,
  revision      INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_practical_transfer_checks_due ON practical_transfer_checks (state, due_at);

CREATE TABLE learning_episodes (
  id               TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  session_id       TEXT    REFERENCES study_sessions (id)
                   ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link
  objective_id     TEXT    REFERENCES daily_objectives (id)
                   ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the curriculum
  competency_id    TEXT    REFERENCES competencies (id)
                   ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: observations hang off episodes
  kind             TEXT    NOT NULL CHECK (kind IN ('GUIDED_LAB','INDEPENDENT_LAB','THEORY','RECALL','DEFENSE','REPORT','OTHER')),
  started_at       INTEGER NOT NULL,
  ended_at         INTEGER CHECK (ended_at IS NULL OR ended_at >= started_at),
  assistance_level TEXT    NOT NULL DEFAULT 'UNAIDED' CHECK (assistance_level IN ('UNAIDED','PROMPTED','HINTED','GUIDED','SHOWN')),
  difficulty       INTEGER CHECK (difficulty IS NULL OR difficulty BETWEEN 1 AND 5),
  outcome_json     TEXT    NOT NULL DEFAULT '{}' CHECK (json_valid(outcome_json)),
  data_domain      TEXT    NOT NULL DEFAULT 'LEARNING'
                   CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification   TEXT    NOT NULL DEFAULT 'PRIVATE'
                   CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision         INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at       INTEGER NOT NULL,
  updated_at       INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_learning_episodes_session    ON learning_episodes (session_id);
CREATE INDEX idx_learning_episodes_competency ON learning_episodes (competency_id, started_at);

-- Append-only competency evidence (ARCHITECTURE §8 "Observations"); corrections supersede.
CREATE TABLE competency_observations (
  id              TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  competency_id   TEXT    NOT NULL REFERENCES competencies (id)
                  ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary
  dimension       TEXT    NOT NULL CHECK (dimension IN ('RECALL','UNDERSTANDING','APPLICATION','TRANSFER')),
  attempt_id      TEXT    REFERENCES attempts (id)
                  ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary
  episode_id      TEXT    REFERENCES learning_episodes (id)
                  ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary
  difficulty      INTEGER NOT NULL CHECK (difficulty BETWEEN 1 AND 5),
  assistance      TEXT    NOT NULL CHECK (assistance IN ('UNAIDED','PROMPTED','HINTED','GUIDED','SHOWN')),
  score           INTEGER NOT NULL CHECK (score BETWEEN 0 AND 100),
  environment     TEXT    NOT NULL,
  mode            TEXT    NOT NULL CHECK (mode IN ('STUDY','LAB','EXAM','REVIEW','EXECUTIVE','RESEARCH','MAINTENANCE')),
  reviewer_id     TEXT,
  provenance_json TEXT    NOT NULL CHECK (json_valid(provenance_json)),
  trust           TEXT    NOT NULL CHECK (trust IN ('OPERATOR','SYSTEM_DETERMINISTIC','APPROVED_REFERENCE',
                    'IMPORTED_DOCUMENT','SKILL_PACKAGE','MODEL_OUTPUT','UNKNOWN')),
  uncertainty     REAL    NOT NULL DEFAULT 0.0 CHECK (uncertainty >= 0.0 AND uncertainty <= 1.0),
  observed_at     INTEGER NOT NULL,
  supersedes      TEXT    REFERENCES competency_observations (id)
                  ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: supersession chain is evidence
  created_at      INTEGER NOT NULL,
  -- an observation is grounded in an attempt or an episode
  CHECK (attempt_id IS NOT NULL OR episode_id IS NOT NULL),
  -- model output can never be authoritative evidence of competence
  CHECK (trust <> 'MODEL_OUTPUT')
) STRICT;
CREATE INDEX idx_competency_observations_competency ON competency_observations (competency_id, observed_at);
CREATE INDEX idx_competency_observations_attempt    ON competency_observations (attempt_id);
CREATE INDEX idx_competency_observations_episode    ON competency_observations (episode_id);
CREATE INDEX idx_competency_observations_supersedes ON competency_observations (supersedes);
CREATE TRIGGER trg_competency_observations_no_update BEFORE UPDATE ON competency_observations
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: competency_observations rows cannot be updated'); END;
CREATE TRIGGER trg_competency_observations_no_delete BEFORE DELETE ON competency_observations
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: competency_observations rows cannot be deleted'); END;

CREATE TABLE interventions (
  id                  TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  hypothesis_id       TEXT    REFERENCES learner_hypotheses (id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: the diagnosis trail must survive
  competency_id       TEXT    REFERENCES competencies (id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: same
  kind                TEXT    NOT NULL,
  description         TEXT    NOT NULL,
  state               TEXT    NOT NULL CHECK (state IN ('HYPOTHESIS','PROPOSED','REVIEWED','APPROVED','REJECTED','SCHEDULED',
                        'APPLIED','RETESTED','EVALUATED','RETAINED','REVISED','RETIRED')),
  proposed_by         TEXT    NOT NULL,
  proposed_at         INTEGER NOT NULL,
  trust               TEXT    NOT NULL CHECK (trust IN ('OPERATOR','SYSTEM_DETERMINISTIC','APPROVED_REFERENCE',
                        'IMPORTED_DOCUMENT','SKILL_PACKAGE','MODEL_OUTPUT','UNKNOWN')),
  approval_id         TEXT    REFERENCES approval_requests (id)
                      ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the token
  scheduled_for       INTEGER,
  applied_at          INTEGER,
  invalidated_at      INTEGER,
  invalidation_reason TEXT,
  data_domain         TEXT    NOT NULL DEFAULT 'LEARNING'
                      CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification      TEXT    NOT NULL DEFAULT 'PRIVATE'
                      CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision            INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at          INTEGER NOT NULL,
  updated_at          INTEGER NOT NULL,
  -- model output can only create HYPOTHESIS/PROPOSED rows (M12)
  CHECK (trust <> 'MODEL_OUTPUT' OR state IN ('HYPOTHESIS','PROPOSED','REJECTED','RETIRED'))
) STRICT;
CREATE INDEX idx_interventions_hypothesis ON interventions (hypothesis_id);
CREATE INDEX idx_interventions_state      ON interventions (state, proposed_at);

CREATE TABLE intervention_outcomes (
  id               TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  intervention_id  TEXT    NOT NULL REFERENCES interventions (id)
                   ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: measurements are evidence
  measured_at      INTEGER NOT NULL,
  metric           TEXT    NOT NULL,
  baseline_value   REAL,
  observed_value   REAL,
  effect_size      REAL,
  uncertainty_json TEXT    CHECK (uncertainty_json IS NULL OR json_valid(uncertainty_json)),
  verdict          TEXT    NOT NULL CHECK (verdict IN ('IMPROVED','NO_EFFECT','WORSE','INCONCLUSIVE')),
  created_at       INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_intervention_outcomes_intervention ON intervention_outcomes (intervention_id, measured_at);
CREATE TRIGGER trg_intervention_outcomes_no_update BEFORE UPDATE ON intervention_outcomes
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: intervention_outcomes rows cannot be updated'); END;
CREATE TRIGGER trg_intervention_outcomes_no_delete BEFORE DELETE ON intervention_outcomes
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: intervention_outcomes rows cannot be deleted'); END;

CREATE TABLE method_experiments (
  id              TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  title           TEXT    NOT NULL,
  hypothesis      TEXT    NOT NULL,
  intervention_id TEXT    REFERENCES interventions (id)
                  ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link
  design_json     TEXT    NOT NULL CHECK (json_valid(design_json)),
  started_at      INTEGER,
  ended_at        INTEGER CHECK (ended_at IS NULL OR started_at IS NULL OR ended_at >= started_at),
  state           TEXT    NOT NULL DEFAULT 'PLANNED' CHECK (state IN ('PLANNED','RUNNING','ANALYZED','CLOSED')),
  result_json     TEXT    CHECK (result_json IS NULL OR json_valid(result_json)),
  revision        INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_method_experiments_state ON method_experiments (state);

CREATE TABLE calibration_observations (
  id                TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  attempt_id        TEXT    REFERENCES attempts (id)
                    ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary
  review_attempt_id TEXT    REFERENCES review_attempts (id)
                    ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary
  stated_confidence REAL    NOT NULL CHECK (stated_confidence >= 0.0 AND stated_confidence <= 1.0),
  outcome           INTEGER NOT NULL CHECK (outcome IN (0, 1)),
  brier             REAL    NOT NULL CHECK (brier >= 0.0 AND brier <= 1.0),
  observed_at       INTEGER NOT NULL,
  created_at        INTEGER NOT NULL,
  CHECK (attempt_id IS NOT NULL OR review_attempt_id IS NOT NULL)
) STRICT;
CREATE INDEX idx_calibration_observations_observed ON calibration_observations (observed_at);
CREATE TRIGGER trg_calibration_observations_no_update BEFORE UPDATE ON calibration_observations
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: calibration_observations rows cannot be updated'); END;
CREATE TRIGGER trg_calibration_observations_no_delete BEFORE DELETE ON calibration_observations
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: calibration_observations rows cannot be deleted'); END;

-- Foreign-key columns not covered by an earlier index. SQLite evaluates ON DELETE RESTRICT / SET NULL on the
-- parent by scanning the child column, so every FK column carries a leading index (checked by
-- every_foreign_key_column_is_index_covered).
CREATE INDEX idx_review_items_objective_id ON review_items (objective_id);
CREATE INDEX idx_review_attempts_session_id ON review_attempts (session_id);
CREATE INDEX idx_practical_transfer_checks_competency_id ON practical_transfer_checks (competency_id);
CREATE INDEX idx_practical_transfer_checks_attempt_id ON practical_transfer_checks (attempt_id);
CREATE INDEX idx_learning_episodes_objective_id ON learning_episodes (objective_id);
CREATE INDEX idx_interventions_competency_id ON interventions (competency_id);
CREATE INDEX idx_interventions_approval_id ON interventions (approval_id);
CREATE INDEX idx_method_experiments_intervention_id ON method_experiments (intervention_id);
CREATE INDEX idx_calibration_observations_attempt_id ON calibration_observations (attempt_id);
CREATE INDEX idx_calibration_observations_review_attempt_id ON calibration_observations (review_attempt_id);
