#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Attempt lifecycle, retakes, assistance and exposure (ARCHITECTURE.md §8
//! "Attempt lifecycle"; handoff §16; gate A04; LRN-13 retake variants).

mod common;

use common::*;
use g9_assessment::{
    finalize, AssessmentDefinition, AssessmentKind, AssessmentVersion, Attempt, AttemptError,
    DefinitionError, ExposureKind,
};
use g9_domain::{
    AssessmentAttemptState as S, AssessmentId, AssistanceLevel, DataDomain, ErrorCategory, G9Error,
    InteractionMode,
};
use pretty_assertions::assert_eq;

#[test]
fn a04_finalized_attempt_rejects_every_mutation() {
    let (_, ver, mut a) = graded("attempt-1");
    let decision = finalize(
        decision_id("gd-1"),
        &a,
        &ver.rubric,
        &self_scored(&a, [100; 6]),
        &learner(),
        t(200),
    )
    .unwrap();
    assert!(decision.is_final);
    a.approve_final(&learner(), t(201)).unwrap();
    assert_eq!(a.state(), S::ApprovedFinal);
    assert!(a.is_finalized());
    let frozen = a.clone();

    let attempts: Vec<(&str, Result<(), AttemptError>)> = vec![
        ("begin", a.begin(&learner(), t(300))),
        ("submit", a.submit(&learner(), t(300))),
        ("review_evidence", a.review_evidence(&learner(), t(300))),
        ("grade_provisional", a.grade_provisional(&learner(), t(300))),
        ("approve_final twice", a.approve_final(&learner(), t(300))),
        ("void", a.void(&learner(), t(300))),
        ("abandon", a.abandon(&learner(), t(300))),
        (
            "record_assistance",
            a.record_assistance(AssistanceLevel::Shown, t(300))
                .map(|_| ()),
        ),
        (
            "record_exposure",
            a.record_exposure(ExposureKind::AnswerKey, &learner(), t(300)),
        ),
    ];
    for (op, result) in attempts {
        let err = result.unwrap_err();
        assert_eq!(
            err,
            AttemptError::Finalized {
                state: S::ApprovedFinal
            },
            "{op}"
        );
        assert_eq!(
            G9Error::from(err).category,
            ErrorCategory::PreconditionFailed
        );
    }
    // nothing changed: no revision bump, no timestamps, no exposure
    assert_eq!(a, frozen);

    // a voided attempt is terminal too
    let (_, _, mut v) = available("attempt-2");
    v.begin(&learner(), t(1)).unwrap();
    v.abandon(&learner(), t(2)).unwrap();
    assert_eq!(v.state(), S::Voided);
    assert!(matches!(
        v.submit(&learner(), t(3)),
        Err(AttemptError::Finalized { state: S::Voided })
    ));
}

#[test]
fn a04_lifecycle_follows_the_machine_and_bumps_revision() {
    let (_, _, mut a) = available("attempt-1");
    assert_eq!(a.state(), S::Available);
    let r0 = a.revision();
    // illegal edge refused without change
    let err = a.submit(&learner(), t(1)).unwrap_err();
    assert!(matches!(err, AttemptError::Transition(_)));
    assert_eq!(a.revision(), r0);
    assert_eq!(
        G9Error::from(err).category,
        ErrorCategory::PreconditionFailed
    );

    a.begin(&learner(), t(1)).unwrap();
    assert_eq!(a.started_at(), Some(t(1)));
    a.submit(&learner(), t(90)).unwrap();
    assert_eq!(a.submitted_at(), Some(t(90)));
    // approve_final directly from Submitted is not an edge
    assert!(matches!(
        a.approve_final(&learner(), t(91)),
        Err(AttemptError::Transition(_))
    ));
    a.review_evidence(&system(), t(91)).unwrap();
    a.grade_provisional(&system(), t(92)).unwrap();
    // back to evidence review and forward again
    a.review_evidence(&learner(), t(93)).unwrap();
    a.grade_provisional(&learner(), t(94)).unwrap();
    assert_eq!(a.state(), S::GradedProvisional);
    assert!(a.revision() > r0);
    assert_eq!(a.updated_at(), t(94));
}

#[test]
fn a04_model_skill_and_system_actors_are_restricted() {
    let (_, _, mut a) = available("attempt-1");
    for actor in [model(), skill()] {
        let err = a.begin(&actor, t(1)).unwrap_err();
        assert!(matches!(err, AttemptError::ActorNotPermitted { .. }));
        assert_eq!(G9Error::from(err).category, ErrorCategory::Unauthorized);
    }
    // another operator is not the learner
    assert!(matches!(
        a.begin(&mentor(), t(1)),
        Err(AttemptError::NotLearner { operation: "begin" })
    ));
    // system may not begin or submit
    assert!(matches!(
        a.begin(&system(), t(1)),
        Err(AttemptError::ActorNotPermitted { .. })
    ));
    a.begin(&learner(), t(1)).unwrap();
    a.submit(&learner(), t(2)).unwrap();
    for actor in [model(), skill()] {
        assert!(matches!(
            a.review_evidence(&actor, t(3)),
            Err(AttemptError::ActorNotPermitted { .. })
        ));
    }
    a.review_evidence(&learner(), t(3)).unwrap();
    a.grade_provisional(&learner(), t(4)).unwrap();
    // approve_final: operator only (system, model and skill refused)
    for actor in [system(), model(), skill()] {
        assert!(matches!(
            a.approve_final(&actor, t(5)),
            Err(AttemptError::ActorNotPermitted {
                operation: "approve_final",
                ..
            })
        ));
    }
    assert_eq!(a.state(), S::GradedProvisional);
}

#[test]
fn a04_abandon_by_learner_or_system_only() {
    let (_, _, mut a) = available("attempt-1");
    a.begin(&learner(), t(1)).unwrap();
    assert!(matches!(
        a.abandon(&mentor(), t(2)),
        Err(AttemptError::NotLearner { .. })
    ));
    assert!(a.abandon(&model(), t(2)).is_err());
    a.abandon(&system(), t(2)).unwrap();
    assert_eq!(a.state(), S::Voided);
}

#[test]
fn a04_retake_is_new_linked_attempt() {
    let def = definition();
    let v1 = version(&def);
    let v2 = version_n(&def, 2, 3);
    let (_, _, mut original) = graded("attempt-1");

    // an open original cannot be retaken
    assert_eq!(
        Attempt::new_retake(attempt_id("attempt-2"), &original, &v2, t(300)).unwrap_err(),
        AttemptError::RetakeOfOpenAttempt
    );
    finalize(
        decision_id("gd-1"),
        &original,
        &v1.rubric,
        &self_scored(&original, [40; 6]),
        &learner(),
        t(200),
    )
    .unwrap();
    original.approve_final(&learner(), t(201)).unwrap();
    let before = original.clone();

    // LRN-13: the same variant (version) is refused ...
    let err = Attempt::new_retake(attempt_id("attempt-2"), &original, &v1, t(300)).unwrap_err();
    assert_eq!(err, AttemptError::RetakeSameVariant { version: 1 });
    assert_eq!(
        G9Error::from(err).category,
        ErrorCategory::PreconditionFailed
    );
    // ... so is a variant of a different declared difficulty
    let harder = version_n(&def, 3, 4);
    assert_eq!(
        Attempt::new_retake(attempt_id("attempt-2"), &original, &harder, t(300)).unwrap_err(),
        AttemptError::RetakeDifficultyMismatch
    );
    // ... and a version of another definition
    let other_def = AssessmentDefinition::new(
        AssessmentId::parse("ad-other").unwrap(),
        "Other",
        vec![comp("comp-boot")],
        AssessmentKind::BlindPractical,
        DataDomain::Learning,
    )
    .unwrap();
    assert_eq!(
        Attempt::new_retake(
            attempt_id("attempt-2"),
            &original,
            &version_n(&other_def, 2, 3),
            t(300)
        )
        .unwrap_err(),
        AttemptError::VersionMismatch
    );

    let retake = Attempt::new_retake(attempt_id("attempt-2"), &original, &v2, t(300)).unwrap();
    assert_eq!(retake.retake_of(), Some(original.id()));
    assert_ne!(retake.id(), original.id());
    assert_eq!(retake.state(), S::Available);
    assert_eq!(retake.version(), 2);
    assert_eq!(retake.learner(), original.learner());
    assert_eq!(retake.mode(), InteractionMode::Exam);
    assert_eq!(retake.assistance(), AssistanceLevel::Unaided);
    assert!(retake.exposure().is_empty());
    assert_eq!(retake.competency_ids(), original.competency_ids());
    // the original is untouched
    assert_eq!(original, before);

    // a voided original may also be retaken
    let (_, _, mut voided) = available("attempt-3");
    voided.begin(&learner(), t(1)).unwrap();
    voided.abandon(&learner(), t(2)).unwrap();
    assert!(Attempt::new_retake(attempt_id("attempt-4"), &voided, &v2, t(3)).is_ok());
}

#[test]
fn assistance_recorded_max_never_erased() {
    let (_, _, mut a) = available("attempt-1");
    // only while Started
    assert!(matches!(
        a.record_assistance(AssistanceLevel::Hinted, t(0)),
        Err(AttemptError::InvalidState { .. })
    ));
    a.begin(&learner(), t(1)).unwrap();
    assert!(a.record_assistance(AssistanceLevel::Hinted, t(2)).unwrap());
    assert_eq!(a.assistance(), AssistanceLevel::Hinted);
    let rev = a.revision();
    // a lower level never lowers the record and does not bump the revision
    assert!(!a
        .record_assistance(AssistanceLevel::Prompted, t(3))
        .unwrap());
    assert!(!a.record_assistance(AssistanceLevel::Unaided, t(3)).unwrap());
    assert_eq!(a.assistance(), AssistanceLevel::Hinted);
    assert_eq!(a.revision(), rev);
    assert!(a.record_assistance(AssistanceLevel::Shown, t(4)).unwrap());
    assert_eq!(a.assistance(), AssistanceLevel::Shown);
    a.submit(&learner(), t(5)).unwrap();
    // after submission the level is frozen
    assert!(matches!(
        a.record_assistance(AssistanceLevel::Unaided, t(6)),
        Err(AttemptError::InvalidState { .. })
    ));
    assert_eq!(a.assistance(), AssistanceLevel::Shown);
    assert!(!a.is_blind());
    // serde round trip keeps the level
    let back: Attempt = serde_json::from_value(serde_json::to_value(&a).unwrap()).unwrap();
    assert_eq!(back.assistance(), AssistanceLevel::Shown);
}

#[test]
fn exposure_log_appended() {
    let (_, _, mut a) = available("attempt-1");
    a.record_exposure(ExposureKind::Task, &learner(), t(0))
        .unwrap();
    a.begin(&learner(), t(1)).unwrap();
    assert!(a.is_blind(), "task exposure alone keeps the attempt blind");
    a.record_exposure(ExposureKind::Task, &model(), t(2))
        .unwrap();
    a.submit(&learner(), t(50)).unwrap();
    assert!(a.is_blind());
    // a reviewer opening the reference solution after submission does not unblind
    a.review_evidence(&learner(), t(51)).unwrap();
    a.record_exposure(ExposureKind::ReferenceSolution, &learner(), t(52))
        .unwrap();
    assert!(a.is_blind());
    let log: Vec<(ExposureKind, i64)> = a
        .exposure()
        .iter()
        .map(|e| (e.what, e.at.as_millis()))
        .collect();
    assert_eq!(
        log,
        vec![
            (ExposureKind::Task, t(0).as_millis()),
            (ExposureKind::Task, t(2).as_millis()),
            (ExposureKind::ReferenceSolution, t(52).as_millis()),
        ]
    );
    assert_eq!(a.exposure()[1].actor, model());

    // an answer key seen before submission unblinds the attempt
    let (_, _, mut leaked) = available("attempt-2");
    leaked.begin(&learner(), t(1)).unwrap();
    leaked
        .record_exposure(ExposureKind::AnswerKey, &learner(), t(2))
        .unwrap();
    leaked.submit(&learner(), t(3)).unwrap();
    assert!(!leaked.is_blind());
    for kind in ExposureKind::ALL {
        assert_eq!(kind.compromises_blindness(), kind != ExposureKind::Task);
    }
}

#[test]
fn official_kinds_require_exam_mode_and_valid_versions() {
    let def = definition();
    let ver = version(&def);
    assert_eq!(
        Attempt::new(
            attempt_id("attempt-1"),
            &def,
            &ver,
            learner().id,
            InteractionMode::Study,
            t(0)
        )
        .unwrap_err(),
        AttemptError::OfficialRequiresExamMode
    );
    let diag = AssessmentDefinition::new(
        AssessmentId::parse("ad-diag").unwrap(),
        "Diagnostic",
        vec![comp("comp-boot")],
        AssessmentKind::DiagnosticCalibration,
        DataDomain::Learning,
    )
    .unwrap();
    let diag_ver = AssessmentVersion {
        definition_id: diag.id.clone(),
        ..ver.clone()
    }
    .with_checksum()
    .unwrap();
    assert!(Attempt::new(
        attempt_id("attempt-2"),
        &diag,
        &diag_ver,
        learner().id,
        InteractionMode::Study,
        t(0)
    )
    .is_ok());
    // a version of another definition is refused
    assert_eq!(
        Attempt::new(
            attempt_id("attempt-3"),
            &diag,
            &ver,
            learner().id,
            InteractionMode::Exam,
            t(0)
        )
        .unwrap_err(),
        AttemptError::VersionMismatch
    );
    // a tampered version is refused (checksum covers rubric anchors too)
    let mut tampered = ver.clone();
    tampered.equivalence_group = "other".into();
    let err = Attempt::new(
        attempt_id("attempt-4"),
        &def,
        &tampered,
        learner().id,
        InteractionMode::Exam,
        t(0),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        AttemptError::Definition(DefinitionError::ChecksumMismatch { .. })
    ));
    assert_eq!(G9Error::from(err).category, ErrorCategory::DataCorruption);
}

#[test]
fn version_checksum_pins_rubric_anchors() {
    let def = definition();
    let ver = version(&def);
    assert!(ver.verify_checksum().is_ok());
    let mut anchors = ver.rubric.anchors().clone();
    anchors.criteria[1].descriptors[3] = "Names the faulty unit and the failing dependency.".into();
    let mut edited = ver.clone();
    edited.rubric =
        g9_assessment::Rubric::with_anchors(ver.rubric.criteria().to_vec(), anchors).unwrap();
    assert!(matches!(
        edited.verify_checksum(),
        Err(DefinitionError::ChecksumMismatch { .. })
    ));
    assert!(edited.with_checksum().unwrap().verify_checksum().is_ok());
    // the anchors are part of the stored version
    let json = serde_json::to_value(&ver).unwrap();
    assert_eq!(
        json["rubric"]["anchors"]["scale"],
        serde_json::json!([0, 25, 50, 80, 100])
    );
    let back: AssessmentVersion = serde_json::from_value(json).unwrap();
    assert_eq!(back, ver);
}
