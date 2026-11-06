//! Evaluation fixtures (`evals/*.json`, contract "Evaluation fixtures") and
//! the static shape checks CI runs on them. Executing fixtures against a
//! model is a Stage 06/07 activity: [`EvalRunner::run_against_model`] is
//! deliberately unimplemented and reports NOT RUN through
//! `G9Error::precondition_failed("skills.eval.not_implemented")`.

use crate::backend::InferenceBackend;
use crate::envelope::validate_against;
use crate::orchestrator::ALLOWED_RECOMMENDED_ACTIONS;
use crate::package::{valid_case_id, SkillPackage};
use g9_domain::{
    Action, Capability, Classification, G9Error, InteractionMode, SkillId, SkillOutputStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Fixture kind, one file each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EvalKind {
    /// Must activate and succeed.
    Positive,
    /// Must not activate, or must abstain.
    Negative,
    /// Malicious/poisoned input.
    Adversarial,
    /// Representative baseline task.
    Acceptance,
}

impl EvalKind {
    /// All kinds in file order.
    pub const ALL: [EvalKind; 4] = [
        EvalKind::Positive,
        EvalKind::Negative,
        EvalKind::Adversarial,
        EvalKind::Acceptance,
    ];

    /// `evals/<file>`.
    pub const fn file_name(self) -> &'static str {
        match self {
            EvalKind::Positive => "positive.json",
            EvalKind::Negative => "negative.json",
            EvalKind::Adversarial => "adversarial.json",
            EvalKind::Acceptance => "acceptance.json",
        }
    }

    /// Case id infix (`T01-POS-001`).
    pub const fn id_prefix(self) -> &'static str {
        match self {
            EvalKind::Positive => "POS",
            EvalKind::Negative => "NEG",
            EvalKind::Adversarial => "ADV",
            EvalKind::Acceptance => "ACC",
        }
    }

    /// Minimum cases per skill (3/3/3/2).
    pub const fn minimum(self) -> usize {
        match self {
            EvalKind::Positive | EvalKind::Negative | EvalKind::Adversarial => 3,
            EvalKind::Acceptance => 2,
        }
    }
}

/// Tags every adversarial suite must cover.
pub const REQUIRED_ADVERSARIAL_TAGS: [&str; 3] = [
    "embedded_instruction",
    "forged_reference",
    "scope_escalation",
];

/// `actor_kind` of a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvalActorKind {
    /// The operator.
    Operator,
    /// A deterministic routine.
    System,
    /// A skill (always refused by the orchestrator — no recursion).
    Skill,
    /// The model (always refused).
    Model,
}

/// A context segment supplied by a fixture.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalContextSegment {
    /// Trust class name as written (PascalCase in the contract example).
    pub trust_class: String,
    /// Classification.
    pub classification: Classification,
    /// Content.
    pub content: String,
}

/// Expected outcome of a case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalExpect {
    /// Whether the skill must activate.
    pub activate: bool,
    /// Expected envelope status.
    pub status: SkillOutputStatus,
    /// Strings/fields that must appear.
    #[serde(default)]
    pub must_include: Vec<String>,
    /// Strings/behaviours that must not appear.
    #[serde(default)]
    pub must_not: Vec<String>,
    /// Expected approval requirements (never operator-only).
    #[serde(default)]
    pub approval_requirements: Vec<Capability>,
    /// Expected recommended action kinds (`Propose`, `PROPOSE`, ...).
    #[serde(default)]
    pub recommended_action_kinds: Vec<String>,
}

/// One fixture case (contract "Evaluation fixtures").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvalCase {
    /// `<CODE>-<POS|NEG|ADV|ACC>-NNN`.
    pub id: String,
    /// Kind (must match the file).
    pub kind: EvalKind,
    /// Title.
    pub title: String,
    /// Interaction mode.
    pub mode: InteractionMode,
    /// Actor kind.
    pub actor_kind: EvalActorKind,
    /// Intent text.
    pub intent: String,
    /// `input_payload` (validated against `input.schema.json` for positive/acceptance).
    pub input: Value,
    /// Optional learner context fixture path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_fixture: Option<String>,
    /// Optional inline context segments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_segments: Option<Vec<EvalContextSegment>>,
    /// Optional tags (adversarial cases need at least one).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// Expectation.
    pub expect: EvalExpect,
    /// Why the case matters.
    pub notes: String,
}

/// Static shape report for one package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticEvalReport {
    /// Skill name.
    pub skill: SkillId,
    /// Cases per kind.
    pub counts: BTreeMap<EvalKind, usize>,
    /// Every failed check, as `case-id: problem` (empty = passed).
    pub issues: Vec<String>,
}

impl StaticEvalReport {
    /// True when no issue was found.
    pub fn passed(&self) -> bool {
        self.issues.is_empty()
    }
}

/// PROPOSED (Stage 06/07): what a model run would produce. Never constructed
/// by this crate today; defined so callers can type the future API.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEvalReport {
    /// Skill name.
    pub skill: SkillId,
    /// Cases executed.
    pub cases_run: usize,
    /// Case ids whose expectation held.
    pub passed: Vec<String>,
    /// Case ids whose expectation failed.
    pub failed: Vec<String>,
}

/// Fixture checks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EvalRunner;

impl EvalRunner {
    /// Shape checks only (no model): counts, id format, kinds, unique ids,
    /// kind-specific expectations, inputs of positive/acceptance cases against
    /// the input schema, no operator-only approval expectations, recommended
    /// action kinds inside the proposal-only set, adversarial tag coverage.
    pub fn check_static(package: &SkillPackage) -> StaticEvalReport {
        let mut issues = Vec::new();
        let mut counts = BTreeMap::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut adversarial_tags: BTreeSet<String> = BTreeSet::new();

        for kind in EvalKind::ALL {
            let cases = package.evals.by_kind(kind);
            counts.insert(kind, cases.len());
            if cases.len() < kind.minimum() {
                issues.push(format!(
                    "{}: needs at least {} cases, found {}",
                    kind.file_name(),
                    kind.minimum(),
                    cases.len()
                ));
            }
            for case in cases {
                let id = case.id.as_str();
                if !valid_case_id(id, &package.code, kind.id_prefix()) {
                    issues.push(format!(
                        "{id}: id must match {}-{}-NNN",
                        package.code.as_str(),
                        kind.id_prefix()
                    ));
                }
                if case.kind != kind {
                    issues.push(format!(
                        "{id}: kind {:?} does not match file {}",
                        case.kind,
                        kind.file_name()
                    ));
                }
                if !seen.insert(id) {
                    issues.push(format!("{id}: duplicate id"));
                }
                for s in [&case.title, &case.intent, &case.notes] {
                    if s.trim().is_empty() {
                        issues.push(format!("{id}: title/intent/notes must be non-empty"));
                        break;
                    }
                }
                if !case.input.is_object() {
                    issues.push(format!("{id}: input must be an object"));
                }
                for cap in &case.expect.approval_requirements {
                    if cap.operator_only() {
                        issues.push(format!(
                            "{id}: expects operator-only approval requirement {cap:?}"
                        ));
                    }
                }
                for k in &case.expect.recommended_action_kinds {
                    match parse_action(k) {
                        None => issues.push(format!("{id}: unknown recommended action kind `{k}`")),
                        Some(a) if !ALLOWED_RECOMMENDED_ACTIONS.contains(&a) => {
                            issues.push(format!("{id}: skills may not recommend {a:?}"))
                        }
                        Some(_) => {}
                    }
                }
                let tags: Vec<&str> = case.tags.iter().flatten().map(String::as_str).collect();
                match kind {
                    EvalKind::Positive => {
                        if !(case.expect.activate
                            && case.expect.status == SkillOutputStatus::Completed)
                        {
                            issues.push(format!("{id}: positive cases must activate and COMPLETE"));
                        }
                        if !package.modes.contains(&case.mode) {
                            issues.push(format!(
                                "{id}: mode {:?} is outside the skill's g9-modes",
                                case.mode
                            ));
                        }
                    }
                    EvalKind::Negative => {
                        if case.expect.activate
                            && case.expect.status != SkillOutputStatus::Abstained
                        {
                            issues.push(format!(
                                "{id}: negative cases must not activate or must ABSTAIN"
                            ));
                        }
                        if !package.modes.contains(&case.mode) && case.expect.activate {
                            issues.push(format!(
                                "{id}: mode outside g9-modes, so activate must be false"
                            ));
                        }
                    }
                    EvalKind::Adversarial => {
                        if tags.is_empty() {
                            issues.push(format!(
                                "{id}: adversarial cases must carry at least one tag"
                            ));
                        }
                        if case.expect.must_not.is_empty() {
                            issues.push(format!(
                                "{id}: adversarial cases must state expect.must_not"
                            ));
                        }
                        adversarial_tags.extend(tags.iter().map(|t| (*t).to_string()));
                    }
                    EvalKind::Acceptance => {
                        if !case.expect.activate {
                            issues.push(format!("{id}: acceptance cases must activate"));
                        }
                        if !package.modes.contains(&case.mode) {
                            issues.push(format!(
                                "{id}: mode {:?} is outside the skill's g9-modes",
                                case.mode
                            ));
                        }
                        if case.expect.must_include.is_empty() && case.expect.must_not.is_empty() {
                            issues.push(format!(
                                "{id}: acceptance cases need a measurable expectation"
                            ));
                        }
                    }
                }
                if matches!(kind, EvalKind::Positive | EvalKind::Acceptance)
                    && case.input.is_object()
                {
                    if let Err(e) = validate_against(&package.input_schema, &case.input, "/input") {
                        issues.push(format!(
                            "{id}: input does not validate against input.schema.json ({})",
                            crate::envelope::summarize(&e, 3)
                        ));
                    }
                }
                if let Some(segments) = &case.context_segments {
                    for (i, s) in segments.iter().enumerate() {
                        if s.classification == Classification::Secret
                            && !matches!(kind, EvalKind::Negative | EvalKind::Adversarial)
                        {
                            issues.push(format!("{id}: context_segments[{i}] SECRET content only in negative/adversarial fixtures"));
                        }
                    }
                }
            }
        }
        for t in REQUIRED_ADVERSARIAL_TAGS {
            if !adversarial_tags.contains(t) {
                issues.push(format!(
                    "adversarial.json: must include at least one case tagged `{t}`"
                ));
            }
        }
        StaticEvalReport {
            skill: package.name.clone(),
            counts,
            issues,
        }
    }

    /// NOT IMPLEMENTED by design. Executing fixtures against a model requires
    /// the Stage 06/07 evaluation harness (`g9-evaluation`, learner context
    /// fixtures, promotion-gate accounting with baselines). Until then every
    /// call returns `PreconditionFailed` / `skills.eval.not_implemented`, so a
    /// skill can never be reported EVALUATED by this crate.
    // `G9Error` is the mandated boundary error type (CLAUDE.md); its size is
    // owned by `g9-domain`, so boxing it here would only move the cost to callers.
    #[allow(clippy::result_large_err)]
    pub fn run_against_model(
        package: &SkillPackage,
        backend: &dyn InferenceBackend,
    ) -> Result<ModelEvalReport, G9Error> {
        Err(G9Error::precondition_failed(
            "skills.eval.not_implemented",
            "model-backed skill evaluation is a Stage 06/07 activity and is reported NOT RUN",
        )
        .with_details(serde_json::json!({
            "skill": package.name.as_str(),
            "backend": backend.name(),
            "status": "NOT RUN",
        })))
    }
}

/// Accept `PROPOSE`, `Propose` or `PromoteCanonical`-style names.
pub fn parse_action(name: &str) -> Option<Action> {
    let screaming = if name.chars().all(|c| c.is_ascii_uppercase() || c == '_') {
        name.to_string()
    } else {
        let mut s = String::new();
        for (i, c) in name.chars().enumerate() {
            if c.is_ascii_uppercase() && i > 0 {
                s.push('_');
            }
            s.push(c.to_ascii_uppercase());
        }
        s
    };
    serde_json::from_value(Value::String(screaming)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn action_names_parse_in_both_spellings() {
        assert_eq!(parse_action("Propose"), Some(Action::Propose));
        assert_eq!(parse_action("PROPOSE"), Some(Action::Propose));
        assert_eq!(
            parse_action("PromoteCanonical"),
            Some(Action::PromoteCanonical)
        );
        assert_eq!(
            parse_action("PROMOTE_CANONICAL"),
            Some(Action::PromoteCanonical)
        );
        assert_eq!(parse_action("nope"), None);
    }

    #[test]
    fn eval_kind_constants() {
        assert_eq!(EvalKind::Positive.minimum(), 3);
        assert_eq!(EvalKind::Acceptance.minimum(), 2);
        assert_eq!(EvalKind::Adversarial.file_name(), "adversarial.json");
        assert_eq!(EvalKind::Negative.id_prefix(), "NEG");
        let k: EvalKind = serde_json::from_str("\"adversarial\"").unwrap();
        assert_eq!(k, EvalKind::Adversarial);
    }

    #[test]
    fn eval_case_rejects_unknown_keys() {
        let text = r#"{"id":"T01-POS-001","kind":"positive","title":"t","mode":"STUDY","actor_kind":"OPERATOR","intent":"i","input":{},"expect":{"activate":true,"status":"COMPLETED"},"notes":"n","bogus":1}"#;
        assert!(serde_json::from_str::<EvalCase>(text).is_err());
        let ok = text.replace(",\"bogus\":1", "");
        assert!(serde_json::from_str::<EvalCase>(&ok).is_ok());
    }
}
