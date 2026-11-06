//! Deterministic fixtures shared by this crate's tests. No clocks, no randomness.

use crate::graph::Competency;
use crate::observation::{CompetencyObservation, ObservationSource};
use g9_domain::{
    ActorId, AssistanceLevel, AttemptId, CompetencyId, Confidence, DataDomain, Difficulty,
    InteractionMode, LearningDimension, ObservationId, Provenance, SourceKind, TrustClass,
    UtcMillis,
};

/// Milliseconds in one day.
pub const DAY_MS: i64 = 86_400_000;

/// Fixed epoch for fixtures: 2026-01-01T00:00:00Z.
pub const T0: UtcMillis = UtcMillis::from_millis(1_767_225_600_000);

pub fn cid(s: &str) -> CompetencyId {
    CompetencyId::parse(s).unwrap()
}

pub fn oid(s: &str) -> ObservationId {
    ObservationId::parse(s).unwrap()
}

pub fn day(n: i64) -> UtcMillis {
    UtcMillis::from_millis(T0.as_millis() + n * DAY_MS)
}

pub fn difficulty(d: u8) -> Difficulty {
    Difficulty::new(d).unwrap()
}

pub fn competency(id: &str) -> Competency {
    Competency {
        id: cid(id),
        name: format!("Competency {id}"),
        pack: "fixture-pack".to_string(),
        domain: DataDomain::Learning,
        description: "fixture".to_string(),
        validity_window_days: 90,
        difficulty_band: (difficulty(1), difficulty(5)),
        nice_refs: vec!["K0001".to_string()],
    }
}

pub fn provenance(trust: TrustClass, at: UtcMillis) -> Provenance {
    let (source_kind, by) = match trust {
        TrustClass::Operator => (SourceKind::OperatorInput, "operator"),
        TrustClass::SystemDeterministic => (SourceKind::SystemGenerated, "system"),
        TrustClass::ModelOutput => (SourceKind::ModelResponse, "model"),
        _ => (SourceKind::ReferenceDocument, "other"),
    };
    Provenance {
        source_kind,
        source_ref: "fixture".to_string(),
        trust,
        content_sha256: None,
        recorded_at: at,
        recorded_by: ActorId::parse(by).unwrap(),
        version: None,
    }
}

/// Builder for observations with sensible defaults (STUDY mode, difficulty 3,
/// unaided, zero uncertainty, system-deterministic provenance).
pub struct Obs {
    pub inner: CompetencyObservation,
}

impl Obs {
    pub fn new(
        id: &str,
        competency: &str,
        dimension: LearningDimension,
        score: u8,
        at: UtcMillis,
    ) -> Self {
        Self {
            inner: CompetencyObservation {
                id: oid(id),
                competency_id: cid(competency),
                dimension,
                source: ObservationSource::Attempt(
                    AttemptId::parse(&format!("attempt-{id}")).unwrap(),
                ),
                mode: InteractionMode::Study,
                difficulty: difficulty(3),
                assistance: AssistanceLevel::Unaided,
                score,
                environment: "lab-default".to_string(),
                reviewer: None,
                provenance: provenance(TrustClass::SystemDeterministic, at),
                uncertainty: Confidence::new(0.0).unwrap(),
                observed_at: at,
                supersedes: None,
            },
        }
    }

    pub fn app(id: &str, competency: &str, score: u8, at: UtcMillis) -> Self {
        Self::new(id, competency, LearningDimension::Application, score, at)
    }

    pub fn mode(mut self, mode: InteractionMode) -> Self {
        self.inner.mode = mode;
        self
    }

    pub fn assistance(mut self, a: AssistanceLevel) -> Self {
        self.inner.assistance = a;
        self
    }

    pub fn difficulty(mut self, d: u8) -> Self {
        self.inner.difficulty = difficulty(d);
        self
    }

    pub fn trust(mut self, t: TrustClass) -> Self {
        self.inner.provenance = provenance(t, self.inner.observed_at);
        self
    }

    pub fn supersedes(mut self, id: &str) -> Self {
        self.inner.supersedes = Some(oid(id));
        self.inner.source = ObservationSource::ManualCorrection;
        self.inner.provenance = provenance(TrustClass::Operator, self.inner.observed_at);
        self
    }

    #[allow(dead_code)]
    pub fn environment(mut self, env: &str) -> Self {
        self.inner.environment = env.to_string();
        self
    }

    pub fn uncertainty(mut self, u: f64) -> Self {
        self.inner.uncertainty = Confidence::new(u).unwrap();
        self
    }

    pub fn build(self) -> CompetencyObservation {
        self.inner
    }
}
