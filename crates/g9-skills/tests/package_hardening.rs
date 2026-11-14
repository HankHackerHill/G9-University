#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! A package is read once, from regular files only, within fixed bounds (audit 2026-11-12, LOW
//! "skill pin verification hashes a second read of the package"): what is parsed is what is
//! hashed, an entry that is not a regular file refuses the package instead of being skipped or
//! waited on, and a package that differs from its pin is refused before any of it is parsed.

mod common;

use common::*;
use g9_skills::package::{package_sha256, MAX_FILE_BYTES};
use g9_skills::{load_package, PackageError, Registry, RegistryError};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Copies `src` into `dst` (std only; the fixtures hold no symlinks).
fn copy_tree(src: &Path, dst: &Path) {
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

/// A fresh copy of the valid fixture root under `CARGO_TARGET_TMPDIR/g9-skills-<case>`.
fn scratch_root(case: &str) -> PathBuf {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("g9-skills-{case}"));
    let _ = std::fs::remove_dir_all(&root);
    copy_tree(&valid_root(), &root);
    root
}

fn pkg(root: &Path) -> PathBuf {
    root.join("g9-session-director")
}

/// An independent statement of the documented pin algorithm (`docs/skills/registry.md`,
/// `scripts/skill_hashes.py`): sorted relative paths, `0x00`, length as 8 big-endian bytes, bytes.
fn documented_hash(dir: &Path) -> String {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                out.push(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/"),
                );
            }
        }
    }
    let mut files = Vec::new();
    walk(dir, dir, &mut files);
    files.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    let mut hasher = Sha256::new();
    for rel in files {
        let bytes = std::fs::read(dir.join(&rel)).unwrap();
        hasher.update(rel.as_bytes());
        hasher.update([0u8]);
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(&bytes);
    }
    hex::encode(hasher.finalize())
}

fn io_refusal(err: &PackageError) -> (&str, &str) {
    match err {
        PackageError::Io { path, reason } => (path, reason),
        other => panic!("expected an I/O refusal, got {other:?}"),
    }
}

#[test]
fn the_pin_algorithm_is_unchanged() {
    let dir = valid_pkg_dir();
    let expected = documented_hash(&dir);
    assert_eq!(package_sha256(&dir).unwrap(), expected);
    assert_eq!(load_package(&dir).unwrap().package_sha256, expected);
}

#[cfg(unix)]
fn mkfifo(path: &Path) {
    let status = std::process::Command::new("mkfifo")
        .arg(path)
        .status()
        .expect("mkfifo is available on a unix host");
    assert!(status.success(), "mkfifo {}", path.display());
}

#[cfg(unix)]
#[test]
fn a_fifo_inside_a_package_is_refused_not_skipped() {
    let root = scratch_root("fifo-extra");
    mkfifo(&pkg(&root).join("references").join("pipe"));
    let err = load_package(&pkg(&root)).unwrap_err();
    let (path, reason) = io_refusal(&err);
    assert_eq!(path, "references/pipe");
    assert!(reason.contains("regular file"), "{reason}");
    assert_eq!(err.rule(), "layout.io");
    // The hash refuses the same entry: it cannot vouch for something it does not cover.
    assert!(package_sha256(&pkg(&root)).is_err());
}

#[cfg(unix)]
#[test]
fn a_required_file_that_is_a_fifo_does_not_block_the_load() {
    let root = scratch_root("fifo-skill-md");
    let skill_md = pkg(&root).join("SKILL.md");
    std::fs::remove_file(&skill_md).unwrap();
    mkfifo(&skill_md);
    let dir = pkg(&root);
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(load_package(&dir));
    });
    // The deadline only trips when the load waits on the pipe for a writer; a run that passes
    // never waits for it.
    let result = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("load_package must not block on a FIFO");
    let err = result.unwrap_err();
    assert_eq!(io_refusal(&err).0, "SKILL.md");
}

#[cfg(unix)]
#[test]
fn a_symlinked_file_is_refused_and_its_target_is_never_read() {
    let root = scratch_root("symlink-file");
    let outside = root.join("outside.md");
    std::fs::write(&outside, "outside the package").unwrap();
    let link = pkg(&root).join("references").join("link.md");
    std::os::unix::fs::symlink(&outside, &link).unwrap();
    let err = load_package(&pkg(&root)).unwrap_err();
    let (path, reason) = io_refusal(&err);
    assert_eq!(path, "references/link.md");
    assert!(reason.contains("symlink"), "{reason}");
}

#[test]
fn a_file_larger_than_the_cap_refuses_the_package_and_one_at_the_cap_does_not() {
    let root = scratch_root("size-cap");
    let big = pkg(&root).join("references").join("big.md");
    std::fs::write(&big, vec![b'a'; MAX_FILE_BYTES as usize]).unwrap();
    assert!(
        load_package(&pkg(&root)).is_ok(),
        "a file of exactly the cap is accepted"
    );
    std::fs::write(&big, vec![b'a'; MAX_FILE_BYTES as usize + 1]).unwrap();
    let err = load_package(&pkg(&root)).unwrap_err();
    let (path, reason) = io_refusal(&err);
    assert_eq!(path, "references/big.md");
    assert!(reason.contains("larger than"), "{reason}");
}

#[test]
fn a_package_that_differs_from_its_pin_is_refused_before_it_is_parsed() {
    let root = scratch_root("pin-before-parse");
    let pins = Registry::load(&root).unwrap().hashes();
    // Change SKILL.md so that it no longer validates AND no longer matches the pin.
    let skill_md = pkg(&root).join("SKILL.md");
    let text = std::fs::read_to_string(&skill_md).unwrap();
    assert!(text.contains("## Mission"));
    std::fs::write(&skill_md, text.replace("## Mission", "## Purpose")).unwrap();
    assert!(
        matches!(Registry::load(&root), Err(RegistryError::Packages(_))),
        "without pins the change is a validation failure"
    );
    match Registry::load_pinned(&root, &pins).unwrap_err() {
        RegistryError::HashMismatch(mismatches) => {
            assert_eq!(mismatches.len(), 1);
            let m = &mismatches[0];
            assert_eq!(m.skill.as_str(), "g9-session-director");
            assert_eq!(
                m.expected.as_deref(),
                pins.values().next().map(String::as_str)
            );
            assert_eq!(
                m.actual.as_deref(),
                Some(package_sha256(&pkg(&root)).unwrap().as_str())
            );
        }
        other => panic!("a changed package is a pin mismatch, not a parse failure: {other:?}"),
    }
}

#[test]
fn an_unpinned_package_is_refused_before_it_is_parsed() {
    let root = scratch_root("unpinned");
    let skill_md = pkg(&root).join("SKILL.md");
    let text = std::fs::read_to_string(&skill_md).unwrap();
    std::fs::write(&skill_md, text.replace("## Mission", "## Purpose")).unwrap();
    let none = std::collections::BTreeMap::new();
    match Registry::load_pinned(&root, &none).unwrap_err() {
        RegistryError::HashMismatch(mismatches) => {
            assert_eq!(mismatches.len(), 1);
            assert_eq!(mismatches[0].expected, None);
            assert!(mismatches[0].actual.is_some());
        }
        other => panic!("an unpinned package is refused as a mismatch: {other:?}"),
    }
}

#[test]
fn a_pin_with_no_package_is_still_reported_and_an_exact_match_still_loads() {
    let root = scratch_root("pin-only");
    let mut pins = Registry::load(&root).unwrap().hashes();
    assert!(Registry::load_pinned(&root, &pins).is_ok());
    let ghost = g9_domain::SkillId::parse("g9-ghost").unwrap();
    pins.insert(ghost.clone(), "0".repeat(64));
    match Registry::load_pinned(&root, &pins).unwrap_err() {
        RegistryError::HashMismatch(mismatches) => {
            assert_eq!(mismatches.len(), 1);
            assert_eq!(mismatches[0].skill, ghost);
            assert_eq!(mismatches[0].actual, None);
        }
        other => panic!("{other:?}"),
    }
}

#[cfg(unix)]
#[test]
fn a_symlinked_package_directory_is_refused() {
    let root = scratch_root("symlink-dir");
    std::os::unix::fs::symlink(pkg(&root), root.join("g9-linked")).unwrap();
    match Registry::load(&root).unwrap_err() {
        RegistryError::Packages(failures) => {
            let linked = failures.iter().find(|f| f.dir == "g9-linked").unwrap();
            assert_eq!(linked.error.rule(), "layout.io");
            assert!(
                io_refusal(&linked.error).1.contains("symlink"),
                "{:?}",
                linked.error
            );
        }
        other => panic!("{other:?}"),
    }
}
