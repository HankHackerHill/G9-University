-- 0003_competency: competency graph, learner memory and derived snapshots (ARCHITECTURE §8, handoff §9, M06/M09).
-- competency_observations is created in 0007 because it references attempts (0006) and learning_episodes (0007).

CREATE TABLE competencies (
  id             TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  code           TEXT    NOT NULL UNIQUE,
  title          TEXT    NOT NULL,
  description    TEXT,
  domain_area    TEXT    NOT NULL,
  validity_days  INTEGER NOT NULL DEFAULT 90 CHECK (validity_days > 0),
  rules_json     TEXT    NOT NULL DEFAULT '{}' CHECK (json_valid(rules_json)),
  data_domain    TEXT    NOT NULL DEFAULT 'LEARNING'
                 CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification TEXT    NOT NULL DEFAULT 'INTERNAL'
                 CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision       INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL
) STRICT;

CREATE TABLE competency_versions (
  id              TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  competency_id   TEXT    NOT NULL REFERENCES competencies (id)
                  ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: versions are history
  version_label   TEXT    NOT NULL,
  definition_json TEXT    NOT NULL CHECK (json_valid(definition_json)),
  created_by      TEXT    NOT NULL,
  created_at      INTEGER NOT NULL,
  UNIQUE (competency_id, version_label)
) STRICT;
CREATE INDEX idx_competency_versions_competency ON competency_versions (competency_id);

CREATE TABLE competency_edges (
  id                 TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  from_competency_id TEXT    NOT NULL REFERENCES competencies (id)
                     ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: graph edges are removed explicitly
  to_competency_id   TEXT    NOT NULL REFERENCES competencies (id)
                     ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: graph edges are removed explicitly
  edge_kind          TEXT    NOT NULL CHECK (edge_kind IN ('PREREQUISITE','SUPPORTS','RELATED')),
  weight             REAL    NOT NULL DEFAULT 1.0 CHECK (weight >= 0.0 AND weight <= 1.0),
  created_at         INTEGER NOT NULL,
  UNIQUE (from_competency_id, to_competency_id, edge_kind),
  CHECK (from_competency_id <> to_competency_id)
) STRICT;
CREATE INDEX idx_competency_edges_from ON competency_edges (from_competency_id);
CREATE INDEX idx_competency_edges_to   ON competency_edges (to_competency_id);

CREATE TABLE objective_competencies (
  objective_id  TEXT    NOT NULL REFERENCES daily_objectives (id)
                ON DELETE CASCADE ON UPDATE RESTRICT, -- CASCADE: structural child of the curriculum version
  competency_id TEXT    NOT NULL REFERENCES competencies (id)
                ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: a mapped competency cannot vanish
  dimension     TEXT    NOT NULL CHECK (dimension IN ('RECALL','UNDERSTANDING','APPLICATION','TRANSFER')),
  weight        REAL    NOT NULL DEFAULT 1.0 CHECK (weight > 0.0 AND weight <= 1.0),
  created_at    INTEGER NOT NULL,
  PRIMARY KEY (objective_id, competency_id, dimension)
) STRICT;
CREATE INDEX idx_objective_competencies_competency ON objective_competencies (competency_id);

-- Learner memory (M09). Model output may only create PROPOSED rows; acceptance is an operator command.
CREATE TABLE learner_hypotheses (
  id                  TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  competency_id       TEXT    REFERENCES competencies (id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: hypotheses are longitudinal evidence
  statement           TEXT    NOT NULL,
  kind                TEXT    NOT NULL CHECK (kind IN ('MISCONCEPTION','STRENGTH','PREFERENCE','BLOCKER')),
  supporting_refs     TEXT    NOT NULL DEFAULT '[]' CHECK (json_valid(supporting_refs) AND json_type(supporting_refs) = 'array'),
  contradicting_refs  TEXT    NOT NULL DEFAULT '[]' CHECK (json_valid(contradicting_refs) AND json_type(contradicting_refs) = 'array'),
  confidence_low      REAL    NOT NULL CHECK (confidence_low >= 0.0 AND confidence_low <= 1.0),
  confidence_high     REAL    NOT NULL CHECK (confidence_high >= confidence_low AND confidence_high <= 1.0),
  status              TEXT    NOT NULL CHECK (status IN ('PROPOSED','ACCEPTED','REJECTED','RETIRED')),
  provenance_json     TEXT    NOT NULL CHECK (json_valid(provenance_json)),
  trust               TEXT    NOT NULL CHECK (trust IN ('OPERATOR','SYSTEM_DETERMINISTIC','APPROVED_REFERENCE',
                        'IMPORTED_DOCUMENT','SKILL_PACKAGE','MODEL_OUTPUT','UNKNOWN')),
  decided_by          TEXT,
  decided_at          INTEGER,
  invalidated_at      INTEGER,
  invalidation_reason TEXT,
  data_domain         TEXT    NOT NULL DEFAULT 'LEARNING'
                      CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification      TEXT    NOT NULL DEFAULT 'PRIVATE'
                      CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision            INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at          INTEGER NOT NULL,
  updated_at          INTEGER NOT NULL,
  -- a hypothesis that was never decided by the operator cannot be ACCEPTED
  CHECK (status <> 'ACCEPTED' OR decided_by IS NOT NULL)
) STRICT;
CREATE INDEX idx_learner_hypotheses_competency ON learner_hypotheses (competency_id);
CREATE INDEX idx_learner_hypotheses_status     ON learner_hypotheses (status, kind);
CREATE INDEX idx_learner_hypotheses_created    ON learner_hypotheses (created_at);

-- Handoff §7 names this table `misconception_hypotheses`; ARCHITECTURE §8 generalises it to learner_hypotheses.
CREATE VIEW misconception_hypotheses AS
  SELECT * FROM learner_hypotheses WHERE kind = 'MISCONCEPTION';

-- Derived projections (replayable fold over observations). Never authoritative; invalidated on correction.
CREATE TABLE competency_state_snapshots (
  id                  TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  competency_id       TEXT    NOT NULL REFERENCES competencies (id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: snapshots document history
  state               TEXT    NOT NULL CHECK (state IN ('UNASSESSED','INTRODUCED','GUIDED','INDEPENDENT','VALIDATED','RETEST_DUE')),
  projection_json     TEXT    NOT NULL CHECK (json_valid(projection_json)),
  rules_version       TEXT    NOT NULL,
  last_observation_id TEXT,
  computed_at         INTEGER NOT NULL,
  invalidated_at      INTEGER,
  invalidation_reason TEXT,
  created_at          INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_competency_state_snapshots_competency ON competency_state_snapshots (competency_id, computed_at);
