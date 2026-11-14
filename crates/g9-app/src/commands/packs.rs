//! The eight competency packs that ship with the application (`data/competency-packs/*/pack.json`,
//! status PROPOSED, version 0.1.0) and the command that loads them (`competency.import_pack`).
//!
//! The files are embedded at build time: the application reads no file at run time and the
//! request names a pack, it does not carry one. A pack competency becomes a `competencies` row
//! (name, description, difficulty band, validity window; `code` is its id) and each prerequisite
//! becomes a PREREQUISITE edge from the prerequisite to the competency that lists it.
//!
//! * Idempotent: a competency that exists is left as it is; running the import again creates nothing.
//! * Edges are completed on every run, over all packs: when a pack is loaded after one that depends
//!   on it, the missing edges are added then.
//! * The NICE references of the packs are `K-UNPINNED` / `S-UNPINNED` with confidence `ASSUMED`:
//!   they name nothing, so none is stored (a label that looks like a mapping but is not one would
//!   mislead). They are PROPOSED until the NICE version is pinned.
//! * The mapping of curriculum objectives to these competencies is in neither source and is not
//!   invented here (PROPOSED).

use super::{decode, new_id, parse_id, rc};
use crate::context::AppContext;
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation};
use g9_competency::Competency;
use g9_domain::{
    Classification, CommandMeta, CommandOutcome, CompetencyId, DataDomain, Difficulty, G9Error,
    ResourceKind, ResourceRef, Revision,
};
use g9_storage::repos::competency::{self as repo, CompetencyEdgeRow, CompetencyRow, EdgeKind};
use g9_storage::repos::ReadScope;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The pack files, embedded. Adding a pack directory means adding it here; the test
/// `every_pack_directory_is_embedded` fails until it is.
const PACK_FILES: [&str; 8] = [
    include_str!("../../../../data/competency-packs/agentic-mcp-supplychain/pack.json"),
    include_str!("../../../../data/competency-packs/detection-soc-hunting/pack.json"),
    include_str!("../../../../data/competency-packs/dfir-forensics-malware/pack.json"),
    include_str!("../../../../data/competency-packs/linux-python-systems/pack.json"),
    include_str!("../../../../data/competency-packs/llm-rag-security/pack.json"),
    include_str!("../../../../data/competency-packs/networking-protocols-recon/pack.json"),
    include_str!("../../../../data/competency-packs/web-api-appsec/pack.json"),
    include_str!("../../../../data/competency-packs/windows-identity-ad/pack.json"),
];

#[derive(Debug, Deserialize)]
struct Band {
    min: u8,
    max: u8,
}

#[derive(Debug, Deserialize)]
struct PackCompetency {
    id: String,
    name: String,
    description: String,
    difficulty_band: Band,
    prerequisites: Vec<String>,
    validity_window_days: u32,
}

#[derive(Debug, Deserialize)]
struct Pack {
    pack_id: String,
    title: String,
    version: String,
    status: String,
    competencies: Vec<PackCompetency>,
}

/// Data that ships with the application and does not parse is a build defect, not an input error.
fn corrupt<E>(_: E) -> G9Error {
    G9Error::internal_redacted("competency.pack_data")
}

fn packs() -> Result<Vec<Pack>, G9Error> {
    PACK_FILES
        .iter()
        .map(|text| serde_json::from_str::<Pack>(text).map_err(corrupt))
        .collect()
}

/// How many competencies the shipped packs define and their ids (what `curriculum.program`
/// counts the loaded ones against).
pub(crate) fn catalog() -> Result<BTreeSet<String>, G9Error> {
    Ok(packs()?
        .into_iter()
        .flat_map(|p| p.competencies.into_iter().map(|c| c.id))
        .collect())
}

/// The competency a prerequisite names: `id` inside the pack, `pack_id:id` across packs.
fn prerequisite_id(reference: &str) -> &str {
    reference.split_once(':').map_or(reference, |(_, id)| id)
}

#[derive(Debug, Deserialize)]
struct ImportRequest {
    pack_ids: Option<Vec<String>>,
}

/// `competency.import_pack`: load the named packs (all eight when none is named).
pub fn import_pack(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: ImportRequest = decode(payload)?;
    let all = packs()?;
    let wanted: Vec<String> = match req.pack_ids {
        None => all.iter().map(|p| p.pack_id.clone()).collect(),
        Some(ids) => {
            let mut seen = BTreeSet::new();
            for id in &ids {
                if !all.iter().any(|p| &p.pack_id == id) {
                    return Err(G9Error::not_found(
                        "competency.pack_unknown",
                        "no competency pack of that id ships with the application",
                    ));
                }
                seen.insert(id.clone());
            }
            seen.into_iter().collect()
        }
    };
    let resource = ResourceRef::new(ResourceKind::Competency, "packs");
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |_, _| {
            Ok(Facts::new_record(rc(
                DataDomain::Learning,
                Classification::Internal,
                false,
            )))
        },
        |env| {
            let scope = ReadScope::domain(DataDomain::Learning);
            let mut present: BTreeSet<String> = repo::list_competencies(env.conn, &scope)?
                .into_iter()
                .map(|c| c.id.as_str().to_string())
                .collect();
            let mut results = Vec::new();
            for pack in all.iter().filter(|p| wanted.contains(&p.pack_id)) {
                let (mut created, mut existing) = (0u32, 0u32);
                for c in &pack.competencies {
                    if present.contains(&c.id) {
                        existing += 1;
                        continue;
                    }
                    let id: CompetencyId = parse_id(&c.id, "competency_id").map_err(corrupt)?;
                    let band = |d: u8| Difficulty::new(d).map_err(corrupt);
                    let competency = Competency {
                        id,
                        name: c.name.trim().to_string(),
                        pack: pack.pack_id.clone(),
                        domain: DataDomain::Learning,
                        description: c.description.clone(),
                        validity_window_days: c.validity_window_days,
                        difficulty_band: (
                            band(c.difficulty_band.min)?,
                            band(c.difficulty_band.max)?,
                        ),
                        nice_refs: Vec::new(),
                    };
                    competency
                        .validate()
                        .map_err(|e| G9Error::invalid_input("competency.invalid", e.to_string()))?;
                    repo::insert_competency(
                        env.conn,
                        &CompetencyRow::from_domain(&competency, &c.id, None, env.now),
                    )?;
                    present.insert(c.id.clone());
                    created += 1;
                }
                results.push((pack, created, existing));
            }

            // Edges over every pack, so a later import completes what an earlier one could not.
            let mut have: BTreeSet<(String, String)> = repo::list_edges(env.conn)?
                .into_iter()
                .filter(|e| e.edge_kind == EdgeKind::Prerequisite)
                .map(|e| {
                    (
                        e.from_competency_id.as_str().to_string(),
                        e.to_competency_id.as_str().to_string(),
                    )
                })
                .collect();
            let mut added: BTreeMap<&str, u32> = BTreeMap::new();
            let mut pending = 0u32;
            for pack in &all {
                for c in &pack.competencies {
                    if !present.contains(&c.id) {
                        continue;
                    }
                    for reference in &c.prerequisites {
                        let from = prerequisite_id(reference);
                        if !present.contains(from) {
                            pending += 1;
                            continue;
                        }
                        if !have.insert((from.to_string(), c.id.clone())) {
                            continue;
                        }
                        repo::insert_edge(
                            env.conn,
                            &CompetencyEdgeRow {
                                id: new_id("edge"),
                                from_competency_id: parse_id(from, "competency_id")
                                    .map_err(corrupt)?,
                                to_competency_id: parse_id(&c.id, "competency_id")
                                    .map_err(corrupt)?,
                                edge_kind: EdgeKind::Prerequisite,
                                weight: 1.0,
                                created_at: env.now,
                            },
                        )?;
                        *added.entry(pack.pack_id.as_str()).or_default() += 1;
                    }
                }
            }

            let available: usize = all.iter().map(|p| p.competencies.len()).sum();
            let loaded = all
                .iter()
                .flat_map(|p| p.competencies.iter())
                .filter(|c| present.contains(&c.id))
                .count();
            let per_pack: Vec<Value> = results
                .iter()
                .map(|(pack, created, existing)| {
                    json!({
                        "pack_id": pack.pack_id,
                        "title": pack.title,
                        "version": pack.version,
                        "status": pack.status,
                        "competencies": pack.competencies.len(),
                        "created": created,
                        "existing": existing,
                        "prerequisites_added": added.get(pack.pack_id.as_str()).copied().unwrap_or(0),
                    })
                })
                .collect();
            let created_total: u32 = results.iter().map(|(_, created, _)| created).sum();
            Ok(Mutation::new(
                json!({
                    "packs": per_pack,
                    "loaded": loaded,
                    "available": available,
                    "prerequisites_added": added.values().sum::<u32>(),
                    "prerequisites_pending": pending,
                }),
                Some(Revision::INITIAL),
            )
            .with_event(EventSpec::learning(
                "competency.packs_imported",
                json!({ "packs": wanted, "created": created_total }),
            )))
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use g9_competency::CompetencyGraphSpec;

    fn spec_of(all: &[Pack]) -> CompetencyGraphSpec {
        let mut competencies = Vec::new();
        let mut edges = Vec::new();
        for pack in all {
            for c in &pack.competencies {
                competencies.push(Competency {
                    id: c.id.parse().unwrap(),
                    name: c.name.clone(),
                    pack: pack.pack_id.clone(),
                    domain: DataDomain::Learning,
                    description: c.description.clone(),
                    validity_window_days: c.validity_window_days,
                    difficulty_band: (
                        Difficulty::new(c.difficulty_band.min).unwrap(),
                        Difficulty::new(c.difficulty_band.max).unwrap(),
                    ),
                    nice_refs: Vec::new(),
                });
                for reference in &c.prerequisites {
                    edges.push((
                        prerequisite_id(reference).parse().unwrap(),
                        c.id.parse().unwrap(),
                    ));
                }
            }
        }
        CompetencyGraphSpec {
            competencies,
            edges,
        }
    }

    #[test]
    fn the_embedded_packs_are_eight_of_eight_and_unique() {
        let all = packs().unwrap();
        assert_eq!(all.len(), 8);
        assert!(all.iter().all(|p| p.competencies.len() == 8));
        let ids = catalog().unwrap();
        assert_eq!(ids.len(), 64, "a competency id appears in two packs");
        assert!(all
            .iter()
            .all(|p| p.status == "PROPOSED" && p.version == "0.1.0"));
    }

    #[test]
    fn every_pack_directory_is_embedded() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/competency-packs");
        let mut on_disk: Vec<String> = std::fs::read_dir(root)
            .unwrap()
            .map(|e| e.unwrap())
            .filter(|e| e.path().join("pack.json").is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        on_disk.sort();
        let mut embedded: Vec<String> = packs().unwrap().into_iter().map(|p| p.pack_id).collect();
        embedded.sort();
        assert_eq!(on_disk, embedded);
    }

    #[test]
    fn the_prerequisites_of_all_packs_resolve_and_form_an_acyclic_graph() {
        let all = packs().unwrap();
        let ids = catalog().unwrap();
        for pack in &all {
            for c in &pack.competencies {
                for reference in &c.prerequisites {
                    assert!(
                        ids.contains(prerequisite_id(reference)),
                        "{} names an unknown prerequisite {reference}",
                        c.id
                    );
                }
            }
        }
        // Rebuilding the graph rejects a cycle, a dangling edge and a duplicate node.
        let json = serde_json::to_value(spec_of(&all)).unwrap();
        let rebuilt: Result<CompetencyGraphSpec, _> = serde_json::from_value(json);
        assert!(rebuilt.is_ok(), "{:?}", rebuilt.err());
    }

    #[test]
    fn a_cross_pack_prerequisite_names_its_competency() {
        assert_eq!(prerequisite_id("a.b"), "a.b");
        assert_eq!(
            prerequisite_id("llm-rag-security:llm.app-mapping"),
            "llm.app-mapping"
        );
    }
}
