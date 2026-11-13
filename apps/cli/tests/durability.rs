#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Owner decision 2026-11-13 (TEC-12): the application's database runs with
//! `synchronous = FULL`, so a power cut loses no committed command. The CLI
//! opens the backend exactly as the desktop bridge does (`bootstrap::open`),
//! and both take their storage options from `g9_app::database_options`.

use g9_cli::bootstrap::{open, BootOptions};

#[test]
fn the_backend_opens_its_database_with_synchronous_full() {
    let dir = tempfile::tempdir().unwrap();
    let mut opts = BootOptions::new(dir.path().join("data"));
    opts.skills_dir = None;
    let booted = open(&opts).unwrap();
    assert_eq!(
        booted.ctx.database().profile().synchronous,
        2,
        "PRAGMA synchronous must read back FULL (2)"
    );
}
