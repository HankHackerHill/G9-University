//! Prerequisite gap analysis (handoff §10 step 3 "Map relevant prerequisite
//! graph and earlier attempts"; ARCHITECTURE.md §8).
//!
//! A *gap* is a competency whose derived state does not yet count as met
//! (`Independent`, `Validated` or `RetestDue`) together with the direct
//! prerequisites that are themselves unmet. Gaps are ranked by how many
//! competencies they transitively block, so remediation targets the most
//! enabling node first. The analysis is a pure function of the graph and the
//! supplied projections; a competency without a projection is `Unassessed`.

use crate::derive::CompetencyProjection;
use crate::graph::CompetencyGraph;
use crate::CompetencyError;
use g9_domain::{CompetencyId, CompetencyState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One competency that is not yet met, with the prerequisites that block it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Gap {
    /// The unmet competency.
    pub competency_id: CompetencyId,
    /// Derived state (or `Unassessed` when no projection exists).
    pub current_state: CompetencyState,
    /// Direct prerequisites whose state is not met, sorted by id.
    pub missing_prerequisites: Vec<CompetencyId>,
    /// Number of competencies (transitively) that depend on this one.
    pub dependents_blocked: u32,
    /// Secret-free explanation used in proposals and reports.
    pub reason: String,
}

/// Whether a projection counts as a met prerequisite. `RetestDue` still counts:
/// the capability was validated and is merely due for re-verification.
pub fn prerequisite_met(projection: Option<&CompetencyProjection>) -> bool {
    matches!(
        projection.map(|p| p.state),
        Some(
            CompetencyState::Independent | CompetencyState::Validated | CompetencyState::RetestDue
        )
    )
}

/// Direct prerequisites of `id` that are not met in `projections`, sorted by id.
pub fn missing_prerequisites_of(
    graph: &CompetencyGraph,
    id: &CompetencyId,
    projections: &BTreeMap<CompetencyId, CompetencyProjection>,
) -> Result<Vec<CompetencyId>, CompetencyError> {
    let mut missing: Vec<CompetencyId> = graph
        .prerequisites_of(id, false)?
        .into_iter()
        .filter(|p| !prerequisite_met(projections.get(p)))
        .collect();
    missing.sort();
    Ok(missing)
}

/// Every unmet competency in the graph, ranked by `dependents_blocked`
/// (descending) then by id. Deterministic for identical inputs.
pub fn gap_analysis(
    graph: &CompetencyGraph,
    projections: &BTreeMap<CompetencyId, CompetencyProjection>,
) -> Vec<Gap> {
    let mut gaps = Vec::new();
    for id in graph.topological_order() {
        let projection = projections.get(&id);
        if prerequisite_met(projection) {
            continue;
        }
        let current_state = projection
            .map(|p| p.state)
            .unwrap_or(CompetencyState::Unassessed);
        // `id` comes from the graph, so neither lookup can fail; fall back to
        // empty lists rather than panicking if the graph were mutated concurrently.
        let missing_prerequisites =
            missing_prerequisites_of(graph, &id, projections).unwrap_or_default();
        let dependents_blocked = graph
            .dependents_of(&id, true)
            .map(|d| d.len() as u32)
            .unwrap_or(0);
        let reason = if missing_prerequisites.is_empty() {
            format!(
                "{id} is {current_state}; all direct prerequisites are met, so it is ready for guided or independent practice"
            )
        } else {
            format!(
                "{id} is {current_state}; blocked by {} unmet prerequisite(s): {}",
                missing_prerequisites.len(),
                missing_prerequisites
                    .iter()
                    .map(|p| p.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        gaps.push(Gap {
            competency_id: id,
            current_state,
            missing_prerequisites,
            dependents_blocked,
            reason,
        });
    }
    gaps.sort_by(|a, b| {
        b.dependents_blocked
            .cmp(&a.dependents_blocked)
            .then_with(|| a.competency_id.cmp(&b.competency_id))
    });
    gaps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{cid, competency, day};
    use g9_domain::Confidence;

    fn projection(id: &str, state: CompetencyState) -> CompetencyProjection {
        CompetencyProjection {
            competency_id: cid(id),
            state,
            per_dimension: BTreeMap::new(),
            contradictions: Vec::new(),
            evidence_refs: Vec::new(),
            derived_at: day(10),
            rules_hash: "test".to_string(),
            provisional: false,
        }
    }

    fn chain() -> CompetencyGraph {
        // a -> b -> c ; a -> d
        let mut g = CompetencyGraph::new();
        for id in ["a", "b", "c", "d"] {
            g.add_node(competency(id)).unwrap();
        }
        g.add_edge(&cid("a"), &cid("b")).unwrap();
        g.add_edge(&cid("b"), &cid("c")).unwrap();
        g.add_edge(&cid("a"), &cid("d")).unwrap();
        g
    }

    #[test]
    fn gap_ranking_by_dependents() {
        let g = chain();
        let projections = BTreeMap::new();
        let gaps = gap_analysis(&g, &projections);
        let ids: Vec<&str> = gaps.iter().map(|g| g.competency_id.as_str()).collect();
        // `a` blocks b, c, d (3); `b` blocks c (1); c and d block nothing.
        assert_eq!(ids, vec!["a", "b", "c", "d"]);
        assert_eq!(gaps[0].dependents_blocked, 3);
        assert_eq!(gaps[1].dependents_blocked, 1);
        assert!(gaps[0].missing_prerequisites.is_empty());
        assert_eq!(gaps[1].missing_prerequisites, vec![cid("a")]);
        assert!(gaps[1].reason.contains("blocked by 1 unmet prerequisite"));
    }

    #[test]
    fn met_prerequisites_drop_out_and_retest_due_counts_as_met() {
        let g = chain();
        let mut projections = BTreeMap::new();
        projections.insert(cid("a"), projection("a", CompetencyState::RetestDue));
        projections.insert(cid("b"), projection("b", CompetencyState::Guided));
        let gaps = gap_analysis(&g, &projections);
        let ids: Vec<&str> = gaps.iter().map(|g| g.competency_id.as_str()).collect();
        assert_eq!(ids, vec!["b", "c", "d"]);
        assert!(
            gaps[0].missing_prerequisites.is_empty(),
            "a is met (RetestDue)"
        );
        assert_eq!(gaps[0].current_state, CompetencyState::Guided);
        assert_eq!(
            missing_prerequisites_of(&g, &cid("c"), &projections).unwrap(),
            vec![cid("b")]
        );
        assert!(!prerequisite_met(Some(&projection(
            "x",
            CompetencyState::Introduced
        ))));
        assert!(prerequisite_met(Some(&projection(
            "x",
            CompetencyState::Validated
        ))));
        let _ = Confidence::new(0.0);
    }

    #[test]
    fn unknown_competency_is_an_error_not_a_panic() {
        let g = chain();
        let err = missing_prerequisites_of(&g, &cid("zzz"), &BTreeMap::new()).unwrap_err();
        assert!(matches!(err, CompetencyError::UnknownCompetency { .. }));
    }
}
