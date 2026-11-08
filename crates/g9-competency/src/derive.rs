//! Replayable derivation of competency state from observations
//! (ARCHITECTURE.md §8 "Derivation (replayable fold)"; handoff §9 "Skill state
//! (not a universal float)"; gate A05).
//!
//! `derive_state` is a pure function of `(competency, observations, rules, now)`:
//!
//! * observations are canonicalised (foreign/untrusted/superseded ones dropped,
//!   the rest sorted by `(observed_at, id)`) so input order is irrelevant;
//! * state advances only through `CompetencyState::transition`, one legal edge
//!   at a time, in observation order — it never jumps;
//! * `Regress` moves the state back but keeps every earlier observation in
//!   `evidence_refs` and in the per-dimension statistics (nothing is erased);
//! * `RetestDue` is applied when the latest validation evidence is older than
//!   the competency's validity window, both while replaying and at `now`;
//! * contradictions between observations are reported, not resolved.

use crate::graph::Competency;
use crate::observation::{retained_for, CompetencyObservation, ObservationSource};
use crate::rules::DerivationRules;
use crate::CompetencyError;
use g9_domain::state::CompetencyEvent;
use g9_domain::{
    AssistanceLevel, CompetencyId, CompetencyState, Confidence, LearningDimension, ObservationId,
    UtcMillis,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Milliseconds per day, used to turn day-based windows into instants.
const DAY_MS: i64 = 86_400_000;

/// Statistics for one learning dimension, computed over the retained observations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DimensionEstimate {
    /// Retained observations in this dimension (any assistance level).
    pub n_total: u32,
    /// Retained unaided observations in this dimension.
    pub n_unaided: u32,
    /// Score of the most recent retained observation in this dimension.
    pub latest_score: u8,
    /// Mean score weighted by `AssistanceLevel::independence_weight`;
    /// `None` when no observation carries independence weight (all guided/shown).
    pub independence_weighted_mean: Option<f64>,
    /// Instant of the most recent retained observation in this dimension.
    pub last_observed_at: UtcMillis,
    /// Mean stated uncertainty of the retained observations (0 = certain).
    pub uncertainty: Confidence,
}

/// Two retained observations whose results are hard to reconcile; surfaced
/// for the diagnosis loop (handoff §10 step 4), never silently resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contradiction {
    /// The earlier observation.
    pub older: ObservationId,
    /// The later observation.
    pub newer: ObservationId,
    /// Secret-free explanation of the conflict.
    pub reason: String,
}

/// Derived view of one competency. Rebuildable at any time from the
/// observation history plus the rules identified by `rules_hash`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompetencyProjection {
    /// Competency this projection describes.
    pub competency_id: CompetencyId,
    /// Current state per the `g9_domain::CompetencyState` machine.
    pub state: CompetencyState,
    /// Per-dimension evidence summary (only dimensions with evidence appear).
    pub per_dimension: BTreeMap<LearningDimension, DimensionEstimate>,
    /// Detected contradictions in canonical `(older, newer)` order.
    pub contradictions: Vec<Contradiction>,
    /// Every retained observation, in `(observed_at, id)` order. Superseded
    /// observations are excluded; regressed-over evidence is retained.
    pub evidence_refs: Vec<ObservationId>,
    /// The `now` the projection was derived for (drives `RetestDue`).
    pub derived_at: UtcMillis,
    /// `DerivationRules::hash()` of the rules that produced this projection.
    pub rules_hash: String,
    /// Validation-level evidence exists but was excluded because it is not
    /// validation-eligible (e.g. derived from a `SELF_ADJUDICATED` grade
    /// decision, ADR-0005/0012, Addendum A item 2), and the state is not
    /// `Validated`: the competency stays at most `Independent`, flagged
    /// provisional. Always `false` from [`derive_state`].
    #[serde(default)]
    pub provisional: bool,
}

impl CompetencyProjection {
    /// Estimate for one dimension, if any evidence exists.
    pub fn dimension(&self, d: LearningDimension) -> Option<&DimensionEstimate> {
        self.per_dimension.get(&d)
    }
}

/// Pure, replayable fold from observations to a projection.
///
/// Observations of other competencies, observations without authoritative
/// provenance (C10/C21) and observations superseded by another input
/// observation never influence the result. Use [`derive_state_checked`] to
/// reject such inputs with an error instead of ignoring them.
pub fn derive_state(
    competency: &Competency,
    observations: &[CompetencyObservation],
    rules: &DerivationRules,
    now: UtcMillis,
) -> CompetencyProjection {
    derive_state_excluding_validation(competency, observations, rules, now, &BTreeSet::new())
}

/// [`derive_state`] where the observations named in `validation_ineligible`
/// never count as validation evidence (ADR-0005/0012, ARCHITECTURE.md
/// Addendum A item 2). The caller lists every observation derived from a
/// grade decision that is not validation-eligible — in particular every
/// decision with reliability class `SELF_ADJUDICATED`
/// (`g9_assessment::ObservationDraft::validation_eligible == false`). Such
/// observations still count toward `Guided`/`Independent`; when one of them
/// would otherwise qualify as validation evidence and the state is not
/// `Validated`, the projection is flagged `provisional`.
pub fn derive_state_excluding_validation(
    competency: &Competency,
    observations: &[CompetencyObservation],
    rules: &DerivationRules,
    now: UtcMillis,
    validation_ineligible: &BTreeSet<ObservationId>,
) -> CompetencyProjection {
    // Validation evidence must trace to a graded attempt (ADR-0005/0012): an
    // observation of any other origin never validates, whatever mode it
    // carries; an operator correction validates only through the attempt it
    // supersedes.
    let by_id: BTreeMap<&ObservationId, &CompetencyObservation> =
        observations.iter().map(|o| (&o.id, o)).collect();
    let validates = |o: &CompetencyObservation| {
        qualifies_validation(o, rules)
            && is_attempt_rooted(o, &by_id)
            && !validation_ineligible.contains(&o.id)
    };
    let retained = retained_for(&competency.id, observations);
    let validity_ms = days_to_ms(rules.validity_window_days_for(competency));
    let window_ms = days_to_ms(rules.independent_window_days);

    let mut state = CompetencyState::Unassessed;
    // Evidence at index >= floor counts toward promotions; a Regress moves the
    // floor forward so that pre-regression highs cannot immediately re-promote.
    let mut floor: usize = 0;
    let mut latest_validation_at: Option<UtcMillis> = None;

    for (i, obs) in retained.iter().enumerate() {
        let t = obs.observed_at;
        state = apply_retest_due(state, latest_validation_at, validity_ms, t);

        if state == CompetencyState::Unassessed {
            state = step(state, CompetencyEvent::Introduce);
        }

        if completes_occasion(&retained, i)
            && is_regress_trigger(&retained[..=i], rules)
            && state.can(CompetencyEvent::Regress)
        {
            state = step(state, CompetencyEvent::Regress);
            floor = i + 1;
        }

        let scope = &retained[floor..=i];
        if state.can(CompetencyEvent::DemonstrateGuided)
            && scope.iter().any(|o| qualifies_guided(o, rules))
        {
            state = step(state, CompetencyEvent::DemonstrateGuided);
        }
        if state.can(CompetencyEvent::DemonstrateIndependent) {
            let count = scope
                .iter()
                .filter(|o| qualifies_independent(o, rules) && within(o.observed_at, t, window_ms))
                .count();
            if count >= usize::from(rules.independent_min_count) {
                state = step(state, CompetencyEvent::DemonstrateIndependent);
            }
        }
        if state.can(CompetencyEvent::Validate)
            && scope
                .iter()
                .any(|o| validates(o) && within(o.observed_at, t, validity_ms))
        {
            state = step(state, CompetencyEvent::Validate);
        }

        if validates(obs) {
            latest_validation_at = Some(t);
        }
    }

    state = apply_retest_due(state, latest_validation_at, validity_ms, now);
    let provisional = state != CompetencyState::Validated
        && retained.iter().any(|o| {
            qualifies_validation(o, rules)
                && is_attempt_rooted(o, &by_id)
                && validation_ineligible.contains(&o.id)
        });

    CompetencyProjection {
        competency_id: competency.id.clone(),
        state,
        per_dimension: per_dimension(&retained),
        contradictions: contradictions(&retained, rules),
        evidence_refs: retained.iter().map(|o| o.id.clone()).collect(),
        derived_at: now,
        rules_hash: rules.hash(),
        provisional,
    }
}

/// Validating variant of [`derive_state`]: rejects invalid rules or
/// competency definitions, observations of another competency, observations
/// that cannot affect state (untrusted provenance, score out of range) and
/// `supersedes` references that do not resolve inside `observations`.
pub fn derive_state_checked(
    competency: &Competency,
    observations: &[CompetencyObservation],
    rules: &DerivationRules,
    now: UtcMillis,
) -> Result<CompetencyProjection, CompetencyError> {
    competency.validate()?;
    rules.validate()?;
    for obs in observations {
        obs.validate()?;
        if obs.competency_id != competency.id {
            return Err(CompetencyError::InvalidObservation {
                reason: format!(
                    "observation {} belongs to competency {}, not {}",
                    obs.id, obs.competency_id, competency.id
                ),
            });
        }
        if let Some(target_id) = &obs.supersedes {
            let target = observations
                .iter()
                .find(|o| &o.id == target_id)
                .ok_or_else(|| CompetencyError::SupersedesUnknown {
                    id: target_id.to_string(),
                })?;
            if target.observed_at > obs.observed_at {
                return Err(CompetencyError::SupersedesInvalid {
                    id: target_id.to_string(),
                    reason: "it was observed later than the correction".to_string(),
                });
            }
        }
    }
    let mut ids: Vec<&ObservationId> = observations.iter().map(|o| &o.id).collect();
    ids.sort();
    if ids.windows(2).any(|w| w[0] == w[1]) {
        return Err(CompetencyError::InvalidObservation {
            reason: "duplicate observation ids in input".to_string(),
        });
    }
    Ok(derive_state(competency, observations, rules, now))
}

fn days_to_ms(days: u32) -> i64 {
    i64::from(days).saturating_mul(DAY_MS)
}

/// `earlier` is within `window_ms` before (or at) `later`.
fn within(earlier: UtcMillis, later: UtcMillis, window_ms: i64) -> bool {
    let elapsed = later.as_millis().saturating_sub(earlier.as_millis());
    (0..=window_ms).contains(&elapsed)
}

/// Apply a transition that the caller has already checked with `can`; if the
/// machine refuses anyway the state is left unchanged (never panics).
fn step(state: CompetencyState, event: CompetencyEvent) -> CompetencyState {
    state.transition(event).unwrap_or(state)
}

fn apply_retest_due(
    state: CompetencyState,
    latest_validation_at: Option<UtcMillis>,
    validity_ms: i64,
    at: UtcMillis,
) -> CompetencyState {
    if state != CompetencyState::Validated {
        return state;
    }
    match latest_validation_at {
        Some(v) if !within(v, at, validity_ms) && at > v => step(state, CompetencyEvent::RetestDue),
        _ => state,
    }
}

fn is_performance_dimension(d: LearningDimension) -> bool {
    matches!(
        d,
        LearningDimension::Application | LearningDimension::Transfer
    )
}

/// Assisted or unaided Application performance above the guided threshold.
/// `Shown` carries no demonstration by the learner and never qualifies.
fn qualifies_guided(o: &CompetencyObservation, rules: &DerivationRules) -> bool {
    o.dimension == LearningDimension::Application
        && o.assistance != AssistanceLevel::Shown
        && o.score >= rules.guided_min_score
}

/// Unaided Application performance at or above the independent threshold and difficulty.
fn qualifies_independent(o: &CompetencyObservation, rules: &DerivationRules) -> bool {
    o.dimension == LearningDimension::Application
        && o.is_unaided()
        && o.score >= rules.independent_min_score
        && o.difficulty.get() >= rules.independent_min_difficulty
}

/// Whether `o` is a graded attempt, or an operator correction whose chain of
/// `supersedes` links ends at one. Episodes, review attempts and corrections
/// of such records are not graded attempts. The walk is bounded by the number
/// of observations, so a malformed supersede cycle terminates as `false`.
fn is_attempt_rooted(
    o: &CompetencyObservation,
    by_id: &BTreeMap<&ObservationId, &CompetencyObservation>,
) -> bool {
    let mut current = o;
    for _ in 0..=by_id.len() {
        match &current.source {
            ObservationSource::Attempt(_) => return true,
            ObservationSource::ManualCorrection => {
                match current.supersedes.as_ref().and_then(|id| by_id.get(id)) {
                    Some(target) => current = target,
                    None => return false,
                }
            }
            ObservationSource::Episode(_) | ObservationSource::ReviewAttempt(_) => return false,
        }
    }
    false
}

/// Blind-exam evidence: EXAM mode, unaided, Application/Transfer, at or above the validated threshold.
/// A necessary condition only: [`derive_state_excluding_validation`] also
/// requires the observation to trace to a graded attempt ([`is_attempt_rooted`]).
fn qualifies_validation(o: &CompetencyObservation, rules: &DerivationRules) -> bool {
    o.is_validation_evidence() && o.score >= rules.validated_min_score
}

/// Identity of the occasion an observation belongs to: one graded attempt
/// writes one observation per dimension, but it is one result. A correction is
/// its own occasion (it replaces the observation it supersedes).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Occasion<'a> {
    Attempt(&'a g9_domain::AttemptId),
    Episode(&'a g9_domain::EpisodeId),
    Review(&'a g9_domain::ReviewItemId),
    Observation(&'a ObservationId),
}

fn occasion_of(o: &CompetencyObservation) -> Occasion<'_> {
    match &o.source {
        ObservationSource::Attempt(a) => Occasion::Attempt(a),
        ObservationSource::Episode(e) => Occasion::Episode(e),
        ObservationSource::ReviewAttempt(r) => Occasion::Review(r),
        ObservationSource::ManualCorrection => Occasion::Observation(&o.id),
    }
}

/// Whether `retained[i]` is the last unaided performance observation of its
/// occasion. Regress is judged only when an occasion is complete: an attempt
/// records its dimensions one after another, and the first of them must not
/// decide the occasion before the others are seen.
fn completes_occasion(retained: &[CompetencyObservation], i: usize) -> bool {
    let key = occasion_of(&retained[i]);
    !retained[i + 1..]
        .iter()
        .any(|o| o.is_unaided() && is_performance_dimension(o.dimension) && occasion_of(o) == key)
}

/// The newest observation is an unaided performance-dimension result below the
/// independent threshold, and so are the `regress_window_count - 1` most recent
/// other unaided performance **occasions** (ADR-0005: "two recent sub-threshold
/// unaided results"). Several observations of one occasion (an attempt records
/// an Application and a Transfer observation) are one result, and an occasion
/// is sub-threshold only when every one of its unaided performance observations
/// is, so a single failing attempt cannot regress a competency by itself and a
/// pass on one dimension is not outvoted by a failure on another.
fn is_regress_trigger(upto: &[CompetencyObservation], rules: &DerivationRules) -> bool {
    let Some(newest) = upto.last() else {
        return false;
    };
    if !(newest.is_unaided() && is_performance_dimension(newest.dimension)) {
        return false;
    }
    let window = usize::from(rules.regress_window_count);
    // Occasions from newest to oldest, each with "every observation so far is below".
    let mut occasions: Vec<(Occasion<'_>, bool)> = Vec::new();
    for o in upto
        .iter()
        .rev()
        .filter(|o| o.is_unaided() && is_performance_dimension(o.dimension))
    {
        let below = o.score < rules.independent_min_score;
        let key = occasion_of(o);
        match occasions.iter_mut().find(|(k, _)| *k == key) {
            Some((_, all_below)) => *all_below &= below,
            None => {
                if occasions.len() == window {
                    break;
                }
                occasions.push((key, below));
            }
        }
    }
    occasions.len() == window && occasions.iter().all(|(_, below)| *below)
}

fn per_dimension(
    retained: &[CompetencyObservation],
) -> BTreeMap<LearningDimension, DimensionEstimate> {
    struct Acc {
        n_total: u32,
        n_unaided: u32,
        latest_score: u8,
        latest_uncertainty: Confidence,
        weighted_sum: f64,
        weight_sum: f64,
        uncertainty_sum: f64,
        last_observed_at: UtcMillis,
    }
    let mut acc: BTreeMap<LearningDimension, Acc> = BTreeMap::new();
    for o in retained {
        let w = o.assistance.independence_weight();
        let entry = acc.entry(o.dimension).or_insert(Acc {
            n_total: 0,
            n_unaided: 0,
            latest_score: o.score,
            latest_uncertainty: o.uncertainty,
            weighted_sum: 0.0,
            weight_sum: 0.0,
            uncertainty_sum: 0.0,
            last_observed_at: o.observed_at,
        });
        entry.n_total = entry.n_total.saturating_add(1);
        if o.is_unaided() {
            entry.n_unaided = entry.n_unaided.saturating_add(1);
        }
        entry.latest_score = o.score;
        entry.latest_uncertainty = o.uncertainty;
        entry.weighted_sum += w * f64::from(o.score);
        entry.weight_sum += w;
        entry.uncertainty_sum += o.uncertainty.get();
        entry.last_observed_at = o.observed_at;
    }
    acc.into_iter()
        .map(|(d, a)| {
            let mean_uncertainty =
                (a.uncertainty_sum / f64::from(a.n_total.max(1))).clamp(0.0, 1.0);
            let uncertainty = Confidence::new(mean_uncertainty).unwrap_or(a.latest_uncertainty);
            let independence_weighted_mean = if a.weight_sum > 0.0 {
                Some(a.weighted_sum / a.weight_sum)
            } else {
                None
            };
            (
                d,
                DimensionEstimate {
                    n_total: a.n_total,
                    n_unaided: a.n_unaided,
                    latest_score: a.latest_score,
                    independence_weighted_mean,
                    last_observed_at: a.last_observed_at,
                    uncertainty,
                },
            )
        })
        .collect()
}

/// Pairwise contradiction rules over the canonically ordered retained set:
///
/// 1. A blind-exam unaided result at or above the validated threshold and an
///    assisted (hinted/guided/shown) result below the guided threshold in the
///    same dimension within the independent window, in either order.
/// 2. An unaided result at or above the independent threshold followed, within
///    the window, by an unaided result below the guided threshold in the same
///    dimension at equal or lower difficulty.
fn contradictions(
    retained: &[CompetencyObservation],
    rules: &DerivationRules,
) -> Vec<Contradiction> {
    let window_ms = days_to_ms(rules.independent_window_days);
    let mut out = Vec::new();
    for i in 0..retained.len() {
        for j in (i + 1)..retained.len() {
            let (older, newer) = (&retained[i], &retained[j]);
            if older.dimension != newer.dimension
                || !within(older.observed_at, newer.observed_at, window_ms)
            {
                continue;
            }
            if let Some(reason) = exam_vs_assisted(older, newer, rules)
                .or_else(|| unaided_collapse(older, newer, rules))
            {
                out.push(Contradiction {
                    older: older.id.clone(),
                    newer: newer.id.clone(),
                    reason,
                });
            }
        }
    }
    out
}

fn is_assisted(o: &CompetencyObservation) -> bool {
    matches!(
        o.assistance,
        AssistanceLevel::Hinted | AssistanceLevel::Guided | AssistanceLevel::Shown
    )
}

fn exam_vs_assisted(
    a: &CompetencyObservation,
    b: &CompetencyObservation,
    rules: &DerivationRules,
) -> Option<String> {
    let pair = if qualifies_validation(a, rules)
        && is_assisted(b)
        && b.score < rules.guided_min_score
    {
        Some((a, b))
    } else if qualifies_validation(b, rules) && is_assisted(a) && a.score < rules.guided_min_score {
        Some((b, a))
    } else {
        None
    };
    pair.map(|(exam, assisted)| {
        format!(
            "EXAM unaided {:?} scored {} ({}) but {:?} {:?} scored {} ({}) within {} days",
            exam.dimension,
            exam.score,
            exam.id,
            assisted.assistance,
            assisted.dimension,
            assisted.score,
            assisted.id,
            rules.independent_window_days
        )
    })
}

fn unaided_collapse(
    older: &CompetencyObservation,
    newer: &CompetencyObservation,
    rules: &DerivationRules,
) -> Option<String> {
    let collapse = older.is_unaided()
        && newer.is_unaided()
        && older.score >= rules.independent_min_score
        && newer.score < rules.guided_min_score
        && newer.difficulty <= older.difficulty;
    collapse.then(|| {
        format!(
            "unaided {:?} fell from {} at difficulty {} ({}) to {} at difficulty {} ({}) within {} days",
            older.dimension,
            older.score,
            older.difficulty.get(),
            older.id,
            newer.score,
            newer.difficulty.get(),
            newer.id,
            rules.independent_window_days
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{competency, day, oid, Obs, DAY_MS, T0};
    use g9_domain::{InteractionMode, TrustClass};
    use pretty_assertions::assert_eq;
    use proptest::prelude::*;
    use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};

    fn rules() -> DerivationRules {
        DerivationRules::default()
    }

    fn independent_pair() -> Vec<CompetencyObservation> {
        vec![
            Obs::app("o1", "c1", 75, day(1)).build(),
            Obs::app("o2", "c1", 80, day(5)).build(),
        ]
    }

    #[test]
    fn fold_walks_the_state_machine_one_edge_at_a_time() {
        let c = competency("c1");
        let empty = derive_state(&c, &[], &rules(), day(10));
        assert_eq!(empty.state, CompetencyState::Unassessed);
        assert!(empty.evidence_refs.is_empty());
        assert!(empty.per_dimension.is_empty());

        let intro = vec![Obs::new("o0", "c1", LearningDimension::Recall, 40, day(0)).build()];
        assert_eq!(
            derive_state(&c, &intro, &rules(), day(1)).state,
            CompetencyState::Introduced
        );

        let guided = vec![Obs::app("o1", "c1", 65, day(1))
            .assistance(AssistanceLevel::Guided)
            .build()];
        assert_eq!(
            derive_state(&c, &guided, &rules(), day(2)).state,
            CompetencyState::Guided
        );

        let shown = vec![Obs::app("o1", "c1", 100, day(1))
            .assistance(AssistanceLevel::Shown)
            .build()];
        assert_eq!(
            derive_state(&c, &shown, &rules(), day(2)).state,
            CompetencyState::Introduced
        );

        let one_unaided = vec![Obs::app("o1", "c1", 90, day(1)).build()];
        assert_eq!(
            derive_state(&c, &one_unaided, &rules(), day(2)).state,
            CompetencyState::Guided
        );

        let p = derive_state(&c, &independent_pair(), &rules(), day(6));
        assert_eq!(p.state, CompetencyState::Independent);
        assert_eq!(p.evidence_refs, vec![oid("o1"), oid("o2")]);
        assert_eq!(p.rules_hash, rules().hash());
        assert_eq!(p.derived_at, day(6));

        // Too easy or too far apart: no Independent.
        let easy = vec![
            Obs::app("o1", "c1", 75, day(1)).difficulty(2).build(),
            Obs::app("o2", "c1", 80, day(5)).difficulty(2).build(),
        ];
        assert_eq!(
            derive_state(&c, &easy, &rules(), day(6)).state,
            CompetencyState::Guided
        );
        let far = vec![
            Obs::app("o1", "c1", 75, day(1)).build(),
            Obs::app("o2", "c1", 80, day(70)).build(),
        ];
        assert_eq!(
            derive_state(&c, &far, &rules(), day(71)).state,
            CompetencyState::Guided
        );
    }

    #[test]
    fn self_adjudicated_exam_evidence_caps_at_independent_with_provisional_flag() {
        // ADR-0005/0012, Addendum A item 2: evidence from a SELF_ADJUDICATED
        // grade decision never validates; the state stays Independent and the
        // projection is flagged provisional.
        let c = competency("c1");
        let mut obs = independent_pair();
        obs.push(
            Obs::app("exam", "c1", 95, day(6))
                .mode(InteractionMode::Exam)
                .build(),
        );
        let self_adjudicated: BTreeSet<ObservationId> = [oid("exam")].into_iter().collect();

        let open = derive_state(&c, &obs, &rules(), day(7));
        assert_eq!(open.state, CompetencyState::Validated);
        assert!(!open.provisional);

        let capped =
            derive_state_excluding_validation(&c, &obs, &rules(), day(7), &self_adjudicated);
        assert_eq!(capped.state, CompetencyState::Independent);
        assert!(capped.provisional);
        // The evidence is retained, never erased.
        assert!(capped.evidence_refs.contains(&oid("exam")));
        // An empty exclusion set is exactly `derive_state`.
        assert_eq!(
            derive_state_excluding_validation(&c, &obs, &rules(), day(7), &BTreeSet::new()),
            open
        );

        // Later eligible (externally reviewed or deterministic) evidence validates
        // and clears the flag.
        obs.push(
            Obs::new("exam-2", "c1", LearningDimension::Transfer, 85, day(8))
                .mode(InteractionMode::Exam)
                .build(),
        );
        let validated =
            derive_state_excluding_validation(&c, &obs, &rules(), day(9), &self_adjudicated);
        assert_eq!(validated.state, CompetencyState::Validated);
        assert!(!validated.provisional);

        // Ineligible evidence below the validation threshold does not flag.
        let low = vec![
            Obs::app("o1", "c1", 75, day(1)).build(),
            Obs::app("o2", "c1", 70, day(2))
                .mode(InteractionMode::Exam)
                .build(),
        ];
        let set: BTreeSet<ObservationId> = [oid("o2")].into_iter().collect();
        assert!(!derive_state_excluding_validation(&c, &low, &rules(), day(3), &set).provisional);

        // The flag round-trips and defaults to false for older snapshots.
        let json = serde_json::to_value(&capped).unwrap();
        assert_eq!(json["provisional"], serde_json::json!(true));
        let mut legacy = json.clone();
        legacy.as_object_mut().unwrap().remove("provisional");
        let back: CompetencyProjection = serde_json::from_value(legacy).unwrap();
        assert!(!back.provisional);
    }

    #[test]
    fn validated_requires_exam_mode_unaided() {
        let c = competency("c1");
        let mut obs = independent_pair();
        obs.push(Obs::app("o3", "c1", 95, day(6)).build()); // STUDY mode, unaided
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(7)).state,
            CompetencyState::Independent
        );

        obs.push(
            Obs::app("o4", "c1", 95, day(7))
                .mode(InteractionMode::Exam)
                .assistance(AssistanceLevel::Hinted)
                .build(),
        );
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(8)).state,
            CompetencyState::Independent
        );

        obs.push(
            Obs::new("o5", "c1", LearningDimension::Recall, 100, day(8))
                .mode(InteractionMode::Exam)
                .build(),
        );
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(9)).state,
            CompetencyState::Independent
        );

        obs.push(
            Obs::app("o6", "c1", 79, day(9))
                .mode(InteractionMode::Exam)
                .build(),
        );
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(10)).state,
            CompetencyState::Independent
        );

        obs.push(
            Obs::new("o7", "c1", LearningDimension::Transfer, 85, day(10))
                .mode(InteractionMode::Exam)
                .build(),
        );
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(11)).state,
            CompetencyState::Validated
        );

        // Validation evidence alone (no Independent yet) cannot jump to Validated.
        let exam_only = vec![Obs::app("e1", "c1", 95, day(1))
            .mode(InteractionMode::Exam)
            .build()];
        assert_eq!(
            derive_state(&c, &exam_only, &rules(), day(2)).state,
            CompetencyState::Guided
        );
        // ... but once a second unaided result makes it Independent, the earlier
        // exam evidence (still within validity) validates it.
        let exam_then_unaided = vec![
            Obs::app("e1", "c1", 95, day(1))
                .mode(InteractionMode::Exam)
                .build(),
            Obs::app("e2", "c1", 75, day(3)).build(),
        ];
        assert_eq!(
            derive_state(&c, &exam_then_unaided, &rules(), day(4)).state,
            CompetencyState::Validated
        );
    }

    #[test]
    fn one_failing_attempt_is_one_result_not_two_for_regress() {
        use crate::observation::ObservationSource;
        let c = competency("c1");
        let two_dims = |id: &str, app: u8, tr: u8, at| {
            let mut a = Obs::app(&format!("{id}a"), "c1", app, at).build();
            let mut t =
                Obs::new(&format!("{id}t"), "c1", LearningDimension::Transfer, tr, at).build();
            // Both observations come from one attempt.
            let attempt = g9_domain::AttemptId::parse(&format!("attempt-{id}")).unwrap();
            a.source = ObservationSource::Attempt(attempt.clone());
            t.source = ObservationSource::Attempt(attempt);
            vec![a, t]
        };
        // Independent through two labs ...
        let mut obs = independent_pair();
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(6)).state,
            CompetencyState::Independent
        );
        // ... then ONE failing attempt (Application 20 and Transfer 20): not a regression.
        obs.extend(two_dims("x", 20, 20, day(8)));
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(9)).state,
            CompetencyState::Independent,
            "one occasion is one result"
        );
        // A second failing occasion regresses.
        obs.extend(two_dims("y", 20, 20, day(10)));
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(11)).state,
            CompetencyState::Guided
        );
        // An occasion with a pass on one dimension is not sub-threshold.
        let mut mixed = independent_pair();
        mixed.extend(two_dims("x", 20, 20, day(8)));
        mixed.extend(two_dims("y", 20, 90, day(10)));
        assert_eq!(
            derive_state(&c, &mixed, &rules(), day(11)).state,
            CompetencyState::Independent,
            "a pass on Transfer means the occasion is not a failure"
        );
    }

    #[test]
    fn validation_evidence_must_trace_to_a_graded_attempt() {
        use crate::observation::ObservationSource;
        use g9_domain::EpisodeId;
        let c = competency("c1");

        // Two unaided results make the competency Independent; the third is an
        // EXAM-mode, unaided, 95-point Application observation of a given origin.
        let with_exam = |exam: CompetencyObservation| {
            let mut obs = independent_pair();
            obs.push(exam);
            derive_state(&c, &obs, &rules(), day(11))
        };

        // Control: a graded attempt validates.
        let attempt = Obs::app("x1", "c1", 95, day(10))
            .mode(InteractionMode::Exam)
            .build();
        assert_eq!(with_exam(attempt).state, CompetencyState::Validated);

        // An episode that merely carries mode=EXAM never validates.
        let mut episode = Obs::app("x1", "c1", 95, day(10))
            .mode(InteractionMode::Exam)
            .build();
        episode.source = ObservationSource::Episode(EpisodeId::parse("ep-1").unwrap());
        let p = with_exam(episode);
        assert_eq!(p.state, CompetencyState::Independent);
        assert!(
            !p.provisional,
            "not validation evidence, so not provisional"
        );

        // A correction that supersedes an episode never validates either ...
        let mut ep = Obs::app("e1", "c1", 60, day(8)).build();
        ep.source = ObservationSource::Episode(EpisodeId::parse("ep-2").unwrap());
        let mut obs = independent_pair();
        obs.push(ep);
        obs.push(
            Obs::app("x2", "c1", 95, day(10))
                .mode(InteractionMode::Exam)
                .supersedes("e1")
                .build(),
        );
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(11)).state,
            CompetencyState::Independent
        );

        // ... a correction with no resolvable target never validates ...
        let mut obs = independent_pair();
        obs.push(
            Obs::app("x3", "c1", 95, day(10))
                .mode(InteractionMode::Exam)
                .supersedes("ghost")
                .build(),
        );
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(11)).state,
            CompetencyState::Independent
        );

        // ... and a supersede cycle terminates without validating.
        let mut a = Obs::app("c-a", "c1", 95, day(10))
            .mode(InteractionMode::Exam)
            .supersedes("c-b")
            .build();
        let mut b = Obs::app("c-b", "c1", 95, day(10))
            .mode(InteractionMode::Exam)
            .supersedes("c-a")
            .build();
        a.observed_at = day(10);
        b.observed_at = day(10);
        let mut obs = independent_pair();
        obs.push(a);
        obs.push(b);
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(11)).state,
            CompetencyState::Independent
        );
    }

    #[test]
    fn an_operator_correction_of_a_graded_exam_attempt_keeps_its_validation_role() {
        let c = competency("c1");
        let mut obs = independent_pair();
        obs.push(
            Obs::app("x1", "c1", 70, day(10))
                .mode(InteractionMode::Exam)
                .build(),
        );
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(11)).state,
            CompetencyState::Independent,
            "70 is below the validated threshold"
        );
        // The regrade is a MANUAL_CORRECTION of the attempt-sourced observation.
        let correction = Obs::app("x2", "c1", 88, day(10))
            .mode(InteractionMode::Exam)
            .supersedes("x1")
            .build();
        correction.validate_against(&obs).unwrap();
        obs.push(correction);
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(11)).state,
            CompetencyState::Validated
        );
    }

    #[test]
    fn retest_due_after_validity_window() {
        let c = competency("c1"); // validity 90 days
        let mut obs = independent_pair();
        obs.push(
            Obs::app("o3", "c1", 90, day(10))
                .mode(InteractionMode::Exam)
                .build(),
        );
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(100)).state,
            CompetencyState::Validated
        );
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(101)).state,
            CompetencyState::RetestDue
        );

        // A rules override shortens the window.
        let mut short = rules();
        short.validity_window_days = Some(30);
        assert_eq!(
            derive_state(&c, &obs, &short, day(41)).state,
            CompetencyState::RetestDue
        );
        assert_eq!(
            derive_state(&c, &obs, &short, day(40)).state,
            CompetencyState::Validated
        );

        // RetestDue is applied during replay too, and a fresh exam re-validates.
        obs.push(
            Obs::app("o4", "c1", 60, day(120))
                .assistance(AssistanceLevel::Guided)
                .build(),
        );
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(121)).state,
            CompetencyState::RetestDue
        );
        obs.push(
            Obs::app("o5", "c1", 88, day(130))
                .mode(InteractionMode::Exam)
                .build(),
        );
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(131)).state,
            CompetencyState::Validated
        );
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(221)).state,
            CompetencyState::RetestDue
        );
    }

    #[test]
    fn a05_correction_supersedes_and_changes_state() {
        let c = competency("c1");
        let mut obs = independent_pair();
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(6)).state,
            CompetencyState::Independent
        );

        let correction = Obs::app("o3", "c1", 40, day(5)).supersedes("o2").build();
        correction.validate_against(&obs).unwrap();
        obs.push(correction);

        let p = derive_state(&c, &obs, &rules(), day(6));
        assert_eq!(p.state, CompetencyState::Guided);
        assert_eq!(
            p.evidence_refs,
            vec![oid("o1"), oid("o3")],
            "superseded o2 is excluded"
        );
        let app = p.dimension(LearningDimension::Application).unwrap();
        assert_eq!(app.n_total, 2);
        assert_eq!(app.latest_score, 40);

        // A correction upward restores the state.
        obs.push(Obs::app("o4", "c1", 85, day(5)).supersedes("o3").build());
        let p = derive_state(&c, &obs, &rules(), day(6));
        assert_eq!(p.state, CompetencyState::Independent);
        assert_eq!(p.evidence_refs, vec![oid("o1"), oid("o4")]);

        let checked = derive_state_checked(&c, &obs, &rules(), day(6)).unwrap();
        assert_eq!(checked, p);
    }

    #[test]
    fn a05_regress_keeps_earlier_evidence() {
        let c = competency("c1");
        let mut obs = independent_pair();
        obs.push(Obs::app("o3", "c1", 50, day(8)).build());
        let p = derive_state(&c, &obs, &rules(), day(9));
        assert_eq!(
            p.state,
            CompetencyState::Independent,
            "one low result is not a regression"
        );

        obs.push(Obs::app("o4", "c1", 45, day(9)).build());
        let p = derive_state(&c, &obs, &rules(), day(10));
        assert_eq!(p.state, CompetencyState::Guided);
        assert_eq!(
            p.evidence_refs,
            vec![oid("o1"), oid("o2"), oid("o3"), oid("o4")]
        );
        let app = p.dimension(LearningDimension::Application).unwrap();
        assert_eq!(app.n_total, 4);
        assert_eq!(app.n_unaided, 4);
        assert_eq!(app.latest_score, 45);
        assert_eq!(app.independence_weighted_mean, Some(62.5));
        assert_eq!(app.last_observed_at, day(9));

        // Pre-regression highs do not re-promote; new evidence does.
        obs.push(Obs::app("o5", "c1", 80, day(12)).build());
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(13)).state,
            CompetencyState::Guided
        );
        obs.push(Obs::app("o6", "c1", 82, day(14)).build());
        assert_eq!(
            derive_state(&c, &obs, &rules(), day(15)).state,
            CompetencyState::Independent
        );

        // Validated regresses to Independent, then to Guided on continued failure.
        let mut v = independent_pair();
        v.push(
            Obs::app("v1", "c1", 90, day(10))
                .mode(InteractionMode::Exam)
                .build(),
        );
        v.push(Obs::app("v2", "c1", 30, day(11)).build());
        v.push(Obs::app("v3", "c1", 35, day(12)).build());
        let p = derive_state(&c, &v, &rules(), day(13));
        assert_eq!(p.state, CompetencyState::Independent);
        assert_eq!(p.evidence_refs.len(), 5);
        v.push(Obs::app("v4", "c1", 20, day(13)).build());
        assert_eq!(
            derive_state(&c, &v, &rules(), day(14)).state,
            CompetencyState::Guided
        );

        // Unaided recall failures (retention reviews) do not regress practical state.
        let mut r = independent_pair();
        r.push(Obs::new("r1", "c1", LearningDimension::Recall, 0, day(8)).build());
        r.push(Obs::new("r2", "c1", LearningDimension::Recall, 0, day(9)).build());
        assert_eq!(
            derive_state(&c, &r, &rules(), day(10)).state,
            CompetencyState::Independent
        );
    }

    #[test]
    fn a05_contradiction_detected_between_exam_and_guided_results() {
        let c = competency("c1");
        let mut obs = independent_pair();
        obs.push(
            Obs::app("x1", "c1", 92, day(10))
                .mode(InteractionMode::Exam)
                .build(),
        );
        obs.push(
            Obs::app("g1", "c1", 30, day(15))
                .assistance(AssistanceLevel::Guided)
                .build(),
        );
        let p = derive_state(&c, &obs, &rules(), day(16));
        assert_eq!(p.contradictions.len(), 1, "{:?}", p.contradictions);
        let cx = &p.contradictions[0];
        assert_eq!(cx.older, oid("x1"));
        assert_eq!(cx.newer, oid("g1"));
        assert!(cx.reason.contains("EXAM"), "{}", cx.reason);
        assert!(cx.reason.contains("Guided"), "{}", cx.reason);
        // State is unchanged by the contradiction (it is reported, not resolved).
        assert_eq!(p.state, CompetencyState::Validated);

        // Reverse order (guided failure, then exam success) is also flagged.
        let rev = vec![
            Obs::app("g1", "c1", 30, day(1))
                .assistance(AssistanceLevel::Guided)
                .build(),
            Obs::app("x1", "c1", 92, day(5))
                .mode(InteractionMode::Exam)
                .build(),
        ];
        let p = derive_state(&c, &rev, &rules(), day(6));
        assert_eq!(p.contradictions.len(), 1);
        assert_eq!(
            (
                p.contradictions[0].older.clone(),
                p.contradictions[0].newer.clone()
            ),
            (oid("g1"), oid("x1"))
        );

        // Outside the window: no contradiction.
        let far = vec![
            Obs::app("x1", "c1", 92, day(1))
                .mode(InteractionMode::Exam)
                .build(),
            Obs::app("g1", "c1", 30, day(70))
                .assistance(AssistanceLevel::Guided)
                .build(),
        ];
        assert!(derive_state(&c, &far, &rules(), day(71))
            .contradictions
            .is_empty());

        // Different dimension: no contradiction.
        let other_dim = vec![
            Obs::app("x1", "c1", 92, day(1))
                .mode(InteractionMode::Exam)
                .build(),
            Obs::new("g1", "c1", LearningDimension::Recall, 30, day(2))
                .assistance(AssistanceLevel::Guided)
                .build(),
        ];
        assert!(derive_state(&c, &other_dim, &rules(), day(3))
            .contradictions
            .is_empty());
    }

    #[test]
    fn unaided_collapse_is_a_contradiction() {
        let c = competency("c1");
        let obs = vec![
            Obs::app("o1", "c1", 85, day(1)).difficulty(4).build(),
            Obs::app("o2", "c1", 20, day(3)).difficulty(3).build(),
        ];
        let p = derive_state(&c, &obs, &rules(), day(4));
        assert_eq!(p.contradictions.len(), 1);
        assert!(p.contradictions[0].reason.contains("fell from 85"));
        // Harder task later: not a contradiction.
        let harder = vec![
            Obs::app("o1", "c1", 85, day(1)).difficulty(3).build(),
            Obs::app("o2", "c1", 20, day(3)).difficulty(5).build(),
        ];
        assert!(derive_state(&c, &harder, &rules(), day(4))
            .contradictions
            .is_empty());
    }

    #[test]
    fn per_dimension_estimates_weight_by_independence_and_average_uncertainty() {
        let c = competency("c1");
        let obs = vec![
            Obs::app("o1", "c1", 80, day(1)).uncertainty(0.2).build(),
            Obs::app("o2", "c1", 40, day(2))
                .assistance(AssistanceLevel::Hinted)
                .uncertainty(0.4)
                .build(),
            Obs::app("o3", "c1", 100, day(3))
                .assistance(AssistanceLevel::Shown)
                .uncertainty(0.0)
                .build(),
            Obs::new("o4", "c1", LearningDimension::Recall, 90, day(4))
                .assistance(AssistanceLevel::Guided)
                .build(),
        ];
        let p = derive_state(&c, &obs, &rules(), day(5));
        let app = p.dimension(LearningDimension::Application).unwrap();
        assert_eq!(app.n_total, 3);
        assert_eq!(app.n_unaided, 1);
        assert_eq!(app.latest_score, 100);
        // (1.0*80 + 0.5*40 + 0.0*100) / 1.5 = 66.666...
        let mean = app.independence_weighted_mean.unwrap();
        assert!((mean - 200.0 / 3.0).abs() < 1e-9, "{mean}");
        assert!((app.uncertainty.get() - 0.2).abs() < 1e-9);
        let recall = p.dimension(LearningDimension::Recall).unwrap();
        assert_eq!(recall.independence_weighted_mean, None);
        assert_eq!(recall.n_unaided, 0);
        assert!(p.dimension(LearningDimension::Transfer).is_none());
    }

    #[test]
    fn untrusted_and_foreign_observations_never_affect_state() {
        let c = competency("c1");
        let obs = vec![
            Obs::app("m1", "c1", 100, day(1))
                .trust(TrustClass::ModelOutput)
                .build(),
            Obs::app("m2", "c1", 100, day(2))
                .trust(TrustClass::ModelOutput)
                .build(),
            Obs::app("f1", "c2", 100, day(3)).build(),
        ];
        let p = derive_state(&c, &obs, &rules(), day(4));
        assert_eq!(p.state, CompetencyState::Unassessed);
        assert!(p.evidence_refs.is_empty());
        let err = derive_state_checked(&c, &obs, &rules(), day(4)).unwrap_err();
        assert!(matches!(err, CompetencyError::UntrustedProvenance { .. }));
        let foreign = vec![Obs::app("f1", "c2", 100, day(3)).build()];
        assert!(matches!(
            derive_state_checked(&c, &foreign, &rules(), day(4)).unwrap_err(),
            CompetencyError::InvalidObservation { .. }
        ));
        let dangling = vec![Obs::app("d1", "c1", 50, day(3)).supersedes("ghost").build()];
        assert!(matches!(
            derive_state_checked(&c, &dangling, &rules(), day(4)).unwrap_err(),
            CompetencyError::SupersedesUnknown { .. }
        ));
    }

    #[test]
    fn projection_serde_round_trip() {
        let c = competency("c1");
        let mut obs = independent_pair();
        obs.push(
            Obs::app("x1", "c1", 92, day(10))
                .mode(InteractionMode::Exam)
                .build(),
        );
        obs.push(
            Obs::app("g1", "c1", 30, day(15))
                .assistance(AssistanceLevel::Guided)
                .build(),
        );
        let p = derive_state(&c, &obs, &rules(), day(16));
        let json = serde_json::to_string(&p).unwrap();
        let back: CompetencyProjection = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
        assert_eq!(serde_json::to_value(&p).unwrap()["state"], "VALIDATED");
    }

    // ---- property tests (A05) -------------------------------------------------

    fn arb_observation(index: usize) -> impl Strategy<Value = CompetencyObservation> {
        let dims = prop_oneof![
            Just(LearningDimension::Recall),
            Just(LearningDimension::Understanding),
            Just(LearningDimension::Application),
            Just(LearningDimension::Transfer),
        ];
        let assist = prop_oneof![
            Just(AssistanceLevel::Unaided),
            Just(AssistanceLevel::Unaided),
            Just(AssistanceLevel::Prompted),
            Just(AssistanceLevel::Hinted),
            Just(AssistanceLevel::Guided),
            Just(AssistanceLevel::Shown),
        ];
        let modes = prop_oneof![
            Just(InteractionMode::Study),
            Just(InteractionMode::Lab),
            Just(InteractionMode::Exam),
            Just(InteractionMode::Review),
        ];
        let trust = prop_oneof![
            4 => Just(TrustClass::SystemDeterministic),
            2 => Just(TrustClass::Operator),
            1 => Just(TrustClass::ModelOutput),
        ];
        let supersedes = if index == 0 {
            Just(None).boxed()
        } else {
            prop_oneof![3 => Just(None), 1 => (0..index).prop_map(Some)].boxed()
        };
        (
            dims,
            assist,
            modes,
            1u8..=5,
            0u8..=100,
            0i64..200,
            0u8..=2,
            trust,
            supersedes,
            0u8..=7,
        )
            .prop_map(
                move |(
                    dimension,
                    assistance,
                    mode,
                    difficulty,
                    score,
                    day_offset,
                    unc,
                    trust,
                    sup,
                    comp,
                )| {
                    let at = UtcMillis::from_millis(T0.as_millis() + day_offset * DAY_MS);
                    let competency = if comp == 0 { "c2" } else { "c1" };
                    let mut b = Obs::new(&format!("o{index:02}"), competency, dimension, score, at)
                        .assistance(assistance)
                        .mode(mode)
                        .difficulty(difficulty)
                        .uncertainty(f64::from(unc) * 0.25)
                        .trust(trust)
                        .build();
                    b.supersedes = sup.map(|i| oid(&format!("o{i:02}")));
                    b
                },
            )
    }

    fn arb_history() -> impl Strategy<Value = Vec<CompetencyObservation>> {
        (0usize..=12).prop_flat_map(|n| (0..n).map(arb_observation).collect::<Vec<_>>())
    }

    #[test]
    fn a05_projection_is_replayable_and_order_independent() {
        let config = Config {
            cases: 256,
            failure_persistence: None,
            ..Config::default()
        };
        let seed = [7u8; 32];
        let mut runner =
            TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::ChaCha, &seed));
        let c = competency("c1");
        let r = rules();
        let strategy = arb_history().prop_flat_map(|history| {
            let shuffled = Just(history.clone()).prop_shuffle();
            (Just(history), shuffled)
        });
        runner
            .run(&strategy, |(history, shuffled)| {
                let now = day(250);
                let a = derive_state(&c, &history, &r, now);
                let b = derive_state(&c, &shuffled, &r, now);
                let replay = derive_state(&c, &history, &r, now);
                prop_assert_eq!(&a, &b);
                prop_assert_eq!(&a, &replay);
                // Invariants that hold for every history.
                let ids: Vec<&ObservationId> = a.evidence_refs.iter().collect();
                let mut sorted = ids.clone();
                sorted.dedup();
                prop_assert_eq!(ids.len(), sorted.len(), "no duplicate evidence refs");
                for id in &a.evidence_refs {
                    let o = history.iter().find(|o| &o.id == id).unwrap();
                    prop_assert!(o.is_authoritative());
                    prop_assert_eq!(&o.competency_id, &c.id);
                    prop_assert!(!history.iter().any(|h| h.is_authoritative()
                        && h.competency_id == c.id
                        && h.supersedes.as_ref() == Some(id)));
                }
                for est in a.per_dimension.values() {
                    prop_assert!(est.n_unaided <= est.n_total);
                    prop_assert!(est.n_total >= 1);
                }
                if a.state == CompetencyState::Validated || a.state == CompetencyState::RetestDue {
                    prop_assert!(history.iter().any(|o| o.is_authoritative()
                        && o.is_validation_evidence()
                        && o.score >= r.validated_min_score));
                }
                prop_assert_eq!(&a.rules_hash, &r.hash());
                Ok(())
            })
            .unwrap();
    }
}
