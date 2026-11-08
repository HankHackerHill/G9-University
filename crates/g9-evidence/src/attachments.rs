//! Attachments: approved roots, media-type allow-list, content addressing
//! (M05; table `attachments`; handoff §12 "arbitrary paths constrained to
//! approved roots"; defect register SEC-13 attachment policy).
//!
//! # Approved roots
//!
//! [`ApprovedRoots::resolve`] is a **lexical** check: it never touches the
//! filesystem. It rejects `..` components, empty components, NUL and other
//! control bytes, backslashes, non-UTF-8 and absolute paths outside every
//! root. Symlinks cannot be detected lexically: **the storage layer must call
//! `std::fs::canonicalize` on the resolved path and run it through
//! [`ApprovedRoots::contains`] again before opening it** (a symlink inside a
//! root that points outside it is otherwise invisible).
//!
//! # Media types
//!
//! The allow-list is `text/markdown`, `text/plain`, `application/pdf`
//! (stored only, never parsed in-app), `image/png`, `image/jpeg`,
//! `application/json`, `text/csv`. Anything else is
//! [`MediaType::Other`] and the attachment is **quarantined** until an
//! operator classifies it. Executables, archives and packet captures are
//! refused outright (SEC-13): the caller receives the hash and must store a
//! written report instead. Declared types are checked against the first bytes
//! where a signature exists; a mismatch quarantines the attachment.

use crate::error::EvidenceError;
use g9_domain::hashing::sha256_hex;
use g9_domain::{Actor, ActorKind, AttachmentId, Classification, Provenance};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

/// Upper bound for a single attachment (128 MiB).
pub const MAX_ATTACHMENT_BYTES: u64 = 128 * 1024 * 1024;

/// Longest accepted path in bytes.
pub const MAX_PATH_BYTES: usize = 4096;

/// Why a path was rejected by [`ApprovedRoots`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PathRejection {
    /// The path is empty or resolves to no component.
    #[error("path is empty")]
    Empty,
    /// The path exceeds [`MAX_PATH_BYTES`].
    #[error("path is longer than the limit")]
    TooLong,
    /// The path contains a NUL byte.
    #[error("path contains a NUL byte")]
    NulByte,
    /// The path contains another control character.
    #[error("path contains a control character")]
    ControlChar,
    /// The path is not valid UTF-8.
    #[error("path is not valid UTF-8")]
    InvalidEncoding,
    /// The path contains a backslash (not a separator on this platform; refused to avoid ambiguity).
    #[error("path contains a backslash")]
    Backslash,
    /// The path contains a `..` component.
    #[error("path contains a parent-directory component")]
    ParentTraversal,
    /// The path contains an empty component (`a//b`, trailing `/`).
    #[error("path contains an empty component")]
    EmptyComponent,
    /// An absolute path is not under any approved root.
    #[error("path is outside every approved root")]
    OutsideApprovedRoots,
    /// No roots were configured.
    #[error("no approved roots configured")]
    NoRoots,
    /// A configured root is not absolute.
    #[error("approved root must be absolute")]
    RootNotAbsolute,
}

/// A path accepted by [`ApprovedRoots::resolve`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedPath {
    /// The approved root the path lives under.
    pub root: PathBuf,
    /// Lexically normalized path relative to `root`.
    pub relative: PathBuf,
    /// `root` joined with `relative`.
    pub absolute: PathBuf,
}

/// The set of directories attachments may live in. Deserialization goes
/// through [`ApprovedRoots::new`], so a stored configuration is re-validated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<PathBuf>", into = "Vec<PathBuf>")]
pub struct ApprovedRoots {
    roots: Vec<PathBuf>,
}

impl TryFrom<Vec<PathBuf>> for ApprovedRoots {
    type Error = PathRejection;
    fn try_from(roots: Vec<PathBuf>) -> Result<Self, Self::Error> {
        ApprovedRoots::new(roots)
    }
}

impl From<ApprovedRoots> for Vec<PathBuf> {
    fn from(r: ApprovedRoots) -> Vec<PathBuf> {
        r.roots
    }
}

impl ApprovedRoots {
    /// Configure roots. Each root must be absolute and lexically clean (no
    /// `..`, empty components or control bytes; one trailing `/` is
    /// tolerated). The first root is the primary root against which relative
    /// paths are resolved.
    pub fn new(roots: Vec<PathBuf>) -> Result<Self, PathRejection> {
        if roots.is_empty() {
            return Err(PathRejection::NoRoots);
        }
        let mut clean = Vec::with_capacity(roots.len());
        for root in roots {
            let (absolute, components) = normalize(&root, true)?;
            if !absolute {
                return Err(PathRejection::RootNotAbsolute);
            }
            if components.is_empty() {
                return Err(PathRejection::Empty);
            }
            clean.push(join_absolute(&components));
        }
        Ok(Self { roots: clean })
    }

    /// The configured roots (normalized).
    pub fn roots(&self) -> &[PathBuf] {
        &self.roots
    }

    /// Lexically resolve `path`: a relative path is placed under the primary
    /// root; an absolute path must be under one of the roots. No filesystem
    /// access; see the module docs for the symlink caveat.
    pub fn resolve(&self, path: &Path) -> Result<ResolvedPath, PathRejection> {
        let (absolute, components) = normalize(path, false)?;
        if components.is_empty() {
            return Err(PathRejection::Empty);
        }
        if !absolute {
            let root = self.roots.first().ok_or(PathRejection::NoRoots)?.clone();
            let relative: PathBuf = components.iter().collect();
            let absolute = root.join(&relative);
            return Ok(ResolvedPath {
                root,
                relative,
                absolute,
            });
        }
        let candidate = join_absolute(&components);
        for root in &self.roots {
            if let Ok(rel) = candidate.strip_prefix(root) {
                if rel.as_os_str().is_empty() {
                    // The root itself is a directory, not an attachment.
                    return Err(PathRejection::Empty);
                }
                return Ok(ResolvedPath {
                    root: root.clone(),
                    relative: rel.to_path_buf(),
                    absolute: candidate,
                });
            }
        }
        Err(PathRejection::OutsideApprovedRoots)
    }

    /// Whether `path` resolves inside an approved root.
    ///
    /// This is the check for **files** (attachments): a path that *is* a root
    /// is a directory, not an attachment, and is refused. Use
    /// [`contains_directory`](Self::contains_directory) for directories.
    pub fn contains(&self, path: &Path) -> bool {
        self.resolve(path).is_ok()
    }

    /// Whether the absolute directory `dir` is an approved root or lies inside
    /// one. Backup, scratch and attachment directories are configured as roots
    /// themselves, so the root-equals case must be accepted here (it is
    /// rejected by [`contains`](Self::contains), which resolves files).
    /// Components are compared, not string prefixes: `/x/backups-evil` is not
    /// inside `/x/backups`. Lexical only; no filesystem access.
    pub fn contains_directory(&self, dir: &Path) -> bool {
        let Ok((absolute, components)) = normalize(dir, true) else {
            return false;
        };
        if !absolute || components.is_empty() {
            return false;
        }
        let candidate = join_absolute(&components);
        self.roots.iter().any(|root| candidate.starts_with(root))
    }
}

/// Lexical normalization: returns `(is_absolute, components)`. A single
/// trailing `/` is accepted only when `directory` is set (roots are
/// directories; attachments are files).
fn normalize(path: &Path, directory: bool) -> Result<(bool, Vec<String>), PathRejection> {
    let s = path.to_str().ok_or(PathRejection::InvalidEncoding)?;
    if s.is_empty() {
        return Err(PathRejection::Empty);
    }
    if s.len() > MAX_PATH_BYTES {
        return Err(PathRejection::TooLong);
    }
    if s.contains('\0') {
        return Err(PathRejection::NulByte);
    }
    if s.chars().any(|c| c.is_control()) {
        return Err(PathRejection::ControlChar);
    }
    if s.contains('\\') {
        return Err(PathRejection::Backslash);
    }
    let absolute = s.starts_with('/');
    let body = if absolute { &s[1..] } else { s };
    let body = match body.strip_suffix('/') {
        Some(trimmed) if directory => trimmed,
        _ => body,
    };
    let mut components = Vec::new();
    if body.is_empty() {
        return Ok((absolute, components));
    }
    for part in body.split('/') {
        match part {
            "" => return Err(PathRejection::EmptyComponent),
            "." => continue,
            ".." => return Err(PathRejection::ParentTraversal),
            other => components.push(other.to_string()),
        }
    }
    Ok((absolute, components))
}

fn join_absolute(components: &[String]) -> PathBuf {
    let mut p = PathBuf::from("/");
    for c in components {
        p.push(c);
    }
    p
}

/// Media type of an attachment: the allow-list plus a quarantined `Other`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum MediaType {
    /// `text/markdown`.
    TextMarkdown,
    /// `text/plain`.
    TextPlain,
    /// `application/pdf` — stored only, never parsed in-app.
    ApplicationPdf,
    /// `image/png`.
    ImagePng,
    /// `image/jpeg`.
    ImageJpeg,
    /// `application/json`.
    ApplicationJson,
    /// `text/csv`.
    TextCsv,
    /// Any other syntactically valid type (normalized, parameters removed).
    /// Attachments with this type are quarantined.
    Other(String),
}

impl MediaType {
    /// The allow-list in canonical spelling.
    pub const ALLOWED: [&'static str; 7] = [
        "text/markdown",
        "text/plain",
        "application/pdf",
        "image/png",
        "image/jpeg",
        "application/json",
        "text/csv",
    ];

    /// Declared types that are refused outright (never stored).
    pub const REFUSED: [&'static str; 12] = [
        "application/x-executable",
        "application/x-elf",
        "application/x-sharedlib",
        "application/x-msdownload",
        "application/x-dosexec",
        "application/vnd.microsoft.portable-executable",
        "application/x-mach-binary",
        "application/java-archive",
        "application/zip",
        "application/gzip",
        "application/x-tar",
        "application/vnd.tcpdump.pcap",
    ];

    /// Parse a declared media type: trimmed, lower-cased, parameters dropped,
    /// `type/subtype` syntax enforced. Unknown types become [`MediaType::Other`].
    pub fn parse(declared: &str) -> Result<Self, EvidenceError> {
        let essence = declared
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        if essence.is_empty() {
            return Err(EvidenceError::MediaTypeInvalid {
                reason: "empty media type",
            });
        }
        if essence.len() > 127 {
            return Err(EvidenceError::MediaTypeInvalid {
                reason: "media type longer than 127 bytes",
            });
        }
        let mut parts = essence.split('/');
        let (Some(top), Some(sub), None) = (parts.next(), parts.next(), parts.next()) else {
            return Err(EvidenceError::MediaTypeInvalid {
                reason: "media type must be type/subtype",
            });
        };
        let token_ok = |s: &str| {
            !s.is_empty()
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'+' | b'_'))
        };
        if !token_ok(top) || !token_ok(sub) {
            return Err(EvidenceError::MediaTypeInvalid {
                reason: "media type contains invalid characters",
            });
        }
        Ok(match essence.as_str() {
            "text/markdown" => MediaType::TextMarkdown,
            "text/plain" => MediaType::TextPlain,
            "application/pdf" => MediaType::ApplicationPdf,
            "image/png" => MediaType::ImagePng,
            "image/jpeg" | "image/jpg" => MediaType::ImageJpeg,
            "application/json" => MediaType::ApplicationJson,
            "text/csv" => MediaType::TextCsv,
            _ => MediaType::Other(essence),
        })
    }

    /// Canonical string form.
    pub fn as_str(&self) -> &str {
        match self {
            MediaType::TextMarkdown => "text/markdown",
            MediaType::TextPlain => "text/plain",
            MediaType::ApplicationPdf => "application/pdf",
            MediaType::ImagePng => "image/png",
            MediaType::ImageJpeg => "image/jpeg",
            MediaType::ApplicationJson => "application/json",
            MediaType::TextCsv => "text/csv",
            MediaType::Other(s) => s,
        }
    }

    /// Whether the type is on the allow-list.
    pub fn is_allowed(&self) -> bool {
        !matches!(self, MediaType::Other(_))
    }

    /// Whether the declared type is refused outright.
    pub fn is_refused(&self) -> bool {
        match self {
            MediaType::Other(s) => Self::REFUSED.contains(&s.as_str()),
            _ => false,
        }
    }

    /// Detect a signature in the first bytes, where one exists.
    pub fn sniff(bytes: &[u8]) -> Option<MediaType> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(MediaType::ImagePng)
        } else if bytes.starts_with(b"\xFF\xD8\xFF") {
            Some(MediaType::ImageJpeg)
        } else if bytes.starts_with(b"%PDF-") {
            Some(MediaType::ApplicationPdf)
        } else {
            None
        }
    }

    /// Whether `bytes` are consistent with this declared type. Binary types
    /// must carry their signature; text types must be UTF-8 without NUL; JSON
    /// must parse. `Other` is always "consistent" (it is quarantined anyway).
    pub fn consistent_with(&self, bytes: &[u8]) -> bool {
        match self {
            MediaType::ImagePng | MediaType::ImageJpeg | MediaType::ApplicationPdf => {
                Self::sniff(bytes).as_ref() == Some(self)
            }
            MediaType::TextMarkdown | MediaType::TextPlain | MediaType::TextCsv => {
                std::str::from_utf8(bytes).is_ok_and(|s| !s.contains('\0'))
            }
            MediaType::ApplicationJson => {
                serde_json::from_slice::<serde_json::Value>(bytes).is_ok()
            }
            MediaType::Other(_) => true,
        }
    }
}

impl fmt::Display for MediaType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl TryFrom<String> for MediaType {
    type Error = EvidenceError;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        MediaType::parse(&s)
    }
}

impl From<MediaType> for String {
    fn from(m: MediaType) -> String {
        m.as_str().to_string()
    }
}

/// Content families refused by byte signature regardless of the declared type.
fn refused_signature(bytes: &[u8]) -> Option<&'static str> {
    const SIGNATURES: &[(&[u8], &str)] = &[
        (b"\x7fELF", "ELF executable"),
        (b"MZ", "PE/DOS executable"),
        (b"\xfe\xed\xfa\xce", "Mach-O executable"),
        (b"\xfe\xed\xfa\xcf", "Mach-O executable"),
        (b"\xce\xfa\xed\xfe", "Mach-O executable"),
        (b"\xcf\xfa\xed\xfe", "Mach-O executable"),
        (b"\xca\xfe\xba\xbe", "Mach-O/Java class"),
        (b"PK\x03\x04", "ZIP archive"),
        (b"\x1f\x8b", "gzip archive"),
        (b"Rar!\x1a\x07", "RAR archive"),
        (b"7z\xbc\xaf\x27\x1c", "7-Zip archive"),
        (b"\xa1\xb2\xc3\xd4", "pcap capture"),
        (b"\xd4\xc3\xb2\xa1", "pcap capture"),
        (b"\xa1\xb2\x3c\x4d", "pcap capture"),
        (b"\x4d\x3c\xb2\xa1", "pcap capture"),
        (b"\x0a\x0d\x0d\x0a", "pcapng capture"),
    ];
    SIGNATURES
        .iter()
        .find(|(sig, _)| bytes.starts_with(sig))
        .map(|(_, name)| *name)
}

/// Why an attachment is quarantined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuarantineReason {
    /// The declared media type is not on the allow-list.
    MediaTypeNotAllowed,
    /// The bytes do not match the declared media type.
    DeclaredTypeMismatch,
}

/// Content address of a byte string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContentAddress {
    /// Hex SHA-256.
    pub sha256: String,
    /// `ab/cd/<sha256>` where `ab` and `cd` are the first two byte pairs of the hex digest.
    pub relpath: String,
}

/// Compute the content address of `bytes`.
pub fn content_address(bytes: &[u8]) -> ContentAddress {
    let sha256 = sha256_hex(bytes);
    let relpath = format!("{}/{}/{}", &sha256[0..2], &sha256[2..4], sha256);
    ContentAddress { sha256, relpath }
}

/// Reference to a stored attachment (table `attachments`). Bytes live in the
/// content-addressed store under an approved root; this record carries the
/// hash, size, type, classification and provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentRef {
    /// Attachment identifier.
    pub id: AttachmentId,
    /// Hex SHA-256 of the bytes.
    pub sha256: String,
    /// Size in bytes.
    pub byte_len: u64,
    /// Declared (normalized) media type.
    pub media_type: MediaType,
    /// Content-addressed relative path under the attachment root.
    pub stored_relpath: String,
    /// Classification; `Secret` content never reaches the model or an export without approval.
    pub classification: Classification,
    /// Provenance; `content_sha256` is filled in by the crate.
    pub provenance: Provenance,
    /// Quarantined until an operator classifies it.
    pub quarantined: bool,
    /// Why it is quarantined.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quarantine_reason: Option<QuarantineReason>,
}

impl AttachmentRef {
    /// Build a reference for `bytes` with a declared media type. Refuses
    /// empty, oversized, refused-type and refused-signature content; quarantines
    /// unknown types and declared/actual mismatches.
    pub fn new(
        id: AttachmentId,
        bytes: &[u8],
        declared_media_type: &str,
        classification: Classification,
        mut provenance: Provenance,
    ) -> Result<Self, EvidenceError> {
        if bytes.is_empty() {
            return Err(EvidenceError::EmptyContent);
        }
        let byte_len = bytes.len() as u64;
        if byte_len > MAX_ATTACHMENT_BYTES {
            return Err(EvidenceError::ContentTooLarge {
                actual: byte_len,
                limit: MAX_ATTACHMENT_BYTES,
            });
        }
        let media_type = MediaType::parse(declared_media_type)?;
        let address = content_address(bytes);
        if let Some(detected) = refused_signature(bytes) {
            return Err(EvidenceError::ContentRefused {
                sha256: address.sha256,
                detected,
            });
        }
        if media_type.is_refused() {
            return Err(EvidenceError::ContentRefused {
                sha256: address.sha256,
                detected: "refused declared media type",
            });
        }
        let quarantine_reason = if !media_type.is_allowed() {
            Some(QuarantineReason::MediaTypeNotAllowed)
        } else if !media_type.consistent_with(bytes) {
            Some(QuarantineReason::DeclaredTypeMismatch)
        } else {
            None
        };
        provenance.content_sha256 = Some(address.sha256.clone());
        Ok(Self {
            id,
            sha256: address.sha256,
            byte_len,
            media_type,
            stored_relpath: address.relpath,
            classification,
            provenance,
            quarantined: quarantine_reason.is_some(),
            quarantine_reason,
        })
    }

    /// Operator classification releases a quarantined attachment (SEC-13).
    /// The classification can only be kept or raised.
    pub fn release_from_quarantine(
        &mut self,
        actor: &Actor,
        classification: Classification,
    ) -> Result<(), EvidenceError> {
        if !matches!(actor.kind, ActorKind::Operator) {
            return Err(EvidenceError::ActorNotPermitted {
                actor_kind: actor_kind_label(&actor.kind).to_string(),
                operation: "release an attachment from quarantine",
            });
        }
        if !self.quarantined {
            return Err(EvidenceError::NotQuarantined);
        }
        self.classification = self.classification.max(classification);
        self.quarantined = false;
        self.quarantine_reason = None;
        Ok(())
    }
}

pub(crate) fn actor_kind_label(kind: &ActorKind) -> &'static str {
    match kind {
        ActorKind::Operator => "OPERATOR",
        ActorKind::System => "SYSTEM",
        ActorKind::Skill { .. } => "SKILL",
        ActorKind::Model => "MODEL",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_domain::{ActorId, SourceKind, TrustClass, UtcMillis};
    use pretty_assertions::assert_eq;

    fn roots() -> ApprovedRoots {
        ApprovedRoots::new(vec![
            PathBuf::from("/var/lib/g9/attachments"),
            PathBuf::from("/home/op/g9-imports/"),
        ])
        .unwrap()
    }

    fn prov() -> Provenance {
        Provenance {
            source_kind: SourceKind::Attachment,
            source_ref: "ui:attach".into(),
            trust: TrustClass::ImportedDocument,
            content_sha256: None,
            recorded_at: UtcMillis::from_millis(1),
            recorded_by: ActorId::parse("operator").unwrap(),
            version: None,
        }
    }

    #[test]
    fn contains_directory_accepts_a_root_itself_and_its_descendants_only() {
        let r = roots();
        // A root is a directory: accepted here, refused by `contains` (a file check).
        assert!(r.contains_directory(Path::new("/var/lib/g9/attachments")));
        assert!(!r.contains(Path::new("/var/lib/g9/attachments")));
        assert!(r.contains_directory(Path::new("/var/lib/g9/attachments/")));
        assert!(r.contains_directory(Path::new("/var/lib/g9/attachments/backups")));
        assert!(r.contains_directory(Path::new("/home/op/g9-imports")));
        // Outside, lookalike prefix, parent of a root, relative, traversal, empty,
        // backslash, NUL: all refused.
        assert!(!r.contains_directory(Path::new("/var/lib/g9")));
        assert!(!r.contains_directory(Path::new("/var/lib/g9/attachments-evil")));
        assert!(!r.contains_directory(Path::new("/var/lib/g9/attachments/../x")));
        assert!(!r.contains_directory(Path::new("/etc")));
        assert!(!r.contains_directory(Path::new("/")));
        assert!(!r.contains_directory(Path::new("attachments")));
        assert!(!r.contains_directory(Path::new("")));
        assert!(!r.contains_directory(Path::new("/var/lib/g9/attachments/a\\b")));
        assert!(!r.contains_directory(Path::new("/var/lib/g9/attachments/\0")));
    }

    #[test]
    fn approved_roots_reject_traversal_and_outside_paths() {
        let r = roots();
        assert_eq!(
            r.resolve(Path::new("../x")).unwrap_err(),
            PathRejection::ParentTraversal
        );
        assert_eq!(
            r.resolve(Path::new("a/../../x")).unwrap_err(),
            PathRejection::ParentTraversal
        );
        assert_eq!(
            r.resolve(Path::new("/etc/passwd")).unwrap_err(),
            PathRejection::OutsideApprovedRoots
        );
        let ok = r.resolve(Path::new("notes/ok.md")).unwrap();
        assert_eq!(ok.root, PathBuf::from("/var/lib/g9/attachments"));
        assert_eq!(ok.relative, PathBuf::from("notes/ok.md"));
        assert_eq!(
            ok.absolute,
            PathBuf::from("/var/lib/g9/attachments/notes/ok.md")
        );
        assert!(r.contains(Path::new("notes/ok.md")));
        assert_eq!(
            r.resolve(Path::new("notes/\0bad")).unwrap_err(),
            PathRejection::NulByte
        );
        assert!(!r.contains(Path::new("notes/\0bad")));

        // Absolute inside a root, including the second root.
        let abs = r
            .resolve(Path::new("/home/op/g9-imports/pdfs/x.pdf"))
            .unwrap();
        assert_eq!(abs.root, PathBuf::from("/home/op/g9-imports"));
        assert_eq!(abs.relative, PathBuf::from("pdfs/x.pdf"));
        // Prefix-string lookalike of a root is outside it.
        assert_eq!(
            r.resolve(Path::new("/var/lib/g9/attachments-evil/x"))
                .unwrap_err(),
            PathRejection::OutsideApprovedRoots
        );
        // Dot components normalize away; `..` inside an absolute path is refused
        // even when it would lexically stay inside the root.
        assert!(r.contains(Path::new("/var/lib/g9/attachments/./a/b")));
        assert_eq!(
            r.resolve(Path::new("/var/lib/g9/attachments/a/../b"))
                .unwrap_err(),
            PathRejection::ParentTraversal
        );
        // Empty components, backslashes, control characters, root itself, empty.
        assert_eq!(
            r.resolve(Path::new("a//b")).unwrap_err(),
            PathRejection::EmptyComponent
        );
        assert_eq!(
            r.resolve(Path::new("a/b/")).unwrap_err(),
            PathRejection::EmptyComponent
        );
        assert_eq!(
            r.resolve(Path::new("a\\b")).unwrap_err(),
            PathRejection::Backslash
        );
        assert_eq!(
            r.resolve(Path::new("a\u{1b}b")).unwrap_err(),
            PathRejection::ControlChar
        );
        assert_eq!(
            r.resolve(Path::new("/var/lib/g9/attachments")).unwrap_err(),
            PathRejection::Empty
        );
        assert_eq!(r.resolve(Path::new("")).unwrap_err(), PathRejection::Empty);
        assert_eq!(r.resolve(Path::new(".")).unwrap_err(), PathRejection::Empty);
        let long = "a/".repeat(MAX_PATH_BYTES) + "b";
        assert_eq!(
            r.resolve(Path::new(&long)).unwrap_err(),
            PathRejection::TooLong
        );
        // Non-UTF-8.
        use std::os::unix::ffi::OsStrExt;
        let bad = std::ffi::OsStr::from_bytes(b"notes/\xff.md");
        assert_eq!(
            r.resolve(Path::new(bad)).unwrap_err(),
            PathRejection::InvalidEncoding
        );
    }

    #[test]
    fn approved_roots_configuration_is_validated() {
        assert_eq!(
            ApprovedRoots::new(vec![]).unwrap_err(),
            PathRejection::NoRoots
        );
        assert_eq!(
            ApprovedRoots::new(vec![PathBuf::from("relative/root")]).unwrap_err(),
            PathRejection::RootNotAbsolute
        );
        assert_eq!(
            ApprovedRoots::new(vec![PathBuf::from("/a/../b")]).unwrap_err(),
            PathRejection::ParentTraversal
        );
        let r = ApprovedRoots::new(vec![PathBuf::from("/a/./b/")]).unwrap();
        assert_eq!(
            r.roots(),
            &[PathBuf::from("/a/b")],
            "one trailing slash on a root is tolerated"
        );
        assert_eq!(
            ApprovedRoots::new(vec![PathBuf::from("/a/b//")]).unwrap_err(),
            PathRejection::EmptyComponent
        );
        assert_eq!(
            ApprovedRoots::new(vec![PathBuf::from("/")]).unwrap_err(),
            PathRejection::Empty,
            "the filesystem root is never an approved root"
        );
        let r = ApprovedRoots::new(vec![PathBuf::from("/a/./b")]).unwrap();
        assert_eq!(r.roots(), &[PathBuf::from("/a/b")]);
        // Deserialization re-validates.
        let ok: ApprovedRoots = serde_json::from_str(r#"["/srv/g9/att/"]"#).unwrap();
        assert_eq!(ok.roots(), &[PathBuf::from("/srv/g9/att")]);
        assert!(serde_json::from_str::<ApprovedRoots>(r#"["srv/../x"]"#).is_err());
        assert!(serde_json::from_str::<ApprovedRoots>("[]").is_err());
    }

    #[test]
    fn media_type_allowlist_quarantines_unknown() {
        for allowed in MediaType::ALLOWED {
            let m = MediaType::parse(allowed).unwrap();
            assert!(m.is_allowed(), "{allowed}");
            assert_eq!(m.as_str(), allowed);
        }
        assert_eq!(
            MediaType::parse("Text/Markdown; charset=UTF-8").unwrap(),
            MediaType::TextMarkdown
        );
        assert_eq!(MediaType::parse("image/jpg").unwrap(), MediaType::ImageJpeg);
        let other = MediaType::parse("application/x-pcap").unwrap();
        assert_eq!(other, MediaType::Other("application/x-pcap".into()));
        assert!(!other.is_allowed());

        let unknown = AttachmentRef::new(
            AttachmentId::parse("att-1").unwrap(),
            b"not really anything",
            "application/octet-stream",
            Classification::Internal,
            prov(),
        )
        .unwrap();
        assert!(unknown.quarantined);
        assert_eq!(
            unknown.quarantine_reason,
            Some(QuarantineReason::MediaTypeNotAllowed)
        );
        assert_eq!(
            unknown.media_type,
            MediaType::Other("application/octet-stream".into())
        );

        let text = AttachmentRef::new(
            AttachmentId::parse("att-2").unwrap(),
            b"# Notes\n",
            "text/markdown",
            Classification::Internal,
            prov(),
        )
        .unwrap();
        assert!(!text.quarantined);
        assert_eq!(text.quarantine_reason, None);
        assert_eq!(
            text.provenance.content_sha256.as_deref(),
            Some(text.sha256.as_str())
        );

        // Malformed declared types are errors, not quarantine.
        for bad in [
            "",
            "text",
            "text/plain/extra",
            "te xt/plain",
            "text/pl\u{0}ain",
        ] {
            assert!(
                matches!(
                    MediaType::parse(bad),
                    Err(EvidenceError::MediaTypeInvalid { .. })
                ),
                "{bad:?}"
            );
        }
        // Serde round trip through the string form.
        let json = serde_json::to_string(&MediaType::Other("application/x-pcap".into())).unwrap();
        assert_eq!(json, "\"application/x-pcap\"");
        let back: MediaType = serde_json::from_str("\"IMAGE/PNG\"").unwrap();
        assert_eq!(back, MediaType::ImagePng);
        assert!(serde_json::from_str::<MediaType>("\"nonsense\"").is_err());
    }

    #[test]
    fn declared_type_mismatch_quarantines_and_signatures_refuse() {
        // Declared PNG, actually text.
        let a = AttachmentRef::new(
            AttachmentId::parse("att-3").unwrap(),
            b"hello",
            "image/png",
            Classification::Internal,
            prov(),
        )
        .unwrap();
        assert!(a.quarantined);
        assert_eq!(
            a.quarantine_reason,
            Some(QuarantineReason::DeclaredTypeMismatch)
        );
        // Real PNG header passes.
        let png = AttachmentRef::new(
            AttachmentId::parse("att-4").unwrap(),
            b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR",
            "image/png",
            Classification::Internal,
            prov(),
        )
        .unwrap();
        assert!(!png.quarantined);
        // JSON must parse; text must be UTF-8 without NUL.
        let bad_json = AttachmentRef::new(
            AttachmentId::parse("att-5").unwrap(),
            b"{not json",
            "application/json",
            Classification::Internal,
            prov(),
        )
        .unwrap();
        assert_eq!(
            bad_json.quarantine_reason,
            Some(QuarantineReason::DeclaredTypeMismatch)
        );
        let nul_text = AttachmentRef::new(
            AttachmentId::parse("att-6").unwrap(),
            b"abc\0def",
            "text/plain",
            Classification::Internal,
            prov(),
        )
        .unwrap();
        assert_eq!(
            nul_text.quarantine_reason,
            Some(QuarantineReason::DeclaredTypeMismatch)
        );
        // Executables, archives and captures are refused regardless of declared type.
        for (bytes, declared) in [
            (&b"\x7fELF\x02\x01\x01"[..], "text/plain"),
            (&b"MZ\x90\x00"[..], "application/octet-stream"),
            (&b"PK\x03\x04rest"[..], "application/json"),
            (&b"\x1f\x8b\x08"[..], "text/csv"),
            (&b"\xd4\xc3\xb2\xa1\x02\x00\x04\x00"[..], "image/png"),
            (&b"\x0a\x0d\x0d\x0a\x1c\x00\x00\x00"[..], "text/plain"),
        ] {
            let err = AttachmentRef::new(
                AttachmentId::parse("att-7").unwrap(),
                bytes,
                declared,
                Classification::Internal,
                prov(),
            )
            .unwrap_err();
            match err {
                EvidenceError::ContentRefused { sha256, .. } => {
                    assert_eq!(sha256, sha256_hex(bytes));
                }
                other => panic!("expected ContentRefused, got {other:?}"),
            }
        }
        let declared_refused = AttachmentRef::new(
            AttachmentId::parse("att-8").unwrap(),
            b"plain bytes",
            "application/zip",
            Classification::Internal,
            prov(),
        );
        assert!(matches!(
            declared_refused,
            Err(EvidenceError::ContentRefused { .. })
        ));
        // Empty and oversized.
        assert_eq!(
            AttachmentRef::new(
                AttachmentId::parse("att-9").unwrap(),
                b"",
                "text/plain",
                Classification::Internal,
                prov()
            )
            .unwrap_err(),
            EvidenceError::EmptyContent
        );
    }

    #[test]
    fn quarantine_release_is_operator_only_and_never_lowers_classification() {
        let mut a = AttachmentRef::new(
            AttachmentId::parse("att-10").unwrap(),
            b"blob",
            "application/octet-stream",
            Classification::Private,
            prov(),
        )
        .unwrap();
        let model = Actor::model(ActorId::parse("m").unwrap());
        assert!(matches!(
            a.release_from_quarantine(&model, Classification::Public),
            Err(EvidenceError::ActorNotPermitted { .. })
        ));
        assert!(a.quarantined);
        let op = Actor::operator(ActorId::parse("operator").unwrap());
        a.release_from_quarantine(&op, Classification::Public)
            .unwrap();
        assert!(!a.quarantined);
        assert_eq!(a.quarantine_reason, None);
        assert_eq!(a.classification, Classification::Private);
        assert_eq!(
            a.release_from_quarantine(&op, Classification::Private),
            Err(EvidenceError::NotQuarantined)
        );
    }

    #[test]
    fn content_address_layout() {
        let addr = content_address(b"abc");
        assert_eq!(
            addr.sha256,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            addr.relpath,
            "ba/78/ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let parts: Vec<&str> = addr.relpath.split('/').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], &addr.sha256[0..2]);
        assert_eq!(parts[1], &addr.sha256[2..4]);
        assert_eq!(parts[2], addr.sha256);
        // The relpath is itself acceptable under an approved root.
        assert!(roots().contains(Path::new(&addr.relpath)));
        // Stable and content-dependent.
        assert_eq!(content_address(b"abc"), addr);
        assert_ne!(content_address(b"abd").sha256, addr.sha256);
    }
}
