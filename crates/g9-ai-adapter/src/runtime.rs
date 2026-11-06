//! The replaceable runtime boundary (ARCHITECTURE.md §10, handoff §13
//! "Inference protocol"; R-013 swap/rollback).
//!
//! An [`InferenceRuntime`] receives an already assembled, provenance-tagged
//! [`InferenceRequest`] and returns an [`InferenceResponse`] or an
//! [`InferenceError`]. It never touches SQLite, the filesystem or native OS
//! APIs; it never decides authority. Every concrete runtime (llama.cpp-compatible,
//! Ollama-compatible, ...) is a separate adapter behind this trait, so a model or
//! runtime can be swapped or rolled back without changing callers.

use crate::cancel::{CancelToken, Guard};
use crate::validate::{validate_structured, ValidationIssue};
use g9_domain::{
    Classification, DurationMs, ErrorCategory, G9Error, InvocationId, ModelId, TrustClass,
};
use serde::{Deserialize, Serialize};

/// Reference to the exact model artifact a runtime is serving. The optional
/// checksum pins the artifact so that an invocation record can be matched to a
/// `model_registry` row even after a swap or rollback (R-013).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelRef {
    /// Registry identifier of the model.
    pub model_id: ModelId,
    /// Lowercase hex SHA-256 of the loaded weights artifact, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum_sha256: Option<String>,
}

impl ModelRef {
    /// Reference a model by id without pinning a checksum.
    pub fn new(model_id: ModelId) -> Self {
        Self {
            model_id,
            checksum_sha256: None,
        }
    }

    /// Pin the artifact checksum (lowercase hex SHA-256).
    pub fn with_checksum(mut self, checksum_sha256: impl Into<String>) -> Self {
        self.checksum_sha256 = Some(checksum_sha256.into());
        self
    }
}

/// Static description of a runtime, returned by [`InferenceRuntime::describe`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeInfo {
    /// Adapter name, e.g. `null`, `scripted`, `llama-cpp-compatible`.
    pub name: String,
    /// Adapter/runtime version string (recorded per invocation for R-013).
    pub version: String,
    /// The loaded model, or `None` when no model is loaded (model-down path).
    pub model: Option<ModelRef>,
    /// Context window in tokens; `0` when no model is loaded.
    pub max_context_tokens: u32,
    /// Whether the runtime can constrain decoding to a JSON Schema. When
    /// `false`, structured output is still validated after the fact (A11); the
    /// flag only tells the caller whether to expect a higher validity rate.
    pub supports_structured_output: bool,
}

/// Role of a context segment. Roles decide ordering and which trust classes may
/// fill them (see [`crate::context`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SegmentRole {
    /// Deterministic policy preamble produced by the orchestrator.
    SystemPolicy,
    /// Skill task instructions produced by the orchestrator's versioned template.
    SkillInstructions,
    /// The concrete task statement for this invocation.
    Task,
    /// Evidence: notes, attempts, references, prior model output. Data, never instructions.
    Evidence,
    /// What the learner typed for this turn.
    LearnerInput,
}

impl SegmentRole {
    /// All roles in prompt order.
    pub const ALL: [SegmentRole; 5] = [
        SegmentRole::SystemPolicy,
        SegmentRole::SkillInstructions,
        SegmentRole::Task,
        SegmentRole::Evidence,
        SegmentRole::LearnerInput,
    ];

    /// Whether the role is read by the model as instructions rather than data.
    pub const fn is_instruction_role(self) -> bool {
        matches!(
            self,
            SegmentRole::SystemPolicy | SegmentRole::SkillInstructions | SegmentRole::Task
        )
    }
}

/// Estimate of the token count of a text: `ceil(chars / 4)`.
///
/// This is a **heuristic**, not a tokenizer. It is used only for budgeting
/// before the runtime is invoked; the runtime's own usage report is the
/// measured value that is persisted in the invocation record.
pub fn estimate_tokens(text: &str) -> u32 {
    let chars = text.chars().count();
    let est = chars.div_ceil(4);
    u32::try_from(est).unwrap_or(u32::MAX)
}

/// One provenance-tagged piece of model context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSegment {
    /// Role of the segment (decides ordering and admissible trust).
    pub role: SegmentRole,
    /// The segment text. In memory only; never persisted.
    pub text: String,
    /// Trust class of the source of `text`.
    pub trust: TrustClass,
    /// Privacy classification of `text`; `Secret` is never admitted.
    pub classification: Classification,
    /// Stable reference to where the text came from (record id, template id, ...).
    pub source_ref: String,
    /// Estimated token count (see [`estimate_tokens`]); an estimate, never a measurement.
    pub approx_tokens: u32,
}

impl ContextSegment {
    /// Build a segment and compute its token estimate from the text.
    pub fn new(
        role: SegmentRole,
        text: impl Into<String>,
        trust: TrustClass,
        classification: Classification,
        source_ref: impl Into<String>,
    ) -> Self {
        let text = text.into();
        let approx_tokens = estimate_tokens(&text);
        Self {
            role,
            text,
            trust,
            classification,
            source_ref: source_ref.into(),
            approx_tokens,
        }
    }
}

/// A fully assembled request. Raw text lives only here, in memory, for the
/// duration of the call; it is never persisted (ARCHITECTURE.md §10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceRequest {
    /// Invocation this request belongs to (correlates the record).
    pub invocation_id: InvocationId,
    /// Assembled, filtered, ordered context (see [`crate::context`]).
    pub segments: Vec<ContextSegment>,
    /// Hard cap on generated tokens.
    pub max_output_tokens: u32,
    /// Hard wall-clock limit for the whole generation.
    pub timeout: DurationMs,
    /// JSON Schema (draft 2020-12) the structured output must satisfy, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_schema: Option<serde_json::Value>,
    /// Sampling temperature in `[0, 2]`.
    pub temperature: f32,
    /// Version of the prompt template that produced the instruction segments.
    pub template_version: String,
}

impl InferenceRequest {
    /// Sum of the segments' token estimates.
    pub fn prompt_tokens_estimate(&self) -> u32 {
        self.segments
            .iter()
            .fold(0u32, |acc, s| acc.saturating_add(s.approx_tokens))
    }

    /// Whether the request carries a schema the output must satisfy.
    pub fn expects_structured(&self) -> bool {
        self.response_schema.is_some()
    }

    /// Reject requests whose budget cannot be honoured before any runtime work
    /// happens: zero output budget, zero timeout, a temperature outside `[0, 2]`
    /// or a prompt estimate that cannot fit the runtime's context window
    /// together with the requested output.
    pub fn check_budget(&self, info: &RuntimeInfo) -> Result<(), InferenceError> {
        if self.max_output_tokens == 0 {
            return Err(InferenceError::Budget {
                reason: "max_output_tokens must be greater than zero".to_string(),
            });
        }
        if self.timeout == DurationMs::ZERO {
            return Err(InferenceError::Budget {
                reason: "timeout must be greater than zero".to_string(),
            });
        }
        if !(0.0..=2.0).contains(&self.temperature) || self.temperature.is_nan() {
            return Err(InferenceError::Budget {
                reason: "temperature must be within [0, 2]".to_string(),
            });
        }
        let needed = self
            .prompt_tokens_estimate()
            .saturating_add(self.max_output_tokens);
        if info.max_context_tokens > 0 && needed > info.max_context_tokens {
            return Err(InferenceError::Budget {
                reason: format!(
                    "estimated prompt + output tokens ({needed}) exceed the context window ({})",
                    info.max_context_tokens
                ),
            });
        }
        Ok(())
    }
}

/// Measured usage reported by the runtime. These are measurements for *this*
/// invocation; they are never aggregated into performance claims by this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    /// Prompt tokens as counted by the runtime's tokenizer.
    pub prompt_tokens: u32,
    /// Generated tokens.
    pub output_tokens: u32,
    /// Time to first token.
    pub ttft_ms: DurationMs,
    /// Wall time of the whole generation as measured by the runtime.
    pub total_ms: DurationMs,
}

impl Default for Usage {
    /// All-zero usage: "nothing measured" (failed calls, test doubles).
    fn default() -> Self {
        Self {
            prompt_tokens: 0,
            output_tokens: 0,
            ttft_ms: DurationMs::ZERO,
            total_ms: DurationMs::ZERO,
        }
    }
}

/// Why generation stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FinishReason {
    /// The model emitted an end-of-sequence token.
    Stop,
    /// `max_output_tokens` was reached; the output may be truncated.
    Length,
    /// The caller cancelled; any partial output must be discarded.
    Cancelled,
    /// The timeout elapsed; any partial output must be discarded.
    Timeout,
    /// The runtime failed; see the redacted error.
    Error,
}

/// What a runtime returns. `text` and `structured` are advisory data; the
/// caller validates `structured` against the request schema before use (A11).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InferenceResponse {
    /// Raw model text. Advisory data, never an instruction.
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Structured output, present when the runtime produced/validated one.
    pub structured: Option<serde_json::Value>,
    /// Measured usage for this call.
    pub usage: Usage,
    /// Why generation stopped.
    pub finish: FinishReason,
    /// The model that produced the response.
    pub model: ModelRef,
    /// Prompt template version the request was built with.
    pub template_version: String,
}

/// Errors a runtime or the gateway can produce. The `Runtime` payload is an
/// operator-local diagnostic; it is redacted when mapped to [`G9Error`].
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum InferenceError {
    /// No model is loaded or the runtime process is not reachable (A09).
    #[error("model unavailable")]
    ModelUnavailable,
    /// The request timeout elapsed.
    #[error("inference timed out")]
    Timeout,
    /// The cancel token was set before or during generation.
    #[error("inference cancelled")]
    Cancelled,
    /// The request cannot be honoured within its budget.
    #[error("inference budget violated: {reason}")]
    /// Secret-free explanation of which limit was violated.
    Budget {
        /// Which limit was violated (static wording, never prompt content).
        reason: String,
    },
    /// Runtime failure; the string is a local diagnostic and never leaves the process.
    #[error("runtime failure (redacted)")]
    Runtime(String),
    /// The structured output did not satisfy the request schema.
    #[error("structured output invalid: {} issue(s)", .0.len())]
    InvalidOutput(Vec<ValidationIssue>),
}

impl InferenceError {
    /// The closed category this variant maps to.
    pub fn category(&self) -> ErrorCategory {
        match self {
            InferenceError::ModelUnavailable => ErrorCategory::ResourceUnavailable,
            InferenceError::Timeout => ErrorCategory::Timeout,
            InferenceError::Cancelled => ErrorCategory::Cancelled,
            InferenceError::Budget { .. } | InferenceError::InvalidOutput(_) => {
                ErrorCategory::PreconditionFailed
            }
            InferenceError::Runtime(_) => ErrorCategory::Internal,
        }
    }

    /// Stable machine-readable code (`ai.<...>`).
    pub fn code(&self) -> &'static str {
        match self {
            InferenceError::ModelUnavailable => "ai.model.unavailable",
            InferenceError::Timeout => "ai.inference.timeout",
            InferenceError::Cancelled => "ai.inference.cancelled",
            InferenceError::Budget { .. } => "ai.inference.budget",
            InferenceError::Runtime(_) => "ai.runtime.failure",
            InferenceError::InvalidOutput(_) => "ai.output.invalid",
        }
    }

    /// The [`FinishReason`] an invocation record should carry for this error.
    pub fn finish_reason(&self) -> FinishReason {
        match self {
            InferenceError::Timeout => FinishReason::Timeout,
            InferenceError::Cancelled => FinishReason::Cancelled,
            InferenceError::ModelUnavailable
            | InferenceError::Budget { .. }
            | InferenceError::Runtime(_)
            | InferenceError::InvalidOutput(_) => FinishReason::Error,
        }
    }
}

impl From<InferenceError> for G9Error {
    fn from(e: InferenceError) -> Self {
        let category = e.category();
        let code = e.code();
        match e {
            InferenceError::ModelUnavailable => G9Error::new(
                category,
                code,
                "no model is available; the application continues without AI assistance",
            ),
            InferenceError::Timeout => G9Error::new(category, code, "inference timed out"),
            InferenceError::Cancelled => G9Error::new(category, code, "inference was cancelled"),
            InferenceError::Budget { reason } => G9Error::new(category, code, reason),
            // Redacted: the runtime diagnostic stays in the local operation log.
            InferenceError::Runtime(_) => G9Error::internal_redacted(code),
            InferenceError::InvalidOutput(issues) => {
                let err = G9Error::new(
                    category,
                    code,
                    format!("model output rejected: {} schema issue(s)", issues.len()),
                );
                match serde_json::to_value(&issues) {
                    Ok(details) => err.with_details(details),
                    Err(_) => err,
                }
            }
        }
    }
}

/// The runtime boundary. Implementations must be `Send + Sync` so that a single
/// runtime can be shared behind an `Arc` by the orchestrator.
///
/// Contract:
/// - `generate` must return promptly with [`InferenceError::Cancelled`] once
///   `cancel.is_cancelled()` is observed, and must release any resources it
///   reserved (handoff §13: "Timeouts and cancellations must cleanly release
///   resources").
/// - A response that arrives after cancellation must be discarded by the caller
///   through [`crate::cancel::Guard::accept`]; implementations should not rely
///   on the caller alone and should check the token before returning.
/// - Implementations never persist raw prompts or responses.
pub trait InferenceRuntime: Send + Sync {
    /// Static description: adapter name/version, loaded model, context window.
    fn describe(&self) -> RuntimeInfo;

    /// Run one generation. Blocking; cancellation is cooperative through `cancel`.
    fn generate(
        &self,
        req: InferenceRequest,
        cancel: &CancelToken,
    ) -> Result<InferenceResponse, InferenceError>;
}

/// Gateway entry point: the one path through which callers obtain a usable
/// response (handoff §13 "reserve resource budget → invoke runtime → schema
/// checks"). It applies, in order:
///
/// 1. cancelled-before-start → [`InferenceError::Cancelled`];
/// 2. [`InferenceRequest::check_budget`] against [`InferenceRuntime::describe`];
/// 3. [`InferenceRuntime::generate`];
/// 4. the late-response rule: a response that arrives after cancellation is
///    discarded through [`Guard::accept`] and reported as `Cancelled`;
/// 5. `Cancelled`/`Timeout`/`Error` finish reasons become the matching error
///    (partial output is never returned);
/// 6. when the request carries `response_schema`, the structured output is
///    validated (A11). The runtime's `structured` value is used when present;
///    otherwise `text` must parse as one JSON document. Any violation is
///    [`InferenceError::InvalidOutput`] with pointer paths — nothing is
///    repaired, defaulted or coerced.
///
/// The returned response's `structured` field is `Some` iff a schema was
/// requested and satisfied.
pub fn run_inference(
    runtime: &dyn InferenceRuntime,
    req: InferenceRequest,
    cancel: &CancelToken,
) -> Result<InferenceResponse, InferenceError> {
    if cancel.is_cancelled() {
        return Err(InferenceError::Cancelled);
    }
    let info = runtime.describe();
    req.check_budget(&info)?;
    let schema = req.response_schema.clone();
    let response = runtime.generate(req, cancel)?;
    let mut response = Guard::accept(response, cancel).ok_or(InferenceError::Cancelled)?;
    match response.finish {
        FinishReason::Cancelled => return Err(InferenceError::Cancelled),
        FinishReason::Timeout => return Err(InferenceError::Timeout),
        FinishReason::Error => {
            return Err(InferenceError::Runtime(
                "runtime reported an error finish".to_string(),
            ))
        }
        FinishReason::Stop | FinishReason::Length => {}
    }
    if let Some(schema) = schema {
        let instance = match response.structured.take() {
            Some(v) => v,
            None => serde_json::from_str::<serde_json::Value>(&response.text).map_err(|_| {
                InferenceError::InvalidOutput(vec![ValidationIssue::new(
                    "",
                    "output is not a single JSON document",
                )])
            })?,
        };
        validate_structured(&instance, &schema).map_err(InferenceError::InvalidOutput)?;
        response.structured = Some(instance);
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn info(max_context_tokens: u32) -> RuntimeInfo {
        RuntimeInfo {
            name: "test".into(),
            version: "0".into(),
            model: None,
            max_context_tokens,
            supports_structured_output: false,
        }
    }

    fn request(segments: Vec<ContextSegment>, max_output_tokens: u32) -> InferenceRequest {
        InferenceRequest {
            invocation_id: InvocationId::parse("inv-1").unwrap(),
            segments,
            max_output_tokens,
            timeout: DurationMs::from_seconds_u32(10),
            response_schema: None,
            temperature: 0.2,
            template_version: "t1".into(),
        }
    }

    #[test]
    fn token_estimate_is_ceil_chars_over_four() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abc"), 1);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
        // Multi-byte characters count as characters, not bytes.
        assert_eq!(estimate_tokens("ääää"), 1);
    }

    #[test]
    fn budget_check_rejects_zero_output_zero_timeout_and_overflow() {
        let seg = ContextSegment::new(
            SegmentRole::Task,
            "x".repeat(400),
            TrustClass::SystemDeterministic,
            Classification::Internal,
            "tpl",
        );
        assert_eq!(seg.approx_tokens, 100);

        let r = request(vec![seg.clone()], 0);
        assert!(matches!(
            r.check_budget(&info(1000)),
            Err(InferenceError::Budget { .. })
        ));

        let mut r = request(vec![seg.clone()], 10);
        r.timeout = DurationMs::ZERO;
        assert!(matches!(
            r.check_budget(&info(1000)),
            Err(InferenceError::Budget { .. })
        ));

        let mut r = request(vec![seg.clone()], 10);
        r.temperature = 3.0;
        assert!(matches!(
            r.check_budget(&info(1000)),
            Err(InferenceError::Budget { .. })
        ));

        // 100 prompt + 50 output > 120 context.
        let r = request(vec![seg.clone()], 50);
        assert!(matches!(
            r.check_budget(&info(120)),
            Err(InferenceError::Budget { .. })
        ));
        assert_eq!(r.check_budget(&info(150)), Ok(()));
        // Unknown context window (0) does not block.
        assert_eq!(r.check_budget(&info(0)), Ok(()));
    }

    #[test]
    fn errors_map_to_closed_categories_and_redact_runtime_detail() {
        let cases = [
            (
                InferenceError::ModelUnavailable,
                ErrorCategory::ResourceUnavailable,
            ),
            (InferenceError::Timeout, ErrorCategory::Timeout),
            (InferenceError::Cancelled, ErrorCategory::Cancelled),
            (
                InferenceError::Budget { reason: "r".into() },
                ErrorCategory::PreconditionFailed,
            ),
            (
                InferenceError::Runtime("CUDA error at /secret/path".into()),
                ErrorCategory::Internal,
            ),
            (
                InferenceError::InvalidOutput(vec![]),
                ErrorCategory::PreconditionFailed,
            ),
        ];
        for (err, cat) in cases {
            let g: G9Error = err.clone().into();
            assert_eq!(g.category, cat, "{err:?}");
            assert!(g.code.starts_with("ai."));
            assert!(g
                .details
                .as_ref()
                .is_none_or(|d| d.get("backtrace").is_none()));
        }
        let g: G9Error = InferenceError::Runtime("CUDA error at /secret/path".into()).into();
        assert!(!g.message.contains("CUDA"));
        assert!(!g.message.contains("/secret"));
        assert_eq!(g.message, "internal error; see local operation log");
    }
}
