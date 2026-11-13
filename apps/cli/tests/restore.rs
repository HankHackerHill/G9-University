//! `g9 restore` and the scratch guard through the `g9` binary, each command in
//! its own process: a backup created by one process is verified and
//! restore-tested by the next, and restoring it replaces the live database
//! offline while the old one is quarantined, never deleted.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{assert_no_key_material, g9};
use g9_domain::{DataDomain, NoteId};
use g9_storage::repos::evidence as evidence_repo;
use g9_storage::repos::ReadScope;
use g9_storage::{backup, Database, OpenOptions};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

struct Env {
    _dir: tempfile::TempDir,
    data: PathBuf,
}

impl Env {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        Self { _dir: dir, data }
    }

    fn d(&self) -> &str {
        self.data.to_str().unwrap()
    }

    fn live(&self) -> PathBuf {
        self.data.join("g9.sqlite")
    }

    /// One `g9` process with `--data-dir` and `--no-skills` appended.
    fn run(&self, args: &[&str]) -> (i32, Value, String) {
        let mut all: Vec<&str> = args.to_vec();
        all.extend(["--data-dir", self.d(), "--no-skills"]);
        g9(&all)
    }

    fn call(&self, op: &str, payload: Value) -> (i32, Value, String) {
        let payload = payload.to_string();
        self.run(&["call", op, "--payload", &payload])
    }

    /// `init` (creates the data directory) and one RETAINED backup made by three
    /// separate processes. Returns the backup id.
    fn retained_backup(&self) -> String {
        self.retained_backup_with_marker().0
    }

    /// Like [`Env::retained_backup`], with a note written before the backup is
    /// taken: it is in the snapshot, so it must be in the database a restore leaves
    /// behind (a restore that quarantined the old database and booted a fresh one
    /// would lose it). Returns the backup id and that note's id.
    fn retained_backup_with_marker(&self) -> (String, String) {
        let (code, init, _) = self.run(&["init"]);
        assert_eq!(code, 0, "{init}");
        let marker = self.create_note_with("before the backup", "written before the backup");
        let (code, created, _) = self.call("backup.create", json!({ "label": "cli" }));
        assert_eq!(code, 0, "{created}");
        assert_eq!(created["result"]["state"], "SNAPSHOT_CREATED");
        let id = created["result"]["backup_id"].as_str().unwrap().to_string();
        let (code, verified, _) = self.call("backup.verify", json!({ "backup_id": id }));
        assert_eq!(code, 0, "a later process finds the manifest: {verified}");
        assert_eq!(verified["result"]["state"], "CHECKED");
        let (code, tested, _) = self.call("backup.restore_test", json!({ "backup_id": id }));
        assert_eq!(code, 0, "{tested}");
        assert_eq!(tested["result"]["state"], "RETAINED");
        (id, marker)
    }

    fn create_note(&self, title: &str) -> String {
        self.create_note_with(title, "written after the backup")
    }

    fn create_note_with(&self, title: &str, body: &str) -> String {
        let (code, note, _) = self.call(
            "notes.create",
            json!({ "title": title, "body_markdown": body, "classification": "PRIVATE", "domain": "LEARNING" }),
        );
        assert_eq!(code, 0, "{note}");
        note["result"]["note_id"].as_str().unwrap().to_string()
    }

    fn quarantine(&self) -> PathBuf {
        self.data.join("quarantine")
    }

    /// Names and sizes in the data directory (not recursive).
    fn listing(&self) -> Vec<(String, u64)> {
        let mut entries: Vec<(String, u64)> = std::fs::read_dir(&self.data)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                (
                    e.file_name().to_string_lossy().into_owned(),
                    e.metadata().unwrap().len(),
                )
            })
            .collect();
        entries.sort();
        entries
    }

    fn live_sha256(&self) -> String {
        backup::sha256_of_file(&self.live()).unwrap()
    }
}

/// Whether a closed database file holds the note (opened through `g9-storage`).
fn has_note(db_file: &Path, note_id: &str) -> bool {
    let db = Database::open(db_file, OpenOptions::new("cli-test").no_auto_migrate()).unwrap();
    let id = NoteId::parse(note_id).unwrap();
    evidence_repo::get_note(
        db.conn(),
        &ReadScope::cross_domain(DataDomain::Learning),
        &id,
    )
    .unwrap()
    .is_some()
}

/// The quarantined main database file (a copy is inspected, so the quarantine is not touched).
fn quarantined_main(out: &Value, scratch: &Path) -> PathBuf {
    let main = out["quarantined"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| PathBuf::from(p.as_str().unwrap()))
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("g9.sqlite.")
        })
        .unwrap();
    let copy = scratch.join("quarantined-copy.sqlite");
    std::fs::copy(&main, &copy).unwrap();
    copy
}

#[test]
fn restore_replaces_the_live_database_with_a_backup_made_by_earlier_processes() {
    let env = Env::new();
    let (id, marker) = env.retained_backup_with_marker();
    assert!(has_note(&env.live(), &marker));

    // `status` in yet another process names the retained backup.
    let (code, status, status_err) = env.run(&["status"]);
    assert_eq!(code, 0, "{status}");
    assert_eq!(status["result"]["last_backup"]["backup_id"], json!(id));
    assert_eq!(status["result"]["last_backup"]["state"], "RETAINED");
    assert!(
        !status.to_string().contains("last_backup_process_scoped"),
        "{status}"
    );

    let note = env.create_note("after the backup");
    assert!(has_note(&env.live(), &note));
    let live_before = env.live_sha256();
    let listing_before = env.listing();

    // Without confirmation: the plan, exit 1, nothing changed.
    let (code, plan, stderr) = env.run(&["restore", "--backup-id", &id]);
    assert_eq!(code, 1, "{plan}");
    assert!(stderr.contains("nothing was changed"), "{stderr}");
    assert_eq!(plan["restore"], "PLAN");
    assert_eq!(plan["confirmed"], false);
    assert_eq!(plan["changed"], false);
    assert_eq!(plan["backup_id"], json!(id));
    assert_eq!(plan["snapshot_matches_expected"], true);
    let expected_sha = plan["expected_sha256"].as_str().unwrap().to_string();
    assert_eq!(expected_sha.len(), 64);
    assert!(plan["snapshot"]
        .as_str()
        .unwrap()
        .ends_with(&format!("backups/{id}.sqlite")));
    assert!(plan["live"].as_str().unwrap().ends_with("g9.sqlite"));
    assert!(plan["quarantine_dir"]
        .as_str()
        .unwrap()
        .ends_with("quarantine"));
    assert_eq!(env.live_sha256(), live_before);
    assert!(has_note(&env.live(), &note));
    assert!(!env.quarantine().exists());
    assert!(!env.data.join("restore-log.jsonl").exists());
    // Planning is read-only: no file appears next to the live database, not even
    // the `-wal` and `-shm` that a plain read-only open of a WAL database creates.
    assert_eq!(env.listing(), listing_before);

    // A wrong checksum is refused before anything is touched (both sources).
    let wrong = "0".repeat(64);
    let (code, err, _) = env.run(&[
        "restore",
        "--backup-id",
        &id,
        "--expect-sha256",
        &wrong,
        "--confirm-replace-live-database",
    ]);
    assert_eq!(
        (code, err["code"].as_str()),
        (2, Some("restore.checksum_mismatch")),
        "{err}"
    );
    let snapshot = plan["snapshot"].as_str().unwrap().to_string();
    let (code, err, _) = env.run(&[
        "restore",
        "--snapshot",
        &snapshot,
        "--expect-sha256",
        &wrong,
        "--confirm-replace-live-database",
    ]);
    assert_eq!(
        (code, err["code"].as_str()),
        (2, Some("restore.checksum_mismatch")),
        "{err}"
    );
    assert_eq!(env.live_sha256(), live_before);
    assert!(!env.quarantine().exists());

    // Confirmed.
    let (code, restored, restore_err) = env.run(&[
        "restore",
        "--backup-id",
        &id,
        "--confirm-replace-live-database",
    ]);
    assert_eq!(code, 0, "{restored} {restore_err}");
    assert_eq!(restored["restored"], true);
    assert_eq!(restored["snapshot_sha256"], json!(expected_sha));
    assert_eq!(restored["restore_log_written"], true);
    assert_eq!(restored["boot"]["recovery"]["audit_chain_ok"], true);
    assert_eq!(restored["boot"]["status"]["storage"]["state"], "READY");
    assert!(restored["audit_rows_verified"].as_u64().is_some());
    // The note written after the backup is gone; the quarantine still holds it.
    assert!(!has_note(&env.live(), &note));
    // The note written before the backup is there: the live database is the
    // snapshot, not a fresh database (a no-op "quarantine and boot" would lose it).
    assert!(has_note(&env.live(), &marker));
    let scratch = tempfile::tempdir().unwrap();
    assert!(has_note(
        &quarantined_main(&restored, scratch.path()),
        &note
    ));
    // The restored database is the one the snapshot was taken from: that database
    // did not yet record this backup's manifest, so no "last backup" is reported.
    let (code, status, _) = env.run(&["status"]);
    assert_eq!(code, 0, "{status}");
    assert_eq!(status["result"]["storage"]["state"], "READY");
    assert_eq!(status["result"]["last_backup"], Value::Null);

    // One metadata-only log line.
    let log = std::fs::read_to_string(env.data.join("restore-log.jsonl")).unwrap();
    let lines: Vec<&str> = log.lines().collect();
    assert_eq!(lines.len(), 1, "{log}");
    let line: Value = serde_json::from_str(lines[0]).unwrap();
    let mut keys: Vec<&str> = line
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["quarantined", "sha256", "snapshot", "timestamp"]);
    assert_eq!(line["sha256"], json!(expected_sha));
    assert!(!line["quarantined"].as_array().unwrap().is_empty());
    assert!(!log.contains("written after the backup"));

    // No output carries the binding key.
    let key_file = env.data.join("binding.key");
    assert_no_key_material(
        &key_file,
        &[
            &plan.to_string(),
            &restored.to_string(),
            &restore_err,
            &status_err,
            &log,
        ],
    );
}

#[test]
fn restore_refuses_what_it_must_and_changes_nothing() {
    let env = Env::new();
    let (code, _, _) = env.run(&["init"]);
    assert_eq!(code, 0);
    // A snapshot without a checksum and a restore without a source are usage errors.
    let (code, _, stderr) = env.run(&["restore", "--snapshot", "x.sqlite"]);
    assert_eq!(code, 1);
    assert!(stderr.contains("expect-sha256"), "{stderr}");
    let (code, _, _) = env.run(&["restore"]);
    assert_eq!(code, 1);
    let (code, _, _) = env.run(&["restore", "--backup-id", "not a valid id!"]);
    assert_eq!(code, 1);

    // Unknown id: NOT_FOUND. A backup that is not RETAINED: PRECONDITION_FAILED.
    let (code, err, _) = env.run(&["restore", "--backup-id", "bk-unknown"]);
    assert_eq!(
        (code, err["code"].as_str()),
        (2, Some("restore.backup_not_found")),
        "{err}"
    );
    let (code, created, _) = env.call("backup.create", json!({}));
    assert_eq!(code, 0);
    let id = created["result"]["backup_id"].as_str().unwrap().to_string();
    let before = env.live_sha256();
    let (code, err, _) = env.run(&[
        "restore",
        "--backup-id",
        &id,
        "--confirm-replace-live-database",
    ]);
    assert_eq!(
        (code, err["code"].as_str()),
        (2, Some("restore.backup_not_retained")),
        "{err}"
    );
    assert_eq!(err["category"], "PRECONDITION_FAILED");
    assert_eq!(env.live_sha256(), before);
    assert!(!env.quarantine().exists());

    // A corrupt snapshot with the right checksum is still refused by verification.
    let junk = env.data.join("backups").join("junk.sqlite");
    std::fs::write(&junk, vec![0x5a_u8; 4096]).unwrap();
    let junk_sha = backup::sha256_of_file(&junk).unwrap();
    let (code, err, _) = env.run(&[
        "restore",
        "--snapshot",
        junk.to_str().unwrap(),
        "--expect-sha256",
        &junk_sha,
        "--confirm-replace-live-database",
    ]);
    assert_eq!(code, 2, "{err}");
    assert_eq!(err["category"], "DATA_CORRUPTION");
    assert_eq!(env.live_sha256(), before);
    assert!(!env.quarantine().exists());
    let leftovers: Vec<String> = std::fs::read_dir(&env.data)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("restore-tmp"))
        .collect();
    assert_eq!(leftovers, Vec::<String>::new());
}

#[test]
fn restore_is_refused_while_another_process_holds_the_database() {
    let env = Env::new();
    let id = env.retained_backup();
    let note = env.create_note("kept");
    // This process plays the running application: an idle open connection.
    let app = Database::open(env.live(), OpenOptions::new("cli-test").no_auto_migrate()).unwrap();
    let (code, err, _) = env.run(&[
        "restore",
        "--backup-id",
        &id,
        "--confirm-replace-live-database",
    ]);
    assert_eq!(
        (code, err["code"].as_str()),
        (2, Some("restore.database_in_use")),
        "{err}"
    );
    assert_eq!(err["category"], "PRECONDITION_FAILED");
    assert!(!env.quarantine().exists());
    drop(app);
    assert!(has_note(&env.live(), &note));
    // Once the application has stopped the same command succeeds.
    let (code, out, _) = env.run(&[
        "restore",
        "--backup-id",
        &id,
        "--confirm-replace-live-database",
    ]);
    assert_eq!(code, 0, "{out}");
    assert!(!has_note(&env.live(), &note));
}

#[test]
fn a_snapshot_restores_into_a_fresh_data_directory_without_quarantine() {
    let source = Env::new();
    let id = source.retained_backup();
    let (_, plan, _) = source.run(&["restore", "--backup-id", &id]);
    let snapshot = plan["snapshot"].as_str().unwrap().to_string();
    let sha = plan["expected_sha256"].as_str().unwrap().to_string();

    let fresh = Env::new();
    assert!(!fresh.data.exists());
    // The plan alone creates nothing.
    let (code, plan, _) = fresh.run(&["restore", "--snapshot", &snapshot, "--expect-sha256", &sha]);
    assert_eq!(code, 1, "{plan}");
    assert!(!fresh.data.exists());
    assert_eq!(plan["live_exists"], false);

    let (code, out, err) = fresh.run(&[
        "restore",
        "--snapshot",
        &snapshot,
        "--expect-sha256",
        &sha,
        "--confirm-replace-live-database",
    ]);
    assert_eq!(code, 0, "{out} {err}");
    assert_eq!(out["quarantined"], json!([]));
    assert!(!fresh.quarantine().exists());
    assert_eq!(out["boot"]["status"]["storage"]["state"], "READY");
    assert!(fresh.data.join("restore-log.jsonl").is_file());
}

#[test]
fn the_seed_writing_commands_refuse_to_run_without_scratch() {
    let env = Env::new();
    for args in [
        vec!["crash-drill"],
        vec!["simulate-week", "--start-date", "2026-11-16"],
    ] {
        let (code, out, stderr) = env.run(&args);
        assert_eq!(code, 1, "{args:?}: {out}");
        assert!(stderr.contains("--scratch"), "{stderr}");
        assert!(stderr.contains("seed rows directly to storage"), "{stderr}");
        assert!(stderr.contains("throwaway data directory"), "{stderr}");
        assert!(!env.data.exists(), "{args:?} created the data directory");
    }
    // With the flag the drill runs on the disposable directory.
    let (code, drill, _) = env.run(&["crash-drill", "--scratch"]);
    assert_eq!(code, 0, "{drill}");
    assert_eq!(drill["state"], "RUNNING");
}

#[cfg(unix)]
#[test]
fn a_path_that_is_not_utf8_is_refused_before_anything_changes() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::process::Command;

    let env = Env::new();
    let (id, marker) = env.retained_backup_with_marker();
    let (_, plan, _) = env.run(&["restore", "--backup-id", &id]);
    let sha = plan["expected_sha256"].as_str().unwrap().to_string();
    let snapshot = plan["snapshot"].as_str().unwrap().to_string();
    // The same snapshot under a name that is not UTF-8.
    let odd = env.data.join(OsStr::from_bytes(b"copy-\xff-of.sqlite"));
    std::fs::copy(&snapshot, &odd).unwrap();
    let before = env.live_sha256();

    for confirm in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_g9"));
        command
            .args(["restore", "--snapshot"])
            .arg(&odd)
            .args(["--expect-sha256", &sha, "--no-skills", "--data-dir"])
            .arg(&env.data);
        if confirm {
            command.arg("--confirm-replace-live-database");
        }
        let out = command.output().unwrap();
        let stderr = String::from_utf8_lossy(&out.stderr);
        // A usage error, not a panic (exit 101) and not a restore.
        assert_eq!(out.status.code(), Some(1), "confirm={confirm}: {stderr}");
        assert!(stderr.contains("UTF-8"), "{stderr}");
        assert!(!stderr.contains("panicked"), "{stderr}");
    }
    // The same for a data directory that is not UTF-8: nothing is created.
    let odd_dir = env.data.with_file_name("data-\u{fffd}");
    let odd_dir = std::path::PathBuf::from(OsStr::from_bytes(
        &[odd_dir.as_os_str().as_bytes(), b"\xfe"].concat(),
    ));
    let out = Command::new(env!("CARGO_BIN_EXE_g9"))
        .args(["restore", "--snapshot"])
        .arg(&snapshot)
        .args(["--expect-sha256", &sha, "--no-skills", "--data-dir"])
        .arg(&odd_dir)
        .arg("--confirm-replace-live-database")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(!odd_dir.exists());

    assert_eq!(env.live_sha256(), before);
    assert!(!env.quarantine().exists());
    assert!(!env.data.join("restore-log.jsonl").exists());
    assert!(has_note(&env.live(), &marker));
}

#[test]
fn a_backup_copied_to_another_drive_restores_from_its_own_manifest() {
    // Keep a copy off this disk (a USB drive, a NAS): `g9 copy-backup` writes the
    // snapshot and a manifest with its checksum, synced and re-hashed; a later
    // `g9 restore --snapshot <copy> --expect-sha256 <manifest sha256>` needs
    // nothing from this disk.
    let env = Env::new();
    let (id, marker) = env.retained_backup_with_marker();
    let usb = tempfile::tempdir().unwrap();
    let to = usb.path().to_str().unwrap();

    let (code, out, _) = env.run(&["copy-backup", "--backup-id", &id, "--to", to]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out["verified"], true);
    let copy = PathBuf::from(out["copied_to"].as_str().unwrap());
    assert!(copy.starts_with(std::fs::canonicalize(usb.path()).unwrap()));
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(out["manifest"].as_str().unwrap()).unwrap())
            .unwrap();
    assert_eq!(manifest["sha256"], out["sha256"]);
    assert_eq!(manifest["backup_id"], id.as_str());

    let (code, again, _) = env.run(&["copy-backup", "--backup-id", &id, "--to", to]);
    assert_ne!(code, 0, "a copy never overwrites: {again}");
    assert_eq!(again["code"], "copy_backup.destination_exists");

    let note_after = env.create_note("after the backup");
    let sha = manifest["sha256"].as_str().unwrap();
    let (code, restored, _) = env.run(&[
        "restore",
        "--snapshot",
        copy.to_str().unwrap(),
        "--expect-sha256",
        sha,
        "--confirm-replace-live-database",
    ]);
    assert_eq!(code, 0, "{restored}");
    assert!(
        has_note(&env.live(), &marker),
        "the snapshot's content is back"
    );
    assert!(!has_note(&env.live(), &note_after), "later content is gone");
}

#[test]
fn only_a_retained_backup_is_copied() {
    let env = Env::new();
    let (code, init, _) = env.run(&["init"]);
    assert_eq!(code, 0, "{init}");
    let (code, created, _) = env.call("backup.create", json!({ "label": "unverified" }));
    assert_eq!(code, 0, "{created}");
    let id = created["result"]["backup_id"].as_str().unwrap().to_string();
    let usb = tempfile::tempdir().unwrap();
    let (code, refused, _) = env.run(&[
        "copy-backup",
        "--backup-id",
        &id,
        "--to",
        usb.path().to_str().unwrap(),
    ]);
    assert_ne!(code, 0, "{refused}");
    assert_eq!(refused["code"], "restore.backup_not_retained");
    assert_eq!(
        std::fs::read_dir(usb.path()).unwrap().count(),
        0,
        "nothing written"
    );
}

#[test]
fn attachment_files_travel_with_a_copied_backup_and_come_back_on_restore() {
    // An attached file lives outside the database: the backup copies it beside
    // the snapshots, copy-backup carries it to the other drive, and a restore
    // onto a fresh machine puts it back into the attachment store.
    let env = Env::new();
    let (code, init, _) = env.run(&["init"]);
    assert_eq!(code, 0, "{init}");
    let (code, attached, _) = env.call(
        "evidence.attach_file",
        json!({ "filename": "notes.txt", "media_type": "text/plain", "content_base64": "bGFiIGV2aWRlbmNlCg==" }),
    );
    assert_eq!(code, 0, "{attached}");
    let sha = attached["result"]["sha256"].as_str().unwrap().to_string();
    let rel = format!("{}/{}/{}", &sha[0..2], &sha[2..4], sha);
    assert_eq!(
        std::fs::read(env.data.join("attachments").join(&rel)).unwrap(),
        b"lab evidence\n"
    );

    let (code, created, _) = env.call("backup.create", json!({}));
    assert_eq!(code, 0, "{created}");
    let id = created["result"]["backup_id"].as_str().unwrap().to_string();
    assert!(env.data.join("backups/attachments").join(&rel).is_file());
    for op in ["backup.verify", "backup.restore_test"] {
        let (code, out, _) = env.call(op, json!({ "backup_id": id }));
        assert_eq!(code, 0, "{out}");
    }

    let usb = tempfile::tempdir().unwrap();
    let (code, copied, _) = env.run(&[
        "copy-backup",
        "--backup-id",
        &id,
        "--to",
        usb.path().to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{copied}");
    assert_eq!(copied["attachments"]["copied"], 1, "{copied}");
    assert!(usb.path().join("attachments").join(&rel).is_file());
    let snapshot = copied["copied_to"].as_str().unwrap().to_string();
    let snapshot_sha = copied["sha256"].as_str().unwrap().to_string();

    // This disk is gone: restore onto a fresh data directory from the copy.
    let fresh = Env::new();
    let (code, restored, _) = fresh.run(&[
        "restore",
        "--snapshot",
        &snapshot,
        "--expect-sha256",
        &snapshot_sha,
        "--confirm-replace-live-database",
    ]);
    assert_eq!(code, 0, "{restored}");
    assert_eq!(restored["attachments"]["restored"], 1, "{restored}");
    assert_eq!(
        std::fs::read(fresh.data.join("attachments").join(&rel)).unwrap(),
        b"lab evidence\n"
    );

    // Without the attachment folder the database still restores, and the
    // missing file is reported.
    std::fs::remove_dir_all(usb.path().join("attachments")).unwrap();
    let bare = Env::new();
    let (code, partial, _) = bare.run(&[
        "restore",
        "--snapshot",
        &snapshot,
        "--expect-sha256",
        &snapshot_sha,
        "--confirm-replace-live-database",
    ]);
    assert_eq!(code, 0, "{partial}");
    assert_eq!(
        partial["attachments"]["missing"].as_array().unwrap().len(),
        1
    );
    assert!(partial["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w.as_str().unwrap().contains("restore.attachments_missing")));
}
