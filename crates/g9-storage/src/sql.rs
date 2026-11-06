//! Small internal helpers shared by the modules: enum ↔ TEXT mapping that
//! mirrors the serde `SCREAMING_SNAKE_CASE` names used by the CHECK lists,
//! optional-column decoding and actor (de)composition.

use crate::error::{StorageError, StorageResult};
use g9_domain::{Actor, ActorId, ActorKind, SkillId, UtcMillis};
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Serialize a unit-variant enum to the TEXT value stored in a CHECK column.
pub(crate) fn enum_to_text<T: Serialize>(value: &T) -> StorageResult<String> {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => Ok(s),
        Ok(other) => Err(StorageError::InvalidInput(format!(
            "enum did not serialize to a string: {other}"
        ))),
        Err(e) => Err(StorageError::InvalidInput(format!(
            "enum serialization failed: {e}"
        ))),
    }
}

/// Parse a TEXT column back into its enum.
pub(crate) fn enum_from_text<T: DeserializeOwned>(text: &str) -> StorageResult<T> {
    serde_json::from_value(serde_json::Value::String(text.to_string()))
        .map_err(|_| StorageError::RowDecode(format!("unexpected enum text {text:?}")))
}

/// Decompose an actor into the `(actor_id, actor_kind, actor_skill_id)` columns.
pub(crate) fn actor_columns(actor: &Actor) -> (String, &'static str, Option<String>) {
    let (kind, skill) = match &actor.kind {
        ActorKind::Operator => ("OPERATOR", None),
        ActorKind::System => ("SYSTEM", None),
        ActorKind::Skill { skill_id } => ("SKILL", Some(skill_id.as_str().to_string())),
        ActorKind::Model => ("MODEL", None),
    };
    (actor.id.as_str().to_string(), kind, skill)
}

/// Rebuild an actor from its columns.
pub(crate) fn actor_from_columns(
    id: &str,
    kind: &str,
    skill_id: Option<&str>,
) -> StorageResult<Actor> {
    let id = ActorId::parse(id).map_err(|e| StorageError::RowDecode(e.to_string()))?;
    let kind = match (kind, skill_id) {
        ("OPERATOR", _) => ActorKind::Operator,
        ("SYSTEM", _) => ActorKind::System,
        ("MODEL", _) => ActorKind::Model,
        ("SKILL", Some(s)) => ActorKind::Skill {
            skill_id: SkillId::parse(s).map_err(|e| StorageError::RowDecode(e.to_string()))?,
        },
        ("SKILL", None) => {
            return Err(StorageError::RowDecode(
                "SKILL actor without skill id".into(),
            ))
        }
        (other, _) => {
            return Err(StorageError::RowDecode(format!(
                "unknown actor kind {other:?}"
            )))
        }
    };
    Ok(Actor { id, kind })
}

/// Convert an optional epoch-ms column.
pub(crate) fn opt_millis(v: Option<i64>) -> Option<UtcMillis> {
    v.map(UtcMillis::from_millis)
}

/// Parse a typed id column, reporting decode failures as `RowDecode`.
pub(crate) fn parse_id<T>(text: &str) -> StorageResult<T>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    text.parse::<T>()
        .map_err(|e| StorageError::RowDecode(format!("invalid id in row: {e}")))
}

/// Parse JSON stored in a `json_valid()` column.
pub(crate) fn json_from_text(text: &str) -> StorageResult<serde_json::Value> {
    serde_json::from_str(text)
        .map_err(|e| StorageError::RowDecode(format!("invalid json column: {e}")))
}

/// Validate a hex SHA-256 string before it reaches a `length(x) = 64` CHECK.
pub(crate) fn require_sha256(field: &'static str, value: &str) -> StorageResult<()> {
    if value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(StorageError::InvalidInput(format!(
            "{field} must be a 64-char hex sha256"
        )))
    }
}
