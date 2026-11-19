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

/**
 * Characters that render as nothing, as a blank or as an arbitrary glyph and that `HIDDEN` does
 * not name (security audit 2026-11-19, NAME-1). Twin of `is_blank_or_ignorable` in `text.rs`:
 * explicit ranges, Unicode 17.0 data, so that the answer does not depend on the Unicode version
 * of the webview engine. The emoji selectors U+FE0E and U+FE0F are judged in context.
 */
const BLANK_OR_IGNORABLE =
  // eslint-disable-next-line no-misleading-character-class -- explicit sets of code points, never character sequences
  /[\u{034F}\u{0600}-\u{0605}\u{06DD}\u{070F}\u{0890}\u{0891}\u{08E2}\u{115F}\u{1160}\u{17B4}\u{17B5}\u{180B}-\u{180F}\u{2800}\u{3164}\u{FDD0}-\u{FDEF}\u{FE00}-\u{FE0D}\u{FFA0}\u{FFF0}-\u{FFF8}\u{FFFC}\u{E000}-\u{F8FF}\u{110BD}\u{110CD}\u{13430}-\u{1343F}\u{13441}\u{13442}\u{16FE4}\u{1BCA0}-\u{1BCA3}\u{1D159}\u{1D173}-\u{1D17A}\u{40000}-\u{10FFFF}]/u;
/** A space other than U+0020. */
const ODD_SPACE = /[\u{00A0}\u{1680}\u{2000}-\u{200A}\u{202F}\u{205F}\u{3000}]/u;
/** A character an emoji variation selector may follow. */
const EMOJI_BASE =
  /[#*0-9\u{00A9}\u{00AE}\u{203C}\u{2049}\u{2122}\u{2139}\u{2194}-\u{21AA}\u{231A}-\u{23FF}\u{24C2}\u{25AA}-\u{25FE}\u{2600}-\u{27BF}\u{2934}\u{2935}\u{2B05}-\u{2B55}\u{3030}\u{303D}\u{3297}\u{3299}\u{1F000}-\u{1FAFF}]/u;
/** A combining mark of the general-purpose mark blocks. */
const GENERIC_MARK =
  // eslint-disable-next-line no-misleading-character-class -- explicit sets of code points, never character sequences
  /[\u{0300}-\u{036F}\u{0483}-\u{0489}\u{1AB0}-\u{1AFF}\u{1DC0}-\u{1DFF}\u{20D0}-\u{20FF}\u{2DE0}-\u{2DFF}\u{3099}\u{309A}\u{FE20}-\u{FE2F}]/u;
/** An unpaired surrogate: matches only in `u` mode when it is not half of a pair. A Rust string never holds one. */
const LONE_SURROGATE = /[\u{D800}-\u{DFFF}]/u;

const hex4 = (ch: string): string => (ch.codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, '0');

/** Why `ch`, which follows `prev` (`null` at the start), cannot be part of a name. Twin of `char_defect`. */
function charDefect(prev: string | null, ch: string): string | null {
  const cp = ch.codePointAt(0) ?? 0;
  if (BLANK_OR_IGNORABLE.test(ch) || (cp & 0xfffe) === 0xfffe) {
    return `holds a blank, filler or invisible character (U+${hex4(ch)})`;
  }
  if (ODD_SPACE.test(ch)) {
    return `holds a space other than the ordinary space (U+${hex4(ch)})`;
  }
  if (cp === 0xfe0e || cp === 0xfe0f) {
    return prev !== null && EMOJI_BASE.test(prev)
      ? null
      : `holds a variation selector that does not follow an emoji (U+${hex4(ch)})`;
  }
  if (GENERIC_MARK.test(ch) && (prev === null || prev === ' ')) {
    return `holds a combining mark with no character to combine with (U+${hex4(ch)})`;
  }
  return null;
}
/** Keys of a request whose value is a name (`NAME_KEYS` in `crates/g9-app/src/dispatch.rs`). */
export const NAME_KEYS: readonly string[] = ['title', 'filename', 'label'];

/**
 * The code point (upper-case hex, at least four digits) of the first hidden character of `text`
 * other than TAB and LF, or `null`. Twin of `hidden_character` in `g9-curriculum`.
 */
export function hiddenCharacter(text: string): string | null {
  for (const ch of text) {
    const cp = ch.codePointAt(0) ?? 0;
    if ((ch !== '\t' && ch !== '\n' && HIDDEN.test(ch)) || BLANK_OR_IGNORABLE.test(ch) || (cp & 0xfffe) === 0xfffe) {
      return (ch.codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, '0');
    }
  }
  return null;
}

/** Why `text` cannot be stored as a name, or `null` when it can. Same words as `name_defect`. */
export function nameDefect(text: string): string | null {
  const hidden = HIDDEN.exec(text) ?? LONE_SURROGATE.exec(text);
  if (hidden !== null) {
    const code = (hidden[0].codePointAt(0) ?? 0).toString(16).toUpperCase().padStart(4, '0');
    return `holds a character that cannot be seen or that changes the line or the order of the text (U+${code})`;
  }
  if (text.trim() === '') {
    return 'is empty or only spaces';
  }
  let prev: string | null = null;
  for (const ch of text) {
    const why = charDefect(prev, ch);
    if (why !== null) {
      return why;
    }
    prev = ch;
  }
  // A run of spaces inside the name (spaces at either end are trimmed when the name is stored).
  if (text.replace(/^ +| +$/g, '').includes('  ')) {
    return 'holds two or more spaces in a row';
  }
  return null;
}

/** Keys of a request whose value is free text shown as a row title (`LINE_TEXT_KEYS` in `dispatch.rs`). */
export const LINE_TEXT_KEYS: readonly string[] = ['prompt'];

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
    if (typeof value === 'string' && LINE_TEXT_KEYS.includes(key)) {
      const code = hiddenCharacter(value);
      if (code !== null) {
        return { field: key, reason: `holds a character that cannot be seen or that changes the order of the text (U+${code})` };
      }
    }
    const found = firstUnprintableName(value);
    if (found !== null) {
      return found;
    }
  }
  return null;
}
