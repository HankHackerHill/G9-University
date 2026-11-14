-- 0015: the program plan (owner request 2026-11-14: the interface must be usable with the imported curriculum).
--
-- The curriculum names weeks, days and blocks but no dates. The operator anchors it on the calendar:
-- the Monday of week 1, and the time of day a planned day's first block starts. Until now this anchor
-- existed only as a command-line flag of the CLI, so the desktop application could not read a daily plan.
-- One row per curriculum; the row is mutable under a revision check (the operator may move the start).
-- Sessions that were already planned keep their own times: the plan does not rewrite them.
CREATE TABLE program_plan (
  curriculum_id    TEXT    PRIMARY KEY NOT NULL REFERENCES curricula (id)
                   ON DELETE RESTRICT ON UPDATE RESTRICT, -- RESTRICT: sessions planned from the program refer to it
  start_monday     TEXT    NOT NULL CHECK (start_monday GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
  day_start_minute INTEGER NOT NULL DEFAULT 540 CHECK (day_start_minute BETWEEN 0 AND 1439),
  revision         INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1),
  created_at       INTEGER NOT NULL,
  updated_at       INTEGER NOT NULL
) STRICT;
