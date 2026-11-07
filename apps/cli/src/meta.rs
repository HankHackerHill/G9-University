//! Wire metadata exactly as the UI client builds it
//! (`apps/desktop/src/ipc/client.ts` `buildMeta`): `request_id`,
//! `idempotency_key` and `trace_id` are uuid v7 unless supplied, the payload
//! schema version is `1.0`, the scope defaults to the operation's default
//! domain/mode from `apps/desktop/src/ipc/operations.ts`, `issued_at` comes
//! from the clock, and there is **no actor** (ADR-0013, Addendum A.8: the
//! backend assigns it and refuses any `actor` key).

use g9_domain::UtcMillis;
use serde_json::{json, Value};

/// Per-operation client defaults (mirrors `OPERATIONS` in `operations.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpDefaults {
    /// Default privacy domain (wire spelling).
    pub domain: &'static str,
    /// Default interaction mode (wire spelling).
    pub mode: &'static str,
    /// Whether the backend policy table answers `APPROVAL_REQUIRED` for an
    /// operator without a presented approval (`crates/g9-app/src/lib.rs`
    /// "Authority"; the UI table omits `curriculum.import`, the backend gates it).
    pub approval_expected: bool,
}

/// Approval-gated operations as enforced by `g9-app`.
pub const APPROVAL_GATED: &[&str] = &[
    "curriculum.import",
    "curriculum.promote",
    "calendar.apply",
    "assessment.approve_final",
    "export.request",
    "deletion.request",
];

/// Client defaults for `op` (unknown operations get the generic default; the
/// backend decides whether the operation exists).
pub fn op_defaults(op: &str) -> OpDefaults {
    let (domain, mode) = match op {
        "system.status" | "approvals.list" | "approvals.decide" | "audit.recent"
        | "export.request" => ("SYSTEM", "STUDY"),
        "backup.create" | "backup.verify" | "backup.restore_test" => ("SYSTEM", "MAINTENANCE"),
        "curriculum.import" | "curriculum.promote" => ("LEARNING", "MAINTENANCE"),
        "assessment.begin" | "assessment.submit" => ("LEARNING", "EXAM"),
        "assessment.list"
        | "assessment.review_evidence"
        | "assessment.grade_provisional"
        | "assessment.approve_final" => ("LEARNING", "REVIEW"),
        _ => ("LEARNING", "STUDY"),
    };
    OpDefaults {
        domain,
        mode,
        approval_expected: APPROVAL_GATED.contains(&op),
    }
}

/// Where request/idempotency/trace ids come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdSource {
    /// uuid v7 (the UI client's `crypto.randomUUID()` counterpart).
    Uuid,
    /// `<prefix>-<kind>-<n>` counters (deterministic scenario runs).
    Sequential {
        /// Id prefix.
        prefix: String,
        /// Next counter value.
        next: u64,
    },
}

impl IdSource {
    /// Deterministic ids with `prefix`.
    pub fn sequential(prefix: impl Into<String>) -> Self {
        IdSource::Sequential {
            prefix: prefix.into(),
            next: 1,
        }
    }

    /// Fresh id of the given kind (`req`, `idem`, `trace`).
    pub fn next_id(&mut self, kind: &str) -> String {
        match self {
            IdSource::Uuid => uuid::Uuid::now_v7().to_string(),
            IdSource::Sequential { prefix, next } => {
                let n = *next;
                *next += 1;
                format!("{prefix}-{kind}-{n:06}")
            }
        }
    }
}

/// Optional metadata fields of one call.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetaOptions {
    /// Approval presented for an approval-gated retry.
    pub approval_id: Option<String>,
    /// Optimistic-concurrency revision.
    pub expected_revision: Option<u64>,
    /// Privacy domain override (wire spelling).
    pub domain: Option<String>,
    /// Interaction mode override (wire spelling).
    pub mode: Option<String>,
    /// Idempotency key (replays return the stored outcome).
    pub idempotency_key: Option<String>,
    /// Trace id spanning several commands.
    pub trace_id: Option<String>,
}

impl MetaOptions {
    /// Present an approval.
    pub fn approval(id: impl Into<String>) -> Self {
        Self {
            approval_id: Some(id.into()),
            ..Self::default()
        }
    }

    /// Expect a revision.
    pub fn revision(r: u64) -> Self {
        Self {
            expected_revision: Some(r),
            ..Self::default()
        }
    }

    /// Override the interaction mode.
    pub fn mode(mode: &str) -> Self {
        Self {
            mode: Some(mode.to_string()),
            ..Self::default()
        }
    }
}

/// Build the wire `ClientCommandMeta` for `op`.
pub fn build_meta(op: &str, opts: &MetaOptions, ids: &mut IdSource, issued_at: UtcMillis) -> Value {
    let defaults = op_defaults(op);
    let mut meta = json!({
        "request_id": ids.next_id("req"),
        "operation": op,
        "idempotency_key": opts.idempotency_key.clone().unwrap_or_else(|| ids.next_id("idem")),
        "scope": {
            "domain": opts.domain.clone().unwrap_or_else(|| defaults.domain.to_string()),
            "mode": opts.mode.clone().unwrap_or_else(|| defaults.mode.to_string()),
        },
        "payload_schema_version": { "major": 1, "minor": 0 },
        "trace_id": opts.trace_id.clone().unwrap_or_else(|| ids.next_id("trace")),
        "issued_at": issued_at.as_millis(),
    });
    if let Some(a) = &opts.approval_id {
        meta["approval_id"] = json!(a);
    }
    if let Some(r) = opts.expected_revision {
        meta["expected_revision"] = json!(r);
    }
    meta
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_mirror_the_ui_operation_table() {
        assert_eq!(op_defaults("session.start").domain, "LEARNING");
        assert_eq!(op_defaults("session.start").mode, "STUDY");
        assert_eq!(op_defaults("system.status").domain, "SYSTEM");
        assert_eq!(op_defaults("backup.create").mode, "MAINTENANCE");
        assert_eq!(op_defaults("curriculum.import").mode, "MAINTENANCE");
        assert_eq!(op_defaults("assessment.begin").mode, "EXAM");
        assert_eq!(op_defaults("assessment.approve_final").mode, "REVIEW");
        assert_eq!(op_defaults("export.request").domain, "SYSTEM");
        for op in APPROVAL_GATED {
            assert!(op_defaults(op).approval_expected, "{op}");
        }
        assert!(!op_defaults("session.start").approval_expected);
        assert!(!op_defaults("calendar.propose").approval_expected);
    }

    #[test]
    fn meta_has_no_actor_and_generates_v7_ids() {
        let mut ids = IdSource::Uuid;
        let m = build_meta(
            "session.start",
            &MetaOptions::default(),
            &mut ids,
            UtcMillis::from_millis(42),
        );
        let obj = m.as_object().unwrap();
        assert!(!obj.contains_key("actor"));
        assert!(!obj.contains_key("approval_id"));
        assert!(!obj.contains_key("expected_revision"));
        assert_eq!(m["operation"], "session.start");
        assert_eq!(m["issued_at"], 42);
        assert_eq!(m["payload_schema_version"], serde_json::json!({ "major": 1, "minor": 0 }));
        assert_eq!(m["scope"]["domain"], "LEARNING");
        assert_eq!(m["scope"]["mode"], "STUDY");
        for key in ["request_id", "idempotency_key", "trace_id"] {
            let id = uuid::Uuid::parse_str(m[key].as_str().unwrap()).unwrap();
            assert_eq!(id.get_version_num(), 7, "{key}");
        }
        assert_ne!(m["request_id"], m["idempotency_key"]);
        // The backend accepts this shape (the schema the UI is built against).
        g9_app::schemas::validate_meta(&m).unwrap();
    }

    #[test]
    fn overrides_and_sequential_ids_are_applied() {
        let mut ids = IdSource::sequential("sim");
        let opts = MetaOptions {
            approval_id: Some("apr-1".into()),
            expected_revision: Some(3),
            domain: Some("SYSTEM".into()),
            mode: Some("EXAM".into()),
            idempotency_key: Some("k-1".into()),
            trace_id: Some("t-1".into()),
        };
        let m = build_meta("skills.invoke", &opts, &mut ids, UtcMillis::from_millis(7));
        assert_eq!(m["approval_id"], "apr-1");
        assert_eq!(m["expected_revision"], 3);
        assert_eq!(m["scope"]["domain"], "SYSTEM");
        assert_eq!(m["scope"]["mode"], "EXAM");
        assert_eq!(m["idempotency_key"], "k-1");
        assert_eq!(m["trace_id"], "t-1");
        assert_eq!(m["request_id"], "sim-req-000001");
        let again = build_meta(
            "skills.invoke",
            &MetaOptions::default(),
            &mut ids,
            UtcMillis::from_millis(7),
        );
        assert_eq!(again["request_id"], "sim-req-000002");
        assert_eq!(again["idempotency_key"], "sim-idem-000003");
        assert_eq!(again["trace_id"], "sim-trace-000004");
        g9_app::schemas::validate_meta(&again).unwrap();
    }
}
