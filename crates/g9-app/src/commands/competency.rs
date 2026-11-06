//! `competency.projection` (read-only; M06; ARCHITECTURE.md §8 "Derivation").
//!
//! State is derived on every read by `g9_competency::derive_state` (a pure,
//! replayable fold over retained observations with the default
//! `DerivationRules`); nothing is cached as authority and no number is
//! invented: a dimension without observations reports `null` estimates.
//! `provisional` is `false` because the reliability-class downgrade (ADR-0005
//! addendum, LRN-11) is not modelled by the wave-1 derivation (PROPOSED).

use super::{decode, parse_id, rc, scope_of, tail};
use crate::context::AppContext;
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, Facts, Mutation};
use g9_competency::{derive_state, DerivationRules};
use g9_domain::{
    Classification, CommandMeta, CommandOutcome, CompetencyId, G9Error, LearningDimension,
    ResourceKind, ResourceRef,
};
use g9_storage::repos::competency as repo;
use serde::Deserialize;
use serde_json::{json, Map, Value};

#[derive(Debug, Deserialize)]
struct ProjectionRequest {
    competency_id: Option<String>,
}

/// `competency.projection`.
pub fn projection(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: ProjectionRequest = decode(payload)?;
    let only: Option<CompetencyId> = req
        .competency_id
        .as_deref()
        .map(|c| parse_id(c, "competency_id"))
        .transpose()?;
    let resource = ResourceRef::new(
        ResourceKind::Competency,
        only.as_ref().map_or("competencies", |c| c.as_str()),
    );
    let scope = scope_of(meta);
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
                meta.scope.domain,
                Classification::Internal,
                false,
            )))
        },
        |env| {
            let rules = DerivationRules::default();
            let rows = match &only {
                Some(id) => vec![repo::get_competency(env.conn, &scope, id)?.ok_or_else(|| {
                    G9Error::not_found("competency.not_found", "competency not found")
                })?],
                None => repo::list_competencies(env.conn, &scope)?,
            };
            let mut projections = Vec::new();
            for row in rows {
                let competency = row.to_domain()?;
                let observations = repo::list_observations(env.conn, &row.id)?;
                let p = derive_state(&competency, &observations, &rules, env.now);
                let mut per_dimension = Map::new();
                for d in LearningDimension::ALL {
                    let key = serde_json::to_value(d)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_default();
                    let value = match p.dimension(d) {
                        Some(e) => json!({
                            "score_estimate": e.independence_weighted_mean,
                            "n_unaided": e.n_unaided,
                            "last_observed_at": e.last_observed_at,
                            "uncertainty": e.uncertainty.get(),
                        }),
                        None => json!({
                            "score_estimate": Value::Null,
                            "n_unaided": 0,
                            "last_observed_at": Value::Null,
                            "uncertainty": Value::Null,
                        }),
                    };
                    per_dimension.insert(key, value);
                }
                let objective_ids: Vec<String> =
                    repo::objectives_for_competency(env.conn, &row.id)?
                        .into_iter()
                        .map(|o| tail(&o.objective_id).to_string())
                        .collect();
                projections.push(json!({
                    "competency_id": row.id,
                    "name": row.title,
                    "state": p.state,
                    "provisional": false,
                    "per_dimension": per_dimension,
                    "contradictions": p.contradictions.iter().map(|c| c.reason.clone()).collect::<Vec<_>>(),
                    "evidence_refs": p.evidence_refs,
                    "objective_ids": objective_ids,
                }));
            }
            Ok(Mutation::new(
                json!({ "derived_at": env.now, "projections": projections }),
                None,
            ))
        },
    )
}
