/**
 * Text that is stored as a name (a title, a file name, a label): the TypeScript twin of
 * `crates/g9-domain/src/text.rs`, used by the mock transport so that it refuses what the backend
 * refuses (security audit 2026-11-13). A name has visible text and no hidden character: a control
 * character, a line or paragraph separator, an invisible or bidirectional format character or a
 * tag character. Such a character can forge a second line in a native confirmation or reorder
 * a file name (`pin<U+202E>txt.exe` is shown as `pinexe.txt`).
 *
 * The backend is the authority. This file exists for parity, and to give a form the same
 * wording before a request is sent.
 */

/** Hidden characters: the set of `g9_domain::text::is_hidden`. `\p{Cc}` is Rust's `char::is_control`. */
const HIDDEN = /[\p{Cc}\u00AD\u061C\u180E\u200B-\u200F\u2028-\u202E\u2060-\u206F\uFEFF\uFFF9-\uFFFB\u{E0000}-\u{E007F}]/u;

/** Keys of a request whose value is a name (`NAME_KEYS` in `crates/g9-app/src/dispatch.rs`). */
export const NAME_KEYS: readonly string[] = ['title', 'filename', 'label'];

/**
 * The code point (upper-case hex, at least four digits) of the first hidden character of `text`
 * other than TAB and LF, or `null`. Twin of `hidden_character` in `g9-curriculum`.
 */
export function hiddenCharacter(text: string): string | null {
  for (const ch of text) {
    if (ch !== '\t' && ch !== '\n' && HIDDEN.test(ch)) {
      return (ch.codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, '0');
    }
  }
  return null;
}

/** Why `text` cannot be stored as a name, or `null` when it can. Same words as `name_defect`. */
export function nameDefect(text: string): string | null {
  const hidden = HIDDEN.exec(text);
  if (hidden !== null) {
    const code = (hidden[0].codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, '0');
    return `holds a character that cannot be seen or that changes the line or the order of the text (U+${code})`;
  }
  if (text.trim() === '') {
    return 'is empty or only spaces';
  }
  return null;
}

/** The first name in `payload` (any depth, by key) that is not storable, as `{ field, reason }`. */
export function firstUnprintableName(payload: unknown): { field: string; reason: string } | null {
  if (Array.isArray(payload)) {
    for (const item of payload) {
      const found = firstUnprintableName(item);
      if (found !== null) {
        return found;
      }
    }
    return null;
  }
  if (typeof payload !== 'object' || payload === null) {
    return null;
  }
  for (const [key, value] of Object.entries(payload)) {
    if (typeof value === 'string' && NAME_KEYS.includes(key)) {
      const reason = nameDefect(value);
      if (reason !== null) {
        return { field: key, reason };
      }
    }
    const found = firstUnprintableName(value);
    if (found !== null) {
      return found;
    }
  }
  return null;
}
