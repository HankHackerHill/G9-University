//! Notes and note revisions (M05; tables `notes`, `note_versions`).
//!
//! A [`Note`] is a metadata record; its content is **not** held here. Every
//! revision appends a [`NoteVersion`] carrying the content hash, byte length,
//! provenance and timestamp, and the crate hands the bytes back to the caller
//! as a [`ContentRecord`] so that `g9-storage` can store them content-addressed
//! (`ab/cd/<sha256>`, see [`crate::attachments::content_address`]).
//!
//! Invariants (checked by [`Note::verify_invariants`]):
//!
//! - versions are append-only and numbered `1..=n` without gaps;
//! - `current_version` is the last version number;
//! - a redacted version keeps its hash and length (tombstone) and has no
//!   content pointer; redaction is single-shot and never reversed here
//!   (a restore is a new revision).
//!
//! Erasure requests (`deletion_requests`, A10) are commands in `g9-app`; this
//! crate only enforces that redaction is performed by the operator or by the
//! deterministic system actor executing an approved request.

use crate::attachments::content_address;
use crate::error::EvidenceError;
use g9_domain::{
    Actor, ActorId, ActorKind, Classification, DataDomain, NoteId, Provenance, UtcMillis,
};
use serde::{Deserialize, Serialize};

/// Upper bound for a single note revision (4 MiB). Larger text belongs in an
/// attachment.
pub const MAX_NOTE_BYTES: u64 = 4 * 1024 * 1024;

/// Content handed back to the caller for storage. The crate never retains it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentRecord {
    /// Hex SHA-256 of `bytes`.
    pub sha256: String,
    /// Length of `bytes`.
    pub byte_len: u64,
    /// Content-addressed relative path `ab/cd/<sha256>`.
    pub relpath: String,
    /// The content itself.
    pub bytes: Vec<u8>,
}

impl ContentRecord {
    /// Hash non-empty content of at most [`MAX_NOTE_BYTES`].
    pub fn new(bytes: Vec<u8>) -> Result<Self, EvidenceError> {
        if bytes.is_empty() {
            return Err(EvidenceError::EmptyContent);
        }
        let byte_len = bytes.len() as u64;
        if byte_len > MAX_NOTE_BYTES {
            return Err(EvidenceError::ContentTooLarge {
                actual: byte_len,
                limit: MAX_NOTE_BYTES,
            });
        }
        let address = content_address(&bytes);
        Ok(Self {
            sha256: address.sha256,
            byte_len,
            relpath: address.relpath,
            bytes,
        })
    }
}

/// One immutable revision of a note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteVersion {
    /// 1-based, strictly increasing without gaps.
    pub version: u32,
    /// Hex SHA-256 of the content as captured; retained after redaction.
    pub content_sha256: String,
    /// Length of the content as captured; retained after redaction.
    pub byte_len: u64,
    /// Where this revision came from; `content_sha256` is filled in by the crate.
    pub provenance: Provenance,
    /// When the revision was recorded.
    pub created_at: UtcMillis,
    /// Whether the content has been erased (tombstone).
    pub redacted: bool,
    /// Content-addressed pointer (`ab/cd/<sha256>`); `None` once redacted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_ref: Option<String>,
    /// When the content was redacted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redacted_at: Option<UtcMillis>,
    /// Who redacted the content.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redacted_by: Option<ActorId>,
}

/// Tombstone returned by [`Note::redact`]: enough to keep audit continuity
/// (hash, length) without the content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionTombstone {
    /// Note identifier.
    pub note_id: NoteId,
    /// Redacted version number.
    pub version: u32,
    /// Hex SHA-256 of the erased content.
    pub content_sha256: String,
    /// Length of the erased content.
    pub byte_len: u64,
    /// Content-addressed pointer that storage must now delete.
    pub released_content_ref: String,
    /// When the redaction happened.
    pub redacted_at: UtcMillis,
    /// Who redacted.
    pub redacted_by: ActorId,
}

/// Metadata for [`Note::create`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteMeta {
    /// Note identifier.
    pub id: NoteId,
    /// Privacy domain (R-017).
    pub data_domain: DataDomain,
    /// Declared classification. Use
    /// [`crate::classification::ContentClassification::effective`] to combine
    /// it with the detected classification before calling.
    pub classification: Classification,
    /// Creating actor.
    pub created_by: ActorId,
    /// Creation instant.
    pub created_at: UtcMillis,
    /// Provenance of the first revision.
    pub provenance: Provenance,
}

/// A note: metadata plus its append-only revision ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    /// Note identifier.
    pub id: NoteId,
    /// Privacy domain.
    pub data_domain: DataDomain,
    /// Classification of the note as a whole (never lowered by a revision).
    pub classification: Classification,
    /// Creating actor.
    pub created_by: ActorId,
    /// Creation instant.
    pub created_at: UtcMillis,
    /// Number of the latest revision.
    pub current_version: u32,
    /// Append-only revision ledger.
    pub versions: Vec<NoteVersion>,
}

impl Note {
    /// Create a note with its first revision. Returns the note and the content
    /// record the caller must store.
    pub fn create(
        content: impl Into<Vec<u8>>,
        meta: NoteMeta,
    ) -> Result<(Note, ContentRecord), EvidenceError> {
        let record = ContentRecord::new(content.into())?;
        let version = Self::version_from(1, &record, meta.provenance, meta.created_at);
        let note = Note {
            id: meta.id,
            data_domain: meta.data_domain,
            classification: meta.classification,
            created_by: meta.created_by,
            created_at: meta.created_at,
            current_version: 1,
            versions: vec![version],
        };
        Ok((note, record))
    }

    fn version_from(
        version: u32,
        record: &ContentRecord,
        mut provenance: Provenance,
        created_at: UtcMillis,
    ) -> NoteVersion {
        provenance.content_sha256 = Some(record.sha256.clone());
        NoteVersion {
            version,
            content_sha256: record.sha256.clone(),
            byte_len: record.byte_len,
            provenance,
            created_at,
            redacted: false,
            content_ref: Some(record.relpath.clone()),
            redacted_at: None,
            redacted_by: None,
        }
    }

    /// Append a new revision. Earlier revisions are untouched. The note's
    /// classification is raised to `classification` if that is stricter and
    /// never lowered.
    pub fn revise(
        &mut self,
        content: impl Into<Vec<u8>>,
        provenance: Provenance,
        classification: Classification,
        now: UtcMillis,
    ) -> Result<ContentRecord, EvidenceError> {
        self.verify_invariants()?;
        let record = ContentRecord::new(content.into())?;
        let next = self.current_version.saturating_add(1);
        self.versions
            .push(Self::version_from(next, &record, provenance, now));
        self.current_version = next;
        self.classification = self.classification.max(classification);
        Ok(record)
    }

    /// Redact one revision: the hash and length stay as a tombstone, the
    /// content pointer is cleared, and the caller receives the pointer that
    /// storage must delete. Only the operator or the deterministic system actor
    /// (executing an approved `deletion_requests` row) may redact.
    pub fn redact(
        &mut self,
        version: u32,
        actor: &Actor,
        now: UtcMillis,
    ) -> Result<RedactionTombstone, EvidenceError> {
        require_operator_or_system(actor, "redact note content")?;
        self.verify_invariants()?;
        let note_id = self.id.clone();
        let v = self
            .versions
            .iter_mut()
            .find(|v| v.version == version)
            .ok_or(EvidenceError::VersionNotFound { version })?;
        if v.redacted {
            return Err(EvidenceError::AlreadyRedacted { version });
        }
        let released = v
            .content_ref
            .take()
            .unwrap_or_else(|| content_relpath(&v.content_sha256));
        v.redacted = true;
        v.redacted_at = Some(now);
        v.redacted_by = Some(actor.id.clone());
        Ok(RedactionTombstone {
            note_id,
            version,
            content_sha256: v.content_sha256.clone(),
            byte_len: v.byte_len,
            released_content_ref: released,
            redacted_at: now,
            redacted_by: actor.id.clone(),
        })
    }

    /// Redact every revision that is not yet redacted (erasure request).
    pub fn redact_all(
        &mut self,
        actor: &Actor,
        now: UtcMillis,
    ) -> Result<Vec<RedactionTombstone>, EvidenceError> {
        require_operator_or_system(actor, "redact note content")?;
        let pending: Vec<u32> = self
            .versions
            .iter()
            .filter(|v| !v.redacted)
            .map(|v| v.version)
            .collect();
        pending
            .into_iter()
            .map(|version| self.redact(version, actor, now))
            .collect()
    }

    /// The latest revision, if the ledger is non-empty.
    pub fn current(&self) -> Option<&NoteVersion> {
        self.versions.last()
    }

    /// A specific revision.
    pub fn version(&self, version: u32) -> Option<&NoteVersion> {
        self.versions.iter().find(|v| v.version == version)
    }

    /// Whether every revision has been redacted.
    pub fn is_fully_redacted(&self) -> bool {
        !self.versions.is_empty() && self.versions.iter().all(|v| v.redacted)
    }

    /// Check the append-only ledger invariants (for records loaded from storage).
    pub fn verify_invariants(&self) -> Result<(), EvidenceError> {
        if self.versions.is_empty() {
            return Err(EvidenceError::VersionLedgerInconsistent {
                index: 0,
                reason: "a note has at least one version",
            });
        }
        for (index, v) in self.versions.iter().enumerate() {
            let expected = index as u32 + 1;
            if v.version != expected {
                return Err(EvidenceError::VersionLedgerInconsistent {
                    index,
                    reason: "version numbers must be 1..=n without gaps",
                });
            }
            if v.content_sha256.len() != 64
                || !v.content_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            {
                return Err(EvidenceError::VersionLedgerInconsistent {
                    index,
                    reason: "content_sha256 must be 64 hex characters",
                });
            }
            if v.provenance.content_sha256.as_deref() != Some(v.content_sha256.as_str()) {
                return Err(EvidenceError::VersionLedgerInconsistent {
                    index,
                    reason: "provenance hash must equal the version hash",
                });
            }
            if v.redacted {
                if v.content_ref.is_some() || v.redacted_at.is_none() || v.redacted_by.is_none() {
                    return Err(EvidenceError::VersionLedgerInconsistent {
                        index,
                        reason: "a redacted version has no content pointer and records who/when",
                    });
                }
            } else if v.content_ref.as_deref() != Some(content_relpath(&v.content_sha256).as_str())
                || v.redacted_at.is_some()
                || v.redacted_by.is_some()
            {
                return Err(EvidenceError::VersionLedgerInconsistent {
                    index,
                    reason:
                        "a live version points at its content address and has no redaction fields",
                });
            }
        }
        if self.current_version != self.versions.len() as u32 {
            return Err(EvidenceError::VersionLedgerInconsistent {
                index: self.versions.len() - 1,
                reason: "current_version must equal the last version number",
            });
        }
        Ok(())
    }
}

/// Content-addressed relative path for a hex digest.
fn content_relpath(sha256: &str) -> String {
    format!("{}/{}/{}", &sha256[0..2], &sha256[2..4], sha256)
}

fn require_operator_or_system(actor: &Actor, operation: &'static str) -> Result<(), EvidenceError> {
    match actor.kind {
        ActorKind::Operator | ActorKind::System => Ok(()),
        ActorKind::Skill { .. } => Err(EvidenceError::ActorNotPermitted {
            actor_kind: "SKILL".to_string(),
            operation,
        }),
        ActorKind::Model => Err(EvidenceError::ActorNotPermitted {
            actor_kind: "MODEL".to_string(),
            operation,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::{SkillId, SourceKind, TrustClass};
    use pretty_assertions::assert_eq;

    fn t(ms: i64) -> UtcMillis {
        UtcMillis::from_millis(ms)
    }

    fn operator() -> Actor {
        Actor::operator(ActorId::parse("operator").unwrap())
    }

    fn prov(trust: TrustClass, at: i64) -> Provenance {
        Provenance {
            source_kind: SourceKind::OperatorInput,
            source_ref: "ui:notes".into(),
            trust,
            content_sha256: None,
            recorded_at: t(at),
            recorded_by: ActorId::parse("operator").unwrap(),
            version: None,
        }
    }

    fn meta() -> NoteMeta {
        NoteMeta {
            id: NoteId::parse("note-1").unwrap(),
            data_domain: DataDomain::Learning,
            classification: Classification::Internal,
            created_by: ActorId::parse("operator").unwrap(),
            created_at: t(1_000),
            provenance: prov(TrustClass::Operator, 1_000),
        }
    }

    #[test]
    fn note_versions_append_only_and_hash_stable() {
        let (mut note, rec1) = Note::create("first draft", meta()).unwrap();
        assert_eq!(note.current_version, 1);
        assert_eq!(rec1.byte_len, 11);
        assert_eq!(rec1.sha256, g9_domain::hashing::sha256_hex(b"first draft"));
        assert_eq!(
            rec1.relpath,
            format!(
                "{}/{}/{}",
                &rec1.sha256[0..2],
                &rec1.sha256[2..4],
                rec1.sha256
            )
        );
        let v1_before = note.versions[0].clone();
        assert_eq!(
            v1_before.provenance.content_sha256.as_deref(),
            Some(rec1.sha256.as_str())
        );

        let rec2 = note
            .revise(
                "second draft",
                prov(TrustClass::Operator, 2_000),
                Classification::Internal,
                t(2_000),
            )
            .unwrap();
        assert_eq!(note.current_version, 2);
        assert_eq!(note.versions.len(), 2);
        assert_eq!(
            note.versions[0], v1_before,
            "earlier versions are untouched"
        );
        assert_eq!(note.versions[1].version, 2);
        assert_ne!(rec1.sha256, rec2.sha256);

        // Revising with identical content yields the identical hash (stable).
        let rec3 = note
            .revise(
                "first draft",
                prov(TrustClass::Operator, 3_000),
                Classification::Internal,
                t(3_000),
            )
            .unwrap();
        assert_eq!(rec3.sha256, rec1.sha256);
        assert_eq!(note.versions.len(), 3);
        assert!(note.verify_invariants().is_ok());

        // There is no API to remove or rewrite a version; the ledger check
        // refuses a tampered copy.
        let mut tampered = note.clone();
        tampered.versions.remove(1);
        assert!(matches!(
            tampered.verify_invariants(),
            Err(EvidenceError::VersionLedgerInconsistent { index: 1, .. })
        ));
        let mut rehashed = note.clone();
        rehashed.versions[0].content_sha256 = "ab".repeat(32);
        assert!(rehashed.verify_invariants().is_err());

        // Classification is only ever raised by a revision.
        let mut n = note.clone();
        n.revise(
            "x",
            prov(TrustClass::Operator, 4_000),
            Classification::Private,
            t(4_000),
        )
        .unwrap();
        assert_eq!(n.classification, Classification::Private);
        n.revise(
            "y",
            prov(TrustClass::Operator, 5_000),
            Classification::Public,
            t(5_000),
        )
        .unwrap();
        assert_eq!(n.classification, Classification::Private);
    }

    #[test]
    fn redaction_keeps_tombstone_hash() {
        let (mut note, rec1) = Note::create("private reflection", meta()).unwrap();
        note.revise(
            "second",
            prov(TrustClass::Operator, 2_000),
            Classification::Internal,
            t(2_000),
        )
        .unwrap();
        let tomb = note.redact(1, &operator(), t(9_000)).unwrap();
        assert_eq!(tomb.content_sha256, rec1.sha256);
        assert_eq!(tomb.byte_len, rec1.byte_len);
        assert_eq!(tomb.released_content_ref, rec1.relpath);
        assert_eq!(tomb.version, 1);
        let v1 = note.version(1).unwrap();
        assert!(v1.redacted);
        assert_eq!(v1.content_sha256, rec1.sha256, "hash survives as tombstone");
        assert_eq!(v1.byte_len, rec1.byte_len);
        assert_eq!(v1.content_ref, None, "content pointer cleared");
        assert_eq!(v1.redacted_at, Some(t(9_000)));
        assert_eq!(v1.redacted_by, Some(ActorId::parse("operator").unwrap()));
        assert!(!note.version(2).unwrap().redacted);
        assert_eq!(note.versions.len(), 2, "redaction never removes a row");
        assert!(note.verify_invariants().is_ok());
        assert!(!note.is_fully_redacted());

        // Single-shot.
        assert_eq!(
            note.redact(1, &operator(), t(9_001)),
            Err(EvidenceError::AlreadyRedacted { version: 1 })
        );
        assert_eq!(
            note.redact(7, &operator(), t(9_001)),
            Err(EvidenceError::VersionNotFound { version: 7 })
        );

        // Erasure of everything.
        let tombs = note.redact_all(&operator(), t(9_002)).unwrap();
        assert_eq!(tombs.len(), 1);
        assert!(note.is_fully_redacted());
        assert!(note.verify_invariants().is_ok());
    }

    #[test]
    fn redaction_requires_operator_or_system_actor() {
        let (mut note, _) = Note::create("x", meta()).unwrap();
        let model = Actor::model(ActorId::parse("local-model").unwrap());
        let skill = Actor::skill(
            ActorId::parse("skill-actor").unwrap(),
            SkillId::parse("g9-journal").unwrap(),
        );
        assert!(matches!(
            note.redact(1, &model, t(1)),
            Err(EvidenceError::ActorNotPermitted { .. })
        ));
        assert!(matches!(
            note.redact_all(&skill, t(1)),
            Err(EvidenceError::ActorNotPermitted { .. })
        ));
        assert!(!note.versions[0].redacted);
        let system = Actor::system(ActorId::parse("deletion-worker").unwrap());
        assert!(note.redact(1, &system, t(1)).is_ok());
    }

    #[test]
    fn create_rejects_empty_and_oversized_content() {
        assert_eq!(
            Note::create(Vec::<u8>::new(), meta()).err(),
            Some(EvidenceError::EmptyContent)
        );
        let big = vec![b'a'; MAX_NOTE_BYTES as usize + 1];
        assert!(matches!(
            Note::create(big, meta()).err(),
            Some(EvidenceError::ContentTooLarge { .. })
        ));
    }

    #[test]
    fn note_serde_round_trip_preserves_ledger() {
        let (mut note, _) = Note::create("a", meta()).unwrap();
        note.revise(
            "b",
            prov(TrustClass::ImportedDocument, 2),
            Classification::Internal,
            t(2),
        )
        .unwrap();
        note.redact(1, &operator(), t(3)).unwrap();
        let json = serde_json::to_string(&note).unwrap();
        let back: Note = serde_json::from_str(&json).unwrap();
        assert_eq!(back, note);
        assert!(back.verify_invariants().is_ok());
        assert!(
            !json.contains("\"bytes\""),
            "content never serializes with the note"
        );
    }
}
