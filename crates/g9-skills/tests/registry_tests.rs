#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Registry tests.

mod common;

use common::*;
use g9_domain::{DataDomain, InteractionMode, SkillCode, SkillFamily, SkillId};
use g9_skills::{Registry, RegistryError};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

#[test]
fn registry_expect_complete_reports_missing_codes() {
    let r = registry();
    assert_eq!(r.len(), 1);
    let err = r.expect_complete().unwrap_err();
    assert_eq!(err.missing.len(), 36);
    assert!(!err.missing.contains(&SkillCode::parse("T01").unwrap()));
    assert!(err.missing.contains(&SkillCode::parse("E01").unwrap()));
    assert!(err.missing.contains(&SkillCode::parse("K03").unwrap()));
    assert!(err.duplicates.is_empty());
    assert!(err.misbound.is_empty());
    let g: g9_domain::G9Error = err.into();
    assert_eq!(g.code, "skills.registry.incomplete");

    // duplicates and misbinding are reported, not silently merged
    let mut copy = package();
    copy.name = SkillId::parse("g9-session-director-copy").unwrap();
    let r2 = Registry::from_packages(valid_root(), vec![package(), copy]);
    let err = r2.expect_complete().unwrap_err();
    assert_eq!(err.duplicates.len(), 1);
    assert_eq!(err.duplicates[0].0.as_str(), "T01");
    assert_eq!(err.misbound.len(), 1);
    assert_eq!(err.misbound[0].1.as_str(), "g9-session-director-copy");
}

#[test]
fn registry_hash_mismatch_detected() {
    let r = registry();
    let pinned = r.hashes();
    assert!(r.verify_hashes(&pinned).is_empty());

    let name = SkillId::parse("g9-session-director").unwrap();
    let mut tampered = pinned.clone();
    tampered.insert(name.clone(), "0".repeat(64));
    let m = r.verify_hashes(&tampered);
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].skill, name);
    assert_eq!(m[0].expected.as_deref(), Some("0".repeat(64).as_str()));
    assert_eq!(
        m[0].actual.as_deref(),
        pinned.get(&name).map(String::as_str)
    );

    // a loaded package without a pin and a pin without a package are both reported
    let empty: BTreeMap<SkillId, String> = BTreeMap::new();
    assert_eq!(r.verify_hashes(&empty).len(), 1);
    let mut extra = pinned.clone();
    extra.insert(SkillId::parse("g9-ghost").unwrap(), "ab".repeat(32));
    let m = r.verify_hashes(&extra);
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].actual, None);

    // load_pinned refuses the whole registry on mismatch (supply-chain rule)
    assert!(matches!(
        Registry::load_pinned(&valid_root(), &tampered),
        Err(RegistryError::HashMismatch(v)) if v.len() == 1
    ));
    assert!(Registry::load_pinned(&valid_root(), &pinned).is_ok());

    // editing any file changes the hash: compare against the invalid variant copies
    let changed = g9_skills::load_package(&invalid_pkg_dir("missing_references"));
    assert!(changed.is_err());
    let v = g9_skills::package::package_sha256(&invalid_pkg_dir("missing_references")).unwrap();
    assert_ne!(v, pinned[&name]);
}

#[test]
fn registry_candidates_and_lookups() {
    let r = registry();
    assert_eq!(
        r.candidates_for(InteractionMode::Study, DataDomain::Learning)
            .len(),
        1
    );
    assert_eq!(
        r.candidates_for(InteractionMode::Lab, DataDomain::Learning)
            .len(),
        1
    );
    assert!(r
        .candidates_for(InteractionMode::Exam, DataDomain::Learning)
        .is_empty());
    assert!(r
        .candidates_for(InteractionMode::Study, DataDomain::Business)
        .is_empty());
    assert_eq!(r.by_family(SkillFamily::Teaching).len(), 1);
    assert!(r.by_family(SkillFamily::Engineering).is_empty());
    assert!(r
        .get(&SkillId::parse("g9-session-director").unwrap())
        .is_some());
    assert!(r.get(&SkillId::parse("g9-nope").unwrap()).is_none());
    assert!(r.by_code(&SkillCode::parse("T01").unwrap()).is_some());
    assert_eq!(r.root(), valid_root());
}

#[test]
fn registry_load_reports_every_invalid_package() {
    let err = Registry::load(&invalid_root("missing_section")).unwrap_err();
    match err {
        RegistryError::Packages(failures) => {
            assert_eq!(failures.len(), 1);
            assert_eq!(failures[0].dir, "g9-session-director");
            assert_eq!(failures[0].error.rule(), "body.sections");
        }
        other => panic!("unexpected {other:?}"),
    }
    let g: g9_domain::G9Error = Registry::load(&invalid_root("bad_name"))
        .unwrap_err()
        .into();
    assert_eq!(g.code, "skills.registry.invalid_packages");
}
