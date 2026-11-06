-- 0010_intervention_review_and_reliability: columns the typed repositories (g9-storage::repos) need and
-- 0001–0009 lack. Applied migrations are never edited; this file only adds columns and triggers.
--
-- interventions (M12, LRN-16, IPC interventions.list):
--   * origin_trust  — trust class of the proposer, written once at insert, immutable afterwards.
--   * trust         — (existing) authority of the row's current state. A proposal that did not originate
--                     with the operator (MODEL_OUTPUT, SKILL_PACKAGE, IMPORTED_DOCUMENT, ...) keeps its
--                     origin_trust and is elevated to trust = 'OPERATOR' only by an operator review
--                     (PROPOSED -> REVIEWED) that records reviewed_by/reviewed_at. The 0007 CHECK
--                     (MODEL_OUTPUT rows stay HYPOTHESIS/PROPOSED/REJECTED/RETIRED) is unchanged and is
--                     generalised below to every non-authoritative trust class.
--   * reviewed_by / reviewed_at — the operator review that elevated the row.
--   * steps_json / time_budget_ms / acceptance — the plan the operator reviews (IPC fields `steps`,
--     `time_budget_ms`, `acceptance`). Rows written before 0010 (none in a released build) carry the
--     defaults '[]' / 0 / ''; the repository requires a non-empty acceptance before a review.
-- grade_decisions (ADR-0012, LRN-11, IPC `grade.reliability_class`):
--   * reliability_class — existing rows default to the least-trusted class, SELF_ADJUDICATED, which never
--     satisfies validation evidence (ADR-0005).
-- study_sessions / calendar_events (IPC session.* `block`/`block_seq`, calendar.* `block`/`adjusted`):
--   * study_block_id — informational link to the curriculum block a session/event realises (SET NULL like the
--     existing training_day_id/objective_id links).
--   * calendar_events.dst_adjustment — GAP/OVERLAP flag recorded when the civil start was resolved (§7).
-- Append-only tables that 0001–0009 left without an immutability trigger (the repositories never UPDATE or
-- DELETE them; these triggers are the second line of defence, A04/R-003). DELETE triggers are added only where
-- no parent RESTRICT semantics are tested through them (assessment_versions and rubric_criteria keep their
-- RESTRICT-on-parent behaviour and get UPDATE triggers only):
--   * assessment_versions, rubrics, rubric_criteria — versioned grading definitions (UPDATE refused).
--   * retake_links, attempt_evidence, competency_versions, content_classifications — UPDATE and DELETE refused.
--   * note_versions — UPDATE refused except the A10 erasure form used by g9-storage::deletion (body emptied,
--     erased 0 -> 1, identity/hash columns unchanged); DELETE refused (erasure redacts, never deletes).
-- Curriculum (C01, "never rewrite objectives"): a version's manifest identity columns never change; an
-- APPROVED_CANONICAL version is frozen except for its is_current/revision bookkeeping and cannot be deleted;
-- the structural rows of any version (weeks, training_days, daily_objectives, study_blocks) are never updated
-- (a different manifest is a new version). DELETE stays possible for non-canonical versions (CASCADE inside the
-- version, ADR-0002). resource_references/deliverable_definitions get no UPDATE trigger because their SET NULL
-- objective links are rewritten by that cascade.

ALTER TABLE interventions ADD COLUMN origin_trust TEXT NOT NULL DEFAULT 'UNKNOWN'
  CHECK (origin_trust IN ('OPERATOR','SYSTEM_DETERMINISTIC','APPROVED_REFERENCE','IMPORTED_DOCUMENT',
                          'SKILL_PACKAGE','MODEL_OUTPUT','UNKNOWN'));
ALTER TABLE interventions ADD COLUMN reviewed_by TEXT;
ALTER TABLE interventions ADD COLUMN reviewed_at INTEGER;
ALTER TABLE interventions ADD COLUMN steps_json TEXT NOT NULL DEFAULT '[]'
  CHECK (json_valid(steps_json) AND json_type(steps_json) = 'array');
ALTER TABLE interventions ADD COLUMN time_budget_ms INTEGER NOT NULL DEFAULT 0 CHECK (time_budget_ms >= 0);
ALTER TABLE interventions ADD COLUMN acceptance TEXT NOT NULL DEFAULT '';

-- Before 0010 `trust` recorded the proposer's trust and could not be changed by any repository.
UPDATE interventions SET origin_trust = trust;

-- A new row states its proposer's trust in both columns and carries no review.
CREATE TRIGGER trg_interventions_insert_origin BEFORE INSERT ON interventions
WHEN NEW.trust IS NOT NEW.origin_trust OR NEW.reviewed_by IS NOT NULL OR NEW.reviewed_at IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'PRECONDITION: a new intervention records its origin trust in trust and origin_trust and carries no review'); END;

-- The proposer is history.
CREATE TRIGGER trg_interventions_origin_no_update BEFORE UPDATE OF origin_trust, proposed_by, proposed_at ON interventions
WHEN NEW.origin_trust IS NOT OLD.origin_trust OR NEW.proposed_by IS NOT OLD.proposed_by
  OR NEW.proposed_at IS NOT OLD.proposed_at
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: intervention origin (origin_trust, proposed_by, proposed_at) cannot be updated'); END;

-- Trust changes only through an operator review (PROPOSED -> REVIEWED, elevated to OPERATOR, reviewer recorded).
CREATE TRIGGER trg_interventions_trust_only_by_review BEFORE UPDATE OF trust ON interventions
WHEN NEW.trust IS NOT OLD.trust
  AND NOT (OLD.state = 'PROPOSED' AND NEW.state = 'REVIEWED' AND NEW.trust = 'OPERATOR'
           AND NEW.reviewed_by IS NOT NULL AND NEW.reviewed_at IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: intervention trust changes only through an operator review (PROPOSED -> REVIEWED)'); END;

-- Review columns are written only by a review step (a revised plan is re-reviewed through PROPOSED -> REVIEWED).
CREATE TRIGGER trg_interventions_review_only_on_review BEFORE UPDATE OF reviewed_by, reviewed_at ON interventions
WHEN (NEW.reviewed_by IS NOT OLD.reviewed_by OR NEW.reviewed_at IS NOT OLD.reviewed_at)
  AND NOT (OLD.state = 'PROPOSED' AND NEW.state = 'REVIEWED'
           AND NEW.reviewed_by IS NOT NULL AND NEW.reviewed_at IS NOT NULL)
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: intervention review columns change only through a review (PROPOSED -> REVIEWED)'); END;

-- Every PROPOSED -> REVIEWED step records a fresh review (whatever the origin).
CREATE TRIGGER trg_interventions_review_recorded BEFORE UPDATE OF state ON interventions
WHEN OLD.state = 'PROPOSED' AND NEW.state = 'REVIEWED'
  AND (NEW.reviewed_by IS NULL OR NEW.reviewed_at IS NULL OR NEW.reviewed_at IS OLD.reviewed_at)
BEGIN SELECT RAISE(ABORT, 'PRECONDITION: PROPOSED -> REVIEWED must record the reviewing operator and review time'); END;

-- Generalises the 0007 MODEL_OUTPUT CHECK: only an authoritative trust class (OPERATOR, SYSTEM_DETERMINISTIC)
-- may hold a state past PROPOSED other than the closing states REJECTED/RETIRED.
CREATE TRIGGER trg_interventions_authority_required BEFORE UPDATE OF state ON interventions
WHEN NEW.trust NOT IN ('OPERATOR','SYSTEM_DETERMINISTIC')
  AND NEW.state NOT IN ('HYPOTHESIS','PROPOSED','REJECTED','RETIRED')
BEGIN SELECT RAISE(ABORT, 'PRECONDITION: a non-authoritative intervention needs an operator review before it can advance'); END;

-- The plan the operator reviewed is not rewritten behind the review: once REVIEWED or later it is frozen
-- (a different plan goes through REVISED -> PROPOSED and a new review).
CREATE TRIGGER trg_interventions_plan_frozen_after_review BEFORE UPDATE OF description, steps_json, time_budget_ms, acceptance, kind ON interventions
WHEN OLD.state NOT IN ('HYPOTHESIS','PROPOSED','REVISED')
  AND (NEW.description IS NOT OLD.description OR NEW.steps_json IS NOT OLD.steps_json
       OR NEW.time_budget_ms IS NOT OLD.time_budget_ms OR NEW.acceptance IS NOT OLD.acceptance
       OR NEW.kind IS NOT OLD.kind)
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: a reviewed intervention plan cannot be rewritten; revise and re-review'); END;

ALTER TABLE grade_decisions ADD COLUMN reliability_class TEXT NOT NULL DEFAULT 'SELF_ADJUDICATED'
  CHECK (reliability_class IN ('DETERMINISTIC','EXTERNAL_REVIEW','SELF_ADJUDICATED'));

ALTER TABLE study_sessions ADD COLUMN study_block_id TEXT REFERENCES study_blocks (id)
  ON DELETE SET NULL ON UPDATE RESTRICT; -- SET NULL: informational link to the curriculum block
CREATE INDEX idx_study_sessions_study_block_id ON study_sessions (study_block_id);

ALTER TABLE calendar_events ADD COLUMN study_block_id TEXT REFERENCES study_blocks (id)
  ON DELETE SET NULL ON UPDATE RESTRICT; -- SET NULL: informational link to the curriculum block
CREATE INDEX idx_calendar_events_study_block_id ON calendar_events (study_block_id);

ALTER TABLE calendar_events ADD COLUMN dst_adjustment TEXT
  CHECK (dst_adjustment IS NULL OR dst_adjustment IN ('GAP','OVERLAP'));

CREATE TRIGGER trg_assessment_versions_no_update BEFORE UPDATE ON assessment_versions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: assessment_versions rows cannot be updated; create a new version'); END;
CREATE TRIGGER trg_rubrics_no_update BEFORE UPDATE ON rubrics
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: rubrics rows cannot be updated; create a new assessment version'); END;
CREATE TRIGGER trg_rubric_criteria_no_update BEFORE UPDATE ON rubric_criteria
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: rubric_criteria rows cannot be updated; create a new assessment version'); END;

CREATE TRIGGER trg_retake_links_no_update BEFORE UPDATE ON retake_links
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: retake_links rows cannot be updated'); END;
CREATE TRIGGER trg_retake_links_no_delete BEFORE DELETE ON retake_links
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: retake_links rows cannot be deleted'); END;
CREATE TRIGGER trg_attempt_evidence_no_update BEFORE UPDATE ON attempt_evidence
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: attempt_evidence rows cannot be updated'); END;
CREATE TRIGGER trg_attempt_evidence_no_delete BEFORE DELETE ON attempt_evidence
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: attempt_evidence rows cannot be deleted'); END;
CREATE TRIGGER trg_competency_versions_no_update BEFORE UPDATE ON competency_versions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: competency_versions rows cannot be updated; insert a new version'); END;
CREATE TRIGGER trg_competency_versions_no_delete BEFORE DELETE ON competency_versions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: competency_versions rows cannot be deleted'); END;
CREATE TRIGGER trg_content_classifications_no_update BEFORE UPDATE ON content_classifications
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: content_classifications rows cannot be updated; append a reclassification'); END;
CREATE TRIGGER trg_content_classifications_no_delete BEFORE DELETE ON content_classifications
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: content_classifications rows cannot be deleted'); END;

CREATE TRIGGER trg_note_versions_update_only_erasure BEFORE UPDATE ON note_versions
WHEN NOT (OLD.erased = 0 AND NEW.erased = 1 AND NEW.body_markdown = ''
          AND NEW.id IS OLD.id AND NEW.note_id IS OLD.note_id AND NEW.version_no IS OLD.version_no
          AND NEW.body_sha256 IS OLD.body_sha256 AND NEW.author_id IS OLD.author_id
          AND NEW.created_at IS OLD.created_at)
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: note_versions rows are append-only; only the A10 erasure may redact them'); END;
CREATE TRIGGER trg_note_versions_no_delete BEFORE DELETE ON note_versions
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: note_versions rows cannot be deleted; erasure redacts in place'); END;

CREATE TRIGGER trg_curriculum_versions_manifest_frozen BEFORE UPDATE OF curriculum_id, version_label, manifest_sha256,
  source_ref, invariants_json, imported_by, imported_at, created_at ON curriculum_versions
WHEN NEW.curriculum_id IS NOT OLD.curriculum_id OR NEW.version_label IS NOT OLD.version_label
  OR NEW.manifest_sha256 IS NOT OLD.manifest_sha256 OR NEW.source_ref IS NOT OLD.source_ref
  OR NEW.invariants_json IS NOT OLD.invariants_json OR NEW.imported_by IS NOT OLD.imported_by
  OR NEW.imported_at IS NOT OLD.imported_at OR NEW.created_at IS NOT OLD.created_at
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: a curriculum version manifest cannot be rewritten; import a new version'); END;
CREATE TRIGGER trg_curriculum_versions_canonical_frozen BEFORE UPDATE OF provenance, approved_by, approved_at,
  approval_id ON curriculum_versions
WHEN OLD.provenance = 'APPROVED_CANONICAL'
  AND (NEW.provenance IS NOT OLD.provenance OR NEW.approved_by IS NOT OLD.approved_by
       OR NEW.approved_at IS NOT OLD.approved_at OR NEW.approval_id IS NOT OLD.approval_id)
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: an APPROVED_CANONICAL curriculum version is frozen (only is_current may change)'); END;
CREATE TRIGGER trg_curriculum_versions_canonical_no_delete BEFORE DELETE ON curriculum_versions
WHEN OLD.provenance = 'APPROVED_CANONICAL'
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: an APPROVED_CANONICAL curriculum version cannot be deleted'); END;

CREATE TRIGGER trg_weeks_no_update BEFORE UPDATE ON weeks
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: weeks rows cannot be updated; import a new curriculum version'); END;
CREATE TRIGGER trg_training_days_no_update BEFORE UPDATE ON training_days
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: training_days rows cannot be updated; import a new curriculum version'); END;
CREATE TRIGGER trg_daily_objectives_no_update BEFORE UPDATE ON daily_objectives
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: daily_objectives rows cannot be updated; import a new curriculum version'); END;
CREATE TRIGGER trg_study_blocks_no_update BEFORE UPDATE ON study_blocks
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: study_blocks rows cannot be updated; import a new curriculum version'); END;
