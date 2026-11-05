//! Interaction modes (handoff §5). Policy is mode-dependent: no hints in EXAM,
//! no self-modification in any mode, no unapproved resources in any mode.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InteractionMode {
    Study,
    Lab,
    Exam,
    Review,
    Executive,
    Research,
    Maintenance,
}

impl InteractionMode {
    pub const ALL: [InteractionMode; 7] = [
        InteractionMode::Study,
        InteractionMode::Lab,
        InteractionMode::Exam,
        InteractionMode::Review,
        InteractionMode::Executive,
        InteractionMode::Research,
        InteractionMode::Maintenance,
    ];

    /// Teaching assistance (hints, Socratic prompts, lessons) is permitted.
    pub const fn teaching_allowed(self) -> bool {
        matches!(self, InteractionMode::Study | InteractionMode::Lab | InteractionMode::Review)
    }

    /// Hints are never allowed in EXAM mode.
    pub const fn hints_allowed(self) -> bool {
        matches!(self, InteractionMode::Study | InteractionMode::Lab)
    }

    /// Application self-modification is never allowed from any runtime mode; the
    /// MAINTENANCE mode only permits operator-approved change sets.
    pub const fn self_modification_allowed(self) -> bool {
        false
    }

    /// Model may see exam task content (minimal context) only when examining.
    pub const fn model_context_minimal(self) -> bool {
        matches!(self, InteractionMode::Exam)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exam_denies_hints_and_nothing_allows_self_modification() {
        assert!(!InteractionMode::Exam.hints_allowed());
        assert!(!InteractionMode::Exam.teaching_allowed());
        for m in InteractionMode::ALL {
            assert!(!m.self_modification_allowed());
        }
    }
}
