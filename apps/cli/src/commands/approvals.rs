//! `g9 approve`, `g9 reject`, `g9 approvals`: wrappers over `approvals.decide`
//! and `approvals.list`. The decision is an operator command like any other
//! (policy, binding MAC, audit row), so the backend owns the outcome.

use crate::bootstrap::{self, BootOptions};
use crate::client::Client;
use crate::output::CliError;
use serde_json::{json, Value};

/// `approvals.decide APPROVE`.
pub fn approve(boot: &BootOptions, approval_id: &str) -> Result<Value, CliError> {
    let booted = bootstrap::open(boot)?;
    let mut client = Client::new(booted.ctx);
    Ok(client.approve(approval_id)?)
}

/// `approvals.decide REJECT`. The IPC contract carries no reason text
/// (PROPOSED); a given `--reason` is echoed as a warning on the outcome only.
pub fn reject(
    boot: &BootOptions,
    approval_id: &str,
    reason: Option<&str>,
) -> Result<Value, CliError> {
    let booted = bootstrap::open(boot)?;
    let mut client = Client::new(booted.ctx);
    let mut outcome = client.reject(approval_id)?;
    if let Some(reason) = reason {
        let note = format!(
            "WARNING approvals.reason_not_persisted: the operator note {reason:?} was not sent; \
             approvals.decide carries no reason field (PROPOSED)"
        );
        outcome = super::status::with_warnings(outcome, &[note]);
    }
    Ok(outcome)
}

/// `approvals.list`, optionally filtered by state.
pub fn list(boot: &BootOptions, state: Option<&str>) -> Result<Value, CliError> {
    let booted = bootstrap::open(boot)?;
    let mut client = Client::new(booted.ctx);
    let payload = match state {
        Some(s) => json!({ "state": s }),
        None => json!({}),
    };
    Ok(client.call("approvals.list", payload)?)
}
