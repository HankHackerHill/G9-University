#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! app-11 (2026-11-10 review): a `binding.key` missing next to an existing database was replaced
//! by a new key without a word, so approvals granted under the old key failed later with no
//! explanation. Opening the backend now warns `binding_key.file_recreated` when it had to create
//! the key while the database already existed. The CLI opens the backend exactly as the desktop
//! bridge does (`bootstrap::open`).

use g9_app::keyfile::KEY_FILE_NAME;
use g9_cli::bootstrap::{open, BootOptions, DATABASE_FILE};

const RECREATED: &str = "WARNING binding_key.file_recreated:";

fn options(data: &std::path::Path) -> BootOptions {
    let mut opts = BootOptions::new(data.to_path_buf());
    opts.skills_dir = None;
    opts
}

fn recreated(warnings: &[String]) -> Vec<&String> {
    warnings
        .iter()
        .filter(|w| w.starts_with(RECREATED))
        .collect()
}

#[test]
fn a_key_lost_next_to_an_existing_database_is_reported_when_it_is_recreated() {
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("data");

    // First run: no database yet, a key is created silently (only the standing fallback warning).
    let first = open(&options(&data)).unwrap();
    assert!(
        recreated(&first.warnings).is_empty(),
        "{:?}",
        first.warnings
    );
    assert!(data.join(DATABASE_FILE).exists());
    drop(first);

    // A normal restart reuses the key.
    let second = open(&options(&data)).unwrap();
    assert!(
        recreated(&second.warnings).is_empty(),
        "{:?}",
        second.warnings
    );
    drop(second);

    // The key is lost (deleted, or not restored with the database).
    std::fs::remove_file(data.join(KEY_FILE_NAME)).unwrap();
    let third = open(&options(&data)).unwrap();
    let found = recreated(&third.warnings);
    assert_eq!(found.len(), 1, "{:?}", third.warnings);
    let dir_text = dir.path().to_string_lossy().to_string();
    assert!(!found[0].contains(&dir_text), "no host path: {}", found[0]);
    assert!(data.join(KEY_FILE_NAME).exists());
    drop(third);

    // The recreated key is the key from now on: the next start is quiet again.
    let fourth = open(&options(&data)).unwrap();
    assert!(
        recreated(&fourth.warnings).is_empty(),
        "{:?}",
        fourth.warnings
    );
}
