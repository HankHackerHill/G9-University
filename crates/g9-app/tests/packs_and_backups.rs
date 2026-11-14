#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Competency packs and the backup list (owner request 2026-11-14): the eight packs that ship
//! with the application load in one step, and the backups recorded in the database are listed
//! for the window to show.

mod common;

use common::{seed_manifest, Harness, Opts};
use g9_domain::ErrorCategory;
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// (pack id, competency ids, prerequisite references) read from the files on disk, independently
/// of the copy the application embeds.
type PackOnDisk = (String, Vec<String>, Vec<(String, String)>);

fn packs_on_disk() -> Vec<PackOnDisk> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/competency-packs");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path().join("pack.json");
        if !path.is_file() {
            continue;
        }
        let doc: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut ids = Vec::new();
        let mut edges = Vec::new();
        for c in doc["competencies"].as_array().unwrap() {
            let id = c["id"].as_str().unwrap().to_string();
            for pre in c["prerequisites"].as_array().unwrap() {
                let pre = pre.as_str().unwrap();
                let from = pre.split_once(':').map_or(pre, |(_, id)| id);
                edges.push((from.to_string(), id.clone()));
            }
            ids.push(id);
        }
        out.push((doc["pack_id"].as_str().unwrap().to_string(), ids, edges));
    }
    out.sort();
    out
}

fn count(h: &Harness, sql: &str) -> i64 {
    h.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn loading_all_packs_stores_the_64_competencies_and_every_prerequisite_edge() {
    let mut h = Harness::new();
    let done = h.call("competency.import_pack", json!({})).unwrap();
    let r = &done["result"];
    assert_eq!(r["loaded"], 64);
    assert_eq!(r["available"], 64);
    assert_eq!(r["prerequisites_pending"], 0);
    let packs = r["packs"].as_array().unwrap();
    assert_eq!(packs.len(), 8);
    for p in packs {
        assert_eq!(p["competencies"], 8);
        assert_eq!(p["created"], 8);
        assert_eq!(p["existing"], 0);
        assert_eq!(p["status"], "PROPOSED");
    }

    let disk = packs_on_disk();
    let expected_edges: BTreeSet<(String, String)> = disk
        .iter()
        .flat_map(|(_, _, e)| e.iter().cloned())
        .collect();
    assert_eq!(r["prerequisites_added"], expected_edges.len());
    assert_eq!(count(&h, "SELECT COUNT(*) FROM competencies"), 64);
    let stored: BTreeSet<(String, String)> = {
        let mut stmt = h
            .conn()
            .prepare(
                "SELECT from_competency_id, to_competency_id FROM competency_edges
                  WHERE edge_kind = 'PREREQUISITE'",
            )
            .unwrap();
        stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert_eq!(
        stored, expected_edges,
        "edges run from the prerequisite to the competency"
    );

    // Every id of every pack on disk is stored under its pack, with its id as its code.
    for (pack, ids, _) in &disk {
        for id in ids {
            let (code, area): (String, String) = h
                .conn()
                .query_row(
                    "SELECT code, domain_area FROM competencies WHERE id = ?1",
                    [id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
            assert_eq!(&code, id);
            assert_eq!(&area, pack);
        }
    }

    // The competency graph of the window lists them, with no state invented.
    let projection = h.call("competency.projection", json!({})).unwrap();
    let rows = projection["result"]["projections"].as_array().unwrap();
    assert_eq!(rows.len(), 64);
    assert!(rows.iter().all(|row| row["state"] == "UNASSESSED"));

    let program = h.call("curriculum.program", json!({})).unwrap();
    assert_eq!(
        program["result"]["competencies"],
        json!({ "loaded": 64, "available": 64 })
    );
}

#[test]
fn loading_twice_stores_nothing_new() {
    let mut h = Harness::new();
    h.call("competency.import_pack", json!({})).unwrap();
    let again = h.call("competency.import_pack", json!({})).unwrap();
    let r = &again["result"];
    assert_eq!(r["prerequisites_added"], 0);
    for p in r["packs"].as_array().unwrap() {
        assert_eq!(p["created"], 0);
        assert_eq!(p["existing"], 8);
        assert_eq!(p["prerequisites_added"], 0);
    }
    assert_eq!(count(&h, "SELECT COUNT(*) FROM competencies"), 64);
}

#[test]
fn a_pack_loaded_later_completes_the_edges_of_the_packs_loaded_before() {
    let mut h = Harness::new();
    // agent.workflow-authority (agentic-mcp-supplychain) has llm-rag-security:llm.app-mapping as a prerequisite.
    let first = h
        .call(
            "competency.import_pack",
            json!({ "pack_ids": ["agentic-mcp-supplychain"] }),
        )
        .unwrap();
    assert_eq!(first["result"]["loaded"], 8);
    assert!(first["result"]["prerequisites_pending"].as_u64().unwrap() >= 1);
    let before = count(&h, "SELECT COUNT(*) FROM competency_edges");
    let missing: i64 = count(
        &h,
        "SELECT COUNT(*) FROM competency_edges
          WHERE from_competency_id = 'llm.app-mapping' AND to_competency_id = 'agent.workflow-authority'",
    );
    assert_eq!(missing, 0);

    let second = h
        .call(
            "competency.import_pack",
            json!({ "pack_ids": ["llm-rag-security"] }),
        )
        .unwrap();
    assert_eq!(second["result"]["loaded"], 16);
    assert!(second["result"]["prerequisites_added"].as_u64().unwrap() >= 1);
    let after = count(&h, "SELECT COUNT(*) FROM competency_edges");
    assert!(after > before);
    let completed: i64 = count(
        &h,
        "SELECT COUNT(*) FROM competency_edges
          WHERE from_competency_id = 'llm.app-mapping' AND to_competency_id = 'agent.workflow-authority'",
    );
    assert_eq!(
        completed, 1,
        "the edge of the earlier pack is added once the pack it needs is loaded"
    );
}

#[test]
fn an_unknown_or_malformed_pack_id_is_refused_and_nothing_is_stored() {
    let mut h = Harness::new();
    let unknown = h
        .call(
            "competency.import_pack",
            json!({ "pack_ids": ["no-such-pack"] }),
        )
        .unwrap_err();
    assert_eq!(unknown.category, ErrorCategory::NotFound);
    assert_eq!(unknown.code, "competency.pack_unknown");
    for bad in [
        json!(["../web-api-appsec"]),
        json!([]),
        json!(["web-api-appsec", "web-api-appsec"]),
        json!(["Web"]),
    ] {
        let e = h
            .call("competency.import_pack", json!({ "pack_ids": bad }))
            .unwrap_err();
        assert_eq!(e.category, ErrorCategory::InvalidInput, "{bad}");
    }
    assert_eq!(count(&h, "SELECT COUNT(*) FROM competencies"), 0);
    assert_eq!(count(&h, "SELECT COUNT(*) FROM competency_edges"), 0);
}

#[test]
fn an_operator_competency_is_not_counted_as_a_pack_competency() {
    let mut h = Harness::new();
    h.call(
        "competency.create",
        json!({ "code": "MINE-1", "title": "My own competency" }),
    )
    .unwrap();
    let program = h.call("curriculum.program", json!({})).unwrap();
    assert_eq!(
        program["result"]["competencies"],
        json!({ "loaded": 0, "available": 64 })
    );
    h.call(
        "competency.import_pack",
        json!({ "pack_ids": ["web-api-appsec"] }),
    )
    .unwrap();
    let program = h.call("curriculum.program", json!({})).unwrap();
    assert_eq!(
        program["result"]["competencies"],
        json!({ "loaded": 8, "available": 64 })
    );
    assert_eq!(count(&h, "SELECT COUNT(*) FROM competencies"), 9);
}

#[test]
fn loading_packs_leaves_one_event_and_an_intact_audit_chain() {
    let mut h = Harness::new();
    h.call("competency.import_pack", json!({})).unwrap();
    assert_eq!(
        count(
            &h,
            "SELECT COUNT(*) FROM domain_events WHERE kind = 'competency.packs_imported'"
        ),
        1
    );
    let audit = h.call("audit.recent", json!({})).unwrap();
    assert_eq!(audit["result"]["chain_ok"], true);
    // The packs are PROPOSED content: loading them must not create any assessment evidence.
    assert_eq!(count(&h, "SELECT COUNT(*) FROM competency_observations"), 0);
}

#[test]
fn the_curriculum_import_and_the_packs_are_independent_of_each_other() {
    let mut h = Harness::new();
    h.approved(
        "curriculum.import",
        json!({ "manifest": seed_manifest(), "provenance": "PROPOSED_SEED" }),
        &Opts::default(),
    );
    let program = h.call("curriculum.program", json!({})).unwrap();
    assert_eq!(program["result"]["competencies"]["loaded"], 0);
    assert_eq!(program["result"]["curriculum"]["objectives"], 216);
}

// ---------------------------------------------------------------------------
// backup.list
// ---------------------------------------------------------------------------

#[test]
fn the_backup_list_is_empty_before_any_backup_and_shows_them_newest_first_after() {
    let mut h = Harness::with_dirs();
    let empty = h.call("backup.list", json!({})).unwrap();
    assert_eq!(empty["result"]["backups"], json!([]));
    assert_eq!(empty["result"]["unlisted"], 0);

    let first = h
        .call("backup.create", json!({ "label": "before the import" }))
        .unwrap();
    h.advance(60_000);
    let second = h.call("backup.create", json!({})).unwrap();
    let listed = h.call("backup.list", json!({})).unwrap();
    let backups = listed["result"]["backups"].as_array().unwrap();
    assert_eq!(backups.len(), 2);
    assert_eq!(backups[0]["backup_id"], second["result"]["backup_id"]);
    assert_eq!(backups[1]["backup_id"], first["result"]["backup_id"]);
    assert_eq!(backups[1]["label"], "before the import");
    assert!(backups[0]["size_bytes"].as_u64().unwrap() > 0);
    assert_eq!(backups[0]["state"], "SNAPSHOT_CREATED");
    assert_eq!(listed["result"]["unlisted"], 0);

    // A backup that was verified shows its new state in the list.
    let id = first["result"]["backup_id"].as_str().unwrap();
    h.call("backup.verify", json!({ "backup_id": id })).unwrap();
    let listed = h.call("backup.list", json!({})).unwrap();
    let verified = listed["result"]["backups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["backup_id"] == id)
        .unwrap();
    assert_eq!(verified["state"], "CHECKED");
    assert_eq!(verified["integrity"], "OK");
}

#[test]
fn a_recorded_path_outside_the_approved_roots_is_not_followed_and_is_counted() {
    let mut h = Harness::with_dirs();
    let made = h.call("backup.create", json!({})).unwrap();
    let id = made["result"]["backup_id"].as_str().unwrap().to_string();
    h.conn()
        .execute(
            "UPDATE backup_manifests SET path = '/etc/passwd' WHERE id = ?1",
            [&id],
        )
        .unwrap();
    let listed = h.call("backup.list", json!({})).unwrap();
    assert_eq!(listed["result"]["backups"], json!([]));
    assert_eq!(listed["result"]["unlisted"], 1);
    assert!(listed["warnings"].as_array().unwrap().iter().any(|w| w
        .as_str()
        .unwrap()
        .starts_with("backup.manifest_path_outside_approved_roots")));
}
