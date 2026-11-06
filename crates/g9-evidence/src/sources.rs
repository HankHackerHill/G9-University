//! Research source records (M15; table `source_registry`; handoff §14 and
//! §16 "Research source" machine; R-016).
//!
//! A [`SourceRecord`] wraps the `g9_domain::SourceState` machine
//! (`UNTRUSTED → QUARANTINED → EVALUATED → APPROVED_REFERENCE →
//! SUPERSEDED/REVOKED`) with the governance fields §14 requires before any
//! fetch is approved: exact domain/resource, resource class, allowed data,
//! frequency, local cache handling, version and license.
//!
//! Trust is **derived from state**, never set directly:
//!
//! | State | `TrustClass` |
//! |---|---|
//! | `ApprovedReference` | `ApprovedReference` |
//! | `Evaluated`, `Superseded` | `ImportedDocument` (data; historical for `Superseded`) |
//! | `Untrusted`, `Quarantined`, `Revoked` | `Unknown` |
//!
//! Actor rules: the `Model` actor can neither register nor transition a
//! source; `Approve`, `Supersede` and `Revoke` require the `Operator`;
//! `Quarantine`, `Evaluate` and `Reject` may also be performed by `System`
//! or a `Skill` (they only move a source *away from* authority). Network
//! fetching itself is a `Connect` operation decided in `g9-policy`; nothing
//! here fetches anything.

use crate::attachments::actor_kind_label;
use crate::error::EvidenceError;
use g9_domain::state::SourceEvent;
use g9_domain::{Actor, ActorId, ActorKind, SourceId, SourceState, TrustClass, UtcMillis};
use serde::{Deserialize, Serialize};

/// Maximum length of `domain_or_resource` in characters.
pub const MAX_RESOURCE_CHARS: usize = 512;
/// Maximum length of the other free-text governance fields in characters.
pub const MAX_FIELD_CHARS: usize = 2048;

/// Class of an external resource (handoff §14 "exact domain/resource class").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResourceClass {
    /// Vendor or project documentation site.
    Documentation,
    /// Normative specification (RFC, W3C, 1EdTech).
    Specification,
    /// Standard or framework (OWASP, NIST, MITRE).
    StandardOrFramework,
    /// Dataset or fixture collection.
    Dataset,
    /// Source-code repository.
    Repository,
    /// Article, blog post or announcement.
    Article,
    /// Book or long-form course material.
    BookOrCourse,
    /// Tool download or release page.
    ToolRelease,
    /// Anything else; must be described in `allowed_data`.
    Other,
}

/// Governance fields supplied at registration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFields {
    /// Exact domain or resource (e.g. `owasp.org/www-project-web-security-testing-guide`).
    pub domain_or_resource: String,
    /// Resource class.
    pub resource_class: ResourceClass,
    /// What may be fetched/stored from it (may be empty until evaluation).
    pub allowed_data: String,
    /// How often it may be fetched (may be empty until evaluation).
    pub frequency: String,
    /// License of the material (may be empty until evaluation).
    pub license: String,
    /// Version of the material (may be empty until evaluation).
    pub version: String,
    /// Local cache handling (may be empty until evaluation).
    pub cache_handling: String,
}

/// One applied transition, kept in [`SourceRecord::history`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceTransitionRecord {
    /// Source identifier.
    pub source_id: SourceId,
    /// State before.
    pub from: SourceState,
    /// Applied event.
    pub event: SourceEvent,
    /// State after.
    pub to: SourceState,
    /// Acting actor id.
    pub actor_id: ActorId,
    /// Acting actor kind.
    pub actor_kind: ActorKind,
    /// When.
    pub at: UtcMillis,
}

/// A research source and its governance state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRecord {
    /// Source identifier.
    pub id: SourceId,
    /// Exact domain or resource.
    pub domain_or_resource: String,
    /// Resource class.
    pub resource_class: ResourceClass,
    /// What may be fetched/stored.
    pub allowed_data: String,
    /// Fetch frequency.
    pub frequency: String,
    /// License.
    pub license: String,
    /// Version.
    pub version: String,
    /// Local cache handling.
    pub cache_handling: String,
    /// Lifecycle state.
    pub state: SourceState,
    /// Trust derived from `state`.
    pub trust: TrustClass,
    /// Who registered the source.
    pub registered_by: ActorId,
    /// When it was registered.
    pub registered_at: UtcMillis,
    /// Operator who last approved/superseded/revoked it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_by: Option<ActorId>,
    /// When that review happened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_at: Option<UtcMillis>,
    /// Replacement source once superseded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<SourceId>,
    /// Applied transitions, oldest first.
    #[serde(default)]
    pub history: Vec<SourceTransitionRecord>,
}

impl SourceRecord {
    /// Register a source in `Untrusted`. The `Model` actor may not register.
    pub fn register(
        id: SourceId,
        fields: SourceFields,
        actor: &Actor,
        now: UtcMillis,
    ) -> Result<Self, EvidenceError> {
        if matches!(actor.kind, ActorKind::Model) {
            return Err(EvidenceError::ActorNotPermitted {
                actor_kind: "MODEL".to_string(),
                operation: "register a research source",
            });
        }
        validate_field(
            "domain_or_resource",
            &fields.domain_or_resource,
            MAX_RESOURCE_CHARS,
            true,
        )?;
        validate_field("allowed_data", &fields.allowed_data, MAX_FIELD_CHARS, false)?;
        validate_field("frequency", &fields.frequency, MAX_FIELD_CHARS, false)?;
        validate_field("license", &fields.license, MAX_FIELD_CHARS, false)?;
        validate_field("version", &fields.version, MAX_FIELD_CHARS, false)?;
        validate_field(
            "cache_handling",
            &fields.cache_handling,
            MAX_FIELD_CHARS,
            false,
        )?;
        let state = SourceState::Untrusted;
        Ok(Self {
            id,
            domain_or_resource: fields.domain_or_resource,
            resource_class: fields.resource_class,
            allowed_data: fields.allowed_data,
            frequency: fields.frequency,
            license: fields.license,
            version: fields.version,
            cache_handling: fields.cache_handling,
            state,
            trust: Self::trust_for_state(state),
            registered_by: actor.id.clone(),
            registered_at: now,
            reviewed_by: None,
            reviewed_at: None,
            superseded_by: None,
            history: Vec::new(),
        })
    }

    /// Trust class implied by a lifecycle state.
    pub const fn trust_for_state(state: SourceState) -> TrustClass {
        match state {
            SourceState::ApprovedReference => TrustClass::ApprovedReference,
            SourceState::Evaluated | SourceState::Superseded => TrustClass::ImportedDocument,
            SourceState::Untrusted | SourceState::Quarantined | SourceState::Revoked => {
                TrustClass::Unknown
            }
        }
    }

    /// Whether the source may be cited as an approved reference.
    pub fn is_approved_reference(&self) -> bool {
        self.state == SourceState::ApprovedReference
    }

    /// Governance fields that are still empty (all must be filled before `Approve`).
    pub fn missing_for_approval(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        for (name, value) in [
            ("allowed_data", &self.allowed_data),
            ("frequency", &self.frequency),
            ("license", &self.license),
            ("version", &self.version),
            ("cache_handling", &self.cache_handling),
        ] {
            if value.trim().is_empty() {
                missing.push(name);
            }
        }
        missing
    }

    /// Update the governance fields (allowed while not yet approved; after
    /// approval a change is a new source that supersedes this one).
    pub fn update_fields(
        &mut self,
        fields: SourceFields,
        actor: &Actor,
    ) -> Result<(), EvidenceError> {
        if matches!(actor.kind, ActorKind::Model) {
            return Err(EvidenceError::ActorNotPermitted {
                actor_kind: "MODEL".to_string(),
                operation: "edit a research source",
            });
        }
        if matches!(
            self.state,
            SourceState::ApprovedReference | SourceState::Superseded | SourceState::Revoked
        ) {
            return Err(EvidenceError::SourceInconsistent {
                reason: "governance fields of an approved, superseded or revoked source are immutable; register a superseding source",
            });
        }
        validate_field(
            "domain_or_resource",
            &fields.domain_or_resource,
            MAX_RESOURCE_CHARS,
            true,
        )?;
        validate_field("allowed_data", &fields.allowed_data, MAX_FIELD_CHARS, false)?;
        validate_field("frequency", &fields.frequency, MAX_FIELD_CHARS, false)?;
        validate_field("license", &fields.license, MAX_FIELD_CHARS, false)?;
        validate_field("version", &fields.version, MAX_FIELD_CHARS, false)?;
        validate_field(
            "cache_handling",
            &fields.cache_handling,
            MAX_FIELD_CHARS,
            false,
        )?;
        self.domain_or_resource = fields.domain_or_resource;
        self.resource_class = fields.resource_class;
        self.allowed_data = fields.allowed_data;
        self.frequency = fields.frequency;
        self.license = fields.license;
        self.version = fields.version;
        self.cache_handling = fields.cache_handling;
        Ok(())
    }

    /// Apply a lifecycle event with actor enforcement. On success the state,
    /// derived trust, review fields and history are updated atomically (the
    /// record is unchanged on any error).
    pub fn transition(
        &mut self,
        event: SourceEvent,
        actor: &Actor,
        now: UtcMillis,
    ) -> Result<SourceTransitionRecord, EvidenceError> {
        let permitted = match (event, &actor.kind) {
            (_, ActorKind::Model) => false,
            (SourceEvent::Approve | SourceEvent::Supersede | SourceEvent::Revoke, kind) => {
                matches!(kind, ActorKind::Operator)
            }
            (SourceEvent::Quarantine | SourceEvent::Evaluate | SourceEvent::Reject, _) => true,
        };
        if !permitted {
            return Err(EvidenceError::ActorNotPermitted {
                actor_kind: actor_kind_label(&actor.kind).to_string(),
                operation: event_operation(event),
            });
        }
        let from = self.state;
        let to = from.transition(event)?;
        if event == SourceEvent::Approve {
            let missing = self.missing_for_approval();
            if !missing.is_empty() {
                return Err(EvidenceError::SourceIncomplete { missing });
            }
        }
        self.state = to;
        self.trust = Self::trust_for_state(to);
        if matches!(
            event,
            SourceEvent::Approve | SourceEvent::Supersede | SourceEvent::Revoke
        ) {
            self.reviewed_by = Some(actor.id.clone());
            self.reviewed_at = Some(now);
        }
        let record = SourceTransitionRecord {
            source_id: self.id.clone(),
            from,
            event,
            to,
            actor_id: actor.id.clone(),
            actor_kind: actor.kind.clone(),
            at: now,
        };
        self.history.push(record.clone());
        Ok(record)
    }

    /// Supersede this source with `replacement` (operator only).
    pub fn supersede_with(
        &mut self,
        replacement: SourceId,
        actor: &Actor,
        now: UtcMillis,
    ) -> Result<SourceTransitionRecord, EvidenceError> {
        if replacement == self.id {
            return Err(EvidenceError::SelfSupersede);
        }
        let record = self.transition(SourceEvent::Supersede, actor, now)?;
        self.superseded_by = Some(replacement);
        Ok(record)
    }

    /// Check the derived-field invariants (for records loaded from storage).
    pub fn verify_invariants(&self) -> Result<(), EvidenceError> {
        if self.trust != Self::trust_for_state(self.state) {
            return Err(EvidenceError::SourceInconsistent {
                reason: "trust must be derived from state",
            });
        }
        if self.state == SourceState::ApprovedReference && self.reviewed_by.is_none() {
            return Err(EvidenceError::SourceInconsistent {
                reason: "an approved reference records its reviewer",
            });
        }
        if self.state == SourceState::ApprovedReference && !self.missing_for_approval().is_empty() {
            return Err(EvidenceError::SourceInconsistent {
                reason: "an approved reference has every governance field",
            });
        }
        if self.superseded_by.is_some() && self.state != SourceState::Superseded {
            return Err(EvidenceError::SourceInconsistent {
                reason: "only a superseded source names a replacement",
            });
        }
        if let Some(last) = self.history.last() {
            if last.to != self.state {
                return Err(EvidenceError::SourceInconsistent {
                    reason: "state must equal the last history entry",
                });
            }
        } else if self.state != SourceState::Untrusted {
            return Err(EvidenceError::SourceInconsistent {
                reason: "a source with no history is UNTRUSTED",
            });
        }
        Ok(())
    }
}

const fn event_operation(event: SourceEvent) -> &'static str {
    match event {
        SourceEvent::Quarantine => "quarantine a research source",
        SourceEvent::Evaluate => "evaluate a research source",
        SourceEvent::Approve => "approve a research source",
        SourceEvent::Supersede => "supersede a research source",
        SourceEvent::Revoke => "revoke a research source",
        SourceEvent::Reject => "reject a research source",
    }
}

fn validate_field(
    field: &'static str,
    value: &str,
    max_chars: usize,
    required: bool,
) -> Result<(), EvidenceError> {
    if required && value.trim().is_empty() {
        return Err(EvidenceError::SourceFieldInvalid {
            field,
            reason: "must not be empty",
        });
    }
    if value.chars().count() > max_chars {
        return Err(EvidenceError::SourceFieldInvalid {
            field,
            reason: "longer than the limit",
        });
    }
    if value
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
    {
        return Err(EvidenceError::SourceFieldInvalid {
            field,
            reason: "contains control characters",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::SkillId;
    use pretty_assertions::assert_eq;

    fn t(ms: i64) -> UtcMillis {
        UtcMillis::from_millis(ms)
    }

    fn operator() -> Actor {
        Actor::operator(ActorId::parse("operator").unwrap())
    }

    fn skill() -> Actor {
        Actor::skill(
            ActorId::parse("skill-k01").unwrap(),
            SkillId::parse("g9-research-curator").unwrap(),
        )
    }

    fn model() -> Actor {
        Actor::model(ActorId::parse("local-model").unwrap())
    }

    fn fields(complete: bool) -> SourceFields {
        SourceFields {
            domain_or_resource: "owasp.org/www-project-web-security-testing-guide".into(),
            resource_class: ResourceClass::StandardOrFramework,
            allowed_data: if complete {
                "HTML pages of the stable guide".into()
            } else {
                String::new()
            },
            frequency: if complete {
                "monthly".into()
            } else {
                String::new()
            },
            license: if complete {
                "CC BY-SA 4.0".into()
            } else {
                String::new()
            },
            version: if complete {
                "v4.2".into()
            } else {
                String::new()
            },
            cache_handling: if complete {
                "local cache under approved root; purge on revoke".into()
            } else {
                String::new()
            },
        }
    }

    fn evaluated(complete: bool) -> SourceRecord {
        let mut s = SourceRecord::register(
            SourceId::parse("src-1").unwrap(),
            fields(complete),
            &skill(),
            t(1),
        )
        .unwrap();
        s.transition(SourceEvent::Quarantine, &skill(), t(2))
            .unwrap();
        s.transition(SourceEvent::Evaluate, &skill(), t(3)).unwrap();
        s
    }

    #[test]
    fn source_state_requires_operator_for_approval() {
        let mut s = evaluated(true);
        assert_eq!(s.state, SourceState::Evaluated);
        assert_eq!(s.trust, TrustClass::ImportedDocument);
        assert!(!s.is_approved_reference());

        // Skill, system and model actors cannot approve.
        for actor in [
            skill(),
            Actor::system(ActorId::parse("sys").unwrap()),
            model(),
        ] {
            let before = s.clone();
            let err = s
                .transition(SourceEvent::Approve, &actor, t(4))
                .unwrap_err();
            assert!(
                matches!(err, EvidenceError::ActorNotPermitted { .. }),
                "{actor:?}"
            );
            assert_eq!(s, before, "record unchanged on refusal");
        }
        assert_eq!(s.trust, TrustClass::ImportedDocument);

        // The operator can.
        let rec = s
            .transition(SourceEvent::Approve, &operator(), t(5))
            .unwrap();
        assert_eq!(rec.from, SourceState::Evaluated);
        assert_eq!(rec.to, SourceState::ApprovedReference);
        assert_eq!(s.state, SourceState::ApprovedReference);
        assert_eq!(s.trust, TrustClass::ApprovedReference);
        assert!(s.is_approved_reference());
        assert_eq!(s.reviewed_by, Some(ActorId::parse("operator").unwrap()));
        assert_eq!(s.reviewed_at, Some(t(5)));
        assert_eq!(s.history.len(), 3);
        assert!(s.verify_invariants().is_ok());

        // Supersede and revoke are operator-only as well.
        let mut s2 = s.clone();
        assert!(matches!(
            s2.supersede_with(SourceId::parse("src-2").unwrap(), &skill(), t(6)),
            Err(EvidenceError::ActorNotPermitted { .. })
        ));
        assert!(matches!(
            s2.transition(SourceEvent::Revoke, &skill(), t(6)),
            Err(EvidenceError::ActorNotPermitted { .. })
        ));
        assert_eq!(
            s2.supersede_with(SourceId::parse("src-1").unwrap(), &operator(), t(6)),
            Err(EvidenceError::SelfSupersede)
        );
        s2.supersede_with(SourceId::parse("src-2").unwrap(), &operator(), t(6))
            .unwrap();
        assert_eq!(s2.state, SourceState::Superseded);
        assert_eq!(s2.trust, TrustClass::ImportedDocument);
        assert_eq!(s2.superseded_by, Some(SourceId::parse("src-2").unwrap()));
        assert!(s2.verify_invariants().is_ok());
        s2.transition(SourceEvent::Revoke, &operator(), t(7))
            .unwrap();
        assert_eq!(s2.trust, TrustClass::Unknown);
        assert!(s2.state.is_terminal());
        assert!(matches!(
            s2.transition(SourceEvent::Approve, &operator(), t(8)),
            Err(EvidenceError::Transition(_))
        ));
    }

    #[test]
    fn approval_requires_complete_governance_fields_and_legal_edge() {
        let mut s = evaluated(false);
        let err = s
            .transition(SourceEvent::Approve, &operator(), t(4))
            .unwrap_err();
        assert_eq!(
            err,
            EvidenceError::SourceIncomplete {
                missing: vec![
                    "allowed_data",
                    "frequency",
                    "license",
                    "version",
                    "cache_handling"
                ]
            }
        );
        assert_eq!(s.state, SourceState::Evaluated);
        s.update_fields(fields(true), &operator()).unwrap();
        assert!(s
            .transition(SourceEvent::Approve, &operator(), t(5))
            .is_ok());
        // Fields are frozen after approval.
        assert!(matches!(
            s.update_fields(fields(true), &operator()),
            Err(EvidenceError::SourceInconsistent { .. })
        ));

        // Approve is only legal from EVALUATED, even for the operator.
        let mut fresh = SourceRecord::register(
            SourceId::parse("src-3").unwrap(),
            fields(true),
            &operator(),
            t(1),
        )
        .unwrap();
        assert_eq!(fresh.state, SourceState::Untrusted);
        assert_eq!(fresh.trust, TrustClass::Unknown);
        assert!(matches!(
            fresh.transition(SourceEvent::Approve, &operator(), t(2)),
            Err(EvidenceError::Transition(_))
        ));
        assert!(fresh.verify_invariants().is_ok());
    }

    #[test]
    fn model_actor_cannot_register_or_transition() {
        assert!(matches!(
            SourceRecord::register(
                SourceId::parse("src-4").unwrap(),
                fields(true),
                &model(),
                t(1)
            ),
            Err(EvidenceError::ActorNotPermitted { .. })
        ));
        let mut s = SourceRecord::register(
            SourceId::parse("src-4").unwrap(),
            fields(true),
            &skill(),
            t(1),
        )
        .unwrap();
        assert!(matches!(
            s.transition(SourceEvent::Quarantine, &model(), t(2)),
            Err(EvidenceError::ActorNotPermitted { .. })
        ));
        assert!(matches!(
            s.update_fields(fields(true), &model()),
            Err(EvidenceError::ActorNotPermitted { .. })
        ));
        // Reject returns an evaluated source to quarantine (away from authority).
        s.transition(SourceEvent::Quarantine, &skill(), t(2))
            .unwrap();
        s.transition(SourceEvent::Evaluate, &skill(), t(3)).unwrap();
        s.transition(SourceEvent::Reject, &skill(), t(4)).unwrap();
        assert_eq!(s.state, SourceState::Quarantined);
        assert_eq!(s.trust, TrustClass::Unknown);
    }

    #[test]
    fn trust_is_derived_from_every_state() {
        for state in SourceState::ALL {
            let trust = SourceRecord::trust_for_state(*state);
            assert_eq!(
                trust == TrustClass::ApprovedReference,
                *state == SourceState::ApprovedReference
            );
            assert!(
                !trust.may_be_authoritative(),
                "{state:?}: a source is never an authority"
            );
        }
        let mut s = evaluated(true);
        s.trust = TrustClass::Operator;
        assert!(matches!(
            s.verify_invariants(),
            Err(EvidenceError::SourceInconsistent { .. })
        ));
        let mut s = evaluated(true);
        s.state = SourceState::ApprovedReference;
        s.trust = TrustClass::ApprovedReference;
        assert!(
            s.verify_invariants().is_err(),
            "approved without reviewer/history"
        );
    }

    #[test]
    fn source_fields_are_validated() {
        let mut f = fields(true);
        f.domain_or_resource = String::new();
        assert!(matches!(
            SourceRecord::register(SourceId::parse("s").unwrap(), f, &operator(), t(1)),
            Err(EvidenceError::SourceFieldInvalid {
                field: "domain_or_resource",
                ..
            })
        ));
        let mut f = fields(true);
        f.license = "x\u{0}".into();
        assert!(matches!(
            SourceRecord::register(SourceId::parse("s").unwrap(), f, &operator(), t(1)),
            Err(EvidenceError::SourceFieldInvalid {
                field: "license",
                ..
            })
        ));
        let mut f = fields(true);
        f.allowed_data = "a".repeat(MAX_FIELD_CHARS + 1);
        assert!(matches!(
            SourceRecord::register(SourceId::parse("s").unwrap(), f, &operator(), t(1)),
            Err(EvidenceError::SourceFieldInvalid {
                field: "allowed_data",
                ..
            })
        ));
    }

    #[test]
    fn source_serde_round_trip() {
        let s = evaluated(true);
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["state"], "EVALUATED");
        assert_eq!(json["trust"], "IMPORTED_DOCUMENT");
        assert_eq!(json["resource_class"], "STANDARD_OR_FRAMEWORK");
        assert_eq!(json["history"][0]["event"], "QUARANTINE");
        let back: SourceRecord = serde_json::from_value(json).unwrap();
        assert_eq!(back, s);
    }
}
