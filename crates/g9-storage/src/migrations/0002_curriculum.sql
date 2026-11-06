-- 0002_curriculum: curriculum manifests and their structural tree (ARCHITECTURE §6, handoff §7 "Curriculum").
-- Deletion rule: a curriculum version owns weeks → training_days → daily_objectives → study_blocks →
-- resource_references / deliverable_definitions. These are structural children and CASCADE *within the
-- version*. Versions themselves and curricula RESTRICT because sessions, attempts and evidence refer to them.
-- objective_competencies is created in 0003 (it references competencies).

CREATE TABLE curricula (
  id             TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  title          TEXT    NOT NULL,
  description    TEXT,
  data_domain    TEXT    NOT NULL DEFAULT 'LEARNING'
                 CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification TEXT    NOT NULL DEFAULT 'INTERNAL'
                 CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision       INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at     INTEGER NOT NULL,
  updated_at     INTEGER NOT NULL
) STRICT;

CREATE TABLE curriculum_versions (
  id              TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  curriculum_id   TEXT    NOT NULL REFERENCES curricula (id)
                  ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: versions are the authoritative history
  version_label   TEXT    NOT NULL,
  provenance      TEXT    NOT NULL CHECK (provenance IN ('PROPOSED_SEED','RECONCILED_CANDIDATE','APPROVED_CANONICAL')),
  manifest_sha256 TEXT    NOT NULL CHECK (length(manifest_sha256) = 64),
  source_ref      TEXT    NOT NULL,
  invariants_json TEXT    NOT NULL CHECK (json_valid(invariants_json)),
  imported_by     TEXT    NOT NULL,
  imported_at     INTEGER NOT NULL,
  approved_by     TEXT,
  approved_at     INTEGER,
  approval_id     TEXT    REFERENCES approval_requests (id)
                  ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the approval token
  is_current      INTEGER NOT NULL DEFAULT 0 CHECK (is_current IN (0, 1)),
  revision        INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL,
  UNIQUE (curriculum_id, version_label),
  -- only an approved canonical version may be current (ManifestProvenance::is_authoritative)
  CHECK (is_current = 0 OR provenance = 'APPROVED_CANONICAL')
) STRICT;
CREATE INDEX idx_curriculum_versions_curriculum ON curriculum_versions (curriculum_id);
CREATE INDEX idx_curriculum_versions_imported   ON curriculum_versions (imported_at);

CREATE TABLE weeks (
  id                    TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  curriculum_version_id TEXT    NOT NULL REFERENCES curriculum_versions (id)
                        ON DELETE CASCADE ON UPDATE RESTRICT, -- CASCADE: structural child of the version
  week_number           INTEGER NOT NULL CHECK (week_number BETWEEN 1 AND 12),
  title                 TEXT    NOT NULL,
  theme                 TEXT,
  planned_minutes       INTEGER NOT NULL CHECK (planned_minutes >= 0),
  created_at            INTEGER NOT NULL,
  UNIQUE (curriculum_version_id, week_number)
) STRICT;
CREATE INDEX idx_weeks_version ON weeks (curriculum_version_id);

CREATE TABLE training_days (
  id                    TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  week_id               TEXT    NOT NULL REFERENCES weeks (id)
                        ON DELETE CASCADE ON UPDATE RESTRICT, -- CASCADE: structural child of the version
  curriculum_version_id TEXT    NOT NULL REFERENCES curriculum_versions (id)
                        ON DELETE CASCADE ON UPDATE RESTRICT, -- CASCADE: structural child of the version
  day_code              TEXT    NOT NULL CHECK (day_code GLOB 'D[0-9][0-9]'),
  day_number            INTEGER NOT NULL CHECK (day_number BETWEEN 1 AND 72),
  day_of_week           INTEGER NOT NULL CHECK (day_of_week BETWEEN 1 AND 6),
  planned_minutes       INTEGER NOT NULL CHECK (planned_minutes >= 0),
  title                 TEXT    NOT NULL,
  created_at            INTEGER NOT NULL,
  UNIQUE (curriculum_version_id, day_code),
  UNIQUE (curriculum_version_id, day_number)
) STRICT;
CREATE INDEX idx_training_days_week    ON training_days (week_id);
CREATE INDEX idx_training_days_version ON training_days (curriculum_version_id);

CREATE TABLE daily_objectives (
  id                    TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  training_day_id       TEXT    NOT NULL REFERENCES training_days (id)
                        ON DELETE CASCADE ON UPDATE RESTRICT, -- CASCADE: structural child of the version
  curriculum_version_id TEXT    NOT NULL REFERENCES curriculum_versions (id)
                        ON DELETE CASCADE ON UPDATE RESTRICT, -- CASCADE: structural child of the version
  objective_code        TEXT    NOT NULL,
  ordinal               INTEGER NOT NULL CHECK (ordinal BETWEEN 1 AND 3),
  statement             TEXT    NOT NULL,
  success_criteria      TEXT,
  dimension             TEXT    CHECK (dimension IS NULL OR dimension IN ('RECALL','UNDERSTANDING','APPLICATION','TRANSFER')),
  created_at            INTEGER NOT NULL,
  UNIQUE (training_day_id, ordinal),
  UNIQUE (curriculum_version_id, objective_code)
) STRICT;
CREATE INDEX idx_daily_objectives_day     ON daily_objectives (training_day_id);
CREATE INDEX idx_daily_objectives_version ON daily_objectives (curriculum_version_id);

CREATE TABLE study_blocks (
  id              TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  training_day_id TEXT    NOT NULL REFERENCES training_days (id)
                  ON DELETE CASCADE ON UPDATE RESTRICT, -- CASCADE: structural child of the version
  objective_id    TEXT    REFERENCES daily_objectives (id)
                  ON DELETE CASCADE ON UPDATE RESTRICT, -- CASCADE: structural child of the version
  ordinal         INTEGER NOT NULL CHECK (ordinal >= 1),
  block_code      TEXT    NOT NULL CHECK (block_code IN ('recall','theory','guided_lab','independent_lab',
                    'defense_verification','evidence_reporting','blind_practical','remediation','report',
                    'performance_review')),
  minutes         INTEGER NOT NULL CHECK (minutes > 0),
  mode            TEXT    NOT NULL CHECK (mode IN ('STUDY','LAB','EXAM','REVIEW','EXECUTIVE','RESEARCH','MAINTENANCE')),
  created_at      INTEGER NOT NULL,
  UNIQUE (training_day_id, ordinal)
) STRICT;
CREATE INDEX idx_study_blocks_day       ON study_blocks (training_day_id);
CREATE INDEX idx_study_blocks_objective ON study_blocks (objective_id);

-- Reading/reference material attached to a version. source_ref is a stable string (an approved
-- source_registry id or an approved-root relative path); no FK because source_registry lands in 0008.
CREATE TABLE resource_references (
  id                    TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  curriculum_version_id TEXT    NOT NULL REFERENCES curriculum_versions (id)
                        ON DELETE CASCADE ON UPDATE RESTRICT, -- CASCADE: structural child of the version
  objective_id          TEXT    REFERENCES daily_objectives (id)
                        ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link
  title                 TEXT    NOT NULL,
  source_ref            TEXT    NOT NULL,
  uri                   TEXT,
  trust                 TEXT    NOT NULL DEFAULT 'UNKNOWN' CHECK (trust IN ('OPERATOR','SYSTEM_DETERMINISTIC',
                          'APPROVED_REFERENCE','IMPORTED_DOCUMENT','SKILL_PACKAGE','MODEL_OUTPUT','UNKNOWN')),
  data_domain           TEXT    NOT NULL DEFAULT 'LEARNING'
                        CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification        TEXT    NOT NULL DEFAULT 'INTERNAL'
                        CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  created_at            INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_resource_references_version   ON resource_references (curriculum_version_id);
CREATE INDEX idx_resource_references_objective ON resource_references (objective_id);

CREATE TABLE deliverable_definitions (
  id              TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  training_day_id TEXT    NOT NULL REFERENCES training_days (id)
                  ON DELETE CASCADE ON UPDATE RESTRICT, -- CASCADE: structural child of the version
  objective_id    TEXT    REFERENCES daily_objectives (id)
                  ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link
  code            TEXT    NOT NULL,
  title           TEXT    NOT NULL,
  description     TEXT,
  evidence_kind   TEXT    NOT NULL CHECK (evidence_kind IN ('NOTE','ATTACHMENT','SESSION','ATTEMPT_SUBMISSION',
                    'CHECKER_OUTPUT','REVIEWER_COMMENT','EXTERNAL')),
  is_required     INTEGER NOT NULL DEFAULT 1 CHECK (is_required IN (0, 1)),
  created_at      INTEGER NOT NULL,
  UNIQUE (training_day_id, code)
) STRICT;
CREATE INDEX idx_deliverable_definitions_day ON deliverable_definitions (training_day_id);

-- Foreign-key columns not covered by an earlier index. SQLite evaluates ON DELETE RESTRICT / SET NULL on the
-- parent by scanning the child column, so every FK column carries a leading index (checked by
-- every_foreign_key_column_is_index_covered).
CREATE INDEX idx_curriculum_versions_approval_id ON curriculum_versions (approval_id);
CREATE INDEX idx_deliverable_definitions_objective_id ON deliverable_definitions (objective_id);
