//! Skill vocabulary (handoff §11, Appendix B). A skill is a versioned task
//! package, not an agent and not an authority grant. The invocation and output
//! envelopes are typed so that the orchestrator can validate every field;
//! `SkillOutputEnvelope` is never treated as execution authorization.

use crate::actor::Actor;
use crate::envelope::{ResourceRef, Scope};
use crate::ids::{EvidenceId, IdempotencyKey, InvocationId, SkillId, TraceId};
use crate::mode::InteractionMode;
use crate::policy::{Action, Capability};
use crate::privacy::Classification;
use crate::revision::Revision;
use serde::{Deserialize, Serialize};

/// Skill families by registry code prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SkillFamily {
    /// E01–E14: engineering skills (build-time, operator-run).
    Engineering,
    /// T01–T08: teaching.
    Teaching,
    /// L01–L06: learning science / measurement.
    Learning,
    /// P01–P06: performance / execution.
    Performance,
    /// K01–K03: knowledge / research.
    Knowledge,
}

impl SkillFamily {
    pub const fn is_runtime(self) -> bool {
        !matches!(self, SkillFamily::Engineering)
    }
}

/// Registry code such as `E01`, `T08`, `K03`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SkillCode(String);

impl SkillCode {
    pub fn parse(s: &str) -> Result<Self, String> {
        let bytes = s.as_bytes();
        if bytes.len() != 3 {
            return Err("skill code must be 3 chars like E01".into());
        }
        let fam = bytes[0] as char;
        if !matches!(fam, 'E' | 'T' | 'L' | 'P' | 'K') {
            return Err("skill code family must be one of E,T,L,P,K".into());
        }
        if !bytes[1].is_ascii_digit() || !bytes[2].is_ascii_digit() {
            return Err("skill code must end with two digits".into());
        }
        let n: u8 = s[1..].parse().map_err(|_| "bad number".to_string())?;
        let max = match fam {
            'E' => 14,
            'T' => 8,
            'L' => 6,
            'P' => 6,
            'K' => 3,
            _ => 0,
        };
        if n == 0 || n > max {
            return Err(format!("skill code {s} out of range for family {fam} (1..={max})"));
        }
        Ok(Self(s.to_string()))
    }

    pub fn family(&self) -> SkillFamily {
        match self.0.as_bytes()[0] as char {
            'E' => SkillFamily::Engineering,
            'T' => SkillFamily::Teaching,
            'L' => SkillFamily::Learning,
            'P' => SkillFamily::Performance,
            _ => SkillFamily::Knowledge,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Total number of logical skills in the registry (14 + 8 + 6 + 6 + 3).
    pub const TOTAL: usize = 37;
}

impl TryFrom<String> for SkillCode {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<SkillCode> for String {
    fn from(c: SkillCode) -> String {
        c.0
    }
}

/// Validate an Agent Skills `name`: 1..=64 chars, lowercase letters, digits and
/// hyphens, no leading/trailing/consecutive hyphens.
pub fn validate_skill_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 64 {
        return Err("skill name must be 1..=64 characters".into());
    }
    if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err("skill name must use only lowercase letters, digits and hyphens".into());
    }
    if name.starts_with('-') || name.ends_with('-') || name.contains("--") {
        return Err("skill name must not start/end with a hyphen or contain consecutive hyphens".into());
    }
    Ok(())
}

/// Hard caps for one invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceBudget {
    pub max_tokens: u32,
    pub max_seconds: u32,
    pub max_tool_calls: u32,
}

impl ResourceBudget {
    pub const fn is_within(&self, used: &ResourceUsage) -> bool {
        used.tokens <= self.max_tokens && used.seconds <= self.max_seconds && used.tool_calls <= self.max_tool_calls
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ResourceUsage {
    pub tokens: u32,
    pub seconds: u32,
    pub tool_calls: u32,
}

/// Policy context injected into an invocation (handoff Appendix B).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyContext {
    pub mode: InteractionMode,
    /// Capabilities the orchestrator has verified for this invocation. Skills can
    /// never widen this set.
    pub granted_capabilities: Vec<Capability>,
    /// Depth of nested invocation; default policy forbids recursion (max 0).
    pub recursion_depth: u8,
}

/// G9 runtime invocation envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillInvocationEnvelope {
    pub skill_id: SkillId,
    pub skill_version: String,
    pub invocation_id: InvocationId,
    pub actor: Actor,
    pub intent: String,
    pub approved_scope: Scope,
    pub data_classification: Classification,
    pub input_schema_version: String,
    pub input_payload: serde_json::Value,
    pub evidence_refs: Vec<EvidenceId>,
    pub policy_context: PolicyContext,
    pub resource_budget: ResourceBudget,
    pub allowed_tool_ids: Vec<String>,
    pub idempotency_key: IdempotencyKey,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_revision: Option<Revision>,
    pub trace_id: TraceId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SkillOutputStatus {
    Completed,
    /// The skill declined to answer (insufficient evidence, out of scope).
    Abstained,
    Error,
}

/// A factual observation with evidence; distinguished from inferences.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub statement: String,
    pub evidence_refs: Vec<EvidenceId>,
}

/// A reasoned inference with explicit confidence limits.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inference {
    pub statement: String,
    pub supporting_evidence: Vec<EvidenceId>,
    pub contradicting_evidence: Vec<EvidenceId>,
    /// Lower/upper confidence in [0,1].
    pub confidence_low: f64,
    pub confidence_high: f64,
}

/// A proposed action. It is a *request*; policy decides and the operator approves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecommendedAction {
    pub action: Action,
    pub resource: ResourceRef,
    pub rationale: String,
    pub evidence_refs: Vec<EvidenceId>,
    /// Parameters of the proposed operation (validated against the operation schema).
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationResult {
    pub check: String,
    pub passed: bool,
    pub detail: String,
}

/// G9 runtime output envelope. Never trusted as execution authorization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillOutputEnvelope {
    pub status: SkillOutputStatus,
    pub result_schema_version: String,
    pub observations: Vec<Observation>,
    pub inferences: Vec<Inference>,
    pub evidence_refs: Vec<EvidenceId>,
    pub confidence_limits: Vec<String>,
    pub recommended_actions: Vec<RecommendedAction>,
    pub approval_requirements: Vec<Capability>,
    pub resource_usage: ResourceUsage,
    pub validation_results: Vec<ValidationResult>,
    pub errors: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_codes_cover_exactly_37() {
        let mut n = 0;
        for (fam, max) in [('E', 14), ('T', 8), ('L', 6), ('P', 6), ('K', 3)] {
            for i in 1..=max {
                assert!(SkillCode::parse(&format!("{fam}{i:02}")).is_ok());
                n += 1;
            }
            assert!(SkillCode::parse(&format!("{fam}{:02}", max + 1)).is_err());
        }
        assert_eq!(n, SkillCode::TOTAL);
        assert!(SkillCode::parse("X01").is_err());
        assert!(SkillCode::parse("E00").is_err());
        assert_eq!(SkillCode::parse("T03").unwrap().family(), SkillFamily::Teaching);
    }

    #[test]
    fn skill_name_rules() {
        assert!(validate_skill_name("g9-session-director").is_ok());
        assert!(validate_skill_name("G9-Session").is_err());
        assert!(validate_skill_name("-g9").is_err());
        assert!(validate_skill_name("g9--x").is_err());
        assert!(validate_skill_name(&"a".repeat(65)).is_err());
    }

    #[test]
    fn budget_enforced() {
        let b = ResourceBudget { max_tokens: 10, max_seconds: 5, max_tool_calls: 1 };
        assert!(b.is_within(&ResourceUsage { tokens: 10, seconds: 5, tool_calls: 1 }));
        assert!(!b.is_within(&ResourceUsage { tokens: 11, seconds: 0, tool_calls: 0 }));
    }
}
