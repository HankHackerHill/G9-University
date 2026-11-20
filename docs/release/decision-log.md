# Stage 00 decision log (one page)

| # | Decision | Rationale / evidence | Reference |
|---|---|---|---|
| 1 | Build the deterministic core first; the model is optional and proposal-only | R-002/A09; corrections C09, C11, C21 | ARCHITECTURE.md §1, §5 |
| 2 | v6.1 (this repository) supersedes v5.1 and v6.0 where they conflict; every override is a register row and an ADR | 22 v6.0 corrections + 152 auditor findings | docs/audit/defect-register.md, docs/adr/ |
| 3 | 13 crates mapped to 17 modules with acyclic layering | TEC-01/TEC-02 | ADR-0010 |
| 4 | SQLite with Online Backup API / VACUUM INTO, verified on a copy; WAL independent; no `.dump` | C04/C05, TEC-09/10 | ADR-0001 |
| 5 | Explicit per-FK deletion semantics; no blanket cascade | C13 | ADR-0002 |
| 6 | HMAC-bound, single-use, expiring approvals; operator-only capabilities never granted to skills/model | TEC-08, SEC-02, SEC-23 | ADR-0003 |
| 7 | Metadata-only hash-chained audit; raw prompts never persisted | C19, SEC-08 | ADR-0004 |
| 8 | Dimension-specific, rule-derived competency state; model observations rejected; reliability class on grades | C10, LRN-06/09/11 | ADR-0005, ADR-0012 |
| 9 | Fixed baseline review schedule; FSRS optional and declarative-only | C14, LRN-04/18 | ADR-0006 |
| 10 | Pre-registered N-of-1 designs; INCONCLUSIVE allowed; no fixed n/p thresholds | C08, LRN-01/02/03/21 | ADR-0007 |
| 11 | No FullCalendar; in-house accessible grid | v7 Premium AGPLv3 (VERIFIED) | ADR-0008 |
| 12 | No model pre-selected; benchmark protocol on the actual host | C06/C07 | ADR-0009 |
| 13 | UTC ms instants, IANA zone + civil time for recurrences, wall+monotonic timer ledger | TEC-07/20 | ADR-0011 |
| 14 | Closed 13-category error set; backend-assigned actor; idempotent replay | TEC-03/04, SEC-01 | ADR-0013 |
| 15 | SKILL.md is not a security boundary; native Rust-side confirmation for protected operations (PROPOSED, Stage 03) | C11, SEC-03 | ADR-0014 |
| 16 | Target host Ubuntu 26.04.1 LTS (Wayland); CI runner ubuntu-26.04 | operator instruction; VERIFIED release dates | README.md, .github/workflows/ci.yml |
| 17 | Appendix A seed stays PROPOSED_SEED until reconciled with the original plan and approved | C01 | docs/curriculum/provenance-and-reconciliation.md |
| 19 | The container engine (Docker) is not used in this repository: not in code, scripts, workflows or proposed commands, and not run by anyone working on it. The rule is lifted only by the owner's explicit instruction. Courseware commands that called it were removed (their ids stay); how intentionally vulnerable applications are hosted in the target guest instead is an OPEN decision | owner instruction 2026-11-20 | `scripts/check_no_docker.py`, `.github/workflows/ci.yml`, `docs/curriculum/v2/LAB_ARCHITECTURE.md` section 9 |
| 18 | Skill packages and knowledge packs are generated from checked-in tables and validated in CI; all PROPOSED | budget and consistency | scripts/gen_skill_packages.py, scripts/gen_competency_packs.py |

## First exact proposed Stage 03 operation (not performed)

**Operation:** on the G9 host, build the Tauri shell and run the desktop UI against the mock transport.

```
sudo apt-get install -y libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
cd ~/G9-University && cargo check -p g9-mentor-desktop && cd apps/desktop && npm ci && npm run build && npm run dev
```

**Purpose:** confirm the shell compiles on Ubuntu 26.04.1 (Wayland/KDE), the capabilities file loads, and the UI renders with model unavailable. **Target:** operator workstation, repository checkout only. **Expected effects:** system packages installed (apt), build artifacts under `target/` and `apps/desktop/dist/`; no database created until a command is issued. **Risks:** apt changes host packages; mitigated by reviewing the package list first. **Verification:** `cargo check` exit 0; UI loads at the Vite dev URL; `system.status` shows `model.available = false`. **Rollback:** `apt-get remove` of the listed packages; delete `target/` and `dist/`.
