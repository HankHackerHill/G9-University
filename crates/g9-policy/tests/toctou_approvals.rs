//! TOCTOU / replay gate for approvals (handoff §12 threat-control matrix,
//! "Policy bypass/TOCTOU": reuse approval on different resource or stale
//! revision; ARCHITECTURE.md §5 step 4, §11).
//!
//! Each test names the single `ApprovalError` variant it expects, checks that a
//! refused consumption leaves the approval untouched (so a legitimate retry can
//! still use it) and that the engine logs a `DENY` record with the
//! `approval.*` reason.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use g9_policy::Clock;

use common::*;
use g9_domain::{
    Action, ApprovalId, ApprovalState, Capability, Classification, DataDomain, DurationMs,
    ErrorCategory, G9Error, InteractionMode, OperationDescriptor, ResourceKind, Revision,
    UtcMillis,
};
use g9_policy::{
    ApprovalError, ApprovalGate, ApprovalLedger, ApprovalRequest, ConsumeIntent, DecisionKind,
    Engine, PolicyError, RuntimeEpoch, SteppingClock, DEFAULT_APPROVAL_TTL,
};
use std::sync::Arc;

use Action as Ac;
use Classification as Cl;
use DataDomain as D;
use InteractionMode as M;
use ResourceKind as R;

/// Operation A: finalize grade decision `gd-1` with parameters `aa…`, revision 4.
fn op_a() -> OperationDescriptor {
    op_with(
        Ac::Finalize,
        R::GradeDecision,
        "gd-1",
        M::Review,
        D::Learning,
        "aa",
    )
}

const REV_A: Option<Revision> = Some(Revision::new(4));

struct Fixture {
    engine: Engine,
    clock: Arc<SteppingClock>,
    ledger: ApprovalLedger,
    id: ApprovalId,
}

/// Engine on a stepping clock, ledger holding one approved token for `op_a`.
fn approved_fixture() -> Fixture {
    let clock = Arc::new(SteppingClock::new(T0));
    let clock_for_engine = Arc::clone(&clock);
    let engine = Engine::builtin_v1(Box::new(move || clock_for_engine.now()));
    let mut ledger = ApprovalLedger::new();
    let id = ApprovalId::parse("apr-a").unwrap();
    ledger
        .request(
            ApprovalRequest::new(
                id.clone(),
                op_a(),
                operator(),
                T0,
                REV_A,
                Capability::GradeFinalize,
                "finalize D01 grade",
            )
            .unwrap(),
        )
        .unwrap();
    clock.advance(DurationMs::from_seconds_u32(5));
    ledger
        .approve(&id, &operator(), clock.now(), &key())
        .unwrap();
    assert_eq!(ledger.get(&id).unwrap().state, ApprovalState::Approved);
    Fixture {
        engine,
        clock,
        ledger,
        id,
    }
}

fn intent<'a>(
    operation: &'a OperationDescriptor,
    expected_revision: Option<Revision>,
    consumer: &'a g9_domain::Actor,
    now: UtcMillis,
) -> ConsumeIntent<'a> {
    ConsumeIntent {
        operation,
        expected_revision,
        required_capability: Capability::GradeFinalize,
        consumer,
        now,
        runtime_epoch: None,
    }
}

/// Run the §5 flow for `operation` with the fixture's approval and return the
/// approval error plus its DENY record.
fn consume_via_engine(
    f: &mut Fixture,
    operation: &OperationDescriptor,
    expected_revision: Option<Revision>,
) -> Result<g9_policy::Authorized, PolicyError> {
    f.engine.authorize_with_approval(
        &operator(),
        operation,
        &ctx(D::Learning, Cl::Internal, false),
        Some(&f.id),
        expected_revision,
        ApprovalGate {
            ledger: &mut f.ledger,
            key: &key(),
        },
    )
}

fn expect_approval_error(
    result: Result<g9_policy::Authorized, PolicyError>,
    expected: &ApprovalError,
) -> G9Error {
    match result {
        Err(PolicyError::Approval { source, record }) => {
            assert_eq!(&source, expected);
            assert_eq!(record.decision, DecisionKind::Deny);
            assert_eq!(record.rule_id, expected.code());
            assert!(
                record.approval_id.is_some(),
                "record carries the approval id"
            );
            assert!(!record.reason.is_empty());
            PolicyError::Approval { source, record }.into()
        }
        other => panic!("expected Approval({expected:?}), got {other:?}"),
    }
}

#[test]
fn toctou_happy_path_consumes_exactly_once() {
    let mut f = approved_fixture();
    let authorized = consume_via_engine(&mut f, &op_a(), REV_A).unwrap();
    assert!(authorized.evaluation.is_allow());
    assert_eq!(
        authorized.evaluation.rule_id(),
        "operator.finalize.grade.approval"
    );
    let consumed = authorized.consumed.unwrap();
    assert_eq!(consumed.approval_id, f.id);
    assert_eq!(consumed.granted_capability, Capability::GradeFinalize);
    assert_eq!(consumed.expected_revision, REV_A);
    assert_eq!(consumed.consumed_at, f.clock.now());
    let record = f.ledger.get(&f.id).unwrap();
    assert_eq!(record.state, ApprovalState::Consumed);
    assert_eq!(record.consumed_at, Some(f.clock.now()));
}

#[test]
fn toctou_approval_consumed_twice_is_already_consumed() {
    let mut f = approved_fixture();
    consume_via_engine(&mut f, &op_a(), REV_A).unwrap();
    f.clock.advance(DurationMs::from_seconds_u32(1));
    let g = expect_approval_error(
        consume_via_engine(&mut f, &op_a(), REV_A),
        &ApprovalError::AlreadyConsumed,
    );
    assert_eq!(g.category, ErrorCategory::Unauthorized);
    assert_eq!(g.code, "approval.already_consumed");
    assert_eq!(f.ledger.get(&f.id).unwrap().state, ApprovalState::Consumed);
    // Raw ledger agrees.
    let consumer = operator();
    assert_eq!(
        f.ledger.verify_and_consume(
            &f.id,
            &intent(&op_a(), REV_A, &consumer, f.clock.now()),
            &key()
        ),
        Err(ApprovalError::AlreadyConsumed)
    );
}

#[test]
fn toctou_replay_against_other_operation_is_operation_mismatch() {
    // Same resource, same action, different parameters → the approval does not transfer.
    let mut f = approved_fixture();
    let op_b = op_with(
        Ac::Finalize,
        R::GradeDecision,
        "gd-1",
        M::Review,
        D::Learning,
        "bb",
    );
    let g = expect_approval_error(
        consume_via_engine(&mut f, &op_b, REV_A),
        &ApprovalError::OperationMismatch,
    );
    assert_eq!(g.category, ErrorCategory::Unauthorized);
    assert_eq!(
        f.ledger.get(&f.id).unwrap().state,
        ApprovalState::Approved,
        "refusal leaves the approval usable"
    );

    // Same resource and parameters, different scope (mode) → still a different operation.
    let op_c = op_with(
        Ac::Finalize,
        R::GradeDecision,
        "gd-1",
        M::Maintenance,
        D::Learning,
        "aa",
    );
    expect_approval_error(
        consume_via_engine(&mut f, &op_c, REV_A),
        &ApprovalError::OperationMismatch,
    );
    assert_eq!(f.ledger.get(&f.id).unwrap().state, ApprovalState::Approved);

    // Same resource and parameters, different action (Correct instead of Finalize).
    let op_d = op_with(
        Ac::Correct,
        R::GradeDecision,
        "gd-1",
        M::Review,
        D::Learning,
        "aa",
    );
    expect_approval_error(
        consume_via_engine(&mut f, &op_d, REV_A),
        &ApprovalError::OperationMismatch,
    );
    assert_eq!(f.ledger.get(&f.id).unwrap().state, ApprovalState::Approved);

    // The original still works afterwards.
    consume_via_engine(&mut f, &op_a(), REV_A).unwrap();
}

#[test]
fn toctou_replay_against_other_resource_is_resource_mismatch() {
    let mut f = approved_fixture();
    let other_id = op_with(
        Ac::Finalize,
        R::GradeDecision,
        "gd-2",
        M::Review,
        D::Learning,
        "aa",
    );
    let g = expect_approval_error(
        consume_via_engine(&mut f, &other_id, REV_A),
        &ApprovalError::ResourceMismatch,
    );
    assert_eq!(g.category, ErrorCategory::Unauthorized);
    assert_eq!(f.ledger.get(&f.id).unwrap().state, ApprovalState::Approved);

    let other_kind = op_with(
        Ac::Finalize,
        R::Attempt,
        "gd-1",
        M::Review,
        D::Learning,
        "aa",
    );
    expect_approval_error(
        consume_via_engine(&mut f, &other_kind, REV_A),
        &ApprovalError::ResourceMismatch,
    );
    assert_eq!(f.ledger.get(&f.id).unwrap().state, ApprovalState::Approved);
}

#[test]
fn toctou_stale_expected_revision_is_revision_mismatch() {
    let mut f = approved_fixture();
    for stale in [Some(Revision::new(3)), Some(Revision::new(5)), None] {
        let g = expect_approval_error(
            consume_via_engine(&mut f, &op_a(), stale),
            &ApprovalError::RevisionMismatch {
                expected: stale,
                bound: REV_A,
            },
        );
        assert_eq!(g.category, ErrorCategory::StaleRevision);
        assert_eq!(g.code, "approval.revision_mismatch");
        let details = g.details.unwrap();
        assert_eq!(details["bound"], 4);
        assert_eq!(
            details["expected"],
            stale
                .map(Revision::get)
                .map_or(serde_json::Value::Null, serde_json::Value::from)
        );
        assert_eq!(f.ledger.get(&f.id).unwrap().state, ApprovalState::Approved);
    }
    consume_via_engine(&mut f, &op_a(), REV_A).unwrap();
}

#[test]
fn toctou_expired_approval_is_expired() {
    let mut f = approved_fixture();
    // One millisecond before expiry is still fine for a *different* check…
    let expires_at = f.ledger.get(&f.id).unwrap().request.expires_at;
    assert_eq!(expires_at, T0.checked_add(DEFAULT_APPROVAL_TTL).unwrap());
    f.clock
        .set(UtcMillis::from_millis(expires_at.as_millis() - 1));
    expect_approval_error(
        consume_via_engine(&mut f, &op_a(), Some(Revision::new(9))),
        &ApprovalError::RevisionMismatch {
            expected: Some(Revision::new(9)),
            bound: REV_A,
        },
    );
    assert_eq!(f.ledger.get(&f.id).unwrap().state, ApprovalState::Approved);

    // …at expiry the token is dead, and the record says so.
    f.clock.set(expires_at);
    let g = expect_approval_error(
        consume_via_engine(&mut f, &op_a(), REV_A),
        &ApprovalError::Expired,
    );
    assert_eq!(g.category, ErrorCategory::ApprovalRequired);
    let record = f.ledger.get(&f.id).unwrap();
    assert_eq!(record.state, ApprovalState::Expired);
    assert_eq!(record.expired_at, Some(expires_at));

    // After that it is NotApproved, never resurrected.
    f.clock.advance(DurationMs::from_seconds_u32(1));
    expect_approval_error(
        consume_via_engine(&mut f, &op_a(), REV_A),
        &ApprovalError::NotApproved {
            state: ApprovalState::Expired,
        },
    );
}

#[test]
fn toctou_approve_after_expiry_is_refused() {
    let mut ledger = ApprovalLedger::new();
    let id = ApprovalId::parse("late").unwrap();
    ledger
        .request(
            ApprovalRequest::new(
                id.clone(),
                op_a(),
                operator(),
                T0,
                REV_A,
                Capability::GradeFinalize,
                "x",
            )
            .unwrap(),
        )
        .unwrap();
    let too_late = T0.checked_add(DEFAULT_APPROVAL_TTL).unwrap();
    assert_eq!(
        ledger
            .approve(&id, &operator(), too_late, &key())
            .unwrap_err(),
        ApprovalError::Expired
    );
    let record = ledger.get(&id).unwrap();
    assert_eq!(record.state, ApprovalState::Expired);
    assert!(
        record.binding_mac.is_none(),
        "no MAC is ever issued for an expired request"
    );
}

#[test]
fn toctou_forged_mac_is_mac_invalid() {
    // (a) A token approved under another installation's key.
    let mut f = approved_fixture();
    let mut foreign = ApprovalLedger::new();
    let fid = ApprovalId::parse("foreign").unwrap();
    foreign
        .request(
            ApprovalRequest::new(
                fid.clone(),
                op_a(),
                operator(),
                T0,
                REV_A,
                Capability::GradeFinalize,
                "x",
            )
            .unwrap(),
        )
        .unwrap();
    foreign
        .approve(&fid, &operator(), T0, &other_key())
        .unwrap();
    let consumer = operator();
    assert_eq!(
        foreign.verify_and_consume(&fid, &intent(&op_a(), REV_A, &consumer, T0), &key()),
        Err(ApprovalError::MacInvalid)
    );
    assert_eq!(foreign.get(&fid).unwrap().state, ApprovalState::Approved);

    // (b) A snapshot whose MAC was replaced with a well-formed but wrong value.
    let mut json: serde_json::Value = serde_json::from_str(&f.ledger.to_json().unwrap()).unwrap();
    let mac = json["records"][f.id.as_str()]["binding_mac"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(mac.len(), 64);
    let forged: String = mac.chars().rev().collect();
    json["records"][f.id.as_str()]["binding_mac"] = serde_json::Value::String(forged);
    f.ledger = ApprovalLedger::from_json(&json.to_string()).unwrap();
    let g = expect_approval_error(
        consume_via_engine(&mut f, &op_a(), REV_A),
        &ApprovalError::MacInvalid,
    );
    assert_eq!(g.category, ErrorCategory::Unauthorized);
    assert_eq!(f.ledger.get(&f.id).unwrap().state, ApprovalState::Approved);

    // (c) A snapshot whose *operation* was rewritten to a juicier one while
    //     keeping the genuine MAC: the field checks pass, the MAC does not.
    //     Consumed at the fixture's decision instant (T0+5s): an instant before
    //     the decision is a wall clock that moved backwards (SEC-02) and would
    //     be refused as EXPIRED before the MAC is checked.
    let after_decision = f.clock.now();
    let mut json: serde_json::Value =
        serde_json::from_str(&approved_fixture().ledger.to_json().unwrap()).unwrap();
    json["records"]["apr-a"]["request"]["operation"]["resource"]["id"] =
        serde_json::Value::String("gd-999".into());
    let mut tampered = ApprovalLedger::from_json(&json.to_string()).unwrap();
    let juicier = op_with(
        Ac::Finalize,
        R::GradeDecision,
        "gd-999",
        M::Review,
        D::Learning,
        "aa",
    );
    assert_eq!(
        tampered.verify_and_consume(
            &ApprovalId::parse("apr-a").unwrap(),
            &intent(&juicier, REV_A, &consumer, after_decision),
            &key()
        ),
        Err(ApprovalError::MacInvalid)
    );

    // (d) A snapshot with the MAC removed entirely.
    let mut json: serde_json::Value =
        serde_json::from_str(&approved_fixture().ledger.to_json().unwrap()).unwrap();
    json["records"]["apr-a"]
        .as_object_mut()
        .unwrap()
        .remove("binding_mac");
    let mut stripped = ApprovalLedger::from_json(&json.to_string()).unwrap();
    assert_eq!(
        stripped.verify_and_consume(
            &ApprovalId::parse("apr-a").unwrap(),
            &intent(&op_a(), REV_A, &consumer, after_decision),
            &key()
        ),
        Err(ApprovalError::MacInvalid)
    );
}

#[test]
fn toctou_pending_rejected_revoked_are_not_approved() {
    let mut ledger = ApprovalLedger::new();
    let consumer = operator();
    let pending = ApprovalId::parse("pending").unwrap();
    ledger
        .request(
            ApprovalRequest::new(
                pending.clone(),
                op_a(),
                operator(),
                T0,
                REV_A,
                Capability::GradeFinalize,
                "x",
            )
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        ledger.verify_and_consume(&pending, &intent(&op_a(), REV_A, &consumer, T0), &key()),
        Err(ApprovalError::NotApproved {
            state: ApprovalState::Pending
        })
    );
    ledger.reject(&pending, &operator(), T0, "no").unwrap();
    assert_eq!(
        ledger.verify_and_consume(&pending, &intent(&op_a(), REV_A, &consumer, T0), &key()),
        Err(ApprovalError::NotApproved {
            state: ApprovalState::Rejected
        })
    );
    let mut f = approved_fixture();
    f.ledger
        .revoke(&f.id, &operator(), f.clock.now(), "withdrawn")
        .unwrap();
    let g = expect_approval_error(
        consume_via_engine(&mut f, &op_a(), REV_A),
        &ApprovalError::NotApproved {
            state: ApprovalState::Revoked,
        },
    );
    assert_eq!(g.category, ErrorCategory::ApprovalRequired);
    assert_eq!(
        f.ledger.verify_and_consume(
            &ApprovalId::parse("nope").unwrap(),
            &intent(&op_a(), REV_A, &consumer, T0),
            &key()
        ),
        Err(ApprovalError::NotFound)
    );
}

#[test]
fn toctou_capability_mismatch_is_refused_before_consumption() {
    // The token grants GradeFinalize; the rule for Export demands DataExport.
    let mut f = approved_fixture();
    let export = op_with(
        Ac::Export,
        R::GradeDecision,
        "gd-1",
        M::Review,
        D::Learning,
        "aa",
    );
    let result = f.engine.authorize_with_approval(
        &operator(),
        &export,
        &ctx(D::Learning, Cl::Internal, false),
        Some(&f.id),
        REV_A,
        ApprovalGate {
            ledger: &mut f.ledger,
            key: &key(),
        },
    );
    expect_approval_error(
        result,
        &ApprovalError::CapabilityMismatch {
            required: Capability::DataExport,
            granted: Capability::GradeFinalize,
        },
    );
    assert_eq!(f.ledger.get(&f.id).unwrap().state, ApprovalState::Approved);
}

#[test]
fn toctou_snapshot_round_trip_keeps_single_use() {
    let mut f = approved_fixture();
    consume_via_engine(&mut f, &op_a(), REV_A).unwrap();
    let snapshot = f.ledger.to_json().unwrap();
    assert!(
        !snapshot.contains("5a5a5a"),
        "key bytes never appear in a snapshot"
    );
    let mut restored = ApprovalLedger::from_json(&snapshot).unwrap();
    assert_eq!(restored, f.ledger);
    let consumer = operator();
    assert_eq!(
        restored.verify_and_consume(
            &f.id,
            &intent(&op_a(), REV_A, &consumer, f.clock.now()),
            &key()
        ),
        Err(ApprovalError::AlreadyConsumed)
    );
}

#[test]
fn toctou_wall_clock_set_back_after_restart_expires_approval() {
    // SEC-02 (defect register D.2): approved at T0+5s, ledger restored from a
    // snapshot (restart), wall clock then set back one day. Before the fix the
    // approval was consumed with now < requested_at.
    let mut f = approved_fixture();
    f.ledger = ApprovalLedger::from_json(&f.ledger.to_json().unwrap()).unwrap();
    f.clock
        .set(UtcMillis::from_millis(T0.as_millis() - 86_400_000));
    let g = expect_approval_error(
        consume_via_engine(&mut f, &op_a(), REV_A),
        &ApprovalError::Expired,
    );
    assert_eq!(g.code, "approval.expired");
    let record = f.ledger.get(&f.id).unwrap();
    assert_eq!(record.state, ApprovalState::Expired);
    assert_eq!(record.consumed_at, None);
    // Restoring the clock does not revive it.
    f.clock.set(T0);
    f.clock.advance(DurationMs::from_seconds_u32(10));
    expect_approval_error(
        consume_via_engine(&mut f, &op_a(), REV_A),
        &ApprovalError::NotApproved {
            state: ApprovalState::Expired,
        },
    );
}

#[test]
fn toctou_wall_clock_set_back_between_decision_and_use_expires_approval() {
    // Inside [requested_at, expires_at) but before the operator's decision
    // instant: the clock moved backwards after approval.
    let mut f = approved_fixture(); // requested T0, approved T0+5s
    f.clock.set(T0);
    f.clock.advance(DurationMs::from_seconds_u32(2));
    expect_approval_error(
        consume_via_engine(&mut f, &op_a(), REV_A),
        &ApprovalError::Expired,
    );
    assert_eq!(f.ledger.get(&f.id).unwrap().state, ApprovalState::Expired);
}

#[test]
fn toctou_restart_under_new_runtime_epoch_expires_approval() {
    // SEC-02: the approval is stamped with the epoch it was issued in; an
    // engine for a restarted process (new process-start id) or a rebooted host
    // (new boot id) refuses it as EXPIRED even inside the 15-minute window.
    let issued = RuntimeEpoch::new("boot-a", "proc-1").unwrap();
    let build = |engine_epoch: RuntimeEpoch| {
        let clock = Arc::new(SteppingClock::new(T0));
        let c = Arc::clone(&clock);
        let engine = Engine::builtin_v1(Box::new(move || c.now())).with_runtime_epoch(engine_epoch);
        let mut ledger = ApprovalLedger::new();
        let id = ApprovalId::parse("apr-epoch").unwrap();
        ledger
            .request_in_epoch(
                ApprovalRequest::new(
                    id.clone(),
                    op_a(),
                    operator(),
                    T0,
                    REV_A,
                    Capability::GradeFinalize,
                    "finalize D01 grade",
                )
                .unwrap(),
                &issued,
            )
            .unwrap();
        clock.advance(DurationMs::from_seconds_u32(5));
        ledger
            .approve(&id, &operator(), clock.now(), &key())
            .unwrap();
        // Snapshot round trip: what g9-app does when it re-reads a record.
        let ledger = ApprovalLedger::from_json(&ledger.to_json().unwrap()).unwrap();
        clock.advance(DurationMs::from_seconds_u32(5));
        Fixture {
            engine,
            clock,
            ledger,
            id,
        }
    };

    let mut same = build(issued.clone());
    assert_eq!(same.engine.runtime_epoch(), Some(&issued));
    let authorized = consume_via_engine(&mut same, &op_a(), REV_A).unwrap();
    assert!(authorized.consumed.is_some());

    for restarted in [
        RuntimeEpoch::new("boot-a", "proc-2").unwrap(),
        RuntimeEpoch::new("boot-b", "proc-1").unwrap(),
    ] {
        let mut f = build(restarted);
        let g = expect_approval_error(
            consume_via_engine(&mut f, &op_a(), REV_A),
            &ApprovalError::Expired,
        );
        assert_eq!(g.code, "approval.expired");
        let record = f.ledger.get(&f.id).unwrap();
        assert_eq!(record.state, ApprovalState::Expired);
        assert_eq!(record.consumed_at, None);
    }
}

#[test]
fn toctou_expire_due_sweeps_only_due_records() {
    let clock = SteppingClock::new(T0);
    let mut ledger = ApprovalLedger::new();
    let early = ApprovalId::parse("early").unwrap();
    let late = ApprovalId::parse("late").unwrap();
    ledger
        .request(
            ApprovalRequest::new(
                early.clone(),
                op_a(),
                operator(),
                T0,
                REV_A,
                Capability::GradeFinalize,
                "x",
            )
            .unwrap(),
        )
        .unwrap();
    ledger
        .request(
            ApprovalRequest::new(
                late.clone(),
                op_a(),
                operator(),
                T0,
                REV_A,
                Capability::GradeFinalize,
                "x",
            )
            .unwrap()
            .with_ttl(DurationMs::from_minutes_u32(60))
            .unwrap(),
        )
        .unwrap();
    ledger
        .approve(&early, &operator(), clock.now(), &key())
        .unwrap();
    clock.advance(DEFAULT_APPROVAL_TTL);
    assert_eq!(ledger.expire_due(clock.now()), vec![early.clone()]);
    assert_eq!(ledger.get(&early).unwrap().state, ApprovalState::Expired);
    assert_eq!(ledger.get(&late).unwrap().state, ApprovalState::Pending);
    // Up to `late`'s own 60-minute window, whatever the default TTL is.
    clock.advance(
        DurationMs::new(
            DurationMs::from_minutes_u32(60).as_millis() - DEFAULT_APPROVAL_TTL.as_millis(),
        )
        .unwrap(),
    );
    assert_eq!(ledger.expire_due(clock.now()), vec![late.clone()]);
    assert!(ledger.expire_due(clock.now()).is_empty());
}

#[test]
fn toctou_revision_bound_at_request_time_is_what_the_operator_saw() {
    // ARCHITECTURE §5 step 4: the approval binds the expected_revision the
    // operator was looking at; a record that moved on cannot be finalized with it.
    let clock = SteppingClock::new(T0);
    let engine = {
        let c = Arc::new(SteppingClock::new(T0));
        let cc = Arc::clone(&c);
        Engine::builtin_v1(Box::new(move || cc.now()))
    };
    let mut ledger = ApprovalLedger::new();
    let eval = engine.evaluate(&operator(), &op_a(), &ctx(D::Learning, Cl::Internal, false));
    assert_eq!(eval.kind(), DecisionKind::ApprovalRequired);
    let id = ApprovalId::parse("rev").unwrap();
    let request = eval
        .approval_request(id.clone(), &operator(), &op_a(), Some(Revision::new(4)))
        .unwrap()
        .unwrap();
    assert_eq!(request.expected_revision, Some(Revision::new(4)));
    ledger.request(request).unwrap();
    ledger
        .approve(&id, &operator(), clock.now(), &key())
        .unwrap();
    // The row was updated by someone else: revision is now 5.
    let result = engine.authorize_with_approval(
        &operator(),
        &op_a(),
        &ctx(D::Learning, Cl::Internal, false),
        Some(&id),
        Some(Revision::new(5)),
        ApprovalGate {
            ledger: &mut ledger,
            key: &key(),
        },
    );
    let g = expect_approval_error(
        result,
        &ApprovalError::RevisionMismatch {
            expected: Some(Revision::new(5)),
            bound: Some(Revision::new(4)),
        },
    );
    assert_eq!(g.category, ErrorCategory::StaleRevision);
}
