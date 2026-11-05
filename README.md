# G9 MENTOR

**Offline-first, local-AI learning and performance operating system for G9 / Cyber Team 9.**
Tauri 2 · Rust modular monolith · React + strict TypeScript · SQLite · replaceable local inference.

| | |
|---|---|
| Controlling architecture | [`ARCHITECTURE.md`](ARCHITECTURE.md) (v6.1, supersedes the v5.1 plan and the v6.0 handoff where they conflict) |
| Corrections register | [`docs/audit/defect-register.md`](docs/audit/defect-register.md) · [`docs/audit/stage00-triple-audit.md`](docs/audit/stage00-triple-audit.md) |
| Decisions | [`docs/adr/`](docs/adr/) |
| Governance and stage gates | [`GOVERNANCE.md`](GOVERNANCE.md) · [`docs/release/stage-report.md`](docs/release/stage-report.md) |
| Security | [`SECURITY.md`](SECURITY.md) · [`docs/threat-model/`](docs/threat-model/) |
| Agent / engineer rules | [`CLAUDE.md`](CLAUDE.md) · [`AGENTS.md`](AGENTS.md) |

## What it is

A single-operator desktop system that answers: *what should I work on next, why, what exactly should I do, how do we prove I improved, what did not work, how does this affect my priorities, and can I independently review or correct the evidence?* It joins a deterministic learning command center (curriculum, calendar, timer, notes, evidence), an independent examiner, an evidence-centered competency model, retention scheduling, a proposal-only adaptive AI teacher, an execution/decision workspace and an approved-source research system. Everything essential works with the model disabled.

## Non-negotiables (enforced in code, tested in CI)

- Deny-by-default authorization in Rust (`crates/g9-policy`); SKILL.md metadata never grants permissions.
- Model/skill/imported content is data, never instructions and never authoritative state.
- Finalized grades, consumed approvals and audit metadata are immutable; corrections supersede.
- Curriculum invariants 12 weeks × 6 days × 3 objectives = 216, 28,800 planned minutes; the Appendix A seed is `PROPOSED_SEED`, never canonical without reconciliation and operator approval.
- Backups use the SQLite Online Backup API or `VACUUM INTO`, are integrity-checked and restore-tested; no `.dump` or live-file copies.
- No shell, network or arbitrary filesystem capability in the mentor runtime.

## Repository map

```
ARCHITECTURE.md  SECURITY.md  GOVERNANCE.md  AGENTS.md  CLAUDE.md  LICENSES.md  CHANGELOG.md  deny.toml
crates/
  g9-domain        ids, time, trust, privacy, envelope, policy types, state machines, invariants (no I/O)
  g9-policy        M16 deny-by-default engine, approval binding (HMAC), decision records
  g9-storage       M17 SQLite: migrations (checksummed), repositories, backup/restore, audit hash chain, idempotency
  g9-curriculum    M02 manifest schema, invariant validator, reconciliation diff, provenance gate
  g9-calendar      M03/M04 civil time + DST, recurrence, conflicts, deterministic proposals, timer ledger
  g9-evidence      M05 notes/revisions, content-addressed attachments, classification, source records
  g9-competency    M06/M09 graph, observations, replayable derivation, learner hypotheses, diagnosis
  g9-assessment    M07 rubrics, attempt lifecycle, reviewer-finalized grades, exam-context policy
  g9-retention     M08 baseline spaced review, optional FSRS (declarative items only), transfer checks, calibration
  g9-ai-adapter    M10 inference trait, context assembler, structured-output validation, model registry, NullRuntime
  g9-skills        M11 skill package loader/validator, invocation state machine, orchestrator
  g9-app           command pipeline (idempotency → policy → approval → transaction → audit), IPC handlers
  g9-evaluation    golden / adversarial / recovery fixtures and harness
apps/desktop       React + strict TS UI (16 screens, mock + tauri transports); src-tauri minimal bridge
.agents/skills     37 skill packages (SKILL.md, references, schemas, evals)
data/curriculum    versioned manifests (seed v0 = Appendix A, PROPOSED_SEED)
data/competency-packs  8 domain knowledge packs
data/fixtures      synthetic data, no secrets
schemas/           ipc, skill-inputs, skill-outputs, event-schemas, curriculum, competency-pack
docs/              requirements, adr, audit, threat-model, privacy, evaluations, release, ui, skills, curriculum, runbooks
tests/             integration, e2e, recovery, adversarial
scripts/           validators run by CI (python3 -I); nothing runs by default
```

## Build and verify

```bash
# Rust (all default workspace members; the Tauri shell is excluded and needs webkit2gtk-4.1 + GTK3)
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets

# Desktop UI
cd apps/desktop && npm ci && npm run typecheck && npm run lint && npm test -- --run && npm run build

# Content gates
python3 -I scripts/check_curriculum_seed.py data/curriculum/g9-ct9-480.seed.v0.json
python3 -I scripts/check_skill_packages.py .agents/skills --expect-count 37
python3 -I scripts/check_competency_packs.py data/competency-packs --expect-count 8
python3 -I scripts/check_schemas.py schemas .agents/skills
```

CI (`.github/workflows/ci.yml`) runs the same steps plus `cargo-deny` and builds the Tauri shell on a host with the system libraries.

## Stage status

See `docs/release/stage-report.md` for the executed-test evidence. Summary labels per `GOVERNANCE.md`: Stage 00 DESIGN (this repository's documents) · Stage 01 SCAFFOLD (workspace, CI, pins) · Stage 02 DATA & IMPORTER (schema, migrations, importer, backup primitives) · Stages 03–15 have contracts, schemas, tests and skill packages but are **PROPOSED** unless the stage report says otherwise.

## Target environment (ASSUMED until inspected)

Ubuntu 26.04 LTS based G9 OS (VERIFIED released 23 Apr 2026; ships Wayland-only GNOME, KDE/Wayland on the host still to verify), RTX 5080 16 GB GDDR7 (VERIFIED hardware spec; host presence/driver unverified), 64 GB RAM (ASSUMED). No model is pre-selected; see `docs/evaluations/model-benchmark-protocol.md`.
