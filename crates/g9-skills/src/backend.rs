//! The inference boundary as seen from the orchestrator (ARCHITECTURE.md §10
//! "AI gateway"; handoff §13 "Inference protocol"), plus cooperative
//! cancellation and two deterministic implementations: the model-down path
//! ([`UnavailableBackend`], gate A09) and a scripted test double
//! ([`ScriptedBackend`]).
//!
//! `g9-ai-adapter` did not expose a public API when this crate was written
//! (its `lib.rs` is a scaffold); `g9-app` adapts the real `InferenceRuntime`
//! to [`InferenceBackend`]. The adapter is the place that assembles the
//! provenance-tagged context from the [`ContextSegment`]s and applies the
//! privacy filter; this crate only decides *what* is sent, never *how* the
//! runtime is driven. No implementation here touches SQLite, the filesystem
//! or the network.

use g9_domain::{
    Classification, DurationMs, ErrorCategory, G9Error, InvocationId, ModelId, SkillId, TrustClass,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Cooperative cancellation flag shared between the orchestrator, the caller
/// and the backend. Once cancelled it stays cancelled.
///
/// A response that arrives after cancellation **must be discarded** by the
/// caller: the orchestrator re-checks the token after `generate` returns and
/// drops the response (`cancel_discards_late_response`).
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A fresh, not-cancelled token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Request cancellation (idempotent).
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Role of a context segment; the adapter orders and filters by role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SegmentRole {
    /// Runtime policy text (deterministic, SystemDeterministic trust).
    SystemPolicy,
    /// `SKILL.md` body (SkillPackage trust — instructional data).
    SkillInstructions,
    /// The task: intent and input payload (operator trust).
    Task,
    /// Evidence references/content supplied by the runtime.
    Evidence,
    /// Free text typed by the learner.
    LearnerInput,
}

/// One provenance-tagged piece of model context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextSegment {
    /// Role.
    pub role: SegmentRole,
    /// Text.
    pub text: String,
    /// Trust class of the source.
    pub trust: TrustClass,
    /// Classification of the content.
    pub classification: Classification,
    /// Stable reference to the source.
    pub source_ref: String,
}

/// What the orchestrator asks the backend to do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendRequest {
    /// Invocation being served.
    pub invocation_id: InvocationId,
    /// Skill name.
    pub skill_id: SkillId,
    /// Skill version.
    pub skill_version: String,
    /// Prompt template version (recorded per invocation; never the prompt text).
    pub template_version: String,
    /// Context in role order.
    pub segments: Vec<ContextSegment>,
    /// Composed output envelope schema the response must satisfy.
    pub response_schema: Value,
    /// Output token cap (from the invocation budget).
    pub max_output_tokens: u32,
    /// Wall-clock budget (from the invocation budget).
    pub timeout: DurationMs,
}

/// Token and latency accounting reported by the runtime (monotonic-clock based
/// on the runtime side; never wall-clock subtraction here).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct BackendUsage {
    /// Prompt tokens.
    pub prompt_tokens: u32,
    /// Generated tokens.
    pub output_tokens: u32,
    /// Total generation time in milliseconds.
    pub total_ms: u64,
}

/// How generation ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BackendFinish {
    /// Natural stop.
    Stop,
    /// Output token cap reached (output may be truncated).
    Length,
    /// Timed out (output discarded).
    Timeout,
    /// Cancelled (output discarded).
    Cancelled,
    /// Runtime error (output discarded).
    Error,
}

/// Backend reply. `structured` is preferred when present; otherwise `text`
/// must parse as one JSON object.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackendResponse {
    /// Raw text (never persisted by this crate).
    pub text: String,
    /// Structured output when the runtime constrained decoding.
    pub structured: Option<Value>,
    /// Accounting.
    pub usage: BackendUsage,
    /// Finish reason.
    pub finish: BackendFinish,
    /// Model that produced the response, when known.
    pub model_id: Option<ModelId>,
}

/// Backend failure. `Runtime` carries a local diagnostic that is redacted when
/// mapped to [`G9Error`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BackendError {
    /// No runtime or model is configured/loaded (model-down path).
    #[error("no inference runtime or model is available")]
    Unavailable,
    /// The budgeted time elapsed.
    #[error("inference timed out")]
    Timeout,
    /// Cancellation was observed.
    #[error("inference cancelled")]
    Cancelled,
    /// The request exceeded a runtime limit before generation.
    #[error("inference budget refused: {0}")]
    Budget(String),
    /// Runtime failure (redacted on the way out).
    #[error("inference runtime failure (details redacted)")]
    Runtime(String),
}

impl BackendError {
    /// Closed error category.
    pub fn category(&self) -> ErrorCategory {
        match self {
            BackendError::Unavailable => ErrorCategory::ResourceUnavailable,
            BackendError::Timeout => ErrorCategory::Timeout,
            BackendError::Cancelled => ErrorCategory::Cancelled,
            BackendError::Budget(_) => ErrorCategory::PreconditionFailed,
            BackendError::Runtime(_) => ErrorCategory::Internal,
        }
    }

    /// Stable code.
    pub fn code(&self) -> &'static str {
        match self {
            BackendError::Unavailable => "skills.backend.unavailable",
            BackendError::Timeout => "skills.backend.timeout",
            BackendError::Cancelled => "skills.backend.cancelled",
            BackendError::Budget(_) => "skills.backend.budget",
            BackendError::Runtime(_) => "skills.backend.runtime",
        }
    }
}

impl From<BackendError> for G9Error {
    fn from(e: BackendError) -> Self {
        match &e {
            BackendError::Runtime(_) => G9Error::internal_redacted(e.code()),
            BackendError::Unavailable => G9Error::resource_unavailable(
                e.code(),
                "the model runtime is unavailable; the application remains usable without it",
            ),
            BackendError::Timeout => G9Error::timeout(e.code(), e.to_string()),
            BackendError::Cancelled => G9Error::cancelled(e.code(), e.to_string()),
            BackendError::Budget(_) => G9Error::precondition_failed(e.code(), e.to_string()),
        }
    }
}

/// The runtime boundary used by [`crate::Orchestrator`].
///
/// Contract: return promptly with [`BackendError::Cancelled`] once
/// `cancel.is_cancelled()` is observed; honour `req.timeout` with
/// [`BackendError::Timeout`]; never persist prompts or responses.
pub trait InferenceBackend: Send + Sync {
    /// Adapter name for traces (e.g. `unavailable`, `scripted`, `llama-cpp-compatible`).
    fn name(&self) -> &str;

    /// Run one generation. Blocking; cancellation is cooperative.
    fn generate(
        &self,
        req: BackendRequest,
        cancel: &CancelToken,
    ) -> Result<BackendResponse, BackendError>;
}

/// Model-down backend: every call fails with [`BackendError::Unavailable`]
/// (ARCHITECTURE.md §10 `NullRuntime` behaviour, gate A09).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct UnavailableBackend;

impl InferenceBackend for UnavailableBackend {
    fn name(&self) -> &str {
        "unavailable"
    }

    fn generate(
        &self,
        _req: BackendRequest,
        _cancel: &CancelToken,
    ) -> Result<BackendResponse, BackendError> {
        Err(BackendError::Unavailable)
    }
}

/// One queued reply of a [`ScriptedBackend`].
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptedReply {
    /// Return this response.
    Respond(BackendResponse),
    /// Return this error.
    Fail(BackendError),
    /// Cancel the token, then return the response anyway — simulates a
    /// response that arrives after cancellation; the caller must discard it.
    CancelThenRespond(BackendResponse),
}

/// Metadata about one request (no text, no schema, no payload).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestMetadata {
    /// Invocation id.
    pub invocation_id: InvocationId,
    /// Skill id.
    pub skill_id: SkillId,
    /// Template version.
    pub template_version: String,
    /// Number of segments.
    pub segment_count: usize,
    /// Roles in order.
    pub roles: Vec<SegmentRole>,
    /// Total characters across segments.
    pub approx_chars: usize,
    /// Output token cap.
    pub max_output_tokens: u32,
    /// Timeout in ms.
    pub timeout_ms: i64,
}

/// Deterministic test double: returns queued replies in order and records
/// request metadata (never content) for assertions.
#[derive(Debug, Default)]
pub struct ScriptedBackend {
    replies: Mutex<VecDeque<ScriptedReply>>,
    seen: Mutex<Vec<RequestMetadata>>,
}

impl ScriptedBackend {
    /// Queue replies in call order.
    pub fn new(replies: Vec<ScriptedReply>) -> Self {
        Self {
            replies: Mutex::new(replies.into()),
            seen: Mutex::new(Vec::new()),
        }
    }

    /// Metadata of every request received so far.
    pub fn requests(&self) -> Vec<RequestMetadata> {
        lock(&self.seen).clone()
    }

    /// Replies not yet consumed.
    pub fn remaining(&self) -> usize {
        lock(&self.replies).len()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl InferenceBackend for ScriptedBackend {
    fn name(&self) -> &str {
        "scripted"
    }

    fn generate(
        &self,
        req: BackendRequest,
        cancel: &CancelToken,
    ) -> Result<BackendResponse, BackendError> {
        lock(&self.seen).push(RequestMetadata {
            invocation_id: req.invocation_id.clone(),
            skill_id: req.skill_id.clone(),
            template_version: req.template_version.clone(),
            segment_count: req.segments.len(),
            roles: req.segments.iter().map(|s| s.role).collect(),
            approx_chars: req.segments.iter().map(|s| s.text.chars().count()).sum(),
            max_output_tokens: req.max_output_tokens,
            timeout_ms: req.timeout.as_millis(),
        });
        if cancel.is_cancelled() {
            return Err(BackendError::Cancelled);
        }
        match lock(&self.replies).pop_front() {
            Some(ScriptedReply::Respond(r)) => Ok(r),
            Some(ScriptedReply::Fail(e)) => Err(e),
            Some(ScriptedReply::CancelThenRespond(r)) => {
                cancel.cancel();
                Ok(r)
            }
            None => Err(BackendError::Runtime("scripted backend exhausted".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn cancel_token_is_sticky_and_shared() {
        let t = CancelToken::new();
        let u = t.clone();
        assert!(!t.is_cancelled());
        u.cancel();
        assert!(t.is_cancelled());
        t.cancel();
        assert!(u.is_cancelled());
    }

    #[test]
    fn errors_map_to_closed_categories_and_redact_runtime_detail() {
        let cases = [
            (
                BackendError::Unavailable,
                ErrorCategory::ResourceUnavailable,
            ),
            (BackendError::Timeout, ErrorCategory::Timeout),
            (BackendError::Cancelled, ErrorCategory::Cancelled),
            (
                BackendError::Budget("x".into()),
                ErrorCategory::PreconditionFailed,
            ),
            (
                BackendError::Runtime("CUDA at /home/secret".into()),
                ErrorCategory::Internal,
            ),
        ];
        for (e, cat) in cases {
            assert_eq!(e.category(), cat);
            let g: G9Error = e.clone().into();
            assert_eq!(g.category, cat, "{e:?}");
            assert!(g.code.starts_with("skills.backend."));
        }
        let g: G9Error = BackendError::Runtime("CUDA at /home/secret".into()).into();
        assert!(!g.message.contains("CUDA"));
        assert!(!g.message.contains("/home"));
    }

    #[test]
    fn scripted_backend_records_only_metadata() {
        let b = ScriptedBackend::new(vec![ScriptedReply::Fail(BackendError::Timeout)]);
        let req = BackendRequest {
            invocation_id: InvocationId::parse("inv-1").unwrap(),
            skill_id: SkillId::parse("g9-x").unwrap(),
            skill_version: "0.1.0".into(),
            template_version: "t".into(),
            segments: vec![ContextSegment {
                role: SegmentRole::Task,
                text: "SECRET-PAYLOAD".into(),
                trust: TrustClass::Operator,
                classification: Classification::Private,
                source_ref: "r".into(),
            }],
            response_schema: Value::Null,
            max_output_tokens: 10,
            timeout: DurationMs::from_seconds_u32(1),
        };
        assert_eq!(
            b.generate(req, &CancelToken::new()),
            Err(BackendError::Timeout)
        );
        let seen = b.requests();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].approx_chars, 14);
        assert_eq!(seen[0].roles, vec![SegmentRole::Task]);
        let json = serde_json::to_string(&seen).unwrap();
        assert!(!json.contains("SECRET-PAYLOAD"));
        assert_eq!(b.remaining(), 0);
    }
}
