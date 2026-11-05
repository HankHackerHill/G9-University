//! Privacy domains and classifications (handoff R-017, §7, §12). Records carry a
//! data domain so that cross-domain retrieval can be denied by policy.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DataDomain {
    /// Curriculum, sessions, assessments, competencies, retention, interventions.
    Learning,
    /// Goals, projects, milestones, decisions, commitments.
    Business,
    /// Opt-in reflections, capacity self-reports, protected rest.
    Personal,
    /// Security-sensitive material: lab authorizations, incident data, credentials metadata.
    SecuritySensitive,
    /// Application state: settings, migrations, backups, policy, audit.
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Classification {
    Public,
    Internal,
    Private,
    Secret,
}

impl Classification {
    /// Content at or above this level must never be included in model context or
    /// exported without an explicit approval.
    pub const fn requires_approval_for_disclosure(self) -> bool {
        matches!(self, Classification::Private | Classification::Secret)
    }
}
