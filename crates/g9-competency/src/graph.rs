//! Competency definitions and the prerequisite graph (M06).
//!
//! The graph is a DAG of `prerequisite -> dependent` edges. Cycles are refused
//! at insertion time, so a graph that exists is always topologically orderable.
//! Iteration order is deterministic (ids are kept in `BTreeMap`/`BTreeSet`).

use crate::CompetencyError;
use g9_domain::{CompetencyId, DataDomain, Difficulty};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// A competency node: a nameable capability whose state is derived from
/// dimension-specific observations (handoff §9, C10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Competency {
    /// Stable competency identifier.
    pub id: CompetencyId,
    /// Human-readable name.
    pub name: String,
    /// Knowledge/competency pack this node belongs to (e.g. `web-security`).
    pub pack: String,
    /// Privacy domain; competencies are always `DataDomain::Learning`.
    pub domain: DataDomain,
    /// What the competency covers and how it is evidenced.
    pub description: String,
    /// Days a validation stays current before `RetestDue` (handoff §9).
    pub validity_window_days: u32,
    /// Inclusive `(easiest, hardest)` difficulty band of tasks in this competency.
    pub difficulty_band: (Difficulty, Difficulty),
    /// External mapping identifiers (e.g. NICE framework work-role / KSA ids).
    /// They are labels for cross-referencing only and never carry authority.
    #[serde(default)]
    pub nice_refs: Vec<String>,
}

impl Competency {
    /// Validate structural rules: learning domain, non-empty name, positive
    /// validity window, ordered difficulty band.
    pub fn validate(&self) -> Result<(), CompetencyError> {
        if self.domain != DataDomain::Learning {
            return Err(CompetencyError::InvalidCompetency {
                reason: format!("domain must be LEARNING, got {:?}", self.domain),
            });
        }
        if self.name.trim().is_empty() {
            return Err(CompetencyError::InvalidCompetency {
                reason: "name must not be empty".to_string(),
            });
        }
        if self.validity_window_days == 0 {
            return Err(CompetencyError::InvalidCompetency {
                reason: "validity_window_days must be at least 1".to_string(),
            });
        }
        if self.difficulty_band.0 > self.difficulty_band.1 {
            return Err(CompetencyError::InvalidCompetency {
                reason: format!(
                    "difficulty_band must be ordered (easiest <= hardest), got ({}, {})",
                    self.difficulty_band.0.get(),
                    self.difficulty_band.1.get()
                ),
            });
        }
        Ok(())
    }
}

/// Serializable form of a graph: nodes plus `(prerequisite, dependent)` edges.
/// Deserialization rebuilds the graph through `add_node`/`add_edge`, so an
/// invalid or cyclic document is rejected rather than silently accepted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompetencyGraphSpec {
    /// Competency definitions.
    pub competencies: Vec<Competency>,
    /// `(prerequisite, dependent)` pairs.
    pub edges: Vec<(CompetencyId, CompetencyId)>,
}

/// Directed acyclic prerequisite graph over competencies.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(try_from = "CompetencyGraphSpec", into = "CompetencyGraphSpec")]
pub struct CompetencyGraph {
    nodes: BTreeMap<CompetencyId, Competency>,
    /// dependent -> direct prerequisites
    prerequisites: BTreeMap<CompetencyId, BTreeSet<CompetencyId>>,
    /// prerequisite -> direct dependents
    dependents: BTreeMap<CompetencyId, BTreeSet<CompetencyId>>,
}

impl CompetencyGraph {
    /// Empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a competency node. Fails on duplicate id or invalid definition.
    pub fn add_node(&mut self, competency: Competency) -> Result<(), CompetencyError> {
        competency.validate()?;
        if self.nodes.contains_key(&competency.id) {
            return Err(CompetencyError::DuplicateCompetency {
                id: competency.id.to_string(),
            });
        }
        self.prerequisites.entry(competency.id.clone()).or_default();
        self.dependents.entry(competency.id.clone()).or_default();
        self.nodes.insert(competency.id.clone(), competency);
        Ok(())
    }

    /// Add a `prerequisite -> dependent` edge. Both nodes must exist; self
    /// edges and edges that would close a cycle are refused. Adding an
    /// existing edge is a no-op.
    pub fn add_edge(
        &mut self,
        prerequisite: &CompetencyId,
        dependent: &CompetencyId,
    ) -> Result<(), CompetencyError> {
        self.require(prerequisite)?;
        self.require(dependent)?;
        if prerequisite == dependent {
            return Err(CompetencyError::Cycle {
                prerequisite: prerequisite.to_string(),
                dependent: dependent.to_string(),
            });
        }
        // A cycle appears iff `prerequisite` is already reachable from `dependent`
        // by following existing dependent edges.
        if self.reachable(dependent, prerequisite, Direction::Dependents) {
            return Err(CompetencyError::Cycle {
                prerequisite: prerequisite.to_string(),
                dependent: dependent.to_string(),
            });
        }
        self.prerequisites
            .entry(dependent.clone())
            .or_default()
            .insert(prerequisite.clone());
        self.dependents
            .entry(prerequisite.clone())
            .or_default()
            .insert(dependent.clone());
        Ok(())
    }

    /// Look up a competency.
    pub fn get(&self, id: &CompetencyId) -> Option<&Competency> {
        self.nodes.get(id)
    }

    /// Whether the node exists.
    pub fn contains(&self, id: &CompetencyId) -> bool {
        self.nodes.contains_key(id)
    }

    /// Number of nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the graph has no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// All competency ids in ascending order.
    pub fn ids(&self) -> impl Iterator<Item = &CompetencyId> {
        self.nodes.keys()
    }

    /// All competencies in ascending id order.
    pub fn competencies(&self) -> impl Iterator<Item = &Competency> {
        self.nodes.values()
    }

    /// All `(prerequisite, dependent)` edges in ascending order.
    pub fn edges(&self) -> Vec<(CompetencyId, CompetencyId)> {
        let mut out = Vec::new();
        for (dependent, prereqs) in &self.prerequisites {
            for p in prereqs {
                out.push((p.clone(), dependent.clone()));
            }
        }
        out.sort();
        out
    }

    /// Prerequisites of `id`: direct only, or the full transitive closure.
    /// The result is sorted by id.
    pub fn prerequisites_of(
        &self,
        id: &CompetencyId,
        transitive: bool,
    ) -> Result<Vec<CompetencyId>, CompetencyError> {
        self.require(id)?;
        Ok(self.collect(id, transitive, Direction::Prerequisites))
    }

    /// Dependents of `id`: direct only, or the full transitive closure.
    /// The result is sorted by id.
    pub fn dependents_of(
        &self,
        id: &CompetencyId,
        transitive: bool,
    ) -> Result<Vec<CompetencyId>, CompetencyError> {
        self.require(id)?;
        Ok(self.collect(id, transitive, Direction::Dependents))
    }

    /// Deterministic topological order (prerequisites before dependents;
    /// ties broken by ascending id). Always total because cycles are refused
    /// at insertion.
    pub fn topological_order(&self) -> Vec<CompetencyId> {
        let mut indegree: BTreeMap<&CompetencyId, usize> = self
            .nodes
            .keys()
            .map(|id| (id, self.prerequisites.get(id).map_or(0, BTreeSet::len)))
            .collect();
        let mut ready: BTreeSet<&CompetencyId> = indegree
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(id, _)| *id)
            .collect();
        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(next) = ready.iter().next().cloned() {
            ready.remove(next);
            order.push(next.clone());
            if let Some(deps) = self.dependents.get(next) {
                for d in deps {
                    if let Some(count) = indegree.get_mut(d) {
                        *count = count.saturating_sub(1);
                        if *count == 0 {
                            ready.insert(d);
                        }
                    }
                }
            }
        }
        order
    }

    fn require(&self, id: &CompetencyId) -> Result<(), CompetencyError> {
        if self.nodes.contains_key(id) {
            Ok(())
        } else {
            Err(CompetencyError::UnknownCompetency { id: id.to_string() })
        }
    }

    fn neighbours(
        &self,
        id: &CompetencyId,
        direction: Direction,
    ) -> Option<&BTreeSet<CompetencyId>> {
        match direction {
            Direction::Prerequisites => self.prerequisites.get(id),
            Direction::Dependents => self.dependents.get(id),
        }
    }

    fn reachable(&self, from: &CompetencyId, target: &CompetencyId, direction: Direction) -> bool {
        let mut seen: BTreeSet<&CompetencyId> = BTreeSet::new();
        let mut queue: VecDeque<&CompetencyId> = VecDeque::new();
        queue.push_back(from);
        while let Some(current) = queue.pop_front() {
            if current == target {
                return true;
            }
            if !seen.insert(current) {
                continue;
            }
            if let Some(next) = self.neighbours(current, direction) {
                queue.extend(next.iter());
            }
        }
        false
    }

    fn collect(
        &self,
        id: &CompetencyId,
        transitive: bool,
        direction: Direction,
    ) -> Vec<CompetencyId> {
        let mut out: BTreeSet<CompetencyId> = BTreeSet::new();
        let mut queue: VecDeque<&CompetencyId> = VecDeque::new();
        queue.push_back(id);
        while let Some(current) = queue.pop_front() {
            if let Some(next) = self.neighbours(current, direction) {
                for n in next {
                    if out.insert(n.clone()) && transitive {
                        queue.push_back(n);
                    }
                }
            }
            if !transitive {
                break;
            }
        }
        out.into_iter().collect()
    }
}

#[derive(Clone, Copy)]
enum Direction {
    Prerequisites,
    Dependents,
}

impl TryFrom<CompetencyGraphSpec> for CompetencyGraph {
    type Error = CompetencyError;

    fn try_from(spec: CompetencyGraphSpec) -> Result<Self, Self::Error> {
        let mut graph = CompetencyGraph::new();
        for c in spec.competencies {
            graph.add_node(c)?;
        }
        for (p, d) in &spec.edges {
            graph.add_edge(p, d)?;
        }
        Ok(graph)
    }
}

impl From<CompetencyGraph> for CompetencyGraphSpec {
    fn from(graph: CompetencyGraph) -> Self {
        let edges = graph.edges();
        CompetencyGraphSpec {
            competencies: graph.nodes.into_values().collect(),
            edges,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{cid, competency};
    use pretty_assertions::assert_eq;

    fn chain() -> CompetencyGraph {
        let mut g = CompetencyGraph::new();
        for id in ["linux", "python", "recon", "web"] {
            g.add_node(competency(id)).unwrap();
        }
        g.add_edge(&cid("linux"), &cid("recon")).unwrap();
        g.add_edge(&cid("python"), &cid("recon")).unwrap();
        g.add_edge(&cid("recon"), &cid("web")).unwrap();
        g
    }

    #[test]
    fn graph_rejects_cycles_and_orders_topologically() {
        let mut g = chain();
        let err = g.add_edge(&cid("web"), &cid("linux")).unwrap_err();
        assert!(matches!(err, CompetencyError::Cycle { .. }), "{err:?}");
        assert!(matches!(
            g.add_edge(&cid("web"), &cid("web")).unwrap_err(),
            CompetencyError::Cycle { .. }
        ));
        let g9: g9_domain::G9Error = err.into();
        assert_eq!(g9.category, g9_domain::ErrorCategory::Conflict);

        let order = g.topological_order();
        assert_eq!(order.len(), 4);
        let pos = |id: &str| order.iter().position(|x| x == &cid(id)).unwrap();
        assert!(pos("linux") < pos("recon"));
        assert!(pos("python") < pos("recon"));
        assert!(pos("recon") < pos("web"));
        // Deterministic tie-break: linux before python (both roots).
        assert_eq!(order[0], cid("linux"));
        assert_eq!(order[1], cid("python"));

        assert_eq!(
            g.prerequisites_of(&cid("web"), false).unwrap(),
            vec![cid("recon")]
        );
        assert_eq!(
            g.prerequisites_of(&cid("web"), true).unwrap(),
            vec![cid("linux"), cid("python"), cid("recon")]
        );
        assert_eq!(
            g.dependents_of(&cid("linux"), true).unwrap(),
            vec![cid("recon"), cid("web")]
        );
        assert_eq!(
            g.dependents_of(&cid("linux"), false).unwrap(),
            vec![cid("recon")]
        );
        assert!(matches!(
            g.prerequisites_of(&cid("nope"), true).unwrap_err(),
            CompetencyError::UnknownCompetency { .. }
        ));
    }

    #[test]
    fn graph_rejects_duplicates_unknown_edges_and_invalid_definitions() {
        let mut g = chain();
        assert!(matches!(
            g.add_node(competency("linux")).unwrap_err(),
            CompetencyError::DuplicateCompetency { .. }
        ));
        assert!(matches!(
            g.add_edge(&cid("linux"), &cid("ghost")).unwrap_err(),
            CompetencyError::UnknownCompetency { .. }
        ));
        let mut bad = competency("bad");
        bad.domain = DataDomain::Business;
        assert!(matches!(
            g.add_node(bad).unwrap_err(),
            CompetencyError::InvalidCompetency { .. }
        ));
        let mut bad = competency("bad2");
        bad.validity_window_days = 0;
        assert!(g.add_node(bad).is_err());
        let mut bad = competency("bad3");
        bad.difficulty_band = (Difficulty::new(4).unwrap(), Difficulty::new(2).unwrap());
        assert!(g.add_node(bad).is_err());
    }

    #[test]
    fn graph_serde_round_trip_rebuilds_and_rejects_cyclic_documents() {
        let g = chain();
        let json = serde_json::to_string(&g).unwrap();
        let back: CompetencyGraph = serde_json::from_str(&json).unwrap();
        assert_eq!(back, g);

        let mut spec: CompetencyGraphSpec = g.into();
        spec.edges.push((cid("web"), cid("linux")));
        let cyclic = serde_json::to_string(&spec).unwrap();
        assert!(serde_json::from_str::<CompetencyGraph>(&cyclic).is_err());
    }
}
