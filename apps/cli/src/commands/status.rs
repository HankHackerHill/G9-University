//! `g9 status`: `system.status`. Bridge-level facts the backend cannot see
//! (key file fallback, zone fallback, skills registry state, read-only
//! storage) are appended to `CommandOutcome.warnings`, as the Tauri bridge does.

use crate::bootstrap::{self, BootOptions};
use crate::client::Client;
use crate::output::CliError;
use serde_json::{json, Value};

/// Run `status`.
pub fn run(boot: &BootOptions) -> Result<Value, CliError> {
    let booted = bootstrap::open(boot)?;
    let mut client = Client::new(booted.ctx);
    let outcome = client.call("system.status", json!({}))?;
    Ok(with_warnings(outcome, &booted.warnings))
}

/// Append `warnings` to the outcome envelope.
pub fn with_warnings(mut outcome: Value, warnings: &[String]) -> Value {
    if warnings.is_empty() {
        return outcome;
    }
    if let Some(envelope) = outcome.as_object_mut() {
        let entry = envelope
            .entry("warnings")
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(list) = entry.as_array_mut() {
            list.extend(warnings.iter().cloned().map(Value::String));
        }
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warnings_are_appended_to_the_envelope() {
        let out = with_warnings(
            json!({ "result": {}, "replayed": false }),
            &["WARNING a: b".into()],
        );
        assert_eq!(out["warnings"], json!(["WARNING a: b"]));
        let untouched = with_warnings(json!({ "result": {}, "replayed": false }), &[]);
        assert!(untouched.get("warnings").is_none());
    }
}
