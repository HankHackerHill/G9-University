//! Curriculum commands (M02; ARCHITECTURE.md §4 invariants; gate A01; C01).
//!
//! * `curriculum.validate` reports, never corrects.
//! * `curriculum.import` validates the manifest **before** policy (an invalid
//!   manifest never produces an approval request), requires an operator
//!   approval (`Capability::CurriculumImport`) and stores only
//!   `PROPOSED_SEED` / `RECONCILED_CANDIDATE` versions.
//! * `curriculum.promote` re-verifies the stored candidate (checksum), runs
//!   `g9_curriculum::reconcile` against the operator-supplied original, refuses
//!   unless the report allows promotion (`check_transition`), and requires an
//!   operator approval (`Capability::CurriculumPromote`).
//! * `curriculum.daily_plan` reads the stored structure of the active version.

use super::{decode, parse_id, rc, tail, to_value};
use crate::context::{AppContext, Services};
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation};
use g9_curriculum::{
    check_transition, json_checksum, manifest_checksum, reconcile, validate as validate_manifest,
    CivilDate, Manifest, PromotionBlocked, ReconciliationReport, ValidationReport,
};
use g9_domain::{
    Classification, CommandMeta, CommandOutcome, CurriculumVersionId, DataDomain, G9Error,
    ManifestProvenance, ResourceKind, ResourceRef,
};
use g9_storage::repos::curriculum::{
    self as repo, CurriculumVersionRow, NewCurriculumVersion, ProvenanceApproval,
};
use g9_storage::repos::sessions as sessions_repo;
use g9_storage::repos::ReadScope;
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{json, Value};

fn parse(value: &Value, what: &str) -> Result<Manifest, G9Error> {
    serde_json::from_value(value.clone()).map_err(|e| {
        // Position-free, content-free message: the manifest is untrusted data.
        let kind = if e.is_data() { "shape" } else { "syntax" };
        G9Error::invalid_input(
            "curriculum.manifest_unparseable",
            format!("{what} does not match the curriculum manifest format v1.0.0 ({kind})"),
        )
    })
}

fn violations(report: &ValidationReport) -> Vec<Value> {
    let mut out: Vec<Value> = report
        .errors
        .iter()
        .map(|e| json!({ "code": e.code, "path": e.path, "message": e.message }))
        .collect();
    out.extend(report.violations.iter().map(|v| {
        json!({
            "code": v.code,
            "path": "$.structure",
            "message": format!("expected {}, found {}", v.expected, v.actual),
        })
    }));
    out
}

/// The version daily plans and objective links read: the current
/// `APPROVED_CANONICAL` version of the configured curriculum, otherwise the
/// most recently imported version (callers label non-canonical results).
pub(crate) fn active_version(
    conn: &Connection,
    svc: &Services,
) -> Result<Option<CurriculumVersionRow>, G9Error> {
    if let Some(v) = repo::current_version(conn, &svc.config.curriculum_id)? {
        return Ok(Some(v));
    }
    let mut versions = repo::list_versions(conn, &svc.config.curriculum_id)?;
    versions.sort_by_key(|v| (v.imported_at, v.id.clone()));
    Ok(versions.pop())
}

#[derive(Debug, Deserialize)]
struct ValidateRequest {
    manifest: Value,
}

/// `curriculum.validate` (read-only).
pub fn validate(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: ValidateRequest = decode(payload)?;
    let resource = ResourceRef::new(ResourceKind::Curriculum, "manifest");
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
        |_env| {
            let checksum = json_checksum(&req.manifest);
            let result = match parse(&req.manifest, "manifest") {
                Ok(manifest) => {
                    let report = validate_manifest(&manifest);
                    json!({
                        "valid": report.is_valid(),
                        "violations": violations(&report),
                        "checksum_sha256": checksum.as_str(),
                        "provenance": manifest.provenance,
                    })
                }
                Err(e) => json!({
                    "valid": false,
                    "violations": [{ "code": e.code, "path": "$", "message": e.message }],
                    "checksum_sha256": checksum.as_str(),
                    "provenance": Value::Null,
                }),
            };
            Ok(Mutation::new(result, None))
        },
    )
}

#[derive(Debug, Deserialize)]
struct ImportRequest {
    manifest: Value,
    provenance: ManifestProvenance,
}

fn checked_import(req: &ImportRequest) -> Result<(Manifest, ValidationReport), G9Error> {
    let manifest = parse(&req.manifest, "manifest")?;
    let report = validate_manifest(&manifest);
    if !report.is_valid() {
        return Err(G9Error::invalid_input(
            "curriculum.invariant_violated",
            "manifest violates curriculum invariants or the manifest contract; import blocked",
        )
        .with_details(json!({ "violations": violations(&report) })));
    }
    if manifest.provenance != req.provenance {
        return Err(G9Error::invalid_input(
            "curriculum.provenance_mismatch",
            "requested provenance differs from the manifest's declared provenance",
        ));
    }
    if req.provenance == ManifestProvenance::ApprovedCanonical {
        return Err(G9Error::unauthorized(
            "curriculum.promote_requires_approval",
            "APPROVED_CANONICAL is reached only through curriculum.promote",
        ));
    }
    Ok((manifest, report))
}

/// `curriculum.import` (approval-gated: `CurriculumImport`).
pub fn import(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: ImportRequest = decode(payload)?;
    // The resource id is derived from the manifest identity so that the
    // approval binds to exactly this curriculum version.
    let version_id = req
        .manifest
        .get("curriculum_id")
        .and_then(Value::as_str)
        .zip(req.manifest.get("version").and_then(Value::as_str))
        .map(|(c, v)| format!("{c}:{v}"))
        .and_then(|s| CurriculumVersionId::parse(&s).ok())
        .unwrap_or_else(|| {
            CurriculumVersionId::parse("unparsed-manifest")
                .unwrap_or_else(|_| CurriculumVersionId::new_v7())
        });
    let resource = ResourceRef::new(ResourceKind::CurriculumVersion, version_id.as_str());
    let facts_id = version_id.clone();
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |conn, _| {
            checked_import(&req)?;
            if repo::get_version(conn, &facts_id)?.is_some() {
                return Err(G9Error::conflict(
                    "curriculum.version_exists",
                    "this curriculum version is already imported; publish a new version",
                ));
            }
            Ok(Facts::new_record(rc(
                DataDomain::Learning,
                Classification::Internal,
                false,
            )))
        },
        |env| {
            let (manifest, report) = checked_import(&req)?;
            let checksum = manifest_checksum(&manifest)?;
            repo::insert_version(
                env.conn,
                &NewCurriculumVersion {
                    id: version_id.clone(),
                    manifest: &manifest,
                    validation: &report,
                    provenance: req.provenance,
                    checksum: &checksum,
                    source_ref: "ipc:curriculum.import".to_string(),
                    imported_by: env.meta.actor.id.clone(),
                    imported_at: env.now,
                },
            )?;
            let result = json!({
                "curriculum_version_id": version_id,
                "checksum_sha256": checksum.as_str(),
                "provenance": req.provenance,
                "weeks": report.counts.weeks,
                "days": report.counts.days,
                "objectives": report.counts.objectives,
            });
            Ok(Mutation::new(result, Some(g9_domain::Revision::INITIAL)).with_event(EventSpec {
                content_sha256: Some(checksum.as_str().to_string()),
                ..EventSpec::learning(
                    "curriculum.imported",
                    json!({ "curriculum_version_id": version_id, "provenance": req.provenance }),
                )
            }))
        },
    )
}

#[derive(Debug, Deserialize)]
struct PromoteRequest {
    curriculum_version_id: String,
    candidate_manifest: Value,
    original_manifest: Value,
}

fn blocked(e: &PromotionBlocked, report: Option<&ReconciliationReport>) -> G9Error {
    G9Error::precondition_failed("curriculum.promotion_blocked", e.to_string()).with_details(
        json!({
            "promotion_allowed": report.map(|r| r.promotion_allowed),
            "summary": report.map(|r| json!({
                "unchanged": r.summary.unchanged,
                "changed": r.summary.changed,
                "missing_in_candidate": r.summary.missing_in_candidate,
                "added_in_candidate": r.summary.added_in_candidate,
            })),
        }),
    )
}

/// Load, verify and reconcile; `Ok` only when promotion is allowed.
fn checked_promotion(
    conn: &Connection,
    id: &CurriculumVersionId,
    req: &PromoteRequest,
) -> Result<(CurriculumVersionRow, ReconciliationReport), G9Error> {
    let row = repo::get_version(conn, id)?.ok_or_else(|| {
        G9Error::not_found(
            "curriculum.version_not_found",
            "curriculum version not found",
        )
    })?;
    let candidate = parse(&req.candidate_manifest, "candidate_manifest")?;
    let original = parse(&req.original_manifest, "original_manifest")?;
    let candidate_sum = manifest_checksum(&candidate)?;
    if !candidate_sum.matches(&row.manifest_sha256) {
        return Err(G9Error::invalid_input(
            "curriculum.candidate_checksum_mismatch",
            "candidate manifest does not hash to the stored version",
        ));
    }
    let report = reconcile(&original, &candidate)?;
    check_transition(
        row.provenance,
        ManifestProvenance::ApprovedCanonical,
        Some(&report),
    )
    .map_err(|e| blocked(&e, Some(&report)))?;
    if !report.promotion_allowed {
        return Err(G9Error::precondition_failed(
            "curriculum.promotion_blocked",
            "the reconciliation report does not allow promotion",
        ));
    }
    Ok((row, report))
}

/// `curriculum.promote` (approval-gated: `CurriculumPromote`).
pub fn promote(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: PromoteRequest = decode(payload)?;
    let id: CurriculumVersionId = parse_id(&req.curriculum_version_id, "curriculum_version_id")?;
    let resource = ResourceRef::new(ResourceKind::CurriculumVersion, id.as_str());
    let facts_id = id.clone();
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |conn, _| {
            let (row, _) = checked_promotion(conn, &facts_id, &req)?;
            Ok(Facts::existing(
                rc(
                    DataDomain::Learning,
                    Classification::Internal,
                    row.provenance == ManifestProvenance::ApprovedCanonical,
                ),
                row.revision,
            ))
        },
        |env| {
            let (row, report) = checked_promotion(env.conn, &id, &req)?;
            let consumed = env.consumed.as_ref().ok_or_else(|| {
                G9Error::approval_required(
                    "curriculum.promote_requires_approval",
                    "promotion requires a consumed operator approval",
                )
            })?;
            let revision = repo::set_provenance(
                env.conn,
                &id,
                row.revision,
                ManifestProvenance::ApprovedCanonical,
                Some(&ProvenanceApproval {
                    approval_id: consumed.approval_id.clone(),
                    approved_by: env.meta.actor.id.clone(),
                    approved_at: env.now,
                }),
                env.now,
            )?;
            let result = json!({
                "curriculum_version_id": id,
                "provenance": ManifestProvenance::ApprovedCanonical,
                "promotion_allowed": report.promotion_allowed,
                "original_checksum": report.original_checksum.as_str(),
                "candidate_checksum": report.candidate_checksum.as_str(),
                "diff_checksum": report.diff_checksum.as_str(),
                "summary": {
                    "unchanged": report.summary.unchanged,
                    "changed": report.summary.changed,
                    "missing_in_candidate": report.summary.missing_in_candidate,
                    "added_in_candidate": report.summary.added_in_candidate,
                    "metadata_changes": report.summary.metadata_changes,
                },
            });
            Ok(Mutation::new(result, Some(revision)).with_event(EventSpec {
                content_sha256: Some(report.candidate_checksum.as_str().to_string()),
                ..EventSpec::learning(
                    "curriculum.promoted",
                    json!({
                        "curriculum_version_id": id,
                        "diff_checksum": report.diff_checksum.as_str(),
                        "approval_id": consumed.approval_id,
                    }),
                )
            }))
        },
    )
}

#[derive(Debug, Deserialize)]
struct DailyPlanRequest {
    date: String,
    zone: Option<String>,
}

fn parse_date(s: &str) -> Result<CivilDate, G9Error> {
    let bad = || G9Error::invalid_input("date.invalid", "date must be 'YYYY-MM-DD'");
    let mut parts = s.split('-');
    let (Some(y), Some(m), Some(d), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(bad());
    };
    let year: i32 = y.parse().map_err(|_| bad())?;
    let month: u8 = m.parse().map_err(|_| bad())?;
    let day: u8 = d.parse().map_err(|_| bad())?;
    CivilDate::new(year, month, day).map_err(|_| bad())
}

/// `curriculum.daily_plan` (read-only).
pub fn daily_plan(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: DailyPlanRequest = decode(payload)?;
    let resource = ResourceRef::new(ResourceKind::CurriculumVersion, "active");
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
            let date = parse_date(&req.date)?;
            let zone = match &req.zone {
                Some(z) => g9_domain::IanaZone::parse(z)
                    .map_err(|_| G9Error::invalid_input("zone.invalid", "unknown IANA zone"))?,
                None => env.svc.config.zone.clone(),
            };
            let start = env.svc.config.program_start.ok_or_else(|| {
                G9Error::precondition_failed(
                    "curriculum.program_start_not_configured",
                    "the program start date (Monday of week 1) is not configured",
                )
            })?;
            let version = active_version(env.conn, env.svc)?.ok_or_else(|| {
                G9Error::not_found(
                    "curriculum.not_imported",
                    "no curriculum version is imported",
                )
            })?;
            let index = date.to_days() - start.to_days();
            let week = index.div_euclid(7) + 1;
            let weekday = index.rem_euclid(7) + 1;
            let empty = |week_number: Option<i64>, title: Option<String>| {
                json!({
                    "date": req.date, "zone": zone.as_str(), "day_id": Value::Null,
                    "week_number": week_number, "week_title": title, "unscheduled": true,
                    "is_assessment_day": false, "planned_minutes": 0, "evidence_artifact": Value::Null,
                    "objectives": [], "blocks": [],
                })
            };
            let weeks = repo::list_weeks(env.conn, &version.id)?;
            let week_row = weeks.iter().find(|w| i64::from(w.week_number) == week);
            let mut m = Mutation::new(Value::Null, None);
            if version.provenance != ManifestProvenance::ApprovedCanonical {
                m = m.with_warning(
                    "curriculum.non_canonical: plan derived from a version that is not APPROVED_CANONICAL",
                );
            }
            let Some(week_row) = week_row.filter(|_| weekday <= 6) else {
                m.result = empty(
                    week_row.map(|w| i64::from(w.week_number)),
                    week_row.map(|w| w.title.clone()),
                );
                return Ok(m);
            };
            let day = repo::list_training_days(env.conn, &version.id)?
                .into_iter()
                .find(|d| d.week_id == week_row.id && i64::from(d.day_of_week.get()) == weekday);
            let Some(day) = day else {
                m.result = empty(Some(week), Some(week_row.title.clone()));
                return Ok(m);
            };
            let objectives: Vec<Value> = repo::objectives_for_day(env.conn, &day.id)?
                .into_iter()
                .map(|o| {
                    json!({
                        "objective_id": o.objective_code,
                        "seq": o.ordinal,
                        "text": o.statement,
                        "status": "PLANNED",
                    })
                })
                .collect();
            let sessions = sessions_repo::list_sessions_for_day(
                env.conn,
                &ReadScope::domain(DataDomain::Learning),
                &day.id,
            )?;
            let blocks: Vec<Value> = repo::study_blocks(env.conn, &day.id)?
                .into_iter()
                .map(|b| {
                    let session = sessions
                        .iter()
                        .find(|s| s.study_block_id.as_deref() == Some(b.id.as_str()))
                        .map(|s| s.id.clone());
                    json!({ "seq": b.ordinal, "block": b.block_code, "minutes": b.minutes, "session_id": session })
                })
                .collect();
            let artifact = repo::deliverables(env.conn, &day.id)?
                .into_iter()
                .next()
                .map(|d| d.title);
            m.result = to_value(&json!({
                "date": req.date,
                "zone": zone.as_str(),
                "day_id": tail(day.day_code.as_str()),
                "week_number": week_row.week_number,
                "week_title": week_row.title,
                "unscheduled": false,
                // Saturday is the blind-practical day of the program structure (ARCHITECTURE §4).
                "is_assessment_day": day.day_of_week.is_saturday(),
                "planned_minutes": day.planned_minutes,
                "evidence_artifact": artifact,
                "objectives": objectives,
                "blocks": blocks,
            }))?;
            Ok(m)
        },
    )
}
