//! Strict minimal parser for the `SKILL.md` frontmatter
//! (`docs/skills/skill-package-contract.md`, "SKILL.md frontmatter").
//!
//! Supported grammar — nothing else, and no YAML engine is used:
//!
//! * the file starts with a `---` line; the frontmatter ends at the next `---` line;
//! * blank lines and lines starting with `#` are ignored;
//! * top-level entries are `key: value` with `key` in `[a-z][a-z0-9-]*`;
//! * `metadata:` introduces a one-level map whose entries are indented by exactly two spaces;
//! * values are double-quoted (escapes `\"`, `\\`, `\/`, `\n`, `\t`), single-quoted (`''` escape)
//!   or bare (a trailing ` #comment` is stripped); bare values starting with
//!   `[ { & * ! | > % @` or a backtick are refused;
//! * tabs, duplicate keys, unknown top-level keys and missing required keys are errors.
//!
//! The only top-level keys are `name`, `description`, `license`, `metadata`
//! (required) and the base Agent Skills optionals `compatibility` and
//! `allowed-tools` (kept verbatim; the G9 runtime ignores them).

use std::collections::BTreeMap;

/// Keys every frontmatter must carry.
pub const REQUIRED_TOP_LEVEL: [&str; 4] = ["name", "description", "license", "metadata"];
/// Optional keys from the base Agent Skills format; accepted and kept verbatim.
pub const OPTIONAL_TOP_LEVEL: [&str; 2] = ["compatibility", "allowed-tools"];

/// Frontmatter parse failure. Messages name line numbers and keys, never values.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FrontmatterError {
    /// The file does not start with `---`.
    #[error("SKILL.md must start with a `---` line")]
    MissingOpen,
    /// No closing `---` line.
    #[error("frontmatter closing `---` not found")]
    MissingClose,
    /// A line does not follow the grammar.
    #[error("frontmatter line {line}: {reason}")]
    Syntax {
        /// 1-based line number in `SKILL.md`.
        line: usize,
        /// What was expected.
        reason: String,
    },
    /// A required top-level key is absent.
    #[error("frontmatter: required key `{0}` missing")]
    MissingKey(&'static str),
    /// A top-level key outside the contract.
    #[error("frontmatter: unknown top-level key `{0}`")]
    UnknownKey(String),
    /// The same key appears twice (top-level or inside `metadata`).
    #[error("frontmatter: duplicate key `{0}`")]
    DuplicateKey(String),
}

/// Parsed frontmatter. All values are strings; `metadata` is one level deep.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Frontmatter {
    /// Agent Skills `name`.
    pub name: String,
    /// Agent Skills `description`.
    pub description: String,
    /// SPDX license id as written.
    pub license: String,
    /// Optional base-format key, verbatim.
    pub compatibility: Option<String>,
    /// Optional base-format key, verbatim.
    pub allowed_tools: Option<String>,
    /// The `metadata:` map (`g9-*` keys).
    pub metadata: BTreeMap<String, String>,
    /// Whether each metadata value was quoted (the contract example quotes all of them).
    pub metadata_quoted: BTreeMap<String, bool>,
}

/// Frontmatter plus the Markdown body that follows the closing `---`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedSkillMd {
    /// The parsed header.
    pub frontmatter: Frontmatter,
    /// Everything after the closing `---` line (line endings normalised to `\n`).
    pub body: String,
}

/// Parse a complete `SKILL.md` text.
pub fn parse(text: &str) -> Result<ParsedSkillMd, FrontmatterError> {
    let lines: Vec<&str> = text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    if lines.first().copied() != Some("---") {
        return Err(FrontmatterError::MissingOpen);
    }
    let end = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| **l == "---")
        .map(|(i, _)| i)
        .ok_or(FrontmatterError::MissingClose)?;
    let body = lines.get(end + 1..).unwrap_or(&[]).join("\n");

    let mut top: BTreeMap<String, String> = BTreeMap::new();
    let mut metadata: BTreeMap<String, String> = BTreeMap::new();
    let mut metadata_quoted: BTreeMap<String, bool> = BTreeMap::new();
    let mut seen_metadata = false;
    let mut in_metadata = false;

    for (idx, line) in lines.iter().enumerate().take(end).skip(1) {
        let n = idx + 1;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if line.starts_with('\t') {
            return Err(syntax(n, "tabs are not allowed"));
        }
        if line.starts_with(' ') {
            if !in_metadata {
                return Err(syntax(n, "unexpected indented line outside `metadata:`"));
            }
            let entry = match line.strip_prefix("  ") {
                Some(e) if !e.starts_with(' ') => e,
                _ => {
                    return Err(syntax(
                        n,
                        "metadata entries must be indented by exactly two spaces",
                    ))
                }
            };
            let (key, raw) = split_entry(entry, n)?;
            if !valid_key(key) {
                return Err(syntax(n, format!("invalid metadata key `{key}`")));
            }
            if metadata.contains_key(key) {
                return Err(FrontmatterError::DuplicateKey(key.to_string()));
            }
            let (value, quoted) = parse_scalar(raw, n)?;
            metadata.insert(key.to_string(), value);
            metadata_quoted.insert(key.to_string(), quoted);
            continue;
        }
        in_metadata = false;
        let (key, raw) = split_entry(line, n)?;
        if !valid_key(key) {
            return Err(syntax(n, format!("invalid key `{key}`")));
        }
        if key == "metadata" {
            if seen_metadata {
                return Err(FrontmatterError::DuplicateKey(key.to_string()));
            }
            if !strip_comment(raw).is_empty() {
                return Err(syntax(n, "`metadata:` must introduce an indented block"));
            }
            seen_metadata = true;
            in_metadata = true;
            continue;
        }
        if top.contains_key(key) {
            return Err(FrontmatterError::DuplicateKey(key.to_string()));
        }
        let (value, _) = parse_scalar(raw, n)?;
        top.insert(key.to_string(), value);
    }

    for key in top.keys() {
        if !REQUIRED_TOP_LEVEL.contains(&key.as_str())
            && !OPTIONAL_TOP_LEVEL.contains(&key.as_str())
        {
            return Err(FrontmatterError::UnknownKey(key.clone()));
        }
    }
    let name = top
        .remove("name")
        .ok_or(FrontmatterError::MissingKey("name"))?;
    let description = top
        .remove("description")
        .ok_or(FrontmatterError::MissingKey("description"))?;
    let license = top
        .remove("license")
        .ok_or(FrontmatterError::MissingKey("license"))?;
    if !seen_metadata {
        return Err(FrontmatterError::MissingKey("metadata"));
    }
    Ok(ParsedSkillMd {
        frontmatter: Frontmatter {
            name,
            description,
            license,
            compatibility: top.remove("compatibility"),
            allowed_tools: top.remove("allowed-tools"),
            metadata,
            metadata_quoted,
        },
        body,
    })
}

fn syntax(line: usize, reason: impl Into<String>) -> FrontmatterError {
    FrontmatterError::Syntax {
        line,
        reason: reason.into(),
    }
}

fn valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn split_entry(line: &str, n: usize) -> Result<(&str, &str), FrontmatterError> {
    let Some((key, rest)) = line.split_once(':') else {
        return Err(syntax(n, "expected `key: value`"));
    };
    if !rest.is_empty() && !rest.starts_with(' ') {
        return Err(syntax(n, "expected a space after `:`"));
    }
    Ok((key.trim(), rest))
}

fn strip_comment(raw: &str) -> &str {
    let raw = raw.trim();
    if raw.starts_with('#') {
        return "";
    }
    match raw.find(" #") {
        Some(i) => raw.get(..i).unwrap_or("").trim_end(),
        None => raw,
    }
}

/// Returns `(value, was_quoted)`.
fn parse_scalar(raw: &str, n: usize) -> Result<(String, bool), FrontmatterError> {
    let raw = raw.trim();
    if let Some(inner) = raw.strip_prefix('"') {
        let mut out = String::new();
        let mut chars = inner.chars();
        loop {
            match chars.next() {
                None => return Err(syntax(n, "unterminated double-quoted scalar")),
                Some('\\') => match chars.next() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(other) => {
                        return Err(syntax(n, format!("unsupported escape `\\{other}`")))
                    }
                    None => return Err(syntax(n, "dangling escape in double-quoted scalar")),
                },
                Some('"') => {
                    let rest = chars.as_str().trim();
                    if !rest.is_empty() && !rest.starts_with('#') {
                        return Err(syntax(n, "trailing content after closing quote"));
                    }
                    return Ok((out, true));
                }
                Some(c) => out.push(c),
            }
        }
    }
    if let Some(inner) = raw.strip_prefix('\'') {
        let mut out = String::new();
        let mut chars = inner.chars().peekable();
        loop {
            match chars.next() {
                None => return Err(syntax(n, "unterminated single-quoted scalar")),
                Some('\'') => {
                    if chars.peek() == Some(&'\'') {
                        chars.next();
                        out.push('\'');
                        continue;
                    }
                    let rest: String = chars.collect();
                    let rest = rest.trim();
                    if !rest.is_empty() && !rest.starts_with('#') {
                        return Err(syntax(n, "trailing content after closing quote"));
                    }
                    return Ok((out, true));
                }
                Some(c) => out.push(c),
            }
        }
    }
    let value = strip_comment(raw);
    if let Some(first) = value.chars().next() {
        if "[{&*!|>%@`".contains(first) {
            return Err(syntax(
                n,
                format!("unsupported YAML construct starting with `{first}`; use a plain or quoted string"),
            ));
        }
    }
    Ok((value.to_string(), false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const GOOD: &str = "---\nname: g9-session-director\ndescription: Prepares the brief. Use at day start; do not use for grading.\nlicense: Apache-2.0\nmetadata:\n  g9-code: \"T01\"\n  g9-family: 'teaching'   # comment\n  g9-modes: STUDY,LAB\n  g9-depends-on: \"\"\n---\n\n## Mission\n\nBody.\n";

    #[test]
    fn parses_quoted_single_quoted_and_bare_values() {
        let p = parse(GOOD).unwrap();
        assert_eq!(p.frontmatter.name, "g9-session-director");
        assert_eq!(p.frontmatter.license, "Apache-2.0");
        assert_eq!(p.frontmatter.metadata["g9-code"], "T01");
        assert_eq!(p.frontmatter.metadata["g9-family"], "teaching");
        assert_eq!(p.frontmatter.metadata["g9-modes"], "STUDY,LAB");
        assert_eq!(p.frontmatter.metadata["g9-depends-on"], "");
        assert_eq!(p.frontmatter.metadata_quoted["g9-code"], true);
        assert_eq!(p.frontmatter.metadata_quoted["g9-modes"], false);
        assert!(p.body.starts_with("\n## Mission"));
    }

    #[test]
    fn crlf_is_accepted() {
        let text = GOOD.replace('\n', "\r\n");
        let p = parse(&text).unwrap();
        assert_eq!(p.frontmatter.metadata["g9-code"], "T01");
    }

    #[test]
    fn rejects_unknown_key_duplicate_key_tabs_and_yaml_features() {
        let unknown = GOOD.replacen("license:", "licence:", 1);
        assert_eq!(
            parse(&unknown).unwrap_err(),
            FrontmatterError::UnknownKey("licence".into())
        );
        let dup = GOOD.replacen("license: Apache-2.0\n", "license: a\nlicense: b\n", 1);
        assert_eq!(
            parse(&dup).unwrap_err(),
            FrontmatterError::DuplicateKey("license".into())
        );
        let tab = GOOD.replacen("  g9-code", "\tg9-code", 1);
        assert!(matches!(
            parse(&tab).unwrap_err(),
            FrontmatterError::Syntax { .. }
        ));
        let flow = GOOD.replacen("g9-modes: STUDY,LAB", "g9-modes: [STUDY, LAB]", 1);
        assert!(matches!(
            parse(&flow).unwrap_err(),
            FrontmatterError::Syntax { .. }
        ));
        let nested = GOOD.replacen("  g9-code: \"T01\"", "  g9:\n    code: T01", 1);
        assert!(matches!(
            parse(&nested).unwrap_err(),
            FrontmatterError::Syntax { .. }
        ));
        let nospace = GOOD.replacen("name: g9", "name:g9", 1);
        assert!(matches!(
            parse(&nospace).unwrap_err(),
            FrontmatterError::Syntax { .. }
        ));
    }

    #[test]
    fn rejects_missing_delimiters_and_keys() {
        assert_eq!(
            parse("name: x\n").unwrap_err(),
            FrontmatterError::MissingOpen
        );
        assert_eq!(
            parse("---\nname: x\n").unwrap_err(),
            FrontmatterError::MissingClose
        );
        let no_meta = "---\nname: a\ndescription: b\nlicense: c\n---\n";
        assert_eq!(
            parse(no_meta).unwrap_err(),
            FrontmatterError::MissingKey("metadata")
        );
        let no_name = "---\ndescription: b\nlicense: c\nmetadata:\n  k: v\n---\n";
        assert_eq!(
            parse(no_name).unwrap_err(),
            FrontmatterError::MissingKey("name")
        );
    }

    #[test]
    fn escapes_and_trailing_garbage() {
        assert_eq!(parse_scalar(r#""a\"b\\c\n""#, 1).unwrap().0, "a\"b\\c\n");
        assert_eq!(parse_scalar("'it''s'", 1).unwrap().0, "it's");
        assert!(parse_scalar(r#""a" b"#, 1).is_err());
        assert!(parse_scalar(r#""a\x""#, 1).is_err());
        assert_eq!(
            parse_scalar("plain value # c", 1).unwrap(),
            ("plain value".into(), false)
        );
    }
}
