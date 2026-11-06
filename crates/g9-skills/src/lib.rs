#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! # g9-skills — M11 Skills Orchestrator
//!
//! Skill registry loader (`SKILL.md` + schemas + evals), typed invocation and
//! output envelopes, the invocation state machine and the bounded orchestrator
//! pipeline (ARCHITECTURE.md §10 "Skill registry" / "Orchestrator"; handoff §11
//! "Runtime pipeline" and "Skill promotion gates"; corrections C11 and C16).
//!
//! | Module | Contract |
//! |---|---|
//! | [`frontmatter`] | strict `SKILL.md` frontmatter subset — no YAML engine |
//! | [`package`] | [`SkillPackage`] + [`load_package`]: every rule of `docs/skills/skill-package-contract.md` |
//! | [`registry`] | [`Registry`]: all 37 codes present once, package hashes (supply-chain pin), candidate filter |
//! | [`envelope`] | invocation envelope builder; JSON-Schema validation of envelopes and skill payloads (A11) |
//! | [`policy`] | [`PolicyPreflight`] boundary, implemented directly for `g9_policy::Engine` |
//! | [`backend`] | [`InferenceBackend`] boundary, [`CancelToken`], model-down and scripted doubles (A09) |
//! | [`orchestrator`] | [`Orchestrator::invoke`]: Requested → Preflight → Authorized → Running → Validating → Completed |
//! | [`evals`] | eval fixture types, static shape checks; model runs are PROPOSED (Stage 06/07) |
//!
//! ## Authority model (C11, C16)
//!
//! A skill is a versioned task package, not an agent and not an authority
//! grant. Nothing in a package grants a capability: the orchestrator asks
//! `g9-policy` for every invocation, the output envelope is advisory, and every
//! [`g9_domain::skill::RecommendedAction`] re-enters the `g9-app` command
//! pipeline as a *new* command under the operator's authority. Model output,
//! skill text and imported content are data, never instructions.
//!
//! ## Boundaries that other crates adapt
//!
//! * [`policy::PolicyPreflight`] is implemented for `g9_policy::Engine`; the
//!   trait exists so tests can inject fixed decisions.
//! * [`backend::InferenceBackend`] is this crate's view of the model runtime.
//!   At the time of writing `g9-ai-adapter` exports no public API yet (its
//!   `lib.rs` is a scaffold), so the boundary is defined here; `g9-app` adapts
//!   the real `InferenceRuntime` to it once that crate lands. The orchestrator
//!   never touches SQLite, the filesystem (beyond reading packages) or the OS.

pub mod backend;
pub mod envelope;
pub mod evals;
pub mod frontmatter;
pub mod orchestrator;
pub mod package;
pub mod policy;
pub mod registry;

pub use backend::{
    BackendError, BackendFinish, BackendRequest, BackendResponse, BackendUsage, CancelToken,
    ContextSegment, InferenceBackend, ScriptedBackend, ScriptedReply, SegmentRole,
    UnavailableBackend,
};
pub use envelope::{
    build_invocation, compose_output_schema, validate_invocation, validate_output, EnvelopeError,
    InvocationRequest, ValidationIssue,
};
pub use evals::{EvalCase, EvalExpect, EvalKind, EvalRunner, StaticEvalReport};
pub use frontmatter::{Frontmatter, FrontmatterError};
pub use orchestrator::{InvocationOutcome, InvocationTrace, Orchestrator};
pub use package::{load_package, EvalSet, PackageError, PackageStatus, SkillPackage};
pub use policy::{PolicyPreflight, StaticPolicy};
pub use registry::{HashMismatch, MissingSkills, PackageFailure, Registry, RegistryError};
