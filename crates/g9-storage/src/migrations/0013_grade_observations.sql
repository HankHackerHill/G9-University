-- 0013_grade_observations: persist what is needed to derive competency observations from a final grade
-- decision (ARCHITECTURE.md §8 "Observations", Addendum A.2; ADR-0005, ADR-0012; defect register LRN-11).
-- Applied migrations are never edited; this file only adds one column, one table, one index and triggers.
--
-- assessment_versions.difficulty:
--   * the 1..=5 task difficulty (g9_domain::Difficulty) that whoever authors the version declares. It is
--     NULLABLE and has NO DEFAULT on purpose: a difficulty is never guessed. Rows written before this
--     migration, and versions authored without a declared difficulty, hold NULL; g9-app derives no
--     competency observation for an attempt on such a version and says so (warning
--     competency.observations_not_derived).
--   * assessment_versions rows stay immutable (trg_assessment_versions_no_update, 0010): the value is written
--     by the INSERT and never changes. ALTER TABLE ADD COLUMN is not an UPDATE and does not fire the trigger.
-- competency_observation_evidence:
--   * one append-only row per observation derived from a grade decision. It records which decision the
--     observation came from, the decision's reliability class (ADR-0012), whether the attempt was blind and
--     whether the observation is validation-eligible (decision passed, attempt blind, class not
--     SELF_ADJUDICATED; g9_assessment::ObservationDraft::validation_eligible). g9-app lists the rows with
--     validation_eligible = 0 for g9_competency::derive_state_excluding_validation, so such an observation
--     never counts as validation evidence.
--   * RESTRICT both ways: an observation or a decision that has an evidence row can be neither deleted nor
--     re-keyed (evidentiary tables, ARCHITECTURE.md §6).
--   * observations of episodes (study/lab) carry no evidence row: they come from no grade decision and are
--     never validation evidence (g9-competency requires an attempt-rooted observation to validate).

ALTER TABLE assessment_versions ADD COLUMN difficulty INTEGER
  CHECK (difficulty IS NULL OR difficulty BETWEEN 1 AND 5);

CREATE TABLE competency_observation_evidence (
  observation_id      TEXT    PRIMARY KEY NOT NULL
                      REFERENCES competency_observations (id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary
  decision_id         TEXT    NOT NULL REFERENCES grade_decisions (id)
                      ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: evidentiary
  reliability_class   TEXT    NOT NULL CHECK (reliability_class IN ('DETERMINISTIC','EXTERNAL_REVIEW','SELF_ADJUDICATED')),
  validation_eligible INTEGER NOT NULL CHECK (validation_eligible IN (0, 1)),
  blind               INTEGER NOT NULL CHECK (blind IN (0, 1)),
  created_at          INTEGER NOT NULL
) STRICT;
CREATE INDEX idx_competency_observation_evidence_decision ON competency_observation_evidence (decision_id);

CREATE TRIGGER trg_competency_observation_evidence_no_update BEFORE UPDATE ON competency_observation_evidence
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: competency_observation_evidence rows cannot be updated'); END;
CREATE TRIGGER trg_competency_observation_evidence_no_delete BEFORE DELETE ON competency_observation_evidence
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: competency_observation_evidence rows cannot be deleted'); END;
