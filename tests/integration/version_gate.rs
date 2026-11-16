//! Version gate (plan step BUILD-3, `docs/release/path-to-10.md`): the
//! application has one version. It is written once, in `[workspace.package]`
//! of the root manifest; every workspace member inherits it, and the desktop
//! package (`apps/desktop/package.json`, its lock file) repeats it. The Tauri
//! configuration carries no version of its own, so the bundle takes the shell
//! crate's. A release whose parts disagree fails here.
//!
//! Only `std::fs` is used: no subprocess, no network.
//!
//! Registered as `[[test]] version_gate` of `crates/g9-evaluation`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(i) => &line[..i],
        None => line,
    }
}

/// How a manifest states the version of its package.
#[derive(Debug, PartialEq, Eq)]
enum Declared {
    /// `version.workspace = true` or `version = { workspace = true }`.
    Inherited,
    /// `version = "x.y.z"`.
    Literal(String),
    Missing,
}

/// The `version` key of one table of a manifest (`package` or
/// `workspace.package`).
fn declared_version(manifest: &str, table: &str) -> Declared {
    let mut section = String::new();
    for raw in manifest.lines() {
        let line = strip_comment(raw).trim();
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']').to_string();
            continue;
        }
        if section != table {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        if key == "version.workspace" && value == "true" {
            return Declared::Inherited;
        }
        if key == "version" {
            if value.starts_with('{') {
                let inner: String = value.chars().filter(|c| !c.is_whitespace()).collect();
                if inner == "{workspace=true}" {
                    return Declared::Inherited;
                }
                return Declared::Missing;
            }
            return Declared::Literal(value.trim_matches('"').to_string());
        }
    }
    Declared::Missing
}

fn members(root_manifest: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut inside = false;
    for raw in root_manifest.lines() {
        let line = strip_comment(raw).trim();
        if !inside {
            inside = line.starts_with("members = [");
            continue;
        }
        if line.starts_with(']') {
            return out;
        }
        let entry = line.trim_end_matches(',').trim().trim_matches('"');
        if !entry.is_empty() {
            out.push(entry.to_string());
        }
    }
    panic!("workspace `members` list not found or not closed");
}

/// Every place that states a version other than `expected`, as
/// `place: found`.
fn disagreements(expected: &str, stated: &[(String, Option<String>)]) -> Vec<String> {
    stated
        .iter()
        .filter(|(_, found)| found.as_deref() != Some(expected))
        .map(|(place, found)| match found {
            Some(v) => format!("{place}: {v}"),
            None => format!("{place}: no version"),
        })
        .collect()
}

fn workspace_version() -> String {
    let manifest = fs::read_to_string(root().join("Cargo.toml")).unwrap();
    match declared_version(&manifest, "workspace.package") {
        Declared::Literal(v) => v,
        other => panic!("[workspace.package] version: {other:?}"),
    }
}

fn json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn version_is_a_plain_release_number() {
    let v = workspace_version();
    let parts: Vec<&str> = v.split('.').collect();
    assert!(
        parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok()),
        "[workspace.package] version is {v:?}, expected MAJOR.MINOR.PATCH"
    );
}

/// A member with its own literal version can fall behind without any build
/// noticing; inheriting is the only form that cannot.
#[test]
fn version_every_workspace_member_inherits_it() {
    let root = root();
    let manifest = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let members = members(&manifest);
    assert!(members.len() >= 14, "{members:?}");
    for member in members {
        let text = fs::read_to_string(root.join(&member).join("Cargo.toml")).unwrap();
        assert_eq!(
            declared_version(&text, "package"),
            Declared::Inherited,
            "{member}/Cargo.toml must say `version.workspace = true`"
        );
    }
}

#[test]
fn version_the_desktop_package_and_its_lock_file_repeat_it() {
    let root = root();
    let package = json(&root.join("apps/desktop/package.json"));
    let lock = json(&root.join("apps/desktop/package-lock.json"));
    let text = |v: &serde_json::Value| v.as_str().map(str::to_string);
    let stated = [
        (
            "apps/desktop/package.json".to_string(),
            text(&package["version"]),
        ),
        (
            "apps/desktop/package-lock.json".to_string(),
            text(&lock["version"]),
        ),
        (
            "apps/desktop/package-lock.json packages[\"\"]".to_string(),
            text(&lock["packages"][""]["version"]),
        ),
    ];
    let found = disagreements(&workspace_version(), &stated);
    assert!(
        found.is_empty(),
        "the workspace is {}, but {found:?}",
        workspace_version()
    );
}

/// With no `version` key Tauri takes the version of the shell crate, which
/// inherits the workspace's. A key here would be a second source.
#[test]
fn version_the_tauri_configuration_states_none_of_its_own() {
    let conf = json(&root().join("apps/desktop/src-tauri/tauri.conf.json"));
    match conf.get("version") {
        None => {}
        Some(v) => assert_eq!(
            v.as_str(),
            Some(workspace_version().as_str()),
            "tauri.conf.json states its own version"
        ),
    }
}

#[test]
fn version_checks_detect_a_literal_and_a_disagreement() {
    let inherited = "[package]\nname = \"a\"\nversion.workspace = true\n";
    let inline = "[package]\nname = \"a\"\nversion = { workspace = true } # same thing\n";
    let literal = "[package]\nname = \"a\"\nversion = \"0.1.0\"\n";
    let missing = "[package]\nname = \"a\"\n[dependencies]\nversion = \"9.9.9\"\n";
    assert_eq!(declared_version(inherited, "package"), Declared::Inherited);
    assert_eq!(declared_version(inline, "package"), Declared::Inherited);
    assert_eq!(
        declared_version(literal, "package"),
        Declared::Literal("0.1.0".into())
    );
    // A `version` key of another table is not the package's version.
    assert_eq!(declared_version(missing, "package"), Declared::Missing);

    let stated = [
        ("package.json".to_string(), Some("0.1.0".to_string())),
        ("package-lock.json".to_string(), Some("0.1.1".to_string())),
        ("somewhere".to_string(), None),
    ];
    assert_eq!(
        disagreements("0.1.0", &stated),
        vec!["package-lock.json: 0.1.1", "somewhere: no version"]
    );
}
