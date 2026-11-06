//! Skill registry (ARCHITECTURE.md §10 "Skill registry"; handoff §11 "37-skill
//! registry"). Loads every package under a root, checks that all 37 codes are
//! present exactly once, and verifies package hashes against a pinned table
//! (supply-chain rule: mismatched hashes are refused at runtime).

use crate::package::{load_package, PackageError, SkillPackage};
use g9_domain::{DataDomain, G9Error, InteractionMode, SkillCode, SkillFamily, SkillId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The 37 logical skills: code → canonical name (handoff §11 table). The
/// content team's validator (`scripts/check_skill_packages.py`) binds the same
/// pairs; [`Registry::expect_complete`] reports a package whose name does not
/// match its code.
pub const CANONICAL_REGISTRY: [(&str, &str); 37] = [
    ("E01", "g9-spec-assurance"),
    ("E02", "g9-skill-engineer"),
    ("E03", "g9-curriculum-compiler"),
    ("E04", "g9-domain-architect"),
    ("E05", "g9-ui-accessibility"),
    ("E06", "g9-security-boundaries"),
    ("E07", "g9-data-integrity"),
    ("E08", "g9-scheduling-engine"),
    ("E09", "g9-learning-data-model"),
    ("E10", "g9-model-runtime-engineer"),
    ("E11", "g9-knowledge-engineer"),
    ("E12", "g9-mentor-evaluation"),
    ("E13", "g9-adversarial-assurance"),
    ("E14", "g9-release-recovery"),
    ("T01", "g9-session-director"),
    ("T02", "g9-prerequisite-diagnosis"),
    ("T03", "g9-adaptive-instruction"),
    ("T04", "g9-socratic-coaching"),
    ("T05", "g9-laboratory-coach"),
    ("T06", "g9-progressive-hints"),
    ("T07", "g9-teachback-review"),
    ("T08", "g9-independent-examiner"),
    ("L01", "g9-misconception-analysis"),
    ("L02", "g9-competency-evidence"),
    ("L03", "g9-retrieval-and-transfer"),
    ("L04", "g9-intervention-designer"),
    ("L05", "g9-learning-experiment"),
    ("L06", "g9-confidence-calibration"),
    ("P01", "g9-adaptive-agenda"),
    ("P02", "g9-performance-review"),
    ("P03", "g9-goal-decomposition"),
    ("P04", "g9-execution-diagnosis"),
    ("P05", "g9-decision-review"),
    ("P06", "g9-reflection-coaching"),
    ("K01", "g9-source-curation"),
    ("K02", "g9-memory-reconciliation"),
    ("K03", "g9-ct9-knowledge-export"),
];

/// All 37 codes in registry order.
pub fn all_codes() -> Vec<SkillCode> {
    CANONICAL_REGISTRY
        .iter()
        .filter_map(|(c, _)| SkillCode::parse(c).ok())
        .collect()
}

/// Canonical name for a code.
pub fn canonical_name(code: &SkillCode) -> Option<&'static str> {
    CANONICAL_REGISTRY
        .iter()
        .find(|(c, _)| *c == code.as_str())
        .map(|(_, n)| *n)
}

/// One package that failed to load.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{dir}: {error}")]
pub struct PackageFailure {
    /// Directory name under the registry root.
    pub dir: String,
    /// Why it was refused.
    pub error: PackageError,
}

/// Registry load failure.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    /// The root is not a directory.
    #[error("{0} is not a directory")]
    NotADirectory(PathBuf),
    /// The root could not be listed.
    #[error("cannot read registry root: {0}")]
    Io(String),
    /// One or more packages were refused; nothing is loaded.
    #[error("{} package(s) failed validation", .0.len())]
    Packages(Vec<PackageFailure>),
    /// A pinned hash does not match (see [`Registry::load_pinned`]).
    #[error("{} package hash mismatch(es)", .0.len())]
    HashMismatch(Vec<HashMismatch>),
}

impl From<RegistryError> for G9Error {
    fn from(e: RegistryError) -> Self {
        match &e {
            RegistryError::HashMismatch(m) => {
                G9Error::precondition_failed("skills.registry.hash_mismatch", e.to_string())
                    .with_details(serde_json::json!({
                        "skills": m.iter().map(|x| x.skill.as_str()).collect::<Vec<_>>()
                    }))
            }
            RegistryError::Packages(p) => {
                G9Error::precondition_failed("skills.registry.invalid_packages", e.to_string())
                    .with_details(serde_json::json!({
                        "packages": p.iter().map(|f| serde_json::json!({ "dir": f.dir, "rule": f.error.rule() })).collect::<Vec<_>>()
                    }))
            }
            RegistryError::NotADirectory(_) | RegistryError::Io(_) => {
                G9Error::resource_unavailable("skills.registry.unreadable", e.to_string())
            }
        }
    }
}

/// Result of [`Registry::expect_complete`] when the registry is not the full set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("registry incomplete: {} missing code(s), {} duplicated code(s), {} name binding mismatch(es)", missing.len(), duplicates.len(), misbound.len())]
pub struct MissingSkills {
    /// Codes with no package.
    pub missing: Vec<SkillCode>,
    /// Codes claimed by more than one package.
    pub duplicates: Vec<(SkillCode, Vec<SkillId>)>,
    /// Packages whose name is not the canonical name of their code.
    pub misbound: Vec<(SkillCode, SkillId)>,
}

impl From<MissingSkills> for G9Error {
    fn from(e: MissingSkills) -> Self {
        G9Error::precondition_failed("skills.registry.incomplete", e.to_string()).with_details(
            serde_json::json!({
                "missing": e.missing.iter().map(SkillCode::as_str).collect::<Vec<_>>(),
                "duplicates": e.duplicates.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(),
                "misbound": e.misbound.iter().map(|(c, _)| c.as_str()).collect::<Vec<_>>(),
            }),
        )
    }
}

/// A package whose computed hash differs from the pinned value, a pin for a
/// package that is not loaded (`actual: None`), or a loaded package with no pin
/// (`expected: None`). All three are refused by [`Registry::load_pinned`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HashMismatch {
    /// Skill name.
    pub skill: SkillId,
    /// Pinned hash, if any.
    pub expected: Option<String>,
    /// Computed hash, if the package is loaded.
    pub actual: Option<String>,
}

/// In-memory registry of validated packages, keyed by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Registry {
    root: PathBuf,
    packages: BTreeMap<SkillId, SkillPackage>,
}

impl Registry {
    /// Load every subdirectory of `root` (sorted by name; entries starting with
    /// `.` and non-directories are skipped). Any invalid package fails the load.
    pub fn load(root: &Path) -> Result<Self, RegistryError> {
        if !root.is_dir() {
            return Err(RegistryError::NotADirectory(root.to_path_buf()));
        }
        let mut dirs: Vec<PathBuf> = fs::read_dir(root)
            .map_err(|e| RegistryError::Io(e.kind().to_string()))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| !n.starts_with('.'))
            })
            .collect();
        dirs.sort();
        let mut packages = BTreeMap::new();
        let mut failures = Vec::new();
        for dir in dirs {
            let dir_name = dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match load_package(&dir) {
                Ok(p) => {
                    packages.insert(p.name.clone(), p);
                }
                Err(error) => failures.push(PackageFailure {
                    dir: dir_name,
                    error,
                }),
            }
        }
        if !failures.is_empty() {
            return Err(RegistryError::Packages(failures));
        }
        Ok(Self {
            root: root.to_path_buf(),
            packages,
        })
    }

    /// [`Registry::load`] followed by [`Registry::verify_hashes`]; any mismatch
    /// refuses the whole registry (runtime supply-chain rule).
    pub fn load_pinned(
        root: &Path,
        expected: &BTreeMap<SkillId, String>,
    ) -> Result<Self, RegistryError> {
        let registry = Self::load(root)?;
        let mismatches = registry.verify_hashes(expected);
        if mismatches.is_empty() {
            Ok(registry)
        } else {
            Err(RegistryError::HashMismatch(mismatches))
        }
    }

    /// Build a registry from already-validated packages (tests, in-memory use).
    /// Later packages with the same name replace earlier ones.
    pub fn from_packages(root: PathBuf, packages: impl IntoIterator<Item = SkillPackage>) -> Self {
        Self {
            root,
            packages: packages.into_iter().map(|p| (p.name.clone(), p)).collect(),
        }
    }

    /// The directory the registry was loaded from.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Package by name.
    pub fn get(&self, id: &SkillId) -> Option<&SkillPackage> {
        self.packages.get(id)
    }

    /// Package by registry code (first match in name order).
    pub fn by_code(&self, code: &SkillCode) -> Option<&SkillPackage> {
        self.packages.values().find(|p| &p.code == code)
    }

    /// Number of packages.
    pub fn len(&self) -> usize {
        self.packages.len()
    }

    /// True when no package is loaded.
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    /// All packages in name order.
    pub fn iter(&self) -> impl Iterator<Item = &SkillPackage> {
        self.packages.values()
    }

    /// Packages of one family, in name order.
    pub fn by_family(&self, family: SkillFamily) -> Vec<&SkillPackage> {
        self.packages
            .values()
            .filter(|p| p.family == family)
            .collect()
    }

    /// Name → package hash for every loaded package (the table to pin).
    pub fn hashes(&self) -> BTreeMap<SkillId, String> {
        self.packages
            .iter()
            .map(|(k, v)| (k.clone(), v.package_sha256.clone()))
            .collect()
    }

    /// All 37 codes present exactly once, each under its canonical name.
    pub fn expect_complete(&self) -> Result<(), MissingSkills> {
        let mut by_code: BTreeMap<String, Vec<SkillId>> = BTreeMap::new();
        for p in self.packages.values() {
            by_code
                .entry(p.code.as_str().to_string())
                .or_default()
                .push(p.name.clone());
        }
        let mut missing = Vec::new();
        let mut duplicates = Vec::new();
        let mut misbound = Vec::new();
        for code in all_codes() {
            match by_code.get(code.as_str()) {
                None => missing.push(code.clone()),
                Some(names) if names.len() > 1 => duplicates.push((code.clone(), names.clone())),
                Some(_) => {}
            }
            if let (Some(names), Some(canonical)) =
                (by_code.get(code.as_str()), canonical_name(&code))
            {
                for n in names {
                    if n.as_str() != canonical {
                        misbound.push((code.clone(), n.clone()));
                    }
                }
            }
        }
        if missing.is_empty() && duplicates.is_empty() && misbound.is_empty() {
            Ok(())
        } else {
            Err(MissingSkills {
                missing,
                duplicates,
                misbound,
            })
        }
    }

    /// Compare computed hashes with a pinned table. Reports differing hashes,
    /// pins without a package and packages without a pin.
    pub fn verify_hashes(&self, expected: &BTreeMap<SkillId, String>) -> Vec<HashMismatch> {
        let mut out = Vec::new();
        for (name, pkg) in &self.packages {
            match expected.get(name) {
                Some(h) if h == &pkg.package_sha256 => {}
                other => out.push(HashMismatch {
                    skill: name.clone(),
                    expected: other.cloned(),
                    actual: Some(pkg.package_sha256.clone()),
                }),
            }
        }
        for (name, h) in expected {
            if !self.packages.contains_key(name) {
                out.push(HashMismatch {
                    skill: name.clone(),
                    expected: Some(h.clone()),
                    actual: None,
                });
            }
        }
        out
    }

    /// Packages that declare both `mode` and `domain`, ordered by code. This is
    /// the candidate set *before* trigger matching and policy preflight; it
    /// grants nothing.
    pub fn candidates_for(&self, mode: InteractionMode, domain: DataDomain) -> Vec<&SkillPackage> {
        let mut v: Vec<&SkillPackage> = self
            .packages
            .values()
            .filter(|p| p.modes.contains(&mode) && p.domains.contains(&domain))
            .collect();
        v.sort_by(|a, b| a.code.as_str().cmp(b.code.as_str()));
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_table_has_37_distinct_valid_codes_and_names() {
        let codes = all_codes();
        assert_eq!(codes.len(), SkillCode::TOTAL);
        let names: std::collections::BTreeSet<&str> =
            CANONICAL_REGISTRY.iter().map(|(_, n)| *n).collect();
        assert_eq!(names.len(), 37);
        for (_, n) in CANONICAL_REGISTRY {
            assert!(g9_domain::skill::validate_skill_name(n).is_ok(), "{n}");
            assert!(n.starts_with("g9-"));
        }
        assert_eq!(
            canonical_name(&SkillCode::parse("T01").unwrap()),
            Some("g9-session-director")
        );
    }

    #[test]
    fn empty_registry_reports_all_codes_missing() {
        let r = Registry::from_packages(PathBuf::from("/nonexistent"), Vec::new());
        let err = r.expect_complete().unwrap_err();
        assert_eq!(err.missing.len(), 37);
        assert!(err.duplicates.is_empty());
        assert!(r.is_empty());
        assert!(matches!(
            Registry::load(Path::new("/definitely/not/here")),
            Err(RegistryError::NotADirectory(_))
        ));
    }
}
