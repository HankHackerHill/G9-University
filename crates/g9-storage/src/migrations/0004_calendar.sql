-- 0004_calendar: calendar, scheduling and the session/timer ledger (ARCHITECTURE §7, M03/M04).

CREATE TABLE calendar_events (
  id              TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  title           TEXT    NOT NULL,
  kind            TEXT    NOT NULL CHECK (kind IN ('STUDY_BLOCK','APPOINTMENT','PROTECTED_REST','ASSESSMENT','REVIEW','OTHER')),
  -- civil intent (wall clock) + IANA zone; start_utc/end_utc are the resolved occurrence instants
  civil_start     TEXT    NOT NULL,
  civil_end       TEXT    NOT NULL,
  zone            TEXT    NOT NULL,
  start_utc       INTEGER NOT NULL,
  end_utc         INTEGER NOT NULL CHECK (end_utc >= start_utc),
  all_day         INTEGER NOT NULL DEFAULT 0 CHECK (all_day IN (0, 1)),
  immutable       INTEGER NOT NULL DEFAULT 0 CHECK (immutable IN (0, 1)),
  protected       INTEGER NOT NULL DEFAULT 0 CHECK (protected IN (0, 1)),
  source          TEXT    NOT NULL CHECK (source IN ('OPERATOR','SCHEDULER','IMPORT')),
  training_day_id TEXT    REFERENCES training_days (id)
                  ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the curriculum
  objective_id    TEXT    REFERENCES daily_objectives (id)
                  ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the curriculum
  data_domain     TEXT    NOT NULL DEFAULT 'LEARNING'
                  CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification  TEXT    NOT NULL DEFAULT 'PRIVATE'
                  CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision        INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at      INTEGER NOT NULL,
  updated_at      INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_calendar_events_start ON calendar_events (start_utc, end_utc);
CREATE INDEX idx_calendar_events_day   ON calendar_events (training_day_id);

CREATE TABLE recurrence_rules (
  id                TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  calendar_event_id TEXT    NOT NULL UNIQUE REFERENCES calendar_events (id)
                    ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: the app deletes the rule explicitly first
  freq              TEXT    NOT NULL CHECK (freq IN ('DAILY','WEEKLY')),
  interval          INTEGER NOT NULL DEFAULT 1 CHECK (interval >= 1),
  by_day            TEXT    NOT NULL DEFAULT '[]' CHECK (json_valid(by_day) AND json_type(by_day) = 'array'),
  until_civil       TEXT,
  count             INTEGER CHECK (count IS NULL OR count >= 1),
  zone              TEXT    NOT NULL,
  revision          INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at        INTEGER NOT NULL,
  updated_at        INTEGER NOT NULL,
  CHECK (until_civil IS NULL OR count IS NULL)
) STRICT;

CREATE TABLE schedule_proposals (
  id                        TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  proposed_by               TEXT    NOT NULL,
  proposed_at               INTEGER NOT NULL,
  range_start_utc           INTEGER NOT NULL,
  range_end_utc             INTEGER NOT NULL CHECK (range_end_utc >= range_start_utc),
  proposal_json             TEXT    NOT NULL CHECK (json_valid(proposal_json)),
  violated_constraints_json TEXT    NOT NULL DEFAULT '[]' CHECK (json_valid(violated_constraints_json)),
  feasible                  INTEGER NOT NULL CHECK (feasible IN (0, 1)),
  state                     TEXT    NOT NULL CHECK (state IN ('PROPOSED','APPLIED','REJECTED','SUPERSEDED')),
  trust                     TEXT    NOT NULL CHECK (trust IN ('OPERATOR','SYSTEM_DETERMINISTIC','APPROVED_REFERENCE',
                              'IMPORTED_DOCUMENT','SKILL_PACKAGE','MODEL_OUTPUT','UNKNOWN')),
  provenance_json           TEXT    NOT NULL CHECK (json_valid(provenance_json)),
  revision                  INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at                INTEGER NOT NULL,
  updated_at                INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_schedule_proposals_state ON schedule_proposals (state, proposed_at);

-- Operator decisions on proposals. Evidentiary: immutable.
CREATE TABLE schedule_decisions (
  id               TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  proposal_id      TEXT    NOT NULL REFERENCES schedule_proposals (id)
                   ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: the decision is evidence about its proposal
  decision         TEXT    NOT NULL CHECK (decision IN ('APPLY','REJECT')),
  decided_by       TEXT    NOT NULL,
  decided_at       INTEGER NOT NULL,
  approval_id      TEXT    REFERENCES approval_requests (id)
                   ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: immutable row cannot be SET NULL
  reason           TEXT,
  applied_revision INTEGER CHECK (applied_revision IS NULL OR applied_revision >= 1),
  created_at       INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_schedule_decisions_proposal ON schedule_decisions (proposal_id);
CREATE TRIGGER trg_schedule_decisions_no_update BEFORE UPDATE ON schedule_decisions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: schedule_decisions rows cannot be updated'); END;
CREATE TRIGGER trg_schedule_decisions_no_delete BEFORE DELETE ON schedule_decisions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: schedule_decisions rows cannot be deleted'); END;

CREATE TABLE study_sessions (
  id                TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  training_day_id   TEXT    REFERENCES training_days (id)
                    ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the curriculum
  objective_id      TEXT    REFERENCES daily_objectives (id)
                    ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the curriculum
  calendar_event_id TEXT    REFERENCES calendar_events (id)
                    ON DELETE SET NULL ON UPDATE RESTRICT, -- SET NULL: informational link to the calendar
  state             TEXT    NOT NULL CHECK (state IN ('PLANNED','READY','RUNNING','PAUSED','COMPLETED','REVIEWED',
                      'CLOSED','INTERRUPTED','CANCELLED')),
  mode              TEXT    NOT NULL CHECK (mode IN ('STUDY','LAB','EXAM','REVIEW','EXECUTIVE','RESEARCH','MAINTENANCE')),
  zone              TEXT    NOT NULL,
  planned_start_utc INTEGER,
  planned_end_utc   INTEGER CHECK (planned_end_utc IS NULL OR planned_start_utc IS NULL OR planned_end_utc >= planned_start_utc),
  planned_minutes   INTEGER NOT NULL DEFAULT 0 CHECK (planned_minutes >= 0),
  started_at        INTEGER,
  ended_at          INTEGER,
  credited_ms       INTEGER NOT NULL DEFAULT 0 CHECK (credited_ms >= 0),
  uncertain_ms      INTEGER NOT NULL DEFAULT 0 CHECK (uncertain_ms >= 0),
  data_domain       TEXT    NOT NULL DEFAULT 'LEARNING'
                    CHECK (data_domain IN ('LEARNING','BUSINESS','PERSONAL','SECURITY_SENSITIVE','SYSTEM')),
  classification    TEXT    NOT NULL DEFAULT 'PRIVATE'
                    CHECK (classification IN ('PUBLIC','INTERNAL','PRIVATE','SECRET')),
  revision          INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at        INTEGER NOT NULL,
  updated_at        INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_study_sessions_day     ON study_sessions (training_day_id);
CREATE INDEX idx_study_sessions_planned ON study_sessions (planned_start_utc);
CREATE INDEX idx_study_sessions_state   ON study_sessions (state);

-- Session state transitions (append-only).
CREATE TABLE session_events (
  id           TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  session_id   TEXT    NOT NULL REFERENCES study_sessions (id)
               ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary ledger
  event        TEXT    NOT NULL CHECK (event IN ('PREPARE','START','PAUSE','RESUME','COMPLETE','REVIEW','CLOSE',
                 'INTERRUPT','CANCEL','RECOVER')),
  from_state   TEXT    NOT NULL,
  to_state     TEXT    NOT NULL,
  occurred_at  INTEGER NOT NULL,
  actor_id     TEXT    NOT NULL,
  actor_kind   TEXT    NOT NULL CHECK (actor_kind IN ('OPERATOR','SYSTEM','SKILL','MODEL')),
  details_json TEXT    CHECK (details_json IS NULL OR json_valid(details_json)),
  created_at   INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_session_events_session ON session_events (session_id, occurred_at);
CREATE TRIGGER trg_session_events_no_update BEFORE UPDATE ON session_events
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: session_events rows cannot be updated'); END;
CREATE TRIGGER trg_session_events_no_delete BEFORE DELETE ON session_events
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: session_events rows cannot be deleted'); END;

-- Timer ledger (A03). An interval is closed exactly once; corrections are separate rows.
CREATE TABLE time_intervals (
  id            TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  session_id    TEXT    NOT NULL REFERENCES study_sessions (id)
                ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary ledger
  start_utc     INTEGER NOT NULL,
  end_utc       INTEGER CHECK (end_utc IS NULL OR end_utc >= start_utc),
  start_mono_ms INTEGER NOT NULL CHECK (start_mono_ms >= 0),
  end_mono_ms   INTEGER CHECK (end_mono_ms IS NULL OR end_mono_ms >= start_mono_ms),
  continuity    TEXT    NOT NULL CHECK (continuity IN ('VERIFIED','UNCERTAIN')),
  closed_by     TEXT    CHECK (closed_by IS NULL OR closed_by IN ('OPERATOR','SYSTEM','RECOVERY')),
  revision      INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL,
  -- an open interval has neither end; a closed one has both
  CHECK ((end_utc IS NULL) = (end_mono_ms IS NULL))
) STRICT;
CREATE INDEX idx_time_intervals_session ON time_intervals (session_id, start_utc);
CREATE INDEX idx_time_intervals_open    ON time_intervals (session_id) WHERE end_utc IS NULL;
CREATE TRIGGER trg_time_intervals_no_delete BEFORE DELETE ON time_intervals
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: time_intervals rows cannot be deleted'); END;
-- a closed interval can only change its revision bookkeeping, never its recorded times
CREATE TRIGGER trg_time_intervals_closed_no_retime BEFORE UPDATE OF start_utc, end_utc, start_mono_ms, end_mono_ms, continuity ON time_intervals
WHEN OLD.end_utc IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: closed time_intervals rows cannot be re-timed; add a manual_time_corrections row'); END;

CREATE TABLE manual_time_corrections (
  id                  TEXT    PRIMARY KEY NOT NULL CHECK (length(id) BETWEEN 1 AND 128),
  session_id          TEXT    NOT NULL REFERENCES study_sessions (id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary ledger
  interval_id         TEXT    REFERENCES time_intervals (id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: the correction references the original
  corrected_start_utc INTEGER,
  corrected_end_utc   INTEGER CHECK (corrected_end_utc IS NULL OR corrected_start_utc IS NULL OR corrected_end_utc >= corrected_start_utc),
  delta_ms            INTEGER NOT NULL,
  reason              TEXT    NOT NULL,
  corrected_by        TEXT    NOT NULL,
  corrected_at        INTEGER NOT NULL,
  approval_id         TEXT    REFERENCES approval_requests (id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: immutable row cannot be SET NULL
  created_at          INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_manual_time_corrections_session  ON manual_time_corrections (session_id);
CREATE INDEX idx_manual_time_corrections_interval ON manual_time_corrections (interval_id);
CREATE TRIGGER trg_manual_time_corrections_no_update BEFORE UPDATE ON manual_time_corrections
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: manual_time_corrections rows cannot be updated'); END;
CREATE TRIGGER trg_manual_time_corrections_no_delete BEFORE DELETE ON manual_time_corrections
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: manual_time_corrections rows cannot be deleted'); END;

-- Foreign-key columns not covered by an earlier index. SQLite evaluates ON DELETE RESTRICT / SET NULL on the
-- parent by scanning the child column, so every FK column carries a leading index (checked by
-- every_foreign_key_column_is_index_covered).
CREATE INDEX idx_calendar_events_objective_id ON calendar_events (objective_id);
CREATE INDEX idx_schedule_decisions_approval_id ON schedule_decisions (approval_id);
CREATE INDEX idx_study_sessions_calendar_event_id ON study_sessions (calendar_event_id);
CREATE INDEX idx_study_sessions_objective_id ON study_sessions (objective_id);
CREATE INDEX idx_manual_time_corrections_approval_id ON manual_time_corrections (approval_id);
