//! Benchmark protocol description (handoff §13 "Mandatory benchmark data";
//! `docs/evaluations/model-benchmark-protocol.md`; corrections C06/C07).
//!
//! This module *describes what must be measured*. It contains **no** measured
//! or target performance number: the v5.1 "1,000 tokens < 300 ms" style claims
//! were removed (C06) and no model is pre-selected (C07). Numbers come from
//! running the protocol at Stage 06 and are recorded in `model_evals`, never
//! here. [`BenchmarkProtocol::to_markdown`] renders the table used by docs.

use serde::{Deserialize, Serialize};

/// One mandatory measurement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measurement {
    /// Stable identifier, e.g. `ttft`.
    pub id: String,
    /// Human name of the metric.
    pub metric: String,
    /// How it is measured (procedure, not a result).
    pub method: String,
    /// Fixture or workload it is measured on.
    pub fixture: String,
}

impl Measurement {
    fn new(id: &str, metric: &str, method: &str, fixture: &str) -> Self {
        Self {
            id: id.into(),
            metric: metric.into(),
            method: method.into(),
            fixture: fixture.into(),
        }
    }
}

/// The mandatory measurement set, per candidate × adapter × quantization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkProtocol {
    /// Protocol title.
    pub title: String,
    /// Mandatory measurements in document order.
    pub measurements: Vec<Measurement>,
    /// Conditions every run must record so that results are comparable.
    pub run_conditions: Vec<String>,
    /// Ordering rule for selection; the numeric eligibility thresholds live in
    /// the protocol document, not in code.
    pub priority_rule: String,
}

impl BenchmarkProtocol {
    /// The mandatory protocol as defined by handoff §13 and the evaluations doc.
    pub fn mandatory() -> Self {
        let measurements = vec![
            Measurement::new(
                "ttft",
                "Time to first token (TTFT)",
                "median and p95 over repeated warm runs; plus cold start",
                "fixed-length prompt from the lesson fixture set",
            ),
            Measurement::new(
                "prefill_throughput",
                "Prompt processing (prefill) throughput, tokens per second",
                "measured at short, medium and long context lengths",
                "synthetic and real lesson context",
            ),
            Measurement::new(
                "output_throughput",
                "Output throughput, tokens per second",
                "measured at small, medium and large output sizes",
                "same prompts as prefill",
            ),
            Measurement::new(
                "latency_p50_p95",
                "Latency p50 and p95 by output size",
                "end-to-end wall time distribution per output size class",
                "same prompts as prefill",
            ),
            Measurement::new(
                "memory_residency",
                "VRAM and RAM residency",
                "runtime-reported plus host sampling at the maximum context",
                "max-context prompt",
            ),
            Measurement::new(
                "cold_start",
                "Cold start",
                "process start to first token over repeated runs",
                "fixed prompt",
            ),
            Measurement::new(
                "crash_recovery",
                "Crash, GPU/driver error and recovery",
                "kill the runtime mid-generation; the app must return RESOURCE_UNAVAILABLE and recover with no data change",
                "adapter test",
            ),
            Measurement::new(
                "cancellation",
                "Cancellation",
                "cancel mid-output; resources released promptly (measured); late response discarded",
                "adapter test",
            ),
            Measurement::new(
                "structured_validity",
                "Structured-output validity",
                "share of responses valid against the skill schema, with and without constrained decoding",
                "prompt set spanning the runtime skills",
            ),
            Measurement::new(
                "context_truncation",
                "Context truncation behaviour",
                "detection when context exceeds the window; must be reported, never silent",
                "assembler test",
            ),
            Measurement::new(
                "correctness",
                "Knowledge and teaching correctness",
                "known-answer technical fixtures and expert-rubric scoring of generated lessons",
                "frozen eval fixtures",
            ),
            Measurement::new(
                "security_regressions",
                "Security regressions",
                "adversarial suite with the model in the loop; zero failures required",
                "adversarial fixtures",
            ),
            Measurement::new(
                "energy_thermal",
                "Energy and thermal impact",
                "power draw sampling during a mixed workload",
                "host tools",
            ),
        ];
        Self {
            title: "Local model and runtime benchmark protocol".into(),
            measurements,
            run_conditions: vec![
                "host and driver versions".into(),
                "runtime adapter and version".into(),
                "model id and artifact checksum".into(),
                "quantization".into(),
                "context length".into(),
                "date of the run".into(),
            ],
            priority_rule:
                "Quality and privacy outrank maximal tokens per second; eligibility thresholds are defined in docs/evaluations/model-benchmark-protocol.md."
                    .into(),
        }
    }

    /// Markdown rendering for `docs/` (a register table plus run conditions).
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("## {}\n\n", self.title));
        out.push_str("| Id | Metric | Method | Fixture |\n|---|---|---|---|\n");
        for m in &self.measurements {
            out.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                m.id, m.metric, m.method, m.fixture
            ));
        }
        out.push_str("\nEvery run records: ");
        out.push_str(&self.run_conditions.join("; "));
        out.push_str(".\n\n");
        out.push_str(&self.priority_rule);
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNITS: [&str; 7] = ["ms", "tok/s", "tokens/s", "GB", "MB", "W", "%"];

    /// True if `s` contains a digit followed (after optional spaces) by a unit.
    fn has_unit_number(s: &str) -> bool {
        let bytes = s.as_bytes();
        for (i, b) in bytes.iter().enumerate() {
            if !b.is_ascii_digit() {
                continue;
            }
            let mut j = i + 1;
            while j < bytes.len() && bytes[j] == b' ' {
                j += 1;
            }
            let rest = &s[j..];
            if UNITS.iter().any(|u| rest.starts_with(u)) {
                return true;
            }
        }
        false
    }

    #[test]
    fn protocol_lists_every_mandatory_metric_and_no_performance_numbers() {
        let p = BenchmarkProtocol::mandatory();
        let ids: Vec<&str> = p.measurements.iter().map(|m| m.id.as_str()).collect();
        for required in [
            "ttft",
            "prefill_throughput",
            "output_throughput",
            "latency_p50_p95",
            "memory_residency",
            "cold_start",
            "crash_recovery",
            "cancellation",
            "structured_validity",
            "context_truncation",
            "correctness",
            "security_regressions",
            "energy_thermal",
        ] {
            assert!(ids.contains(&required), "missing {required}");
        }
        let md = p.to_markdown();
        assert!(md.contains("| `ttft` |"));
        assert!(!has_unit_number(&md), "performance number found:\n{md}");
        let json = serde_json::to_string(&p).unwrap();
        assert!(!has_unit_number(&json));
        // No obsolete / pre-selected model names (C07).
        for name in ["Llama 2", "Vicuna", "llama-2", "vicuna"] {
            assert!(!md.contains(name));
        }
        assert!(has_unit_number("1000 tokens in 300 ms"));
        assert!(has_unit_number("98 %"));
        assert!(!has_unit_number("p50/p95"));
    }
}
