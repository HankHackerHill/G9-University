# Improvement backlog (Stage 00)

Improvements beyond the source documents, ranked. Maintainer items (IMP-M*) come from the architecture review; auditor items carry their register ids. The dedicated improvement auditor did not complete (usage limit), so this list is not exhaustive.

## Maintainer items

| ID | Priority | Improvement | Target stage | Lands in |
|---|---|---|---|---|
| IMP-M01 | HIGH | Generated IPC types: a `g9-ipc` crate deriving `schemars` JSON Schema and TypeScript (`ts-rs`) so `schemas/ipc` and `apps/desktop/src/ipc/types.ts` are build artifacts checked for drift in CI | Stage 03 | `crates/g9-ipc, schemas/ipc, apps/desktop/src/ipc` |
| IMP-M02 | HIGH | Native Rust-side confirmation dialog for protected operations (ADR-0014) using tauri-plugin-dialog with `dialog:allow-confirm` only | Stage 03 | `apps/desktop/src-tauri, crates/g9-app::pipeline` |
| IMP-M03 | HIGH | Reliability class on grade decisions and per-criterion minimum in the pass rule (ADR-0012) | Wave 2 / Stage 04 | `crates/g9-assessment, crates/g9-competency::rules` |
| IMP-M04 | HIGH | Exposure ledger + exam item variants + blind-eligibility rule (LRN-13) | Stage 04 | `crates/g9-assessment, g9-storage migrations 00xx_exam_items.sql` |
| IMP-M05 | MEDIUM | Workspace layering test over `cargo metadata` (ADR-0010) | Wave 2 | `crates/g9-evaluation/tests/layering.rs` |
| IMP-M06 | MEDIUM | SQLCipher option behind `storage-encrypted` feature with OS keyring key (docs/privacy) | Stage 03 | `crates/g9-storage` |
| IMP-M07 | MEDIUM | Grammar-constrained structured decoding (GBNF/JSON-schema) in the llama.cpp adapter to raise schema validity before validation | Stage 06 | `crates/g9-ai-adapter` |
| IMP-M08 | MEDIUM | Encrypted backup sets (age/XChaCha20-Poly1305) with key id in the manifest | Stage 03 | `crates/g9-storage::backup` |
| IMP-M09 | LOW | CycloneDX SBOM export (`cargo-cyclonedx`) attached to releases | Stage 14 | `.github/workflows/ci.yml` |
| IMP-M10 | LOW | Skill eval harness execution against a model with stratified precision/recall reporting | Stage 07 | `crates/g9-skills::evals, docs/evaluations/skill-evals` |

## Auditor-flagged improvements (11)

| ID | Severity | Improvement (correction text) | Affects |
|---|---|---|---|
| LRN-31 | MEDIUM | Before each blind practical, M13 stores a system forecast per sampled item: P(pass) derived from the current competency state by a versioned deterministic mapping (e.g., VALIDATED 0.85, INDEPENDENT 0.70, GUIDED 0.45, INTRODUCED 0.25, UNASSESSED base rate), in calibration_observations with actor=SYSTEM and forecast_type=STATE_PREDICTION. After grading, compu… | M13, M06, L02, L06, crates/g9-evaluation, crates/g9-competency, docs/evaluations |
| TEC-13 | MEDIUM | Replace 'with correct driver support' with: 'rusqlite = "=0.41.1", features = ["bundled", "backup"] (add "hooks" only if the preupdate/commit hooks are used for audit); bundled SQLite 3.54.1 provides FTS5, JSON1, VACUUM INTO (>= 3.27) and the Backup API; never link the system libsqlite3 for the desktop build. Enable and assert PRAGMA foreign_keys=ON on ever… | M17, crates/g9-storage, E07, E14, docs/release |
| TEC-36 | MEDIUM | Add to Stage 01 exit criteria: 'On the target host (KDE Plasma/Wayland, NVIDIA driver version recorded) the scaffold window renders a test pattern, resizes and screenshots correctly without environment overrides; if an override such as WEBKIT_DISABLE_DMABUF_RENDERER=1 is required, record it as a known limitation with the driver/WebKitGTK versions and add it… | M01, apps/desktop/src-tauri, E14, Stage 01 |
| CON-38 | LOW | Define in Section 6/Appendix: PolicyDecision = Allow{decision_id} \| Deny{reason_code} \| ApprovalRequired{approval_request_id, expires_at_ms}; every decision is logged to policy_decisions; UI maps ApprovalRequired to error category APPROVAL_REQUIRED. | M16 Authorization, crates/g9-policy, schemas/ipc |
| CON-41 | LOW | Make Appendix B the single canonical field list (union of the three, each marked required/optional) and have Sections 11 and 23 reference 'the Appendix B field list'; encode it in schemas/skill-manifest.schema.json used by E02 and the loader. | E02 g9-skill-engineer, .agents/skills, crates/g9-skills, schemas |
| CON-43 | LOW | Week 7 title 'SOC, detection engineering and investigation'; Week 8 title 'DFIR, malware triage and threat hunting'; update seed JSON week titles in the same version bump as CON-12. | data/curriculum, KP-05, KP-06 |
| SEC-19 | LOW | Add an 'OWASP ASI' column to the §12 matrix: ASI01 Agent Goal Hijack ↔ document prompt injection/skill descriptions; ASI02 Tool Misuse ↔ data exfiltration, lab escape, research fetch; ASI03 Identity & Privilege Abuse ↔ policy bypass/TOCTOU, actor binding; ASI04 Agentic Supply Chain ↔ skill/model/runtime/CI tampering; ASI05 Unexpected Code Execution ↔ packag… | docs/threat-model, Appendix C, E06, E13 |
| SEC-30 | LOW | Manifest schema v1.1: add `content_sha256` (over JCS-canonical JSON excluding the field), `source_document{title, sha256}`, `approval_id` (null while provenance=PROPOSED_SEED); importer recomputes and stores the hash in `curriculum_versions.content_hash`, rejects manifests >5 MB or non-UTF-8, and marks any URL as `untrusted_url` (display only; opening goes … | M02, g9-curriculum, schemas, data/curriculum, E03 |
| TEC-37 | LOW | Rename to `competency_versions` and `milestone_dependencies`; adopt `STRICT` on every table and `WITHOUT ROWID` for pure join tables with composite text keys; add a schema test that fails if any table lacks STRICT or any `_ms` column is not INTEGER. | crates/g9-storage, M06, M14, E07 |
| TEC-38 | LOW | Rust types in g9-ipc and g9-skills are the source of truth; derive `schemars::JsonSchema` and `ts_rs::TS`, generate schemas/** and apps/desktop/src/ipc/generated/** by `cargo xtask gen-schemas`, commit the outputs, and fail CI on diff. Semantic validation is a `TryFrom<Dto> for DomainCommand` layer in g9-app (ranges, state-dependent enums, cross-field rules… | crates/g9-ipc, crates/g9-skills, schemas, apps/desktop/src, E04, E02 |
| TEC-39 | LOW | Move host assumptions into one 'Target host (ASSUMED until inspected)' table in Section 13: CPU (Ryzen 9 9800X3D, assumed), RAM 64 GB (assumed), GPU RTX 5080 16 GB (verified spec, host unverified), storage type/free space (unknown), driver/CUDA version (unknown); Stage 01 read-only inspection fills the 'observed' column before any runtime is selected. | M10, E10, docs/evaluations, Section 23 prompt |
