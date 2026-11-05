//! Trust classes and provenance (handoff §7, §12, §14). Every significant record
//! carries where it came from, how much it may be trusted, and a content hash.

use crate::ids::ActorId;
use crate::time::UtcMillis;
use serde::{Deserialize, Serialize};

/// How much a piece of content may be trusted. Only `Operator` and
/// `SystemDeterministic` may ever produce authoritative state directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TrustClass {
    /// Direct operator input through the trusted UI/IPC path.
    Operator,
    /// Deterministic backend computation (timers, checkers, migrations).
    SystemDeterministic,
    /// Reference material that passed the research source approval workflow.
    ApprovedReference,
    /// Imported documents, notes, PDFs, logs, repository files: data, not instructions.
    ImportedDocument,
    /// Skill package content (SKILL.md, references): instructional metadata only.
    SkillPackage,
    /// Model-generated text or structured output: proposal/advisory only.
    ModelOutput,
    /// Provenance unknown; treated as the lowest trust.
    Unknown,
}

impl TrustClass {
    /// Whether content of this class may become authoritative state without a
    /// separate operator decision.
    pub const fn may_be_authoritative(self) -> bool {
        matches!(self, TrustClass::Operator | TrustClass::SystemDeterministic)
    }

    /// Whether content of this class may be interpreted as instructions by the
    /// orchestrator. Everything except operator/system input is data.
    pub const fn may_carry_instructions(self) -> bool {
        self.may_be_authoritative()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceKind {
    OperatorInput,
    SystemGenerated,
    CurriculumManifest,
    Note,
    Attachment,
    ReferenceDocument,
    ModelResponse,
    SkillPackage,
    AssessmentSubmission,
    ExternalFeed,
}

/// Provenance record attached to evidence, observations, claims and events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub source_kind: SourceKind,
    /// Stable reference to the source (record id, file path inside an approved
    /// root, URL of an approved source, model+template version, ...).
    pub source_ref: String,
    pub trust: TrustClass,
    /// Hex SHA-256 of the content as captured, when the content is immutable.
    pub content_sha256: Option<String>,
    pub recorded_at: UtcMillis,
    pub recorded_by: ActorId,
    /// Version of the source (manifest version, skill version, model id).
    pub version: Option<String>,
}

impl Provenance {
    pub fn is_trusted_for_authority(&self) -> bool {
        self.trust.may_be_authoritative()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_operator_and_system_are_authoritative() {
        assert!(TrustClass::Operator.may_be_authoritative());
        assert!(TrustClass::SystemDeterministic.may_be_authoritative());
        for t in [
            TrustClass::ApprovedReference,
            TrustClass::ImportedDocument,
            TrustClass::SkillPackage,
            TrustClass::ModelOutput,
            TrustClass::Unknown,
        ] {
            assert!(!t.may_be_authoritative(), "{t:?}");
            assert!(!t.may_carry_instructions(), "{t:?}");
        }
    }
}
