//! Layering gate (ADR-0010, ARCHITECTURE.md §2): every workspace member's
//! manifest is read from disk and its internal (`g9-*`) dependencies are
//! checked against the dependency law. A new crate, or a new edge, fails this
//! test until the layering table below is changed deliberately.
//!
//! A manifest shows only what a crate names. What a crate pulls in through
//! other crates is checked from `Cargo.lock` (the second half of this file):
//! no SQLite below the storage layer and no network client under any default
//! member, on any platform.
//!
//! Only `std::fs` is used by the tests that run by default: no subprocess, no
//! network. One test is `#[ignore]`d: it asks `cargo metadata` for the graph
//! of the target that `deny.toml` checks, shell included, which needs the
//! Tauri crates in the local registry (not WebKit). `scripts/ci_preflight.sh`
//! and `.github/workflows/ci.yml` run it in the `rust` job.
//!
//! Registered as `[[test]] layering_gate` of `crates/g9-evaluation`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
        // L6 shell, second front end (ADR-0010 addendum): the command-line
        // driver composes the same bootstrap as the Tauri bridge and drives
        // every operation through `g9_app::dispatch`. The pure domain crates
        // and storage repositories are named by `simulate-week` and
        // `crash-drill` to materialize records that have no IPC creation path
        // yet (labelled SEEDED, never reachable from `g9 call`, both behind
        // `--scratch`), and `g9-storage` is named by `g9 restore`, an offline
        // maintenance command that replaces the database file through
        // `g9_storage::restore` and so cannot run through the pipeline of a
        // database it is replacing. Nothing depends on it and it never names
        // `rusqlite` (rule 3 test).
        "g9-cli" => {
            set.extend([
                "g9-domain",
                "g9-policy",
                "g9-storage",
                "g9-skills",
                "g9-app",
                "g9-curriculum",
                "g9-calendar",
                "g9-assessment",
                "g9-competency",
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

/// The `package = "..."` value of an inline dependency table, if any.
fn renamed_package(value: &str) -> Option<String> {
    let rest = &value[value.find("package")? + "package".len()..];
    let rest = rest.trim_start().strip_prefix('=')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
}

fn parse_manifest(text: &str) -> Manifest {
    let mut m = Manifest::default();
    let mut section = String::new();
    // Inside a `[dependencies.foo]` table: a `package = ".."` line names the real crate.
    let mut in_dep_table = false;
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            // TOML allows spaces inside a header and around its dots (`[ dependencies ]`).
            section = line
                .trim_matches(|c| c == '[' || c == ']')
                .split('.')
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(".");
            in_dep_table = false;
            // `[dependencies.foo]` / `[dev-dependencies.foo]` form.
            for table in ["dependencies.", "dev-dependencies.", "build-dependencies."] {
                if let Some(pos) = section.find(table) {
                    m.deps
                        .insert(section[pos + table.len()..].trim_matches('"').to_string());
                    in_dep_table = true;
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
        if in_dep_table && key == "package" {
            m.deps.insert(value.trim().trim_matches('"').to_string());
        }
        if section.ends_with("dependencies") {
            // `foo.workspace = true` → `foo`; `key = { package = "real" }` → `real`.
            let dep = key.split('.').next().unwrap_or(key);
            m.deps
                .insert(renamed_package(value).unwrap_or_else(|| dep.to_string()));
        }
    }
    m
}

fn workspace_members(root: &Path) -> Vec<String> {
    member_list(root, "members")
}

/// The entries of a list key of the root manifest: `members` or
/// `default-members` (`members = [` does not match the other key's line, nor
/// the reverse).
fn member_list(root: &Path, key: &str) -> Vec<String> {
    let text = fs::read_to_string(root.join("Cargo.toml")).unwrap();
    let open = format!("{key} = [");
    let mut members = Vec::new();
    let mut inside = false;
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if !inside {
            inside = line.starts_with(&open);
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
    panic!("workspace `{key}` list not found or not closed");
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
        for top in ["g9-evaluation", "g9-mentor-desktop", "g9-cli"] {
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

#[test]
fn layering_parser_sees_renamed_dependencies_and_spaced_headers() {
    // Final security review supply-1: a dependency renamed with `package =`
    // was recorded under its key, and a header written `[ dependencies ]`
    // (valid TOML) was not recognised, so a forbidden edge passed the gate.
    let m = parse_manifest(
        "[package]\nname = \"g9-policy\"\n[ dependencies ]\nrusqlite = \"0.41\"\n\
         adapter = { package = \"g9-ai-adapter\", path = \"..\" }\n\
         [ dev-dependencies . store ]\npackage = \"g9-storage\"\npath = \"..\"\n",
    );
    for dep in ["rusqlite", "g9-ai-adapter", "g9-storage"] {
        assert!(m.deps.contains(dep), "{dep} missing from {:?}", m.deps);
    }
}

// ---- What a manifest cannot show: everything a crate pulls in -------------

/// `Cargo.lock`: package `(name, version)` to its dependency entries (`name`,
/// `name version` or `name version (source)`). The lock file lists every
/// platform and every dependency kind (dev and build included), so a closure
/// computed from it is a superset of what any one build links: a crate that is
/// absent from the closure is absent from every build.
type Lock = BTreeMap<(String, String), Vec<String>>;

fn parse_lock(text: &str) -> Lock {
    fn flush(lock: &mut Lock, name: &mut String, version: &mut String, deps: &mut Vec<String>) {
        if !name.is_empty() {
            lock.insert(
                (std::mem::take(name), std::mem::take(version)),
                std::mem::take(deps),
            );
        }
        name.clear();
        version.clear();
        deps.clear();
    }
    let mut lock = Lock::new();
    let (mut name, mut version, mut deps) = (String::new(), String::new(), Vec::new());
    let mut in_deps = false;
    let mut in_package = false;
    for raw in text.lines() {
        let line = raw.trim();
        if in_deps {
            if line == "]" {
                in_deps = false;
            } else {
                deps.push(line.trim_end_matches(',').trim_matches('"').to_string());
            }
            continue;
        }
        if line.starts_with('[') {
            // `[[package]]` starts a package; any other table is not one.
            flush(&mut lock, &mut name, &mut version, &mut deps);
            in_package = line == "[[package]]";
            continue;
        }
        if !in_package {
            // The lock format version at the top, `[metadata]` at the end.
            continue;
        }
        if let Some(v) = line.strip_prefix("name = ") {
            name = v.trim_matches('"').to_string();
        } else if let Some(v) = line.strip_prefix("version = ") {
            version = v.trim_matches('"').to_string();
        } else if line == "dependencies = [" {
            in_deps = true;
        }
    }
    flush(&mut lock, &mut name, &mut version, &mut deps);
    lock
}

/// Names of every package reachable from `root` (itself included). An entry
/// that names a version follows that version only; a bare name follows every
/// version of that name in the lock file (there is exactly one when Cargo
/// writes a bare name).
fn lock_closure(lock: &Lock, root: &str) -> BTreeSet<String> {
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut stack: Vec<(String, Option<String>)> = vec![(root.to_string(), None)];
    while let Some((name, version)) = stack.pop() {
        for ((n, v), deps) in lock.range((name.clone(), String::new())..) {
            if *n != name {
                break;
            }
            if version.as_ref().is_some_and(|want| want != v) {
                continue;
            }
            if !seen.insert((n.clone(), v.clone())) {
                continue;
            }
            for entry in deps {
                let mut parts = entry.split(' ');
                let dep = parts.next().unwrap_or_default().to_string();
                stack.push((dep, parts.next().map(str::to_string)));
            }
        }
    }
    seen.into_iter().map(|(n, _)| n).collect()
}

fn load_lock() -> Lock {
    let lock = parse_lock(&fs::read_to_string(root().join("Cargo.lock")).unwrap());
    assert!(
        lock.len() > 100,
        "Cargo.lock was not parsed: {}",
        lock.len()
    );
    lock
}

/// What `deny.toml` bans (`[bans] deny`) and the target it checks
/// (`[graph] targets`): the same two facts `cargo deny check bans` uses.
fn deny_config(root: &Path) -> (BTreeSet<String>, String) {
    let text = fs::read_to_string(root.join("deny.toml")).unwrap();
    let mut section = String::new();
    let mut in_deny_list = false;
    let mut bans = BTreeSet::new();
    let mut target = String::new();
    for raw in text.lines() {
        let line = strip_comment(raw).trim();
        if line.starts_with('[') {
            section = line.trim_matches(|c| c == '[' || c == ']').to_string();
            in_deny_list = false;
            continue;
        }
        if section == "graph" && line.starts_with("targets") {
            target = line.split('"').nth(1).unwrap_or_default().to_string();
        }
        if section == "bans" {
            if line.starts_with("deny = [") {
                in_deny_list = true;
            }
            if in_deny_list {
                if let Some(name) = line
                    .split("crate = \"")
                    .nth(1)
                    .and_then(|r| r.split('"').next())
                {
                    bans.insert(name.to_string());
                }
                if line.ends_with(']') {
                    in_deny_list = false;
                }
            }
        }
    }
    (bans, target)
}

/// Package names of the members a plain `cargo build` or `cargo test` covers
/// (everything but the Tauri shell).
fn default_member_names() -> Vec<String> {
    let root = root();
    member_list(&root, "default-members")
        .into_iter()
        .map(|member| {
            let text = fs::read_to_string(root.join(&member).join("Cargo.toml")).unwrap();
            parse_manifest(&text).name
        })
        .collect()
}

/// Rules 1, 3 and 4 for everything the lower layers pull in, not only for what
/// their manifests name: no SQLite and no storage crate under the domain, the
/// policy, the AI adapter or the skill orchestrator.
#[test]
fn layering_no_sqlite_in_anything_the_lower_layers_pull_in() {
    let lock = load_lock();
    let lower = ["g9-domain", "g9-policy", "g9-ai-adapter", "g9-skills"];
    for krate in lower.iter().chain(PURE) {
        let closure = lock_closure(&lock, krate);
        assert!(closure.len() > 1, "{krate} is not in Cargo.lock");
        for sqlite in ["rusqlite", "libsqlite3-sys", "g9-storage", "g9-app"] {
            assert!(
                !closure.contains(sqlite),
                "{krate} pulls in {sqlite} through its dependencies (ARCHITECTURE.md §2 rules 1, 3, 4)"
            );
        }
    }
    // The control: the same search does find SQLite where it belongs.
    assert!(lock_closure(&lock, "g9-storage").contains("libsqlite3-sys"));
}

/// R-001 for everything a default member pulls in, on every platform: no
/// network client from the list above and no crate `deny.toml` bans.
#[test]
fn layering_no_network_client_in_anything_the_default_members_pull_in() {
    let lock = load_lock();
    let (bans, _) = deny_config(&root());
    assert!(
        bans.contains("reqwest") && bans.contains("hyper"),
        "the ban list of deny.toml was not read: {bans:?}"
    );
    let members = default_member_names();
    assert!(members.len() >= 13, "default members: {members:?}");
    for member in &members {
        let closure = lock_closure(&lock, member);
        assert!(closure.len() > 1, "{member} is not in Cargo.lock");
        for banned in bans
            .iter()
            .map(String::as_str)
            .chain(NETWORK_CLIENTS.iter().copied())
        {
            assert!(
                !closure.contains(banned),
                "{member} pulls in {banned} through its dependencies (deny.toml bans, R-001)"
            );
        }
    }
    // The control, and the reason the shell is not checked from the lock file:
    // Tauri names an HTTP client for platforms this application is not built
    // for. The test below checks the shell on the target `deny.toml` names.
    assert!(
        lock_closure(&lock, "g9-mentor-desktop").contains("tauri"),
        "the shell's closure was not read"
    );
}

/// The graph Cargo resolves for the target `deny.toml` checks, shell
/// included: no banned crate under any workspace member. This is what
/// `cargo deny check bans` decides, without that tool.
#[test]
#[ignore = "resolves the whole workspace: needs the Tauri crates in the local registry (desktop-shell job)"]
fn layering_no_banned_crate_in_the_deny_target_graph_of_any_member() {
    let root = root();
    let (bans, target) = deny_config(&root);
    assert!(!bans.is_empty() && !target.is_empty(), "deny.toml not read");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = Command::new(cargo)
        .current_dir(&root)
        .args(["metadata", "--format-version", "1", "--locked", "--offline"])
        .args(["--filter-platform", &target])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let name_of: BTreeMap<&str, &str> = meta["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| (p["id"].as_str().unwrap(), p["name"].as_str().unwrap()))
        .collect();
    // Every edge of the resolved graph, whatever its kind: what cargo-deny reads.
    let deps_of: BTreeMap<&str, Vec<&str>> = meta["resolve"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| {
            let deps = n["deps"].as_array().unwrap();
            (
                n["id"].as_str().unwrap(),
                deps.iter().map(|d| d["pkg"].as_str().unwrap()).collect(),
            )
        })
        .collect();
    let mut examined = BTreeSet::new();
    for member in meta["workspace_members"].as_array().unwrap() {
        let member = member.as_str().unwrap();
        let mut seen = BTreeSet::new();
        let mut stack = vec![member];
        while let Some(id) = stack.pop() {
            if seen.insert(id) {
                stack.extend(deps_of[id].iter().copied());
            }
        }
        let names: BTreeSet<&str> = seen.iter().map(|id| name_of[id]).collect();
        for banned in &bans {
            assert!(
                !names.contains(banned.as_str()),
                "{} pulls in {banned} on {target} (deny.toml bans)",
                name_of[member]
            );
        }
        if name_of[member] == "g9-mentor-desktop" {
            assert!(
                names.contains("tauri"),
                "the shell's graph was not resolved"
            );
        }
        examined.insert(name_of[member]);
    }
    assert!(examined.contains("g9-mentor-desktop"), "{examined:?}");
    assert!(examined.len() >= 14, "{examined:?}");
}

#[test]
fn layering_lock_closure_follows_versions_and_finds_transitive_edges() {
    let lock = parse_lock(
        "# generated\nversion = 4\n\n[[package]]\nname = \"pure\"\nversion = \"0.1.0\"\n\
         dependencies = [\n \"helper\",\n]\n\n[[package]]\nname = \"helper\"\nversion = \"1.0.0\"\n\
         source = \"registry+x\"\ndependencies = [\n \"rusqlite 0.41.1\",\n]\n\n\
         [[package]]\nname = \"rusqlite\"\nversion = \"0.30.0\"\ndependencies = [\n \"old-only\",\n]\n\n\
         [[package]]\nname = \"rusqlite\"\nversion = \"0.41.1\"\ndependencies = [\n \
         \"libsqlite3-sys 0.33.0 (registry+x)\",\n]\n\n[[package]]\nname = \"libsqlite3-sys\"\n\
         version = \"0.33.0\"\n\n[[package]]\nname = \"old-only\"\nversion = \"1.0.0\"\n\n\
         [[package]]\nname = \"unrelated\"\nversion = \"2.0.0\"\n\n[metadata]\nname = \"not-a-package\"\n",
    );
    assert_eq!(lock.len(), 7, "{:?}", lock.keys().collect::<Vec<_>>());
    let closure = lock_closure(&lock, "pure");
    // A dependency two steps away is found: the manifest of `pure` names only `helper`.
    assert!(closure.contains("rusqlite") && closure.contains("libsqlite3-sys"));
    // A versioned entry follows that version only.
    assert!(!closure.contains("old-only"), "{closure:?}");
    assert!(!closure.contains("unrelated"));
    assert_eq!(lock_closure(&lock, "missing").len(), 0);
}

#[test]
fn layering_deny_configuration_is_read() {
    let (bans, target) = deny_config(&root());
    assert!(
        bans.contains("reqwest") && bans.contains("hyper"),
        "{bans:?}"
    );
    assert!(target.contains("linux"), "deny.toml checks {target:?}");
}
