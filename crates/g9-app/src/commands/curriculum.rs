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
//!   An objective's `status` is derived from the study sessions linked to it
//!   (`objective_status`; app-7), never a constant and never `VERIFIED`.
//!
//! Two curriculum shapes are accepted (ADR-0023, `g9_domain::curriculum`): the
//! Appendix A shape (`CT9_480`: six days a week, Saturday assessed) and the
//! owner's v2.0 plan (`MASTERY_V2`: five days a week, Friday assessed). The
//! shape of a stored version is the profile its validation report names
//! (`CurriculumVersionRow::structure_profile`); which dates have a training day,
//! which day is assessed and what a block is called come from that profile.

use super::{decode, parse_id, rc, tail, to_value};
use crate::context::{AppContext, Services};
use crate::ops::OpSpec;
use crate::pipeline::{self, Command, EventSpec, Facts, Mutation};
use g9_curriculum::{
    check_transition, json_checksum, manifest_checksum, reconcile, validate as validate_manifest,
    CivilDate, Manifest, PromotionBlocked, ReconciliationReport, ValidationReport,
};
use g9_domain::{
    Classification, CommandMeta, CommandOutcome, CurriculumId, CurriculumVersionId, DataDomain,
    G9Error, ManifestProvenance, ResourceKind, ResourceRef, SessionState, StructureProfile,
};
use g9_storage::repos::curriculum::{
    self as repo, CurriculumVersionRow, NewCurriculumVersion, ProvenanceApproval, TrainingDayRow,
    WeekRow,
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
            format!(
                "{what} does not match the curriculum manifest format (v1.0.0 or v1.1.0) ({kind})"
            ),
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

/// The version daily plans and objective links read (`repo::active_version`): the current
/// `APPROVED_CANONICAL` version, otherwise the most recently imported version (callers label
/// non-canonical results). It is not bound to `AppConfig::curriculum_id`: two curricula can share
/// a database (ADR-0023), and the program the operator imported or promoted last is the one in use.
pub(crate) fn active_version(
    conn: &Connection,
    _svc: &Services,
) -> Result<Option<CurriculumVersionRow>, G9Error> {
    Ok(repo::active_version(conn)?)
}

/// The curriculum the program plan (the start Monday and the day start) belongs to: that of the
/// active version, the configured one while nothing is imported.
pub(crate) fn active_curriculum_id(
    conn: &Connection,
    svc: &Services,
) -> Result<CurriculumId, G9Error> {
    Ok(match active_version(conn, svc)? {
        Some(version) => version.curriculum_id,
        None => svc.config.curriculum_id.clone(),
    })
}

/// The Monday of week 1: the bridge's override (`AppConfig::program_start`, the CLI's
/// `--program-start`) or, failing that, the plan the operator set (`curriculum.set_program_start`).
pub(crate) fn effective_program_start(
    conn: &Connection,
    svc: &Services,
) -> Result<Option<CivilDate>, G9Error> {
    if let Some(start) = svc.config.program_start {
        return Ok(Some(start));
    }
    match repo::get_program_plan(conn, &active_curriculum_id(conn, svc)?)? {
        Some(plan) => Ok(Some(parse_date(&plan.start_monday)?)),
        None => Ok(None),
    }
}

/// The training day of the active version that falls on `date`: the program runs on the training
/// days of the version's structure profile (Monday to Saturday in `CT9_480`, Monday to Friday in
/// `MASTERY_V2`) of each week counted from the program start. `None` for a day the profile does not
/// train on (a Sunday; a Saturday of a `MASTERY_V2` version), a date outside the weeks the version
/// holds and a day the version does not define.
pub(crate) fn training_day_on(
    conn: &Connection,
    svc: &Services,
    date: CivilDate,
) -> Result<Option<(CurriculumVersionRow, WeekRow, TrainingDayRow)>, G9Error> {
    let version = active_version(conn, svc)?.ok_or_else(|| {
        G9Error::not_found(
            "curriculum.not_imported",
            "no curriculum version is imported",
        )
    })?;
    let start = effective_program_start(conn, svc)?.ok_or_else(|| {
        G9Error::precondition_failed(
            "curriculum.program_start_not_configured",
            "the program start date (Monday of week 1) is not configured",
        )
    })?;
    let profile = version.structure_profile()?;
    let index = date.to_days() - start.to_days();
    let week = index.div_euclid(7) + 1;
    let weekday = index.rem_euclid(7) + 1;
    if !is_training_weekday(profile, weekday) {
        return Ok(None);
    }
    let Some(week_row) = repo::list_weeks(conn, &version.id)?
        .into_iter()
        .find(|w| i64::from(w.week_number) == week)
    else {
        return Ok(None);
    };
    let day = repo::list_training_days(conn, &version.id)?
        .into_iter()
        .find(|d| d.week_id == week_row.id && i64::from(d.day_of_week.get()) == weekday);
    Ok(day.map(|d| (version, week_row, d)))
}

/// Whether `weekday` (1 = Monday .. 7 = Sunday) is a training day of `profile`.
fn is_training_weekday(profile: &StructureProfile, weekday: i64) -> bool {
    u8::try_from(weekday).is_ok_and(|d| profile.is_training_day(d))
}

/// The minute of the day a planned day starts when the operator set no other (09:00).
pub(crate) const DEFAULT_DAY_START_MINUTE: u32 = 540;

/// `HH:MM` as minutes after midnight.
fn parse_day_start(text: &str) -> Result<u32, G9Error> {
    let bad = || {
        G9Error::invalid_input(
            "program.day_start_invalid",
            "the time a day starts is HH:MM, from 00:00 to 23:59",
        )
    };
    let (h, m) = text.split_once(':').ok_or_else(bad)?;
    if h.len() != 2 || m.len() != 2 {
        return Err(bad());
    }
    let (h, m): (u32, u32) = (h.parse().map_err(|_| bad())?, m.parse().map_err(|_| bad())?);
    if h > 23 || m > 59 {
        return Err(bad());
    }
    Ok(h * 60 + m)
}

fn format_day_start(minute: u32) -> String {
    format!("{:02}:{:02}", minute / 60, minute % 60)
}

/// `curriculum.program`: what is imported and where the program sits on the calendar
/// (read-only). The setup of the interface reads this to know what is left to do.
pub fn program(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let resource = ResourceRef::new(ResourceKind::Curriculum, "program");
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
            let curriculum = match active_version(env.conn, env.svc)? {
                None => Value::Null,
                Some(version) => {
                    let profile = version.structure_profile()?;
                    let days = repo::list_training_days(env.conn, &version.id)?;
                    json!({
                        "version_id": version.id,
                        "name": repo::curriculum_title(env.conn, &version.curriculum_id)?,
                        "version_label": version.version_label,
                        "provenance": version.provenance,
                        // The structure profile the version was imported under (ADR-0023) and its
                        // training days a week (6, or 5 in MASTERY_V2).
                        "profile": profile.id,
                        "days_per_week": profile.days_per_week(),
                        "weeks": repo::list_weeks(env.conn, &version.id)?.len(),
                        "days": days.len(),
                        "objectives": repo::count_objectives(env.conn, &version.id)?,
                        "planned_minutes": days.iter().map(|d| u64::from(d.planned_minutes)).sum::<u64>(),
                        "imported_at": version.imported_at,
                    })
                }
            };
            let start = match repo::get_program_plan(
                env.conn,
                &active_curriculum_id(env.conn, env.svc)?,
            )? {
                Some(plan) => json!({
                    "monday": plan.start_monday,
                    "day_start": format_day_start(plan.day_start_minute),
                    "revision": plan.revision.get(),
                }),
                None => Value::Null,
            };
            let shipped = super::packs::catalog()?;
            let loaded = g9_storage::repos::competency::list_competencies(
                env.conn,
                &ReadScope::domain(DataDomain::Learning),
            )?
            .iter()
            .filter(|c| shipped.contains(c.id.as_str()))
            .count();
            let mut m = Mutation::new(Value::Null, None);
            m.result = to_value(&json!({
                "curriculum": curriculum,
                "program_start": start,
                "competencies": { "loaded": loaded, "available": shipped.len() },
            }))?;
            Ok(m)
        },
    )
}

#[derive(Debug, Deserialize)]
struct SetProgramStartRequest {
    date: String,
    day_start: Option<String>,
}

/// `curriculum.set_program_start`: the Monday of week 1 and the time of day a planned day
/// starts. The curriculum names weeks, days and blocks but no dates; this anchors it on the
/// calendar. Sessions already planned keep their own times.
pub fn set_program_start(
    ctx: &mut AppContext,
    spec: &'static OpSpec,
    meta: &CommandMeta,
    payload: &Value,
) -> Result<CommandOutcome<Value>, G9Error> {
    let req: SetProgramStartRequest = decode(payload)?;
    let date = parse_date(&req.date)?;
    if date.iso_weekday() != 1 {
        return Err(G9Error::invalid_input(
            "program.start_not_a_monday",
            "the program starts on a Monday: each program week begins on its Monday",
        ));
    }
    let minute = parse_day_start(req.day_start.as_deref().unwrap_or("09:00"))?;
    // The plan belongs to the curriculum of the active version (ADR-0023), not to the configured one.
    let curriculum_id = active_curriculum_id(ctx.database().conn(), &ctx.svc)?;
    let resource = ResourceRef::new(ResourceKind::Curriculum, curriculum_id.as_str());
    pipeline::execute(
        ctx,
        meta,
        Command {
            spec,
            resource: &resource,
            payload,
        },
        |conn, svc| {
            let context = rc(DataDomain::Learning, Classification::Internal, false);
            Ok(
                match repo::get_program_plan(conn, &active_curriculum_id(conn, svc)?)? {
                    Some(plan) => Facts::existing(context, plan.revision),
                    None => Facts::new_record(context),
                },
            )
        },
        |env| {
            if active_version(env.conn, env.svc)?.is_none() {
                return Err(G9Error::not_found(
                    "curriculum.not_imported",
                    "no curriculum version is imported: import the curriculum first",
                ));
            }
            let program_curriculum = active_curriculum_id(env.conn, env.svc)?;
            let revision = repo::put_program_plan(
                env.conn,
                &program_curriculum,
                &req.date,
                minute,
                None,
                env.now,
            )?;
            Ok(Mutation::new(
                json!({
                    "curriculum_id": program_curriculum,
                    "monday": req.date,
                    "day_start": format_day_start(minute),
                }),
                Some(revision),
            )
            .with_event(EventSpec::learning(
                "curriculum.program_start_set",
                json!({ "curriculum_id": program_curriculum, "monday": req.date }),
            )))
        },
    )
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

pub(crate) fn parse_date(s: &str) -> Result<CivilDate, G9Error> {
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

/// What the study sessions linked to an objective show (app-7): `PLANNED`
/// while none has started, `IN_PROGRESS` once one has, `COMPLETED` when every
/// linked session is completed (or reviewed, or closed). Cancelled sessions do
/// not count. Only sessions linked to the objective itself count
/// (`session.create` with `objective_id`): the sessions `session.plan_day`
/// makes are linked to the day's blocks, and no manifest says which block
/// serves which objective. `VERIFIED` is never derived: what verifies an
/// objective (a passing final decision, an observation) is an owner decision.
fn objective_status(linked: &[sessions_repo::StudySessionRow]) -> &'static str {
    let counted: Vec<SessionState> = linked
        .iter()
        .map(|s| s.state)
        .filter(|s| *s != SessionState::Cancelled)
        .collect();
    let done = |s: &SessionState| {
        matches!(
            s,
            SessionState::Completed | SessionState::Reviewed | SessionState::Closed
        )
    };
    let started = |s: &SessionState| !matches!(s, SessionState::Planned | SessionState::Ready);
    if !counted.is_empty() && counted.iter().all(done) {
        "COMPLETED"
    } else if counted.iter().any(started) {
        "IN_PROGRESS"
    } else {
        "PLANNED"
    }
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
            let version = active_version(env.conn, env.svc)?.ok_or_else(|| {
                G9Error::not_found(
                    "curriculum.not_imported",
                    "no curriculum version is imported",
                )
            })?;
            let start = effective_program_start(env.conn, env.svc)?.ok_or_else(|| {
                G9Error::precondition_failed(
                    "curriculum.program_start_not_configured",
                    "the program start date (Monday of week 1) is not configured",
                )
            })?;
            let profile = version.structure_profile()?;
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
            // A day the profile does not train on (a Sunday; a Saturday of MASTERY_V2) is answered
            // as a Sunday is: unscheduled, with the week it falls in.
            let Some(week_row) = week_row.filter(|_| is_training_weekday(profile, weekday)) else {
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
            let learning = ReadScope::domain(DataDomain::Learning);
            let objectives: Vec<Value> = repo::objectives_for_day(env.conn, &day.id)?
                .into_iter()
                .map(|o| {
                    let linked =
                        sessions_repo::list_sessions_for_objective(env.conn, &learning, &o.id)?;
                    Ok(json!({
                        "objective_id": o.objective_code,
                        "seq": o.ordinal,
                        "text": o.statement,
                        "status": objective_status(&linked),
                    }))
                })
                .collect::<Result<_, G9Error>>()?;
            let sessions = sessions_repo::list_sessions_for_day(
                env.conn,
                &ReadScope::domain(DataDomain::Learning),
                &day.id,
            )?;
            let assessment_day = profile.is_assessment_day(day.day_of_week.get());
            let blocks: Vec<Value> = repo::study_blocks(env.conn, &day.id)?
                .into_iter()
                .map(|b| {
                    let session = sessions
                        .iter()
                        .find(|s| s.study_block_id.as_deref() == Some(b.id.as_str()))
                        .map(|s| s.id.clone());
                    // `label`: what the profile calls this block on this kind of day (the
                    // code with spaces in CT9_480; "Core lesson", "Knowledge exam" ... in MASTERY_V2).
                    json!({
                        "seq": b.ordinal,
                        "block": b.block_code,
                        "label": profile.label_or_code(&b.block_code, assessment_day),
                        "minutes": b.minutes,
                        "session_id": session,
                    })
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
                // The assessed day of the version's profile: the Saturday of the Appendix A
                // structure (ARCHITECTURE §4), the Friday of MASTERY_V2.
                "is_assessment_day": assessment_day,
                "planned_minutes": day.planned_minutes,
                "evidence_artifact": artifact,
                "objectives": objectives,
                "blocks": blocks,
            }))?;
            Ok(m)
        },
    )
}
