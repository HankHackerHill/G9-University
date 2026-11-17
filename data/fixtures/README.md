# Evaluation fixtures (`data/fixtures`)

Synthetic data for `crates/g9-evaluation`. No real names, no personal data, no secrets, no real hosts (network examples use the 203.0.113.0/24 documentation range).

| File | Kind | Schema | Used by |
|---|---|---|---|
| `week03-learner-history.json` | `LEARNER_HISTORY` | `data/fixtures/schemas/learner-history.schema.json` | `g9_evaluation::fixtures::seed` (gates A04, A05, A06, A09, A10; ST03, ST05, ST09; A08/A03 recovery drill) |
| `adversarial-payloads.json` | `ADVERSARIAL_PAYLOADS` | `data/fixtures/schemas/adversarial-payloads.schema.json` | `g9_evaluation::adversarial` (ST01–ST12) |

Rules enforced by the loader (`crates/g9-evaluation/src/fixtures.rs`):

1. Every `*.json` file directly in this directory is a fixture; `fixture_kind` selects the schema.
2. The file is validated against its JSON Schema (draft 2020-12, embedded at compile time, no `$ref` resolution) **before** deserialization; unknown kinds and unknown fields are refused (`INVALID_INPUT fixture.*`).
3. `synthetic` must be `true`.
4. Records with an IPC creation path (notes, attempts, grades, approvals, session transitions) are created through `g9_app::dispatch`; records without one (competencies, assessment definitions/versions/rubrics, planned study sessions, competency observations) are inserted through the `g9-storage` repositories, which run the same domain validation (model-provenance observations are refused).
5. Loader tests (`crates/g9-evaluation/src/fixtures.rs`, `mod tests`): every file loads; unknown fields, `synthetic: false`, unknown kinds and non-JSON are refused with JSON-path-only messages; no secret markers; IPv4 literals only in 203.0.113.0/24.

Slice content (`week03-learner-history.json`): 3 study sessions (D13–D15; two completed, one paused), 2 notes, 2 attempts (one finalized through the operator approval flow, one left `GRADED_PROVISIONAL`), 4 competency observations, 2 competencies, 2 assessment definitions.
