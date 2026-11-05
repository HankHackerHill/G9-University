# G9 MENTOR — Architecture (v6.1, repository-controlling)

**Status:** DESIGN APPROVED FOR SCAFFOLD (Stage 00 → 01 → 02 in this repository). Supersedes the v5.1 plan and the v6.0 handoff where they conflict; every deviation is registered in `docs/audit/defect-register.md` and decided in `docs/adr/`.

**Evidence labels used in this repository:** VERIFIED (examined reliable evidence), OBSERVED (present in logs/tests run here), INFERRED, ASSUMED, PROPOSED (not implemented), TESTED (executed here), BLOCKED (missing gate), NOT RUN.

---

## 1. Product in one paragraph

G9 MENTOR is an offline-first, single-operator, local-first desktop system that joins a deterministic learning command center (curriculum, calendar, session timer, notes, evidence), an independent examiner (rubrics, blind attempts, reviewer-finalized grades), an evidence-centered competency model (four dimensions, replayable), a retention engine (fixed baseline schedule; FSRS optional for declarative items only), an adaptive AI teacher that can only *propose* (lessons, hints, diagnoses, interventions), an execution/decision workspace, and a research reference system with approved sources. The model never writes authoritative state; policy, scheduling, scoring, timers, storage and recovery are deterministic Rust. The application must remain fully usable with the model disabled (R-002, A09).

## 2. Dependency law

```
apps/desktop (React + strict TS)
   │  typed IPC DTOs (schemas/ipc/*.json, SchemaVersion)
   ▼
apps/desktop/src-tauri (thin Tauri 2 bridge; minimal capabilities; no raw native APIs)
   │
   ▼
crates/g9-app            command handlers / use cases (CommandMeta → CommandResult<T>)
   │           ┌──────────────┴──────────────┐
   ▼           ▼                             ▼
crates/g9-policy   domain crates (pure)   crates/g9-skills ── crates/g9-ai-adapter
 (deny-default)    curriculum, calendar,   (orchestrator)      (replaceable runtime)
                   evidence, competency,
                   assessment, retention
   │                      │
   ▼                      ▼
crates/g9-storage  (SQLite: migrations, repositories, backup, audit chain, idempotency)
   │
   ▼
crates/g9-domain   (ids, time, trust, privacy, envelope, policy types, state machines, invariants)
```

Rules (enforced by crate boundaries and reviewed in CI):

1. `g9-domain` depends on nothing internal. Pure domain crates depend only on `g9-domain`.
2. `g9-storage` depends on `g9-domain` and the pure domain crates (for row ↔ type mapping) — never on `g9-app`, `g9-skills` or `g9-ai-adapter`.
3. `g9-ai-adapter` never touches SQLite or native OS APIs. It receives an assembled, provenance-tagged context and returns validated structured output or text.
4. `g9-skills` depends on `g9-policy` and `g9-ai-adapter`; it produces `SkillOutputEnvelope` and `RecommendedAction`s. It never mutates storage.
5. `g9-app` is the only crate that composes policy + storage + skills; every mutation passes through its command pipeline (§5).
6. The UI never queries SQLite or spawns a model process; the Tauri layer exposes only `g9-app` commands.
7. No cyclic dependencies. `cargo tree` is checked in CI; a cycle is a build failure.

## 3. Module → crate mapping (the 17 authoritative modules)

| Module | Owner crate / path | Boundary enforced here |
|---|---|---|
| M01 Application Shell | `apps/desktop`, `apps/desktop/src-tauri` | capabilities file lists only `core:default` + app commands; no shell/fs/http plugins |
| M02 Curriculum | `crates/g9-curriculum` | import blocked on invariant/provenance failure; `ManifestProvenance` gate |
| M03 Calendar | `crates/g9-calendar::{civil, recurrence, conflicts, proposal}` | civil time + IANA zone; occurrences as `UtcMillis`; DST fixtures |
| M04 Session/Timer | `crates/g9-calendar::timer` | append-only interval ledger; no overlap; INTERRUPTED/UNCERTAIN on discontinuity |
| M05 Notes/Evidence | `crates/g9-evidence` | note revisions, content hashes, classification, provenance; render sanitization in UI |
| M06 Competency | `crates/g9-competency` | observations append-only; state derived by evidence rules; replayable |
| M07 Assessment | `crates/g9-assessment` | attempt state machine; `GradeSource::ModelAdvisory` can never finalize; sealed keys |
| M08 Retention | `crates/g9-retention` | fixed baseline first; `fsrs` feature optional and item-level only |
| M09 Learner Memory | `crates/g9-competency::memory` | hypotheses with provenance, conflicts, corrections, derived-state invalidation |
| M10 AI Gateway | `crates/g9-ai-adapter` | `InferenceRuntime` trait; budgets; cancellation; JSON-Schema output validation; `NullRuntime` |
| M11 Skills Orchestrator | `crates/g9-skills` | registry + `SkillInvocationState`; no recursion; typed envelopes |
| M12 Adaptive Teacher | `crates/g9-skills` (T/L skill packages) + `crates/g9-app::teaching` | proposals only; objective-linked; evidence-grounded |
| M13 Analytics | `crates/g9-app::analytics` (read models over storage) | reproducible SQL; no fabricated metrics |
| M14 Goals/Execution | `crates/g9-app::executive` + `g9-storage::repos::business` | business domain isolation (`DataDomain::Business`) |
| M15 Research | `crates/g9-app::research` + `g9-evidence::sources` | network off by default; `SourceState` machine |
| M16 Authorization | `crates/g9-policy` | deny-by-default; approval binding; decision log |
| M17 Recovery/Audit | `crates/g9-storage::{backup, audit, migrations}` | Online Backup API / VACUUM INTO; restore-tested; hash-chained metadata |

Stage ownership: Stages 01–02 (scaffold, storage, importer, backup) are implemented in this repository. Stages 03+ have contracts, schemas, tests and skill packages but are feature-flagged `PROPOSED` where not implemented. A `TODO` is never presented as done (handoff §21).

## 4. Shared vocabulary (`crates/g9-domain`)

- **Identifiers:** typed newtypes (`SessionId`, `AttemptId`, …); UUIDv7 for generated ids, validated slugs (`D01`, `g9-session-director`) for stable ids.
- **Time:** `UtcMillis` (i64 epoch ms) for instants; `DurationMs` (non-negative); `IanaZone` (validated); `CivilDateTime` for wall-clock intent. Elapsed time for timers comes from a monotonic clock, never from wall-clock subtraction alone.
- **Trust:** `TrustClass` (Operator, SystemDeterministic, ApprovedReference, ImportedDocument, SkillPackage, ModelOutput, Unknown). Only Operator/SystemDeterministic may be authoritative or carry instructions.
- **Privacy:** `DataDomain` (Learning, Business, Personal, SecuritySensitive, System) and `Classification` on every record that can reach the model or an export.
- **Envelope:** `CommandMeta` (request_id, actor, operation, resource, expected_revision, idempotency_key, scope, payload_schema_version, approval_id, trace_id, issued_at); `G9Error` with the closed 13-category set; `CommandOutcome<T>` with revision, audit_ref, replayed flag.
- **Policy types:** `Action`, `ResourceKind`, `Capability` (with `operator_only()`), `OperationDescriptor` (binds parameters by SHA-256), `Decision`.
- **State machines (§16 of the handoff):** Session, AssessmentAttempt, Intervention, SkillInvocation, ResearchSource, Backup, Competency, Approval — explicit transition tables with tests.
- **Curriculum invariants:** 12 × 6 × 3; 420/300 minutes; 2,400/week; 28,800 total; `ManifestProvenance` {ProposedSeed, ReconciledCandidate, ApprovedCanonical}.
- **Competency:** `LearningDimension` {Recall, Understanding, Application, Transfer}; `AssistanceLevel` with independence weights; `Difficulty` 1–5; `Confidence` [0,1] with Brier.
- **Assessment:** `BaselineRubric` (6 criteria, weights = 100, critical gate on scope compliance); `GradeSource` rule; `PASS_THRESHOLD = 80`.
- **Skills:** `SkillCode` (E01–E14, T01–T08, L01–L06, P01–P06, K01–K03 = 37), Agent Skills name validation, `ResourceBudget`, invocation/output envelopes.

## 5. Command pipeline (`crates/g9-app`)

Every mutating command executes exactly this sequence; read-only queries skip steps 4–9 but still pass step 3.

1. **Decode + schema check** — DTO deserialized with serde; `payload_schema_version` must be accepted by the handler's `SchemaVersion`; otherwise `INVALID_INPUT`.
2. **Idempotency lookup** — `(actor_id, idempotency_key)` in `idempotency_records`; a hit returns the stored outcome with `replayed = true` (same request hash) or `CONFLICT` (different request hash).
3. **Policy evaluation** — `OperationDescriptor { action, resource, scope, parameters_sha256 }` → `g9_policy::Engine::evaluate(actor, descriptor, mode)`. `Deny` → `UNAUTHORIZED`; `ApprovalRequired` without a valid approval → create/return `approval_id` with `APPROVAL_REQUIRED`.
4. **Approval consumption** — if `approval_id` is supplied: it must be `Approved`, unexpired, bound to the same `parameters_sha256`, same resource, same `expected_revision`; it is moved to `Consumed` inside the same transaction (TOCTOU-safe, single-use).
5. **Transaction** — `BEGIN IMMEDIATE`; `expected_revision` compared with the stored revision → `STALE_REVISION` on mismatch.
6. **Domain mutation** — pure domain function (state machine transition, invariant check) computes the new state; the handler persists it; revision increments.
7. **Significant event + audit metadata** — append to `domain_events` (when significant) and `audit_metadata` (always): hash chained (`prev_hash`, `row_hash`), metadata only (no raw prompts, no private content).
8. **Commit** and store the idempotency outcome.
9. **Return** `CommandOutcome<T>`; errors are mapped to the closed category set; internal causes are logged locally and redacted.

Skill invocations are commands too: `skills.invoke` runs the orchestrator (preflight → authorize → run → validate) and returns a `SkillOutputEnvelope`; any `RecommendedAction` becomes a *new* command that re-enters this pipeline at step 1 with the operator as actor and an explicit approval.

## 6. Storage design (`crates/g9-storage`)

- SQLite via `rusqlite` 0.41 (bundled). Per-connection: `PRAGMA foreign_keys = ON`, `busy_timeout = 5000`, `journal_mode = WAL` (chosen concurrency profile; not required for backup), `synchronous = NORMAL` in WAL, `temp_store = MEMORY`, `trusted_schema = OFF`.
- Tables are `STRICT` where types allow. Ids are `TEXT`; instants are `INTEGER` epoch milliseconds; durations are `INTEGER` ms; zones are `TEXT` IANA; booleans are `INTEGER` 0/1 with CHECK; enums are `TEXT` with CHECK lists mirroring the Rust enums; JSON payloads are `TEXT` validated with `json_valid()` CHECKs; every mutable row has `revision INTEGER NOT NULL DEFAULT 1`, `created_at`, `updated_at`.
- **Deletion semantics are explicit per relationship** (no blanket `ON DELETE CASCADE`): evidentiary tables (`attempts`, `criterion_results`, `grade_decisions`, `competency_observations`, `audit_metadata`, `approval_decisions`, `policy_decisions`, `domain_events`) use `ON DELETE RESTRICT`; structural children of a versioned manifest (`weeks`, `training_days`, `daily_objectives`, `study_blocks`) cascade *within a curriculum version only*; personal content (`notes`, `note_versions`, `attachments`, `opt_in_reflections`) supports erasure through `deletion_requests` which redacts content and keeps a tombstone with hash for audit continuity.
- **Migrations:** numbered SQL files embedded at compile time; `schema_migrations(version, name, sha256, applied_at, app_version)`; each migration runs in a transaction; checksums of applied migrations are verified on open (`DATA_CORRUPTION` on mismatch); `migrate --dry-run` executes against a `VACUUM INTO` copy first.
- **Backup (M17):** `Backup::new(src, dst).run_to_completion(...)` (Online Backup API) to a temporary file, then `PRAGMA integrity_check`, `PRAGMA foreign_key_check`, row-count/relationship invariants and attachment manifest check on the copy; `BackupState` is driven Requested → SnapshotCreated → Checked → RestoreTested → Retained. `VACUUM INTO` is the alternative for compact exports. Live file copy and `.dump` recipes are rejected by design.
- **Restore:** always into a disposable copy first; app/schema compatibility recorded in `backup_manifests`; forward migration only; never binary downgrade.
- **Audit chain:** `audit_metadata(id, prev_hash, row_hash, occurred_at, actor_id, actor_kind, operation, resource_kind, resource_id, decision, outcome, request_id, trace_id, details_sha256)`; `row_hash = sha256(prev_hash || canonical_json(row_without_hash))`; periodic anchors stored in `backup_manifests`.
- **Idempotency:** `idempotency_records(actor_id, idempotency_key, request_sha256, outcome_json, created_at)` with retention.
- **Privacy:** `data_domain` column on domain tables; repositories accept a `Scope` and refuse cross-domain reads unless `Capability::CrossDomainRead` was granted (A?/R-017).

## 7. Time, calendar and timer (`crates/g9-calendar`)

- Recurring events store `CivilDateTime` + `IanaZone` + RRULE-subset (FREQ=DAILY|WEEKLY, BYDAY, INTERVAL, UNTIL/COUNT). Occurrences are resolved to `UtcMillis`.
- DST policy: a nonexistent local time (spring-forward gap) resolves to the first valid instant after the gap and is flagged `Adjusted::Gap`; an ambiguous local time (fall-back overlap) resolves to the earlier (pre-transition) offset and is flagged `Adjusted::Overlap`; all-day events are date-only; 29 February recurrences skip non-leap years. Fixtures cover Europe/Berlin, America/New_York, Australia/Sydney, UTC.
- Conflict checker: exhaustive pairwise interval overlap on resolved occurrences within a range; protected rest and immutable appointments are hard constraints; the proposal object lists violated constraints explicitly (`Infeasible` is a first-class outcome, never silently dropped).
- Schedule priority (deterministic heuristic): immutable appointments/protected rest → safety and assessment deadlines → prerequisite readiness → due reviews → operator priorities → preferred workload.
- Timer ledger: `time_intervals(session_id, start_utc, end_utc, start_mono_ms, end_mono_ms, continuity)` where `continuity ∈ {VERIFIED, UNCERTAIN}`; an open interval at process start is closed `UNCERTAIN` and the session becomes `Interrupted`; overlapping intervals are rejected; corrections are new rows (`manual_time_corrections`) that reference and never overwrite the original.

## 8. Assessment, competency and memory

- **Attempt lifecycle:** `AssessmentAttemptState` (Available → Started → Submitted → EvidenceReview → GradedProvisional → ApprovedFinal; Voided). `ApprovedFinal` has no outgoing edges; a retake is a new attempt row with `retake_of`.
- **Grading:** `criterion_results(attempt_id, criterion_code, score, source ∈ {DETERMINISTIC_CHECKER, HUMAN_REVIEWER, MODEL_ADVISORY}, evidence_refs)`; a `grade_decisions` row is final only if every criterion has at least one non-advisory result and all critical gates pass; `g9_assessment::finalize` rejects otherwise. Hint use records `AssistanceLevel` on the attempt and the derived observations.
- **Sealed keys:** answer keys and reference solutions live in `assessment_versions.sealed_key_ref` (content-addressed, classification `Secret`); the context assembler refuses `Secret` content in `InteractionMode::Exam`; every exposure is logged in `exposure_log`.
- **Observations:** `competency_observations(competency_id, dimension, attempt_id|episode_id, difficulty, assistance, score, environment, reviewer, provenance, uncertainty, observed_at, supersedes)` — append-only; corrections supersede.
- **Derivation (replayable fold):** `derive_state(observations, rules) -> CompetencyProjection { state, per_dimension: {score_estimate, n_unaided, last_observed_at, uncertainty}, contradictions, evidence_refs }`. Default rules (configurable per competency): `Introduced` on any observation; `Guided` on ≥1 guided/hinted application observation ≥ 60; `Independent` on ≥2 unaided application observations ≥ 70 at difficulty ≥ 3 within 60 days; `Validated` on ≥1 blind-exam (EXAM mode) application/transfer observation ≥ 80; `RetestDue` when the last validated evidence is older than the competency's validity window; `Regress` when the two most recent unaided observations fall below threshold — earlier high results are retained, not erased.
- **Learner memory (M09):** `learner_hypotheses(statement, kind ∈ {MISCONCEPTION, STRENGTH, PREFERENCE, BLOCKER}, supporting_refs, contradicting_refs, confidence_low/high, status ∈ {PROPOSED, ACCEPTED, REJECTED, RETIRED}, provenance)`; model output can only create `PROPOSED`; operator acceptance/rejection is a command; derived plans referencing a rejected hypothesis are invalidated.

## 9. Retention (`crates/g9-retention`)

- Baseline schedule (default, configurable): review offsets 1, 3, 7, 21, 60 days after the last successful recall; a failed recall resets to offset 1; a bounded daily review-time budget (default 15 minutes inside the 15-minute recall block) caps the queue, overflow is reported, not silently dropped.
- `fsrs` feature (off by default) wraps `rs-fsrs` for **declarative items only**; desired retention is a parameter (0.90 candidate), not a mandate; enabling it is an operator decision after the evaluation in `docs/evaluations/fsrs-evaluation-protocol.md`.
- Practical transfer checks are separate `practical_transfer_checks` rows scheduled by competency validity windows, never by memory curves.

## 10. AI gateway and skills

- `InferenceRuntime` trait: `describe() -> RuntimeInfo`, `generate(InferenceRequest, CancelToken) -> Result<InferenceResponse, InferenceError>`; `InferenceRequest` carries `max_tokens`, `timeout`, `response_schema: Option<Value>`, context segments each tagged with `TrustClass` and `Classification`. `NullRuntime` implements model-down behavior (`RESOURCE_UNAVAILABLE`), and the UI shows explicit fallback states.
- Structured output: responses are validated with `jsonschema` 0.60 against the skill's `schemas/output.schema.json`; invalid output is rejected (A11), never auto-repaired into authoritative data.
- Context assembly: minimal, provenance-tagged, privacy-filtered; raw prompts are not persisted; per-invocation record keeps model id, template version, token counts, latency, validation outcome.
- Model registry: `model_registry(model_id, architecture, weights_source, license, quantization, checksum, context_length, tokenizer_template, runtime_version)`; load requires `Capability::ModelLoad` (operator-only) and a checksum match. No model is pre-selected; selection follows `docs/evaluations/model-benchmark-protocol.md`.
- Skill registry: `.agents/skills/<name>/SKILL.md` (frontmatter `name`, `description`, `metadata.g9.*`), `schemas/input.schema.json`, `schemas/output.schema.json`, `evals/{positive,negative,adversarial,acceptance}.json`. `g9_skills::Registry::load(dir)` validates all 37 and computes package hashes; mismatched hashes are refused at runtime (supply-chain rule).
- Orchestrator: intent → candidate skills (positive/negative triggers) → policy preflight (`Capability::SkillInvoke`, mode) → envelope → runtime → schema + evidence checks → `SkillOutputEnvelope`. Recursion depth 0; budgets enforced; cancellation releases resources; a late response for a cancelled invocation is discarded.

## 11. Authorization (`crates/g9-policy`)

- Default decision is `Deny`. Rules are data (`PolicyRule { id, actor_kinds, actions, resource_kinds, modes, domains, effect }`) loaded from a versioned built-in table; operator edits require `Capability::PolicyModify` through an approved MAINTENANCE change set.
- Model and skill actors can never receive `operator_only()` capabilities; `Execute`, `Connect` (network), `FilesystemWrite` and `Load` are operator-only *and* require a fresh per-operation approval even for the operator.
- Approval tokens: `ApprovalRequest { id, operation: OperationDescriptor, requested_by, requested_at, expires_at, expected_revision }` → operator decision → `ApprovalRecord` with `binding_mac = HMAC-SHA256(install_secret, canonical_json(operation) || expected_revision || approval_id)`; verification uses constant-time comparison; single-use; expiry default 15 minutes; replay/mismatch logs a `policy_decisions` row with `DENY` and a reason.
- Every evaluation produces a `PolicyDecisionRecord` (metadata only) persisted by `g9-app`.

## 12. UI (`apps/desktop`)

- React 19 + strict TypeScript + Vite; no FullCalendar dependency (ADR); an accessible in-house calendar grid; typed IPC client with two transports (`tauri` and `mock`), so every screen renders and is testable without Tauri or a model.
- Screens (16): Mission Control, Daily Classroom, Calendar, Curriculum Navigator, Session Tracker, Competency Graph, Assessments, Retention Center, Intervention Center, Learning Journal, Performance Dashboard, Executive Workspace, Mentor Memory Explorer, Research Center, Approvals/Audit, Settings/Recovery.
- Markdown rendering is sanitized (allow-list) and the Tauri CSP forbids inline scripts and remote origins. AI output is always visually labeled "recommendation" vs "verified".
- WCAG 2.2 AA target; keyboard-first; model-down and offline states are explicit UI states.

## 13. Security model summary

Threat model and controls: `docs/threat-model/`. Non-negotiables: deny-by-default policy independent of SKILL.md; no shell, network or arbitrary filesystem capability in the mentor runtime; untrusted content is data; approvals bind to exact operations; finalized grades are immutable; backups are restore-tested; secrets never enter logs or IPC errors; the app runs as an unprivileged user; lab orchestration (if ever added) is a separately approved adapter targeting isolated VMs.

## 14. Testing and gates

| Gate | Where it is tested |
|---|---|
| A01 curriculum invariants / provenance | `g9-curriculum` tests + `tests/integration/a01_curriculum_gate.rs` |
| A02 recurrence/DST/conflicts | `g9-calendar` fixtures |
| A03 timer discontinuity | `g9-calendar::timer` tests + recovery test |
| A04 finalized grade immutability | `g9-assessment` + `g9-app` + storage trigger tests |
| A05 competency replay/correction | `g9-competency` property tests |
| A06 policy denies unapproved actions | `g9-policy` matrix tests |
| A07 malicious note/skill cannot grant privilege | `tests/adversarial/` fixtures via `g9-evaluation` |
| A08 backup/restore/migration | `g9-storage` + `tests/recovery/` |
| A09 model-down operation | `NullRuntime` tests + app tests |
| A10 export/deletion propagation | storage + app tests |
| A11 output validation | `g9-ai-adapter` + `g9-skills` schema tests |
| A12 SBOM/licenses/pins/accessibility | CI (`cargo deny`, `cargo audit`, `npm audit`, axe in e2e) |

Unexecuted tests are reported NOT RUN, never PASS (`docs/release/stage-report.md`).

## 15. Repository layout

See `README.md` → "Repository map". Executable scripts under `scripts/` are proposals and are not run by any build step except where CI explicitly lists them.
