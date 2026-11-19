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

/// Characters that render as nothing, as a blank or as an arbitrary glyph and that
/// [`is_hidden`] does not name (security audit 2026-11-19, NAME-1): default-ignorable code
/// points (the emoji selectors U+FE0E and U+FE0F are judged in context), the remaining format
/// characters (Cf), blank-by-design symbols, Hangul and Khitan fillers, private use,
/// noncharacters, and every code point from plane 4 up (unassigned, or a tag, variation-selector
/// or private-use block). Unicode 17.0 data; revisit when the toolchain moves to a newer version.
pub fn is_blank_or_ignorable(c: char) -> bool {
    // Noncharacters: U+FDD0..U+FDEF and U+xFFFE, U+xFFFF of every plane.
    if (u32::from(c) & 0xFFFE) == 0xFFFE {
        return true;
    }
    matches!(
        c,
        '\u{034F}'                      // combining grapheme joiner
            | '\u{0600}'..='\u{0605}'   // Arabic number signs (Cf)
            | '\u{06DD}'                // Arabic end of ayah (Cf)
            | '\u{070F}'                // Syriac abbreviation mark (Cf)
            | '\u{0890}'..='\u{0891}'   // Arabic pound and piastre mark above (Cf)
            | '\u{08E2}'                // Arabic disputed end of ayah (Cf)
            | '\u{115F}'..='\u{1160}'   // Hangul choseong and jungseong filler
            | '\u{17B4}'..='\u{17B5}'   // Khmer inherent vowels
            | '\u{180B}'..='\u{180F}'   // Mongolian free variation selectors (U+180E is in is_hidden)
            | '\u{2800}'                // Braille pattern blank
            | '\u{3164}'                // Hangul filler
            | '\u{FDD0}'..='\u{FDEF}'   // noncharacters
            | '\u{FE00}'..='\u{FE0D}'   // variation selectors 1-14 (15 and 16 are judged in context)
            | '\u{FFA0}'                // halfwidth Hangul filler
            | '\u{FFF0}'..='\u{FFF8}'   // reserved default-ignorable
            | '\u{FFFC}'                // object replacement character
            | '\u{E000}'..='\u{F8FF}'   // private use area
            | '\u{110BD}' | '\u{110CD}' // Kaithi number signs (Cf)
            | '\u{13430}'..='\u{1343F}' // Egyptian hieroglyph format controls (Cf)
            | '\u{13441}'..='\u{13442}' // Egyptian hieroglyph full and half blank
            | '\u{16FE4}'               // Khitan small script filler
            | '\u{1BCA0}'..='\u{1BCA3}' // shorthand format controls
            | '\u{1D159}'               // musical null notehead (blank)
            | '\u{1D173}'..='\u{1D17A}' // musical format controls
            | '\u{40000}'..='\u{10FFFF}' // planes 4-16 (the tag characters are in is_hidden)
    )
}

/// A space other than U+0020: no-break, ogham, en and em spaces down to hair, narrow no-break,
/// medium mathematical, ideographic. Blank to the eye, and not one character with the space.
fn is_odd_space(c: char) -> bool {
    matches!(
        c,
        '\u{00A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

/// A character an emoji variation selector may follow: the emoji blocks and the keycap bases
/// (a superset of the bases in `emoji-variation-sequences.txt`).
fn is_emoji_base(c: char) -> bool {
    matches!(
        c,
        '#' | '*' | '0'..='9'
            | '\u{00A9}' | '\u{00AE}' | '\u{203C}' | '\u{2049}' | '\u{2122}' | '\u{2139}'
            | '\u{2194}'..='\u{21AA}' | '\u{231A}'..='\u{23FF}' | '\u{24C2}' | '\u{25AA}'..='\u{25FE}'
            | '\u{2600}'..='\u{27BF}' | '\u{2934}'..='\u{2935}' | '\u{2B05}'..='\u{2B55}'
            | '\u{3030}' | '\u{303D}' | '\u{3297}' | '\u{3299}' | '\u{1F000}'..='\u{1FAFF}'
    )
}

/// A combining mark of the general-purpose mark blocks. A full Mn/Mc/Me table is the exact form.
fn is_generic_combining_mark(c: char) -> bool {
    matches!(
        c,
        '\u{0300}'..='\u{036F}' | '\u{0483}'..='\u{0489}' | '\u{1AB0}'..='\u{1AFF}'
            | '\u{1DC0}'..='\u{1DFF}' | '\u{20D0}'..='\u{20FF}' | '\u{2DE0}'..='\u{2DFF}'
            | '\u{3099}'..='\u{309A}' | '\u{FE20}'..='\u{FE2F}'
    )
}

/// Why `c`, which follows `prev` (`None` at the start), cannot be part of a name, other than
/// [`is_hidden`] and a run of spaces.
fn char_defect(prev: Option<char>, c: char) -> Option<String> {
    let code = u32::from(c);
    if is_blank_or_ignorable(c) {
        return Some(format!(
            "holds a blank, filler or invisible character (U+{code:04X})"
        ));
    }
    if is_odd_space(c) {
        return Some(format!(
            "holds a space other than the ordinary space (U+{code:04X})"
        ));
    }
    if c == '\u{FE0E}' || c == '\u{FE0F}' {
        return match prev {
            Some(p) if is_emoji_base(p) => None,
            _ => Some(format!(
                "holds a variation selector that does not follow an emoji (U+{code:04X})"
            )),
        };
    }
    if is_generic_combining_mark(c) && matches!(prev, None | Some(' ')) {
        return Some(format!(
            "holds a combining mark with no character to combine with (U+{code:04X})"
        ));
    }
    None
}

/// The first character of `text` that must not be stored in text a person reads as a line or
/// inside a sentence (a review prompt, a curriculum objective): everything [`is_hidden`] names
/// except TAB and LF, and every blank, filler or invisible character of
/// [`is_blank_or_ignorable`]. Ordinary spaces, no-break spaces, non-ASCII letters and the emoji
/// selectors U+FE0E and U+FE0F pass (an emoji needs its selector; a stray one is not judged here):
/// unlike a name, such text may hold them (audit 2026-11-19, P2A-03).
pub fn hidden_in_text(text: &str) -> Option<char> {
    text.chars()
        .find(|c| (is_hidden(*c) && *c != '\t' && *c != '\n') || is_blank_or_ignorable(*c))
}

/// `text` as one printable line: every run of whitespace (line breaks, tabs and
/// the Unicode separators included) becomes one space, every other hidden
/// character becomes U+FFFD so that its presence shows, and the result is capped
/// at `max` characters with an ellipsis.
pub fn one_line(text: &str, max: usize) -> String {
    let mut out = String::new();
    let mut last_space = true;
    let mut prev: Option<char> = None;
    for c in text.chars() {
        if c.is_whitespace() {
            if !last_space {
                out.push(' ');
            }
            last_space = true;
            prev = Some(' ');
            continue;
        }
        let shown = if is_hidden(c) || char_defect(prev, c).is_some() {
            '\u{FFFD}'
        } else {
            c
        };
        out.push(shown);
        last_space = false;
        prev = Some(c);
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
/// `None` when it can: a name has visible text, no hidden character, no blank, filler or
/// invisible character, no space but U+0020 and no run of them, an emoji selector only after an
/// emoji and a combining mark only after a base character.
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
    let mut prev: Option<char> = None;
    for c in text.chars() {
        if let Some(why) = char_defect(prev, c) {
            return Some(why);
        }
        prev = Some(c);
    }
    // A run of spaces inside the name (spaces at either end are trimmed when the name is stored).
    if text.trim_matches(' ').contains("  ") {
        return Some("holds two or more spaces in a row".to_string());
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
    #[test]
    fn a_name_has_no_blank_filler_or_stray_selector() {
        // Ordinary text of every kind stays valid, emoji with their selector included.
        for ok in [
            "Dvo\u{159}\u{e1}k \u{2013} Symphony No. 9",
            "\u{5C71}\u{7530}\u{592A}\u{90CE}",
            "\u{5D0}\u{5E9}\u{5DC}\u{5D5}\u{5DD} 2026",
            "e\u{301}clair",
            "Plan \u{2764}\u{FE0F}",
            "1\u{FE0F}\u{20E3} first",
            "\u{00A9}\u{FE0E} text style",
            "\u{1F1E9}\u{1F1EA}",
        ] {
            assert_eq!(name_defect(ok), None, "{ok:?}");
        }
        for (hostile, code) in [
            ("invoice\u{3164}.exe", "U+3164"),
            ("\u{2800}", "U+2800"),
            ("a\u{FFA0}b", "U+FFA0"),
            ("a\u{115F}\u{1160}b", "U+115F"),
            ("a\u{034F}b", "U+034F"),
            ("a\u{17B5}b", "U+17B5"),
            ("a\u{180B}b", "U+180B"),
            ("a\u{FFFC}b", "U+FFFC"),
            ("a\u{E0100}b", "U+E0100"),
            ("a\u{FE00}b", "U+FE00"),
            ("a\u{E000}b", "U+E000"),
            ("a\u{FDD0}b", "U+FDD0"),
            ("a\u{600}b", "U+0600"),
            ("a\u{00A0}b", "U+00A0"),
            ("a\u{3000}b", "U+3000"),
            ("a\u{202F}b", "U+202F"),
            ("a\u{FE0F}b", "U+FE0F"),
            ("\u{FE0F}", "U+FE0F"),
            ("\u{2764}\u{FE0F}\u{FE0F}", "U+FE0F"),
            ("\u{301}abc", "U+0301"),
            ("abc \u{301}def", "U+0301"),
        ] {
            let why = name_defect(hostile).unwrap_or_default();
            assert!(why.contains(code), "{hostile:?}: {why}");
        }
        assert_eq!(
            name_defect("report   final").as_deref(),
            Some("holds two or more spaces in a row")
        );
        // Spaces at either end are trimmed when the name is stored.
        assert_eq!(name_defect("  Dentist  "), None);
    }

    #[test]
    fn free_text_may_keep_layout_and_spaces_but_not_hidden_or_blank_characters() {
        assert_eq!(
            hidden_in_text("a\tb\nc \u{00A0} d \u{3000} \u{FE00}x"),
            Some('\u{FE00}')
        );
        // The emoji selectors pass: an emoji needs its selector.
        assert_eq!(hidden_in_text("Plan \u{2764}\u{FE0F}"), None);
        assert_eq!(hidden_in_text("a\tb\nc \u{00A0} d \u{3000} e"), None);
        assert_eq!(
            hidden_in_text("Recall the flag \u{202E}.tpircs"),
            Some('\u{202E}')
        );
        assert_eq!(hidden_in_text("x\u{3164}"), Some('\u{3164}'));
        assert_eq!(hidden_in_text("a\r\nb"), Some('\r'));
        assert_eq!(hidden_in_text("\u{5831}\u{544a} \u{fc}ber"), None);
    }

    #[test]
    fn one_line_shows_the_blank_and_the_stray_selector() {
        assert_eq!(one_line("a\u{3164}b\u{2800}c", 96), "a\u{FFFD}b\u{FFFD}c");
        assert_eq!(one_line("a\u{FE0F}b", 96), "a\u{FFFD}b");
        assert_eq!(
            one_line("Plan \u{2764}\u{FE0F}", 96),
            "Plan \u{2764}\u{FE0F}"
        );
        assert_eq!(one_line("x\u{E0100}\u{E0101}y", 96), "x\u{FFFD}\u{FFFD}y");
        assert_eq!(one_line("a\u{00A0}\u{3000}b", 96), "a b");
    }
}
