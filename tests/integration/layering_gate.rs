//! Layering gate (ADR-0010, ARCHITECTURE.md §2): every workspace member's
//! manifest is read from disk and its internal (`g9-*`) dependencies are
//! checked against the dependency law. A new crate, or a new edge, fails this
//! test until the layering table below is changed deliberately.
//!
//! Only `std::fs` is used: no `cargo metadata` subprocess, no network.
//!
//! Registered as `[[test]] layering_gate` of `crates/g9-evaluation`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Pure domain crates: depend only on `g9-domain` (§2 rule 1).
const PURE: &[&str] = &[
    "g9-curriculum",
    "g9-calendar",
    "g9-evidence",
    "g9-competency",
    "g9-assessment",
    "g9-retention",
];

/// External crates that give a network client capability; none may appear in
/// any manifest (CLAUDE.md integrity rules, ARCHITECTURE.md §13).
const NETWORK_CLIENTS: &[&str] = &[
    "reqwest",
    "hyper",
    "ureq",
    "curl",
    "isahc",
    "surf",
    "attohttpc",
];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Allowed internal dependencies per package, from §2 rules 1–7.
fn allowed(package: &str) -> Option<BTreeSet<&'static str>> {
    let mut set: BTreeSet<&'static str> = BTreeSet::new();
    match package {
        "g9-domain" => {}
        "g9-policy" | "g9-ai-adapter" => {
            set.insert("g9-domain");
        }
        p if PURE.contains(&p) => {
            set.insert("g9-domain");
        }
        // Rule 2: domain + pure domain crates (row ↔ type mapping); never
        // app, skills or ai-adapter.
        "g9-storage" => {
            set.insert("g9-domain");
            set.extend(PURE.iter().copied());
        }
        // Rule 4: policy + ai-adapter; never storage.
        "g9-skills" => {
            set.extend(["g9-domain", "g9-policy", "g9-ai-adapter"]);
        }
        // Rule 5: the only composer of policy + storage + skills.
        "g9-app" => {
            set.extend([
                "g9-domain",
                "g9-policy",
                "g9-storage",
                "g9-ai-adapter",
                "g9-skills",
            ]);
            set.extend(PURE.iter().copied());
        }
        // Harness above g9-app; nothing depends on it.
        "g9-evaluation" => {
            set.extend([
                "g9-domain",
                "g9-policy",
                "g9-storage",
                "g9-ai-adapter",
                "g9-skills",
                "g9-app",
            ]);
            set.extend(PURE.iter().copied());
        }
        // Rule 6: the shell exposes only g9-app commands; storage, policy and
        // skills are named for bootstrap composition (`Database::open`,
        // `BindingKey`, `Registry::load`) only.
        "g9-mentor-desktop" => {
            set.extend([
                "g9-domain",
                "g9-policy",
                "g9-storage",
                "g9-skills",
                "g9-app",
            ]);
        }
        _ => return None,
    }
    Some(set)
}

/// Crates that may name `rusqlite` directly (rule 3: the AI adapter and the
/// pure crates never touch SQLite).
const SQLITE_ALLOWED: &[&str] = &["g9-storage", "g9-app"];

/// Crates that may name `tauri` directly (rule 6).
const TAURI_ALLOWED: &[&str] = &["g9-mentor-desktop"];

#[derive(Debug, Default)]
struct Manifest {
    name: String,
    /// Every dependency key in any `*dependencies` table (normal, dev, build,
    /// target-specific).
    deps: BTreeSet<String>,
}

fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(i) => &line[..i],
        None => line,
    }
}

fn parse_manifest(text: &str) -> Manifest {
    let mut m = Manifest::default();
    let mut section = String::new();
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']').to_string();
            // `[dependencies.foo]` / `[dev-dependencies.foo]` form.
            for table in ["dependencies.", "dev-dependencies.", "build-dependencies."] {
                if let Some(pos) = section.find(table) {
                    m.deps
                        .insert(section[pos + table.len()..].trim_matches('"').to_string());
                }
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().trim_matches('"');
        if section == "package" && key == "name" {
            m.name = value.trim().trim_matches('"').to_string();
        }
        if section.ends_with("dependencies") {
            // `foo.workspace = true` → `foo`.
            let dep = key.split('.').next().unwrap_or(key);
            m.deps.insert(dep.to_string());
        }
    }
    m
}

fn workspace_members(root: &Path) -> Vec<String> {
    let text = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    // The `members` key itself, not `default-members`.
    let mut members = Vec::new();
    let mut inside = false;
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if !inside {
            inside = line.starts_with("members = [");
            continue;
        }
        if line.starts_with(']') {
            return members;
        }
        let entry = line.trim_end_matches(',').trim().trim_matches('"');
        if !entry.is_empty() {
            members.push(entry.to_string());
        }
    }
    panic!("workspace `members` list not found or not closed");
}

fn load() -> BTreeMap<String, Manifest> {
    let root = root();
    let members = workspace_members(&root);
    assert!(
        members.len() >= 14,
        "expected 13 crates plus the Tauri shell, found {members:?}"
    );
    let mut out = BTreeMap::new();
    for member in members {
        let text = fs::read_to_string(root.join(&member).join("Cargo.toml"))
            .unwrap_or_else(|e| panic!("{member}/Cargo.toml: {e}"));
        let m = parse_manifest(&text);
        assert!(!m.name.is_empty(), "{member}: package name not found");
        out.insert(m.name.clone(), m);
    }
    out
}

#[test]
fn layering_every_member_has_a_declared_layer() {
    for name in load().keys() {
        assert!(
            allowed(name).is_some(),
            "{name} has no entry in the layering table (ADR-0010)"
        );
    }
}

/// Internal edges not permitted by [`allowed`].
fn forbidden_edges(manifests: &BTreeMap<String, Manifest>) -> Vec<String> {
    let mut violations = Vec::new();
    for (name, m) in manifests {
        let Some(ok) = allowed(name) else { continue };
        for dep in m.deps.iter().filter(|d| d.starts_with("g9-")) {
            if !ok.contains(dep.as_str()) {
                violations.push(format!("{name} -> {dep}"));
            }
        }
    }
    violations
}

/// Kahn's algorithm over the internal edges: `true` iff every node can be
/// removed, i.e. there is no cycle.
fn is_acyclic(manifests: &BTreeMap<String, Manifest>) -> bool {
    let graph: BTreeMap<&str, Vec<&str>> = manifests
        .iter()
        .map(|(n, m)| {
            (
                n.as_str(),
                m.deps
                    .iter()
                    .filter(|d| manifests.contains_key(d.as_str()))
                    .map(String::as_str)
                    .collect(),
            )
        })
        .collect();
    let mut indegree: BTreeMap<&str, usize> = graph.keys().map(|k| (*k, 0)).collect();
    for deps in graph.values() {
        for d in deps {
            *indegree.get_mut(d).unwrap() += 1;
        }
    }
    let mut ready: Vec<&str> = indegree
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(k, _)| *k)
        .collect();
    let mut removed = 0;
    while let Some(node) = ready.pop() {
        removed += 1;
        for d in &graph[node] {
            let n = indegree.get_mut(d).unwrap();
            *n -= 1;
            if *n == 0 {
                ready.push(*d);
            }
        }
    }
    removed == graph.len()
}

fn synthetic(edges: &[(&str, &[&str])]) -> BTreeMap<String, Manifest> {
    edges
        .iter()
        .map(|(name, deps)| {
            (
                (*name).to_string(),
                Manifest {
                    name: (*name).to_string(),
                    deps: deps.iter().map(|d| (*d).to_string()).collect(),
                },
            )
        })
        .collect()
}

#[test]
fn layering_internal_edges_follow_the_dependency_law() {
    let violations = forbidden_edges(&load());
    assert!(violations.is_empty(), "forbidden edges: {violations:?}");
}

#[test]
fn layering_nothing_depends_on_the_harness_or_the_shell() {
    for (name, m) in load() {
        for top in ["g9-evaluation", "g9-mentor-desktop"] {
            assert!(!m.deps.contains(top), "{name} depends on {top}");
        }
    }
}

#[test]
fn layering_graph_is_acyclic() {
    assert!(
        is_acyclic(&load()),
        "dependency cycle among workspace members"
    );
}

#[test]
fn layering_checks_detect_a_forbidden_edge_and_a_cycle() {
    // Rule 2 and rule 4 violations are reported by name.
    let bad = synthetic(&[
        ("g9-domain", &[]),
        ("g9-storage", &["g9-domain", "g9-app"]),
        ("g9-skills", &["g9-domain", "g9-storage"]),
        ("g9-app", &["g9-domain", "g9-storage"]),
    ]);
    assert_eq!(
        forbidden_edges(&bad),
        vec!["g9-skills -> g9-storage", "g9-storage -> g9-app"]
    );
    // storage -> app -> storage is a cycle.
    assert!(!is_acyclic(&bad));
    let good = synthetic(&[
        ("g9-domain", &[]),
        ("g9-storage", &["g9-domain"]),
        ("g9-app", &["g9-domain", "g9-storage"]),
    ]);
    assert!(forbidden_edges(&good).is_empty());
    assert!(is_acyclic(&good));
}

#[test]
fn layering_sqlite_tauri_and_network_clients_are_confined() {
    for (name, m) in load() {
        if m.deps.contains("rusqlite") {
            assert!(
                SQLITE_ALLOWED.contains(&name.as_str()),
                "{name} names rusqlite"
            );
        }
        if m.deps.contains("tauri") {
            assert!(TAURI_ALLOWED.contains(&name.as_str()), "{name} names tauri");
        }
        for net in NETWORK_CLIENTS {
            assert!(!m.deps.contains(*net), "{name} names network client {net}");
        }
    }
}

#[test]
fn layering_parser_reads_all_dependency_table_forms() {
    let m = parse_manifest(
        "[package]\nname = \"x\" # c\n[dependencies]\ng9-domain = { workspace = true }\n\
         g9-policy.workspace = true\n[dev-dependencies]\ntempfile = \"3\"\n\
         [target.'cfg(unix)'.dependencies]\nrusqlite = \"0.41\"\n[dependencies.g9-app]\npath = \"..\"\n",
    );
    assert_eq!(m.name, "x");
    let want: BTreeSet<String> = ["g9-domain", "g9-policy", "tempfile", "rusqlite", "g9-app"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(m.deps, want);
}
