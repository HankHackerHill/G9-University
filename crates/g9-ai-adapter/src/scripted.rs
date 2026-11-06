//! Deterministic test double for [`InferenceRuntime`]. Returns queued
//! responses or errors in FIFO order and records **metadata only** about each
//! request (never segment text, never schemas), so tests can assert what was
//! asked without a recorded prompt ever existing.
//!
//! Used by this crate's gate tests and by `g9-skills`/`g9-app` tests. Not a
//! model; never selected at runtime.

use crate::cancel::CancelToken;
use crate::runtime::{
    FinishReason, InferenceError, InferenceRequest, InferenceResponse, InferenceRuntime, ModelRef,
    RuntimeInfo, SegmentRole, Usage,
};
use g9_domain::{DurationMs, InvocationId, TrustClass};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Mutex;

/// Adapter name reported by [`ScriptedRuntime::describe`].
pub const SCRIPTED_RUNTIME_NAME: &str = "scripted";

/// What the double remembers about one request: shape and budget, no content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordedRequest {
    /// Invocation id of the request.
    pub invocation_id: InvocationId,
    /// Template version of the request.
    pub template_version: String,
    /// Roles of the segments, in request order.
    pub segment_roles: Vec<SegmentRole>,
    /// Trust classes of the segments, in request order.
    pub segment_trusts: Vec<TrustClass>,
    /// Sum of the segments' token estimates.
    pub prompt_tokens_estimate: u32,
    /// Requested output cap.
    pub max_output_tokens: u32,
    /// Requested timeout.
    pub timeout: DurationMs,
    /// Requested temperature.
    pub temperature: f32,
    /// Whether a response schema was attached.
    pub expects_structured: bool,
    /// Whether the cancel token was already set when the request arrived.
    pub cancelled_on_entry: bool,
}

impl RecordedRequest {
    fn from_request(req: &InferenceRequest, cancelled_on_entry: bool) -> Self {
        Self {
            invocation_id: req.invocation_id.clone(),
            template_version: req.template_version.clone(),
            segment_roles: req.segments.iter().map(|s| s.role).collect(),
            segment_trusts: req.segments.iter().map(|s| s.trust).collect(),
            prompt_tokens_estimate: req.prompt_tokens_estimate(),
            max_output_tokens: req.max_output_tokens,
            timeout: req.timeout,
            temperature: req.temperature,
            expects_structured: req.expects_structured(),
            cancelled_on_entry,
        }
    }
}

/// Scripted runtime. `Send + Sync` via internal mutexes; a poisoned mutex is
/// recovered (the double holds no invariant that a panic could break).
#[derive(Debug)]
pub struct ScriptedRuntime {
    model: ModelRef,
    info: RuntimeInfo,
    queue: Mutex<VecDeque<Result<InferenceResponse, InferenceError>>>,
    recorded: Mutex<Vec<RecordedRequest>>,
}

impl ScriptedRuntime {
    /// A scripted runtime that reports `model` and `max_context_tokens`.
    pub fn new(model: ModelRef, max_context_tokens: u32) -> Self {
        Self {
            info: RuntimeInfo {
                name: SCRIPTED_RUNTIME_NAME.to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                model: Some(model.clone()),
                max_context_tokens,
                supports_structured_output: false,
            },
            model,
            queue: Mutex::new(VecDeque::new()),
            recorded: Mutex::new(Vec::new()),
        }
    }

    /// Report `supports_structured_output = true` (behaviour is unchanged; the
    /// flag only tells callers what to expect).
    pub fn with_structured_output_support(mut self, supported: bool) -> Self {
        self.info.supports_structured_output = supported;
        self
    }

    /// Queue a response.
    pub fn push_response(&self, response: InferenceResponse) -> &Self {
        self.lock_queue().push_back(Ok(response));
        self
    }

    /// Queue an error.
    pub fn push_error(&self, error: InferenceError) -> &Self {
        self.lock_queue().push_back(Err(error));
        self
    }

    /// Build a `Stop` response for this runtime's model with the given text and
    /// optional structured value; usage is zero (no measurement is invented).
    pub fn response(
        &self,
        text: impl Into<String>,
        structured: Option<serde_json::Value>,
    ) -> InferenceResponse {
        InferenceResponse {
            text: text.into(),
            structured,
            usage: Usage::default(),
            finish: FinishReason::Stop,
            model: self.model_ref(),
            template_version: String::new(),
        }
    }

    /// The model this double reports.
    pub fn model_ref(&self) -> ModelRef {
        self.model.clone()
    }

    /// Snapshot of the recorded request metadata, in call order.
    pub fn recorded(&self) -> Vec<RecordedRequest> {
        self.recorded
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    /// Number of scripted items not yet consumed.
    pub fn remaining(&self) -> usize {
        self.lock_queue().len()
    }

    fn lock_queue(
        &self,
    ) -> std::sync::MutexGuard<'_, VecDeque<Result<InferenceResponse, InferenceError>>> {
        self.queue.lock().unwrap_or_else(|p| p.into_inner())
    }
}

impl InferenceRuntime for ScriptedRuntime {
    fn describe(&self) -> RuntimeInfo {
        self.info.clone()
    }

    /// Records metadata, then: cancelled token → `Cancelled`; next scripted
    /// item if any; otherwise `ModelUnavailable` (an exhausted script behaves
    /// like a missing model rather than inventing output).
    fn generate(
        &self,
        req: InferenceRequest,
        cancel: &CancelToken,
    ) -> Result<InferenceResponse, InferenceError> {
        let cancelled = cancel.is_cancelled();
        self.recorded
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(RecordedRequest::from_request(&req, cancelled));
        if cancelled {
            return Err(InferenceError::Cancelled);
        }
        let mut item = self
            .lock_queue()
            .pop_front()
            .unwrap_or(Err(InferenceError::ModelUnavailable));
        if let Ok(resp) = &mut item {
            if resp.template_version.is_empty() {
                resp.template_version = req.template_version;
            }
        }
        item
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::ContextSegment;
    use g9_domain::{Classification, ModelId};
    use pretty_assertions::assert_eq;

    fn req(text: &str) -> InferenceRequest {
        InferenceRequest {
            invocation_id: InvocationId::parse("inv-s").unwrap(),
            segments: vec![ContextSegment::new(
                SegmentRole::Task,
                text,
                TrustClass::SystemDeterministic,
                Classification::Private,
                "task",
            )],
            max_output_tokens: 8,
            timeout: DurationMs::from_seconds_u32(1),
            response_schema: None,
            temperature: 0.0,
            template_version: "tpl-7".into(),
        }
    }

    #[test]
    fn fifo_then_unavailable_and_metadata_only() {
        let rt = ScriptedRuntime::new(ModelRef::new(ModelId::parse("m").unwrap()), 100);
        rt.push_response(rt.response("hello", None))
            .push_error(InferenceError::Timeout);
        assert_eq!(rt.remaining(), 2);
        let c = CancelToken::new();
        let r1 = rt.generate(req("PRIVATE QUESTION"), &c).unwrap();
        assert_eq!(r1.text, "hello");
        assert_eq!(r1.template_version, "tpl-7");
        assert_eq!(rt.generate(req("q2"), &c), Err(InferenceError::Timeout));
        assert_eq!(
            rt.generate(req("q3"), &c),
            Err(InferenceError::ModelUnavailable)
        );
        let rec = rt.recorded();
        assert_eq!(rec.len(), 3);
        assert_eq!(rec[0].segment_roles, vec![SegmentRole::Task]);
        assert_eq!(rec[0].prompt_tokens_estimate, 4);
        let dumped = serde_json::to_string(&rec).unwrap();
        assert!(!dumped.contains("PRIVATE QUESTION"));
        assert!(!dumped.contains("\"text\""));
        assert!(!dumped.contains("\"segments\""));
    }

    #[test]
    fn cancelled_token_short_circuits() {
        let rt = ScriptedRuntime::new(ModelRef::new(ModelId::parse("m").unwrap()), 100);
        rt.push_response(rt.response("never", None));
        let c = CancelToken::new();
        c.cancel();
        assert_eq!(rt.generate(req("q"), &c), Err(InferenceError::Cancelled));
        assert_eq!(rt.remaining(), 1);
        assert!(rt.recorded()[0].cancelled_on_entry);
    }
}
