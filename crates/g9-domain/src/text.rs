//! Text that is shown to the operator as a fact, or stored as a name.
//!
//! Stored text is data. On a surface that states facts (the native approval
//! confirmation, a command printed for the operator to run, a row of a list) two
//! kinds of character turn data into a lie: those that cannot be seen, and those
//! that change the line or the order of what is shown. [`is_hidden`] names them
//! once; [`one_line`] makes any text safe to show on one line; [`name_defect`]
//! says why a text cannot be stored as a title, file name or label
//! (security audit 2026-11-13).

/// Whether `c` cannot be seen, or changes the line or the order in which the
/// characters around it are shown: a control character, a line or paragraph
/// separator, an invisible or bidirectional format character, or a tag character.
pub fn is_hidden(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{00AD}'                      // soft hyphen
                | '\u{061C}'                // Arabic letter mark
                | '\u{180E}'                // Mongolian vowel separator
                | '\u{200B}'..='\u{200F}'   // zero-width space, joiners, direction marks
                | '\u{2028}'..='\u{202E}'   // line and paragraph separators, embeddings, overrides
                | '\u{2060}'..='\u{206F}'   // word joiner, invisible operators, isolates
                | '\u{FEFF}'                // byte order mark
                | '\u{FFF9}'..='\u{FFFB}'   // interlinear annotation
                | '\u{E0000}'..='\u{E007F}' // tag characters
        )
}

/// `text` as one printable line: every run of whitespace (line breaks, tabs and
/// the Unicode separators included) becomes one space, every other hidden
/// character becomes U+FFFD so that its presence shows, and the result is capped
/// at `max` characters with an ellipsis.
pub fn one_line(text: &str, max: usize) -> String {
    let mut out = String::new();
    let mut last_space = true;
    for c in text.chars() {
        if c.is_whitespace() {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
            continue;
        }
        out.push(if is_hidden(c) { '\u{FFFD}' } else { c });
        last_space = false;
    }
    let trimmed = out.trim();
    if trimmed.chars().count() > max {
        let cut: String = trimmed.chars().take(max).collect();
        format!("{}…", cut.trim_end())
    } else {
        trimmed.to_string()
    }
}

/// Why `text` cannot be stored as a name (a title, a file name, a label), or
/// `None` when it can: a name has visible text and no hidden character.
pub fn name_defect(text: &str) -> Option<String> {
    if let Some(c) = text.chars().find(|c| is_hidden(*c)) {
        return Some(format!(
            "holds a character that cannot be seen or that changes the line or the order of the text (U+{:04X})",
            u32::from(c)
        ));
    }
    if text.trim().is_empty() {
        return Some("is empty or only spaces".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_removes_what_could_forge_or_reorder_a_line() {
        assert_eq!(one_line("  a \n\n Approve:\tb  ", 96), "a Approve: b");
        assert_eq!(one_line("pin\u{202E}txt.exe", 96), "pin\u{FFFD}txt.exe");
        assert_eq!(
            one_line("a\u{0007}b\u{2028}c\u{200B}d", 96),
            "a\u{FFFD}b c\u{FFFD}d"
        );
        assert_eq!(one_line("x\u{E0041}\u{E0042}y", 96), "x\u{FFFD}\u{FFFD}y");
        assert_eq!(
            one_line(&"x".repeat(200), 10),
            format!("{}…", "x".repeat(10))
        );
        assert_eq!(one_line("", 10), "");
    }

    #[test]
    fn a_name_has_visible_text_and_no_hidden_character() {
        for ok in [
            "Week 3 lab",
            "Überblick: Tag 1",
            "evidence (final).pdf",
            " padded ",
        ] {
            assert_eq!(name_defect(ok), None, "{ok:?}");
        }
        for blank in ["", " ", "\u{00A0}\u{3000}"] {
            assert_eq!(
                name_defect(blank).as_deref(),
                Some("is empty or only spaces"),
                "{blank:?}"
            );
        }
        for (hostile, code) in [
            ("a\nb", "U+000A"),
            ("a\tb", "U+0009"),
            ("pin\u{202E}txt.exe", "U+202E"),
            ("a\u{200B}b", "U+200B"),
            ("a\u{2028}b", "U+2028"),
            ("a\u{007F}b", "U+007F"),
            ("a\u{0085}b", "U+0085"),
            ("a\u{E0041}b", "U+E0041"),
            ("\u{FEFF}", "U+FEFF"),
        ] {
            let why = name_defect(hostile).unwrap_or_default();
            assert!(why.contains(code), "{hostile:?}: {why}");
        }
    }
}
