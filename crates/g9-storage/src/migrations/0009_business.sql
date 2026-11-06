-- 0009_business: goals, projects, milestones, dependencies, commitments, execution observations, decisions
-- and opt-in reflections (M14 Goals/Execution; ARCHITECTURE §3 "business domain isolation").
-- Every row defaults to data_domain BUSINESS (reflections: PERSONAL) so that repositories refuse cross-domain
-- reads without Capability::CrossDomainRead (ARCHITECTURE §6 "Privacy").
-- Handoff §7 names the dependency table `dependencies`; defect TEC-37 asks for an unambiguous name, so the
-- table is `milestone_dependencies` and `dependencies` is a compatibility view.

CREATE TABLE goals (
  id                TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  title             TEXT    NOT NULL,
  description       TEXT,
  horizon           TEXT    NOT NULL CHECK (horizon IN ('WEEK','MONTH','QUARTER','YEAR','PROGRAM')),
  priority          INTEGER NOT NULL DEFAULT 3 CHECK (priority BETWEEN 1 AND 5),
  state             TEXT    NOT NULL DEFAULT 'PROPOSED' CHECK (state IN ('PROPOSED','ACTIVE','ACHIEVED','DROPPED')),
  parent_goal_id    TEXT    REFERENCES goals (id)
                    ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: a parent with children is removed explicitly
  target_date_civil TEXT,
  zone              TEXT,
  owner_id          TEXT    NOT NULL,
  data_domain       TEXT    NOT NULL DEFAULT 'BUSINESS'
                    CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification    TEXT    NOT NULL DEFAULT 'PRIVATE'
                    CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision          INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at        INTEGER NOT NULL,
  updated_at        INTEGER NOT NULL,
  CHECK (id <> parent_goal_id),
  CHECK ((target_date_civil IS NULL) = (zone IS NULL))
) STRICT;
CREATE INDEX idx_goals_state  ON goals (state, priority);
CREATE INDEX idx_goals_parent ON goals (parent_goal_id);

CREATE TABLE projects (
  id             TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  goal_id        TEXT    REFERENCES goals (id)
                 ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: projects are unlinked explicitly
  title          TEXT    NOT NULL,
  description    TEXT,
  state          TEXT    NOT NULL DEFAULT 'PLANNED' CHECK (state IN ('PLANNED','ACTIVE','BLOCKED','DONE','CANCELLED')),
  starts_at      INTEGER,
  ends_at        INTEGER CHECK (ends_at IS NULL OR starts_at IS NULL OR ends_at >= starts_at),
  data_domain    TEXT    NOT NULL DEFAULT 'BUSINESS'
                 CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification TEXT    NOT NULL DEFAULT 'PRIVATE'
                 CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision       INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_projects_goal  ON projects (goal_id);
CREATE INDEX idx_projects_state ON projects (state);

CREATE TABLE milestones (
  id             TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  project_id     TEXT    NOT NULL REFERENCES projects (id)
                 ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: milestones carry commitments and observations
  title          TEXT    NOT NULL,
  description    TEXT,
  due_at         INTEGER,
  state          TEXT    NOT NULL DEFAULT 'PLANNED' CHECK (state IN ('PLANNED','DUE','DONE','MISSED','CANCELLED')),
  completed_at   INTEGER,
  ordinal        INTEGER NOT NULL DEFAULT 1 CHECK (ordinal >= 1),
  data_domain    TEXT    NOT NULL DEFAULT 'BUSINESS'
                 CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification TEXT    NOT NULL DEFAULT 'PRIVATE'
                 CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision       INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL,
  CHECK (state <> 'DONE' OR completed_at IS NOT NULL)
) STRICT;
CREATE INDEX idx_milestones_project ON milestones (project_id, ordinal);
CREATE INDEX idx_milestones_due     ON milestones (state, due_at);

CREATE TABLE milestone_dependencies (
  id                TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  from_milestone_id TEXT    NOT NULL REFERENCES milestones (id)
                    ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: edges are removed explicitly
  to_milestone_id   TEXT    NOT NULL REFERENCES milestones (id)
                    ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: edges are removed explicitly
  kind              TEXT    NOT NULL CHECK (kind IN ('BLOCKS','REQUIRES','RELATED')),
  created_at        INTEGER NOT NULL,
  UNIQUE (from_milestone_id, to_milestone_id, kind),
  CHECK (from_milestone_id <> to_milestone_id)
) STRICT;
CREATE INDEX idx_milestone_dependencies_from ON milestone_dependencies (from_milestone_id);
CREATE INDEX idx_milestone_dependencies_to   ON milestone_dependencies (to_milestone_id);
CREATE VIEW dependencies AS SELECT * FROM milestone_dependencies;

-- Operator promises with a due instant; may be mirrored into the calendar (M14 ↔ M03).
CREATE TABLE commitments (
  id                TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  project_id        TEXT    REFERENCES projects (id)
                    ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link
  milestone_id      TEXT    REFERENCES milestones (id)
                    ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link
  calendar_event_id TEXT    REFERENCES calendar_events (id)
                    ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the calendar
  title             TEXT    NOT NULL,
  description       TEXT,
  due_at            INTEGER NOT NULL,
  state             TEXT    NOT NULL DEFAULT 'OPEN' CHECK (state IN ('OPEN','KEPT','MISSED','RENEGOTIATED','CANCELLED')),
  resolved_at       INTEGER,
  data_domain       TEXT    NOT NULL DEFAULT 'BUSINESS'
                    CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification    TEXT    NOT NULL DEFAULT 'PRIVATE'
                    CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision          INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at        INTEGER NOT NULL,
  updated_at        INTEGER NOT NULL,
  CHECK (state = 'OPEN' OR resolved_at IS NOT NULL)
) STRICT;
CREATE INDEX idx_commitments_due       ON commitments (state, due_at);
CREATE INDEX idx_commitments_project   ON commitments (project_id);
CREATE INDEX idx_commitments_milestone ON commitments (milestone_id);

-- Measured execution facts (throughput, lead time, kept-commitment ratio ...). Immutable measurements.
CREATE TABLE execution_observations (
  id              TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  project_id      TEXT    REFERENCES projects (id)
                  ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: measurement keeps its value
  milestone_id    TEXT    REFERENCES milestones (id)
                  ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: same
  commitment_id   TEXT    REFERENCES commitments (id)
                  ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: same
  metric          TEXT    NOT NULL,
  value           REAL    NOT NULL,
  unit            TEXT    NOT NULL,
  observed_at     INTEGER NOT NULL,
  source_kind     TEXT    NOT NULL CHECK (source_kind IN ('OPERATOR_INPUT','SYSTEM_GENERATED','CURRICULUM_MANIFEST',
                    'NOTE','ATTACHMENT','REFERENCE_DOCUMENT','MODEL_RESPONSE','SKILL_PACKAGE','ASSESSMENT_SUBMISSION',
                    'EXTERNAL_FEED')),
  trust           TEXT    NOT NULL CHECK (trust IN ('OPERATOR','SYSTEM_DETERMINISTIC','APPROVED_REFERENCE',
                    'IMPORTED_DOCUMENT','SKILL_PACKAGE','MODEL_OUTPUT','UNKNOWN')),
  provenance_json TEXT    NOT NULL CHECK (json_valid(provenance_json)),
  data_domain     TEXT    NOT NULL DEFAULT 'BUSINESS'
                  CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification  TEXT    NOT NULL DEFAULT 'PRIVATE'
                  CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  created_at      INTEGER NOT NULL,
  -- model output is never an execution measurement
  CHECK (trust <> 'MODEL_OUTPUT')
) STRICT;
CREATE INDEX idx_execution_observations_project  ON execution_observations (project_id, observed_at);
CREATE INDEX idx_execution_observations_observed ON execution_observations (observed_at);
CREATE TRIGGER trg_execution_observations_no_update BEFORE UPDATE ON execution_observations
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: execution_observations rows cannot be updated'); END;
CREATE TRIGGER trg_execution_observations_no_delete BEFORE DELETE ON execution_observations
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: execution_observations rows cannot be deleted'); END;

-- Decision records (executive workspace). A revisited decision is a new row linked by supersedes.
CREATE TABLE decisions (
  id             TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  goal_id        TEXT    REFERENCES goals (id)
                 ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link
  project_id     TEXT    REFERENCES projects (id)
                 ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link
  title          TEXT    NOT NULL,
  context        TEXT    NOT NULL,
  options_json   TEXT    NOT NULL DEFAULT '[]' CHECK (json_valid(options_json) AND json_type(options_json) = 'array'),
  chosen_option  TEXT,
  rationale      TEXT,
  state          TEXT    NOT NULL DEFAULT 'PROPOSED' CHECK (state IN ('PROPOSED','DECIDED','REVISITED','REVERSED')),
  decided_by     TEXT,
  decided_at     INTEGER,
  review_due_at  INTEGER,
  supersedes     TEXT    REFERENCES decisions (id)
                 ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: supersession chain is history
  trust          TEXT    NOT NULL CHECK (trust IN ('OPERATOR','SYSTEM_DETERMINISTIC','APPROVED_REFERENCE',
                   'IMPORTED_DOCUMENT','SKILL_PACKAGE','MODEL_OUTPUT','UNKNOWN')),
  data_domain    TEXT    NOT NULL DEFAULT 'BUSINESS'
                 CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification TEXT    NOT NULL DEFAULT 'PRIVATE'
                 CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision       INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL,
  CHECK (id <> supersedes),
  -- a decision is DECIDED only by an operator (model output can only propose)
  CHECK (state = 'PROPOSED' OR (decided_by IS NOT NULL AND decided_at IS NOT NULL AND chosen_option IS NOT NULL)),
  CHECK (trust <> 'MODEL_OUTPUT' OR state = 'PROPOSED')
) STRICT;
CREATE INDEX idx_decisions_state   ON decisions (state, decided_at);
CREATE INDEX idx_decisions_project ON decisions (project_id);
CREATE INDEX idx_decisions_review  ON decisions (review_due_at);

-- What actually happened after a decision. Immutable.
CREATE TABLE decision_outcomes (
  id          TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  decision_id TEXT    NOT NULL REFERENCES decisions (id)
              ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: outcomes are evidence about the decision
  measured_at INTEGER NOT NULL,
  summary     TEXT    NOT NULL,
  verdict     TEXT    NOT NULL CHECK (verdict IN ('BETTER','AS_EXPECTED','WORSE','UNKNOWN')),
  recorded_by TEXT    NOT NULL,
  created_at  INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_decision_outcomes_decision ON decision_outcomes (decision_id, measured_at);
CREATE TRIGGER trg_decision_outcomes_no_update BEFORE UPDATE ON decision_outcomes
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: decision_outcomes rows cannot be updated'); END;
CREATE TRIGGER trg_decision_outcomes_no_delete BEFORE DELETE ON decision_outcomes
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: decision_outcomes rows cannot be deleted'); END;

-- PERSONAL domain, opt-in only; excluded from exports by default; erasable through deletion_requests (A10).
CREATE TABLE opt_in_reflections (
  id                 TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  session_id         TEXT    REFERENCES study_sessions (id)
                     ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link
  recorded_at        INTEGER NOT NULL,
  body               TEXT    NOT NULL,
  body_sha256        TEXT    NOT NULL CHECK (length(body_sha256) = 64),
  energy_level       INTEGER CHECK (energy_level IS NULL OR energy_level BETWEEN 1 AND 5),
  focus_level        INTEGER CHECK (focus_level IS NULL OR focus_level BETWEEN 1 AND 5),
  data_domain        TEXT    NOT NULL DEFAULT 'PERSONAL'
                     CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification     TEXT    NOT NULL DEFAULT 'PRIVATE'
                     CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  erased             INTEGER NOT NULL DEFAULT 0 CHECK (erased IN (0, 1)),
  erased_at          INTEGER,
  erasure_request_id TEXT    REFERENCES deletion_requests (id)
                     ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: the erasure anchor must survive
  revision           INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at         INTEGER NOT NULL,
  updated_at         INTEGER NOT NULL,
  CHECK (erased = 0 OR (erased_at IS NOT NULL AND erasure_request_id IS NOT NULL))
) STRICT;
CREATE INDEX idx_opt_in_reflections_recorded ON opt_in_reflections (recorded_at);
CREATE INDEX idx_opt_in_reflections_session  ON opt_in_reflections (session_id);

-- Foreign-key columns not covered by an earlier index. SQLite evaluates ON DELETE RESTRICT / SET NULL on the
-- parent by scanning the child column, so every FK column carries a leading index (checked by
-- every_foreign_key_column_is_index_covered).
CREATE INDEX idx_commitments_calendar_event_id ON commitments (calendar_event_id);
CREATE INDEX idx_decisions_goal_id ON decisions (goal_id);
CREATE INDEX idx_decisions_supersedes ON decisions (supersedes);
CREATE INDEX idx_execution_observations_milestone_id ON execution_observations (milestone_id);
CREATE INDEX idx_execution_observations_commitment_id ON execution_observations (commitment_id);
CREATE INDEX idx_opt_in_reflections_erasure_request_id ON opt_in_reflections (erasure_request_id);
