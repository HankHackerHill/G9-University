//! Authoring (M06, M07): competencies and assessments the operator writes.
//! Before, they existed only through the CLI seed.
//!
//! * `competency.create` validates through `g9_competency::Competency` and
//!   stores the row (difficulty band 1–5, no NICE references).
//! * `assessment.define` writes, in one transaction, the definition, version 1
//!   (task statement, declared difficulty, checksum) and its rubric. The rubric
//!   is the baseline rubric: grading maps criterion codes to competency
//!   dimensions through `Rubric::baseline_dimension`, and `rubric_criteria` has
//!   no dimension column, so custom criteria could not be graded (PROPOSED: a
//!   dimension column and a criteria editor).

use super::{creation_id, decode, new_id, parse_id, rc};
use crate::context::AppContext;
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation};
use g9_assessment::{AssessmentDefinition, AssessmentKind, AssessmentVersion, Rubric};
use g9_competency::Competency;
use g9_domain::{
    AssessmentId, Classification, CommandMeta, CommandOutcome, CompetencyId, DataDomain,
    Difficulty, EvidenceId, G9Error, ResourceKind, ResourceRef,
};
use g9_storage::repos::assessment::{self as assessment_repo, AssessmentDefinitionRow};
use g9_storage::repos::competency::{self as competency_repo, CompetencyRow};
use g9_storage::repos::ReadScope;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
struct CompetencyRequest {
    code: String,
    title: String,
    description: Option<String>,
    validity_days: Option<u32>,
}

/// `competency.create`.
pub fn competency_create(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: CompetencyRequest = decode(payload)?;
    let id: CompetencyId = parse_id(
        &creation_id(ctx, spec, meta, || new_id("comp"))?,
        "competency_id",
    )?;
    let resource = ResourceRef::new(ResourceKind::Competency, id.as_str());
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
            let band = |d: u8| {
                Difficulty::new(d).map_err(|_| G9Error::internal_redacted("authoring.difficulty"))
            };
            let competency = Competency {
                id: id.clone(),
                name: req.title.trim().to_string(),
                pack: "operator".to_string(),
                domain: DataDomain::Learning,
                description: req.description.clone().unwrap_or_default(),
                validity_window_days: req.validity_days.unwrap_or(180),
                difficulty_band: (band(1)?, band(5)?),
                nice_refs: Vec::new(),
            };
            competency
                .validate()
                .map_err(|e| G9Error::invalid_input("competency.invalid", e.to_string()))?;
            let code = req.code.trim().to_string();
            let revision = competency_repo::insert_competency(
                env.conn,
                &CompetencyRow::from_domain(&competency, &code, None, env.now),
            )?;
            Ok(Mutation::new(
                json!({
                    "competency_id": id,
                    "code": code,
                    "title": competency.name,
                    "validity_days": competency.validity_window_days,
                }),
                Some(revision),
            )
            .with_event(EventSpec::learning(
                "competency.created",
                json!({ "competency_id": id }),
            )))
        },
    )
}

#[derive(Debug, Deserialize)]
struct DefineRequest {
    code: String,
    title: String,
    kind: String,
    competency_id: String,
    task_statement: String,
    difficulty: u8,
}

fn kind_of(label: &str) -> Result<AssessmentKind, G9Error> {
    Ok(match label {
        // The kinds storage can persist (weekly and diagnostic kinds are a
        // reported storage gap).
        "BLIND_PRACTICAL" => AssessmentKind::BlindPractical,
        "CAPSTONE" => AssessmentKind::Capstone,
        "TRANSFER_CHECK" => AssessmentKind::TransferCheck,
        _ => {
            return Err(G9Error::invalid_input(
                "assessment.kind_invalid",
                "an assessment you define is BLIND_PRACTICAL, CAPSTONE or TRANSFER_CHECK",
            ))
        }
    })
}

/// `assessment.define`: definition, version 1 and its (baseline) rubric.
pub fn assessment_define(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: DefineRequest = decode(payload)?;
    let id: AssessmentId = parse_id(
        &creation_id(ctx, spec, meta, || new_id("ad"))?,
        "assessment_id",
    )?;
    let resource = ResourceRef::new(ResourceKind::AssessmentDefinition, id.as_str());
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
            let competency_id: CompetencyId = parse_id(&req.competency_id, "competency_id")?;
            competency_repo::get_competency(
                env.conn,
                &ReadScope::cross_domain(DataDomain::Learning),
                &competency_id,
            )?
            .ok_or_else(|| G9Error::not_found("competency.not_found", "competency not found"))?;
            let kind = kind_of(&req.kind)?;
            let difficulty = Difficulty::new(req.difficulty).map_err(|_| {
                G9Error::invalid_input("assessment.difficulty_invalid", "difficulty is 1 to 5")
            })?;
            let code = req.code.trim().to_string();
            let definition = AssessmentDefinition::new(
                id.clone(),
                req.title.trim().to_string(),
                vec![competency_id.clone()],
                kind,
                DataDomain::Learning,
            )
            .map_err(|e| G9Error::invalid_input("assessment.definition_invalid", e.to_string()))?;
            let row = AssessmentDefinitionRow::from_domain(&definition, &code, env.now)
                .map_err(G9Error::from)?;
            assessment_repo::insert_definition(env.conn, &row)?;

            let task_ref = EvidenceId::parse(&format!("{}-task", id.as_str()))
                .map_err(|_| G9Error::internal_redacted("authoring.task_ref"))?;
            let version = AssessmentVersion {
                definition_id: id.clone(),
                version: 1,
                rubric: Rubric::baseline(),
                task_ref,
                sealed_key_ref: None,
                difficulty,
                equivalence_group: code.clone(),
                created_at: env.now,
                checksum: String::new(),
            }
            .with_checksum()
            .map_err(|e| G9Error::invalid_input("assessment.version_invalid", e.to_string()))?;
            let version_id = format!("{}:v1", id.as_str());
            let version_row = assessment_repo::AssessmentVersionRow::from_domain(
                &version,
                &version_id,
                req.task_statement.trim(),
                None,
                env.meta.actor.id.clone(),
            )
            .map_err(G9Error::from)?;
            assessment_repo::insert_version(env.conn, &version_row)?;
            let (rubric, criteria) = assessment_repo::rubric_rows_from_domain(
                &version.rubric,
                &format!("{}:rubric", id.as_str()),
                &version_id,
                "Baseline rubric",
                "v1",
                env.now,
            );
            assessment_repo::insert_rubric(env.conn, &rubric, &criteria)?;
            let codes: Vec<String> = version
                .rubric
                .criteria()
                .iter()
                .map(|c| c.code.clone())
                .collect();
            Ok(Mutation::new(
                json!({
                    "assessment_id": id,
                    "code": code,
                    "title": definition.title,
                    "kind": req.kind,
                    "competency_id": competency_id,
                    "version_id": version_id,
                    "difficulty": req.difficulty,
                    "criteria": codes,
                }),
                Some(g9_domain::Revision::INITIAL),
            )
            .with_event(EventSpec::learning(
                "assessment.defined",
                json!({ "assessment_id": id }),
            )))
        },
    )
}
