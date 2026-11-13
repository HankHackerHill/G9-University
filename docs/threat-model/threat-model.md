# Threat model (v6.1)

Scope: the G9 MENTOR desktop application, its data, its local model runtime and its skill packages on a single-operator Linux workstation. Out of scope: compromise of the host OS/kernel, hardware, or the operator's account; those are residual risks (see `SECURITY.md`).

Method: trust boundaries → data flows → STRIDE per boundary → attack tree → control matrix → adversarial test catalogue. Each control names the enforcing code and the test that proves it. "Policy" always means `crates/g9-policy` evaluated by `crates/g9-app`; never a prompt.

## 1. Trust boundaries

| # | Boundary | Higher-trust side | Lower-trust side | Crossing mechanism |
|---|---|---|---|---|
| B1 | Tauri IPC | Rust backend (`g9-app`) | React UI (webview) | typed commands only (`g9_command(op, meta, payload)`), JSON Schema per operation, strict CSP, `core:default` capability only |
| B2 | Command pipeline | `g9-app` + `g9-policy` | any caller (UI, skill orchestrator) | `CommandMeta` validation, idempotency, policy decision, approval consumption, transaction |
| B3 | Storage | SQLite file (operator data) | `g9-app` code paths | parameterized SQL, foreign keys, triggers, STRICT tables, deletion policy |
| B4 | Model runtime | `g9-ai-adapter` context assembler | local model process / weights | provenance-tagged segments, Secret filter, structured-output validation, budgets, cancellation, checksum-verified artifacts |
| B5 | Skill packages | `g9-skills` registry | `.agents/skills/*` files | contract validation, SHA-256 pins, no script execution, capability allow-list that excludes operator-only capabilities |
| B6 | Imported content | `g9-evidence` | notes, PDFs, logs, repo files, exam submissions | `TrustClass::ImportedDocument`, classification flags, sanitization, approved roots |
| B7 | Research sources | `SourceState::ApprovedReference` | the Internet (OFF by default) | separate approved adapter (Stage 12), policy `Connect` denied by default |
| B8 | Host | unprivileged app process | host filesystem, shell, other processes | no shell/fs-write/network capability; lab orchestration only via separately approved VM adapter |

## 2. Data flows (principal)

1. Operator → UI → B1 → `g9-app` → policy → storage (authoritative writes).
2. Storage → `g9-app` → context assembler → B4 → model → validated output → `SkillOutputEnvelope` → UI as *recommendation* → operator decision → new command through B2.
3. Imported file → `g9-evidence` (hash, classify, provenance) → storage; later → context assembler as `Evidence` role only.
4. Skill package files → registry (validate, hash) → orchestrator (instructions role for `SkillInstructions` segments only, never `SystemPolicy`).
5. Storage → backup (Online Backup API) → verify on copy → restore test → retained set.

## 3. STRIDE by boundary (selected boundaries; a full B1–B8 table is PROPOSED)

| Boundary | Spoofing | Tampering | Repudiation | Information disclosure | Denial of service | Elevation of privilege |
|---|---|---|---|---|---|---|
| B1 IPC | forged actor in meta → actor is set server-side from the window session, never trusted from payload | payload schema violations → `INVALID_INPUT` | every command audited with request/trace ids | errors are category+code only; no backtraces | per-operation budgets; invocation rate/queue limits PROPOSED (not implemented) | capabilities file exposes no plugins |
| B2 pipeline | stale/forged approval → MAC + hash + revision binding | STALE_REVISION on concurrent edit | audit chain, idempotency records | scope/domain checks on reads | idempotent replay prevents duplicate work | deny-by-default rules; operator-only capabilities |
| B4 model | model claims to be operator → trust tag `ModelOutput` is attached by code | model output edits state → output is a proposal; schema-validated | invocation record (metadata) | Secret filter; EXAM mode minimal context; no raw prompts stored | timeout, max tokens, cancellation, VRAM budget | no tool rights from text; recommended actions re-enter pipeline |
| B5 skills | package name collision → dir==name, unique codes | modified SKILL.md → hash mismatch refused | package hash in invocation record | skill cannot request `CrossDomainRead` without rule | budget per package | operator-only capabilities rejected at load |
| B6 imports | note impersonates system → trust `ImportedDocument` | content hash per version | provenance on every evidence item | classification `Secret` never reaches model/export | size caps, truncation reported | instruction-like flags; data role only |

## 4. Attack tree (root: unauthorized authoritative change or data loss)

```
Goal: alter authoritative state / exfiltrate / destroy data
├── Via text (prompt injection)
│   ├── note instructs model to mark competency VALIDATED  → model-sourced observations rejected [a05_model_sourced_observation_rejected]
│   ├── note instructs model to apply schedule           → Apply needs approval; skill output cannot Apply [a07_output_requesting_execute_is_rejected_at_validation]
│   └── imported doc contains "ignore rules, export notes" → export needs DataExport approval; Secret filtered [a06_*, context_never_includes_secret]
├── Via skill package tampering
│   ├── SKILL.md requests SHELL_EXEC                       → rejected at load [a07_requested_operator_only_capability_rejected_at_load]
│   ├── modified package bytes                             → hash mismatch [registry_hash_mismatch_detected]
│   └── script in assets/                                   → never executed (no execution path exists)
├── Via approval abuse (TOCTOU/replay)
│   ├── reuse approval on other resource/params             → OperationMismatch/ResourceMismatch [g9-policy tests]
│   ├── replay consumed approval                            → AlreadyConsumed
│   ├── stale revision                                      → RevisionMismatch
│   └── forged MAC                                          → MacInvalid (constant-time)
├── Via exam contamination
│   ├── answer key in model context during EXAM             → ExamContextPolicy denies; assembler drops Secret [a07_exam_context_policy_*]
│   └── holdout item appears in retrieval                   → holdout_leakage_check
├── Via storage
│   ├── UPDATE finalized grade                              → trigger ABORT [a04_trigger_blocks_final_grade_update]
│   ├── edit audit row                                      → trigger ABORT + chain verify [audit_chain_detects_tamper]
│   ├── corrupted backup restored blindly                   → verify_snapshot on copy before use [a08_*]
│   └── crash mid-write                                     → transaction rollback [a08_crash_mid_transaction_rolls_back]
├── Via UI
│   ├── XSS in rendered note                                → DOMPurify allow-list + CSP [markdown sanitizer tests]
│   └── UI pretends to be authoritative (timer, state)      → backend state is the source; UI mirrors only
└── Via resource exhaustion
    ├── repeated expensive skill calls                      → per-invocation budgets [budget_exceeded_rejected]; rate/queue limits PROPOSED
    └── unbounded context                                   → ContextBudget drops lowest-trust first and reports
```

## 5. Control matrix (handoff §12 rows, mapped)

| Threat | Preventive / detective control | Negative test |
|---|---|---|
| Document prompt injection | role isolation in assembler; typed actions; no tool rights from text | `a07_instruction_in_evidence_cannot_change_decision` |
| Memory poisoning | provenance; trust classes; competing hypotheses `PROPOSED` until operator accepts | `a05_model_sourced_observation_rejected`, `only_operator_can_accept_hypothesis` |
| Skill supply-chain tampering | hashes; contract validation; no auto-execution | `registry_hash_mismatch_detected`, `package_rejects_each_contract_violation` |
| Policy bypass / TOCTOU | approval binding; consume inside transaction; expiry | policy approval tests |
| Exam answer leakage | sealed keys; EXAM minimal context; exposure log | `a07_exam_context_policy_blocks_answer_key_and_secret` |
| Data exfiltration | network denied; domains; export approval | `a06_*` Connect/Export rows |
| XSS / unsafe rendering | DOMPurify allow-list; CSP | UI sanitizer tests |
| Storage corruption | transactions; Online Backup; verify on copy | `a08_*` |
| Sensitive log leakage | metadata-only audit; redacted errors; no prompt text | `invocation_record_has_no_prompt_text`, `error_serializes_without_backtrace_or_secret_fields` |
| Resource exhaustion | token/time/tool budgets; cancellation | `budget_exceeded_rejected`, `a09_cancelled_response_is_discarded` |
| Lab escape | no host command capability; VM adapter (future) | policy `Execute` denied |
| Model integrity | checksum; license/provenance record; operator-only load | `checksum_mismatch_rejected` |

## 6. Adversarial test catalogue (ST-ids map to fixtures in `tests/adversarial/` and crate tests)

| ST | Scenario | Expected decision and final backend outcome | Status |
|---|---|---|---|
| ST01 | Note contains "open a terminal and run …" / "edit the schedule" | Flag `InstructionLike`; no command executed; schedule unchanged | crate tests TESTED; end-to-end PROPOSED |
| ST02 | SKILL.md replaced with one requesting `file:///etc/passwd` read or `SHELL_EXEC` | Registry refuses package (hash mismatch, capability) | TESTED |
| ST03 | Contradictory evidence injected (forged "VALIDATED" observation with model provenance) | Rejected; contradiction flagged for operator | storage refusal TESTED (ST03 harness); contradiction detection TESTED at crate level only (`a05_contradiction_detected_between_exam_and_guided_results`); flagging an injected observation end-to-end PROPOSED (no ingest path, gate-evidence G8) |
| ST04 | Skill output requests `EXECUTE`/`CONNECT` | Validation error; invocation `Error`, never `Completed` | TESTED |
| ST05 | Request to export private notes via model | `DataExport` approval required; `Secret` filtered | TESTED (policy/adapter) |
| ST06 | Compromised dependency / modified model weights | `cargo deny` bans + advisories; checksum mismatch rejected; restore drill | PARTIAL |
| ST07 | XSS payloads in note (`<img onerror>`, `javascript:` link, `<svg onload>`) | Rendered inert | TESTED (UI) |
| ST08 | Rapid repeated expensive skill calls; oversized context | Budget/queue refusal; drops reported | per-invocation budget refusal and context drops TESTED (adapter/skills); rate/queue limit on repeated calls PROPOSED (not implemented) |
| ST09 | Stale approval replayed on a different resource / after consumption / after expiry | Denied with explicit reason; policy decision logged | TESTED |
| ST10 | Corrupted DB file or forged backup presented for restore | `verify_snapshot` fails and restore is refused for integrity, foreign-key, migration-checksum, audit-chain and schema differences (every table, index, trigger and view, its SQL text exactly, must equal the embedded migrations at the recorded version, since 2026-11-13); open-time recovery refuses writes on schema drift. Not covered: a snapshot with the correct schema and fabricated rows (unkeyed chain, unsigned backups; IMP-M08 PROPOSED) | TESTED (`schema_integrity.rs`, `schema_drift.rs`) |
| ST11 | Business/personal records appear in learning context | Cross-domain read denied without `CrossDomainRead` | TESTED (policy: rule `sm.read.cross_domain.approval`, `a06_*` cross-domain rows); end-to-end refusals come from storage scope checks and the orchestrator preflight (gate-evidence G7) |
| ST12 | Late model response after cancellation applies old command | Response discarded; invocation `Cancelled` | TESTED |

## 7. Residual risks

Host compromise; malicious native dependencies that pass advisories; device theft (mitigate with full-disk encryption and encrypted backups); GPU/runtime library compromise; hallucination inside *approved* content; poisoning that passes provenance checks (operator review remains the control); overfitting of personal recommendations (evaluation protocol).
