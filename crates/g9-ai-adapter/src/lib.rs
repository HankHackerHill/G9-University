#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
#![warn(missing_docs)]
//! # g9-ai-adapter — M10 AI Gateway (ARCHITECTURE.md §10, handoff §13)
//!
//! The replaceable inference boundary of G9 MENTOR. This crate:
//!
//! - defines the [`InferenceRuntime`] trait and its typed request/response
//!   vocabulary ([`runtime`]);
//! - assembles minimal, provenance-tagged, privacy-filtered model context
//!   ([`context`]) — `Secret` content never enters context, EXAM mode is
//!   minimal, untrusted content can never take an instruction role, and every
//!   drop is reported;
//! - validates structured output against JSON Schema draft 2020-12
//!   ([`validate`]) — invalid output is rejected (A11), never repaired;
//! - provides cooperative cancellation with a late-response guard ([`cancel`]);
//! - ships the model-down path [`NullRuntime`] (A09, R-002) and a deterministic
//!   test double [`ScriptedRuntime`];
//! - carries the model registry vocabulary and checksum verification
//!   ([`registry`]) — loading needs an approval id and a checksum match, policy
//!   decides (`Capability::ModelLoad` is operator-only);
//! - produces the per-invocation metadata record ([`record`]) that contains
//!   **no** prompt or response text;
//! - describes the mandatory benchmark protocol ([`crate::bench`]) without asserting
//!   any performance number (corrections C06/C07).
//!
//! Dependency law (ARCHITECTURE.md §2 rule 3): this crate never touches SQLite,
//! the filesystem or native OS APIs. Model output is data, never instructions
//! and never authoritative; authority is decided in `g9-policy` and applied by
//! `g9-app`.
//!
//! No concrete model runtime (llama.cpp-compatible, Ollama-compatible) is
//! implemented here; they are Stage 06 adapters behind [`InferenceRuntime`]
//! and are listed as PROPOSED in `docs/evaluations/model-benchmark-protocol.md`.

pub mod bench;
pub mod cancel;
pub mod context;
pub mod null;
pub mod record;
pub mod registry;
pub mod runtime;
pub mod scripted;
pub mod validate;

pub use bench::{BenchmarkProtocol, Measurement};
pub use cancel::{CancelToken, Guard};
pub use context::{AssembleReport, ContextAssembler, ContextBudget, DropReason};
pub use null::NullRuntime;
pub use record::InvocationRecord;
pub use registry::{
    verify_checksum, LicenseClass, ModelLoadRequest, ModelRegistryEntry, RegistryError,
};
pub use runtime::{
    estimate_tokens, run_inference, ContextSegment, FinishReason, InferenceError, InferenceRequest,
    InferenceResponse, InferenceRuntime, ModelRef, RuntimeInfo, SegmentRole, Usage,
};
pub use scripted::{RecordedRequest, ScriptedRuntime};
pub use validate::{
    compose_output_schema, output_envelope_schema, validate_against_envelope, validate_structured,
    ValidationIssue,
};
