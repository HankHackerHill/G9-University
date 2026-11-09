#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Package loader tests against `tests/fixtures/`.

mod common;

use common::*;
use g9_domain::{Capability, DataDomain, InteractionMode, SkillFamily};
use g9_skills::{load_package, EvalKind, EvalRunner, PackageError, PackageStatus};
use pretty_assertions::assert_eq;

#[test]
fn package_valid_fixture_loads() {
    let p = package();
    assert_eq!(p.name.as_str(), "g9-session-director");
    assert_eq!(p.code.as_str(), "T01");
    assert_eq!(p.family, SkillFamily::Teaching);
    assert_eq!(p.version, "0.1.0");
    assert_eq!(p.owner_module, "M12");
    assert_eq!(p.modes, vec![InteractionMode::Study, InteractionMode::Lab]);
    assert_eq!(p.domains, vec![DataDomain::Learning]);
    assert_eq!(
        p.requested_capabilities,
        vec![Capability::SkillInvoke, Capability::ModelInference]
    );
    assert_eq!(
        (
            p.budget.max_tokens,
            p.budget.max_seconds,
            p.budget.max_tool_calls
        ),
        (4000, 60, 0)
    );
    assert_eq!(p.recursion_depth, 0);
    assert!(p.depends_on.is_empty());
    assert_eq!(p.status, PackageStatus::Proposed);
    assert_eq!(p.body_sections, g9_skills::package::REQUIRED_SECTIONS);
    assert_eq!(p.evals.positive.len(), 3);
    assert_eq!(p.evals.negative.len(), 3);
    assert_eq!(p.evals.adversarial.len(), 3);
    assert_eq!(p.evals.acceptance.len(), 2);
    assert_eq!(p.package_sha256.len(), 64);
    assert!(p.package_sha256.chars().all(|c| c.is_ascii_hexdigit()));
    assert!(p.body.contains("## Mission"));
    // deterministic hash
    assert_eq!(
        load_package(&valid_pkg_dir()).unwrap().package_sha256,
        p.package_sha256
    );
    // static eval shape checks pass on the fixture
    let report = EvalRunner::check_static(&p);
    assert!(report.passed(), "{:?}", report.issues);
    assert_eq!(report.counts[&EvalKind::Positive], 3);
}

#[test]
fn package_rejects_each_contract_violation() {
    type Matcher = fn(&PackageError) -> bool;
    let cases: Vec<(&str, Matcher)> = vec![
        ("missing_section", |e| {
            matches!(e, PackageError::SectionsMismatch { .. })
        }),
        ("operator_only_capability", |e| {
            matches!(
                e,
                PackageError::OperatorOnlyCapability(Capability::ShellExec)
            )
        }),
        (
            "recursion_depth_1",
            |e| matches!(e, PackageError::RecursionDepth(v) if v == "1"),
        ),
        ("bad_name", |e| matches!(e, PackageError::InvalidName(_))),
        ("name_mismatch", |e| {
            matches!(e, PackageError::NameMismatch { .. })
        }),
        (
            "ref_in_schema",
            |e| matches!(e, PackageError::SchemaRef { which, .. } if which == "output"),
        ),
        (
            "eval_id_wrong_prefix",
            |e| matches!(e, PackageError::EvalIdPrefix { id, .. } if id == "T02-ADV-002"),
        ),
        (
            "unknown_metadata_key",
            |e| matches!(e, PackageError::MetadataUnknownKeys(k) if k == &["g9-extra".to_string()]),
        ),
        ("family_mismatch", |e| {
            matches!(e, PackageError::FamilyMismatch { .. })
        }),
        ("missing_references", |e| {
            matches!(e, PackageError::MissingReferences)
        }),
        (
            "eval_too_few",
            |e| matches!(e, PackageError::EvalCount { file, found: 1, minimum: 2 } if file == "acceptance.json"),
        ),
        ("eval_kind_mismatch", |e| {
            matches!(
                e,
                PackageError::EvalKindMismatch {
                    expected: EvalKind::Negative,
                    ..
                }
            )
        }),
        (
            "schema_open",
            |e| matches!(e, PackageError::SchemaInvalid { which, .. } if which == "input"),
        ),
        (
            "budget_out_of_range",
            |e| matches!(e, PackageError::InvalidBudget { key, .. } if key == "g9-budget-tokens"),
        ),
        (
            "unknown_frontmatter_key",
            |e| matches!(e, PackageError::Frontmatter(g9_skills::FrontmatterError::UnknownKey(k)) if k == "author"),
        ),
        ("activation_one_negative", |e| {
            matches!(e, PackageError::ActivationTriggers(_))
        }),
        ("self_dependency", |e| {
            matches!(e, PackageError::InvalidDependency(_))
        }),
    ];
    for (variant, matcher) in cases {
        let err = load_package(&invalid_pkg_dir(variant)).expect_err(variant);
        assert!(matcher(&err), "variant {variant}: unexpected error {err:?}");
        assert!(!err.rule().is_empty());
    }
    assert!(matches!(
        load_package(&fixtures().join("does-not-exist")),
        Err(PackageError::NotADirectory(_))
    ));
}

/// Copies `src` into `dst` (std only; the package has no symlinks).
fn copy_tree(src: &std::path::Path, dst: &std::path::Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &to);
        } else {
            std::fs::copy(entry.path(), &to).unwrap();
        }
    }
}

/// A copy of the valid fixture under `CARGO_TARGET_TMPDIR/<case>/<name>`,
/// with a fixed path so the test stays deterministic and re-runnable.
fn scratch_copy_of_valid(case: &str) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("g9-skills-{case}"));
    let _ = std::fs::remove_dir_all(&root);
    let pkg = root.join("g9-session-director");
    copy_tree(&valid_pkg_dir(), &pkg);
    pkg
}

/// Git cannot store an empty directory, so a fixture directory that must be
/// empty vanishes on a fresh checkout. Both shapes of the violation (directory
/// absent, directory present with no `*.md`) must therefore be rejected with
/// the same contract error, and each is built here rather than checked in.
#[test]
fn package_missing_references_is_a_contract_violation_whether_absent_or_empty() {
    let absent = scratch_copy_of_valid("refs-absent");
    std::fs::remove_dir_all(absent.join("references")).unwrap();
    assert!(!absent.join("references").exists());
    assert_eq!(
        load_package(&absent).unwrap_err(),
        PackageError::MissingReferences
    );

    let empty = scratch_copy_of_valid("refs-empty");
    for e in std::fs::read_dir(empty.join("references")).unwrap() {
        std::fs::remove_file(e.unwrap().path()).unwrap();
    }
    assert!(empty.join("references").is_dir());
    assert_eq!(
        load_package(&empty).unwrap_err(),
        PackageError::MissingReferences
    );

    let not_markdown = scratch_copy_of_valid("refs-not-markdown");
    for e in std::fs::read_dir(not_markdown.join("references")).unwrap() {
        std::fs::remove_file(e.unwrap().path()).unwrap();
    }
    std::fs::write(not_markdown.join("references").join("notes.txt"), "x").unwrap();
    assert_eq!(
        load_package(&not_markdown).unwrap_err(),
        PackageError::MissingReferences
    );

    // the unmodified copy still loads, so the three failures are caused by
    // the removed references and nothing else
    let intact = scratch_copy_of_valid("refs-intact");
    assert!(load_package(&intact).is_ok());
}

#[test]
fn a07_requested_operator_only_capability_rejected_at_load() {
    let err = load_package(&invalid_pkg_dir("operator_only_capability")).unwrap_err();
    assert_eq!(
        err,
        PackageError::OperatorOnlyCapability(Capability::ShellExec)
    );
    assert_eq!(err.rule(), "metadata.capabilities.operator_only");
    let g: g9_domain::G9Error = err.into();
    assert_eq!(g.category, g9_domain::ErrorCategory::PreconditionFailed);
    assert!(!g.message.contains('\n'));
}

#[test]
fn eval_run_against_model_is_explicitly_not_implemented() {
    let p = package();
    let err = EvalRunner::run_against_model(&p, &g9_skills::UnavailableBackend).unwrap_err();
    assert_eq!(err.category, g9_domain::ErrorCategory::PreconditionFailed);
    assert_eq!(err.code, "skills.eval.not_implemented");
    assert_eq!(err.details.unwrap()["status"], "NOT RUN");
}
