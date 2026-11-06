//! Scenario reports and their Markdown rendering.
//!
//! A [`ScenarioReport`] is the evidence of one scenario: every step with the
//! decision the backend recorded and the final backend state read back after
//! the step, plus the checks evaluated against that state. `passed` is
//! computed, never asserted: it is `true` only when at least one check ran and
//! every check passed (handoff §3 Audit B: "Every test must capture the actual
//! decision and final backend outcome").

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// One step of a scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioStep {
    /// IPC operation name, or a `harness:`/`storage:`/`registry:`/`orchestrator:`
    /// prefixed label for steps that do not go through `dispatch`.
    pub op: String,
    /// Request id sent to the backend (dispatch steps only).
    pub request_id: Option<String>,
    /// Decision recorded by the backend for this request (see
    /// [`crate::StepOutcome::decision`]); for non-dispatch steps, the boundary's
    /// verdict (`REFUSED`, `ACCEPTED`, …).
    pub decision: String,
    /// `OK` or the closed error category.
    pub outcome_category: String,
    /// Machine error code, when the step failed.
    pub error_code: Option<String>,
    /// Backend state read after the step (metadata only; never note bodies).
    pub state_after: Value,
}

/// One evaluated check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceCheck {
    /// What was checked.
    pub check: String,
    /// Expected value.
    pub expected: Value,
    /// Observed value.
    pub actual: Value,
    /// Whether the observation matched.
    pub passed: bool,
}

/// Evidence of one scenario.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioReport {
    /// Scenario id (`ST01`, `A04-01`, …).
    pub id: String,
    /// Gate the scenario evidences (`A01`…`A11`).
    pub gate: String,
    /// One-line title.
    pub title: String,
    /// Steps in execution order.
    pub steps: Vec<ScenarioStep>,
    /// `true` iff at least one check ran and all passed.
    pub passed: bool,
    /// Checks against recorded decisions and backend state.
    pub evidence: Vec<EvidenceCheck>,
    /// Scope notes: what this scenario does not cover, wave-1 gaps observed.
    pub notes: Vec<String>,
}

impl ScenarioReport {
    /// Empty report.
    pub fn new(id: impl Into<String>, gate: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            gate: gate.into(),
            title: title.into(),
            steps: Vec::new(),
            passed: false,
            evidence: Vec::new(),
            notes: Vec::new(),
        }
    }

    /// Append a step.
    pub fn push_step(&mut self, step: ScenarioStep) {
        self.steps.push(step);
    }

    /// Append a step that did not go through `dispatch`.
    pub fn boundary_step(
        &mut self,
        op: impl Into<String>,
        decision: impl Into<String>,
        outcome_category: impl Into<String>,
        error_code: Option<String>,
        state_after: Value,
    ) {
        self.steps.push(ScenarioStep {
            op: op.into(),
            request_id: None,
            decision: decision.into(),
            outcome_category: outcome_category.into(),
            error_code,
            state_after,
        });
    }

    /// Record a boolean check.
    pub fn check(
        &mut self,
        check: impl Into<String>,
        passed: bool,
        expected: Value,
        actual: Value,
    ) {
        self.evidence.push(EvidenceCheck {
            check: check.into(),
            expected,
            actual,
            passed,
        });
    }

    /// Record an equality check (values compared after JSON serialization).
    pub fn check_eq<T: Serialize>(&mut self, check: impl Into<String>, expected: T, actual: T) {
        let e = serde_json::to_value(&expected).unwrap_or(Value::Null);
        let a = serde_json::to_value(&actual).unwrap_or(Value::Null);
        let passed = e == a;
        self.check(check, passed, e, a);
    }

    /// Record a predicate over an observed value.
    pub fn check_that<T: Serialize>(
        &mut self,
        check: impl Into<String>,
        expected: &str,
        actual: T,
        passed: bool,
    ) {
        let a = serde_json::to_value(&actual).unwrap_or(Value::Null);
        self.check(check, passed, Value::String(expected.to_string()), a);
    }

    /// Add a scope note.
    pub fn note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }

    /// Compute `passed` and return the report.
    #[must_use]
    pub fn finish(mut self) -> Self {
        self.passed = !self.evidence.is_empty() && self.evidence.iter().all(|c| c.passed);
        self
    }

    /// Steps whose operation is `op`, in execution order.
    pub fn steps_for(&self, op: &str) -> Vec<&ScenarioStep> {
        self.steps.iter().filter(|s| s.op == op).collect()
    }

    /// The check with this exact label, if it ran.
    pub fn evidence_for(&self, check: &str) -> Option<&EvidenceCheck> {
        self.evidence.iter().find(|c| c.check == check)
    }

    /// Checks that failed.
    pub fn failed_checks(&self) -> Vec<&EvidenceCheck> {
        self.evidence.iter().filter(|c| !c.passed).collect()
    }

    /// `PASS` or `FAIL`.
    pub fn verdict(&self) -> &'static str {
        if self.passed {
            "PASS"
        } else {
            "FAIL"
        }
    }

    /// Markdown section for this scenario.
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "### {} — {} ({})\n",
            self.id,
            cell(&self.title),
            self.verdict()
        );
        let targets = test_targets(&self.id);
        let _ = writeln!(
            out,
            "Gate: {} · Test: {}\n",
            self.gate,
            if targets.is_empty() {
                "—".to_string()
            } else {
                targets
                    .iter()
                    .map(|t| format!("`{t}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        );
        let _ = writeln!(out, "| # | Operation | Decision | Outcome | Code |");
        let _ = writeln!(out, "|---|---|---|---|---|");
        for (i, s) in self.steps.iter().enumerate() {
            let _ = writeln!(
                out,
                "| {} | `{}` | {} | {} | {} |",
                i + 1,
                cell(&s.op),
                cell(&s.decision),
                cell(&s.outcome_category),
                s.error_code
                    .as_deref()
                    .map(|c| format!("`{}`", cell(c)))
                    .unwrap_or_else(|| "—".to_string())
            );
        }
        let _ = writeln!(out, "\n| Check | Expected | Actual | Result |");
        let _ = writeln!(out, "|---|---|---|---|");
        for c in &self.evidence {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} |",
                cell(&c.check),
                cell(&compact(&c.expected)),
                cell(&compact(&c.actual)),
                if c.passed { "PASS" } else { "FAIL" }
            );
        }
        if !self.notes.is_empty() {
            let _ = writeln!(out);
            for n in &self.notes {
                let _ = writeln!(out, "- {}", n.replace(['\n', '\r'], " "));
            }
        }
        out
    }
}

/// Escape a Markdown table cell (pipes, newlines) and bound its length.
fn cell(s: &str) -> String {
    let flat: String = s
        .replace('|', "\\|")
        .replace(['\n', '\r'], " ")
        .chars()
        .take(240)
        .collect();
    flat
}

fn compact(v: &Value) -> String {
    let text = match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    };
    scrub_uuids(&text)
}

fn is_uuid_at(bytes: &[u8], start: usize) -> bool {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let mut pos = start;
    for (i, len) in GROUPS.iter().enumerate() {
        let Some(group) = bytes.get(pos..pos + len) else {
            return false;
        };
        if !group.iter().all(u8::is_ascii_hexdigit) {
            return false;
        }
        pos += len;
        if i < GROUPS.len() - 1 {
            if bytes.get(pos) != Some(&b'-') {
                return false;
            }
            pos += 1;
        }
    }
    // Not the middle of a longer hex run.
    !bytes.get(pos).is_some_and(u8::is_ascii_hexdigit)
        && !(start > 0 && bytes.get(start - 1).is_some_and(u8::is_ascii_hexdigit))
}

/// Replace generated UUIDs (backend-assigned ids, UUIDv7 with a wall-clock
/// component) by `<uuid>` so that the rendered evidence is identical across
/// runs. Equality of ids is still checked on the raw values.
pub fn scrub_uuids(text: &str) -> String {
    const UUID_LEN: usize = 36;
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if is_uuid_at(bytes, i) {
            out.push_str("<uuid>");
            i += UUID_LEN;
        } else {
            // Copy one UTF-8 character.
            let ch_len = text
                .get(i..)
                .and_then(|rest| rest.chars().next())
                .map_or(1, char::len_utf8);
            out.push_str(text.get(i..i + ch_len).unwrap_or_default());
            i += ch_len;
        }
    }
    out
}

/// Static description of a gate for the evidence document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateInfo {
    /// Gate id.
    pub gate: &'static str,
    /// What the gate requires (ARCHITECTURE.md §14).
    pub requirement: &'static str,
}

/// Gates exercised by this harness, in order.
pub const GATES: &[GateInfo] = &[
    GateInfo {
        gate: "A01",
        requirement: "curriculum invariants / provenance",
    },
    GateInfo {
        gate: "A03",
        requirement: "timer discontinuity (recovery path)",
    },
    GateInfo {
        gate: "A04",
        requirement: "finalized grade immutability",
    },
    GateInfo {
        gate: "A05",
        requirement: "competency replay/correction",
    },
    GateInfo {
        gate: "A06",
        requirement: "policy denies unapproved actions",
    },
    GateInfo {
        gate: "A07",
        requirement: "malicious note/skill cannot grant privilege",
    },
    GateInfo {
        gate: "A08",
        requirement: "backup/restore/recovery",
    },
    GateInfo {
        gate: "A09",
        requirement: "model-down operation",
    },
    GateInfo {
        gate: "A10",
        requirement: "export/deletion propagation",
    },
    GateInfo {
        gate: "A11",
        requirement: "output validation",
    },
];

const ST_CATALOGUE_TARGET: &str = "tests/adversarial/st_adversarial_catalogue.rs";

/// Test files (relative to the repository root) that run a scenario through
/// `cargo test -p g9-evaluation`.
pub fn test_targets(scenario_id: &str) -> Vec<&'static str> {
    let gate_file = match scenario_id.get(..3) {
        Some("A01") => Some("tests/integration/a01_gate.rs"),
        Some("A04" | "A05") => Some("tests/integration/a04_a05_gate.rs"),
        Some("A06") => Some("tests/integration/a06_a07_gate.rs"),
        Some("A08") => Some("tests/recovery/a08_recovery_drill.rs"),
        Some("A09") => Some("tests/integration/a09_model_down.rs"),
        Some("A10") => Some("tests/integration/a10_export_deletion.rs"),
        Some("A11") => Some("tests/integration/a11_output_validation.rs"),
        _ => None,
    };
    match (gate_file, scenario_id) {
        (Some(f), _) => vec![f],
        // The A07 claims (malicious note, tampered package, hostile output)
        // are also asserted by the A06/A07 gate file.
        (None, "ST01" | "ST02" | "ST04") => {
            vec!["tests/integration/a06_a07_gate.rs", ST_CATALOGUE_TARGET]
        }
        // Budget and cancellation boundaries are also asserted by the A11 gate file.
        (None, "ST08" | "ST12") => {
            vec![
                "tests/integration/a11_output_validation.rs",
                ST_CATALOGUE_TARGET,
            ]
        }
        (None, id) if id.starts_with("ST") => vec![ST_CATALOGUE_TARGET],
        (None, _) => Vec::new(),
    }
}

/// Gates of ARCHITECTURE.md §14 this harness does not execute.
pub const GATES_NOT_RUN_HERE: &[(&str, &str)] = &[
    (
        "A02",
        "recurrence/DST/conflicts — g9-calendar fixtures (crate tests); NOT RUN by g9-evaluation",
    ),
    (
        "A12",
        "SBOM/licenses/pins/accessibility — CI (cargo deny, cargo audit, npm audit, axe); NOT RUN by g9-evaluation",
    ),
];

/// Render the gate evidence document (`docs/evaluations/gate-evidence.md`).
/// Deterministic: no wall-clock timestamp, ordered by gate then scenario id.
pub fn render_gate_evidence(reports: &[ScenarioReport]) -> String {
    let mut by_gate: BTreeMap<&str, Vec<&ScenarioReport>> = BTreeMap::new();
    for r in reports {
        for g in r.gate.split(',').map(str::trim) {
            by_gate.entry(g).or_default().push(r);
        }
    }
    let mut out = String::new();
    let _ = writeln!(out, "# Gate evidence (g9-evaluation harness)\n");
    let _ = writeln!(
        out,
        "Generated by the test `gate_evidence_document` in `crates/g9-evaluation/src/lib.rs`. Do not edit by hand.\n"
    );
    let _ = writeln!(out, "## How to regenerate\n");
    let _ = writeln!(out, "```sh");
    let _ = writeln!(
        out,
        "G9_WRITE_EVIDENCE=1 CARGO_TARGET_DIR=target/g9-evaluation \\\n  cargo test -p g9-evaluation --lib gate_evidence_document"
    );
    let _ = writeln!(out, "```\n");
    let _ = writeln!(
        out,
        "Without `G9_WRITE_EVIDENCE=1` the test renders and checks the document but writes nothing.\n"
    );
    let _ = writeln!(out, "## Conditions\n");
    let _ = writeln!(out, "| Input | Value |");
    let _ = writeln!(out, "|---|---|");
    let _ = writeln!(
        out,
        "| Entry point | `g9_app::dispatch` for every IPC step; boundary steps are labelled `registry:`, `storage:`, `orchestrator:`, `harness:` |"
    );
    let _ = writeln!(
        out,
        "| Database | in-memory SQLite; file database in a disposable directory for the recovery drill |"
    );
    let _ = writeln!(
        out,
        "| Clock | fixed start {} (2026-11-16T08:00:00Z), advanced explicitly |",
        crate::harness::T0
    );
    let _ = writeln!(
        out,
        "| Binding key | fixed 32-byte test key (not a production secret) |"
    );
    let _ = writeln!(
        out,
        "| Runtime | `ScriptedRuntime` (queued replies) or `NullRuntime` (A09) — no model is run |"
    );
    let _ = writeln!(
        out,
        "| Skills | the real packages under `.agents/skills` (validated by `Registry::load`) |"
    );
    let _ = writeln!(
        out,
        "| Fixtures | synthetic only (`data/fixtures/*.json`); no secrets, no personal data |"
    );
    let _ = writeln!(
        out,
        "| Decision column | read back from the audit row the backend wrote for the request; `NO_AUDIT_ROW` = none written (allowed reads, requests refused before the pipeline) |\n"
    );
    let _ = writeln!(out, "## Summary\n");
    let _ = writeln!(
        out,
        "| Gate | Requirement | Scenarios | Result | Test target |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|");
    for g in GATES {
        let list = by_gate.get(g.gate).cloned().unwrap_or_default();
        let ids: Vec<String> = list
            .iter()
            .map(|r| format!("{} {}", r.id, r.verdict()))
            .collect();
        let result = if list.is_empty() {
            "NOT RUN"
        } else if list.iter().all(|r| r.passed) {
            "PASS"
        } else {
            "FAIL"
        };
        let targets: std::collections::BTreeSet<&str> =
            list.iter().flat_map(|r| test_targets(&r.id)).collect();
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} |",
            g.gate,
            g.requirement,
            if ids.is_empty() {
                "—".to_string()
            } else {
                ids.join(", ")
            },
            result,
            if targets.is_empty() {
                "—".to_string()
            } else {
                targets
                    .iter()
                    .map(|t| format!("`{t}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        );
    }
    for (gate, why) in GATES_NOT_RUN_HERE {
        let _ = writeln!(out, "| {gate} | {why} | — | NOT RUN | — |");
    }
    let total = reports.len();
    let passed = reports.iter().filter(|r| r.passed).count();
    let _ = writeln!(
        out,
        "\nScenarios: {total}; PASS {passed}; FAIL {}.\n",
        total - passed
    );
    let _ = writeln!(out, "## Wave-1 gaps observed by the harness\n");
    let _ = writeln!(out, "| # | Gap |");
    let _ = writeln!(out, "|---|---|");
    for (i, gap) in crate::WAVE1_GAPS.iter().enumerate() {
        let _ = writeln!(out, "| G{} | {} |", i + 1, gap.replace('|', "\\|"));
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "## Scenarios\n");
    let mut sorted: Vec<&ScenarioReport> = reports.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    for r in sorted {
        let _ = writeln!(out, "{}", r.to_markdown());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn passed_requires_at_least_one_check_and_all_passing() {
        let r = ScenarioReport::new("X", "A01", "t").finish();
        assert!(!r.passed, "no evidence is not a pass");
        let mut r = ScenarioReport::new("X", "A01", "t");
        r.check_eq("a", 1, 1);
        assert!(r.clone().finish().passed);
        r.check_eq("b", 1, 2);
        let r = r.finish();
        assert!(!r.passed);
        assert_eq!(r.failed_checks().len(), 1);
    }

    #[test]
    fn markdown_escapes_cells_and_lists_steps() {
        let mut r = ScenarioReport::new("ST99", "A07", "pipe | title");
        r.boundary_step(
            "registry:load",
            "REFUSED",
            "PRECONDITION_FAILED",
            None,
            json!({}),
        );
        r.check_eq("x|y", "a\nb", "a\nb");
        let md = r.finish().to_markdown();
        assert!(md.contains("pipe \\| title"));
        assert!(md.contains("x\\|y"));
        assert!(md.contains("`registry:load`"));
        assert!(!md.contains("a\nb"));
    }

    #[test]
    fn every_scenario_family_maps_to_a_registered_test_file() {
        let manifest = include_str!("../Cargo.toml");
        for id in [
            "A01-01", "A04-01", "A05-01", "A06-03", "A08-01", "A09-01", "A10-02", "A11-01", "ST01",
            "ST07", "ST12",
        ] {
            let targets = test_targets(id);
            assert!(!targets.is_empty(), "{id} has no test target");
            for t in targets {
                assert!(
                    manifest.contains(&format!("path = \"../../{t}\"")),
                    "{t} is not a [[test]] of g9-evaluation"
                );
            }
        }
        assert!(test_targets("X99").is_empty());
    }

    #[test]
    fn uuids_are_scrubbed_from_rendered_values() {
        assert_eq!(
            scrub_uuids(r#"["0190f3a2-7c1d-7e2a-9b3c-1d2e3f405162","apr-x"]"#),
            r#"["<uuid>","apr-x"]"#
        );
        assert_eq!(
            scrub_uuids("no ids here: 1234-5678"),
            "no ids here: 1234-5678"
        );
        assert_eq!(
            scrub_uuids("é0190f3a2-7c1d-7e2a-9b3c-1d2e3f405162é"),
            "é<uuid>é"
        );
        let mut r = ScenarioReport::new("X", "A04", "t");
        r.check_eq(
            "linked approval",
            "0190f3a2-7c1d-7e2a-9b3c-1d2e3f405162",
            "0190f3a2-7c1d-7e2a-9b3c-1d2e3f405162",
        );
        let md = r.finish().to_markdown();
        assert!(md.contains("| linked approval | <uuid> | <uuid> | PASS |"));
    }

    #[test]
    fn report_round_trips_through_json() {
        let mut r = ScenarioReport::new("ST99", "A07", "t");
        r.boundary_step(
            "registry:load",
            "REFUSED",
            "PRECONDITION_FAILED",
            Some("c".into()),
            json!({ "k": 1 }),
        );
        r.check_eq("x", 1, 1);
        let r = r.finish();
        let text = serde_json::to_string(&r).unwrap();
        let back: ScenarioReport = serde_json::from_str(&text).unwrap();
        assert_eq!(back, r);
        assert!(text.contains("\"outcome_category\":\"PRECONDITION_FAILED\""));
    }

    #[test]
    fn evidence_document_marks_unrun_gates() {
        let mut r = ScenarioReport::new("A01-01", "A01", "t");
        r.check_eq("ok", true, true);
        let doc = render_gate_evidence(&[r.finish()]);
        assert!(doc.contains(
            "| A01 | curriculum invariants / provenance | A01-01 PASS | PASS | `tests/integration/a01_gate.rs` |"
        ));
        assert!(doc.contains("| A09 | model-down operation | — | NOT RUN |"));
        assert!(doc.contains("| A12 |"));
        assert!(doc.contains("G9_WRITE_EVIDENCE=1"));
    }
}
