//! `g9 call <op>`: one dispatch with client-built metadata. The payload is
//! inline JSON, a file, or `{}`; it is passed to the backend untouched (the
//! backend validates it against the operation's request schema before
//! deserializing it).

use crate::bootstrap::{self, BootOptions};
use crate::cli::{CallArgs, DomainArg, ModeArg};
use crate::client::Client;
use crate::meta::MetaOptions;
use crate::output::CliError;
use serde_json::Value;
use std::path::Path;

/// Parse the payload source.
pub fn payload_from(inline: Option<&str>, file: Option<&Path>) -> Result<Value, CliError> {
    let text = match (inline, file) {
        (Some(text), _) => text.to_string(),
        (None, Some(path)) => {
            std::fs::read_to_string(path).map_err(|e| CliError::io("payload file", &e))?
        }
        (None, None) => "{}".to_string(),
    };
    let value: Value = serde_json::from_str(&text).map_err(|e| {
        CliError::Usage(format!(
            "payload is not valid JSON (line {}, column {})",
            e.line(),
            e.column()
        ))
    })?;
    Ok(value)
}

/// Metadata options from the arguments.
pub fn meta_options(args: &CallArgs) -> MetaOptions {
    MetaOptions {
        approval_id: args.approval.clone(),
        expected_revision: args.expected_revision,
        domain: args.domain.map(DomainArg::as_wire).map(str::to_string),
        mode: args.mode.map(ModeArg::as_wire).map(str::to_string),
        idempotency_key: args.idempotency_key.clone(),
        trace_id: args.trace_id.clone(),
    }
}

/// Run `call`.
pub fn run(boot: &BootOptions, args: &CallArgs) -> Result<Value, CliError> {
    let payload = payload_from(args.payload.as_deref(), args.payload_file.as_deref())?;
    let opts = meta_options(args);
    let booted = bootstrap::open(boot)?;
    let mut client = Client::new(booted.ctx);
    Ok(client.call_with(&args.op, payload, &opts)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_sources() {
        assert_eq!(payload_from(None, None).unwrap(), serde_json::json!({}));
        assert_eq!(
            payload_from(Some("{\"a\":1}"), None).unwrap(),
            serde_json::json!({ "a": 1 })
        );
        assert!(matches!(
            payload_from(Some("{nope"), None),
            Err(CliError::Usage(_))
        ));
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("p.json");
        std::fs::write(&file, b"[1,2]").unwrap();
        assert_eq!(
            payload_from(None, Some(&file)).unwrap(),
            serde_json::json!([1, 2])
        );
        assert!(matches!(
            payload_from(None, Some(&dir.path().join("missing.json"))),
            Err(CliError::Io(_))
        ));
    }

    #[test]
    fn meta_options_map_wire_spellings() {
        let args = CallArgs {
            op: "x.y".into(),
            payload: None,
            payload_file: None,
            approval: Some("a".into()),
            expected_revision: Some(2),
            mode: Some(ModeArg::Maintenance),
            domain: Some(DomainArg::System),
            idempotency_key: None,
            trace_id: Some("t".into()),
        };
        let m = meta_options(&args);
        assert_eq!(m.mode.as_deref(), Some("MAINTENANCE"));
        assert_eq!(m.domain.as_deref(), Some("SYSTEM"));
        assert_eq!(m.approval_id.as_deref(), Some("a"));
        assert_eq!(m.expected_revision, Some(2));
        assert_eq!(m.trace_id.as_deref(), Some("t"));
        assert!(m.idempotency_key.is_none());
    }
}
