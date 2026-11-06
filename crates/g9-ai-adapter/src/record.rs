//! Per-invocation metadata record (ARCHITECTURE.md §10 "raw prompts are not
//! persisted; per-invocation record keeps model id, template version, token
//! counts, latency, validation outcome"; handoff §13 "Model version and prompt
//! template tracked per invocation without storing unnecessary private raw
//! prompts").
//!
//! [`InvocationRecord`] is the only thing from an inference that `g9-app` is
//! expected to persist. It contains **no prompt, response, segment or error
//! text** — a unit test walks its serialized form and asserts that.

use crate::runtime::{FinishReason, InferenceError, InferenceResponse, ModelRef, Usage};
use g9_domain::{DurationMs, InvocationId, UtcMillis};
use serde::{Deserialize, Serialize};

/// Metadata-only record of one inference call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvocationRecord {
    /// Invocation this record describes.
    pub invocation_id: InvocationId,
    /// The model that served the call; `None` on the model-down path (A09),
    /// where no model was loaded but the attempt is still recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelRef>,
    /// Prompt template version used for the call.
    pub template_version: String,
    /// Prompt tokens as measured by the runtime.
    pub prompt_tokens: u32,
    /// Generated tokens as measured by the runtime.
    pub output_tokens: u32,
    /// Time to first token.
    pub ttft_ms: DurationMs,
    /// Total generation wall time.
    pub total_ms: DurationMs,
    /// Why generation stopped (or the failure class).
    pub finish: FinishReason,
    /// Whether the structured output satisfied the request schema (false also
    /// when the call failed before any output existed).
    pub validation_ok: bool,
    /// Number of context segments the assembler dropped (each drop was reported
    /// to the caller at assembly time; this is the count only).
    pub segments_dropped: u32,
    /// When the record was created.
    pub created_at: UtcMillis,
}

impl InvocationRecord {
    /// Record for an accepted response (after the late-response guard).
    pub fn from_response(
        invocation_id: InvocationId,
        response: &InferenceResponse,
        validation_ok: bool,
        segments_dropped: u32,
        created_at: UtcMillis,
    ) -> Self {
        Self {
            invocation_id,
            model: Some(response.model.clone()),
            template_version: response.template_version.clone(),
            prompt_tokens: response.usage.prompt_tokens,
            output_tokens: response.usage.output_tokens,
            ttft_ms: response.usage.ttft_ms,
            total_ms: response.usage.total_ms,
            finish: response.finish,
            validation_ok,
            segments_dropped,
            created_at,
        }
    }

    /// Record for a failed call. Only the error's finish reason is kept; the
    /// error text (possibly a runtime diagnostic) is deliberately not stored.
    pub fn from_error(
        invocation_id: InvocationId,
        model: Option<ModelRef>,
        template_version: impl Into<String>,
        error: &InferenceError,
        segments_dropped: u32,
        created_at: UtcMillis,
    ) -> Self {
        let usage = Usage::default();
        Self {
            invocation_id,
            model,
            template_version: template_version.into(),
            prompt_tokens: usage.prompt_tokens,
            output_tokens: usage.output_tokens,
            ttft_ms: usage.ttft_ms,
            total_ms: usage.total_ms,
            finish: error.finish_reason(),
            validation_ok: false,
            segments_dropped,
            created_at,
        }
    }
}

/// JSON keys that must never appear in a persisted invocation record.
pub const FORBIDDEN_RECORD_KEYS: [&str; 8] = [
    "prompt",
    "text",
    "content",
    "segments",
    "response",
    "messages",
    "structured",
    "message",
];

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::ModelId;
    use serde_json::Value;

    fn keys(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                for (k, v) in m {
                    out.push(k.clone());
                    keys(v, out);
                }
            }
            Value::Array(a) => a.iter().for_each(|v| keys(v, out)),
            _ => {}
        }
    }

    #[test]
    fn record_serializes_without_prompt_or_response_keys() {
        let resp = InferenceResponse {
            text: "THE PROMPT ECHO".into(),
            structured: Some(serde_json::json!({"k": "PRIVATE"})),
            usage: Usage::default(),
            finish: FinishReason::Stop,
            model: ModelRef::new(ModelId::parse("m").unwrap()),
            template_version: "t1".into(),
        };
        let rec = InvocationRecord::from_response(
            InvocationId::parse("inv").unwrap(),
            &resp,
            true,
            2,
            UtcMillis::from_millis(1),
        );
        let json = serde_json::to_value(&rec).unwrap();
        let mut ks = Vec::new();
        keys(&json, &mut ks);
        for k in &ks {
            assert!(
                !FORBIDDEN_RECORD_KEYS.contains(&k.as_str()),
                "forbidden key {k}"
            );
        }
        let s = json.to_string();
        assert!(!s.contains("PROMPT ECHO"));
        assert!(!s.contains("PRIVATE"));
        let back: InvocationRecord = serde_json::from_value(json).unwrap();
        assert_eq!(back, rec);
    }

    #[test]
    fn error_record_keeps_finish_reason_not_diagnostic() {
        let err = InferenceError::Runtime("CUDA OOM at /home/user/.cache/x".into());
        let rec = InvocationRecord::from_error(
            InvocationId::parse("inv").unwrap(),
            None,
            "t1",
            &err,
            0,
            UtcMillis::from_millis(1),
        );
        assert_eq!(rec.finish, FinishReason::Error);
        assert!(!rec.validation_ok);
        let s = serde_json::to_string(&rec).unwrap();
        assert!(!s.contains("CUDA"));
        assert!(!s.contains("/home/"));
        assert!(!s.contains("\"model\""));
    }
}
