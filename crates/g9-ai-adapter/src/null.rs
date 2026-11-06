//! Model-down runtime (A09, R-002). [`NullRuntime`] is the runtime that is
//! configured when no model is loaded or the operator disabled AI assistance.
//! Every generation fails with [`InferenceError::ModelUnavailable`], which maps
//! to `ErrorCategory::ResourceUnavailable`, so callers and the UI render an
//! explicit "model unavailable" state and the rest of the application keeps
//! working (calendar, records, exams, export, restore never require inference).

use crate::cancel::CancelToken;
use crate::runtime::{
    InferenceError, InferenceRequest, InferenceResponse, InferenceRuntime, RuntimeInfo,
};

/// Adapter name reported by [`NullRuntime::describe`].
pub const NULL_RUNTIME_NAME: &str = "null";

/// The always-unavailable runtime.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NullRuntime;

impl NullRuntime {
    /// Construct the model-down runtime.
    pub const fn new() -> Self {
        Self
    }
}

impl InferenceRuntime for NullRuntime {
    /// Reports `model: None` and a zero context window: nothing is loaded.
    fn describe(&self) -> RuntimeInfo {
        RuntimeInfo {
            name: NULL_RUNTIME_NAME.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            model: None,
            max_context_tokens: 0,
            supports_structured_output: false,
        }
    }

    /// Always `Err(ModelUnavailable)`; never blocks, never allocates a model.
    fn generate(
        &self,
        _req: InferenceRequest,
        _cancel: &CancelToken,
    ) -> Result<InferenceResponse, InferenceError> {
        Err(InferenceError::ModelUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::{DurationMs, ErrorCategory, G9Error, InvocationId};

    #[test]
    fn describe_reports_no_model() {
        let info = NullRuntime::new().describe();
        assert_eq!(info.name, NULL_RUNTIME_NAME);
        assert_eq!(info.model, None);
        assert_eq!(info.max_context_tokens, 0);
        assert!(!info.supports_structured_output);
    }

    #[test]
    fn generate_is_model_unavailable_even_when_cancelled() {
        let req = InferenceRequest {
            invocation_id: InvocationId::parse("inv-null").unwrap(),
            segments: vec![],
            max_output_tokens: 1,
            timeout: DurationMs::from_seconds_u32(1),
            response_schema: None,
            temperature: 0.0,
            template_version: "t".into(),
        };
        let cancel = CancelToken::new();
        cancel.cancel();
        let err = NullRuntime.generate(req, &cancel).unwrap_err();
        assert_eq!(err, InferenceError::ModelUnavailable);
        let g: G9Error = err.into();
        assert_eq!(g.category, ErrorCategory::ResourceUnavailable);
    }
}
