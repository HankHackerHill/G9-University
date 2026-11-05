//! Actors: who is asking. Only the operator (and deterministic system routines
//! acting on operator-approved commands) can be authoritative. Skills and the
//! model are proposal-only actors (handoff §6, §12).

use crate::ids::{ActorId, SkillId};
use crate::trust::TrustClass;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActorKind {
    /// The single human operator of this installation.
    Operator,
    /// Deterministic backend routine (migrations, backups, timers) executing an
    /// approved or scheduled operation.
    System,
    /// A logical skill (task package) executing under the operator's authority
    /// but with a narrower permitted scope.
    Skill { skill_id: SkillId },
    /// The local language model. Never authoritative.
    Model,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Actor {
    pub id: ActorId,
    pub kind: ActorKind,
}

impl Actor {
    pub fn operator(id: ActorId) -> Self {
        Self {
            id,
            kind: ActorKind::Operator,
        }
    }

    pub fn system(id: ActorId) -> Self {
        Self {
            id,
            kind: ActorKind::System,
        }
    }

    pub fn skill(id: ActorId, skill_id: SkillId) -> Self {
        Self {
            id,
            kind: ActorKind::Skill { skill_id },
        }
    }

    pub fn model(id: ActorId) -> Self {
        Self {
            id,
            kind: ActorKind::Model,
        }
    }

    pub fn is_operator(&self) -> bool {
        matches!(self.kind, ActorKind::Operator)
    }

    /// Trust class implied by the actor kind.
    pub fn trust(&self) -> TrustClass {
        match self.kind {
            ActorKind::Operator => TrustClass::Operator,
            ActorKind::System => TrustClass::SystemDeterministic,
            ActorKind::Skill { .. } => TrustClass::SkillPackage,
            ActorKind::Model => TrustClass::ModelOutput,
        }
    }
}
