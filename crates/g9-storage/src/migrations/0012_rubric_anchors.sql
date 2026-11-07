-- 0012_rubric_anchors: persist the anchored scoring standard with the rubric (ADR-0012, ARCHITECTURE.md
-- Addendum A.1: "anchored 0-4 levels stored with the assessment version"; defect register LRN-10).
-- `anchors_json` is the JSON form of g9_assessment::RubricAnchors (level scale, standard-setting note,
-- per-criterion descriptors) and `anchors_sha256` the hex SHA-256 of exactly those bytes, re-checked on read.
-- Rows written before this migration carry NULL in both columns and read back with the standard anchors;
-- the rubrics table stays immutable (trg_rubrics_no_update, 0010).
ALTER TABLE rubrics ADD COLUMN anchors_json TEXT
  CHECK (anchors_json IS NULL OR json_valid(anchors_json));
ALTER TABLE rubrics ADD COLUMN anchors_sha256 TEXT
  CHECK (anchors_sha256 IS NULL OR length(anchors_sha256) = 64);
CREATE TRIGGER trg_rubrics_anchors_paired BEFORE INSERT ON rubrics
WHEN (NEW.anchors_json IS NULL) <> (NEW.anchors_sha256 IS NULL)
BEGIN SELECT RAISE(ABORT, 'rubrics.anchors_json and anchors_sha256 are written together'); END;
