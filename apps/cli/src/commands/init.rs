//! `g9 init`: create/open the data directory, run recovery (inside
//! `AppContext::builder(..).open()`), print the `RecoveryReport` and
//! `system.status`. Sessions that recovery moved to `INTERRUPTED` are listed
//! with their `UNCERTAIN` intervals so that the crash drill is observable.

use crate::bootstrap::{self, BootOptions};
use crate::client::Client;
use crate::output::CliError;
use serde_json::{json, Value};

/// Run `init`.
pub fn run(boot: &BootOptions) -> Result<Value, CliError> {
    let booted = bootstrap::open(boot)?;
    let report = booted.ctx.recovery_report().clone();
    let operator = booted.ctx.operator().id.as_str().to_string();
    let mut client = Client::new(booted.ctx);
    let status = client.call("system.status", json!({}))?;
    let interrupted = interrupted_sessions(&mut client, &report)?;
    Ok(json!({
        "data_dir": booted.layout.root,
        "database": booted.layout.database,
        "operator_id": operator,
        "zone": booted.zone.as_str(),
        "skills": booted.skills,
        "recovery": report,
        "interrupted_sessions": interrupted,
        "status": status["result"],
        "warnings": booted.warnings,
    }))
}

/// Reduced views of the sessions recovery interrupted at this open (state,
/// credit and interval continuity only).
fn interrupted_sessions(
    client: &mut Client,
    report: &g9_app::RecoveryReport,
) -> Result<Vec<Value>, CliError> {
    if report.interrupted_sessions.is_empty() {
        return Ok(Vec::new());
    }
    let wanted: Vec<&str> = report
        .interrupted_sessions
        .iter()
        .map(|s| s.as_str())
        .collect();
    let list = client.call("session.list", json!({}))?;
    let views = list["result"]["sessions"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| s["session_id"].as_str().is_some_and(|id| wanted.contains(&id)))
        .map(|s| {
            json!({
                "session_id": s["session_id"],
                "state": s["state"],
                "requires_correction": s["requires_correction"],
                "credited_ms": s["credited_ms"],
                "uncertain_ms": s["uncertain_ms"],
                "intervals": s["intervals"].as_array().cloned().unwrap_or_default().iter().map(|i| {
                    json!({ "interval_id": i["interval_id"], "continuity": i["continuity"], "closed": !i["end_utc"].is_null() })
                }).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(views)
}
