//! Review items: small declarative/procedural/conceptual prompts with a
//! private answer reference. Items belong to a competency but their schedule
//! says nothing about practical competence (B02).

use g9_domain::{CompetencyId, DataDomain, EvidenceId, ReviewItemId, UtcMillis};
use serde::{Deserialize, Serialize};

/// What kind of memory the item exercises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ItemKind {
    /// A fact, flag, command or definition; eligible for FSRS.
    DeclarativeFact,
    /// A short procedure recalled step by step; baseline schedule only.
    Procedure,
    /// A concept explained in the learner's words; baseline schedule only.
    Concept,
}

impl ItemKind {
    pub const fn is_declarative(self) -> bool {
        matches!(self, ItemKind::DeclarativeFact)
    }
}

/// One review item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewItem {
    pub id: ReviewItemId,
    pub competency_id: CompetencyId,
    pub kind: ItemKind,
    /// Prompt shown to the learner (evidence reference, never inline content).
    pub prompt_ref: EvidenceId,
    /// Answer/key reference; classification `Private`; shown only after the answer.
    pub answer_ref: EvidenceId,
    pub created_at: UtcMillis,
    pub data_domain: DataDomain,
}
