//! Deterministic failure classification (handoff §10 "Diagnosis → prescription
//! loop", steps 2–5): classify an observed failure into competing hypotheses
//! with supporting and contradicting evidence and a discriminating test, and
//! return `Underdetermined` instead of asserting a root cause when the leading
//! hypotheses tie.
//!
//! This is rule-based and transparent: every score contribution is traceable
//! to a named signal. It never consults a model; model-generated hypotheses
//! enter the system only as `PROPOSED` learner hypotheses (see `memory`).

use crate::observation::CompetencyObservation;
use g9_domain::{AssistanceLevel, InteractionMode, LearningDimension};
use serde::{Deserialize, Serialize};

/// Classes of observed failure (handoff §10 step 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FailureClass {
    Recall,
    Understanding,
    Procedure,
    Reasoning,
    Transfer,
    Environment,
    InstructionAmbiguity,
    FlawedTest,
}

impl FailureClass {
    /// All classes in a stable order.
    pub const ALL: [FailureClass; 8] = [
        FailureClass::Recall,
        FailureClass::Understanding,
        FailureClass::Procedure,
        FailureClass::Reasoning,
        FailureClass::Transfer,
        FailureClass::Environment,
        FailureClass::InstructionAmbiguity,
        FailureClass::FlawedTest,
    ];

    /// The unbiased diagnostic that would discriminate this class from the others
    /// (handoff §10 step 5).
    pub const fn discriminating_test(self) -> &'static str {
        match self {
            FailureClass::Recall => "closed-book, unaided recall probe of the prerequisite facts and commands",
            FailureClass::Understanding => "teach-back of the mechanism with a why-question chain; no procedure steps asked",
            FailureClass::Procedure => "repeat the same task with the reference procedure open; success isolates execution from understanding",
            FailureClass::Reasoning => "worked variant with intermediate checkpoints that expose where the inference chain breaks",
            FailureClass::Transfer => "the already-passed task family in a new context with surface features changed",
            FailureClass::Environment => "re-run the identical task in a verified lab after an environment health check",
            FailureClass::InstructionAmbiguity => "re-issue the task with a clarified statement and an explicit acceptance criterion",
            FailureClass::FlawedTest => "independent review of the item, key and checker against a known-good solution",
        }
    }
}

/// Environment signals recorded with the attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EnvironmentFlags {
    /// Lab/VM/container was unavailable or degraded during the task.
    pub lab_unavailable: bool,
    /// A required tool crashed or returned errors unrelated to the learner's actions.
    pub tool_error: bool,
    /// The time limit was exceeded.
    pub time_exceeded: bool,
}

/// Instruction-quality signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct InstructionFlags {
    /// The task statement was flagged ambiguous by a reviewer.
    pub reviewer_flagged_ambiguous: bool,
    /// The learner reported ambiguity before or during the attempt.
    pub learner_reported_ambiguity: bool,
}

/// Test-validity signals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TestFlags {
    /// The item is already flagged defective (checker, key or fixture).
    pub item_flagged_defective: bool,
    /// The answer key is under dispute.
    pub answer_key_disputed: bool,
}

/// Context for one diagnosis.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosisContext {
    /// Earlier retained observations for the same competency (any dimension).
    pub prior_observations: Vec<CompetencyObservation>,
    pub environment: EnvironmentFlags,
    pub instruction: InstructionFlags,
    pub test: TestFlags,
    /// Score at or above which the observation is not a failure.
    pub failure_threshold: u8,
}

impl DiagnosisContext {
    /// Context with no flags and the given threshold.
    pub fn new(prior_observations: Vec<CompetencyObservation>, failure_threshold: u8) -> Self {
        Self {
            prior_observations,
            environment: EnvironmentFlags::default(),
            instruction: InstructionFlags::default(),
            test: TestFlags::default(),
            failure_threshold,
        }
    }
}

/// One ranked hypothesis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureHypothesis {
    pub class: FailureClass,
    /// Plausibility score 0..=100 from named signals (not a probability).
    pub score: u8,
    pub supporting: Vec<String>,
    pub contradicting: Vec<String>,
    pub discriminating_test: String,
}

/// Result of a diagnosis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiagnosisOutcome {
    /// The observation is not a failure at the configured threshold.
    NotAFailure { score: u8, threshold: u8 },
    /// A single leading hypothesis exists.
    Ranked {
        leading: FailureClass,
        hypotheses: Vec<FailureHypothesis>,
    },
    /// The top hypotheses tie; request a discriminating diagnostic (step 5).
    Underdetermined {
        tied: Vec<FailureClass>,
        hypotheses: Vec<FailureHypothesis>,
    },
}

fn get(t: &mut [(FailureClass, Tally)], c: FailureClass) -> &mut Tally {
    let idx = FailureClass::ALL.iter().position(|x| *x == c).unwrap_or(0);
    &mut t[idx].1
}

struct Tally {
    score: i32,
    supporting: Vec<String>,
    contradicting: Vec<String>,
}

impl Tally {
    fn new() -> Self {
        Self {
            score: 0,
            supporting: Vec::new(),
            contradicting: Vec::new(),
        }
    }
    fn support(&mut self, points: i32, why: &str) {
        self.score += points;
        self.supporting.push(format!("+{points}: {why}"));
    }
    fn contradict(&mut self, points: i32, why: &str) {
        self.score -= points;
        self.contradicting.push(format!("-{points}: {why}"));
    }
}

fn prior_good(ctx: &DiagnosisContext, dim: LearningDimension) -> Option<bool> {
    let mut latest: Option<&CompetencyObservation> = None;
    for o in ctx.prior_observations.iter().filter(|o| o.dimension == dim) {
        if latest.is_none_or(|l| o.observed_at > l.observed_at) {
            latest = Some(o);
        }
    }
    latest.map(|o| o.score >= ctx.failure_threshold)
}

/// Classify a failed observation into ranked competing hypotheses.
pub fn classify_failure(
    observation: &CompetencyObservation,
    ctx: &DiagnosisContext,
) -> DiagnosisOutcome {
    if observation.score >= ctx.failure_threshold {
        return DiagnosisOutcome::NotAFailure {
            score: observation.score,
            threshold: ctx.failure_threshold,
        };
    }
    let mut t: Vec<(FailureClass, Tally)> = FailureClass::ALL
        .iter()
        .map(|c| (*c, Tally::new()))
        .collect();

    // Environment, instruction and test-validity signals dominate when present.
    if ctx.environment.lab_unavailable || ctx.environment.tool_error {
        get(&mut t, FailureClass::Environment).support(
            60,
            "lab unavailable or tool error recorded during the attempt",
        );
    }
    if ctx.environment.time_exceeded {
        get(&mut t, FailureClass::Environment).support(15, "time limit exceeded");
        get(&mut t, FailureClass::Procedure).support(
            15,
            "time limit exceeded suggests slow or uncertain execution",
        );
    }
    if ctx.instruction.reviewer_flagged_ambiguous || ctx.instruction.learner_reported_ambiguity {
        get(&mut t, FailureClass::InstructionAmbiguity)
            .support(50, "task statement flagged ambiguous");
    }
    if ctx.test.item_flagged_defective || ctx.test.answer_key_disputed {
        get(&mut t, FailureClass::FlawedTest).support(50, "item flagged defective or key disputed");
    }

    // Dimension of the failed observation.
    match observation.dimension {
        LearningDimension::Recall => get(&mut t, FailureClass::Recall)
            .support(40, "failed observation is in the RECALL dimension"),
        LearningDimension::Understanding => get(&mut t, FailureClass::Understanding)
            .support(40, "failed observation is in the UNDERSTANDING dimension"),
        LearningDimension::Application => {
            match prior_good(ctx, LearningDimension::Understanding) {
                Some(true) => {
                    get(&mut t, FailureClass::Procedure).support(35, "latest UNDERSTANDING evidence is good, so execution rather than mechanism is implicated");
                    get(&mut t, FailureClass::Reasoning).support(20, "application failure with sound understanding can be an inference-chain error");
                    get(&mut t, FailureClass::Understanding)
                        .contradict(20, "latest UNDERSTANDING evidence is good");
                }
                Some(false) => get(&mut t, FailureClass::Understanding)
                    .support(35, "latest UNDERSTANDING evidence is also below threshold"),
                None => {
                    get(&mut t, FailureClass::Procedure).support(
                        20,
                        "no UNDERSTANDING evidence available; execution failure is plausible",
                    );
                    get(&mut t, FailureClass::Understanding).support(
                        20,
                        "no UNDERSTANDING evidence available; mechanism gap is plausible",
                    );
                }
            }
            if prior_good(ctx, LearningDimension::Recall) == Some(false) {
                get(&mut t, FailureClass::Recall)
                    .support(25, "latest RECALL evidence is below threshold");
            }
        }
        LearningDimension::Transfer => match prior_good(ctx, LearningDimension::Application) {
            Some(true) => {
                get(&mut t, FailureClass::Transfer)
                    .support(50, "APPLICATION is good but the novel-context task failed");
                get(&mut t, FailureClass::Procedure)
                    .contradict(20, "latest APPLICATION evidence is good");
            }
            Some(false) => get(&mut t, FailureClass::Procedure).support(
                30,
                "APPLICATION evidence is also below threshold; transfer cannot be isolated",
            ),
            None => get(&mut t, FailureClass::Transfer)
                .support(20, "transfer failure without application evidence"),
        },
    }

    // Assistance and mode modifiers.
    match observation.assistance {
        AssistanceLevel::Unaided => get(&mut t, FailureClass::Recall)
            .support(10, "unaided attempt: missing recall is a candidate cause"),
        AssistanceLevel::Prompted | AssistanceLevel::Hinted => {
            get(&mut t, FailureClass::Procedure)
                .support(10, "failed despite hints: execution difficulty");
            get(&mut t, FailureClass::Reasoning)
                .support(10, "failed despite hints: reasoning difficulty");
            get(&mut t, FailureClass::Recall).contradict(10, "hints supplied the recall component");
        }
        AssistanceLevel::Guided | AssistanceLevel::Shown => {
            get(&mut t, FailureClass::Procedure).support(20, "failed even when guided or shown");
            get(&mut t, FailureClass::Recall).contradict(20, "guidance removed the recall demand");
            get(&mut t, FailureClass::Understanding).support(
                10,
                "failure under guidance suggests the mechanism is not understood",
            );
        }
    }
    if observation.mode == InteractionMode::Exam {
        get(&mut t, FailureClass::Environment).support(
            5,
            "exam-mode restrictions can surface environment fragility",
        );
    }
    if observation.uncertainty.get() >= 0.5 {
        for (_, tally) in t.iter_mut() {
            tally.contradicting.push(
                "-0: observation uncertainty >= 0.5 weakens every hypothesis equally".to_string(),
            );
        }
    }

    let mut hypotheses: Vec<FailureHypothesis> = t
        .into_iter()
        .map(|(class, tally)| FailureHypothesis {
            class,
            score: tally.score.clamp(0, 100) as u8,
            supporting: tally.supporting,
            contradicting: tally.contradicting,
            discriminating_test: class.discriminating_test().to_string(),
        })
        .collect();
    hypotheses.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.class.cmp(&b.class)));

    let top = hypotheses[0].score;
    let tied: Vec<FailureClass> = hypotheses
        .iter()
        .filter(|h| h.score == top)
        .map(|h| h.class)
        .collect();
    if top == 0 || tied.len() > 1 {
        DiagnosisOutcome::Underdetermined { tied, hypotheses }
    } else {
        DiagnosisOutcome::Ranked {
            leading: hypotheses[0].class,
            hypotheses,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{day, Obs};

    #[test]
    fn not_a_failure_above_threshold() {
        let o = Obs::app("o1", "c", 85, day(5)).build();
        let out = classify_failure(&o, &DiagnosisContext::new(vec![], 70));
        assert!(matches!(
            out,
            DiagnosisOutcome::NotAFailure {
                score: 85,
                threshold: 70
            }
        ));
    }

    #[test]
    fn environment_signal_dominates() {
        let o = Obs::app("o1", "c", 40, day(5)).build();
        let mut ctx = DiagnosisContext::new(vec![], 70);
        ctx.environment.lab_unavailable = true;
        match classify_failure(&o, &ctx) {
            DiagnosisOutcome::Ranked {
                leading,
                hypotheses,
            } => {
                assert_eq!(leading, FailureClass::Environment);
                assert!(hypotheses[0]
                    .supporting
                    .iter()
                    .any(|s| s.contains("lab unavailable")));
                assert_eq!(
                    hypotheses[0].discriminating_test,
                    FailureClass::Environment.discriminating_test()
                );
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn application_failure_with_good_understanding_points_to_procedure() {
        let prior = Obs::new("u1", "c", LearningDimension::Understanding, 90, day(2)).build();
        let o = Obs::app("o1", "c", 40, day(5)).build();
        let out = classify_failure(&o, &DiagnosisContext::new(vec![prior], 70));
        match out {
            DiagnosisOutcome::Ranked {
                leading,
                hypotheses,
            } => {
                assert_eq!(leading, FailureClass::Procedure);
                let und = hypotheses
                    .iter()
                    .find(|h| h.class == FailureClass::Understanding)
                    .unwrap();
                assert!(und
                    .contradicting
                    .iter()
                    .any(|c| c.contains("UNDERSTANDING evidence is good")));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn diagnosis_returns_underdetermined_on_tie() {
        // Application failure with no prior evidence: Procedure and Understanding tie at 20 + 10 (unaided adds to Recall only).
        let o = Obs::app("o1", "c", 40, day(5)).build();
        let out = classify_failure(&o, &DiagnosisContext::new(vec![], 70));
        match out {
            DiagnosisOutcome::Underdetermined { tied, hypotheses } => {
                assert!(tied.contains(&FailureClass::Procedure));
                assert!(tied.contains(&FailureClass::Understanding));
                assert_eq!(hypotheses[0].score, hypotheses[1].score);
                assert!(hypotheses.iter().all(|h| !h.discriminating_test.is_empty()));
            }
            other => panic!("expected underdetermined, got {other:?}"),
        }
    }

    #[test]
    fn transfer_failure_with_good_application_is_transfer() {
        let prior = Obs::app("a1", "c", 90, day(2)).build();
        let o = Obs::new("t1", "c", LearningDimension::Transfer, 30, day(5)).build();
        match classify_failure(&o, &DiagnosisContext::new(vec![prior], 70)) {
            DiagnosisOutcome::Ranked { leading, .. } => assert_eq!(leading, FailureClass::Transfer),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn outcome_serializes_with_screaming_tags() {
        let o = Obs::app("o1", "c", 40, day(5)).build();
        let mut ctx = DiagnosisContext::new(vec![], 70);
        ctx.test.item_flagged_defective = true;
        let v = serde_json::to_value(classify_failure(&o, &ctx)).unwrap();
        assert_eq!(v["outcome"], "RANKED");
        assert_eq!(v["leading"], "FLAWED_TEST");
    }
}
