//! Skill package model and loader (`docs/skills/skill-package-contract.md`).
//!
//! [`load_package`] validates **every** contract rule and computes the package
//! hash used as the supply-chain pin (handoff §11 "Skill packages and reference
//! assets must be pinned/hashed"). A package that fails any rule is refused as a
//! whole; the first violation found (in the documented order) is returned.
//!
//! Package content is [`g9_domain::TrustClass::SkillPackage`]: instructional
//! metadata only. Loading a package never grants a capability (C11).

use crate::evals::{EvalCase, EvalKind};
use crate::frontmatter::{self, FrontmatterError};
use g9_domain::skill::validate_skill_name;
use g9_domain::{
    Capability, DataDomain, G9Error, InteractionMode, ResourceBudget, SkillCode, SkillFamily,
    SkillId,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The only accepted `$schema`.
pub const DRAFT_2020_12: &str = "https://json-schema.org/draft/2020-12/schema";

/// The 12 required H2 sections of `SKILL.md`, in order.
pub const REQUIRED_SECTIONS: [&str; 12] = [
    "Mission",
    "Activation",
    "Inputs and provenance requirements",
    "Procedure",
    "Non-goals",
    "Tool and data scopes",
    "Output contract",
    "Confidence and abstention",
    "Escalation and stop conditions",
    "Failure recovery",
    "Acceptance tests",
    "References",
];

/// The exact `metadata` keys.
pub const METADATA_KEYS: [&str; 13] = [
    "g9-code",
    "g9-family",
    "g9-version",
    "g9-owner-module",
    "g9-modes",
    "g9-data-domains",
    "g9-capabilities",
    "g9-budget-tokens",
    "g9-budget-seconds",
    "g9-budget-tool-calls",
    "g9-recursion-depth",
    "g9-depends-on",
    "g9-status",
];

/// Budget bounds, mirroring `resource_budget` in the invocation envelope schema.
pub const BUDGET_TOKENS: (u32, u32) = (1, 32_768);
/// See [`BUDGET_TOKENS`].
pub const BUDGET_SECONDS: (u32, u32) = (1, 600);
/// See [`BUDGET_TOKENS`].
pub const BUDGET_TOOL_CALLS: (u32, u32) = (0, 16);

/// Promotion status (`g9-status`), handoff §11 "Skill promotion gates".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PackageStatus {
    /// Contract and fixtures exist; no runtime wiring.
    Proposed,
    /// Runtime wiring exists.
    Implemented,
    /// Promotion gates measured and recorded in `docs/evaluations/skill-evals/<name>.md`.
    Evaluated,
}

impl PackageStatus {
    fn parse(s: &str) -> Option<Self> {
        match s {
            "PROPOSED" => Some(Self::Proposed),
            "IMPLEMENTED" => Some(Self::Implemented),
            "EVALUATED" => Some(Self::Evaluated),
            _ => None,
        }
    }
}

/// The four evaluation fixture files of a package.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct EvalSet {
    /// Must activate and succeed.
    pub positive: Vec<EvalCase>,
    /// Must not activate, or must abstain.
    pub negative: Vec<EvalCase>,
    /// Malicious/poisoned input: no escalation, no fabrication, no operator-only requests.
    pub adversarial: Vec<EvalCase>,
    /// Representative baseline task with a measurable expectation.
    pub acceptance: Vec<EvalCase>,
}

impl EvalSet {
    /// Cases of one kind.
    pub fn by_kind(&self, kind: EvalKind) -> &[EvalCase] {
        match kind {
            EvalKind::Positive => &self.positive,
            EvalKind::Negative => &self.negative,
            EvalKind::Adversarial => &self.adversarial,
            EvalKind::Acceptance => &self.acceptance,
        }
    }

    fn slot_mut(&mut self, kind: EvalKind) -> &mut Vec<EvalCase> {
        match kind {
            EvalKind::Positive => &mut self.positive,
            EvalKind::Negative => &mut self.negative,
            EvalKind::Adversarial => &mut self.adversarial,
            EvalKind::Acceptance => &mut self.acceptance,
        }
    }

    /// All cases in file order (positive, negative, adversarial, acceptance).
    pub fn all(&self) -> impl Iterator<Item = &EvalCase> {
        self.positive
            .iter()
            .chain(&self.negative)
            .chain(&self.adversarial)
            .chain(&self.acceptance)
    }

    /// Total number of cases.
    pub fn len(&self) -> usize {
        self.positive.len() + self.negative.len() + self.adversarial.len() + self.acceptance.len()
    }

    /// True when no case is present.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A validated skill package. Every field is derived from the files on disk;
/// nothing here is an authority grant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillPackage {
    /// Agent Skills `name` (= directory name, `g9-` prefixed).
    pub name: SkillId,
    /// Registry code (`E01`..`K03`).
    pub code: SkillCode,
    /// Family derived from the code and asserted by `g9-family`.
    pub family: SkillFamily,
    /// `g9-version` (MAJOR.MINOR.PATCH).
    pub version: String,
    /// Agent Skills `description` (1..=1024 chars).
    pub description: String,
    /// SPDX license id as written in the frontmatter.
    pub license: String,
    /// `g9-owner-module` (`M01`..`M17`).
    pub owner_module: String,
    /// Modes the skill may run in.
    pub modes: Vec<InteractionMode>,
    /// Data domains the skill may operate on.
    pub domains: Vec<DataDomain>,
    /// Capabilities the skill may *request*; never operator-only ones.
    pub requested_capabilities: Vec<Capability>,
    /// Hard caps for one invocation.
    pub budget: ResourceBudget,
    /// Always 0 (no recursion).
    pub recursion_depth: u8,
    /// Skills whose outputs this one may consume.
    pub depends_on: Vec<SkillId>,
    /// Promotion status.
    pub status: PackageStatus,
    /// The `SKILL.md` body after the frontmatter — instructions of trust class
    /// `SkillPackage`, injected as data into the model context.
    pub body: String,
    /// H2 titles in order (always the 12 contract sections after validation).
    pub body_sections: Vec<String>,
    /// `schemas/input.schema.json` (describes `input_payload` only).
    pub input_schema: Value,
    /// `schemas/output.schema.json` (describes `result` only).
    pub output_schema: Value,
    /// Evaluation fixtures.
    pub evals: EvalSet,
    /// Hex SHA-256 over sorted relative paths and file bytes (see [`package_sha256`]).
    pub package_sha256: String,
    /// Package directory.
    pub dir: PathBuf,
}

/// Why a package was refused. Variants map 1:1 to contract rules; see
/// [`PackageError::rule`] for the stable rule id. Messages carry relative
/// paths, key names and enum values only — never file contents.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PackageError {
    /// The path is not a directory.
    #[error("{0} is not a directory")]
    NotADirectory(PathBuf),
    /// The directory name is not valid UTF-8.
    #[error("package directory name is not valid UTF-8")]
    DirName,
    /// A file could not be read.
    #[error("cannot read {path}: {reason}")]
    Io {
        /// Path relative to the package.
        path: String,
        /// `std::io::ErrorKind` description.
        reason: String,
    },
    /// A file is not valid JSON.
    #[error("{path}: invalid JSON: {reason}")]
    Json {
        /// Path relative to the package.
        path: String,
        /// serde_json message (line/column and token, never content).
        reason: String,
    },
    /// Frontmatter grammar violation.
    #[error(transparent)]
    Frontmatter(#[from] FrontmatterError),
    /// `name` violates the Agent Skills name rules.
    #[error("name: {0}")]
    InvalidName(String),
    /// `name` lacks the `g9-` prefix.
    #[error("name must start with `g9-`")]
    MissingPrefix,
    /// `name` differs from the directory name.
    #[error("name `{name}` does not match directory `{dir}`")]
    NameMismatch {
        /// Frontmatter name.
        name: String,
        /// Directory name.
        dir: String,
    },
    /// `description` length outside 1..=1024 characters.
    #[error("description must be 1..=1024 characters, got {0}")]
    DescriptionLength(usize),
    /// Required `metadata` keys absent.
    #[error("metadata: missing keys {0:?}")]
    MetadataMissingKeys(Vec<String>),
    /// `metadata` keys outside the contract.
    #[error("metadata: unknown keys {0:?}")]
    MetadataUnknownKeys(Vec<String>),
    /// `g9-code` is not a registry code.
    #[error("metadata: g9-code: {0}")]
    InvalidCode(String),
    /// `g9-family` is not one of the five families.
    #[error(
        "metadata: g9-family `{0}` is not engineering|teaching|learning|performance|knowledge"
    )]
    InvalidFamily(String),
    /// `g9-family` disagrees with the code prefix.
    #[error("metadata: g9-family `{family}` does not match code {code}")]
    FamilyMismatch {
        /// The code.
        code: String,
        /// The declared family.
        family: String,
    },
    /// `g9-version` is not `MAJOR.MINOR.PATCH`.
    #[error("metadata: g9-version `{0}` is not MAJOR.MINOR.PATCH")]
    InvalidVersion(String),
    /// `g9-owner-module` is not `M01`..`M17`.
    #[error("metadata: g9-owner-module `{0}` is not M01..M17")]
    InvalidOwnerModule(String),
    /// `g9-modes` is empty or holds an unknown mode.
    #[error("metadata: g9-modes: {0}")]
    InvalidModes(String),
    /// `g9-data-domains` is empty or holds an unknown domain.
    #[error("metadata: g9-data-domains: {0}")]
    InvalidDomains(String),
    /// `g9-capabilities` holds an unknown capability.
    #[error("metadata: g9-capabilities: {0}")]
    InvalidCapabilities(String),
    /// `g9-capabilities` requests an operator-only capability (A07).
    #[error("metadata: g9-capabilities requests operator-only capability {0:?}")]
    OperatorOnlyCapability(Capability),
    /// An engineering skill requests anything but `FILESYSTEM_READ`.
    #[error("metadata: engineering skills may request FILESYSTEM_READ only, found {0:?}")]
    EngineeringCapability(Capability),
    /// A budget value is not an integer within the envelope bounds.
    #[error("metadata: {key} = `{value}` is not an integer within {lo}..={hi}")]
    InvalidBudget {
        /// Metadata key.
        key: String,
        /// Value as written.
        value: String,
        /// Lower bound.
        lo: u32,
        /// Upper bound.
        hi: u32,
    },
    /// `g9-recursion-depth` is not `"0"`.
    #[error("metadata: g9-recursion-depth must be \"0\", got `{0}`")]
    RecursionDepth(String),
    /// `g9-depends-on` holds an invalid or self-referencing name.
    #[error("metadata: g9-depends-on: {0}")]
    InvalidDependency(String),
    /// `g9-status` is not `PROPOSED|IMPLEMENTED|EVALUATED`.
    #[error("metadata: g9-status `{0}` is not PROPOSED|IMPLEMENTED|EVALUATED")]
    InvalidStatus(String),
    /// The H2 sections are not exactly the 12 contract sections in order.
    #[error(
        "body: H2 sections must be exactly the 12 contract sections in order; found {found:?}"
    )]
    SectionsMismatch {
        /// H2 titles found.
        found: Vec<String>,
    },
    /// `## Activation` lacks the two trigger blocks.
    #[error("body: Activation must contain `### Positive triggers` (>=1 bullet) then `### Negative triggers` (>=2 bullets): {0}")]
    ActivationTriggers(String),
    /// A fenced code block is never closed.
    #[error("body: unterminated fenced code block")]
    UnterminatedFence,
    /// `references/` has no `.md` file.
    #[error("references/ must contain at least one .md file")]
    MissingReferences,
    /// A schema file violates a structural rule or does not compile.
    #[error("schemas/{which}.schema.json: {reason}")]
    SchemaInvalid {
        /// `input` or `output`.
        which: String,
        /// Which rule failed.
        reason: String,
    },
    /// A schema contains `$ref` (resolution is disabled).
    #[error("schemas/{which}.schema.json: `$ref` is not allowed (found at {path})")]
    SchemaRef {
        /// `input` or `output`.
        which: String,
        /// JSON path of the `$ref`.
        path: String,
    },
    /// An eval file is not a JSON array of well-formed cases.
    #[error("evals/{file}: {reason}")]
    EvalInvalid {
        /// File name.
        file: String,
        /// serde message.
        reason: String,
    },
    /// Too few cases.
    #[error("evals/{file}: needs at least {minimum} cases, found {found}")]
    EvalCount {
        /// File name.
        file: String,
        /// Cases found.
        found: usize,
        /// Required minimum.
        minimum: usize,
    },
    /// A case id is not `<code>-<PREFIX>-NNN`.
    #[error("evals/{file}: case id `{id}` must match {code}-{prefix}-NNN")]
    EvalIdPrefix {
        /// File name.
        file: String,
        /// Offending id.
        id: String,
        /// Expected code.
        code: String,
        /// Expected prefix.
        prefix: String,
    },
    /// A case's `kind` does not match its file.
    #[error("evals/{file}: case `{id}` has kind {found:?}, expected {expected:?}")]
    EvalKindMismatch {
        /// File name.
        file: String,
        /// Case id.
        id: String,
        /// Kind found.
        found: EvalKind,
        /// Kind expected.
        expected: EvalKind,
    },
    /// The same case id appears twice.
    #[error("evals: duplicate case id `{0}`")]
    EvalDuplicateId(String),
}

impl PackageError {
    /// Stable rule id (used in error codes and reports).
    pub fn rule(&self) -> &'static str {
        match self {
            Self::NotADirectory(_) => "layout.directory",
            Self::DirName => "layout.dir_name",
            Self::Io { .. } => "layout.io",
            Self::Json { .. } => "layout.json",
            Self::Frontmatter(_) => "frontmatter.grammar",
            Self::InvalidName(_) => "name.agent_skills_rules",
            Self::MissingPrefix => "name.g9_prefix",
            Self::NameMismatch { .. } => "name.matches_dir",
            Self::DescriptionLength(_) => "description.length",
            Self::MetadataMissingKeys(_) => "metadata.missing_keys",
            Self::MetadataUnknownKeys(_) => "metadata.unknown_keys",
            Self::InvalidCode(_) => "metadata.code",
            Self::InvalidFamily(_) => "metadata.family",
            Self::FamilyMismatch { .. } => "metadata.family_matches_code",
            Self::InvalidVersion(_) => "metadata.version",
            Self::InvalidOwnerModule(_) => "metadata.owner_module",
            Self::InvalidModes(_) => "metadata.modes",
            Self::InvalidDomains(_) => "metadata.domains",
            Self::InvalidCapabilities(_) => "metadata.capabilities",
            Self::OperatorOnlyCapability(_) => "metadata.capabilities.operator_only",
            Self::EngineeringCapability(_) => "metadata.capabilities.engineering",
            Self::InvalidBudget { .. } => "metadata.budget",
            Self::RecursionDepth(_) => "metadata.recursion_depth",
            Self::InvalidDependency(_) => "metadata.depends_on",
            Self::InvalidStatus(_) => "metadata.status",
            Self::SectionsMismatch { .. } => "body.sections",
            Self::ActivationTriggers(_) => "body.activation",
            Self::UnterminatedFence => "body.fence",
            Self::MissingReferences => "references.present",
            Self::SchemaInvalid { .. } => "schema.structure",
            Self::SchemaRef { .. } => "schema.no_ref",
            Self::EvalInvalid { .. } => "evals.parse",
            Self::EvalCount { .. } => "evals.minimum_count",
            Self::EvalIdPrefix { .. } => "evals.id_prefix",
            Self::EvalKindMismatch { .. } => "evals.kind",
            Self::EvalDuplicateId(_) => "evals.unique_ids",
        }
    }
}

impl From<PackageError> for G9Error {
    fn from(e: PackageError) -> Self {
        G9Error::precondition_failed("skills.package_invalid", e.to_string())
            .with_details(serde_json::json!({ "rule": e.rule() }))
    }
}

/// Load and validate one package directory.
///
/// Validation order: layout → frontmatter → name → description → metadata →
/// body sections → references → schemas → evals → hash.
pub fn load_package(dir: &Path) -> Result<SkillPackage, PackageError> {
    if !dir.is_dir() {
        return Err(PackageError::NotADirectory(dir.to_path_buf()));
    }
    let dir_name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or(PackageError::DirName)?
        .to_string();

    let text = read_text(dir, "SKILL.md")?;
    let parsed = frontmatter::parse(&text)?;
    let fm = parsed.frontmatter;

    // name
    validate_skill_name(&fm.name).map_err(PackageError::InvalidName)?;
    if !fm.name.starts_with("g9-") {
        return Err(PackageError::MissingPrefix);
    }
    if fm.name != dir_name {
        return Err(PackageError::NameMismatch {
            name: fm.name.clone(),
            dir: dir_name,
        });
    }
    let name = SkillId::parse(&fm.name).map_err(|e| PackageError::InvalidName(e.reason))?;

    // description
    let desc_len = fm.description.chars().count();
    if !(1..=1024).contains(&desc_len) {
        return Err(PackageError::DescriptionLength(desc_len));
    }

    // metadata keys
    let md = &fm.metadata;
    let missing: Vec<String> = METADATA_KEYS
        .iter()
        .filter(|k| !md.contains_key(**k))
        .map(|k| (*k).to_string())
        .collect();
    if !missing.is_empty() {
        return Err(PackageError::MetadataMissingKeys(missing));
    }
    let unknown: Vec<String> = md
        .keys()
        .filter(|k| !METADATA_KEYS.contains(&k.as_str()))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        return Err(PackageError::MetadataUnknownKeys(unknown));
    }
    let get = |k: &str| md.get(k).map(String::as_str).unwrap_or_default();

    // code / family
    let code = SkillCode::parse(get("g9-code")).map_err(PackageError::InvalidCode)?;
    let family = parse_family(get("g9-family"))
        .ok_or_else(|| PackageError::InvalidFamily(get("g9-family").to_string()))?;
    if family != code.family() {
        return Err(PackageError::FamilyMismatch {
            code: code.as_str().to_string(),
            family: get("g9-family").to_string(),
        });
    }

    // version / owner module
    let version = get("g9-version").to_string();
    if !is_semver(&version) {
        return Err(PackageError::InvalidVersion(version));
    }
    let owner_module = get("g9-owner-module").to_string();
    if !is_owner_module(&owner_module) {
        return Err(PackageError::InvalidOwnerModule(owner_module));
    }

    // modes / domains / capabilities
    let mode_names = csv(get("g9-modes")).map_err(PackageError::InvalidModes)?;
    if mode_names.is_empty() {
        return Err(PackageError::InvalidModes("must not be empty".into()));
    }
    let modes = mode_names
        .iter()
        .map(|m| {
            parse_enum::<InteractionMode>(m)
                .ok_or_else(|| PackageError::InvalidModes(format!("unknown mode `{m}`")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let domain_names = csv(get("g9-data-domains")).map_err(PackageError::InvalidDomains)?;
    if domain_names.is_empty() {
        return Err(PackageError::InvalidDomains("must not be empty".into()));
    }
    let domains = domain_names
        .iter()
        .map(|d| {
            parse_enum::<DataDomain>(d)
                .ok_or_else(|| PackageError::InvalidDomains(format!("unknown domain `{d}`")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let cap_names = csv(get("g9-capabilities")).map_err(PackageError::InvalidCapabilities)?;
    let requested_capabilities = cap_names
        .iter()
        .map(|c| {
            parse_enum::<Capability>(c).ok_or_else(|| {
                PackageError::InvalidCapabilities(format!("unknown capability `{c}`"))
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    for cap in &requested_capabilities {
        if cap.operator_only() {
            return Err(PackageError::OperatorOnlyCapability(*cap));
        }
        if family == SkillFamily::Engineering && *cap != Capability::FilesystemRead {
            return Err(PackageError::EngineeringCapability(*cap));
        }
    }

    // budget / recursion / depends_on / status
    let budget = ResourceBudget {
        max_tokens: parse_budget(get("g9-budget-tokens"), "g9-budget-tokens", BUDGET_TOKENS)?,
        max_seconds: parse_budget(
            get("g9-budget-seconds"),
            "g9-budget-seconds",
            BUDGET_SECONDS,
        )?,
        max_tool_calls: parse_budget(
            get("g9-budget-tool-calls"),
            "g9-budget-tool-calls",
            BUDGET_TOOL_CALLS,
        )?,
    };
    if get("g9-recursion-depth") != "0" {
        return Err(PackageError::RecursionDepth(
            get("g9-recursion-depth").to_string(),
        ));
    }
    let dep_names = csv(get("g9-depends-on")).map_err(PackageError::InvalidDependency)?;
    let mut depends_on = Vec::with_capacity(dep_names.len());
    for d in &dep_names {
        validate_skill_name(d).map_err(PackageError::InvalidDependency)?;
        if !d.starts_with("g9-") {
            return Err(PackageError::InvalidDependency(format!(
                "`{d}` lacks the g9- prefix"
            )));
        }
        if d == name.as_str() {
            return Err(PackageError::InvalidDependency(
                "a skill cannot depend on itself".into(),
            ));
        }
        depends_on.push(SkillId::parse(d).map_err(|e| PackageError::InvalidDependency(e.reason))?);
    }
    let status = PackageStatus::parse(get("g9-status"))
        .ok_or_else(|| PackageError::InvalidStatus(get("g9-status").to_string()))?;

    // body
    let sections = split_sections(&parsed.body)?;
    let body_sections: Vec<String> = sections.iter().map(|s| s.title.clone()).collect();
    if body_sections != REQUIRED_SECTIONS {
        return Err(PackageError::SectionsMismatch {
            found: body_sections,
        });
    }
    if let Some(activation) = sections.iter().find(|s| s.title == "Activation") {
        check_activation(&activation.lines)?;
    }

    // references
    let refs_dir = dir.join("references");
    let has_md = fs::read_dir(&refs_dir)
        .map_err(|e| io_err("references", &e))?
        .flatten()
        .any(|e| e.path().is_file() && e.path().extension().is_some_and(|x| x == "md"));
    if !has_md {
        return Err(PackageError::MissingReferences);
    }

    // schemas
    let input_schema = load_schema(dir, "input", name.as_str())?;
    let output_schema = load_schema(dir, "output", name.as_str())?;

    // evals
    let evals = load_evals(dir, &code)?;

    // hash
    let package_sha256 = package_sha256(dir)?;

    Ok(SkillPackage {
        name,
        code,
        family,
        version,
        description: fm.description,
        license: fm.license,
        owner_module,
        modes,
        domains,
        requested_capabilities,
        budget,
        recursion_depth: 0,
        depends_on,
        status,
        body: parsed.body,
        body_sections,
        input_schema,
        output_schema,
        evals,
        package_sha256,
        dir: dir.to_path_buf(),
    })
}

/// Hex SHA-256 of a package directory: for every regular file (recursively,
/// sorted by `/`-joined relative path) the hasher absorbs the path bytes, a
/// `0x00` separator, the file length as 8 big-endian bytes and the file bytes.
/// Symlinks are refused (a package must be self-contained).
pub fn package_sha256(dir: &Path) -> Result<String, PackageError> {
    let mut files = Vec::new();
    collect_files(dir, dir, &mut files)?;
    files.sort();
    let mut hasher = Sha256::new();
    for rel in files {
        let bytes = fs::read(dir.join(&rel)).map_err(|e| io_err(&rel, &e))?;
        hasher.update(rel.as_bytes());
        hasher.update([0u8]);
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(&bytes);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), PackageError> {
    let rel_dir = relative(root, dir);
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(|e| io_err(&rel_dir, &e))?
        .collect::<Result<_, _>>()
        .map_err(|e| io_err(&rel_dir, &e))?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let rel = relative(root, &path);
        let ft = entry.file_type().map_err(|e| io_err(&rel, &e))?;
        if ft.is_symlink() {
            return Err(PackageError::Io {
                path: rel,
                reason: "symlinks are not allowed in a skill package".into(),
            });
        }
        if ft.is_dir() {
            collect_files(root, &path, out)?;
        } else if ft.is_file() {
            out.push(rel);
        }
    }
    Ok(())
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

fn io_err(rel: &str, e: &std::io::Error) -> PackageError {
    PackageError::Io {
        path: rel.to_string(),
        reason: e.kind().to_string(),
    }
}

fn read_text(dir: &Path, rel: &str) -> Result<String, PackageError> {
    fs::read_to_string(dir.join(rel)).map_err(|e| io_err(rel, &e))
}

fn parse_family(s: &str) -> Option<SkillFamily> {
    match s {
        "engineering" => Some(SkillFamily::Engineering),
        "teaching" => Some(SkillFamily::Teaching),
        "learning" => Some(SkillFamily::Learning),
        "performance" => Some(SkillFamily::Performance),
        "knowledge" => Some(SkillFamily::Knowledge),
        _ => None,
    }
}

fn is_semver(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

fn is_owner_module(s: &str) -> bool {
    s.strip_prefix('M')
        .filter(|rest| rest.len() == 2 && rest.chars().all(|c| c.is_ascii_digit()))
        .and_then(|rest| rest.parse::<u8>().ok())
        .is_some_and(|n| (1..=17).contains(&n))
}

/// Comma-separated list: trimmed, no empty or duplicate elements; `""` → empty.
fn csv(raw: &str) -> Result<Vec<String>, String> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let parts: Vec<String> = raw.split(',').map(|p| p.trim().to_string()).collect();
    if parts.iter().any(String::is_empty) {
        return Err("empty element".into());
    }
    let unique: BTreeSet<&String> = parts.iter().collect();
    if unique.len() != parts.len() {
        return Err("duplicate element".into());
    }
    Ok(parts)
}

fn parse_enum<T: DeserializeOwned>(s: &str) -> Option<T> {
    serde_json::from_value(Value::String(s.to_string())).ok()
}

fn parse_budget(raw: &str, key: &str, (lo, hi): (u32, u32)) -> Result<u32, PackageError> {
    let err = || PackageError::InvalidBudget {
        key: key.to_string(),
        value: raw.to_string(),
        lo,
        hi,
    };
    if raw.is_empty() || !raw.chars().all(|c| c.is_ascii_digit()) {
        return Err(err());
    }
    let v: u32 = raw.parse().map_err(|_| err())?;
    if v < lo || v > hi {
        return Err(err());
    }
    Ok(v)
}

/// One H2 section of the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Section {
    pub(crate) title: String,
    pub(crate) lines: Vec<String>,
}

/// Split the body at H2 headings, ignoring headings inside fenced code blocks.
pub(crate) fn split_sections(body: &str) -> Result<Vec<Section>, PackageError> {
    let mut sections: Vec<Section> = Vec::new();
    let mut fence: Option<&str> = None;
    for line in body.lines() {
        let t = line.trim_start();
        let marker = if t.starts_with("```") {
            Some("```")
        } else if t.starts_with("~~~") {
            Some("~~~")
        } else {
            None
        };
        if let Some(m) = marker {
            match fence {
                None => fence = Some(m),
                Some(f) if f == m => fence = None,
                Some(_) => {}
            }
            if let Some(s) = sections.last_mut() {
                s.lines.push(line.to_string());
            }
            continue;
        }
        if fence.is_some() {
            if let Some(s) = sections.last_mut() {
                s.lines.push(line.to_string());
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("## ") {
            sections.push(Section {
                title: rest.trim().to_string(),
                lines: Vec::new(),
            });
            continue;
        }
        if let Some(s) = sections.last_mut() {
            s.lines.push(line.to_string());
        }
    }
    if fence.is_some() {
        return Err(PackageError::UnterminatedFence);
    }
    Ok(sections)
}

fn is_bullet(line: &str) -> bool {
    let mut c = line.trim_start().chars();
    matches!(c.next(), Some('-' | '*' | '+'))
        && matches!(c.next(), Some(' '))
        && c.next().is_some_and(|x| !x.is_whitespace())
}

fn check_activation(lines: &[String]) -> Result<(), PackageError> {
    let mut blocks: Vec<(String, usize)> = Vec::new();
    for line in lines {
        if let Some(t) = line.strip_prefix("### ") {
            blocks.push((t.trim().to_string(), 0));
        } else if is_bullet(line) {
            if let Some(b) = blocks.last_mut() {
                b.1 += 1;
            }
        }
    }
    let titles: Vec<&str> = blocks.iter().map(|b| b.0.as_str()).collect();
    if titles != ["Positive triggers", "Negative triggers"] {
        return Err(PackageError::ActivationTriggers(format!(
            "found H3 blocks {titles:?}"
        )));
    }
    let positive = blocks.first().map(|b| b.1).unwrap_or(0);
    let negative = blocks.get(1).map(|b| b.1).unwrap_or(0);
    if positive < 1 {
        return Err(PackageError::ActivationTriggers(
            "Positive triggers needs at least one bullet".into(),
        ));
    }
    if negative < 2 {
        return Err(PackageError::ActivationTriggers(
            "Negative triggers needs at least two bullets".into(),
        ));
    }
    Ok(())
}

fn load_schema(dir: &Path, which: &str, name: &str) -> Result<Value, PackageError> {
    let rel = format!("schemas/{which}.schema.json");
    let text = read_text(dir, &rel)?;
    let value: Value = serde_json::from_str(&text).map_err(|e| PackageError::Json {
        path: rel,
        reason: e.to_string(),
    })?;
    check_schema(&value, which, name)?;
    Ok(value)
}

/// Structural rules for `schemas/*.schema.json` (contract "Schemas").
pub fn check_schema(value: &Value, which: &str, name: &str) -> Result<(), PackageError> {
    let invalid = |reason: &str| PackageError::SchemaInvalid {
        which: which.to_string(),
        reason: reason.to_string(),
    };
    let obj = value
        .as_object()
        .ok_or_else(|| invalid("root must be an object"))?;
    if obj.get("$schema").and_then(Value::as_str) != Some(DRAFT_2020_12) {
        return Err(invalid("`$schema` must be the draft 2020-12 URI"));
    }
    let expected_id = format!("https://g9.local/skills/{name}/{which}");
    if obj.get("$id").and_then(Value::as_str) != Some(expected_id.as_str()) {
        return Err(invalid(
            "`$id` must be https://g9.local/skills/<name>/<input|output>",
        ));
    }
    if obj
        .get("title")
        .and_then(Value::as_str)
        .is_none_or(|t| t.trim().is_empty())
    {
        return Err(invalid("`title` is required"));
    }
    if obj.get("type").and_then(Value::as_str) != Some("object") {
        return Err(invalid("`type` must be \"object\""));
    }
    if obj.get("additionalProperties") != Some(&Value::Bool(false)) {
        return Err(invalid("`additionalProperties` must be false"));
    }
    let props = obj
        .get("properties")
        .and_then(Value::as_object)
        .filter(|p| !p.is_empty())
        .ok_or_else(|| invalid("`properties` must be a non-empty object"))?;
    let required = obj
        .get("required")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("`required` must be an array"))?;
    let mut seen = BTreeSet::new();
    for r in required {
        let key = r
            .as_str()
            .ok_or_else(|| invalid("`required` entries must be strings"))?;
        if !props.contains_key(key) {
            return Err(invalid("`required` names a property that does not exist"));
        }
        if !seen.insert(key) {
            return Err(invalid("`required` has duplicates"));
        }
    }
    if let Some(path) = find_ref(value, "$") {
        return Err(PackageError::SchemaRef {
            which: which.to_string(),
            path,
        });
    }
    jsonschema::validator_for(value).map_err(|e| {
        invalid(&format!(
            "does not compile as draft 2020-12 (`{}`)",
            e.kind().keyword()
        ))
    })?;
    Ok(())
}

fn find_ref(node: &Value, path: &str) -> Option<String> {
    match node {
        Value::Object(map) => {
            for (k, v) in map {
                let p = format!("{path}.{k}");
                if k == "$ref" {
                    return Some(p);
                }
                if let Some(found) = find_ref(v, &p) {
                    return Some(found);
                }
            }
            None
        }
        Value::Array(items) => items
            .iter()
            .enumerate()
            .find_map(|(i, v)| find_ref(v, &format!("{path}[{i}]"))),
        _ => None,
    }
}

fn load_evals(dir: &Path, code: &SkillCode) -> Result<EvalSet, PackageError> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut set = EvalSet::default();
    for kind in EvalKind::ALL {
        let file = kind.file_name();
        let text = read_text(dir, &format!("evals/{file}"))?;
        let cases: Vec<EvalCase> =
            serde_json::from_str(&text).map_err(|e| PackageError::EvalInvalid {
                file: file.to_string(),
                reason: e.to_string(),
            })?;
        if cases.len() < kind.minimum() {
            return Err(PackageError::EvalCount {
                file: file.to_string(),
                found: cases.len(),
                minimum: kind.minimum(),
            });
        }
        for case in &cases {
            if !valid_case_id(&case.id, code, kind.id_prefix()) {
                return Err(PackageError::EvalIdPrefix {
                    file: file.to_string(),
                    id: case.id.clone(),
                    code: code.as_str().to_string(),
                    prefix: kind.id_prefix().to_string(),
                });
            }
            if case.kind != kind {
                return Err(PackageError::EvalKindMismatch {
                    file: file.to_string(),
                    id: case.id.clone(),
                    found: case.kind,
                    expected: kind,
                });
            }
            if !seen.insert(case.id.clone()) {
                return Err(PackageError::EvalDuplicateId(case.id.clone()));
            }
        }
        *set.slot_mut(kind) = cases;
    }
    Ok(set)
}

/// `<CODE>-<PREFIX>-NNN`.
pub(crate) fn valid_case_id(id: &str, code: &SkillCode, prefix: &str) -> bool {
    id.strip_prefix(code.as_str())
        .and_then(|r| r.strip_prefix('-'))
        .and_then(|r| r.strip_prefix(prefix))
        .and_then(|r| r.strip_prefix('-'))
        .is_some_and(|n| n.len() == 3 && n.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn sections_split_ignores_fenced_headings() {
        let body = "intro\n## A\nx\n```\n## not a heading\n```\n## B\n- y\n";
        let s = split_sections(body).unwrap();
        let titles: Vec<&str> = s.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, vec!["A", "B"]);
        assert_eq!(s[0].lines.len(), 4);
        assert!(split_sections("## A\n```\nopen").is_err());
    }

    #[test]
    fn activation_requires_both_blocks() {
        let ok: Vec<String> = [
            "### Positive triggers",
            "- a",
            "### Negative triggers",
            "- b",
            "- c",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert!(check_activation(&ok).is_ok());
        let one_neg: Vec<String> = [
            "### Positive triggers",
            "- a",
            "### Negative triggers",
            "- b",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert!(matches!(
            check_activation(&one_neg),
            Err(PackageError::ActivationTriggers(_))
        ));
        let swapped: Vec<String> = [
            "### Negative triggers",
            "- a",
            "- b",
            "### Positive triggers",
            "- c",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert!(check_activation(&swapped).is_err());
    }

    #[test]
    fn schema_rules() {
        let good = json!({
            "$schema": DRAFT_2020_12,
            "$id": "https://g9.local/skills/g9-x/input",
            "title": "t", "type": "object", "additionalProperties": false,
            "required": ["a"], "properties": { "a": { "type": "string" } }
        });
        assert!(check_schema(&good, "input", "g9-x").is_ok());
        let mut with_ref = good.clone();
        with_ref["properties"]["a"] = json!({ "$ref": "#/$defs/x" });
        assert!(matches!(
            check_schema(&with_ref, "input", "g9-x"),
            Err(PackageError::SchemaRef { .. })
        ));
        let mut open = good.clone();
        open["additionalProperties"] = json!(true);
        assert!(matches!(
            check_schema(&open, "input", "g9-x"),
            Err(PackageError::SchemaInvalid { .. })
        ));
        let mut bad_id = good.clone();
        bad_id["$id"] = json!("https://elsewhere/x");
        assert!(check_schema(&bad_id, "input", "g9-x").is_err());
        let mut bad_req = good.clone();
        bad_req["required"] = json!(["zzz"]);
        assert!(check_schema(&bad_req, "input", "g9-x").is_err());
    }

    #[test]
    fn helpers() {
        assert!(is_semver("0.1.0"));
        assert!(!is_semver("0.1"));
        assert!(!is_semver("v0.1.0"));
        assert!(is_owner_module("M12"));
        assert!(!is_owner_module("M18"));
        assert!(!is_owner_module("M1"));
        assert_eq!(csv("STUDY,LAB").unwrap(), vec!["STUDY", "LAB"]);
        assert_eq!(csv("").unwrap(), Vec::<String>::new());
        assert!(csv("A,,B").is_err());
        assert!(csv("A,A").is_err());
        assert_eq!(parse_budget("4000", "k", BUDGET_TOKENS).unwrap(), 4000);
        assert!(parse_budget("0", "k", BUDGET_TOKENS).is_err());
        assert!(parse_budget("-1", "k", BUDGET_TOOL_CALLS).is_err());
        assert!(parse_budget("40000", "k", BUDGET_TOKENS).is_err());
        let code = SkillCode::parse("T01").unwrap();
        assert!(valid_case_id("T01-POS-001", &code, "POS"));
        assert!(!valid_case_id("T02-POS-001", &code, "POS"));
        assert!(!valid_case_id("T01-NEG-001", &code, "POS"));
        assert!(!valid_case_id("T01-POS-1", &code, "POS"));
    }

    #[test]
    fn package_error_maps_to_precondition_failed_with_rule() {
        let g: G9Error = PackageError::MissingPrefix.into();
        assert_eq!(g.category, g9_domain::ErrorCategory::PreconditionFailed);
        assert_eq!(g.details.unwrap()["rule"], "name.g9_prefix");
    }
}
