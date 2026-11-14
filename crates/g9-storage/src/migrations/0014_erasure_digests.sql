-- 0014: a digest of erasable content is content (ADR-0017, security audit 2026-11-13).
--
-- The digest of a short text is the text: whoever holds the SHA-256 of a four-digit code
-- recovers the code in ten thousand guesses. The erasure of a note version therefore replaces
-- its body digest as well (with a digest that identifies the tombstone, never the content).
-- The trigger of 0010 required the digest to survive the erasure; this one requires the
-- erasure to replace it, so an erasure that keeps the digest of the erased body is refused by
-- the schema itself. Everything else about a version stays append-only: the only update allowed
-- is still the one erasure of a row that was not erased, to an empty body.
DROP TRIGGER trg_note_versions_update_only_erasure;
CREATE TRIGGER trg_note_versions_update_only_erasure BEFORE UPDATE ON note_versions
WHEN NOT (OLD.erased = 0 AND NEW.erased = 1 AND NEW.body_markdown = ''
          AND NEW.body_sha256 IS NOT OLD.body_sha256
          AND NEW.id IS OLD.id AND NEW.note_id IS OLD.note_id AND NEW.version_no IS OLD.version_no
          AND NEW.author_id IS OLD.author_id AND NEW.created_at IS OLD.created_at)
BEGIN SELECT RAISE(ABORT, 'IMMUTABLE: note_versions rows are append-only; only the A10 erasure may redact them'); END;
