//! A stand-in for the desktop bridge, for tests: the same backend, driven by JSON lines.
//!
//! The desktop shell (`apps/desktop/src-tauri`) cannot be built where there is no GTK or
//! WebKit, but everything the window talks to is one function, `g9_command(op, meta,
//! payload)`. This example opens the backend exactly as the shell does (`bootstrap::open`, with
//! `native_confirmation = true`) and answers the same call over stdin and stdout, so that a
//! browser can drive the real interface against the real Rust backend
//! (`scripts/e2e/real_backend.cjs`). It is a test tool: it is not part of the shipped `g9`
//! binary, takes no network input and offers nothing `g9 call` does not.
//!
//! Protocol, one JSON object per line:
//!
//! * in:  `{"id": 1, "op": "notes.create", "meta": {..}, "payload": {..}}`
//! * out: `{"id": 1, "ok": <CommandOutcome>}` or `{"id": 1, "err": <G9Error>}`
//! * out, while an approval is being decided: `{"confirm": {"id": 1, "text": "..", "summary": {..}}}`
//!   followed by one line in: `{"answer": {"id": 1, "approve": true}}`. The text is what the
//!   native dialog shows; the answer is the operator's.
//!
//! Usage: `bridge_stdio --data-dir <dir> [--zone Europe/Berlin] [--program-start YYYY-MM-DD]`

use g9_cli::bootstrap::{self, BootOptions};
use g9_domain::IanaZone;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::PathBuf;

fn argument(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn emit(value: &Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{value}");
    let _ = out.flush();
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(data_dir) = argument(&args, "--data-dir") else {
        eprintln!("bridge_stdio: --data-dir <dir> is required");
        std::process::exit(1);
    };
    let mut options = BootOptions::new(PathBuf::from(data_dir));
    options.skills_dir = None;
    options.native_confirmation = true;
    if let Some(zone) = argument(&args, "--zone") {
        match IanaZone::parse(&zone) {
            Ok(z) => options.zone = Some(z),
            Err(_) => {
                eprintln!("bridge_stdio: {zone:?} is not an IANA zone");
                std::process::exit(1);
            }
        }
    }
    if let Some(date) = argument(&args, "--program-start") {
        match g9_cli::seed::parse_date(&date) {
            Ok(d) => options.program_start = Some(d),
            Err(_) => {
                eprintln!("bridge_stdio: --program-start must be YYYY-MM-DD");
                std::process::exit(1);
            }
        }
    }
    let mut booted = match bootstrap::open(&options) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("bridge_stdio: {e}");
            std::process::exit(1);
        }
    };
    emit(&json!({ "ready": { "warnings": booted.warnings } }));

    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();
    while let Some(Ok(line)) = lines.next() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(request) = serde_json::from_str::<Value>(&line) else {
            emit(&json!({ "fatal": "a line that is not JSON" }));
            continue;
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let op = request
            .get("op")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let meta = request.get("meta").cloned().unwrap_or(Value::Null);
        let payload = request.get("payload").cloned().unwrap_or(Value::Null);
        let mut confirm = |summary: &g9_app::ApprovalSummary| -> bool {
            emit(&json!({ "confirm": { "id": id, "text": summary.text(), "summary": summary } }));
            for answer in lines.by_ref() {
                let Ok(answer) = answer else { return false };
                let Ok(answer) = serde_json::from_str::<Value>(&answer) else {
                    continue;
                };
                if let Some(a) = answer.get("answer") {
                    if a.get("id") == Some(&id) {
                        return a.get("approve").and_then(Value::as_bool).unwrap_or(false);
                    }
                }
            }
            false
        };
        let outcome =
            g9_app::dispatch_confirming(&mut booted.ctx, &op, meta, payload, &mut confirm);
        match outcome {
            Ok(value) => emit(&json!({ "id": id, "ok": value })),
            Err(error) => emit(&json!({ "id": id, "err": g9_app_error_value(&error) })),
        }
    }
}

fn g9_app_error_value(error: &g9_domain::G9Error) -> Value {
    serde_json::to_value(error).unwrap_or_else(|_| {
        json!({ "category": "INTERNAL", "code": "bridge.serialize", "message": "internal error" })
    })
}
