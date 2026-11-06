//! Typed identifiers. Each identifier type is a distinct newtype so that an
//! `AttemptId` can never be passed where a `SessionId` is expected.
//!
//! Identifiers are either time-ordered UUIDv7 strings (generated locally) or
//! stable human-readable slugs (curriculum day IDs such as `D01`, skill IDs such
//! as `g9-session-director`). Both forms are validated on parse.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Error produced when an identifier string is not acceptable.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {kind} identifier: {reason}")]
pub struct IdError {
    /// Identifier kind (e.g. `SessionId`).
    pub kind: &'static str,
    /// Human-readable reason; never contains the offending value in full to avoid
    /// echoing untrusted input into logs.
    pub reason: String,
}

pub(crate) const MAX_ID_LEN: usize = 128;

pub(crate) fn validate_id(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("empty".to_string());
    }
    if s.len() > MAX_ID_LEN {
        return Err(format!("longer than {MAX_ID_LEN} bytes"));
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ':'))
    {
        return Err("contains characters outside [A-Za-z0-9._:-]".to_string());
    }
    Ok(())
}

macro_rules! typed_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// Identifier kind name, used in error messages.
            pub const KIND: &'static str = stringify!($name);

            /// Generate a new time-ordered UUIDv7 identifier.
            pub fn new_v7() -> Self {
                Self(uuid::Uuid::now_v7().to_string())
            }

            /// Validate and wrap an identifier string.
            pub fn parse(s: &str) -> Result<Self, IdError> {
                validate_id(s)
                    .map(|_| Self(s.to_string()))
                    .map_err(|reason| IdError { kind: Self::KIND, reason })
            }

            /// Borrow the underlying string.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", Self::KIND, self.0)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdError;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::parse(s)
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdError;
            fn try_from(s: String) -> Result<Self, Self::Error> {
                Self::parse(&s)
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> String {
                id.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

typed_id!(/// Operator, system, skill or model actor.
    ActorId);
typed_id!(/// Per-command request identifier (unique per call).
    RequestId);
typed_id!(/// Idempotency key supplied by the caller; replays return the stored outcome.
    IdempotencyKey);
typed_id!(/// Correlation/trace identifier spanning a user intent across commands.
    TraceId);
typed_id!(/// Curriculum (program) identifier, e.g. `g9-ct9-480`.
    CurriculumId);
typed_id!(/// Immutable curriculum version identifier.
    CurriculumVersionId);
typed_id!(/// Training day identifier, e.g. `D01`.
    DayId);
typed_id!(/// Daily objective identifier, e.g. `D01-O1`.
    ObjectiveId);
typed_id!(/// Competency node identifier.
    CompetencyId);
typed_id!(/// Study session identifier.
    SessionId);
typed_id!(/// Calendar event identifier.
    CalendarEventId);
typed_id!(/// Schedule proposal identifier.
    ScheduleProposalId);
typed_id!(/// Note identifier.
    NoteId);
typed_id!(/// Attachment identifier (content-addressed store reference).
    AttachmentId);
typed_id!(/// Evidence item identifier.
    EvidenceId);
typed_id!(/// Assessment definition identifier.
    AssessmentId);
typed_id!(/// Assessment attempt identifier.
    AttemptId);
typed_id!(/// Grade decision identifier.
    GradeDecisionId);
typed_id!(/// Competency observation identifier.
    ObservationId);
typed_id!(/// Retention review item identifier.
    ReviewItemId);
typed_id!(/// Intervention identifier.
    InterventionId);
typed_id!(/// Method experiment identifier.
    ExperimentId);
typed_id!(/// Goal identifier.
    GoalId);
typed_id!(/// Project identifier.
    ProjectId);
typed_id!(/// Milestone identifier.
    MilestoneId);
typed_id!(/// Decision record identifier.
    DecisionId);
typed_id!(/// Research source identifier.
    SourceId);
typed_id!(/// Knowledge document identifier.
    DocumentId);
typed_id!(/// Local model registry identifier.
    ModelId);
typed_id!(/// Skill identifier (Agent Skills `name`), e.g. `g9-session-director`.
    SkillId);
typed_id!(/// Skill invocation identifier.
    InvocationId);
typed_id!(/// Approval request identifier.
    ApprovalId);
typed_id!(/// Policy decision log identifier.
    PolicyDecisionId);
typed_id!(/// Audit metadata row identifier.
    AuditRef);
typed_id!(/// Backup manifest identifier.
    BackupId);
typed_id!(/// Significant event identifier.
    EventId);
typed_id!(/// Learning episode identifier.
    EpisodeId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_slugs_and_uuids() {
        assert!(DayId::parse("D01").is_ok());
        assert!(SkillId::parse("g9-session-director").is_ok());
        let id = SessionId::new_v7();
        assert!(SessionId::parse(id.as_str()).is_ok());
        assert_eq!(
            uuid::Uuid::parse_str(id.as_str()).map(|u| u.get_version_num()),
            Ok(7)
        );
    }

    #[test]
    fn parse_rejects_bad_input() {
        assert!(DayId::parse("").is_err());
        assert!(DayId::parse("D01 ").is_err());
        assert!(DayId::parse("../etc/passwd").is_err());
        assert!(DayId::parse(&"x".repeat(129)).is_err());
    }

    #[test]
    fn serde_round_trip_validates() {
        let ok: Result<DayId, _> = serde_json::from_str("\"D72\"");
        assert!(ok.is_ok());
        let bad: Result<DayId, _> = serde_json::from_str("\"bad id\"");
        assert!(bad.is_err());
        assert_eq!(
            serde_json::to_string(&DayId::parse("D01").unwrap()).unwrap(),
            "\"D01\""
        );
    }

    #[test]
    fn types_are_distinct() {
        fn takes_session(_: &SessionId) {}
        let s = SessionId::parse("s").unwrap();
        takes_session(&s);
        // `takes_session(&AttemptId::parse("a").unwrap())` must not compile.
    }
}
