//! Dispatch wrapper. Every call builds the wire metadata ([`crate::meta`]),
//! goes through [`g9_app::dispatch`] and is checked against the IPC contract
//! the UI is built against (`command-outcome.schema.json`,
//! `<op>.response.schema.json`, `g9-error.schema.json`); a violation is
//! surfaced as `INTERNAL cli.envelope_schema_violation` so that scripted runs
//! fail instead of silently accepting an unshaped answer.

use crate::meta::{build_meta, IdSource, MetaOptions};
use g9_app::AppContext;
use g9_domain::{ErrorCategory, G9Error, UtcMillis};
use serde_json::{json, Value};
use std::fmt;

/// A dispatching client over one opened context.
pub struct Client {
    /// The backend.
    pub ctx: AppContext,
    /// Request/idempotency/trace id source.
    pub ids: IdSource,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("operator", self.ctx.operator())
            .field("ids", &self.ids)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Client with uuid v7 ids.
    pub fn new(ctx: AppContext) -> Self {
        Self {
            ctx,
            ids: IdSource::Uuid,
        }
    }

    /// Client with deterministic ids.
    pub fn with_ids(ctx: AppContext, ids: IdSource) -> Self {
        Self { ctx, ids }
    }

    /// Current instant of the context clock.
    pub fn now(&self) -> UtcMillis {
        self.ctx.now()
    }

    /// Dispatch with default metadata.
    pub fn call(&mut self, op: &str, payload: Value) -> Result<Value, G9Error> {
        self.call_with(op, payload, &MetaOptions::default())
    }

    /// Dispatch with metadata options; the outcome is contract-checked.
    pub fn call_with(
        &mut self,
        op: &str,
        payload: Value,
        opts: &MetaOptions,
    ) -> Result<Value, G9Error> {
        let now = self.now();
        let meta = build_meta(op, opts, &mut self.ids, now);
        let out = g9_app::dispatch(&mut self.ctx, op, meta, payload);
        let envelope_ok = match &out {
            Ok(v) => {
                g9_app::schemas::validate_outcome(v).is_ok()
                    && v.get("result")
                        .is_some_and(|r| g9_app::schemas::validate_response(op, r).is_ok())
            }
            Err(e) => serde_json::to_value(e)
                .ok()
                .is_some_and(|v| g9_app::schemas::validate_error(&v).is_ok()),
        };
        if envelope_ok {
            out
        } else {
            Err(G9Error::internal_redacted("cli.envelope_schema_violation")
                .with_details(json!({ "operation": op })))
        }
    }

    /// `approvals.decide` with `APPROVE`.
    pub fn approve(&mut self, approval_id: &str) -> Result<Value, G9Error> {
        self.call(
            "approvals.decide",
            json!({ "approval_id": approval_id, "decision": "APPROVE" }),
        )
    }

    /// `approvals.decide` with `REJECT`.
    pub fn reject(&mut self, approval_id: &str) -> Result<Value, G9Error> {
        self.call(
            "approvals.decide",
            json!({ "approval_id": approval_id, "decision": "REJECT" }),
        )
    }

    /// Approval round trip: the first call must answer `APPROVAL_REQUIRED`,
    /// the operator approves, the retry presents the approval. Returns the
    /// approval id and the retry's outcome.
    pub fn run_approved(
        &mut self,
        op: &str,
        payload: Value,
        opts: &MetaOptions,
    ) -> Result<(String, Value), G9Error> {
        let first = self.call_with(op, payload.clone(), opts);
        let approval = match first {
            Err(e) if e.category == ErrorCategory::ApprovalRequired => e
                .approval_id
                .map(|a| a.as_str().to_string())
                .ok_or_else(|| G9Error::internal_redacted("cli.approval_id_missing"))?,
            Err(e) => return Err(e),
            Ok(_) => {
                return Err(G9Error::precondition_failed(
                    "cli.approval_not_required",
                    format!("{op} succeeded without an approval"),
                ))
            }
        };
        self.approve(&approval)?;
        let retry = MetaOptions {
            approval_id: Some(approval.clone()),
            ..opts.clone()
        };
        let out = self.call_with(op, payload, &retry)?;
        Ok((approval, out))
    }
}
