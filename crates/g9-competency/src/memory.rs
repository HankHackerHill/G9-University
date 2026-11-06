//! Learner memory (M09): hypotheses about the learner with provenance,
//! evidence links, operator decisions and derived-record invalidation
//! (ARCHITECTURE.md §8 "Learner memory"; gate B05; C19/C21).
//!
//! Any actor (including the model, through a skill) may *propose* a
//! hypothesis. Only the operator can accept, reject or retire one. Rejecting a
//! hypothesis returns the derived plans/interventions/recommendations that
//! referenced it so the application layer can invalidate them.

use crate::CompetencyError;
use g9_domain::{Actor, ActorId, CompetencyId, Confidence, EvidenceId, Provenance, UtcMillis};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Identifier of a learner hypothesis. Same character rules as the
/// `g9_domain` typed ids (`[A-Za-z0-9._:-]`, 1..=128 bytes).
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct HypothesisId(String);

impl HypothesisId {
    /// Validate and wrap an identifier string.
    pub fn parse(s: &str) -> Result<Self, CompetencyError> {
        let ok = !s.is_empty()
            && s.len() <= 128
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'));
        if ok {
            Ok(Self(s.to_string()))
        } else {
            Err(CompetencyError::InvalidHypothesis {
                reason: "hypothesis id must be 1..=128 bytes of [A-Za-z0-9._:-]".to_string(),
            })
        }
    }

    /// Borrow the underlying string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for HypothesisId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HypothesisId({})", self.0)
    }
}

impl fmt::Display for HypothesisId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for HypothesisId {
    type Error = CompetencyError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

impl From<HypothesisId> for String {
    fn from(id: HypothesisId) -> String {
        id.0
    }
}

/// What kind of claim the hypothesis makes about the learner.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HypothesisKind {
    /// A specific wrong belief or mental model.
    Misconception,
    /// A reliable capability worth building on.
    Strength,
    /// A learning-method preference (evidence-backed, not self-reported taste).
    Preference,
    /// A recurring obstacle (environment, prerequisite, habit).
    Blocker,
}

/// Lifecycle of a hypothesis. Model output can only create `Proposed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HypothesisStatus {
    /// Proposed by some actor; not yet an operator decision.
    Proposed,
    /// Operator accepted it; derived plans may rely on it.
    Accepted,
    /// Operator rejected it; derived records referencing it are invalid.
    Rejected,
    /// No longer relevant (e.g. the misconception was resolved); terminal.
    Retired,
}

impl HypothesisStatus {
    /// Whether the status is terminal (no further decisions).
    pub const fn is_terminal(self) -> bool {
        matches!(self, HypothesisStatus::Rejected | HypothesisStatus::Retired)
    }
}

/// Operator decision events on a hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HypothesisEvent {
    /// Accept a proposed hypothesis.
    Accept,
    /// Reject a proposed or (as a correction) accepted hypothesis.
    Reject,
    /// Retire a proposed or accepted hypothesis.
    Retire,
}

/// Allowed `(from, event, to)` edges.
const TRANSITIONS: &[(HypothesisStatus, HypothesisEvent, HypothesisStatus)] = &[
    (
        HypothesisStatus::Proposed,
        HypothesisEvent::Accept,
        HypothesisStatus::Accepted,
    ),
    (
        HypothesisStatus::Proposed,
        HypothesisEvent::Reject,
        HypothesisStatus::Rejected,
    ),
    (
        HypothesisStatus::Proposed,
        HypothesisEvent::Retire,
        HypothesisStatus::Retired,
    ),
    (
        HypothesisStatus::Accepted,
        HypothesisEvent::Reject,
        HypothesisStatus::Rejected,
    ),
    (
        HypothesisStatus::Accepted,
        HypothesisEvent::Retire,
        HypothesisStatus::Retired,
    ),
];

/// A claim about the learner, with the evidence for and against it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearnerHypothesis {
    /// Hypothesis identifier.
    pub id: HypothesisId,
    /// Kind of claim.
    pub kind: HypothesisKind,
    /// The claim, in plain language.
    pub statement: String,
    /// Competencies the claim is about.
    pub competency_ids: Vec<CompetencyId>,
    /// Evidence items that support the claim.
    pub supporting_refs: Vec<EvidenceId>,
    /// Evidence items that contradict the claim.
    pub contradicting_refs: Vec<EvidenceId>,
    /// Lower bound of the proposer's confidence.
    pub confidence_low: Confidence,
    /// Upper bound of the proposer's confidence.
    pub confidence_high: Confidence,
    /// Lifecycle status.
    pub status: HypothesisStatus,
    /// Who/what proposed it and how far that source may be trusted.
    pub provenance: Provenance,
    /// When it was proposed.
    pub created_at: UtcMillis,
    /// Operator who made the last decision, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_by: Option<ActorId>,
    /// When the last decision was made, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_at: Option<UtcMillis>,
}

impl LearnerHypothesis {
    /// Structural validation for a new proposal.
    pub fn validate_proposal(&self) -> Result<(), CompetencyError> {
        let invalid = |reason: &str| CompetencyError::InvalidHypothesis {
            reason: reason.to_string(),
        };
        if self.statement.trim().is_empty() {
            return Err(invalid("statement must not be empty"));
        }
        if self.confidence_low > self.confidence_high {
            return Err(invalid("confidence_low must not exceed confidence_high"));
        }
        if self.status != HypothesisStatus::Proposed {
            return Err(invalid(
                "a new hypothesis must be PROPOSED; decisions are operator commands",
            ));
        }
        if self.decided_by.is_some() || self.decided_at.is_some() {
            return Err(invalid("a new hypothesis cannot carry a decision"));
        }
        Ok(())
    }
}

/// Kind of derived record that can depend on a hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DerivedRefKind {
    /// A study/teaching plan.
    Plan,
    /// A remedial intervention (M12).
    Intervention,
    /// A skill recommendation.
    Recommendation,
}

/// Reference to a derived record that relies on a hypothesis.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DerivedRef {
    /// Kind of record.
    pub kind: DerivedRefKind,
    /// Record identifier (opaque to this crate).
    pub id: String,
}

/// In-memory ledger of hypotheses and their dependents. The application layer
/// persists it; this type only enforces the decision rules.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct HypothesisLedger {
    hypotheses: BTreeMap<HypothesisId, LearnerHypothesis>,
    dependents: BTreeMap<HypothesisId, BTreeSet<DerivedRef>>,
}

impl HypothesisLedger {
    /// Empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a proposal from any actor. The hypothesis must be `Proposed`
    /// and carry no decision; its provenance may be of any trust class
    /// (model output can only ever create proposals).
    pub fn propose(
        &mut self,
        hypothesis: LearnerHypothesis,
    ) -> Result<&LearnerHypothesis, CompetencyError> {
        hypothesis.validate_proposal()?;
        if self.hypotheses.contains_key(&hypothesis.id) {
            return Err(CompetencyError::DuplicateHypothesis {
                id: hypothesis.id.to_string(),
            });
        }
        let id = hypothesis.id.clone();
        self.dependents.entry(id.clone()).or_default();
        self.hypotheses.insert(id.clone(), hypothesis);
        self.hypotheses
            .get(&id)
            .ok_or_else(|| CompetencyError::UnknownHypothesis { id: id.to_string() })
    }

    /// Operator accepts a proposed hypothesis.
    pub fn accept(
        &mut self,
        actor: &Actor,
        id: &HypothesisId,
        at: UtcMillis,
    ) -> Result<&LearnerHypothesis, CompetencyError> {
        self.decide(actor, id, HypothesisEvent::Accept, at)
    }

    /// Operator rejects a hypothesis (proposed, or accepted as a correction).
    /// Returns the derived records that referenced it and must now be revised.
    pub fn reject(
        &mut self,
        actor: &Actor,
        id: &HypothesisId,
        at: UtcMillis,
    ) -> Result<Vec<DerivedRef>, CompetencyError> {
        self.decide(actor, id, HypothesisEvent::Reject, at)?;
        self.invalidations(id)
    }

    /// Operator retires a hypothesis that is no longer relevant.
    pub fn retire(
        &mut self,
        actor: &Actor,
        id: &HypothesisId,
        at: UtcMillis,
    ) -> Result<&LearnerHypothesis, CompetencyError> {
        self.decide(actor, id, HypothesisEvent::Retire, at)
    }

    /// Register a derived record that relies on `hypothesis_id`. Allowed while
    /// the hypothesis is `Proposed` or `Accepted`; refused once it is terminal.
    pub fn register_dependent(
        &mut self,
        hypothesis_id: &HypothesisId,
        dependent: DerivedRef,
    ) -> Result<(), CompetencyError> {
        let h = self.require(hypothesis_id)?;
        if h.status.is_terminal() {
            return Err(CompetencyError::DependentNotAllowed {
                id: hypothesis_id.to_string(),
                status: status_name(h.status),
            });
        }
        self.dependents
            .entry(hypothesis_id.clone())
            .or_default()
            .insert(dependent);
        Ok(())
    }

    /// Derived records invalidated by the rejection of `hypothesis_id`: all of
    /// its registered dependents when it is `Rejected`, otherwise none.
    pub fn invalidations(
        &self,
        hypothesis_id: &HypothesisId,
    ) -> Result<Vec<DerivedRef>, CompetencyError> {
        let h = self.require(hypothesis_id)?;
        if h.status != HypothesisStatus::Rejected {
            return Ok(Vec::new());
        }
        Ok(self.dependents_of(hypothesis_id))
    }

    /// Registered dependents of a hypothesis in canonical order.
    pub fn dependents_of(&self, hypothesis_id: &HypothesisId) -> Vec<DerivedRef> {
        self.dependents
            .get(hypothesis_id)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Look up a hypothesis.
    pub fn get(&self, id: &HypothesisId) -> Option<&LearnerHypothesis> {
        self.hypotheses.get(id)
    }

    /// All hypotheses in ascending id order.
    pub fn iter(&self) -> impl Iterator<Item = &LearnerHypothesis> {
        self.hypotheses.values()
    }

    /// Hypotheses with the given status, in ascending id order.
    pub fn by_status(&self, status: HypothesisStatus) -> Vec<&LearnerHypothesis> {
        self.hypotheses
            .values()
            .filter(|h| h.status == status)
            .collect()
    }

    /// Number of hypotheses.
    pub fn len(&self) -> usize {
        self.hypotheses.len()
    }

    /// Whether the ledger is empty.
    pub fn is_empty(&self) -> bool {
        self.hypotheses.is_empty()
    }

    fn require(&self, id: &HypothesisId) -> Result<&LearnerHypothesis, CompetencyError> {
        self.hypotheses
            .get(id)
            .ok_or_else(|| CompetencyError::UnknownHypothesis { id: id.to_string() })
    }

    fn decide(
        &mut self,
        actor: &Actor,
        id: &HypothesisId,
        event: HypothesisEvent,
        at: UtcMillis,
    ) -> Result<&LearnerHypothesis, CompetencyError> {
        if !actor.is_operator() {
            return Err(CompetencyError::OperatorRequired {
                action: format!("hypothesis.{}", event_name(event)),
                actor_kind: actor_kind_name(actor),
            });
        }
        let current = self.require(id)?.status;
        let next = TRANSITIONS
            .iter()
            .find(|(from, on, _)| *from == current && *on == event)
            .map(|(_, _, to)| *to)
            .ok_or_else(|| CompetencyError::HypothesisTransition {
                id: id.to_string(),
                status: status_name(current),
                event: event_name(event).to_string(),
            })?;
        let h = self
            .hypotheses
            .get_mut(id)
            .ok_or_else(|| CompetencyError::UnknownHypothesis { id: id.to_string() })?;
        h.status = next;
        h.decided_by = Some(actor.id.clone());
        h.decided_at = Some(at);
        Ok(h)
    }
}

fn status_name(s: HypothesisStatus) -> String {
    serde_json::to_value(s)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{s:?}"))
}

fn event_name(e: HypothesisEvent) -> &'static str {
    match e {
        HypothesisEvent::Accept => "accept",
        HypothesisEvent::Reject => "reject",
        HypothesisEvent::Retire => "retire",
    }
}

fn actor_kind_name(actor: &Actor) -> String {
    serde_json::to_value(&actor.kind)
        .ok()
        .and_then(|v| v.get("kind").and_then(|k| k.as_str()).map(str::to_string))
        .unwrap_or_else(|| "UNKNOWN".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{cid, day, provenance};
    use g9_domain::{ErrorCategory, G9Error, SkillId, TrustClass};
    use pretty_assertions::assert_eq;

    fn hid(s: &str) -> HypothesisId {
        HypothesisId::parse(s).unwrap()
    }

    fn operator() -> Actor {
        Actor::operator(ActorId::parse("operator").unwrap())
    }

    fn model_actor() -> Actor {
        Actor::model(ActorId::parse("model").unwrap())
    }

    fn proposal(id: &str, trust: TrustClass) -> LearnerHypothesis {
        LearnerHypothesis {
            id: hid(id),
            kind: HypothesisKind::Misconception,
            statement: "Believes a 302 redirect terminates request processing".to_string(),
            competency_ids: vec![cid("web")],
            supporting_refs: vec![EvidenceId::parse("ev1").unwrap()],
            contradicting_refs: vec![],
            confidence_low: Confidence::new(0.3).unwrap(),
            confidence_high: Confidence::new(0.7).unwrap(),
            status: HypothesisStatus::Proposed,
            provenance: provenance(trust, day(0)),
            created_at: day(0),
            decided_by: None,
            decided_at: None,
        }
    }

    fn dep(kind: DerivedRefKind, id: &str) -> DerivedRef {
        DerivedRef {
            kind,
            id: id.to_string(),
        }
    }

    #[test]
    fn b05_rejected_hypothesis_invalidates_dependents() {
        let mut ledger = HypothesisLedger::new();
        ledger
            .propose(proposal("h1", TrustClass::ModelOutput))
            .unwrap();
        assert_eq!(
            ledger.get(&hid("h1")).unwrap().status,
            HypothesisStatus::Proposed
        );

        ledger.accept(&operator(), &hid("h1"), day(1)).unwrap();
        let h = ledger.get(&hid("h1")).unwrap();
        assert_eq!(h.status, HypothesisStatus::Accepted);
        assert_eq!(h.decided_by, Some(ActorId::parse("operator").unwrap()));
        assert_eq!(h.decided_at, Some(day(1)));

        ledger
            .register_dependent(&hid("h1"), dep(DerivedRefKind::Plan, "plan-7"))
            .unwrap();
        ledger
            .register_dependent(&hid("h1"), dep(DerivedRefKind::Intervention, "int-2"))
            .unwrap();
        ledger
            .register_dependent(&hid("h1"), dep(DerivedRefKind::Recommendation, "rec-9"))
            .unwrap();
        ledger
            .register_dependent(&hid("h1"), dep(DerivedRefKind::Plan, "plan-7"))
            .unwrap(); // idempotent
        assert!(
            ledger.invalidations(&hid("h1")).unwrap().is_empty(),
            "accepted: nothing to invalidate"
        );

        // Operator corrects the wrong hypothesis: accepted -> rejected.
        let invalidated = ledger.reject(&operator(), &hid("h1"), day(2)).unwrap();
        assert_eq!(
            invalidated,
            vec![
                dep(DerivedRefKind::Plan, "plan-7"),
                dep(DerivedRefKind::Intervention, "int-2"),
                dep(DerivedRefKind::Recommendation, "rec-9"),
            ]
        );
        assert_eq!(ledger.invalidations(&hid("h1")).unwrap(), invalidated);
        assert_eq!(
            ledger.get(&hid("h1")).unwrap().status,
            HypothesisStatus::Rejected
        );
        assert_eq!(ledger.get(&hid("h1")).unwrap().decided_at, Some(day(2)));

        // No new derived records may attach to a rejected hypothesis.
        let err = ledger
            .register_dependent(&hid("h1"), dep(DerivedRefKind::Plan, "plan-8"))
            .unwrap_err();
        assert!(matches!(err, CompetencyError::DependentNotAllowed { .. }));
        assert_eq!(
            G9Error::from(err).category,
            ErrorCategory::PreconditionFailed
        );

        // Terminal: cannot be re-accepted.
        assert!(matches!(
            ledger.accept(&operator(), &hid("h1"), day(3)).unwrap_err(),
            CompetencyError::HypothesisTransition { .. }
        ));
        assert!(matches!(
            ledger.invalidations(&hid("nope")).unwrap_err(),
            CompetencyError::UnknownHypothesis { .. }
        ));
    }

    #[test]
    fn only_operator_can_accept_hypothesis() {
        let mut ledger = HypothesisLedger::new();
        ledger
            .propose(proposal("h1", TrustClass::ModelOutput))
            .unwrap();
        ledger
            .propose(proposal("h2", TrustClass::Operator))
            .unwrap();

        let skill = Actor::skill(
            ActorId::parse("skill").unwrap(),
            SkillId::parse("g9-learner-model").unwrap(),
        );
        let system = Actor::system(ActorId::parse("system").unwrap());
        for actor in [model_actor(), skill, system] {
            let err = ledger.accept(&actor, &hid("h1"), day(1)).unwrap_err();
            assert!(
                matches!(err, CompetencyError::OperatorRequired { .. }),
                "{actor:?}"
            );
            assert_eq!(G9Error::from(err).category, ErrorCategory::Unauthorized);
            assert!(ledger.reject(&actor, &hid("h1"), day(1)).is_err());
            assert!(ledger.retire(&actor, &hid("h1"), day(1)).is_err());
            assert_eq!(
                ledger.get(&hid("h1")).unwrap().status,
                HypothesisStatus::Proposed
            );
        }
        let model_err: G9Error = ledger
            .accept(&model_actor(), &hid("h1"), day(1))
            .unwrap_err()
            .into();
        assert!(model_err.message.contains("MODEL"), "{model_err}");

        ledger.accept(&operator(), &hid("h1"), day(1)).unwrap();
        ledger.retire(&operator(), &hid("h2"), day(1)).unwrap();
        assert_eq!(ledger.by_status(HypothesisStatus::Accepted).len(), 1);
        assert_eq!(ledger.by_status(HypothesisStatus::Retired).len(), 1);
        assert_eq!(ledger.len(), 2);
    }

    #[test]
    fn proposals_are_validated_and_decisions_cannot_be_smuggled_in() {
        let mut ledger = HypothesisLedger::new();
        let mut pre_accepted = proposal("h1", TrustClass::ModelOutput);
        pre_accepted.status = HypothesisStatus::Accepted;
        assert!(matches!(
            ledger.propose(pre_accepted).unwrap_err(),
            CompetencyError::InvalidHypothesis { .. }
        ));
        let mut decided = proposal("h1", TrustClass::ModelOutput);
        decided.decided_by = Some(ActorId::parse("operator").unwrap());
        assert!(ledger.propose(decided).is_err());
        let mut inverted = proposal("h1", TrustClass::ModelOutput);
        inverted.confidence_low = Confidence::new(0.9).unwrap();
        assert!(ledger.propose(inverted).is_err());
        let mut blank = proposal("h1", TrustClass::ModelOutput);
        blank.statement = " ".to_string();
        assert!(ledger.propose(blank).is_err());

        ledger
            .propose(proposal("h1", TrustClass::ModelOutput))
            .unwrap();
        assert!(matches!(
            ledger
                .propose(proposal("h1", TrustClass::ModelOutput))
                .unwrap_err(),
            CompetencyError::DuplicateHypothesis { .. }
        ));
        assert!(HypothesisId::parse("bad id").is_err());
        assert!(HypothesisId::parse("").is_err());
    }

    #[test]
    fn ledger_serde_round_trip() {
        let mut ledger = HypothesisLedger::new();
        ledger
            .propose(proposal("h1", TrustClass::ModelOutput))
            .unwrap();
        ledger
            .register_dependent(&hid("h1"), dep(DerivedRefKind::Plan, "plan-1"))
            .unwrap();
        ledger.accept(&operator(), &hid("h1"), day(1)).unwrap();
        let json = serde_json::to_string(&ledger).unwrap();
        let back: HypothesisLedger = serde_json::from_str(&json).unwrap();
        assert_eq!(back, ledger);
        let v = serde_json::to_value(ledger.get(&hid("h1")).unwrap()).unwrap();
        assert_eq!(v["status"], "ACCEPTED");
        assert_eq!(v["kind"], "MISCONCEPTION");
        assert_eq!(v["provenance"]["trust"], "MODEL_OUTPUT");
    }
}
