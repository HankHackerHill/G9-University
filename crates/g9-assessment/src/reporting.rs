//! First-attempt versus eventual pass rates (ARCHITECTURE.md Addendum A
//! item 3; LRN-13: "KPIs report first_attempt_pass_rate and
//! eventual_pass_rate separately").
//!
//! Attempts are grouped into retake chains by following `retake_of` to the
//! original. Only effective final decisions count: a decision superseded by
//! another decision in the input is ignored; when an attempt still has
//! several, the latest `(decided_at, id)` wins.

use crate::attempt::Attempt;
use crate::grading::GradeDecision;
use g9_domain::{AttemptId, GradeDecisionId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Pass rates over retake chains.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PassRates {
    /// Chains whose original attempt has an effective final decision.
    pub first_attempts_decided: u32,
    /// Of those, chains whose original attempt passed.
    pub first_attempt_passes: u32,
    /// Chains with at least one effective final decision on any attempt.
    pub chains_decided: u32,
    /// Of those, chains in which some attempt passed.
    pub eventual_passes: u32,
    /// Retake attempts with an effective final decision.
    pub retakes_decided: u32,
    /// `first_attempt_passes / first_attempts_decided`; `None` without data.
    pub first_attempt_pass_rate: Option<f64>,
    /// `eventual_passes / chains_decided`; `None` without data.
    pub eventual_pass_rate: Option<f64>,
}

fn rate(num: u32, den: u32) -> Option<f64> {
    (den > 0).then(|| f64::from(num) / f64::from(den))
}

/// Root (original) of an attempt's retake chain. Cycles and missing links stop
/// at the last attempt reached.
fn root_of(id: &AttemptId, parent: &BTreeMap<&AttemptId, &AttemptId>) -> AttemptId {
    let mut current = id;
    let mut seen = BTreeSet::new();
    while let Some(p) = parent.get(current) {
        if !seen.insert(current) {
            break;
        }
        current = p;
    }
    current.clone()
}

/// Compute first-attempt and eventual pass rates.
pub fn pass_rates(attempts: &[Attempt], decisions: &[GradeDecision]) -> PassRates {
    let parent: BTreeMap<&AttemptId, &AttemptId> = attempts
        .iter()
        .filter_map(|a| a.retake_of().map(|p| (a.id(), p)))
        .collect();
    let superseded: BTreeSet<&GradeDecisionId> = decisions
        .iter()
        .filter_map(|d| d.supersedes.as_ref())
        .collect();
    let mut effective: BTreeMap<&AttemptId, &GradeDecision> = BTreeMap::new();
    for d in decisions
        .iter()
        .filter(|d| d.is_final && !superseded.contains(&d.id))
    {
        let newer = effective
            .get(&d.attempt_id)
            .is_none_or(|prev| (d.decided_at, &d.id) > (prev.decided_at, &prev.id));
        if newer {
            effective.insert(&d.attempt_id, d);
        }
    }

    // chain root -> (original decided?, original passed?, any decided, any passed)
    let mut chains: BTreeMap<AttemptId, (bool, bool, bool, bool)> = BTreeMap::new();
    let mut retakes_decided = 0u32;
    for a in attempts {
        let root = root_of(a.id(), &parent);
        let entry = chains.entry(root.clone()).or_default();
        let Some(d) = effective.get(a.id()) else {
            continue;
        };
        entry.2 = true;
        entry.3 |= d.pass;
        if *a.id() == root {
            entry.0 = true;
            entry.1 = d.pass;
        } else {
            retakes_decided += 1;
        }
    }
    let count = |f: fn(&(bool, bool, bool, bool)) -> bool| -> u32 {
        u32::try_from(chains.values().filter(|c| f(c)).count()).unwrap_or(u32::MAX)
    };
    let first_attempts_decided = count(|c| c.0);
    let first_attempt_passes = count(|c| c.0 && c.1);
    let chains_decided = count(|c| c.2);
    let eventual_passes = count(|c| c.2 && c.3);
    PassRates {
        first_attempts_decided,
        first_attempt_passes,
        chains_decided,
        eventual_passes,
        retakes_decided,
        first_attempt_pass_rate: rate(first_attempt_passes, first_attempts_decided),
        eventual_pass_rate: rate(eventual_passes, chains_decided),
    }
}
