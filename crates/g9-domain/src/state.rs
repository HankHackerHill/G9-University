//! Explicit state machines (handoff v6.0 §16). Each machine is an enum with a
//! pure `transition` function and an enumerable transition table so that every
//! allowed and forbidden edge is testable. Persistence, actors, timestamps and
//! revision checks wrap these in the application layer; the machines themselves
//! only answer "is this edge legal?".

use serde::{Deserialize, Serialize};
use std::fmt;

/// A refused transition.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{machine}: event {event} is not allowed in state {from}")]
pub struct TransitionError {
    pub machine: &'static str,
    pub from: String,
    pub event: String,
}

fn refuse<S: fmt::Debug, E: fmt::Debug>(
    machine: &'static str,
    from: &S,
    event: &E,
) -> TransitionError {
    TransitionError {
        machine,
        from: format!("{from:?}"),
        event: format!("{event:?}"),
    }
}

macro_rules! state_machine {
    (
        $(#[$meta:meta])*
        machine $name:ident / $event:ident ($label:literal) {
            states { $($state:ident),+ $(,)? }
            events { $($ev:ident),+ $(,)? }
            terminal { $($term:ident),* $(,)? }
            transitions { $( $from:ident + $on:ident => $to:ident ),+ $(,)? }
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum $name { $($state),+ }

        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum $event { $($ev),+ }

        impl $name {
            pub const MACHINE: &'static str = $label;
            pub const ALL: &'static [$name] = &[$($name::$state),+];
            /// Complete transition table `(from, event, to)`.
            pub const TRANSITIONS: &'static [($name, $event, $name)] = &[
                $( ($name::$from, $event::$on, $name::$to) ),+
            ];

            /// Terminal states (no outgoing edges).
            pub const TERMINAL: &'static [$name] = &[$($name::$term),*];

            pub fn is_terminal(self) -> bool {
                Self::TERMINAL.contains(&self)
            }

            /// Apply an event; returns the next state or a refusal.
            pub fn transition(self, event: $event) -> Result<$name, TransitionError> {
                for (from, on, to) in Self::TRANSITIONS {
                    if *from == self && *on == event {
                        return Ok(*to);
                    }
                }
                Err(refuse(Self::MACHINE, &self, &event))
            }

            pub fn can(self, event: $event) -> bool {
                self.transition(event).is_ok()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                let v = serde_json::to_value(self).unwrap_or_default();
                f.write_str(v.as_str().unwrap_or("?"))
            }
        }
    };
}

state_machine! {
    /// Study session (M04). `Interrupted` is entered when continuity is uncertain
    /// (suspend/reboot/crash); it requires an operator correction before any
    /// time is credited (R-? A03).
    machine SessionState / SessionEvent ("session") {
        states { Planned, Ready, Running, Paused, Completed, Reviewed, Closed, Interrupted, Cancelled }
        events { Prepare, Start, Pause, Resume, Complete, Review, Close, Interrupt, Cancel, Recover }
        terminal { Closed, Cancelled }
        transitions {
            Planned + Prepare => Ready,
            Planned + Cancel => Cancelled,
            Ready + Start => Running,
            Ready + Cancel => Cancelled,
            Running + Pause => Paused,
            Running + Complete => Completed,
            Running + Interrupt => Interrupted,
            Paused + Resume => Running,
            Paused + Complete => Completed,
            Paused + Interrupt => Interrupted,
            Interrupted + Recover => Paused,
            Interrupted + Complete => Completed,
            Interrupted + Cancel => Cancelled,
            Completed + Review => Reviewed,
            Reviewed + Close => Closed,
        }
    }
}

state_machine! {
    /// Assessment attempt (M07). Grades become official only at `ApprovedFinal`
    /// through an operator/reviewer decision; a retake is a NEW attempt linked to
    /// this one, never a mutation of it. Finalized attempts have no outgoing edges.
    machine AssessmentAttemptState / AssessmentEvent ("assessment_attempt") {
        states { Available, Started, Submitted, EvidenceReview, GradedProvisional, ApprovedFinal, Voided }
        events { Begin, Submit, ReviewEvidence, GradeProvisional, ApproveFinal, Void, Abandon }
        terminal { ApprovedFinal, Voided }
        transitions {
            Available + Begin => Started,
            Started + Submit => Submitted,
            Started + Abandon => Voided,
            Submitted + ReviewEvidence => EvidenceReview,
            Submitted + Void => Voided,
            EvidenceReview + GradeProvisional => GradedProvisional,
            EvidenceReview + Void => Voided,
            GradedProvisional + ReviewEvidence => EvidenceReview,
            GradedProvisional + ApproveFinal => ApprovedFinal,
            GradedProvisional + Void => Voided,
        }
    }
}

state_machine! {
    /// Intervention lifecycle (M12/L04). Model output can only create `Hypothesis`
    /// and `Proposed`; everything from `Approved` onward requires an operator decision.
    machine InterventionState / InterventionEvent ("intervention") {
        states { Hypothesis, Proposed, Reviewed, Approved, Rejected, Scheduled, Applied, Retested, Evaluated, Retained, Revised, Retired }
        events { Propose, Review, Approve, Reject, Schedule, Apply, Retest, Evaluate, Retain, Revise, Retire }
        terminal { Rejected, Retained, Retired }
        transitions {
            Hypothesis + Propose => Proposed,
            Hypothesis + Reject => Rejected,
            Proposed + Review => Reviewed,
            Proposed + Reject => Rejected,
            Reviewed + Approve => Approved,
            Reviewed + Reject => Rejected,
            Approved + Schedule => Scheduled,
            Approved + Retire => Retired,
            Scheduled + Apply => Applied,
            Scheduled + Retire => Retired,
            Applied + Retest => Retested,
            Applied + Retire => Retired,
            Retested + Evaluate => Evaluated,
            Evaluated + Retain => Retained,
            Evaluated + Revise => Revised,
            Evaluated + Retire => Retired,
            Revised + Propose => Proposed,
        }
    }
}

state_machine! {
    /// Skill invocation (M11). `Authorized` is only reachable from `Preflight`
    /// after a policy decision; there is no edge from any state back to `Running`
    /// (no recursion/retry loops inside one invocation).
    machine SkillInvocationState / SkillInvocationEvent ("skill_invocation") {
        states { Requested, Preflight, Authorized, Running, Validating, Completed, Error, Denied, TimedOut, Cancelled }
        events { Preflight, Authorize, Deny, Run, Validate, Complete, Fail, Timeout, Cancel }
        terminal { Completed, Error, Denied, TimedOut, Cancelled }
        transitions {
            Requested + Preflight => Preflight,
            Requested + Cancel => Cancelled,
            Preflight + Authorize => Authorized,
            Preflight + Deny => Denied,
            Preflight + Fail => Error,
            Preflight + Cancel => Cancelled,
            Authorized + Run => Running,
            Authorized + Cancel => Cancelled,
            Running + Validate => Validating,
            Running + Fail => Error,
            Running + Timeout => TimedOut,
            Running + Cancel => Cancelled,
            Validating + Complete => Completed,
            Validating + Fail => Error,
        }
    }
}

state_machine! {
    /// Research source (M15).
    machine SourceState / SourceEvent ("research_source") {
        states { Untrusted, Quarantined, Evaluated, ApprovedReference, Superseded, Revoked }
        events { Quarantine, Evaluate, Approve, Supersede, Revoke, Reject }
        terminal { Revoked }
        transitions {
            Untrusted + Quarantine => Quarantined,
            Untrusted + Revoke => Revoked,
            Quarantined + Evaluate => Evaluated,
            Quarantined + Revoke => Revoked,
            Evaluated + Approve => ApprovedReference,
            Evaluated + Reject => Quarantined,
            Evaluated + Revoke => Revoked,
            ApprovedReference + Supersede => Superseded,
            ApprovedReference + Revoke => Revoked,
            Superseded + Revoke => Revoked,
        }
    }
}

state_machine! {
    /// Backup set (M17). A backup is only `Retained` after a restore test.
    machine BackupState / BackupEvent ("backup") {
        states { Requested, SnapshotCreated, Checked, RestoreTested, Retained, Expired, Failed }
        events { Snapshot, Check, TestRestore, Retain, Expire, Fail }
        terminal { Expired, Failed }
        transitions {
            Requested + Snapshot => SnapshotCreated,
            Requested + Fail => Failed,
            SnapshotCreated + Check => Checked,
            SnapshotCreated + Fail => Failed,
            Checked + TestRestore => RestoreTested,
            Checked + Fail => Failed,
            RestoreTested + Retain => Retained,
            RestoreTested + Fail => Failed,
            Retained + Expire => Expired,
        }
    }
}

state_machine! {
    /// Competency skill state (handoff §9 "Skill state (not a universal float)").
    /// Transitions are driven by evidence rules in `g9-competency`, never by
    /// model praise or self-rating.
    machine CompetencyState / CompetencyEvent ("competency") {
        states { Unassessed, Introduced, Guided, Independent, Validated, RetestDue }
        events { Introduce, DemonstrateGuided, DemonstrateIndependent, Validate, RetestDue, Regress, Reset }
        terminal { }
        transitions {
            Unassessed + Introduce => Introduced,
            Unassessed + DemonstrateGuided => Guided,
            Unassessed + DemonstrateIndependent => Independent,
            Introduced + DemonstrateGuided => Guided,
            Introduced + DemonstrateIndependent => Independent,
            Guided + DemonstrateIndependent => Independent,
            Independent + Validate => Validated,
            Independent + Regress => Guided,
            Validated + RetestDue => RetestDue,
            Validated + Regress => Independent,
            RetestDue + Validate => Validated,
            RetestDue + Regress => Independent,
            Introduced + Reset => Unassessed,
            Guided + Reset => Unassessed,
            Independent + Reset => Unassessed,
            Validated + Reset => Unassessed,
            RetestDue + Reset => Unassessed,
        }
    }
}

state_machine! {
    /// Approval request (M16). `Consumed` is single-use: an approval binds to one
    /// exact operation hash and is spent on apply.
    machine ApprovalState / ApprovalEvent ("approval") {
        states { Pending, Approved, Rejected, Expired, Consumed, Revoked }
        events { Approve, Reject, Expire, Consume, Revoke }
        terminal { Rejected, Expired, Consumed, Revoked }
        transitions {
            Pending + Approve => Approved,
            Pending + Reject => Rejected,
            Pending + Expire => Expired,
            Approved + Consume => Consumed,
            Approved + Expire => Expired,
            Approved + Revoke => Revoked,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_happy_path_and_interruption() {
        let s = SessionState::Planned;
        let s = s.transition(SessionEvent::Prepare).unwrap();
        let s = s.transition(SessionEvent::Start).unwrap();
        let s = s.transition(SessionEvent::Pause).unwrap();
        let s = s.transition(SessionEvent::Resume).unwrap();
        let s = s.transition(SessionEvent::Interrupt).unwrap();
        assert_eq!(s, SessionState::Interrupted);
        assert!(
            s.transition(SessionEvent::Resume).is_err(),
            "interrupted sessions need recovery"
        );
        let s = s.transition(SessionEvent::Recover).unwrap();
        assert_eq!(s, SessionState::Paused);
        let s = s.transition(SessionEvent::Complete).unwrap();
        let s = s.transition(SessionEvent::Review).unwrap();
        let s = s.transition(SessionEvent::Close).unwrap();
        assert!(s.is_terminal());
        assert!(s.transition(SessionEvent::Start).is_err());
    }

    #[test]
    fn finalized_attempt_is_immutable() {
        let s = AssessmentAttemptState::ApprovedFinal;
        for ev in [
            AssessmentEvent::Begin,
            AssessmentEvent::Submit,
            AssessmentEvent::ReviewEvidence,
            AssessmentEvent::GradeProvisional,
            AssessmentEvent::ApproveFinal,
            AssessmentEvent::Void,
            AssessmentEvent::Abandon,
        ] {
            assert!(s.transition(ev).is_err(), "{ev:?}");
        }
        assert!(AssessmentAttemptState::Started
            .transition(AssessmentEvent::ApproveFinal)
            .is_err());
        assert!(AssessmentAttemptState::Submitted
            .transition(AssessmentEvent::ApproveFinal)
            .is_err());
    }

    #[test]
    fn skill_invocation_cannot_skip_preflight_or_loop() {
        assert!(SkillInvocationState::Requested
            .transition(SkillInvocationEvent::Run)
            .is_err());
        assert!(SkillInvocationState::Requested
            .transition(SkillInvocationEvent::Authorize)
            .is_err());
        for (_, _, to) in SkillInvocationState::TRANSITIONS {
            if *to == SkillInvocationState::Running {
                // The only way into Running is from Authorized.
            }
        }
        let into_running: Vec<_> = SkillInvocationState::TRANSITIONS
            .iter()
            .filter(|(_, _, to)| *to == SkillInvocationState::Running)
            .map(|(from, _, _)| *from)
            .collect();
        assert_eq!(into_running, vec![SkillInvocationState::Authorized]);
    }

    #[test]
    fn approval_is_single_use() {
        let a = ApprovalState::Pending
            .transition(ApprovalEvent::Approve)
            .unwrap();
        let c = a.transition(ApprovalEvent::Consume).unwrap();
        assert_eq!(c, ApprovalState::Consumed);
        assert!(c.transition(ApprovalEvent::Consume).is_err());
        assert!(ApprovalState::Pending
            .transition(ApprovalEvent::Consume)
            .is_err());
    }

    #[test]
    fn competency_never_jumps_to_validated_without_independent_evidence() {
        assert!(CompetencyState::Unassessed
            .transition(CompetencyEvent::Validate)
            .is_err());
        assert!(CompetencyState::Guided
            .transition(CompetencyEvent::Validate)
            .is_err());
        assert!(CompetencyState::Independent
            .transition(CompetencyEvent::Validate)
            .is_ok());
    }

    #[test]
    fn backup_retained_only_after_restore_test() {
        assert!(BackupState::Checked
            .transition(BackupEvent::Retain)
            .is_err());
        assert!(BackupState::RestoreTested
            .transition(BackupEvent::Retain)
            .is_ok());
    }

    #[test]
    fn every_non_terminal_state_has_an_outgoing_edge() {
        for s in SessionState::ALL {
            if !s.is_terminal() {
                assert!(
                    SessionState::TRANSITIONS.iter().any(|(f, _, _)| f == s),
                    "{s:?}"
                );
            }
        }
        for s in InterventionState::ALL {
            if !s.is_terminal() {
                assert!(
                    InterventionState::TRANSITIONS
                        .iter()
                        .any(|(f, _, _)| f == s),
                    "{s:?}"
                );
            }
        }
        for s in SourceState::ALL {
            if !s.is_terminal() {
                assert!(
                    SourceState::TRANSITIONS.iter().any(|(f, _, _)| f == s),
                    "{s:?}"
                );
            }
        }
    }

    #[test]
    fn display_uses_screaming_snake_case() {
        assert_eq!(SessionState::Running.to_string(), "RUNNING");
        assert_eq!(
            AssessmentAttemptState::GradedProvisional.to_string(),
            "GRADED_PROVISIONAL"
        );
    }
}
