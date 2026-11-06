//! Session record (M04) wrapping `g9_domain::SessionState` with actor,
//! timestamp and A03 semantics.
//!
//! Rules:
//! - only the operator may drive a session; the deterministic system actor may
//!   only raise `Interrupt` (process restart found an open interval); skills and
//!   the model may not transition sessions at all;
//! - `Recover` (Interrupted → Paused) goes through [`SessionRecord::recover`],
//!   which refuses while the timer ledger still holds an uncorrected
//!   `Uncertain` interval for the session: unverified time is never carried
//!   forward silently;
//! - every transition appends a [`SessionEventRow`] and bumps the revision.

use crate::timer::Ledger;
use g9_domain::state::SessionEvent;
use g9_domain::{
    Actor, ActorKind, DataDomain, Revision, SessionId, SessionState, TransitionError, UtcMillis,
};
use serde::{Deserialize, Serialize};

/// One applied transition (persisted as a `session_events` row).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionEventRow {
    /// Sequence within the session (0-based).
    pub seq: u32,
    /// Session identifier.
    pub session_id: SessionId,
    /// Event applied.
    pub event: SessionEvent,
    /// State before.
    pub from: SessionState,
    /// State after.
    pub to: SessionState,
    /// Who applied it.
    pub actor: Actor,
    /// When (wall clock; ordering is by `seq`).
    pub occurred_at: UtcMillis,
}

/// Session record errors.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    /// The state machine refused the edge.
    #[error(transparent)]
    Transition(#[from] TransitionError),
    /// The actor kind may not raise this event.
    #[error("actor kind {actor_kind} may not raise session event {event}")]
    ActorNotPermitted {
        /// Event name.
        event: String,
        /// Actor kind label.
        actor_kind: String,
    },
    /// Recovery refused: uncorrected uncertain time exists.
    #[error(
        "session {session_id} has uncertain time that needs an operator correction before recovery"
    )]
    UncorrectedUncertainTime {
        /// Session identifier.
        session_id: SessionId,
    },
    /// `Recover` must be applied through `SessionRecord::recover` with the ledger.
    #[error("RECOVER requires the timer ledger check; use SessionRecord::recover")]
    RecoverRequiresLedgerCheck,
}

impl SessionError {
    /// Stable machine-readable error code.
    pub const fn code(&self) -> &'static str {
        match self {
            SessionError::Transition(_) => "session.transition_refused",
            SessionError::ActorNotPermitted { .. } => "session.actor_not_permitted",
            SessionError::UncorrectedUncertainTime { .. } => "session.uncorrected_uncertain_time",
            SessionError::RecoverRequiresLedgerCheck => "session.recover_requires_ledger_check",
        }
    }
}

const fn actor_kind_label(actor: &Actor) -> &'static str {
    match actor.kind {
        ActorKind::Operator => "OPERATOR",
        ActorKind::System => "SYSTEM",
        ActorKind::Skill { .. } => "SKILL",
        ActorKind::Model => "MODEL",
    }
}

fn actor_may_raise(actor: &Actor, event: SessionEvent) -> bool {
    match actor.kind {
        ActorKind::Operator => true,
        ActorKind::System => matches!(event, SessionEvent::Interrupt),
        ActorKind::Skill { .. } | ActorKind::Model => false,
    }
}

/// A study session with its state, revision and event history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    /// Session identifier.
    pub id: SessionId,
    /// Current state.
    pub state: SessionState,
    /// Optimistic concurrency revision.
    pub revision: Revision,
    /// Privacy domain (always `Learning`).
    pub data_domain: DataDomain,
    /// Applied transitions.
    pub events: Vec<SessionEventRow>,
}

impl SessionRecord {
    /// New session in `Planned`.
    pub fn new(id: SessionId) -> Self {
        Self {
            id,
            state: SessionState::Planned,
            revision: Revision::INITIAL,
            data_domain: DataDomain::Learning,
            events: Vec::new(),
        }
    }

    fn apply(
        &mut self,
        event: SessionEvent,
        actor: &Actor,
        now: UtcMillis,
    ) -> Result<SessionEventRow, SessionError> {
        if !actor_may_raise(actor, event) {
            return Err(SessionError::ActorNotPermitted {
                event: format!("{event:?}"),
                actor_kind: actor_kind_label(actor).to_string(),
            });
        }
        let from = self.state;
        let to = from.transition(event)?;
        let row = SessionEventRow {
            seq: u32::try_from(self.events.len()).unwrap_or(u32::MAX),
            session_id: self.id.clone(),
            event,
            from,
            to,
            actor: actor.clone(),
            occurred_at: now,
        };
        self.state = to;
        self.revision = self.revision.next();
        self.events.push(row.clone());
        Ok(row)
    }

    /// Apply an event. `Recover` is refused here; use [`SessionRecord::recover`].
    pub fn transition(
        &mut self,
        event: SessionEvent,
        actor: &Actor,
        now: UtcMillis,
    ) -> Result<SessionEventRow, SessionError> {
        if event == SessionEvent::Recover {
            return Err(SessionError::RecoverRequiresLedgerCheck);
        }
        self.apply(event, actor, now)
    }

    /// Recover an interrupted session (→ `Paused`). Refused while the ledger
    /// holds an uncorrected `Uncertain` interval for this session.
    pub fn recover(
        &mut self,
        actor: &Actor,
        now: UtcMillis,
        ledger: &Ledger,
    ) -> Result<SessionEventRow, SessionError> {
        if ledger.has_uncorrected_uncertain(&self.id) {
            return Err(SessionError::UncorrectedUncertainTime {
                session_id: self.id.clone(),
            });
        }
        self.apply(SessionEvent::Recover, actor, now)
    }

    /// Whether the session is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        self.state.is_terminal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::ActorId;
    use pretty_assertions::assert_eq;

    fn op() -> Actor {
        Actor::operator(ActorId::parse("op").unwrap())
    }

    fn sys() -> Actor {
        Actor::system(ActorId::parse("sys").unwrap())
    }

    #[test]
    fn happy_path_appends_rows_and_bumps_revision() {
        let mut s = SessionRecord::new(SessionId::parse("s1").unwrap());
        let t = UtcMillis::from_millis(1);
        for ev in [
            SessionEvent::Prepare,
            SessionEvent::Start,
            SessionEvent::Pause,
            SessionEvent::Resume,
            SessionEvent::Complete,
            SessionEvent::Review,
            SessionEvent::Close,
        ] {
            s.transition(ev, &op(), t).unwrap();
        }
        assert_eq!(s.state, SessionState::Closed);
        assert_eq!(s.revision, Revision::new(8));
        assert_eq!(s.events.len(), 7);
        assert_eq!(s.events[1].from, SessionState::Ready);
        assert_eq!(s.events[1].to, SessionState::Running);
        assert_eq!(
            s.events.iter().map(|e| e.seq).collect::<Vec<_>>(),
            (0..7).collect::<Vec<_>>()
        );
        assert!(s.is_terminal());
        assert!(matches!(
            s.transition(SessionEvent::Start, &op(), t).unwrap_err(),
            SessionError::Transition(_)
        ));
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["state"], "CLOSED");
        assert_eq!(json["events"][0]["event"], "PREPARE");
    }

    #[test]
    fn skill_and_model_cannot_drive_sessions_and_system_only_interrupts() {
        let mut s = SessionRecord::new(SessionId::parse("s1").unwrap());
        let t = UtcMillis::from_millis(1);
        let model = Actor::model(ActorId::parse("m").unwrap());
        let skill = Actor::skill(
            ActorId::parse("k").unwrap(),
            g9_domain::SkillId::parse("g9-x").unwrap(),
        );
        assert!(matches!(
            s.transition(SessionEvent::Prepare, &model, t).unwrap_err(),
            SessionError::ActorNotPermitted { .. }
        ));
        assert!(matches!(
            s.transition(SessionEvent::Prepare, &skill, t).unwrap_err(),
            SessionError::ActorNotPermitted { .. }
        ));
        assert!(matches!(
            s.transition(SessionEvent::Prepare, &sys(), t).unwrap_err(),
            SessionError::ActorNotPermitted { .. }
        ));
        s.transition(SessionEvent::Prepare, &op(), t).unwrap();
        s.transition(SessionEvent::Start, &op(), t).unwrap();
        s.transition(SessionEvent::Interrupt, &sys(), t).unwrap();
        assert_eq!(s.state, SessionState::Interrupted);
        assert_eq!(s.revision, Revision::new(4));
    }

    #[test]
    fn recover_requires_ledger_path() {
        let mut s = SessionRecord::new(SessionId::parse("s1").unwrap());
        let t = UtcMillis::from_millis(1);
        s.transition(SessionEvent::Prepare, &op(), t).unwrap();
        s.transition(SessionEvent::Start, &op(), t).unwrap();
        s.transition(SessionEvent::Interrupt, &op(), t).unwrap();
        assert_eq!(
            s.transition(SessionEvent::Recover, &op(), t).unwrap_err(),
            SessionError::RecoverRequiresLedgerCheck
        );
        let ledger = Ledger::new();
        s.recover(&op(), t, &ledger).unwrap();
        assert_eq!(s.state, SessionState::Paused);
    }
}
