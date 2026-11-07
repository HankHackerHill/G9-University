//! Exit codes and rendering.
//!
//! * exit 0 — success; the result is printed as JSON (or in the `--human` form).
//! * exit 2 — the backend refused (`G9Error`, printed serialized) or a scripted
//!   scenario ended in FAIL (its report is printed).
//! * exit 1 — usage or host I/O error (message on stderr).
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
/// Exit code for a backend refusal (`G9Error`) or a scenario FAIL.
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
    /// A scripted scenario ended in FAIL (exit 2). Carries the report.
    #[error("scenario FAIL")]
    Scenario(Value),
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
            CliError::Usage(_) | CliError::Io(_) => EXIT_USAGE,
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
pub fn render_value(value: &Value, human: bool) -> String {
    if human {
        render_human(value)
    } else {
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".to_string())
    }
}

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
            stderr: Some(format!("g9: {msg}")),
            code: EXIT_USAGE,
        },
        CliError::Backend(e) => Rendered {
            stdout: Some(render_value(&error_value(e), human)),
            stderr: None,
            code: EXIT_REFUSED,
        },
        CliError::Scenario(report) => Rendered {
            stdout: Some(render_value(report, human)),
            stderr: Some("g9: scenario FAIL".to_string()),
            code: EXIT_REFUSED,
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
                out.push_str(&format!("{k}: {}\n", compact(v)));
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
                        out.push_str(&format!("{pad}{k}:\n"));
                        render_tree(v, indent + 2, out);
                    }
                    Value::Array(items) if items.iter().any(Value::is_object) => {
                        out.push_str(&format!("{pad}{k}:\n"));
                        render_tree(v, indent + 2, out);
                    }
                    _ => out.push_str(&format!("{pad}{k}: {}\n", compact(v))),
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
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        other => serde_json::to_string(other).unwrap_or_else(|_| "?".to_string()),
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
}
