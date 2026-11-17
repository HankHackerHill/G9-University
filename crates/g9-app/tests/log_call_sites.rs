//! Source gate for the operations log (`g9_app::oplog`, plan step OBS-1).
//!
//! The log's rule is that no payload, title, path or free text reaches it. At
//! run time the sink keeps only allow-listed fields of a code-like shape, but a
//! shape cannot tell a migration name from a note title. This gate closes that
//! side at the source: every logging call of the backend must
//!
//! * be written `tracing::<level>!(...)` in full (no imported macro, no
//!   `event!`, no span), so this scan sees all of them;
//! * end with a string literal that formats nothing into itself;
//! * name only fields of [`g9_app::oplog::ALLOWED_FIELDS`];
//! * be in [`REGISTER`], the reviewed list of call sites. A new or changed
//!   call fails here until the register changes in the same change, which puts
//!   the expression it logs in front of a reviewer.
//!
//! Only production code is scanned: every `#[cfg(test)]` item is removed first.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Every logging call of the backend: `file level: arguments`, whitespace
/// collapsed. Review what a new line logs before adding it: a field may carry a
/// code, a number, an identifier the backend made or an enum variant, never
/// operator content or a path.
const REGISTER: &[&str] = &[
    "crates/g9-app/src/commands/backup.rs warn: code = e.code(), \"restore-test copy was not removed\"",
    "crates/g9-app/src/pipeline.rs error: code = %e.code, \"approval ledger insert failed\"",
    "crates/g9-app/src/pipeline.rs error: code = %e.code, \"approval ledger resync failed\"",
    "crates/g9-app/src/pipeline.rs error: code = %e.code, \"failed to record a declined confirmation\"",
    "crates/g9-app/src/pipeline.rs error: code = %e.code, \"failed to record a refusal whose approval request was not stored\"",
    "crates/g9-app/src/pipeline.rs error: code = %storage.code, original = %refusal.error.code, \"failed to record a refused command\"",
    "crates/g9-app/src/recovery.rs error: first_bad_seq = ?chain.first_bad_seq, \"audit chain verification failed; mutations are refused\"",
    "crates/g9-app/src/recovery.rs error: session = %session_id, \"policy refused recovery of an open interval\"",
    "crates/g9-app/src/recovery.rs error: version = schema.version, differences = report.schema_differences.len(), \"schema differs from the embedded migrations; mutations are refused\"",
    "crates/g9-app/src/recovery.rs warn: code = e.code(), \"migration dry-run copy was not removed\"",
    "crates/g9-app/src/recovery.rs warn: code = e.code(), \"refused pre-migration snapshot was not removed\"",
    "crates/g9-app/src/schemas.rs error: operation = op, differences, \"handler result violates its response schema\"",
    "crates/g9-app/src/schemas.rs error: schema = name, \"embedded IPC schema failed to compile\"",
    "crates/g9-storage/src/backup.rs warn: code = e.code(), \"scratch copy was not removed\"",
    "crates/g9-storage/src/connection.rs warn: ?pending, \"database opened with pending migrations; back up, dry-run, then migrate\"",
    "crates/g9-storage/src/error.rs error: kind = rusqlite_variant(&other), \"unmapped rusqlite error\"",
    "crates/g9-storage/src/error.rs error: sqlite = ?code, extended_code, \"unmapped sqlite failure\"",
    "crates/g9-storage/src/error.rs error: sqlite = ?error.code, extended_code = error.extended_code, \"sql input error\"",
    "crates/g9-storage/src/error.rs warn: context, kind = ?err.kind(), \"storage i/o failure\"",
    "crates/g9-storage/src/error.rs warn: sqlite = ?code, extended_code, \"sqlite i/o failure\"",
    "crates/g9-storage/src/migrations.rs info: version = m.version, migration = m.name, what = \"live\", \"migration applied\"",
    "crates/g9-storage/src/migrations.rs info: version = m.version, migration = m.name, what = \"rehearsal\", \"migration applied\"",
    "crates/g9-storage/src/migrations.rs warn: code = e.code(), \"failed rehearsal copy was not removed\"",
    "crates/g9-storage/src/restore.rs error: kind = ?e.kind(), \"restore could not put a quarantined file back\"",
    "crates/g9-storage/src/restore.rs error: kind = ?e.kind(), \"restore could not remove its quarantine link\"",
    "crates/g9-storage/src/restore.rs warn: code = e.code(), \"live WAL checkpoint failed; the sidecars are quarantined with the database\"",
    "crates/g9-storage/src/restore.rs warn: code = other.code(), \"live database could not be opened for locking; it is quarantined without a lock\"",
    "crates/g9-storage/src/restore.rs warn: kind = ?e.kind(), \"directory sync after restore failed\"",
    "crates/g9-storage/src/restore.rs warn: kind = ?e.kind(), what = message, \"a file a restore no longer needs was not removed\"",
    "crates/g9-storage/src/restore.rs warn: kind = ?e.kind(), what, \"restore swap failed\"",
];

/// How many times a registered line occurs in the source (the same call
/// written twice is two call sites).
const REPEATED: &[(&str, usize)] = &[(
    "crates/g9-app/src/pipeline.rs error: code = %e.code, \"approval ledger resync failed\"",
    2,
)];

const LEVELS: [&str; 5] = ["trace", "debug", "info", "warn", "error"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|x| x == "rs") {
            out.push(path);
        }
    }
}

/// Production sources of the backend: every crate's `src`, the command-line
/// driver and the desktop bridge.
fn production_sources() -> Vec<(String, String)> {
    let root = root();
    let mut files = Vec::new();
    for entry in fs::read_dir(root.join("crates")).unwrap() {
        rust_files(&entry.unwrap().path().join("src"), &mut files);
    }
    rust_files(&root.join("apps/cli/src"), &mut files);
    rust_files(&root.join("apps/cli/examples"), &mut files);
    rust_files(&root.join("apps/desktop/src-tauri/src"), &mut files);
    files.sort();
    assert!(files.len() > 100, "sources not found: {}", files.len());
    files
        .into_iter()
        .map(|path| {
            let name = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            (name, fs::read_to_string(&path).unwrap())
        })
        .collect()
}

/// `text` with every test-only item removed: an item whose attribute line
/// starts with `#[cfg(test)]` or `#[cfg(all(test`, together with the
/// attributes that follow it and the item itself (up to its `;`, or its
/// closing `}`). Strings, character literals and comments are skipped while the
/// braces are counted. An error names a gated item whose end cannot be found.
fn production_part(name: &str, text: &str) -> Result<String, String> {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while at < bytes.len() {
        let line_end = text[at..].find('\n').map_or(bytes.len(), |i| at + i + 1);
        let line = text[at..line_end].trim_start();
        if line.starts_with("#[cfg(test)]") || line.starts_with("#[cfg(all(test") {
            match item_end(text, line_end) {
                Some(end) => {
                    at = end;
                    continue;
                }
                None => return Err(format!("{name}: a test-only item has no end")),
            }
        }
        out.push_str(&text[at..line_end]);
        at = line_end;
    }
    Ok(out)
}

/// End (exclusive, after the line break) of the item that starts at `from`:
/// further attribute lines are part of it; the item ends at the first `;` at
/// brace depth 0 or at the `}` that closes its first `{`.
fn item_end(text: &str, from: usize) -> Option<usize> {
    let chars: Vec<(usize, char)> = text[from..].char_indices().collect();
    let mut depth = 0_i32;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i].1;
        let next = chars.get(i + 1).map(|x| x.1);
        match c {
            '/' if next == Some('/') => {
                while i < chars.len() && chars[i].1 != '\n' {
                    i += 1;
                }
                continue;
            }
            '/' if next == Some('*') => {
                i += 2;
                while i + 1 < chars.len() && !(chars[i].1 == '*' && chars[i + 1].1 == '/') {
                    i += 1;
                }
                i += 2;
                continue;
            }
            'r' if matches!(next, Some('"') | Some('#')) && !ident_before(&chars, i) => {
                // Raw string: r"...", r#"..."#, ...
                let mut j = i + 1;
                let mut hashes = 0;
                while j < chars.len() && chars[j].1 == '#' {
                    hashes += 1;
                    j += 1;
                }
                if j < chars.len() && chars[j].1 == '"' {
                    j += 1;
                    'raw: while j < chars.len() {
                        if chars[j].1 == '"' {
                            let mut k = 0;
                            while k < hashes && j + 1 + k < chars.len() && chars[j + 1 + k].1 == '#'
                            {
                                k += 1;
                            }
                            if k == hashes {
                                j += 1 + hashes;
                                break 'raw;
                            }
                        }
                        j += 1;
                    }
                    i = j;
                    continue;
                }
            }
            '"' => {
                i += 1;
                while i < chars.len() && chars[i].1 != '"' {
                    if chars[i].1 == '\\' {
                        i += 1;
                    }
                    i += 1;
                }
                i += 1;
                continue;
            }
            '\'' => {
                // A character literal ('x', '\n', '\u{7b}') or a lifetime ('a).
                if next == Some('\\') {
                    let mut j = i + 2;
                    while j < chars.len() && chars[j].1 != '\'' {
                        j += 1;
                    }
                    i = j + 1;
                    continue;
                }
                if chars.get(i + 2).map(|x| x.1) == Some('\'') {
                    i += 3;
                    continue;
                }
            }
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(line_after(text, from + chars[i].0));
                }
            }
            ';' if depth == 0 => return Some(line_after(text, from + chars[i].0)),
            _ => {}
        }
        i += 1;
    }
    None
}

fn ident_before(chars: &[(usize, char)], i: usize) -> bool {
    i > 0 && (chars[i - 1].1.is_alphanumeric() || chars[i - 1].1 == '_')
}

/// The position after the line break that follows `at`.
fn line_after(text: &str, at: usize) -> usize {
    text[at..].find('\n').map_or(text.len(), |i| at + i + 1)
}

#[derive(Debug, PartialEq, Eq)]
struct Call {
    level: String,
    /// Arguments with whitespace collapsed.
    body: String,
}

/// Every `tracing::<level>!( ... )` of `text`, with its arguments.
fn calls(text: &str) -> Vec<Call> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(found) = text[from..].find("tracing::") {
        let after = from + found + "tracing::".len();
        from = after;
        let Some(level) = LEVELS
            .iter()
            .find(|l| text[after..].starts_with(&format!("{l}!(")))
        else {
            continue;
        };
        let open = after + level.len() + 2;
        let Some(end) = closing_paren(&text[open..]) else {
            panic!("unbalanced logging call near byte {open}");
        };
        let body = text[open..open + end]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        out.push(Call {
            level: (*level).to_string(),
            body,
        });
        from = open + end;
    }
    out
}

/// Index of the `)` that closes a parenthesis opened just before `text`.
fn closing_paren(text: &str) -> Option<usize> {
    let mut depth = 1_u32;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in text.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// The arguments of a call, split at the commas that are not inside a string,
/// a parenthesis, a bracket or a brace.
fn arguments(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut depth = 0_i32;
    let mut in_string = false;
    let mut escaped = false;
    for c in body.chars() {
        if in_string {
            current.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }
        match c {
            '"' => {
                in_string = true;
                current.push(c);
            }
            '(' | '[' | '{' => {
                depth += 1;
                current.push(c);
            }
            ')' | ']' | '}' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                out.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}

/// The name of a field argument: `name = value`, `name = ?value`, `?name`,
/// `%name` or `name`.
fn field_name(argument: &str) -> String {
    let head = argument.split('=').next().unwrap_or(argument).trim();
    head.trim_start_matches(['?', '%']).to_string()
}

/// What is wrong with one call, if anything.
fn defects(call: &Call) -> Vec<String> {
    let mut out = Vec::new();
    let args = arguments(&call.body);
    let Some((message, fields)) = args.split_last() else {
        return vec!["no arguments".to_string()];
    };
    let literal = message.len() >= 2 && message.starts_with('"') && message.ends_with('"');
    if !literal {
        out.push(format!(
            "the last argument must be the message, a string literal: `{message}`"
        ));
    } else if message.replace("{{", "").contains('{') {
        out.push(format!(
            "the message formats a value into itself: {message}"
        ));
    }
    for field in fields {
        if field.starts_with('"') {
            out.push(format!(
                "a string literal before the message (format arguments are not allowed): {field}"
            ));
            continue;
        }
        let name = field_name(field);
        if !g9_app::oplog::ALLOWED_FIELDS.contains(&name.as_str()) {
            out.push(format!(
                "field `{name}` is not in g9_app::oplog::ALLOWED_FIELDS"
            ));
        }
    }
    out
}

/// Forms of logging this scan would not see, so they are not allowed.
fn unseen_forms(text: &str) -> Vec<&'static str> {
    let mut out = Vec::new();
    for (needle, what) in [
        (
            "use tracing",
            "an import from `tracing` (write `tracing::warn!(...)` in full)",
        ),
        ("tracing::event!", "`tracing::event!`"),
        ("tracing::span!", "a span"),
        ("_span!(", "a span"),
        (
            "tracing::instrument",
            "`#[tracing::instrument]` (it records arguments)",
        ),
        ("#[instrument", "`#[instrument]` (it records arguments)"),
    ] {
        if text.contains(needle) {
            out.push(what);
        }
    }
    out
}

#[test]
fn every_logging_call_is_literal_allow_listed_and_registered() {
    let mut problems = Vec::new();
    let mut found = Vec::new();
    for (name, text) in production_sources() {
        // The sink itself names the subscriber API; it has no logging call of its own.
        let is_sink = name == "crates/g9-app/src/oplog.rs";
        let production = match production_part(&name, &text) {
            Ok(part) => part,
            Err(problem) => {
                problems.push(problem);
                continue;
            }
        };
        if !is_sink {
            for what in unseen_forms(&production) {
                problems.push(format!("{name}: {what}"));
            }
        }
        for call in calls(&production) {
            for defect in defects(&call) {
                problems.push(format!("{name} {}: {defect}", call.level));
            }
            found.push(format!("{name} {}: {}", call.level, call.body));
        }
    }
    assert!(
        problems.is_empty(),
        "logging calls that could carry content:\n  {}",
        problems.join("\n  ")
    );

    let mut expected: Vec<String> = Vec::new();
    for line in REGISTER {
        let times = REPEATED
            .iter()
            .find(|(l, _)| l == line)
            .map_or(1, |(_, n)| *n);
        expected.extend(std::iter::repeat_n((*line).to_string(), times));
    }
    expected.sort();
    found.sort();
    let not_registered: Vec<&String> = found.iter().filter(|f| !expected.contains(f)).collect();
    let gone: Vec<&String> = expected.iter().filter(|e| !found.contains(e)).collect();
    assert!(
        found == expected,
        "the logging calls differ from the reviewed register (REGISTER in this file).\n\
         In the source, not in the register (review what each logs, then add it):\n  {not_registered:#?}\n\
         In the register, not in the source (remove it):\n  {gone:#?}\n\
         {} calls in the source, {} in the register",
        found.len(),
        expected.len()
    );
    let unique: BTreeSet<&str> = REGISTER.iter().copied().collect();
    assert_eq!(unique.len(), REGISTER.len(), "a register line is repeated");
}

#[test]
fn the_gate_finds_each_kind_of_defect() {
    let source = "fn f() {\n    tracing::warn!(kind = ?e.kind(), what, \"restore swap failed\");\n    \
                  tracing::error!(\n        title = %note.title,\n        \"refused\"\n    );\n    \
                  tracing::warn!(\"note {title} failed\");\n    \
                  tracing::info!(code = e.code(), \"took {} ms\", elapsed);\n    \
                  tracing::warn!(code = f(a, \"x, y\"), ?pending, \"a comma (inside) a string, and a ) too\");\n    \
                  tracing::debug!(message);\n}\n";
    let found = calls(source);
    assert_eq!(found.len(), 6);
    assert_eq!(found[0].level, "warn");
    assert_eq!(
        found[0].body,
        "kind = ?e.kind(), what, \"restore swap failed\""
    );
    assert_eq!(defects(&found[0]), Vec::<String>::new());
    assert_eq!(
        defects(&found[1]),
        vec!["field `title` is not in g9_app::oplog::ALLOWED_FIELDS"]
    );
    assert_eq!(
        defects(&found[2]),
        vec!["the message formats a value into itself: \"note {title} failed\""]
    );
    assert_eq!(
        defects(&found[3]),
        vec![
            "the last argument must be the message, a string literal: `elapsed`",
            "a string literal before the message (format arguments are not allowed): \"took {} ms\"",
        ]
    );
    // Commas and parentheses inside strings and calls do not split an argument.
    assert_eq!(
        arguments(&found[4].body),
        vec![
            "code = f(a, \"x, y\")",
            "?pending",
            "\"a comma (inside) a string, and a ) too\""
        ]
    );
    assert_eq!(defects(&found[4]), Vec::<String>::new());
    assert_eq!(
        defects(&found[5]),
        vec!["the last argument must be the message, a string literal: `message`"]
    );

    assert_eq!(
        unseen_forms("use tracing::warn;\n#[tracing::instrument]\nfn f() {}\n"),
        vec![
            "an import from `tracing` (write `tracing::warn!(...)` in full)",
            "`tracing::instrument]` (it records arguments)"
        ]
        .into_iter()
        .map(|s| if s.starts_with("`tracing::instrument]") {
            "`#[tracing::instrument]` (it records arguments)"
        } else {
            s
        })
        .collect::<Vec<_>>()
    );
}

#[test]
fn test_only_items_are_removed_wherever_they_are() {
    let src = "fn f() {}\n\n#[cfg(test)]\nmod tests;\n\n#[cfg(test)]\n#[derive(Debug)]\nenum Point {\n    A,\n}\n\n\
               fn g() {\n    tracing::warn!(code = c, \"kept\");\n}\n\n#[cfg(all(test, unix))]\nmod tests {\n    \
               fn t() {\n        let s = \"}}} not a close\";\n        let c = '}';\n        let r = r#\"}\"#;\n        \
               // } a comment\n        /* } */\n        tracing::warn!(title = 1, \"x\");\n    }\n}\n\nfn after() {\n    \
               tracing::error!(code = d, \"also kept\");\n}\n";
    let kept = production_part("a.rs", src).unwrap();
    assert_eq!(
        kept,
        "fn f() {}\n\n\n\nfn g() {\n    tracing::warn!(code = c, \"kept\");\n}\n\n\nfn after() {\n    \
         tracing::error!(code = d, \"also kept\");\n}\n"
    );
    let found: Vec<String> = calls(&kept).into_iter().map(|c| c.body).collect();
    assert_eq!(found, vec!["code = c, \"kept\"", "code = d, \"also kept\""]);
    let thread_local = "#[cfg(test)]\nthread_local! {\n    static X: u8 = 0;\n}\nfn h() {}\n";
    assert_eq!(
        production_part("a.rs", thread_local).unwrap(),
        "fn h() {}\n"
    );
    assert_eq!(
        production_part("a.rs", "#[cfg(test)]\nmod open {\n").unwrap_err(),
        "a.rs: a test-only item has no end"
    );
}
