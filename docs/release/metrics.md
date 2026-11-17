# Repository metrics

Generated from the files of the tree by `scripts/gen_metrics.py --write`. `python3 -I scripts/gen_metrics.py` fails when this file differs from what the tree gives. No date and no commit id: the same tree gives the same file. Counting rules: the header of `scripts/gen_metrics.py`. Label: TESTED by the generator for the tree that holds this file; a number quoted in another document is not checked against it.

## Summary

| Metric | Value | Counted from |
|---|---|---|
| Workspace members (crates) | 15 (13 under `crates/`; 14 default members) | `Cargo.toml` |
| Rust lines | 129,323 (src 94,088, tests 34,511, other 724) | the members' `.rs` files and `[[test]]` paths |
| `#[test]` attributes | 1,161 | the same files |
| Routed IPC operations | 53 | `OPERATIONS` in `crates/g9-app/src/ops.rs` |
| PROPOSED IPC operations | 15 (analytics 3, executive 4, research 4, teaching 4) | `PROPOSED_OPERATIONS` of the modules `crates/g9-app/src/dispatch.rs` names |
| Migrations | 15 (0001 to 0015) | `crates/g9-storage/src/migrations/` |
| `CREATE TABLE` statements in the migrations | 81 | the same files |
| Tables after the last migration | 81 (0 `DROP TABLE`, 0 renames) | the same files |
| ADRs | 22 | `docs/adr/` |
| JSON schemas | 115 (109 in `schemas/ipc/`, 6 elsewhere) | `schemas/` |
| Skill packages | 37 | `.agents/skills/` |
| Competency packs | 8 | `data/competency-packs/` |
| Frontend TS/TSX | 125 files, 25,143 lines; 41 test files | `apps/desktop/` |
| CSS | 16 files, 4,224 lines | `apps/desktop/` |
| Scripts | 21 in `scripts/`, 3 in `scripts/e2e/`, 11 test modules in `scripts/tests/` | `scripts/` |

## Rust per workspace member

| Crate | Path | Default member | Files | src lines | tests lines | other lines | `#[test]` |
|---|---|---|---|---|---|---|---|
| g9-domain | `crates/g9-domain` | yes | 19 | 2,782 | 145 | 0 | 37 |
| g9-policy | `crates/g9-policy` | yes | 11 | 4,892 | 3,211 | 0 | 87 |
| g9-storage | `crates/g9-storage` | yes | 33 | 28,470 | 3,278 | 0 | 207 |
| g9-curriculum | `crates/g9-curriculum` | yes | 10 | 2,546 | 1,266 | 0 | 32 |
| g9-calendar | `crates/g9-calendar` | yes | 10 | 3,951 | 916 | 0 | 56 |
| g9-evidence | `crates/g9-evidence` | yes | 9 | 3,756 | 369 | 0 | 40 |
| g9-competency | `crates/g9-competency` | yes | 9 | 4,006 | 0 | 0 | 38 |
| g9-assessment | `crates/g9-assessment` | yes | 14 | 3,655 | 2,051 | 0 | 54 |
| g9-retention | `crates/g9-retention` | yes | 7 | 952 | 0 | 0 | 11 |
| g9-ai-adapter | `crates/g9-ai-adapter` | yes | 11 | 2,613 | 614 | 0 | 40 |
| g9-skills | `crates/g9-skills` | yes | 15 | 4,484 | 1,423 | 0 | 63 |
| g9-app | `crates/g9-app` | yes | 80 | 14,494 | 15,803 | 46 | 299 |
| g9-evaluation | `crates/g9-evaluation` | yes | 18 | 6,714 | 1,873 | 0 | 60 |
| g9-cli | `apps/cli` | yes | 38 | 9,181 | 3,562 | 660 | 114 |
| g9-mentor-desktop | `apps/desktop/src-tauri` | no | 4 | 1,592 | 0 | 18 | 23 |
| Total | | | 288 | 94,088 | 34,511 | 724 | 1,161 |

Rust files that no member builds: none.

## IPC operations

| Area | Routed | PROPOSED |
|---|---|---|
| `analytics` | 0 | 3 |
| `approvals` | 2 | 0 |
| `assessment` | 7 | 0 |
| `audit` | 1 | 0 |
| `backup` | 4 | 0 |
| `calendar` | 6 | 0 |
| `competency` | 3 | 0 |
| `curriculum` | 6 | 0 |
| `deletion` | 1 | 0 |
| `evidence` | 3 | 0 |
| `executive` | 0 | 4 |
| `export` | 1 | 0 |
| `interventions` | 1 | 0 |
| `notes` | 4 | 0 |
| `research` | 0 | 4 |
| `retention` | 3 | 0 |
| `session` | 9 | 0 |
| `skills` | 1 | 0 |
| `system` | 1 | 0 |
| `teaching` | 0 | 4 |
| Total | 53 | 15 |

Routed operations with a request and a response schema in `schemas/ipc/`: 53 of 53. Schemas in `schemas/ipc/`: 53 request, 53 response, 3 shared.

Cross-checks passed: operations.parse, operations.unique, operations.disjoint, operations.cli-areas, operations.test-assertion.

## Storage

| Metric | Value |
|---|---|
| Migration files | 15 |
| Migrations embedded by `crates/g9-storage/src/migrations.rs` | 15 |
| `CREATE TABLE` statements | 81 |
| `DROP TABLE` statements | 0 |
| `ALTER TABLE ... RENAME TO` statements | 0 |
| Tables after the last migration | 81 |

## Frontend

| Group | Files | Lines |
|---|---|---|
| Application source (`apps/desktop/src/`, .ts/.tsx) | 79 | 17,891 |
| Test files (`*.test.ts`, `*.test.tsx`) | 41 | 7,091 |
| Test support (`apps/desktop/src/test/`) | 4 | 135 |
| Build configuration (.ts outside `apps/desktop/src/`) | 1 | 26 |
| CSS | 16 | 4,224 |

## Content and scripts

| Metric | Value |
|---|---|
| ADRs (`docs/adr/`) | 22 |
| JSON schemas in `schemas/ipc/` | 109 |
| JSON schemas elsewhere in `schemas/` | 6 |
| Skill packages (`.agents/skills/`) | 37 |
| Competency packs (`data/competency-packs/`) | 8 |
| Files in `scripts/` | 21 (.json 1, .py 17, .sh 3) |
| Files in `scripts/e2e/` | 3 |
| Test modules `scripts/tests/test_*.py` | 11 |
