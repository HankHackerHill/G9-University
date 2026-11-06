//! Text sanitization helpers for untrusted content (handoff §12 "untrusted
//! surfaces": documents, notes, model outputs, exam submissions).
//!
//! These helpers make text *safe to display and store as data*; they do not
//! make it trustworthy. Markdown/HTML rendering is sanitized separately in the
//! UI with an allow-list (ARCHITECTURE.md §12, SEC-05). Nothing here interprets
//! the text.

use serde::{Deserialize, Serialize};

/// Whether a character is removed by [`strip_control_chars`].
///
/// Removed: C0 controls except TAB/LF/CR, DEL, C1 controls (U+0080–U+009F),
/// and the Unicode bidirectional embedding/override/isolate controls
/// (U+202A–U+202E, U+2066–U+2069) that enable "Trojan Source" rendering
/// attacks. Everything else — including other-script letters, combining marks
/// and zero-width joiners used by legitimate text — is kept.
fn is_stripped(c: char) -> bool {
    matches!(c,
        '\u{0000}'..='\u{0008}'
        | '\u{000B}'..='\u{000C}'
        | '\u{000E}'..='\u{001F}'
        | '\u{007F}'
        | '\u{0080}'..='\u{009F}'
        | '\u{202A}'..='\u{202E}'
        | '\u{2066}'..='\u{2069}')
}

/// Remove control characters (C0 except TAB/LF/CR, DEL, C1, and the bidi
/// embedding/override/isolate controls U+202A–U+202E and U+2066–U+2069).
/// TAB, LF and CR are preserved; use [`normalize_newlines`] afterwards.
pub fn strip_control_chars(input: &str) -> String {
    input.chars().filter(|c| !is_stripped(*c)).collect()
}

/// Normalize `\r\n` and lone `\r` to `\n`.
pub fn normalize_newlines(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    out
}

/// Truncate to at most `max_bytes` bytes without splitting a UTF-8 code point.
///
/// Returns the (possibly shortened) text and whether truncation happened. The
/// cut is at a code-point boundary; it may still separate a base character
/// from a following combining mark, which is harmless for storage and display.
pub fn truncate_utf8_safe(input: &str, max_bytes: usize) -> (String, bool) {
    if input.len() <= max_bytes {
        return (input.to_string(), false);
    }
    let mut cut = max_bytes;
    while cut > 0 && !input.is_char_boundary(cut) {
        cut -= 1;
    }
    (input[..cut].to_string(), true)
}

/// Result of [`sanitize_for_display`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sanitized {
    /// Cleaned text.
    pub text: String,
    /// Whether the text was cut to the byte budget.
    pub truncated: bool,
    /// Number of characters removed by [`strip_control_chars`].
    pub stripped_chars: usize,
}

/// Strip control characters, normalize newlines, then truncate safely.
/// Truncation is reported, never silent.
pub fn sanitize_for_display(input: &str, max_bytes: usize) -> Sanitized {
    let stripped = strip_control_chars(input);
    let stripped_chars = input.chars().count() - stripped.chars().count();
    let normalized = normalize_newlines(&stripped);
    let (text, truncated) = truncate_utf8_safe(&normalized, max_bytes);
    Sanitized {
        text,
        truncated,
        stripped_chars,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn truncate_utf8_safe_never_splits_codepoint() {
        // "héllo" = h(1) é(2) l l o => 6 bytes; a cut at 2 falls inside 'é'.
        let (s, truncated) = truncate_utf8_safe("héllo", 2);
        assert_eq!(s, "h");
        assert!(truncated);
        // 4-byte emoji: any budget below 4 yields the empty string.
        let (s, truncated) = truncate_utf8_safe("😀abc", 3);
        assert_eq!(s, "");
        assert!(truncated);
        let (s, truncated) = truncate_utf8_safe("😀abc", 4);
        assert_eq!(s, "😀");
        assert!(truncated);
        // Exact fit is not a truncation.
        let (s, truncated) = truncate_utf8_safe("abc", 3);
        assert_eq!(s, "abc");
        assert!(!truncated);
        // Every budget produces valid UTF-8 (the String type guarantees it) and
        // never exceeds the budget.
        let sample = "aé😀b\u{0301}c";
        for budget in 0..=sample.len() {
            let (s, _) = truncate_utf8_safe(sample, budget);
            assert!(s.len() <= budget);
            assert!(sample.starts_with(&s));
        }
    }

    #[test]
    fn strip_control_chars_removes_c0_c1_and_bidi_overrides() {
        let input = "a\u{0000}b\u{0007}c\td\ne\r\nf\u{007F}g\u{0085}h\u{202E}i\u{2066}j";
        assert_eq!(strip_control_chars(input), "abc\td\ne\r\nfghij");
        // Non-ASCII letters, combining marks and ZWJ survive.
        let keep = "ßé\u{0301}👨\u{200D}👩";
        assert_eq!(strip_control_chars(keep), keep);
    }

    #[test]
    fn normalize_newlines_handles_crlf_and_lone_cr() {
        assert_eq!(normalize_newlines("a\r\nb\rc\nd\r"), "a\nb\nc\nd\n");
        assert_eq!(normalize_newlines("\r\n\r\n"), "\n\n");
    }

    #[test]
    fn sanitize_for_display_reports_strips_and_truncation() {
        let s = sanitize_for_display("x\u{0001}y\r\nz", 3);
        assert_eq!(
            s,
            Sanitized {
                text: "xy\n".into(),
                truncated: true,
                stripped_chars: 1
            }
        );
    }
}
