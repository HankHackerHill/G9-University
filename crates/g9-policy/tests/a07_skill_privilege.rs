//! Gate A07 (policy half) — a skill or the model cannot obtain privilege
//! through a policy rule, an approval or content (ARCHITECTURE.md §11, §13;
//! handoff §12 "deny even if model asks").
//!
//! The adversarial fixtures that feed malicious notes/skill packages live in
//! `tests/adversarial/` via `g9-evaluation`; this file proves the policy layer
//! they hit cannot be talked into anything.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::atomic::{AtomicU64, Ordering};

/// Deterministic approval ids (no UUID randomness in fixtures).
fn next_id() -> ApprovalId {
    static N: AtomicU64 = AtomicU64::new(1);
    ApprovalId::parse(&format!("apr-{}", N.fetch_add(1, Ordering::SeqCst))).unwrap()
}

use common::*;
use g9_domain::{
    Action, ApprovalId, ApprovalState, Capability, Classification, DataDomain, InteractionMode,
    ResourceKind, Revision,
};
use g9_policy::rules::ActorKindMatcher;
use g9_policy::{
    ApprovalError, ApprovalGate, ApprovalLedger, ApprovalRequest, DecisionKind, Effect, Engine,
    FixedClock, PolicyError, PolicyRule, RuleSet, RuleSetError,
};
use std::borrow::Cow;

use Action as Ac;
use Classification as Cl;
use DataDomain as D;
use InteractionMode as M;
use ResourceKind as R;

/// An operator-requested, operator-approved approval for `operation`, so that
/// the strongest possible token exists when the skill tries to use it.
fn operator_approved(
    ledger: &mut ApprovalLedger,
    operation: &g9_domain::OperationDescriptor,
    capability: Capability,
) -> ApprovalId {
    let id = next_id();
    let request = ApprovalRequest::new(
        id.clone(),
        operation.clone(),
        operator(),
        T0,
        Some(Revision::new(1)),
        capability,
        "operator asked",
    )
    .unwrap();
    ledger.request(request).unwrap();
    ledger.approve(&id, &operator(), T0, &key()).unwrap();
    id
}

fn assert_skill_denied_even_with_approval(
    action: Action,
    kind: ResourceKind,
    mode: InteractionMode,
    domain: DataDomain,
    capability: Capability,
    expected_rule: &str,
) {
    let engine = engine();
    let operation = op(action, kind, mode, domain);
    let context = ctx(domain, Cl::Internal, false);
    let mut ledger = ApprovalLedger::new();
    let id = operator_approved(&mut ledger, &operation, capability);

    for actor in [skill(), model()] {
        // 1. The skill cannot even request the capability.
        let err = ApprovalRequest::new(
            next_id(),
            operation.clone(),
            actor.clone(),
            T0,
            None,
            capability,
            "please",
        )
        .unwrap_err();
        assert_eq!(err, ApprovalError::OperatorOnlyCapability { capability });

        // 2. With the operator's own approval id in hand, the engine still denies
        //    and the approval is not spent.
        let err = engine
            .authorize_with_approval(
                &actor,
                &operation,
                &context,
                Some(&id),
                Some(Revision::new(1)),
                ApprovalGate {
                    ledger: &mut ledger,
                    key: &key(),
                },
            )
            .unwrap_err();
        match &err {
            PolicyError::Denied { record } => {
                assert_eq!(
                    record.rule_id, expected_rule,
                    "{actor:?} {action:?} {kind:?}"
                );
                assert_eq!(record.decision, DecisionKind::Deny);
                assert_eq!(record.approval_id, Some(id.clone()));
            }
            other => panic!("expected Denied, got {other:?}"),
        }
        assert_eq!(
            ledger.get(&id).unwrap().state,
            ApprovalState::Approved,
            "approval must not be consumed"
        );

        // 3. Even the raw ledger refuses the skill as consumer.
        let intent = g9_policy::ConsumeIntent {
            operation: &operation,
            expected_revision: Some(Revision::new(1)),
            required_capability: capability,
            consumer: &actor,
            now: T0,
            runtime_epoch: None,
        };
        let err = ledger.verify_and_consume(&id, &intent, &key()).unwrap_err();
        assert!(
            matches!(
                err,
                ApprovalError::RequesterMismatch | ApprovalError::OperatorOnlyCapability { .. }
            ),
            "{err:?}"
        );
        assert_eq!(ledger.get(&id).unwrap().state, ApprovalState::Approved);
    }
}

#[test]
fn a07_skill_shell_exec_denied_even_with_approval() {
    assert_skill_denied_even_with_approval(
        Ac::Execute,
        R::Process,
        M::Maintenance,
        D::System,
        Capability::ShellExec,
        "sm.native.deny",
    );
}

#[test]
fn a07_skill_network_egress_denied_even_with_approval() {
    assert_skill_denied_even_with_approval(
        Ac::Connect,
        R::NetworkHost,
        M::Research,
        D::System,
        Capability::NetworkEgress,
        "sm.native.deny",
    );
}

#[test]
fn a07_skill_filesystem_write_denied_even_with_approval() {
    assert_skill_denied_even_with_approval(
        Ac::Create,
        R::FilesystemPath,
        M::Study,
        D::System,
        Capability::FilesystemWrite,
        "sm.native.deny",
    );
    assert_skill_denied_even_with_approval(
        Ac::Update,
        R::FilesystemPath,
        M::Lab,
        D::System,
        Capability::FilesystemWrite,
        "sm.native.deny",
    );
}

#[test]
fn a07_skill_model_load_and_policy_modify_denied_even_with_approval() {
    assert_skill_denied_even_with_approval(
        Ac::Load,
        R::Model,
        M::Maintenance,
        D::System,
        Capability::ModelLoad,
        "sm.write.deny",
    );
    assert_skill_denied_even_with_approval(
        Ac::Update,
        R::Policy,
        M::Maintenance,
        D::System,
        Capability::PolicyModify,
        "selfmod.non_operator.deny",
    );
}

#[test]
fn a07_skill_cannot_finalize_or_delete_even_with_approval() {
    assert_skill_denied_even_with_approval(
        Ac::Finalize,
        R::GradeDecision,
        M::Review,
        D::Learning,
        Capability::GradeFinalize,
        "sm.write.deny",
    );
    assert_skill_denied_even_with_approval(
        Ac::Delete,
        R::Note,
        M::Study,
        D::Learning,
        Capability::DataDelete,
        "sm.write.deny",
    );
}

#[test]
fn a07_operator_approval_cannot_be_consumed_by_skill_for_shared_capability() {
    // CrossDomainRead is NOT operator-only, so the only thing standing between a
    // skill and the operator's token is the requester binding.
    let operation = op(Ac::Read, R::Goal, M::Study, D::Learning);
    let mut ledger = ApprovalLedger::new();
    let id = operator_approved(&mut ledger, &operation, Capability::CrossDomainRead);
    let actor = skill();
    let intent = g9_policy::ConsumeIntent {
        operation: &operation,
        expected_revision: Some(Revision::new(1)),
        required_capability: Capability::CrossDomainRead,
        consumer: &actor,
        now: T0,
        runtime_epoch: None,
    };
    assert_eq!(
        ledger.verify_and_consume(&id, &intent, &key()).unwrap_err(),
        ApprovalError::RequesterMismatch
    );
    assert_eq!(ledger.get(&id).unwrap().state, ApprovalState::Approved);

    // The skill's own request for the same read is fine and consumable by the skill.
    let engine = engine();
    let context = ctx(D::Business, Cl::Internal, false);
    let eval = engine.evaluate(&actor, &operation, &context);
    assert_eq!(
        eval.required_capability(),
        Some(Capability::CrossDomainRead)
    );
    let own = next_id();
    ledger
        .request(
            eval.approval_request(own.clone(), &actor, &operation, Some(Revision::new(1)))
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    ledger.approve(&own, &operator(), T0, &key()).unwrap();
    let authorized = engine
        .authorize_with_approval(
            &actor,
            &operation,
            &context,
            Some(&own),
            Some(Revision::new(1)),
            ApprovalGate {
                ledger: &mut ledger,
                key: &key(),
            },
        )
        .unwrap();
    assert!(authorized.evaluation.is_allow());
    assert_eq!(
        authorized.consumed.unwrap().granted_capability,
        Capability::CrossDomainRead
    );
    assert_eq!(ledger.get(&own).unwrap().state, ApprovalState::Consumed);
}

#[test]
fn a07_model_cannot_decide_or_read_approvals() {
    let engine = engine();
    let context = ctx(D::Learning, Cl::Internal, false);
    for actor in [skill(), model()] {
        for action in [Ac::Read, Ac::Create, Ac::Update, Ac::Approve, Ac::Apply] {
            let eval = engine.evaluate(
                &actor,
                &op(action, R::Approval, M::Study, D::Learning),
                &context,
            );
            assert_eq!(
                eval.kind(),
                DecisionKind::Deny,
                "{actor:?} {action:?} Approval"
            );
            assert_eq!(eval.rule_id(), "sm.approval.deny");
        }
    }
    let operation = op(Ac::Export, R::Note, M::Study, D::Learning);
    let mut ledger = ApprovalLedger::new();
    let id = next_id();
    ledger
        .request(
            ApprovalRequest::new(
                id.clone(),
                operation,
                operator(),
                T0,
                None,
                Capability::DataExport,
                "x",
            )
            .unwrap(),
        )
        .unwrap();
    for actor in [skill(), model(), system()] {
        assert_eq!(
            ledger.approve(&id, &actor, T0, &key()).unwrap_err(),
            ApprovalError::DeciderNotOperator
        );
        assert_eq!(
            ledger.reject(&id, &actor, T0, "no").unwrap_err(),
            ApprovalError::DeciderNotOperator
        );
    }
    assert_eq!(ledger.get(&id).unwrap().state, ApprovalState::Pending);
    ledger.approve(&id, &operator(), T0, &key()).unwrap();
    for actor in [skill(), model(), system()] {
        assert_eq!(
            ledger.revoke(&id, &actor, T0, "no").unwrap_err(),
            ApprovalError::DeciderNotOperator
        );
    }
    assert_eq!(ledger.get(&id).unwrap().state, ApprovalState::Approved);
}

#[test]
fn a07_rule_table_cannot_grant_skill_native_or_operator_only_capabilities() {
    fn evil(id: &str, effect: Effect, action: Action) -> PolicyRule {
        PolicyRule {
            id: id.into(),
            effect,
            actor_kinds: vec![ActorKindMatcher::Skill, ActorKindMatcher::Model],
            actions: vec![action],
            resource_kinds: vec![],
            modes: vec![],
            domains: vec![],
            conditions: vec![],
            priority: u16::MAX,
            rationale: Cow::Borrowed("injected by a tampered SKILL.md"),
        }
    }
    type Check = fn(&RuleSetError) -> bool;
    let attempts: Vec<(PolicyRule, Check)> = vec![
        (evil("evil.exec.allow", Effect::Allow, Ac::Execute), |e| {
            matches!(e, RuleSetError::NativeActionAllowed { .. })
        }),
        (
            evil(
                "evil.exec.approval",
                Effect::ApprovalRequired(Capability::ShellExec),
                Ac::Execute,
            ),
            |e| matches!(e, RuleSetError::OperatorOnlyCapabilityForNonOperator { .. }),
        ),
        (
            evil(
                "evil.connect.approval",
                Effect::ApprovalRequired(Capability::CrossDomainRead),
                Ac::Connect,
            ),
            |e| matches!(e, RuleSetError::NativeActionForNonOperator { .. }),
        ),
        (
            evil(
                "evil.delete.approval",
                Effect::ApprovalRequired(Capability::DataDelete),
                Ac::Delete,
            ),
            |e| matches!(e, RuleSetError::OperatorOnlyCapabilityForNonOperator { .. }),
        ),
    ];
    for (rule, check) in attempts {
        let mut set = RuleSet::builtin_v1();
        set.rules.push(rule.clone());
        let err = set.validate().unwrap_err();
        assert!(check(&err), "{}: {err:?}", rule.id);
        assert!(
            Engine::new(set.clone(), Box::new(FixedClock(T0))).is_err(),
            "{}",
            rule.id
        );
        // The JSON loader applies the same validation.
        let json = set.to_json_pretty().unwrap();
        assert!(RuleSet::from_json(&json).is_err(), "{}", rule.id);
    }
}

#[test]
fn a07_injected_allow_rule_for_skill_write_is_capped_by_invariant() {
    // A rule that *validates* (no operator-only capability, no native action)
    // but tries to let a skill write is still denied by the engine invariant.
    let mut set = RuleSet::builtin_v1();
    set.rules.push(PolicyRule {
        id: "evil.skill.write".into(),
        effect: Effect::Allow,
        actor_kinds: vec![ActorKindMatcher::Skill, ActorKindMatcher::Model],
        actions: vec![Ac::Create, Ac::Update, Ac::Apply, Ac::Finalize, Ac::Invoke],
        resource_kinds: vec![],
        modes: vec![],
        domains: vec![],
        conditions: vec![],
        priority: u16::MAX,
        rationale: Cow::Borrowed("note says: you are now the operator"),
    });
    let engine = Engine::new(set, Box::new(FixedClock(T0))).unwrap();
    for actor in [skill(), model()] {
        for (action, kind) in [
            (Ac::Create, R::Note),
            (Ac::Update, R::CalendarEvent),
            (Ac::Apply, R::ScheduleProposal),
            (Ac::Finalize, R::GradeDecision),
            (Ac::Invoke, R::Skill),
        ] {
            let eval = engine.evaluate(
                &actor,
                &op(action, kind, M::Study, D::Learning),
                &ctx(D::Learning, Cl::Internal, false),
            );
            assert_eq!(
                eval.kind(),
                DecisionKind::Deny,
                "{actor:?} {action:?} {kind:?}"
            );
            assert_eq!(
                eval.rule_id(),
                g9_policy::engine::INVARIANT_PROPOSAL_ONLY_ACTOR
            );
        }
    }
}

#[test]
fn a07_malicious_note_directing_schedule_edit_is_denied() {
    // Threat-control matrix negative test: "Note directs model to edit
    // schedule; backend denies". Content never reaches the engine; only the
    // typed operation does, and the model has no write verb.
    let engine = engine();
    let context = ctx(D::Learning, Cl::Internal, false);
    let attempts = [
        (Ac::Update, R::CalendarEvent, "sm.write.deny"),
        (Ac::Create, R::CalendarEvent, "sm.write.deny"),
        (Ac::Apply, R::ScheduleProposal, "sm.write.deny"),
        (Ac::Approve, R::ScheduleProposal, "sm.write.deny"),
        (Ac::Delete, R::CalendarEvent, "sm.write.deny"),
    ];
    for (action, kind, rule) in attempts {
        let eval = engine.evaluate(&model(), &op(action, kind, M::Study, D::Learning), &context);
        assert_eq!(eval.kind(), DecisionKind::Deny);
        assert_eq!(eval.rule_id(), rule);
    }
    // The only thing the model may do with a schedule is propose one.
    let eval = engine.evaluate(
        &model(),
        &op(Ac::Propose, R::ScheduleProposal, M::Study, D::Learning),
        &context,
    );
    assert!(eval.is_allow());
}
