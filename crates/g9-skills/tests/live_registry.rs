#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Live registry gate: `.agents/skills` must hold all 37 packages, each valid.
//!
//! The directory is populated by the content team (wave 1b). While it is
//! incomplete this test is NOT RUN by default and reports
//! FAILING-PENDING-CONTENT when executed with
//! `cargo test -p g9-skills --test live_registry -- --ignored`.
//! Remove the `#[ignore]` attribute once the 37 packages have landed; the
//! assertion itself must not be weakened.

use g9_skills::Registry;
use std::path::Path;

fn live_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.agents/skills")
}

#[test]

fn live_registry_is_complete_and_valid() {
    let root = live_root();
    let registry = match Registry::load(&root) {
        Ok(r) => r,
        Err(g9_skills::RegistryError::Packages(failures)) => {
            for f in &failures {
                eprintln!("{}: [{}] {}", f.dir, f.error.rule(), f.error);
            }
            panic!("{} live package(s) failed validation", failures.len());
        }
        Err(e) => panic!("{e}"),
    };
    if let Err(missing) = registry.expect_complete() {
        panic!(
            "live registry incomplete: missing {:?}, duplicates {:?}, misbound {:?}",
            missing
                .missing
                .iter()
                .map(|c| c.as_str())
                .collect::<Vec<_>>(),
            missing.duplicates,
            missing.misbound
        );
    }
    assert_eq!(registry.len(), 37);
    for p in registry.iter() {
        let report = g9_skills::EvalRunner::check_static(p);
        assert!(report.passed(), "{}: {:?}", p.name, report.issues);
    }
}

/// The pins compiled into the desktop binary (`apps/desktop/src-tauri/skill-pins.json`) must be
/// what the loader computes for the packages in the repository: the call the shell makes
/// (`Registry::load_pinned`) accepts the shipped tree under the shipped pins. The same file is
/// checked against an independent implementation by `scripts/skill_hashes.py --check`.
#[test]
fn live_registry_loads_under_the_shipped_pins() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/src-tauri/skill-pins.json");
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(doc["algorithm"], "g9_skills::package::package_sha256");
    let pins: std::collections::BTreeMap<g9_domain::SkillId, String> = doc["pins"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(name, hash)| {
            (
                g9_domain::SkillId::parse(name).unwrap(),
                hash.as_str().unwrap().to_string(),
            )
        })
        .collect();
    assert_eq!(pins.len(), 37);
    let registry = Registry::load_pinned(&live_root(), &pins)
        .unwrap_or_else(|e| panic!("the shipped packages do not match the shipped pins: {e}"));
    assert_eq!(registry.len(), 37);
}
