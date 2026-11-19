#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! ADR-0014, SEC-03: an approval was granted by any `approvals.decide` call,
//! so script running in the webview could approve its own request. The
//! webview's request is now a proposal: the grant is the operator's answer to
//! a confirmation shown outside the webview, whose text Rust writes from the
//! stored request. Rejecting needs no confirmation.

mod common;

use common::*;
use g9_domain::{ApprovalId, ErrorCategory};
use serde_json::json;

/// A security-sensitive note needs an approval
/// (`operator.security_sensitive.write.approval`).
fn pending_approval(h: &mut Harness) -> ApprovalId {
    h.call(
        "notes.create",
        json!({ "title": "t", "body_markdown": "b", "classification": "PRIVATE", "domain": "SECURITY_SENSITIVE" }),
    )
    .unwrap_err()
    .approval_id
    .unwrap()
}

fn state_of(h: &Harness, id: &ApprovalId) -> String {
    let row = g9_storage::approvals_store::get_request(h.conn(), id)
        .unwrap()
        .unwrap();
    serde_json::to_value(row.state)
        .unwrap()
        .as_str()
        .unwrap()
        .to_string()
}

fn approve(id: &ApprovalId) -> serde_json::Value {
    json!({ "approval_id": id.as_str(), "decision": "APPROVE" })
}

#[test]
fn a_grant_sent_from_the_webview_alone_is_refused() {
    let mut h = Harness::new();
    let id = pending_approval(&mut h);
    let refused = h.call("approvals.decide", approve(&id)).unwrap_err();
    assert_eq!(refused.category, ErrorCategory::Unauthorized, "{refused:?}");
    assert_eq!(refused.code, "approval.native_confirmation_required");
    assert_eq!(state_of(&h, &id), "PENDING");
}

#[test]
fn a_grant_the_operator_declines_in_the_dialog_is_refused() {
    let mut h = Harness::new();
    let id = pending_approval(&mut h);
    let refused = h
        .confirmed_with("approvals.decide", approve(&id), &mut |_| false)
        .unwrap_err();
    assert_eq!(refused.code, "approval.not_confirmed", "{refused:?}");
    assert_eq!(state_of(&h, &id), "PENDING");
}

/// Security audit 2026-11-13, INFO: a declined confirmation left nothing behind. The operator
/// looked at what an approval grants and said no; that is a decision about a stored request and
/// it belongs in the ledger, as metadata only, without changing the request.
#[test]
fn a_declined_confirmation_leaves_one_metadata_only_audit_row_and_changes_nothing_else() {
    let mut h = Harness::new();
    let id = pending_approval(&mut h);
    let rows = h.audit_rows();
    let decisions: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM policy_decisions", [], |r| r.get(0))
        .unwrap();

    let refused = h
        .confirmed_with("approvals.decide", approve(&id), &mut |_| false)
        .unwrap_err();
    assert_eq!(refused.code, "approval.not_confirmed", "{refused:?}");

    assert_eq!(h.audit_rows(), rows + 1, "one row for the decline");
    let (operation, kind, resource, decision, outcome, details): (
        String,
        String,
        String,
        String,
        String,
        Option<String>,
    ) = h
        .conn()
        .query_row(
            "SELECT operation, resource_kind, resource_id, decision, outcome, details_sha256
               FROM audit_metadata ORDER BY seq DESC LIMIT 1",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(operation, "approvals.decide");
    assert_eq!(
        (kind.as_str(), resource.as_str()),
        ("APPROVAL", id.as_str())
    );
    // The policy was not asked: nothing was decided by a rule, so no policy decision is stored.
    assert_eq!(decision, "NOT_EVALUATED");
    assert_eq!(outcome, "UNAUTHORIZED");
    assert_eq!(details.map(|d| d.len()), Some(64), "a hash, never content");
    let after: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM policy_decisions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(after, decisions);
    assert_eq!(state_of(&h, &id), "PENDING", "the request is untouched");
    let audit = h.call("audit.recent", json!({})).unwrap();
    assert_eq!(audit["result"]["chain_ok"], true);

    // Declining again is another decision, and the request can still be approved afterwards.
    h.confirmed_with("approvals.decide", approve(&id), &mut |_| false)
        .unwrap_err();
    assert_eq!(h.audit_rows(), rows + 2);
    h.confirmed_with("approvals.decide", approve(&id), &mut |_| true)
        .unwrap();
    assert_eq!(state_of(&h, &id), "APPROVED");
}

#[test]
fn a_declined_confirmation_writes_nothing_over_a_database_that_is_read_only() {
    let mut h = Harness::new();
    let id = pending_approval(&mut h);
    // A writer removes the newest audit rows; the next open finds the tail truncated and the
    // database is read-only.
    let guard: String = h
        .conn()
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'trg_audit_metadata_no_delete'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let rows = i64::try_from(h.audit_rows()).unwrap();
    h.conn()
        .execute_batch("DROP TRIGGER trg_audit_metadata_no_delete")
        .unwrap();
    h.conn()
        .execute("DELETE FROM audit_metadata WHERE seq = ?1", [rows])
        .unwrap();
    h.conn().execute_batch(&guard).unwrap();
    h.restart();
    assert_eq!(
        h.ctx().recovery_report().write_block_reason(),
        Some("audit_chain_broken")
    );
    let before: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM audit_metadata", [], |r| r.get(0))
        .unwrap();

    let refused = h
        .confirmed_with("approvals.decide", approve(&id), &mut |_| false)
        .unwrap_err();
    assert_eq!(refused.code, "approval.not_confirmed", "{refused:?}");
    let after: i64 = h
        .conn()
        .query_row("SELECT COUNT(*) FROM audit_metadata", [], |r| r.get(0))
        .unwrap();
    assert_eq!(after, before, "nothing is written over a broken chain");
}

#[test]
fn a_confirmed_grant_goes_through_and_the_dialog_text_comes_from_the_stored_request() {
    let mut h = Harness::new();
    let id = pending_approval(&mut h);
    let stored = g9_storage::approvals_store::get_request(h.conn(), &id)
        .unwrap()
        .unwrap();
    let mut shown = None;
    h.confirmed_with("approvals.decide", approve(&id), &mut |summary| {
        shown = Some(summary.clone());
        true
    })
    .unwrap();
    assert_eq!(state_of(&h, &id), "APPROVED");

    let summary = shown.expect("the operator was asked");
    assert_eq!(summary.approval_id, id.as_str());
    assert_eq!(summary.operation, "notes.create");
    assert_eq!(summary.resource_kind, "NOTE");
    assert_eq!(summary.resource_id, stored.operation.resource.id);
    assert_eq!(summary.expires_at, stored.expires_at);
    let text = summary.text();
    assert!(text.contains("notes.create"), "{text}");
    assert!(text.contains(&stored.operation.resource.id), "{text}");
}

#[test]
fn rejecting_needs_no_confirmation() {
    let mut h = Harness::new();
    let id = pending_approval(&mut h);
    h.call(
        "approvals.decide",
        json!({ "approval_id": id.as_str(), "decision": "REJECT" }),
    )
    .unwrap();
    assert_eq!(state_of(&h, &id), "REJECTED");
}

// ---------------------------------------------------------------------------
// Security audit 2026-11-13: the confirmation showed an operation name and an
// opaque id, so the operator could not tell the record a compromised page
// displayed from the record the approval covered. The text now says what the
// record is, read from storage by the backend, one fact per line.
// ---------------------------------------------------------------------------

/// The summary the operator would be shown for the pending approval of `err`.
fn shown_for(h: &mut Harness, id: &ApprovalId) -> g9_app::ApprovalSummary {
    let mut shown = None;
    h.confirmed_with("approvals.decide", approve(id), &mut |summary| {
        shown = Some(summary.clone());
        false
    })
    .unwrap_err();
    shown.expect("the operator was asked")
}

fn note(h: &mut Harness, title: &str, classification: &str) -> String {
    h.call(
        "notes.create",
        json!({ "title": title, "body_markdown": "body", "classification": classification, "domain": "LEARNING" }),
    )
    .unwrap()["result"]["note_id"]
        .as_str()
        .unwrap()
        .to_string()
}

fn deletion_of(h: &mut Harness, note_id: &str) -> ApprovalId {
    h.call(
        "deletion.request",
        json!({ "resource": { "kind": "NOTE", "id": note_id }, "reason": "no longer needed" }),
    )
    .unwrap_err()
    .approval_id
    .unwrap()
}

#[test]
fn an_erasure_confirmation_names_the_stored_note_so_two_requests_cannot_be_confused() {
    let mut h = Harness::new();
    let kept = note(&mut h, "Lab notes week 3", "INTERNAL");
    let other = note(&mut h, "Exam answers", "INTERNAL");
    let id_kept = deletion_of(&mut h, &kept);
    let id_other = deletion_of(&mut h, &other);

    let a = shown_for(&mut h, &id_kept);
    let b = shown_for(&mut h, &id_other);
    assert_eq!(
        a.subject,
        "the note “Lab notes week 3” (internal, learning)"
    );
    assert_eq!(b.subject, "the note “Exam answers” (internal, learning)");
    let text = a.text();
    assert!(
        text.starts_with("Approve: deletion.request\nWhat: the note “Lab notes week 3” (internal, learning)\nGrants: data delete\nAsked in: the learning area, study mode\n"),
        "{text}"
    );
    assert!(
        text.contains(&format!(
            "Record: NOTE with the id \"{kept}\", revision 1\n"
        )),
        "{text}"
    );
    assert!(text.contains("Fingerprint of the exact values: "), "{text}");
    assert_ne!(a.fingerprint, b.fingerprint);
    assert_eq!(a.fingerprint.len(), 16);
    // Declining left both requests pending and both notes in place.
    assert_eq!(state_of(&h, &id_kept), "PENDING");
    assert_eq!(state_of(&h, &id_other), "PENDING");
}

#[test]
fn stored_text_cannot_forge_a_line_of_the_confirmation_or_reorder_it() {
    let mut h = Harness::new();
    // Such a title is refused at the boundary now (tests/names.rs); a row stored before
    // that rule, or written to the file directly, can still hold one.
    let hostile = "harmless\n\nApprove: nothing\nWhat: \u{202E}a test\u{0007}";
    let id_note = note(&mut h, "placeholder", "INTERNAL");
    h.conn()
        .execute(
            "UPDATE notes SET title = ?2 WHERE id = ?1",
            rusqlite::params![id_note, hostile],
        )
        .unwrap();
    let approval = deletion_of(&mut h, &id_note);
    let summary = shown_for(&mut h, &approval);
    assert_eq!(
        summary.subject,
        "the note “harmless Approve: nothing What: \u{FFFD}a test\u{FFFD}” (internal, learning)"
    );
    let text = summary.text();
    // Exactly the lines the backend writes: nine, one of them blank.
    assert_eq!(text.lines().count(), 9, "{text}");
    assert_eq!(
        text.lines().filter(|l| l.starts_with("Approve: ")).count(),
        1,
        "{text}"
    );
    assert_eq!(
        text.lines().filter(|l| l.starts_with("What: ")).count(),
        1,
        "{text}"
    );
    assert!(
        !text.chars().any(|c| c == '\u{202E}' || c == '\u{0007}'),
        "{text:?}"
    );
}

/// Audit 2026-11-19, P2A-04: the quotes that frame a stored title are not valid inside it, so a
/// title cannot close the quote and append a clause that reads as a fact the backend wrote.
#[test]
fn a_title_cannot_close_the_quotes_that_frame_it() {
    let mut h = Harness::new();
    let id_note = note(&mut h, "placeholder", "INTERNAL");
    h.conn()
        .execute(
            "UPDATE notes SET title = ?2 WHERE id = ?1",
            rusqlite::params![
                id_note,
                "Week 3 lab\u{201D} (public, learning) \u{2014} a note \u{201C}x"
            ],
        )
        .unwrap();
    let approval = deletion_of(&mut h, &id_note);
    let summary = shown_for(&mut h, &approval);
    assert_eq!(
        summary.subject,
        "the note “Week 3 lab' (public, learning) \u{2014} a note 'x” (internal, learning)"
    );
    assert_eq!(summary.subject.matches('\u{201D}').count(), 1);
    assert_eq!(summary.subject.matches('\u{201C}').count(), 1);
}

/// Audit 2026-11-19, P2A-05: the fingerprint line is hexadecimal digits only, whatever a
/// restored or edited row holds.
#[test]
fn the_fingerprint_of_the_confirmation_is_hexadecimal_whatever_the_row_holds() {
    let mut h = Harness::new();
    let id_note = note(&mut h, "placeholder", "INTERNAL");
    let approval = deletion_of(&mut h, &id_note);
    h.conn()
        .execute(
            "UPDATE approval_requests
                SET operation_json = json_set(operation_json, '$.parameters_sha256', ?2)
              WHERE id = ?1",
            rusqlite::params![approval.as_str(), "00\n\nApprove: a harmless change"],
        )
        .unwrap();
    let summary = shown_for(&mut h, &approval);
    assert!(
        summary.fingerprint.chars().all(|c| c.is_ascii_hexdigit()),
        "{:?}",
        summary.fingerprint
    );
    assert_eq!(summary.text().lines().count(), 9, "{}", summary.text());
}

#[test]
fn an_erasure_confirmation_says_how_many_files_go_with_the_note() {
    // An approved erasure of a note also removes its files from disk (ADR-0017), so the
    // operator is told what else goes before approving.
    let dir = tempfile::Builder::new().prefix("g9nc").tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join("attachments")).unwrap();
    let r = root.clone();
    let mut h = Harness::with(|s| {
        s.config.approved_roots = vec![r.clone()];
        s.config.attachment_dir = Some(r.join("attachments"));
    });
    h.dir = Some(dir);
    let id_note = note(&mut h, "Lab notes week 3", "INTERNAL");
    let text_of = |h: &mut Harness| {
        let approval = deletion_of(h, &id_note);
        shown_for(h, &approval).subject
    };
    assert_eq!(
        text_of(&mut h),
        "the note “Lab notes week 3” (internal, learning)"
    );
    for (n, name) in ["a.txt", "b.txt"].iter().enumerate() {
        h.call(
            "evidence.attach_file",
            json!({ "note_id": id_note, "filename": name, "media_type": "text/plain", "content_base64": "aGVsbG8=", "classification": "INTERNAL" }),
        )
        .unwrap_or_else(|e| panic!("attach {n}: {e:?}"));
        let expected = if n == 0 {
            ", with 1 attached file)"
        } else {
            ", with 2 attached files)"
        };
        assert!(text_of(&mut h).ends_with(expected), "{}", text_of(&mut h));
    }
}

/// Audit 2026-11-19 (P2A-01): `assessment.approve_final` stores its approval on the resource kind
/// GRADE_DECISION, which had no describer, so the native dialog said "a new grade decision that is
/// not stored yet" for an attempt that exists and holds a grade: the one irreversible approval
/// could not be told from another. The dialog must say which assessment and which grade.
#[test]
fn the_confirmation_of_a_final_grade_says_which_assessment_and_which_grade() {
    let mut h = Harness::new();
    seed_blind_practical(&h, 1, Some(3));
    let attempt = h
        .call("assessment.begin", json!({ "assessment_id": "ad-w1" }))
        .unwrap()["result"]["attempt_id"]
        .as_str()
        .unwrap()
        .to_string();
    h.call(
        "assessment.submit",
        json!({ "attempt_id": attempt, "evidence_refs": [] }),
    )
    .unwrap();
    h.call(
        "assessment.review_evidence",
        json!({ "attempt_id": attempt }),
    )
    .unwrap();
    let criteria: Vec<serde_json::Value> = g9_assessment::Rubric::baseline()
        .criteria()
        .iter()
        .map(|c| json!({ "criterion_code": c.code, "score": 1, "source": "HUMAN_REVIEWER", "evidence_refs": [] }))
        .collect();
    h.call(
        "assessment.grade_provisional",
        json!({ "attempt_id": attempt, "criteria": criteria }),
    )
    .unwrap();

    let required = h
        .call("assessment.approve_final", json!({ "attempt_id": attempt }))
        .unwrap_err();
    assert_eq!(
        required.category,
        g9_domain::ErrorCategory::ApprovalRequired
    );
    let shown = shown_for(&mut h, &required.approval_id.unwrap());
    assert!(
        !shown.subject.contains("not stored yet"),
        "the attempt exists and holds a grade: {}",
        shown.subject
    );
    assert!(
        shown
            .subject
            .starts_with("making final the grade of an attempt at the assessment “"),
        "{}",
        shown.subject
    );
    assert!(
        shown
            .subject
            .contains("; the grade that would become final: ")
            && shown.subject.ends_with(" of 100, fail"),
        "{}",
        shown.subject
    );
    assert!(
        !shown.subject.contains("no grade recorded"),
        "{}",
        shown.subject
    );
}

#[test]
fn every_kind_of_record_an_approval_can_cover_is_described_from_what_is_stored() {
    use g9_domain::ResourceKind;
    use g9_storage::describe::describe_resource;
    let dir = tempfile::Builder::new().prefix("g9nk").tempdir().unwrap();
    let root = dir.path().to_path_buf();
    std::fs::create_dir_all(root.join("attachments")).unwrap();
    let r = root.clone();
    let mut h = Harness::with(|s| {
        s.config.approved_roots = vec![r.clone()];
        s.config.attachment_dir = Some(r.join("attachments"));
    });
    h.dir = Some(dir);
    let said =
        |h: &Harness, kind: ResourceKind, id: &str| describe_resource(h.conn(), kind, id).unwrap();

    // An attempt: the assessment it is for, its mode and state, and the latest grade (none yet).
    seed_blind_practical(&h, 1, Some(3));
    let attempt = h
        .call("assessment.begin", json!({ "assessment_id": "ad-w1" }))
        .unwrap()["result"]["attempt_id"]
        .as_str()
        .unwrap()
        .to_string();
    let text = said(&h, ResourceKind::Attempt, &attempt).unwrap();
    assert!(text.starts_with("an attempt at the assessment “"), "{text}");
    assert!(
        text.ends_with(" exam mode, started; no grade recorded"),
        "{text}"
    );

    // An event: its title, kind, and when.
    let start = h.now().as_millis() + 2 * 3_600_000;
    let created = h
        .call(
            "calendar.create_event",
            json!({ "title": "Lab review", "kind": "APPOINTMENT", "start": start, "end": start + 3_600_000 }),
        )
        .unwrap();
    let event = created["result"]["event_id"].as_str().unwrap().to_string();
    let text = said(&h, ResourceKind::CalendarEvent, &event).unwrap();
    assert!(
        text.starts_with("the calendar event “Lab review” (appointment), "),
        "{text}"
    );
    assert!(text.contains(" to "), "{text}");

    // A file: its name, size, class and the note it is on, and that an erasure of it names it too.
    let id_note = note(&mut h, "Lab notes", "INTERNAL");
    let attached = h
        .call(
            "evidence.attach_file",
            json!({ "note_id": id_note, "filename": "a.txt", "media_type": "text/plain", "content_base64": "aGVsbG8=" }),
        )
        .unwrap();
    let file = attached["result"]["attachment_id"]
        .as_str()
        .unwrap()
        .to_string();
    let expected = "the file “a.txt” (5 bytes, internal) of the note “Lab notes”";
    assert_eq!(
        said(&h, ResourceKind::Attachment, &file).as_deref(),
        Some(expected)
    );
    let erase = h
        .call(
            "deletion.request",
            json!({ "resource": { "kind": "ATTACHMENT", "id": file }, "reason": "no longer needed" }),
        )
        .unwrap_err()
        .approval_id
        .unwrap();
    assert_eq!(shown_for(&mut h, &erase).subject, expected);

    // A curriculum version: its label, where it came from, and whether it is in use.
    h.approved(
        "curriculum.import",
        json!({ "manifest": seed_manifest(), "provenance": "PROPOSED_SEED" }),
        &Opts::default(),
    );
    let version: String = h
        .conn()
        .query_row("SELECT id FROM curriculum_versions LIMIT 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    let text = said(&h, ResourceKind::CurriculumVersion, &version).unwrap();
    assert!(
        text.starts_with("the stored curriculum version “"),
        "{text}"
    );
    assert!(text.contains("proposed seed"), "{text}");

    // A record that is not stored says nothing; an erased one says so.
    for kind in [
        ResourceKind::Note,
        ResourceKind::Attachment,
        ResourceKind::CalendarEvent,
        ResourceKind::Attempt,
        ResourceKind::CurriculumVersion,
    ] {
        assert_eq!(said(&h, kind, "no-such-record"), None, "{kind:?}");
    }
    h.approved(
        "deletion.request",
        json!({ "resource": { "kind": "NOTE", "id": id_note }, "reason": "no longer needed" }),
        &Opts::default(),
    );
    assert_eq!(
        said(&h, ResourceKind::Note, &id_note).as_deref(),
        Some("a note that is already erased")
    );
    assert_eq!(
        said(&h, ResourceKind::Attachment, &file).as_deref(),
        Some("a file that is already erased")
    );
}

#[test]
fn a_record_that_does_not_exist_yet_is_said_to_be_new_and_what_the_approval_grants_is_shown() {
    let mut h = Harness::new();
    let id = pending_approval(&mut h);
    let summary = shown_for(&mut h, &id);
    assert_eq!(
        summary.subject,
        "a new note that is not stored yet; its content is fixed by the fingerprint below"
    );
    // Policy decided what the approval grants; the requester did not.
    assert_eq!(summary.grants, "protected record write");
    // The request's own context is labelled as that: the note is security sensitive,
    // the request was made from the learning area, and the text does not confuse them.
    assert_eq!(summary.scope, "the learning area, study mode");
    let text = summary.text();
    assert!(
        text.contains(
            "\nGrants: protected record write\nAsked in: the learning area, study mode\n"
        ),
        "{text}"
    );
    assert!(text.contains("Record: NOTE with the id \""), "{text}");
}

#[test]
fn an_export_confirmation_says_which_areas_leave_and_whether_private_records_do() {
    let mut h = Harness::new();
    let id = h
        .call(
            "export.request",
            json!({ "domains": ["LEARNING", "SYSTEM"], "format": "CSV", "include_private": true }),
        )
        .unwrap_err()
        .approval_id
        .unwrap();
    let summary = shown_for(&mut h, &id);
    assert_eq!(
        summary.subject,
        "an export of the areas learning, system as CSV; private records included, secret records never"
    );
}

#[test]
fn a_request_the_boundary_refuses_is_never_shown_to_the_operator() {
    // Final security review (ipc hardening): the native dialog was raised before
    // the meta, the payload cap, the schema and the name checks, so the operator
    // was asked about a request that was then refused (and the shell held the
    // backend while the dialog waited).
    let mut h = Harness::new();
    let id = pending_approval(&mut h);
    let mut shown = 0;

    let mut extra = approve(&id);
    extra["unexpected"] = json!(1);
    let refused = h
        .confirmed_with("approvals.decide", extra, &mut |_| {
            shown += 1;
            true
        })
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::InvalidInput, "{refused:?}");

    let meta = h.meta("notes.list", &Opts::default());
    let refused =
        g9_app::dispatch_confirming(h.ctx(), "approvals.decide", meta, approve(&id), &mut |_| {
            shown += 1;
            true
        })
        .unwrap_err();
    assert_eq!(refused.category, ErrorCategory::InvalidInput, "{refused:?}");

    assert_eq!(
        shown, 0,
        "the dialog was shown for a request the boundary refuses"
    );
    assert_eq!(state_of(&h, &id), "PENDING");
}
