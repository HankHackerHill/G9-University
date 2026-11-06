//! Context assembly (ARCHITECTURE.md §10 "Context assembly: minimal,
//! provenance-tagged, privacy-filtered"; §8 "the context assembler refuses
//! `Secret` content in `InteractionMode::Exam`"; threat model B4;
//! `docs/privacy/data-lifecycle.md`: `SECRET` never enters model context).
//!
//! Rules applied by [`ContextAssembler::assemble`], in this order, and every
//! exclusion is reported in [`AssembleReport::segments_dropped`] — nothing is
//! dropped silently:
//!
//! 1. `Classification::Secret` is never included in any mode
//!    ([`DropReason::SecretInExam`] in EXAM, [`DropReason::SecretNeverAllowed`]
//!    elsewhere).
//! 2. In `InteractionMode::Exam` only `SystemPolicy`, `Task` and `LearnerInput`
//!    segments are allowed ([`DropReason::RoleNotAllowedInExam`] for the rest):
//!    no evidence, no skill instructions, no hints.
//! 3. Instruction roles require instruction-capable trust: `SystemPolicy` and
//!    `Task` need `Operator`/`SystemDeterministic`; `SkillInstructions` also
//!    accepts `SkillPackage` (hash-verified package text); `LearnerInput` needs
//!    `Operator`/`SystemDeterministic`. Content of trust `ModelOutput`,
//!    `ImportedDocument`, `ApprovedReference`, `SkillPackage` (outside
//!    `SkillInstructions`) or `Unknown` can only be `Evidence`
//!    ([`DropReason::UntrustedInstructionRole`]).
//! 4. Ordering: `SystemPolicy`, `SkillInstructions`, `Task`, `Evidence` (most
//!    trusted first, insertion order within a trust class), `LearnerInput`.
//! 5. Budget: while the token estimate exceeds `ContextBudget::max_tokens`,
//!    the least trusted segment is dropped first ([`DropReason::OverBudget`]);
//!    among equal trust, `Evidence` goes before `SkillInstructions`, then
//!    `LearnerInput`, `Task`, `SystemPolicy`; the most recently pushed goes first.
//!
//! `Classification::Private` is *not* filtered here: disclosure of private
//! content to the model requires an approval decided by policy before the
//! segment is pushed (`docs/privacy/data-lifecycle.md`). The assembler sees
//! only what `g9-app` already authorised.
//!
//! The report holds segment text in memory for the duration of the call; it is
//! never persisted. Persist [`AssembleReport::dropped_count`] via
//! [`crate::InvocationRecord`] only.

use crate::runtime::{ContextSegment, SegmentRole};
use g9_domain::{Classification, InteractionMode, TrustClass};
use serde::{Deserialize, Serialize};

/// Token budget for the assembled prompt (estimate-based, see
/// [`crate::estimate_tokens`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextBudget {
    /// Maximum estimated prompt tokens the assembled context may use.
    pub max_tokens: u32,
}

/// Why a segment was excluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DropReason {
    /// `Secret` content offered in EXAM mode (sealed keys, reference solutions).
    SecretInExam,
    /// `Secret` content offered in any other mode.
    SecretNeverAllowed,
    /// The segment did not fit the token budget.
    OverBudget,
    /// A trust class that cannot carry instructions was offered in an
    /// instruction or learner role.
    UntrustedInstructionRole,
    /// EXAM mode admits only `SystemPolicy`, `Task` and `LearnerInput`.
    RoleNotAllowedInExam,
}

/// Result of assembly. `segments_included` is in prompt order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssembleReport {
    /// Mode the context was assembled for.
    pub mode: InteractionMode,
    /// Budget that was applied.
    pub budget: ContextBudget,
    /// Admitted segments in prompt order (text in memory only).
    pub segments_included: Vec<ContextSegment>,
    /// `(source_ref, reason)` for every excluded segment, in exclusion order.
    pub segments_dropped: Vec<(String, DropReason)>,
    /// Sum of the included segments' token estimates.
    pub total_tokens: u32,
    /// Roles of the dropped segments, parallel to `segments_dropped`.
    pub dropped_roles: Vec<SegmentRole>,
}

impl AssembleReport {
    /// Number of dropped segments, for [`crate::InvocationRecord::segments_dropped`].
    pub fn dropped_count(&self) -> u32 {
        u32::try_from(self.segments_dropped.len()).unwrap_or(u32::MAX)
    }

    /// Whether any segment was dropped for a reason other than budget.
    pub fn has_policy_drops(&self) -> bool {
        self.segments_dropped
            .iter()
            .any(|(_, r)| *r != DropReason::OverBudget)
    }

    /// Whether a `SystemPolicy` or `Task` segment was dropped: the prompt is
    /// then missing its deterministic preamble and the caller should abort.
    pub fn lost_instruction_role(&self) -> bool {
        self.dropped_roles
            .iter()
            .any(|r| matches!(r, SegmentRole::SystemPolicy | SegmentRole::Task))
    }

    /// Take the included segments in prompt order.
    pub fn into_segments(self) -> Vec<ContextSegment> {
        self.segments_included
    }
}

/// Builder: `ContextAssembler::new(budget).with_mode(mode)`, then `push` each
/// candidate segment, then [`ContextAssembler::assemble`].
#[derive(Debug, Clone)]
pub struct ContextAssembler {
    budget: ContextBudget,
    /// Defaults to the most restrictive mode (EXAM) so that a forgotten
    /// `with_mode` fails closed.
    mode: InteractionMode,
    pending: Vec<ContextSegment>,
}

impl ContextAssembler {
    /// New assembler; mode defaults to `InteractionMode::Exam` (fail closed).
    pub fn new(budget: ContextBudget) -> Self {
        Self {
            budget,
            mode: InteractionMode::Exam,
            pending: Vec::new(),
        }
    }

    /// Set the interaction mode the context is assembled for.
    pub fn with_mode(mut self, mode: InteractionMode) -> Self {
        self.mode = mode;
        self
    }

    /// Offer a segment. Nothing is decided until [`Self::assemble`].
    pub fn push(&mut self, segment: ContextSegment) -> &mut Self {
        self.pending.push(segment);
        self
    }

    /// Offer several segments.
    pub fn extend(&mut self, segments: impl IntoIterator<Item = ContextSegment>) -> &mut Self {
        self.pending.extend(segments);
        self
    }

    /// The mode this assembler filters for.
    pub fn mode(&self) -> InteractionMode {
        self.mode
    }

    /// Apply the rules and produce the ordered, filtered, budgeted report.
    pub fn assemble(self) -> AssembleReport {
        let mode = self.mode;
        let budget = self.budget;
        let mut dropped: Vec<(String, DropReason)> = Vec::new();
        let mut dropped_roles: Vec<SegmentRole> = Vec::new();
        // (insertion index, segment) for candidates that passed the policy rules.
        let mut kept: Vec<(usize, ContextSegment)> = Vec::new();

        for (idx, seg) in self.pending.into_iter().enumerate() {
            if let Some(reason) = policy_drop_reason(mode, &seg) {
                dropped.push((seg.source_ref, reason));
                dropped_roles.push(seg.role);
                continue;
            }
            kept.push((idx, seg));
        }

        // Budget: drop least-trusted first until the estimate fits.
        let mut total = kept
            .iter()
            .fold(0u32, |acc, (_, s)| acc.saturating_add(s.approx_tokens));
        while total > budget.max_tokens && !kept.is_empty() {
            let victim = kept
                .iter()
                .enumerate()
                .max_by_key(|(_, (idx, s))| (s.trust, drop_priority(s.role), *idx))
                .map(|(pos, _)| pos);
            let Some(pos) = victim else { break };
            let (_, seg) = kept.remove(pos);
            total = total.saturating_sub(seg.approx_tokens);
            dropped.push((seg.source_ref, DropReason::OverBudget));
            dropped_roles.push(seg.role);
        }

        // Prompt order.
        kept.sort_by_key(|(idx, s)| (role_order(s.role), evidence_trust_key(s), *idx));
        let segments_included: Vec<ContextSegment> = kept.into_iter().map(|(_, s)| s).collect();

        AssembleReport {
            mode,
            budget,
            segments_included,
            segments_dropped: dropped,
            total_tokens: total,
            dropped_roles,
        }
    }
}

/// Non-budget exclusion rules, evaluated in priority order.
fn policy_drop_reason(mode: InteractionMode, seg: &ContextSegment) -> Option<DropReason> {
    if seg.classification == Classification::Secret {
        return Some(if mode == InteractionMode::Exam {
            DropReason::SecretInExam
        } else {
            DropReason::SecretNeverAllowed
        });
    }
    if mode == InteractionMode::Exam
        && !matches!(
            seg.role,
            SegmentRole::SystemPolicy | SegmentRole::Task | SegmentRole::LearnerInput
        )
    {
        return Some(DropReason::RoleNotAllowedInExam);
    }
    if !role_accepts_trust(seg.role, seg.trust) {
        return Some(DropReason::UntrustedInstructionRole);
    }
    None
}

/// Which trust classes may fill a role.
fn role_accepts_trust(role: SegmentRole, trust: TrustClass) -> bool {
    match role {
        SegmentRole::SystemPolicy | SegmentRole::Task | SegmentRole::LearnerInput => {
            trust.may_carry_instructions()
        }
        SegmentRole::SkillInstructions => {
            trust.may_carry_instructions() || trust == TrustClass::SkillPackage
        }
        SegmentRole::Evidence => true,
    }
}

/// Prompt position of a role.
fn role_order(role: SegmentRole) -> u8 {
    match role {
        SegmentRole::SystemPolicy => 0,
        SegmentRole::SkillInstructions => 1,
        SegmentRole::Task => 2,
        SegmentRole::Evidence => 3,
        SegmentRole::LearnerInput => 4,
    }
}

/// Within `Evidence`, most trusted first (`TrustClass` is ordered most → least).
fn evidence_trust_key(seg: &ContextSegment) -> Option<TrustClass> {
    (seg.role == SegmentRole::Evidence).then_some(seg.trust)
}

/// Among equal trust, which role is sacrificed first under budget pressure
/// (higher = dropped first).
fn drop_priority(role: SegmentRole) -> u8 {
    match role {
        SegmentRole::Evidence => 4,
        SegmentRole::SkillInstructions => 3,
        SegmentRole::LearnerInput => 2,
        SegmentRole::Task => 1,
        SegmentRole::SystemPolicy => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn seg(role: SegmentRole, trust: TrustClass, class: Classification, r: &str) -> ContextSegment {
        ContextSegment::new(role, "x".repeat(40), trust, class, r)
    }

    fn assembler(mode: InteractionMode, max_tokens: u32) -> ContextAssembler {
        ContextAssembler::new(ContextBudget { max_tokens }).with_mode(mode)
    }

    #[test]
    fn default_mode_is_exam_fail_closed() {
        assert_eq!(
            ContextAssembler::new(ContextBudget { max_tokens: 10 }).mode(),
            InteractionMode::Exam
        );
    }

    #[test]
    fn secret_dropped_in_every_mode_role_and_trust() {
        for mode in InteractionMode::ALL {
            for role in SegmentRole::ALL {
                for trust in [
                    TrustClass::Operator,
                    TrustClass::SystemDeterministic,
                    TrustClass::Unknown,
                ] {
                    let mut a = assembler(mode, 1_000);
                    a.push(seg(role, trust, Classification::Secret, "sealed-key"));
                    let r = a.assemble();
                    assert!(
                        r.segments_included.is_empty(),
                        "{mode:?} {role:?} {trust:?}"
                    );
                    let expected = if mode == InteractionMode::Exam {
                        DropReason::SecretInExam
                    } else {
                        DropReason::SecretNeverAllowed
                    };
                    assert_eq!(
                        r.segments_dropped,
                        vec![("sealed-key".to_string(), expected)]
                    );
                }
            }
        }
    }

    #[test]
    fn exam_mode_keeps_only_policy_task_and_learner_input() {
        let mut a = assembler(InteractionMode::Exam, 1_000);
        a.push(seg(
            SegmentRole::SystemPolicy,
            TrustClass::SystemDeterministic,
            Classification::Internal,
            "pol",
        ))
        .push(seg(
            SegmentRole::SkillInstructions,
            TrustClass::SkillPackage,
            Classification::Internal,
            "skill",
        ))
        .push(seg(
            SegmentRole::Task,
            TrustClass::SystemDeterministic,
            Classification::Internal,
            "task",
        ))
        .push(seg(
            SegmentRole::Evidence,
            TrustClass::ApprovedReference,
            Classification::Public,
            "ref",
        ))
        .push(seg(
            SegmentRole::LearnerInput,
            TrustClass::Operator,
            Classification::Private,
            "learner",
        ));
        let r = a.assemble();
        let refs: Vec<&str> = r
            .segments_included
            .iter()
            .map(|s| s.source_ref.as_str())
            .collect();
        assert_eq!(refs, vec!["pol", "task", "learner"]);
        assert_eq!(
            r.segments_dropped,
            vec![
                ("skill".to_string(), DropReason::RoleNotAllowedInExam),
                ("ref".to_string(), DropReason::RoleNotAllowedInExam)
            ]
        );
        assert!(r.has_policy_drops());
        assert!(!r.lost_instruction_role());
    }

    #[test]
    fn untrusted_content_cannot_take_instruction_roles() {
        let untrusted = [
            TrustClass::ModelOutput,
            TrustClass::ImportedDocument,
            TrustClass::SkillPackage,
            TrustClass::Unknown,
            TrustClass::ApprovedReference,
        ];
        for trust in untrusted {
            for role in [
                SegmentRole::SystemPolicy,
                SegmentRole::Task,
                SegmentRole::LearnerInput,
            ] {
                let mut a = assembler(InteractionMode::Study, 1_000);
                a.push(seg(role, trust, Classification::Internal, "inj"));
                let r = a.assemble();
                assert!(r.segments_included.is_empty(), "{trust:?} {role:?}");
                assert_eq!(
                    r.segments_dropped[0].1,
                    DropReason::UntrustedInstructionRole
                );
                assert!(r.lost_instruction_role() || role == SegmentRole::LearnerInput);
            }
            // The same content is accepted as Evidence.
            let mut a = assembler(InteractionMode::Study, 1_000);
            a.push(seg(
                SegmentRole::Evidence,
                trust,
                Classification::Internal,
                "ev",
            ));
            assert_eq!(a.assemble().segments_included.len(), 1);
        }
        // SkillInstructions accepts SkillPackage but not ModelOutput.
        let mut a = assembler(InteractionMode::Study, 1_000);
        a.push(seg(
            SegmentRole::SkillInstructions,
            TrustClass::SkillPackage,
            Classification::Internal,
            "ok",
        ))
        .push(seg(
            SegmentRole::SkillInstructions,
            TrustClass::ModelOutput,
            Classification::Internal,
            "bad",
        ));
        let r = a.assemble();
        assert_eq!(r.segments_included.len(), 1);
        assert_eq!(
            r.segments_dropped,
            vec![("bad".to_string(), DropReason::UntrustedInstructionRole)]
        );
    }

    #[test]
    fn prompt_order_is_role_then_trust_then_insertion() {
        let mut a = assembler(InteractionMode::Study, 10_000);
        a.push(seg(
            SegmentRole::LearnerInput,
            TrustClass::Operator,
            Classification::Private,
            "learner",
        ))
        .push(seg(
            SegmentRole::Evidence,
            TrustClass::ModelOutput,
            Classification::Internal,
            "ev-model",
        ))
        .push(seg(
            SegmentRole::Evidence,
            TrustClass::Operator,
            Classification::Private,
            "ev-note-1",
        ))
        .push(seg(
            SegmentRole::Task,
            TrustClass::SystemDeterministic,
            Classification::Internal,
            "task",
        ))
        .push(seg(
            SegmentRole::Evidence,
            TrustClass::ApprovedReference,
            Classification::Public,
            "ev-ref",
        ))
        .push(seg(
            SegmentRole::Evidence,
            TrustClass::Operator,
            Classification::Private,
            "ev-note-2",
        ))
        .push(seg(
            SegmentRole::SkillInstructions,
            TrustClass::SystemDeterministic,
            Classification::Internal,
            "skill",
        ))
        .push(seg(
            SegmentRole::SystemPolicy,
            TrustClass::SystemDeterministic,
            Classification::Internal,
            "pol",
        ));
        let r = a.assemble();
        let refs: Vec<&str> = r
            .segments_included
            .iter()
            .map(|s| s.source_ref.as_str())
            .collect();
        assert_eq!(
            refs,
            vec![
                "pol",
                "skill",
                "task",
                "ev-note-1",
                "ev-note-2",
                "ev-ref",
                "ev-model",
                "learner"
            ]
        );
        assert_eq!(r.total_tokens, 80);
        assert!(r.segments_dropped.is_empty());
    }

    #[test]
    fn over_budget_drops_least_trusted_first_then_evidence_then_latest() {
        // Each segment is 10 tokens; budget 35 → two must go.
        let mut a = assembler(InteractionMode::Study, 35);
        a.push(seg(
            SegmentRole::SystemPolicy,
            TrustClass::SystemDeterministic,
            Classification::Internal,
            "pol",
        ))
        .push(seg(
            SegmentRole::Task,
            TrustClass::SystemDeterministic,
            Classification::Internal,
            "task",
        ))
        .push(seg(
            SegmentRole::Evidence,
            TrustClass::Operator,
            Classification::Private,
            "note",
        ))
        .push(seg(
            SegmentRole::Evidence,
            TrustClass::ImportedDocument,
            Classification::Internal,
            "doc",
        ))
        .push(seg(
            SegmentRole::Evidence,
            TrustClass::ModelOutput,
            Classification::Internal,
            "prior-model",
        ));
        let r = a.assemble();
        assert_eq!(
            r.segments_dropped,
            vec![
                ("prior-model".to_string(), DropReason::OverBudget),
                ("doc".to_string(), DropReason::OverBudget)
            ]
        );
        let refs: Vec<&str> = r
            .segments_included
            .iter()
            .map(|s| s.source_ref.as_str())
            .collect();
        assert_eq!(refs, vec!["pol", "task", "note"]);
        assert_eq!(r.total_tokens, 30);
        assert_eq!(r.dropped_count(), 2);
        assert!(!r.has_policy_drops());

        // Equal trust: Evidence before Task before SystemPolicy; latest pushed first.
        let mut a = assembler(InteractionMode::Study, 25);
        a.push(seg(
            SegmentRole::SystemPolicy,
            TrustClass::SystemDeterministic,
            Classification::Internal,
            "pol",
        ))
        .push(seg(
            SegmentRole::Task,
            TrustClass::SystemDeterministic,
            Classification::Internal,
            "task",
        ))
        .push(seg(
            SegmentRole::Evidence,
            TrustClass::SystemDeterministic,
            Classification::Internal,
            "ev-1",
        ))
        .push(seg(
            SegmentRole::Evidence,
            TrustClass::SystemDeterministic,
            Classification::Internal,
            "ev-2",
        ));
        let r = a.assemble();
        assert_eq!(
            r.segments_dropped,
            vec![
                ("ev-2".to_string(), DropReason::OverBudget),
                ("ev-1".to_string(), DropReason::OverBudget)
            ]
        );
        // Zero budget: everything is dropped and reported, nothing silent.
        let mut a = assembler(InteractionMode::Study, 0);
        a.push(seg(
            SegmentRole::SystemPolicy,
            TrustClass::SystemDeterministic,
            Classification::Internal,
            "pol",
        ));
        let r = a.assemble();
        assert!(r.segments_included.is_empty());
        assert_eq!(r.segments_dropped.len(), 1);
        assert!(r.lost_instruction_role());
    }
}
