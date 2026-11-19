//! Gate A01 (curriculum invariants / provenance) and the crate's public contract.
//! Tests are deterministic and need no I/O beyond the compiled-in seed.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use g9_curriculum::*;
use g9_domain::{ErrorCategory, G9Error, ManifestProvenance, UtcMillis, CURRICULUM_INVARIANTS};
use pretty_assertions::assert_eq;
use std::collections::BTreeSet;

const SEED: &str = include_str!("../../../data/curriculum/g9-ct9-480.seed.v0.json");

fn seed() -> Manifest {
    parse_manifest(SEED).expect("seed parses")
}

fn hex64(c: char) -> String {
    std::iter::repeat_n(c, 64).collect()
}

fn reconciliation_block(original: &str, diff: &str) -> Reconciliation {
    Reconciliation {
        original_checksum: original.to_string(),
        diff_checksum: diff.to_string(),
        approved_by: "operator".to_string(),
        approved_at: UtcMillis::from_millis(1_762_387_200_000),
    }
}

// ---------------------------------------------------------------- A01: seed

#[test]
fn a01_seed_passes_all_invariants() {
    let m = seed();
    let report = validate(&m);
    assert_eq!(report.errors, Vec::<ManifestError>::new());
    assert_eq!(report.violations, vec![]);
    assert!(report.is_valid(), "{}", report.summary_line());

    let inv = CURRICULUM_INVARIANTS;
    assert_eq!(report.counts.weeks, inv.weeks);
    assert_eq!(report.counts.days, inv.total_days);
    assert_eq!(report.counts.objectives, inv.total_objectives);
    assert_eq!(report.counts.total_minutes, inv.total_minutes);
    assert_eq!(report.counts.weekday_days, inv.weeks * 5);
    assert_eq!(report.counts.saturday_days, inv.weeks);

    // The seed is a transcription: one objective text repeats ("Verify mitigations").
    // That is a warning, never an error.
    assert_eq!(report.warnings.len(), 1, "{:?}", report.warnings);
    assert_eq!(
        report.warnings[0].code,
        codes::WARN_OBJECTIVE_TEXT_DUPLICATE
    );

    // Structural facts, independent of the validator.
    let ids: Vec<&str> = m.days().map(|(_, d)| d.day_id.as_str()).collect();
    let expected: Vec<String> = (1..=72).map(|i| format!("D{i:02}")).collect();
    assert_eq!(ids, expected);
    for (w, d) in m.days() {
        assert_eq!(d.objectives.len(), 3, "{}", d.day_id);
        assert_eq!(d.is_assessment_day, d.day_of_week == 6, "{}", d.day_id);
        assert_eq!(
            d.planned_minutes,
            if d.day_of_week == 6 { 300 } else { 420 }
        );
        assert!(w.week_number >= 1 && w.week_number <= 12);
        for (i, o) in d.objectives.iter().enumerate() {
            assert_eq!(o.objective_id, format!("{}-O{}", d.day_id, i + 1));
        }
    }
    assert_eq!(m.provenance, ManifestProvenance::ProposedSeed);
    assert!(!m.is_authoritative());
    assert!(m.reconciliation.is_none());
    assert_eq!(m.manifest_schema_version, MANIFEST_FORMAT_VERSION);

    let (_, loaded) = load_manifest(SEED).expect("load_manifest accepts the seed");
    assert_eq!(loaded, report);
}

// ------------------------------------------------- A01: every violation code

type Mutation = (&'static str, fn(&mut Manifest));

fn error_mutations() -> Vec<Mutation> {
    vec![
        (codes::SCHEMA_VERSION_UNSUPPORTED, |m| {
            m.manifest_schema_version = "2.0.0".into()
        }),
        // ADR-0023: format 1.1.0 is read (one optional field); the first minor that is
        // refused is now 1.2. (This mutation was "1.1.0" while 1.0 was the only format.)
        (codes::SCHEMA_VERSION_UNSUPPORTED, |m| {
            m.manifest_schema_version = "1.2.0".into()
        }),
        (codes::SCHEMA_VERSION_UNSUPPORTED, |m| {
            m.manifest_schema_version = "1.0".into()
        }),
        (codes::CURRICULUM_ID_INVALID, |m| {
            m.curriculum_id = "bad id!".into()
        }),
        (codes::NAME_EMPTY, |m| m.name = "   ".into()),
        (codes::VERSION_EMPTY, |m| m.version = String::new()),
        (codes::STRUCTURE_MISMATCH, |m| {
            m.structure.total_minutes = 28_000
        }),
        (codes::STRUCTURE_MISMATCH, |m| {
            m.structure.objectives_per_day = 1
        }),
        // The seed is format 1.0.0: the field that format 1.1.0 adds is refused in it,
        // whatever its value (6 is what its absence means; 5 would declare the other profile).
        (codes::STRUCTURE_ASSESSMENT_DAY_FORMAT, |m| {
            m.structure.assessment_day_of_week = Some(6)
        }),
        (codes::STRUCTURE_ASSESSMENT_DAY_FORMAT, |m| {
            m.structure.assessment_day_of_week = Some(5)
        }),
        // A five-day week without the field matches no profile: still the Appendix A check.
        (codes::STRUCTURE_MISMATCH, |m| m.structure.days_per_week = 5),
        (codes::STRUCTURE_SUNDAY, |m| {
            m.structure.sunday = "optional".into()
        }),
        (codes::WEEKDAY_BLOCKS_CANONICAL, |m| {
            m.weekday_blocks[0].minutes = 20
        }),
        (codes::WEEKDAY_BLOCKS_CANONICAL, |m| {
            m.weekday_blocks[1].block = "lecture".into()
        }),
        (codes::SATURDAY_BLOCKS_CANONICAL, |m| {
            m.saturday_blocks.pop();
        }),
        (codes::WEEK_COUNT, |m| {
            m.weeks.pop();
        }),
        (codes::WEEK_NUMBER_SEQUENCE, |m| m.weeks[3].week_number = 9),
        (codes::WEEK_TITLE_EMPTY, |m| {
            m.weeks[0].title = String::new()
        }),
        (codes::WEEK_DAYS_COUNT, |m| {
            m.weeks[0].days.pop();
        }),
        (codes::DAY_OF_WEEK_RANGE, |m| {
            m.weeks[0].days[0].day_of_week = 0
        }),
        (codes::DAY_OF_WEEK_RANGE, |m| {
            m.weeks[0].days[0].day_of_week = 8
        }),
        (codes::DAY_SUNDAY_PRESENT, |m| {
            m.weeks[0].days[5].day_of_week = 7
        }),
        (codes::DAY_OF_WEEK_ORDER, |m| {
            m.weeks[0].days[1].day_of_week = 1
        }),
        (codes::DAY_OF_WEEK_ORDER, |m| {
            m.weeks[2].days.swap(1, 2);
        }),
        (codes::DAY_ID_MISMATCH, |m| {
            m.weeks[0].days[0].day_id = "D1".into()
        }),
        (codes::DAY_ID_MISMATCH, |m| {
            // D07 placed in week 1: impossible day order
            m.weeks[0].days[5].day_id = "D07".into();
        }),
        (codes::DAY_ID_DUPLICATE, |m| {
            m.weeks[0].days[1].day_id = "D01".into()
        }),
        (codes::DAY_PLANNED_MINUTES, |m| {
            m.weeks[0].days[0].planned_minutes = 300
        }),
        (codes::DAY_OBJECTIVES_COUNT, |m| {
            m.weeks[0].days[0].objectives.pop();
        }),
        (codes::DAY_OBJECTIVES_COUNT, |m| {
            let extra = m.weeks[0].days[0].objectives[0].clone();
            m.weeks[0].days[0].objectives.push(extra);
        }),
        (codes::DAY_EVIDENCE_EMPTY, |m| {
            m.weeks[0].days[0].evidence_artifact = " ".into()
        }),
        (codes::DAY_BLOCKS_SUM, |m| {
            m.weeks[0].days[0].blocks[0].minutes = 16
        }),
        (codes::DAY_BLOCKS_CANONICAL, |m| {
            m.weeks[0].days[0].blocks[0].block = "warmup".into()
        }),
        (codes::DAY_BLOCKS_CANONICAL, |m| {
            // Saturday blocks on a weekday (sum still 300 != 420 -> also a sum error)
            let sat = m.weeks[0].days[5].blocks.clone();
            m.weeks[0].days[0].blocks = sat;
        }),
        (codes::DAY_ASSESSMENT_FLAG, |m| {
            m.weeks[0].days[0].is_assessment_day = true
        }),
        (codes::DAY_ASSESSMENT_FLAG, |m| {
            m.weeks[0].days[5].is_assessment_day = false
        }),
        (codes::OBJECTIVE_SEQ, |m| {
            m.weeks[0].days[0].objectives[1].seq = 5
        }),
        (codes::OBJECTIVE_ID_MISMATCH, |m| {
            m.weeks[0].days[0].objectives[0].objective_id = "D01-O9".into()
        }),
        (codes::OBJECTIVE_ID_DUPLICATE, |m| {
            m.weeks[0].days[0].objectives[1].objective_id = "D01-O1".into()
        }),
        (codes::OBJECTIVE_TEXT_EMPTY, |m| {
            m.weeks[0].days[0].objectives[0].text = "\t".into()
        }),
        (codes::TEXT_HIDDEN_CHARACTER, |m| {
            // right-to-left override: "...report<U+202E>fdp.exe" is shown reordered
            m.weeks[0].days[0].objectives[0].text = "Review the ACL report\u{202E}fdp.exe".into()
        }),
        (codes::TEXT_HIDDEN_CHARACTER, |m| {
            m.weeks[0].days[0].evidence_artifact = "report\u{200B}".into()
        }),
        (codes::TEXT_HIDDEN_CHARACTER, |m| {
            m.name = "name\u{2066}".into()
        }),
        (codes::TEXT_HIDDEN_CHARACTER, |m| {
            m.version = "2.0.0\u{200B}".into()
        }),
        (codes::TEXT_HIDDEN_CHARACTER, |m| {
            // a blank-rendering filler is not in `is_hidden` and is refused all the same
            m.weeks[0].days[0].objectives[0].text = "report\u{3164}".into()
        }),
        (codes::TEXT_HIDDEN_CHARACTER, |m| {
            m.weeks[0].title = "title\u{FEFF}".into()
        }),
        (codes::TEXT_HIDDEN_CHARACTER, |m| {
            m.provenance_note = Some("note\u{E0041}".into())
        }),
        (codes::TEXT_HIDDEN_CHARACTER, |m| {
            m.weeks[0].days[0].resources.push(Resource {
                reference: "https://portswigger.net/web-security\u{200E}".into(),
                kind: ResourceKind::External,
                license_note: Some("free tier".into()),
                version: Some("2026-11".into()),
            })
        }),
        (codes::COMPETENCY_ID_INVALID, |m| {
            m.weeks[0].days[0].objectives[0].competency_ids = vec!["has space".into()]
        }),
        (codes::COMPETENCY_ID_INVALID, |m| {
            m.weeks[0].days[0].objectives[0].competency_ids = vec![String::new()]
        }),
        (codes::BLOCK_SEQ, |m| m.weeks[0].days[0].blocks[0].seq = 2),
        (codes::DURATION_NON_POSITIVE, |m| {
            m.weeks[0].days[0].blocks[0].minutes = -15
        }),
        (codes::DURATION_NON_POSITIVE, |m| {
            m.weeks[0].days[0].blocks[0].minutes = 0
        }),
        (codes::DURATION_NON_POSITIVE, |m| {
            m.weeks[0].days[0].planned_minutes = 0
        }),
        (codes::DURATION_NON_POSITIVE, |m| {
            m.weekday_blocks[0].minutes = -1
        }),
        (codes::RESOURCE_REF_EMPTY, |m| {
            m.weeks[0].days[0].resources.push(Resource {
                reference: "  ".into(),
                kind: ResourceKind::Internal,
                license_note: None,
                version: None,
            })
        }),
        (codes::RESOURCE_VERSION_REQUIRED, |m| {
            m.weeks[0].days[0].resources.push(Resource {
                reference: "https://portswigger.net/web-security".into(),
                kind: ResourceKind::External,
                license_note: Some("free tier".into()),
                version: None,
            })
        }),
        (codes::RESOURCE_VERSION_REQUIRED, |m| {
            m.weeks[0].days[0].resources.push(Resource {
                reference: "https://portswigger.net/web-security".into(),
                kind: ResourceKind::External,
                license_note: None,
                version: Some("  ".into()),
            })
        }),
        (codes::PROVENANCE_RECONCILIATION_REQUIRED, |m| {
            m.provenance = ManifestProvenance::ReconciledCandidate
        }),
        (codes::PROVENANCE_RECONCILIATION_REQUIRED, |m| {
            m.provenance = ManifestProvenance::ApprovedCanonical
        }),
        (codes::RECONCILIATION_INCOMPLETE, |m| {
            m.provenance = ManifestProvenance::ReconciledCandidate;
            m.reconciliation = Some(reconciliation_block("not-hex", &hex64('a')));
        }),
        (codes::RECONCILIATION_INCOMPLETE, |m| {
            m.provenance = ManifestProvenance::ReconciledCandidate;
            m.reconciliation = Some(reconciliation_block(&hex64('a'), &hex64('A')));
        }),
        (codes::RECONCILIATION_INCOMPLETE, |m| {
            m.provenance = ManifestProvenance::ApprovedCanonical;
            let mut r = reconciliation_block(&hex64('a'), &hex64('b'));
            r.approved_by = String::new();
            m.reconciliation = Some(r);
        }),
        (codes::RECONCILIATION_INCOMPLETE, |m| {
            m.provenance = ManifestProvenance::ApprovedCanonical;
            let mut r = reconciliation_block(&hex64('a'), &hex64('b'));
            r.approved_at = UtcMillis::from_millis(0);
            m.reconciliation = Some(r);
        }),
    ]
}

fn warning_mutations() -> Vec<Mutation> {
    vec![
        (codes::WARN_OBJECTIVE_TEXT_DUPLICATE, |_| {}),
        (codes::WARN_COMPETENCY_ID_DUPLICATE, |m| {
            m.weeks[0].days[0].objectives[0].competency_ids = vec!["c1".into(), "c1".into()]
        }),
        (codes::WARN_RECONCILIATION_IGNORED, |m| {
            m.reconciliation = Some(reconciliation_block(&hex64('a'), &hex64('b')));
        }),
    ]
}

#[test]
fn a01_rejects_each_violation() {
    let mut covered: BTreeSet<&str> = BTreeSet::new();
    for (i, (code, mutate)) in error_mutations().into_iter().enumerate() {
        let mut m = seed();
        mutate(&mut m);
        let report = validate(&m);
        assert!(
            report.has_error(code),
            "mutation #{i} expected error `{code}`; got {:?}",
            report.error_codes()
        );
        assert!(!report.is_valid(), "mutation #{i} must block import");
        // The report maps to the closed category set and carries the codes.
        let err: G9Error = report.clone().into_result().unwrap_err().into();
        assert_eq!(err.category, ErrorCategory::InvalidInput);
        assert_eq!(err.code, "curriculum.manifest.invalid");
        let details = err.details.expect("details");
        assert!(details["errors"].as_array().is_some_and(|e| !e.is_empty()));
        // Every error names a path into the manifest.
        for e in &report.errors {
            assert!(e.path.starts_with('/'), "{e:?}");
        }
        covered.insert(code);
    }
    let all: BTreeSet<&str> = codes::ALL_ERRORS.iter().copied().collect();
    let missing: Vec<&&str> = all.difference(&covered).collect();
    assert!(
        missing.is_empty(),
        "error codes without a mutation: {missing:?}"
    );

    let mut covered_warnings: BTreeSet<&str> = BTreeSet::new();
    for (code, mutate) in warning_mutations() {
        let mut m = seed();
        mutate(&mut m);
        let report = validate(&m);
        assert!(
            report.has_warning(code),
            "expected warning `{code}`; got {:?}",
            report.warnings
        );
        assert!(report.is_valid(), "warnings never block import: {code}");
        covered_warnings.insert(code);
    }
    let all_w: BTreeSet<&str> = codes::ALL_WARNINGS.iter().copied().collect();
    assert_eq!(covered_warnings, all_w);
}

#[test]
fn free_text_may_hold_tab_and_line_feed_and_the_hidden_character_report_is_bounded() {
    // TAB and LF are the layout characters free text may hold.
    let mut m = seed();
    m.weeks[0].days[0].objectives[0].text = "a\tb\nc".into();
    assert!(!validate(&m).has_error(codes::TEXT_HIDDEN_CHARACTER));

    // A manifest with a hidden character in every objective yields a bounded report whose
    // messages quote no text (audit 2026-11-18, V2-01 and V2-05).
    let mut m = seed();
    for week in &mut m.weeks {
        for day in &mut week.days {
            for obj in &mut day.objectives {
                obj.text = format!("{}\u{200B}", obj.text);
            }
        }
    }
    let report = validate(&m);
    let hidden: Vec<_> = report
        .errors
        .iter()
        .filter(|e| e.code == codes::TEXT_HIDDEN_CHARACTER)
        .collect();
    assert_eq!(hidden.len(), 20);
    assert_eq!(hidden[0].path, "/weeks/0/days/0/objectives/0/text");
    assert!(hidden[0].message.contains("U+200B"));
    assert!(!hidden[0]
        .message
        .contains(&m.weeks[0].days[0].objectives[0].text));
}

#[test]
fn a01_arithmetic_violations_come_from_domain_counts() {
    let mut m = seed();
    m.weeks[11].days.pop(); // drop D72
    let report = validate(&m);
    let codes_: Vec<&str> = report.violations.iter().map(|v| v.code).collect();
    assert_eq!(
        codes_,
        vec![
            "curriculum.days",
            "curriculum.objectives",
            "curriculum.total_minutes",
            "curriculum.saturday_days",
        ]
    );
    assert_eq!(report.counts.days, 71);
    assert_eq!(report.counts.total_minutes, 28_500);
    assert!(report.has_error(codes::WEEK_DAYS_COUNT));
}

// -------------------------------------------------- A01: provenance gate

#[test]
fn a01_seed_is_not_promotable_without_reconciliation() {
    use ManifestProvenance::*;
    let candidate = seed();

    // 1. No original approved plan -> no report -> blocked.
    assert_eq!(
        check_transition(ProposedSeed, ReconciledCandidate, None),
        Err(PromotionBlocked::ReconciliationReportRequired)
    );
    assert!(matches!(
        check_transition(ProposedSeed, ApprovedCanonical, None),
        Err(PromotionBlocked::TransitionNotAllowed { .. })
    ));

    // 2. An original that differs (stands in for the operator-supplied plan).
    let mut original = seed();
    original.version = "1.0.0-approved".into();
    original.weeks[4].days[2].objectives[1].text = "Original wording of D27-O2".into();
    let report = reconcile(&original, &candidate).unwrap();
    assert!(!report.promotion_allowed);
    assert_eq!(report.binding, ReconciliationBinding::Absent);
    assert_eq!(
        can_promote(&report),
        Err(PromotionBlocked::UnreconciledDifferences {
            changed: 1,
            missing: 0,
            added: 0,
            metadata: 0,
        })
    );
    assert!(matches!(
        check_transition(ProposedSeed, ReconciledCandidate, Some(&report)),
        Err(PromotionBlocked::UnreconciledDifferences { .. })
    ));
    assert!(report.human_readable.contains("promotion: BLOCKED"));
    assert!(report
        .human_readable
        .contains("D27 CHANGED: objectives[1].text"));

    // 3. Reconciliation block pointing at a different original -> blocked.
    let mut wrong = candidate.clone();
    wrong.provenance = ReconciledCandidate;
    wrong.reconciliation = Some(reconciliation_block(
        &hex64('0'),
        report.diff_checksum.as_str(),
    ));
    let r = reconcile(&original, &wrong).unwrap();
    assert!(!r.promotion_allowed);
    assert!(matches!(
        can_promote(&r),
        Err(PromotionBlocked::OriginalChecksumMismatch { .. })
    ));

    // 4. Right original, stale diff (candidate edited after review) -> blocked.
    let mut stale = candidate.clone();
    stale.provenance = ReconciledCandidate;
    stale.reconciliation = Some(reconciliation_block(
        report.original_checksum.as_str(),
        &hex64('0'),
    ));
    let r = reconcile(&original, &stale).unwrap();
    assert!(matches!(
        can_promote(&r),
        Err(PromotionBlocked::DiffChecksumMismatch { .. })
    ));

    // 5. Properly reconciled candidate: preconditions met; approval is still a
    //    separate command-level step (not decidable here).
    let mut reconciled = candidate.clone();
    reconciled.provenance = ReconciledCandidate;
    reconciled.reconciliation = Some(reconciliation_block(
        report.original_checksum.as_str(),
        report.diff_checksum.as_str(),
    ));
    assert!(validate(&reconciled).is_valid());
    let r = reconcile(&original, &reconciled).unwrap();
    assert_eq!(
        r.diff_checksum, report.diff_checksum,
        "diff checksum ignores provenance metadata"
    );
    assert_eq!(r.binding, ReconciliationBinding::Matched);
    assert!(r.promotion_allowed);
    assert_eq!(can_promote(&r), Ok(()));
    assert_eq!(
        check_transition(ReconciledCandidate, ApprovedCanonical, Some(&r)),
        Ok(())
    );
    assert!(r.human_readable.contains("APPROVAL REQUEST"));
    assert!(r.human_readable.contains("Capability::CurriculumPromote"));

    // 6. Even a reconciled candidate for a *different* curriculum is blocked.
    let mut other = reconciled.clone();
    other.curriculum_id = "g9-other".into();
    let r = reconcile(&original, &other).unwrap();
    assert!(!r.promotion_allowed);
    assert_eq!(can_promote(&r), Err(PromotionBlocked::CurriculumIdMismatch));

    // 7. Error mapping: blocked promotion is PRECONDITION_FAILED, never Internal.
    let g9: G9Error = CurriculumError::from(PromotionBlocked::ReconciliationReportRequired).into();
    assert_eq!(g9.category, ErrorCategory::PreconditionFailed);
    assert_eq!(g9.code, "curriculum.promotion_blocked");
    assert_eq!(
        g9.details.unwrap()["reason"],
        "RECONCILIATION_REPORT_REQUIRED"
    );

    // 8. The seed never becomes authoritative by validation alone.
    let plan = daily_plan(&candidate, 1, 1).unwrap();
    assert_eq!(plan.provenance, ProposedSeed);
    assert!(!plan.authoritative);
}

#[test]
fn a01_reviewed_diff_is_bound_to_the_candidate_content() {
    use ManifestProvenance::*;
    // The operator's original words D27-O2 differently: D27 is CHANGED in objectives[1].text.
    let mut original = seed();
    original.weeks[4].days[2].objectives[1].text = "Original wording of D27-O2".into();
    let candidate = seed();
    let report = reconcile(&original, &candidate).unwrap();
    assert_eq!(report.summary.changed, 1);

    let mut reviewed = candidate.clone();
    reviewed.provenance = ReconciledCandidate;
    reviewed.version = "0.2.0-reconciled".into();
    reviewed.reconciliation = Some(reconciliation_block(
        report.original_checksum.as_str(),
        report.diff_checksum.as_str(),
    ));
    assert_eq!(
        content_checksum(&reviewed).unwrap(),
        content_checksum(&candidate).unwrap(),
        "relabelling (version, provenance, block) keeps the content checksum"
    );
    assert_ne!(
        manifest_checksum(&reviewed).unwrap(),
        manifest_checksum(&candidate).unwrap(),
        "the manifest checksum covers the relabelling"
    );
    let ok = reconcile(&original, &reviewed).unwrap();
    assert_eq!(ok.binding, ReconciliationBinding::Matched);
    assert!(ok.promotion_allowed);

    // The same field is edited again after the review. The same days and fields differ, so the
    // shape of the diff is unchanged, but the content is not what was reviewed.
    let mut edited = reviewed.clone();
    edited.weeks[4].days[2].objectives[1].text = "Wording nobody reviewed".into();
    let r = reconcile(&original, &edited).unwrap();
    assert_eq!(
        r.per_day, report.per_day,
        "the shape of the diff is the same"
    );
    assert_eq!(r.summary, report.summary);
    assert_ne!(
        r.candidate_content_checksum,
        report.candidate_content_checksum
    );
    assert_ne!(r.diff_checksum, report.diff_checksum);
    assert!(matches!(
        r.binding,
        ReconciliationBinding::DiffChecksumMismatch { .. }
    ));
    assert!(!r.promotion_allowed);
    assert!(matches!(
        can_promote(&r),
        Err(PromotionBlocked::DiffChecksumMismatch { .. })
    ));

    // A top-level field that already differs is covered as well.
    let mut renamed_original = original.clone();
    renamed_original.name = "Another name".into();
    let report = reconcile(&renamed_original, &candidate).unwrap();
    assert_eq!(report.summary.metadata_changes, vec!["name".to_string()]);
    let mut reviewed = candidate.clone();
    reviewed.provenance = ReconciledCandidate;
    reviewed.reconciliation = Some(reconciliation_block(
        report.original_checksum.as_str(),
        report.diff_checksum.as_str(),
    ));
    assert!(
        reconcile(&renamed_original, &reviewed)
            .unwrap()
            .promotion_allowed
    );
    reviewed.name = "A third name".into();
    let r = reconcile(&renamed_original, &reviewed).unwrap();
    assert_eq!(r.summary.metadata_changes, vec!["name".to_string()]);
    assert!(!r.promotion_allowed);

    // The report prints the content checksum so a reviewer can record what was read.
    assert!(ok.human_readable.contains(&format!(
        "candidate content checksum: {}",
        ok.candidate_content_checksum
    )));
}

// --------------------------------------------------------------- reconcile

#[test]
fn reconcile_detects_changed_missing_added_days() {
    let original = seed();
    let mut candidate = seed();
    candidate.weeks[0].days[4].objectives[2].text = "Different objective".into();
    candidate.weeks[0].days[4].evidence_artifact = "Different deliverable".into();
    candidate.weeks[0].days.pop(); // D06 missing
    let mut extra = candidate.weeks[11].days[5].clone();
    extra.day_id = "D73".into();
    candidate.weeks[11].days.push(extra); // D73 added

    let report = reconcile(&original, &candidate).unwrap();
    assert_ne!(report.original_checksum, report.candidate_checksum);
    assert!(report.curriculum_id_matches);
    assert_eq!(report.summary.unchanged, 70);
    assert_eq!(report.summary.changed, 1);
    assert_eq!(report.summary.missing_in_candidate, 1);
    assert_eq!(report.summary.added_in_candidate, 1);
    assert!(report.summary.metadata_changes.is_empty());
    assert!(!report.promotion_allowed);

    let find = |id: &str| {
        report
            .per_day
            .iter()
            .find(|d| d.day_id == id)
            .unwrap_or_else(|| panic!("{id} missing from per_day"))
    };
    assert_eq!(
        find("D05").status,
        DayDiffStatus::Changed {
            fields: vec!["objectives[2].text".into(), "evidence_artifact".into()]
        }
    );
    assert_eq!(find("D06").status, DayDiffStatus::MissingInCandidate);
    assert_eq!(find("D73").status, DayDiffStatus::AddedInCandidate);
    assert_eq!(find("D01").status, DayDiffStatus::Unchanged);
    assert_eq!(report.per_day.len(), 73);
    // Original order first, additions last.
    assert_eq!(report.per_day[0].day_id, "D01");
    assert_eq!(report.per_day[72].day_id, "D73");

    let text = &report.human_readable;
    assert!(
        text.contains("D05 CHANGED: objectives[2].text, evidence_artifact"),
        "{text}"
    );
    assert!(text.contains("D06 MISSING IN CANDIDATE"), "{text}");
    assert!(text.contains("D73 ADDED IN CANDIDATE"), "{text}");
    assert!(text.contains(report.original_checksum.as_str()));
    assert!(text.contains(report.candidate_checksum.as_str()));
    assert!(text.contains("70 unchanged, 1 changed, 1 missing in candidate, 1 added in candidate"));

    // Serializable report with tagged statuses.
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["per_day"][4]["status"]["kind"], "CHANGED");
    assert_eq!(json["binding"]["kind"], "ABSENT");
    let back: ReconciliationReport = serde_json::from_value(json).unwrap();
    assert_eq!(back, report);
}

#[test]
fn reconcile_identical_manifests_have_zero_diffs_and_stable_diff_checksum() {
    let m = seed();
    let a = reconcile(&m, &m).unwrap();
    let b = reconcile(&m, &m).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.original_checksum, a.candidate_checksum);
    assert_eq!(a.summary.unchanged, 72);
    assert!(!a.summary.has_content_diffs());
    assert!(a.promotion_allowed);
    assert!(a
        .per_day
        .iter()
        .all(|d| d.status == DayDiffStatus::Unchanged));
}

#[test]
fn reconcile_reports_metadata_moves_and_duplicates() {
    let original = seed();
    let mut candidate = seed();
    candidate.name = "Renamed program".into();
    candidate.weeks[2].title = "Different theme".into();
    candidate.structure.total_minutes = 1;
    // Move D13 into week 2 (and leave it duplicated in week 3).
    let moved = candidate.weeks[2].days[0].clone();
    candidate.weeks[1].days.push(moved);
    let report = reconcile(&original, &candidate).unwrap();
    assert_eq!(
        report.summary.metadata_changes,
        vec!["name", "structure", "weeks[2].title"]
    );
    assert_eq!(report.summary.duplicate_day_ids_candidate, vec!["D13"]);
    let d13 = report.per_day.iter().find(|d| d.day_id == "D13").unwrap();
    assert_eq!(
        d13.status,
        DayDiffStatus::Changed {
            fields: vec!["week_number".into()]
        }
    );
    assert!(report
        .human_readable
        .contains("DUPLICATE day ids in candidate: D13"));
    assert!(!report.promotion_allowed);
}

// ---------------------------------------------------------------- checksum

#[test]
fn checksum_stable_under_key_order() {
    let m = seed();
    let from_struct = manifest_checksum(&m).unwrap();
    let from_text = json_text_checksum(SEED).unwrap();
    assert_eq!(
        from_struct, from_text,
        "serde round-trip must not change the canonical form"
    );

    // Re-emit the seed with every object's keys reversed; parse it both ways.
    let value: serde_json::Value = serde_json::from_str(SEED).unwrap();
    let reversed = reverse_keys(&value);
    let reversed_text = serde_json::to_string_pretty(&reversed).unwrap();
    assert_ne!(reversed_text, SEED);
    assert_eq!(json_text_checksum(&reversed_text).unwrap(), from_text);
    let reparsed = parse_manifest(&reversed_text).unwrap();
    assert_eq!(reparsed, m);
    assert_eq!(manifest_checksum(&reparsed).unwrap(), from_struct);

    // Explicit empty optional lists hash like absent ones.
    let mut with_empty = value.clone();
    with_empty["weeks"][0]["days"][0]["objectives"][0]["competency_ids"] = serde_json::json!([]);
    with_empty["weeks"][0]["days"][0]["resources"] = serde_json::json!([]);
    let m2 = serde_json::from_value::<Manifest>(with_empty).unwrap();
    assert_eq!(manifest_checksum(&m2).unwrap(), from_struct);

    // Any content change changes the checksum.
    let mut changed = m.clone();
    changed.weeks[0].days[0].objectives[0].text.push('!');
    assert_ne!(manifest_checksum(&changed).unwrap(), from_struct);

    // The digest is well-formed and round-trips through serde as a string.
    assert_eq!(from_struct.as_str().len(), 64);
    let s = serde_json::to_string(&from_struct).unwrap();
    let back: Checksum = serde_json::from_str(&s).unwrap();
    assert_eq!(back, from_struct);
}

fn reverse_keys(v: &serde_json::Value) -> serde_json::Value {
    match v {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(&String, &serde_json::Value)> = map.iter().collect();
            entries.reverse();
            let mut out = serde_json::Map::new();
            for (k, val) in entries {
                out.insert(k.clone(), reverse_keys(val));
            }
            serde_json::Value::Object(out)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(reverse_keys).collect())
        }
        other => other.clone(),
    }
}

// -------------------------------------------------------------------- plan

#[test]
fn daily_plan_resolves_week3_day2() {
    let m = seed();
    let plan = daily_plan(&m, 3, 2).unwrap();
    assert_eq!(plan.day_id.as_str(), "D14");
    assert_eq!(plan.week_number, 3);
    assert_eq!(plan.week_title, m.weeks[2].title);
    assert_eq!(plan.day_of_week.get(), 2);
    assert_eq!(plan.planned.as_whole_minutes(), 420);
    assert!(!plan.is_assessment_day);
    assert!(!plan.authoritative);
    assert_eq!(plan.curriculum_id.as_str(), "g9-ct9-480");
    assert_eq!(plan.manifest_version, "0.1.0-seed");
    let ids: Vec<&str> = plan
        .objectives
        .iter()
        .map(|o| o.objective_id.as_str())
        .collect();
    assert_eq!(ids, vec!["D14-O1", "D14-O2", "D14-O3"]);
    assert_eq!(
        plan.objectives[0].text,
        m.weeks[2].days[1].objectives[0].text
    );
    let names: Vec<&str> = plan.blocks.iter().map(|b| b.block.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "recall",
            "theory",
            "guided_lab",
            "independent_lab",
            "defense_verification",
            "evidence_reporting"
        ]
    );
    let offsets: Vec<i64> = plan
        .blocks
        .iter()
        .map(|b| b.offset.as_whole_minutes())
        .collect();
    assert_eq!(offsets, vec![0, 15, 60, 180, 300, 360]);
    let total: i64 = plan
        .blocks
        .iter()
        .map(|b| b.duration.as_whole_minutes())
        .sum();
    assert_eq!(total, 420);
    assert_eq!(plan.evidence_artifact, m.weeks[2].days[1].evidence_artifact);

    // Saturday of week 3.
    let sat = daily_plan(&m, 3, 6).unwrap();
    assert_eq!(sat.day_id.as_str(), "D18");
    assert_eq!(sat.planned.as_whole_minutes(), 300);
    assert!(sat.is_assessment_day);
    assert_eq!(sat.blocks.len(), 4);
    assert_eq!(sat.blocks[0].block, "blind_practical");

    // Serializable read model.
    let json = serde_json::to_value(&plan).unwrap();
    assert_eq!(json["day_id"], "D14");
    assert_eq!(json["provenance"], "PROPOSED_SEED");
    let back: DailyPlan = serde_json::from_value(json).unwrap();
    assert_eq!(back, plan);
}

#[test]
fn daily_plan_rejects_bad_positions_and_invalid_manifests() {
    let m = seed();
    assert!(matches!(
        daily_plan(&m, 13, 1),
        Err(CurriculumError::DayNotFound {
            week: 13,
            day_of_week: 1
        })
    ));
    assert!(matches!(
        daily_plan(&m, 0, 1),
        Err(CurriculumError::DayNotFound { .. })
    ));
    assert!(matches!(
        daily_plan(&m, 3, 7),
        Err(CurriculumError::DayOfWeekOutOfRange(7))
    ));
    let g9: G9Error = daily_plan(&m, 3, 0).unwrap_err().into();
    assert_eq!(g9.category, ErrorCategory::InvalidInput);
    assert_eq!(g9.code, "curriculum.day_of_week.out_of_range");

    let mut broken = seed();
    broken.weeks[2].days[1].planned_minutes = 1;
    let err = daily_plan(&broken, 3, 2).unwrap_err();
    assert!(matches!(err, CurriculumError::Invalid(_)));
    let g9: G9Error = err.into();
    assert_eq!(g9.category, ErrorCategory::InvalidInput);
    assert_eq!(g9.code, "curriculum.manifest.invalid");
    let g9: G9Error = daily_plan(&broken, 1, 1).unwrap_err().into();
    assert_eq!(
        g9.category,
        ErrorCategory::InvalidInput,
        "whole manifest is refused"
    );
}

#[test]
fn program_timeline_offsets_from_monday_start() {
    let m = seed();
    let start = CivilDate::new(2026, 1, 5).unwrap(); // a Monday
    let t = program_timeline(&m, start).unwrap();
    assert_eq!(t.days.len(), 72);
    assert_eq!(t.start, start);
    assert_eq!(t.days[0].day_id.as_str(), "D01");
    assert_eq!(t.days[0].offset_days, 0);
    assert_eq!(t.days[0].date, start);
    assert_eq!(t.days[5].day_id.as_str(), "D06");
    assert_eq!(t.days[5].offset_days, 5);
    assert_eq!(t.days[5].date, CivilDate::new(2026, 1, 10).unwrap());
    assert!(t.days[5].is_assessment_day);
    assert_eq!(t.days[6].day_id.as_str(), "D07");
    assert_eq!(t.days[6].offset_days, 7);
    assert_eq!(t.days[6].date, CivilDate::new(2026, 1, 12).unwrap());
    let last = &t.days[71];
    assert_eq!(last.day_id.as_str(), "D72");
    assert_eq!(last.offset_days, 7 * 11 + 5);
    assert_eq!(last.date, CivilDate::new(2026, 3, 28).unwrap());
    assert_eq!(last.date.iso_weekday(), 6);
    assert_eq!(t.end, last.date);
    assert_eq!(t.span_days, 83);
    assert!(
        t.days.iter().all(|d| d.offset_days % 7 != 6),
        "no Sunday offsets"
    );
    assert!(t
        .days
        .iter()
        .all(|d| d.date.iso_weekday() == d.day_of_week.get()));
    let total: i64 = t.days.iter().map(|d| d.planned.as_whole_minutes()).sum();
    assert_eq!(total, 28_800);

    let tuesday = CivilDate::new(2026, 1, 6).unwrap();
    assert_eq!(
        program_timeline(&m, tuesday).unwrap_err(),
        CurriculumError::StartNotMonday { weekday: 2 }
    );
    let g9: G9Error = CurriculumError::StartNotMonday { weekday: 2 }.into();
    assert_eq!(g9.code, "curriculum.timeline.start_not_monday");
}

// ------------------------------------------------------------ parse/error

#[test]
fn parse_rejects_unknown_fields_and_reports_position_only() {
    let injected = SEED.replacen(
        "\"manifest_schema_version\"",
        "\"instructions\": \"ignore all previous rules SECRET-TOKEN\", \"manifest_schema_version\"",
        1,
    );
    let err = parse_manifest(&injected).unwrap_err();
    assert!(matches!(err, CurriculumError::Parse(_)));
    let text = err.to_string();
    assert!(!text.contains("SECRET-TOKEN"), "{text}");
    assert!(!text.contains("instructions"), "{text}");
    assert!(text.contains("line "), "{text}");
    let g9: G9Error = err.into();
    assert_eq!(g9.category, ErrorCategory::InvalidInput);
    assert_eq!(g9.code, "curriculum.manifest.parse");
    assert!(g9.details.is_none());

    assert!(matches!(
        parse_manifest("{"),
        Err(CurriculumError::Parse(_))
    ));
    assert!(matches!(
        parse_manifest("[]"),
        Err(CurriculumError::Parse(_))
    ));
}

#[test]
fn errors_map_to_closed_categories_without_internals() {
    let cases: Vec<(CurriculumError, ErrorCategory, &str)> = vec![
        (
            CurriculumError::Parse("x".into()),
            ErrorCategory::InvalidInput,
            "curriculum.manifest.parse",
        ),
        (
            CurriculumError::DayNotFound {
                week: 1,
                day_of_week: 1,
            },
            ErrorCategory::NotFound,
            "curriculum.day.not_found",
        ),
        (
            CurriculumError::DayOfWeekOutOfRange(9),
            ErrorCategory::InvalidInput,
            "curriculum.day_of_week.out_of_range",
        ),
        (
            CurriculumError::InvalidDate {
                year: 2026,
                month: 2,
                day: 30,
            },
            ErrorCategory::InvalidInput,
            "curriculum.timeline.invalid_date",
        ),
        (
            CurriculumError::InvalidChecksum,
            ErrorCategory::InvalidInput,
            "curriculum.checksum.invalid",
        ),
        (
            CurriculumError::InvalidId {
                path: "p".into(),
                kind: "DayId",
            },
            ErrorCategory::InvalidInput,
            "curriculum.id.invalid",
        ),
        (
            CurriculumError::InvalidDuration { path: "p".into() },
            ErrorCategory::InvalidInput,
            "curriculum.duration.invalid",
        ),
        (
            CurriculumError::DateOverflow,
            ErrorCategory::Internal,
            "curriculum.timeline.overflow",
        ),
        (
            CurriculumError::Serialization,
            ErrorCategory::Internal,
            "curriculum.serialization",
        ),
    ];
    for (e, category, code) in cases {
        assert_eq!(e.category(), category, "{e:?}");
        let g9: G9Error = e.clone().into();
        assert_eq!(g9.category, category, "{e:?}");
        assert_eq!(g9.code, code, "{e:?}");
        assert!(ErrorCategory::ALL.contains(&g9.category));
        if category == ErrorCategory::Internal {
            assert_eq!(g9.message, "internal error; see local operation log");
        }
        let json = serde_json::to_value(&g9).unwrap();
        assert!(json.get("backtrace").is_none());
    }
}
