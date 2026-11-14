//! Exit codes and rendering.
//!
//! * exit 0 — success; the result is printed as JSON (or in the `--human` form).
//! * exit 2 — the backend refused (`G9Error`, printed serialized) or a scripted
//!   scenario did not `PASS` (`INCOMPLETE` or `FAIL`; its report is printed).
//! * exit 1 — usage or host I/O error (message on stderr), or a destructive
//!   command that was not confirmed (`g9 restore` without
//!   `--confirm-replace-live-database`: the plan on stdout, nothing changed).
//!
//! Nothing here ever prints the binding key: the only key handle in the
//! process is `g9_policy::BindingKey`, whose `Debug` output is redacted, and
//! it is never serialized.

use g9_domain::G9Error;
use serde_json::Value;

/// Exit code for success.
pub const EXIT_OK: i32 = 0;
/// Exit code for usage and host I/O errors.
pub const EXIT_USAGE: i32 = 1;
/// Exit code for a backend refusal (`G9Error`) or a scenario that did not PASS (`INCOMPLETE`/`FAIL`).
pub const EXIT_REFUSED: i32 = 2;

/// Every way a command ends without a result.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// Bad arguments or unusable host input (exit 1).
    #[error("usage: {0}")]
    Usage(String),
    /// Host filesystem problem (exit 1). Carries the `io::ErrorKind` only.
    #[error("io: {0}")]
    Io(String),
    /// The backend refused (exit 2). Printed serialized to stdout.
    #[error("backend refused: {} {}", .0.category_str(), .0.code)]
    Backend(G9Error),
    /// A scripted scenario did not PASS (`INCOMPLETE` or `FAIL`; exit 2). Carries the report.
    #[error("scenario {}", scenario_result(.0))]
    Scenario(Value),
    /// A destructive command was not confirmed (exit 1). Carries the plan; nothing was changed.
    #[error("not confirmed: nothing was changed")]
    Unconfirmed(Value),
}

/// Wire spelling of an error category (`APPROVAL_REQUIRED`, …).
pub trait CategoryStr {
    /// `SCREAMING_SNAKE_CASE` category name.
    fn category_str(&self) -> String;
}

impl CategoryStr for G9Error {
    fn category_str(&self) -> String {
        serde_json::to_value(self.category)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| format!("{:?}", self.category))
    }
}

/// The `result` field of a scenario report (`FAIL` when absent).
pub fn scenario_result(report: &Value) -> &str {
    report["result"].as_str().unwrap_or("FAIL")
}

impl From<G9Error> for CliError {
    fn from(e: G9Error) -> Self {
        CliError::Backend(e)
    }
}

impl CliError {
    /// Host I/O failure without the path or message text.
    pub fn io(what: &str, e: &std::io::Error) -> Self {
        CliError::Io(format!("{what}: {}", e.kind()))
    }

    /// Process exit code.
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Usage(_) | CliError::Io(_) | CliError::Unconfirmed(_) => EXIT_USAGE,
            CliError::Backend(_) | CliError::Scenario(_) => EXIT_REFUSED,
        }
    }
}

/// What a finished command hands back to `main`.
#[derive(Debug)]
pub struct Completion {
    /// Printed to stdout.
    pub value: Value,
    /// When set, the process must exit **without** running destructors (the
    /// crash drill leaves the database connection open on purpose).
    pub abandon: Option<crate::client::Client>,
}

impl Completion {
    /// Ordinary completion.
    pub fn of(value: Value) -> Self {
        Self {
            value,
            abandon: None,
        }
    }
}

/// Serialize a `G9Error` as the closed wire shape (`schemas/ipc/g9-error.schema.json`).
pub fn error_value(error: &G9Error) -> Value {
    serde_json::to_value(error).unwrap_or_else(|_| {
        serde_json::json!({
            "category": "INTERNAL",
            "code": "cli.serialize",
            "message": "internal error; the error could not be serialized",
        })
    })
}

/// Render a success value.
///
/// No output carries a hidden character raw (`g9_domain::text::is_hidden`): a
/// stored title or a path is data, and a terminal acts on an escape sequence or a
/// direction override before anyone reads it (security audit 2026-11-13). In JSON
/// such a character is written as a `\u` escape, which every parser reads back as
/// the same text; in the `--human` form it is written as `\u{…}`.
pub fn render_value(value: &Value, human: bool) -> String {
    if human {
        render_human(value)
    } else {
        json_for_terminal(
            &serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".to_string()),
        )
    }
}

/// `json` (as serde_json wrote it) with every hidden character inside a string
/// written as a JSON `\u` escape. serde_json already escapes U+0000 to U+001F, so
/// the only line breaks left are the ones between members, which stay.
fn json_for_terminal(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    for c in json.chars() {
        if c != '\n' && g9_domain::text::is_hidden(c) {
            let mut units = [0u16; 2];
            for unit in c.encode_utf16(&mut units) {
                out.push_str(&format!("\\u{unit:04x}"));
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// `text` for a terminal outside JSON: every hidden character as `\u{…}`, so a
/// value stays on its line and cannot move the cursor, recolour or reorder.
pub fn visible(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if g9_domain::text::is_hidden(c) {
            out.push_str(&format!("\\u{{{:04x}}}", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    out
}

/// `arg` as one word of a POSIX shell (sh, bash, dash, zsh): unchanged when it
/// holds only characters no shell interprets, otherwise in single quotes with
/// each single quote written as `'\''`.
pub fn sh_word(arg: &str) -> String {
    let plain = !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '/' | ':' | ',' | '-'));
    if plain {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

/// A command for the operator to run next, in two forms: the exact arguments as
/// a JSON array, and one line quoted for a POSIX shell. The line is `null` when
/// an argument holds a hidden character: a terminal acts on such a character
/// when it is pasted, before the shell reads the quotes, so no quoting makes the
/// line safe. Returns `(line, argv)`.
pub fn next_command(argv: &[String]) -> (Value, Value) {
    let hidden = argv
        .iter()
        .any(|a| a.chars().any(g9_domain::text::is_hidden));
    let line = if hidden {
        Value::Null
    } else {
        Value::String(
            argv.iter()
                .map(|a| sh_word(a))
                .collect::<Vec<_>>()
                .join(" "),
        )
    };
    (line, serde_json::json!(argv))
}

/// How to read the two forms of [`next_command`]; printed next to them.
pub const NEXT_COMMAND_NOTE: &str = "the line is quoted for a POSIX shell (sh, bash, zsh) and is null when a path holds a control or invisible character; in any other shell, or when it is null, pass the arguments of the list exactly as they are";

/// What to print and where for an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// Text for stdout, if any.
    pub stdout: Option<String>,
    /// Text for stderr, if any.
    pub stderr: Option<String>,
    /// Exit code.
    pub code: i32,
}

/// Render an error according to the exit-code contract.
pub fn render_error(error: &CliError, human: bool) -> Rendered {
    match error {
        CliError::Usage(msg) | CliError::Io(msg) => Rendered {
            stdout: None,
            // A message can quote a path the caller gave.
            stderr: Some(format!("g9: {}", visible(msg))),
            code: EXIT_USAGE,
        },
        CliError::Backend(e) => Rendered {
            stdout: Some(render_value(&error_value(e), human)),
            stderr: None,
            code: EXIT_REFUSED,
        },
        CliError::Scenario(report) => Rendered {
            stdout: Some(render_value(report, human)),
            stderr: Some(format!("g9: scenario {}", scenario_result(report))),
            code: EXIT_REFUSED,
        },
        CliError::Unconfirmed(plan) => Rendered {
            stdout: Some(render_value(plan, human)),
            stderr: Some(
                "g9: not confirmed; nothing was changed (pass --confirm-replace-live-database to replace the live database)"
                    .to_string(),
            ),
            code: EXIT_USAGE,
        },
    }
}

/// Human rendering: a scenario report with `steps` becomes a table; anything
/// else is an indented key/value listing.
pub fn render_human(value: &Value) -> String {
    let mut out = String::new();
    if let Some(steps) = value.get("steps").and_then(Value::as_array) {
        out.push_str(&render_steps_table(steps));
        if let Some(obj) = value.as_object() {
            for (k, v) in obj {
                if k == "steps" {
                    continue;
                }
                out.push_str(&format!("{}: {}\n", visible(k), compact(v)));
            }
        }
        return out;
    }
    render_tree(value, 0, &mut out);
    out
}

fn render_tree(value: &Value, indent: usize, out: &mut String) {
    let pad = " ".repeat(indent);
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                match v {
                    Value::Object(inner) if !inner.is_empty() => {
                        let k = visible(k);
                        out.push_str(&format!("{pad}{k}:\n"));
                        render_tree(v, indent + 2, out);
                    }
                    Value::Array(items) if items.iter().any(Value::is_object) => {
                        let k = visible(k);
                        out.push_str(&format!("{pad}{k}:\n"));
                        render_tree(v, indent + 2, out);
                    }
                    _ => out.push_str(&format!("{pad}{}: {}\n", visible(k), compact(v))),
                }
            }
        }
        Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                match item {
                    Value::Object(_) => {
                        out.push_str(&format!("{pad}- [{i}]\n"));
                        render_tree(item, indent + 2, out);
                    }
                    _ => out.push_str(&format!("{pad}- {}\n", compact(item))),
                }
            }
        }
        other => out.push_str(&format!("{pad}{}\n", compact(other))),
    }
}

fn compact(v: &Value) -> String {
    match v {
        Value::String(s) => visible(s),
        Value::Null => "null".to_string(),
        other => visible(&serde_json::to_string(other).unwrap_or_else(|_| "?".to_string())),
    }
}

fn render_steps_table(steps: &[Value]) -> String {
    let headers = ["step", "op", "category", "decision", "state after"];
    let rows: Vec<[String; 5]> = steps
        .iter()
        .map(|s| {
            [
                compact(s.get("step").unwrap_or(&Value::Null)),
                compact(s.get("op").unwrap_or(&Value::Null)),
                compact(s.get("category").unwrap_or(&Value::Null)),
                compact(s.get("decision").unwrap_or(&Value::Null)),
                compact(s.get("state_after").unwrap_or(&Value::Null)),
            ]
        })
        .collect();
    let mut widths: [usize; 5] = headers.map(str::len);
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if i < 4 {
                widths[i] = widths[i].max(cell.chars().count());
            }
        }
    }
    let line = |cells: [&str; 5]| -> String {
        let mut s = String::new();
        for (i, cell) in cells.iter().enumerate() {
            if i < 4 {
                s.push_str(&format!("{:<w$}  ", cell, w = widths[i]));
            } else {
                s.push_str(cell);
            }
        }
        s.trim_end().to_string() + "\n"
    };
    let mut out = line(headers);
    out.push_str(&line([
        &"-".repeat(widths[0]),
        &"-".repeat(widths[1]),
        &"-".repeat(widths[2]),
        &"-".repeat(widths[3]),
        "-----------",
    ]));
    for row in &rows {
        out.push_str(&line([&row[0], &row[1], &row[2], &row[3], &row[4]]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exit_codes_follow_the_contract() {
        assert_eq!(CliError::Usage("x".into()).exit_code(), 1);
        assert_eq!(CliError::Io("x".into()).exit_code(), 1);
        assert_eq!(
            CliError::Backend(G9Error::invalid_input("a.b", "m")).exit_code(),
            2
        );
        assert_eq!(CliError::Scenario(json!({})).exit_code(), 2);
        assert_eq!(CliError::Unconfirmed(json!({})).exit_code(), 1);
    }

    #[test]
    fn an_unconfirmed_plan_is_printed_on_stdout_with_exit_1() {
        let plan = json!({ "confirmed": false, "live": "/d/g9.sqlite" });
        let r = render_error(&CliError::Unconfirmed(plan.clone()), false);
        assert_eq!(r.code, 1);
        let printed: Value = serde_json::from_str(r.stdout.as_deref().unwrap()).unwrap();
        assert_eq!(printed, plan);
        assert!(r.stderr.as_deref().unwrap().contains("nothing was changed"));
    }

    #[test]
    fn backend_error_is_printed_serialized_on_stdout() {
        let e = CliError::Backend(G9Error::precondition_failed("x.y", "why"));
        let r = render_error(&e, false);
        assert_eq!(r.code, 2);
        assert!(r.stderr.is_none());
        let v: Value = serde_json::from_str(r.stdout.as_deref().unwrap()).unwrap();
        assert_eq!(v["category"], "PRECONDITION_FAILED");
        assert_eq!(v["code"], "x.y");
        let usage = render_error(&CliError::Usage("bad".into()), false);
        assert_eq!(usage.code, 1);
        assert!(usage.stdout.is_none());
        assert_eq!(usage.stderr.as_deref(), Some("g9: bad"));
        let incomplete = render_error(
            &CliError::Scenario(json!({ "result": "INCOMPLETE", "steps": [] })),
            false,
        );
        assert_eq!(incomplete.code, 2);
        assert_eq!(
            incomplete.stderr.as_deref(),
            Some("g9: scenario INCOMPLETE")
        );
        assert_eq!(
            render_error(&CliError::Scenario(json!({})), false)
                .stderr
                .as_deref(),
            Some("g9: scenario FAIL")
        );
    }

    #[test]
    fn human_rendering_makes_a_table_for_steps() {
        let report = json!({
            "steps": [
                { "step": 1, "op": "system.status", "category": "OK", "decision": "OK", "state_after": { "a": 1 } },
                { "step": 2, "op": "calendar.apply", "category": "APPROVAL_REQUIRED", "decision": "OK (expected)", "state_after": null }
            ],
            "passed": true
        });
        let text = render_human(&report);
        assert!(text.starts_with("step  op"));
        assert!(text.contains("calendar.apply"));
        assert!(text.contains("passed: true"));
        let tree = render_human(&json!({ "a": { "b": [1, 2] }, "c": "x" }));
        assert_eq!(tree, "a:\n  b: [1,2]\nc: x\n");
    }

    /// Stored text and paths reach a terminal: an escape sequence, a C1 control,
    /// a direction override or a tag character must not arrive raw in any form.
    const HOSTILE: &str = "t\u{1b}[2K\u{9b}31m\u{7f}\u{202e}gpj.exe\u{e0041}\nnext: forged";

    #[test]
    fn json_output_escapes_hidden_characters_and_still_parses_to_the_same_value() {
        let value = json!({ "title": HOSTILE, "list": [HOSTILE], HOSTILE: 1 });
        let text = render_value(&value, false);
        assert!(
            !text
                .chars()
                .any(|c| c != '\n' && g9_domain::text::is_hidden(c)),
            "{text:?}"
        );
        assert!(text.contains("\\u001b") && text.contains("\\u009b") && text.contains("\\u202e"));
        // A character beyond U+FFFF is written as its surrogate pair.
        assert!(text.contains("\\udb40\\udc41"), "{text}");
        let back: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(back, value);
        // Every line break left is one the printer put between members.
        assert_eq!(text.lines().count(), 7, "{text}");
    }

    #[test]
    fn human_output_keeps_each_value_on_its_line_and_shows_hidden_characters() {
        let text = render_human(&json!({ "title": HOSTILE, "n": { "inner": [HOSTILE] } }));
        assert!(
            !text
                .chars()
                .any(|c| c != '\n' && g9_domain::text::is_hidden(c)),
            "{text:?}"
        );
        assert!(
            !text.lines().any(|l| l.trim_start().starts_with("next:")),
            "{text}"
        );
        assert!(
            text.contains("\\u{001b}[2K")
                && text.contains("\\u{202e}")
                && text.contains("\\u{000a}"),
            "{text}"
        );
        let usage = render_error(&CliError::Usage(format!("cannot read {HOSTILE}")), true);
        assert!(!usage
            .stderr
            .unwrap()
            .chars()
            .any(g9_domain::text::is_hidden));
    }

    #[test]
    fn a_shell_word_is_quoted_unless_no_shell_would_interpret_it() {
        for plain in [
            "g9",
            "restore",
            "--data-dir",
            "/home/op/.local/share/com.g9.mentor",
            "bk-01J:2,3",
        ] {
            assert_eq!(sh_word(plain), plain);
        }
        assert_eq!(sh_word(""), "''");
        assert_eq!(sh_word("a b"), "'a b'");
        assert_eq!(sh_word("$(id)"), "'$(id)'");
        assert_eq!(sh_word("it's"), "'it'\\''s'");
        // A leading tilde or equals sign is expanded by some shells; a glob by all.
        for special in [
            "~/x", "=ls", "a*", "a;b", "a&b", "a|b", "a#b", "a\\b", "a\"b", "a`b`", "{a,b}", "a>b",
            "!x", "a%b", "a@b", "a+b",
        ] {
            assert!(sh_word(special).starts_with('\''), "{special}");
        }
    }

    #[test]
    fn a_next_command_has_an_exact_list_and_a_line_only_when_it_can_be_pasted() {
        let (line, argv) = next_command(&[
            "g9".to_string(),
            "init".to_string(),
            "--data-dir".to_string(),
            "/d/a b".to_string(),
        ]);
        assert_eq!(line, json!("g9 init --data-dir '/d/a b'"));
        assert_eq!(argv, json!(["g9", "init", "--data-dir", "/d/a b"]));
        for hidden in ["/d/a\nb", "/d/a\u{1b}[2Kb", "/d/a\u{202e}b", "/d/a\u{7f}"] {
            let (line, argv) = next_command(&["g9".to_string(), hidden.to_string()]);
            assert_eq!(line, Value::Null, "{hidden:?}");
            assert_eq!(argv[1], hidden);
        }
    }
}
