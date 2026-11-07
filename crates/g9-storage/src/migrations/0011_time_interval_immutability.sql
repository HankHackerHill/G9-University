-- 0011_time_interval_immutability: widen the time_intervals backstop trigger (A03/A04).
-- 0004 guarded only the time columns of a closed interval, so a closed (credited) interval could be moved to
-- another session (`session_id`) or have its closer relabelled (`closed_by`), contradicting "an interval is
-- closed exactly once; corrections are separate rows" (ARCHITECTURE.md §7). Applied migrations are never
-- edited (checksums), so the trigger is replaced here under the same name.
-- * Every row: `id`, `session_id`, `start_utc`, `start_mono_ms` and `created_at` never change.
-- * A closed row: additionally `end_utc`, `end_mono_ms`, `continuity` and `closed_by` never change; only the
--   revision bookkeeping (`revision`, `updated_at`) may. The single close (`end_utc IS NULL` → set) is untouched.
DROP TRIGGER trg_time_intervals_closed_no_retime;
CREATE TRIGGER trg_time_intervals_closed_no_retime BEFORE UPDATE ON time_intervals
WHEN NEW.id IS NOT OLD.id
  OR NEW.session_id IS NOT OLD.session_id
  OR NEW.start_utc IS NOT OLD.start_utc
  OR NEW.start_mono_ms IS NOT OLD.start_mono_ms
  OR NEW.created_at IS NOT OLD.created_at
  OR (OLD.end_utc IS NOT NULL AND (
       NEW.end_utc IS NOT OLD.end_utc
    OR NEW.end_mono_ms IS NOT OLD.end_mono_ms
    OR NEW.continuity IS NOT OLD.continuity
    OR NEW.closed_by IS NOT OLD.closed_by))
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: time_intervals rows keep their session and start, and closed rows cannot be changed; add a manual_time_corrections row'); END;
