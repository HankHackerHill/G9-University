#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Exam integrity (gate A07) and the exposure ledger / blind eligibility /
//! retake variants (ARCHITECTURE.md Addendum A item 3; LRN-13).

mod common;

use common::*;
use g9_assessment::{
    blind_eligible, check_retake_item, ensure_no_holdout_leakage, holdout_leakage_check,
    AssessmentVersion, ExamContextPolicy, ExamItem, ExamItemStatus, ExposureEntry, ExposureError,
    ExposureKind, ExposureLedger, ExposureType, HintPolicy, Ineligibility, IntegrityError,
    SIBLING_VARIANT_COOLDOWN_DAYS,
};
use g9_domain::{
    ActorId, AssistanceLevel, Classification, ErrorCategory, G9Error, InteractionMode,
};
use pretty_assertions::assert_eq;

// ---------------------------------------------------------------------------
// A07: exam context and holdout leakage
// ---------------------------------------------------------------------------

#[test]
fn a07_exam_context_policy_blocks_answer_key_and_secret() {
    use Classification as C;
    use ExposureKind as K;
    use InteractionMode as M;
    // answer keys never, in any mode or classification
    for mode in M::ALL {
        for class in [C::Public, C::Internal, C::Private, C::Secret] {
            assert!(
                !ExamContextPolicy::allowed(mode, class, K::AnswerKey),
                "{mode:?} {class:?}"
            );
        }
        // sealed keys are Secret: never in model context, whatever the kind
        for kind in ExposureKind::ALL {
            assert!(
                !ExamContextPolicy::allowed(
                    mode,
                    AssessmentVersion::SEALED_KEY_CLASSIFICATION,
                    kind
                ),
                "{mode:?} {kind:?}"
            );
        }
    }
    // EXAM mode: only the task, never hints or reference solutions
    assert!(ExamContextPolicy::allowed(M::Exam, C::Internal, K::Task));
    assert!(!ExamContextPolicy::allowed(M::Exam, C::Internal, K::Hint));
    assert!(!ExamContextPolicy::allowed(
        M::Exam,
        C::Internal,
        K::ReferenceSolution
    ));
    // teaching modes may use hints/reference material that is not private
    assert!(ExamContextPolicy::allowed(M::Study, C::Internal, K::Hint));
    assert!(!ExamContextPolicy::allowed(M::Study, C::Private, K::Hint));
    let err = ExamContextPolicy::check(M::Exam, C::Secret, K::AnswerKey).unwrap_err();
    assert!(matches!(err, IntegrityError::ContextNotAllowed { .. }));
    let g = G9Error::from(err);
    assert_eq!(g.category, ErrorCategory::Unauthorized);
    assert_eq!(g.code, "assessment.exam.context_not_allowed");

    // hints: none in EXAM
    assert!(!HintPolicy::hints_allowed(M::Exam));
    assert_eq!(
        HintPolicy::max_assistance(M::Exam),
        AssistanceLevel::Unaided
    );
    assert!(HintPolicy::check(M::Exam, AssistanceLevel::Prompted).is_err());
    assert!(HintPolicy::check(M::Exam, AssistanceLevel::Unaided).is_ok());
    assert!(HintPolicy::check(M::Study, AssistanceLevel::Shown).is_ok());
    assert!(HintPolicy::check(M::Review, AssistanceLevel::Hinted).is_err());
}

#[test]
fn a07_holdout_leakage_detected() {
    let exam = vec!["AA11".to_string(), "bb22".to_string(), " ".to_string()];
    let context = vec![
        "cc33".to_string(),
        " aa11 ".to_string(),
        "BB22".to_string(),
        "bb22".to_string(),
        String::new(),
    ];
    assert_eq!(
        holdout_leakage_check(&exam, &context),
        vec!["aa11".to_string(), "bb22".to_string()]
    );
    assert!(holdout_leakage_check(&exam, &["dd44".to_string()]).is_empty());
    assert!(holdout_leakage_check(&[], &context).is_empty());
    let err = ensure_no_holdout_leakage(&exam, &context).unwrap_err();
    let g = G9Error::from(err);
    assert_eq!(g.category, ErrorCategory::Unauthorized);
    assert_eq!(
        g.details.unwrap()["hashes"],
        serde_json::json!(["aa11", "bb22"])
    );
    assert!(ensure_no_holdout_leakage(&exam, &["dd44".to_string()]).is_ok());
}

// ---------------------------------------------------------------------------
// LRN-13: exposure ledger and blind eligibility
// ---------------------------------------------------------------------------

fn item(version: u32, family: &str, variant: &str, status: ExamItemStatus) -> ExamItem {
    let def = definition();
    ExamItem::for_version(
        &version_n(&def, version, 3),
        family,
        variant,
        "same fixture size and fault depth as the week-1 calibration item",
        status,
    )
    .unwrap()
}

fn sealed(version: u32, family: &str, variant: &str) -> ExamItem {
    item(version, family, variant, ExamItemStatus::Sealed)
}

fn me() -> ActorId {
    learner().id
}

#[test]
fn exposure_ledger_kinds_are_enumerated() {
    let names: Vec<serde_json::Value> = ExposureType::ALL
        .iter()
        .map(|t| serde_json::to_value(t).unwrap())
        .collect();
    assert_eq!(
        names,
        vec![
            serde_json::json!("DISPLAYED"),
            serde_json::json!("AI_CONTEXT"),
            serde_json::json!("PUBLIC_SOURCE_DECLARED")
        ]
    );
    assert_eq!(
        ExposureType::for_audience(&learner()),
        Some(ExposureType::Displayed)
    );
    assert_eq!(
        ExposureType::for_audience(&model()),
        Some(ExposureType::AiContext)
    );
    assert_eq!(
        ExposureType::for_audience(&skill()),
        Some(ExposureType::AiContext)
    );
    assert_eq!(ExposureType::for_audience(&system()), None);
}

#[test]
fn blind_eligible_requires_sealed_and_unexposed_variant() {
    let a = sealed(1, "boot-chain", "v1");
    let mut ledger = ExposureLedger::new();
    assert!(blind_eligible(&a, &ledger, &me(), day(0)));

    // status must be SEALED
    for status in [
        ExamItemStatus::Draft,
        ExamItemStatus::Reviewed,
        ExamItemStatus::Exposed,
        ExamItemStatus::Retired,
    ] {
        let e = ledger.blind_eligibility(&item(1, "boot-chain", "v1", status), &me(), day(0));
        assert_eq!(e.reasons, vec![Ineligibility::NotSealed { status }]);
    }

    // DISPLAYED and AI_CONTEXT make the variant permanently ineligible
    for (exposure, n) in [(ExposureType::Displayed, 1), (ExposureType::AiContext, 2)] {
        ledger
            .record_item(&a, &me(), exposure, InteractionMode::Exam, &me(), day(1))
            .unwrap();
        let e = ledger.blind_eligibility(&a, &me(), day(400));
        assert!(!e.is_eligible());
        assert_eq!(e.reasons.len(), n);
        assert!(matches!(
            e.reasons[n - 1],
            Ineligibility::VariantExposed { exposure: x, .. } if x == exposure
        ));
    }
    // another learner's exposure does not count
    let other = ActorId::parse("operator-2").unwrap();
    assert!(blind_eligible(&a, &ledger, &other, day(2)));
    // the ledger is append-only and keeps every entry in order
    assert_eq!(ledger.entries().len(), 2);
    assert_eq!(ledger.entries()[0].exposure, ExposureType::Displayed);
}

#[test]
fn public_source_declared_blocks_the_whole_family() {
    let v1 = sealed(1, "boot-chain", "v1");
    let v2 = sealed(2, "boot-chain", "v2");
    let elsewhere = sealed(3, "dns-path", "v1");
    let mut ledger = ExposureLedger::new();
    ledger
        .record(ExposureEntry {
            learner: me(),
            fixture_family: "boot-chain".into(),
            variant: None,
            exposure: ExposureType::PublicSourceDeclared,
            mode: InteractionMode::Study,
            recorded_by: me(),
            at: day(0),
        })
        .unwrap();
    for i in [&v1, &v2] {
        let e = ledger.blind_eligibility(i, &me(), day(365));
        assert_eq!(
            e.reasons,
            vec![Ineligibility::FamilyDeclaredFamiliar { at: day(0) }]
        );
    }
    assert!(blind_eligible(&elsewhere, &ledger, &me(), day(1)));
}

#[test]
fn sibling_variant_exposure_has_a_28_day_cooldown() {
    let v1 = sealed(1, "boot-chain", "v1");
    let v2 = sealed(2, "boot-chain", "v2");
    let mut ledger = ExposureLedger::new();
    ledger
        .record_item(
            &v1,
            &me(),
            ExposureType::Displayed,
            InteractionMode::Exam,
            &me(),
            day(10),
        )
        .unwrap();
    assert_eq!(SIBLING_VARIANT_COOLDOWN_DAYS, 28);
    let e = ledger.blind_eligibility(&v2, &me(), day(37));
    assert_eq!(
        e.reasons,
        vec![Ineligibility::SiblingVariantExposed {
            variant: "v1".into(),
            at: day(10)
        }]
    );
    // exactly 28 days later the sibling is eligible again
    assert!(blind_eligible(&v2, &ledger, &me(), day(38)));
    // an entry dated after `now` is treated as inside the window
    assert!(!blind_eligible(&v2, &ledger, &me(), day(5)));
}

#[test]
fn retake_must_use_a_different_blind_eligible_variant_of_equal_difficulty() {
    let original = sealed(1, "boot-chain", "v1");
    let sibling = sealed(2, "boot-chain", "v2");
    let other_family = sealed(3, "boot-loader", "v1");
    let mut ledger = ExposureLedger::new();
    ledger
        .record_item(
            &original,
            &me(),
            ExposureType::Displayed,
            InteractionMode::Exam,
            &me(),
            day(0),
        )
        .unwrap();

    // same variant: refused
    assert_eq!(
        check_retake_item(&original, &original, &ledger, &me(), day(60)).unwrap_err(),
        ExposureError::SameVariant
    );
    // same version under another code is still the same variant
    let renamed = sealed(1, "boot-chain-copy", "v9");
    assert_eq!(
        check_retake_item(&original, &renamed, &ledger, &me(), day(60)).unwrap_err(),
        ExposureError::SameVariant
    );
    // sibling within 28 days: not blind-eligible
    let err = check_retake_item(&original, &sibling, &ledger, &me(), day(2)).unwrap_err();
    assert!(matches!(err, ExposureError::NotBlindEligible { .. }));
    let g = G9Error::from(err);
    assert_eq!(g.category, ErrorCategory::PreconditionFailed);
    assert_eq!(
        g.details.unwrap()["reasons"][0]["reason"],
        "SIBLING_VARIANT_EXPOSED"
    );
    // sibling after the cooldown, or another family at once: allowed
    assert!(check_retake_item(&original, &sibling, &ledger, &me(), day(28)).is_ok());
    assert!(check_retake_item(&original, &other_family, &ledger, &me(), day(2)).is_ok());
    // different declared difficulty: refused
    let def = definition();
    let harder = ExamItem::for_version(
        &version_n(&def, 4, 5),
        "boot-loader",
        "v2",
        "two faults instead of one",
        ExamItemStatus::Sealed,
    )
    .unwrap();
    assert_eq!(
        check_retake_item(&original, &harder, &ledger, &me(), day(60)).unwrap_err(),
        ExposureError::DifficultyMismatch {
            original: 3,
            candidate: 5
        }
    );
}

#[test]
fn ledger_records_attempt_exposures_by_audience() {
    let i = sealed(1, "boot-chain", "v1");
    let (_, _, mut a) = available("attempt-1");
    a.record_exposure(ExposureKind::Task, &learner(), t(0))
        .unwrap();
    a.begin(&learner(), t(1)).unwrap();
    a.record_exposure(ExposureKind::Task, &model(), t(2))
        .unwrap();
    a.record_exposure(ExposureKind::AnswerKey, &system(), t(3))
        .unwrap();
    let mut ledger = ExposureLedger::new();
    assert_eq!(ledger.record_attempt_exposures(&i, &a).unwrap(), 2);
    let kinds: Vec<ExposureType> = ledger.entries().iter().map(|e| e.exposure).collect();
    assert_eq!(
        kinds,
        vec![ExposureType::Displayed, ExposureType::AiContext]
    );
    assert!(ledger.entries().iter().all(|e| e.learner == me()
        && e.variant.as_deref() == Some("v1")
        && e.mode == InteractionMode::Exam));
    assert!(!blind_eligible(&i, &ledger, &me(), t(10)));
}

#[test]
fn ledger_and_items_validate_their_codes() {
    let mut ledger = ExposureLedger::new();
    let bad = ExposureEntry {
        learner: me(),
        fixture_family: "boot-chain".into(),
        variant: None,
        exposure: ExposureType::Displayed,
        mode: InteractionMode::Exam,
        recorded_by: me(),
        at: day(0),
    };
    assert_eq!(
        ledger.record(bad.clone()).unwrap_err(),
        ExposureError::MissingVariant(ExposureType::Displayed)
    );
    assert!(ledger.entries().is_empty());
    let mut spaced = bad.clone();
    spaced.variant = Some("v 1".into());
    assert!(matches!(
        ExposureLedger::from_entries(vec![spaced]),
        Err(ExposureError::InvalidCode { field: "variant" })
    ));
    let def = definition();
    let v = version_n(&def, 1, 3);
    assert!(matches!(
        ExamItem::for_version(&v, "", "v1", "r", ExamItemStatus::Sealed),
        Err(ExposureError::InvalidCode {
            field: "fixture_family"
        })
    ));
    assert_eq!(
        ExamItem::for_version(&v, "f", "v1", " ", ExamItemStatus::Sealed).unwrap_err(),
        ExposureError::EmptyDifficultyRationale
    );
    assert_eq!(
        G9Error::from(ExposureError::EmptyDifficultyRationale).category,
        ErrorCategory::InvalidInput
    );
    // ledger serde round trip
    let mut ok = ExposureLedger::new();
    ok.record_item(
        &sealed(1, "f", "v1"),
        &me(),
        ExposureType::AiContext,
        InteractionMode::Lab,
        &model().id,
        day(1),
    )
    .unwrap();
    let json = serde_json::to_value(&ok).unwrap();
    let back: ExposureLedger = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(back, ok);
    // a stored entry that breaks the variant rule cannot be loaded
    let mut broken = json;
    broken[0]["variant"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<ExposureLedger>(broken).is_err());
}
