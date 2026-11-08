# Wave 2 integration report

> Update 2026-11-08: findings D2 and D6 below (observations not derived; placeholder difficulty) were resolved by migration 0013 and the `approve_final` derivation (commit `4b404cc`). This report is otherwise a point-in-time record of 2026-11-06.

**Date:** 2026-11-06. **Verifier:** wave-2 integration verifier. **Scope:** whole workspace, with a fix mandate for `crates/g9-storage/src/repos`, `crates/g9-app`, `crates/g9-evaluation`, `tests/` and `apps/desktop/src-tauri`.
**Labels:** TESTED (executed here, result observed), INFERRED, PROPOSED (not implemented), NOT RUN.

## 1. Result

| Gate command (`CARGO_TARGET_DIR=target/integration`) | Result | Label |
|---|---|---|
| `cargo fmt --all --check` | exit 0, no diff (includes the Tauri crate and the `tests/` targets) | TESTED |
| `cargo clippy --all-targets -- -D warnings` (default members) | exit 0, 0 warnings, 0 errors | TESTED |
| `cargo test --all-targets` (default members) | exit 0: **35 test binaries, 525 passed, 0 failed, 0 ignored** | TESTED |
| `cargo test --doc` (default members) | exit 0; 0 doctests in all 13 crates | TESTED |
| `cargo clippy -p g9-retention --all-targets --features fsrs -- -D warnings` / `cargo test -p g9-retention --all-targets --features fsrs` | exit 0 / 9 passed (8 default + 1 `fsrs`) | TESTED |
| `cargo check -p g9-mentor-desktop` | exit 101: `gdk-sys` build script cannot find `gdk-3.0` (§4) | TESTED (expected failure) |

The full test suite was run three times (two before and one after the change in §3). Every run passed; the first two gave 518/518 and the third 525/525. The difference is the 7 new layering tests.

No compile error, clippy warning, fmt diff or failing test was found in the wave-2 crates. No wave-1 crate was modified.

**Build-environment note.** The first `cargo test` attempt failed with `No space left on device (os error 28)`: the host disk had 75 MB free, and the wave-1 and wave-2 per-crate target directories held about 27 GB. I deleted only my own `target/integration/debug/{incremental,deps,.fingerprint}` and reran every command with `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`. This drops debug info only; `debug-assertions` and `overflow-checks` stay as they were. While the work was in progress, something other than this verifier freed space on the host (free space rose to 27 GB).

## 2. Per-crate test counts (`cargo test --all-targets`, final run)

| Crate | Unit (`src`) | Integration binaries (passed) | Total |
|---|---|---|---|
| g9-domain | 33 | — | 33 |
| g9-policy | 35 | `a06_policy_matrix` 15, `a07_skill_privilege` 10, `toctou_approvals` 13 | 73 |
| g9-storage | 51 | `gates` 35 | 86 |
| g9-curriculum | 14 | `a01_gate` 13 | 27 |
| g9-calendar | 40 | `gates` 16 | 56 |
| g9-evidence | 33 | `gates` 6 | 39 |
| g9-competency | 34 | — | 34 |
| g9-assessment | 3 | — | 3 |
| g9-retention | 8 | — | 8 (9 with `--features fsrs`) |
| g9-ai-adapter | 28 | `gates` 12 | 40 |
| g9-skills | 25 | `live_registry` 1, `orchestrator_tests` 14, `package_tests` 4, `registry_tests` 4 | 48 |
| g9-app | 6 | `gates` 22 | 28 |
| g9-evaluation | 13 | `a01_gate` 1, `a04_a05_gate` 3, `a06_a07_gate` 6, `a08_recovery_drill` 1, `a09_model_down` 2, `a10_export_deletion` 2, `a11_output_validation` 2, `st_adversarial_catalogue` 13, `layering_gate` 7 | 50 |
| **Total (default members)** | | | **525** |
| g9-mentor-desktop (Tauri-free part, scratch harness, §4) | 30 (`keyring` 12, bridge 18) | — | 30, TESTED outside the workspace build |

Other suites run as a cross-check of the IPC contract (not Rust, not part of the fix scope):

| Command (in `apps/desktop`) | Result | Label |
|---|---|---|
| `npx vitest run` | 9 files, 114 passed | TESTED |
| `npx vitest run src/ipc` (schema/operation parity) | 4 files, 43 passed | TESTED |
| `npx tsc --noEmit -p tsconfig.json` | exit 0 | TESTED |
| Rust `g9_app::ops::OPERATIONS` vs `apps/desktop/src/ipc/operations.ts` (script comparison: name, mutating flag, action, resource kind) | 35 = 35 operations, 0 mismatches | TESTED |
| `cargo-deny 0.20.2 --frozen --all-features check` (local advisory-db snapshot, last commit 2026-11-06 16:34 +01:00) | `advisories ok, bans ok, licenses ok, sources ok`; warnings: duplicate crate versions, `license-not-encountered` | TESTED (offline snapshot) |

The g9-app engineer said the UI parity test would fail until `curriculum.promote` was registered. That is resolved: `operations.ts` registers it, and the parity tests pass.

## 3. Changes made by this verifier

| File | Change | Why |
|---|---|---|
| `tests/integration/layering_gate.rs` (new) | 7 tests. The manifests are read with `std::fs` only (no subprocess). The tests check that every member has a declared layer; that internal edges follow ARCHITECTURE.md §2 rules 1–7; that nothing depends on `g9-evaluation` or `g9-mentor-desktop`; that the graph is acyclic (Kahn's algorithm); that `rusqlite` appears only in g9-storage/g9-app, `tauri` only in the shell, and no network client crate anywhere. A negative test proves a forbidden edge and a cycle are detected. | ADR-0010 requires "a workspace test over `cargo metadata` asserts the layering (wave 2)". No such test existed. |
| `crates/g9-evaluation/Cargo.toml` | Registered `[[test]] layering_gate` (path `../../tests/integration/layering_gate.rs`). No new dependency. | Cross-crate tests under `tests/` are compiled as g9-evaluation integration tests (`tests/README.md`). |

## 4. g9-mentor-desktop (Tauri shell)

`cargo check -p g9-mentor-desktop` (exit 101), exact error:

```
error: failed to run custom build command for `gdk-sys v0.18.2`
  pkg-config exited with status code 1
  > PKG_CONFIG_ALLOW_SYSTEM_CFLAGS=1 pkg-config --libs --cflags gdk-3.0 'gdk-3.0 >= 3.22'
    Package gdk-3.0 was not found in the pkg-config search path.
  The system library `gdk-3.0` required by crate `gdk-sys` was not found.
  The file `gdk-3.0.pc` needs to be installed and the PKG_CONFIG_PATH environment variable must contain its parent directory.
  The PKG_CONFIG_PATH environment variable is not set.
```

Host facts: pkg-config 1.8.1 is installed, but `pkg-config --exists webkit2gtk-4.1` and `pkg-config --exists gtk+-3.0` both return 1. No error came from the crate's own code before the build stopped. `cargo test --workspace` stops at the same build script (reported by the g9-app engineer; INFERRED, not re-run).

**Tauri-free part, TESTED through a scratch harness outside the repository.** The harness copied `src/lib.rs` without the `mod shell` block and `pub use shell::run;` (original lines 456–535). It also copied `src/keyring.rs` verbatim and pinned the same `Cargo.lock` versions, the same lints and the same path dependencies. The `.agents/skills` path was preserved through a symlink. Results: `cargo clippy --all-targets -- -D warnings` exit 0; `cargo test --all-targets` 30 passed, 0 failed. The 30 tests are the 12 `keyring::tests` and 18 bridge tests, including `ui_supplied_actor_is_refused`, `approval_granted_before_restart_is_consumed_once_after_restart`, `replaced_binding_key_invalidates_granted_approval`, `idempotent_retry_replays_and_changed_retry_conflicts` and `bundled_skills_load_and_are_reported_unpinned`.

**NOT RUN:** the `shell` module (`g9_command`, `setup`, `run`), `tauri::generate_context!()`, `tauri-build`, the capability file, the CSP and bundling.

**Known blocker on a prepared host (INFERRED):** `tauri.conf.json` `bundle.icon` lists six files under `apps/desktop/src-tauri/icons/`, and that directory does not exist. `generate_context!` and `tauri-build` read the icons, so the build will fail until the set exists. I did not add icons: the binary assets cannot be verified here, and it is the product owner's choice. Remedy: run `npx tauri icon <source.png>` in `apps/desktop`.

## 5. Gate tests that exist and were executed

Function names matching `a0*_`, `a1*_`, `b0*_`, `st*_` (and the new `layering_*`) were extracted from the passing tests of the final run: 126 tests. A further 22 functions with gate prefixes are `pub fn` scenario bodies in `crates/g9-evaluation/src/{scenarios,adversarial}.rs` (for example `a04_finalized_grade_immutable`, `st09_replayed_stale_forged_approvals`). They are executed by the tests below and by `g9_evaluation::tests::gate_evidence_document`. They are not test entry points themselves.

| Gate | Executed tests | Gate | Executed tests |
|---|---|---|---|
| A01 | 7 | A09 | 6 |
| A02 | 10 | A10 | 7 |
| A03 | 6 | A11 | 5 |
| A04 | 9 | A12 | 0 (CI only, §6) |
| A05 | 6 | B01, B03, B04, B06 | 0 |
| A06 | 19 | B02, B05 | 1 each |
| A07 | 20 | ST01–ST12 | 1 each (+ `catalogue_covers_every_threat_model_scenario`) |
| A08 | 10 | ADR-0010 layering | 7 |

ADR "Verification" names checked against the run. Present and passing: `a08_snapshot_verify_restore_roundtrip`, `a08_crash_mid_transaction_rolls_back`, `a08_migration_checksum_tamper_detected`, `deletion_restrict_on_evidentiary_rows`, `a10_deletion_request_redacts_and_tombstones`, `foreign_keys_enforced`, `audit_chain_detects_tamper`, `a04_trigger_blocks_audit_row_mutation`, `invocation_record_has_no_prompt_text`, `a05_projection_is_replayable_and_order_independent`, `a05_model_sourced_observation_rejected`, `validated_requires_exam_mode_unaided`, `b02_transfer_checks_separate_from_item_reviews`, `baseline_intervals_monotone_and_reset_on_fail`, `checksum_mismatch_rejected`, `error_serializes_without_backtrace_or_secret_fields`, `categories_are_exactly_thirteen_and_closed`, `a07_requested_operator_only_capability_rejected_at_load`, `a07_output_requesting_execute_is_rejected_at_validation`. **Absent:** `pass_threshold_80_boundary` and `critical_gate_failure_yields_final_fail` (ADR-0012). g9-assessment has 3 unit tests, all in `rubric::tests`, and no grading or finalize tests in the crate.

### 5.1 Register of executed gate tests

| Gate | Test function | Crate | File |
|---|---|---|---|
| ADR-0010 | `layering_checks_detect_a_forbidden_edge_and_a_cycle` | g9-evaluation | `tests/integration/layering_gate.rs` |
| ADR-0010 | `layering_every_member_has_a_declared_layer` | g9-evaluation | `tests/integration/layering_gate.rs` |
| ADR-0010 | `layering_graph_is_acyclic` | g9-evaluation | `tests/integration/layering_gate.rs` |
| ADR-0010 | `layering_internal_edges_follow_the_dependency_law` | g9-evaluation | `tests/integration/layering_gate.rs` |
| ADR-0010 | `layering_nothing_depends_on_the_harness_or_the_shell` | g9-evaluation | `tests/integration/layering_gate.rs` |
| ADR-0010 | `layering_parser_reads_all_dependency_table_forms` | g9-evaluation | `tests/integration/layering_gate.rs` |
| ADR-0010 | `layering_sqlite_tauri_and_network_clients_are_confined` | g9-evaluation | `tests/integration/layering_gate.rs` |
| A01 | `a01_arithmetic_violations_come_from_domain_counts` | g9-curriculum | `crates/g9-curriculum/tests/a01_gate.rs` |
| A01 | `a01_curriculum_import_rejects_invariant_violation_end_to_end` | g9-app | `crates/g9-app/tests/gates.rs` |
| A01 | `a01_curriculum_invariants_and_provenance_end_to_end` | g9-evaluation | `tests/integration/a01_gate.rs` |
| A01 | `a01_promote_requires_reconciliation_and_approval` | g9-app | `crates/g9-app/tests/gates.rs` |
| A01 | `a01_rejects_each_violation` | g9-curriculum | `crates/g9-curriculum/tests/a01_gate.rs` |
| A01 | `a01_seed_is_not_promotable_without_reconciliation` | g9-curriculum | `crates/g9-curriculum/tests/a01_gate.rs` |
| A01 | `a01_seed_passes_all_invariants` | g9-curriculum | `crates/g9-curriculum/tests/a01_gate.rs` |
| A02 | `a02_all_day_events_are_date_only` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A02 | `a02_conflicts_detected_and_protected_rest_respected` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A02 | `a02_daily_recurrence_through_gap_reports_adjustment_once` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A02 | `a02_dst_fall_back_overlap_berlin` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A02 | `a02_dst_spring_forward_gap_berlin` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A02 | `a02_feb29_only_in_leap_years` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A02 | `a02_infeasible_schedule_reports_violations_not_silent_drop` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A02 | `a02_monthly_31st_skips_short_months` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A02 | `a02_new_york_and_sydney_transitions` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A02 | `a02_weekly_byday_recurrence_preserves_wall_time_across_dst` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A03 | `a03_credit_uses_monotonic_delta_not_wall_clock` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A03 | `a03_ledger_survives_serialization_roundtrip_after_restart` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A03 | `a03_overlapping_intervals_rejected` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A03 | `a03_restart_marks_open_interval_uncertain` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A03 | `a03_restart_recovery_marks_sessions_interrupted` | g9-app | `crates/g9-app/tests/gates.rs` |
| A03 | `a03_uncertain_time_not_credited_without_correction` | g9-calendar | `crates/g9-calendar/tests/gates.rs` |
| A04 | `a04_approve_final_requires_approval_and_operator_and_is_immutable_after` | g9-app | `crates/g9-app/tests/gates.rs` |
| A04 | `a04_finalized_grade_cannot_be_regraded_refinalized_reopened_or_deleted` | g9-evaluation | `tests/integration/a04_a05_gate.rs` |
| A04 | `a04_immutable_tables_reject_update_and_delete` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A04 | `a04_model_advisory_grade_never_finalizes` | g9-evaluation | `tests/integration/a04_a05_gate.rs` |
| A04 | `a04_model_output_cannot_be_competency_observation` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A04 | `a04_trigger_blocks_audit_row_mutation` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A04 | `a04_trigger_blocks_final_grade_update` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A04 | `a04_trigger_blocks_finalized_attempt_update` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A04 | `a04_trigger_blocks_terminal_approval_update` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A05 | `a05_contradiction_detected_between_exam_and_guided_results` | g9-competency | crates/g9-competency/src (unit, `derive::tests`) |
| A05 | `a05_correction_supersedes_and_changes_state` | g9-competency | crates/g9-competency/src (unit, `derive::tests`) |
| A05 | `a05_model_sourced_observation_rejected` | g9-competency | crates/g9-competency/src (unit, `observation::tests`) |
| A05 | `a05_projection_is_replayable_and_order_independent` | g9-competency | crates/g9-competency/src (unit, `derive::tests`) |
| A05 | `a05_projection_replays_correction_supersedes_and_model_evidence_is_refused` | g9-evaluation | `tests/integration/a04_a05_gate.rs` |
| A05 | `a05_regress_keeps_earlier_evidence` | g9-competency | crates/g9-competency/src (unit, `derive::tests`) |
| A06 | `a06_all_kinds_fixture_is_complete` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_builtin_rules_serialize_to_json` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_decision_records_are_metadata_only` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_deny_by_default_when_no_rule_matches` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_every_gated_operation_requires_a_bound_approval_and_mutates_nothing` | g9-evaluation | `tests/integration/a06_a07_gate.rs` |
| A06 | `a06_every_rule_has_unique_id` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_exam_mode_blocks_model_teaching_proposals_and_sealed_keys` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_finalized_records_never_mutated_exhaustive` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_matrix_covers_every_builtin_rule_and_default_deny` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_matrix_decisions_and_rule_ids` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_matrix_has_at_least_sixty_rows` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_native_actions_never_plain_allow_exhaustive` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_operator_only_capabilities_never_required_of_non_operators_exhaustive` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_pipeline_order_idempotency_before_policy_and_revision_check` | g9-evaluation | `tests/integration/a06_a07_gate.rs` |
| A06 | `a06_secret_content_never_reaches_skill_or_model_exhaustive` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_self_modification_only_in_maintenance_with_approval_exhaustive` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_skill_and_model_never_allowed_to_write_exhaustive` | g9-policy | `crates/g9-policy/tests/a06_policy_matrix.rs` |
| A06 | `a06_ui_actor_is_refused_and_approvals_bind_to_parameters` | g9-evaluation | `tests/integration/a06_a07_gate.rs` |
| A06 | `a06_ui_cannot_set_actor_identity` | g9-app | `crates/g9-app/tests/gates.rs` |
| A07 | `a07_attachment_paths_and_types_are_confined` | g9-evidence | `crates/g9-evidence/tests/gates.rs` |
| A07 | `a07_injected_allow_rule_for_skill_write_is_capped_by_invariant` | g9-policy | `crates/g9-policy/tests/a07_skill_privilege.rs` |
| A07 | `a07_injected_note_is_labeled_and_stays_data` | g9-evidence | `crates/g9-evidence/tests/gates.rs` |
| A07 | `a07_instruction_in_evidence_cannot_change_decision` | g9-skills | `crates/g9-skills/tests/orchestrator_tests.rs` |
| A07 | `a07_malicious_note_directing_schedule_edit_is_denied` | g9-policy | `crates/g9-policy/tests/a07_skill_privilege.rs` |
| A07 | `a07_model_cannot_decide_or_read_approvals` | g9-policy | `crates/g9-policy/tests/a07_skill_privilege.rs` |
| A07 | `a07_note_with_embedded_instructions_grants_nothing` | g9-evaluation | `tests/integration/a06_a07_gate.rs` |
| A07 | `a07_operator_approval_cannot_be_consumed_by_skill_for_shared_capability` | g9-policy | `crates/g9-policy/tests/a07_skill_privilege.rs` |
| A07 | `a07_output_requesting_execute_is_rejected_at_validation` | g9-skills | `crates/g9-skills/tests/orchestrator_tests.rs` |
| A07 | `a07_requested_operator_only_capability_rejected_at_load` | g9-skills | `crates/g9-skills/tests/package_tests.rs` |
| A07 | `a07_rule_table_cannot_grant_skill_native_or_operator_only_capabilities` | g9-policy | `crates/g9-policy/tests/a07_skill_privilege.rs` |
| A07 | `a07_skill_cannot_finalize_or_delete_even_with_approval` | g9-policy | `crates/g9-policy/tests/a07_skill_privilege.rs` |
| A07 | `a07_skill_filesystem_write_denied_even_with_approval` | g9-policy | `crates/g9-policy/tests/a07_skill_privilege.rs` |
| A07 | `a07_skill_invoke_output_requesting_execute_is_rejected_and_nothing_persisted` | g9-app | `crates/g9-app/tests/gates.rs` |
| A07 | `a07_skill_model_load_and_policy_modify_denied_even_with_approval` | g9-policy | `crates/g9-policy/tests/a07_skill_privilege.rs` |
| A07 | `a07_skill_network_egress_denied_even_with_approval` | g9-policy | `crates/g9-policy/tests/a07_skill_privilege.rs` |
| A07 | `a07_skill_or_model_cannot_redact_or_approve` | g9-evidence | `crates/g9-evidence/tests/gates.rs` |
| A07 | `a07_skill_output_requesting_execute_or_connect_is_rejected` | g9-evaluation | `tests/integration/a06_a07_gate.rs` |
| A07 | `a07_skill_shell_exec_denied_even_with_approval` | g9-policy | `crates/g9-policy/tests/a07_skill_privilege.rs` |
| A07 | `a07_tampered_skill_package_is_refused_and_cannot_be_invoked` | g9-evaluation | `tests/integration/a06_a07_gate.rs` |
| A08 | `a08_backup_commands_drive_state_machine_to_retained` | g9-app | `crates/g9-app/tests/gates.rs` |
| A08 | `a08_backup_state_machine_requires_restore_test_before_retain` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A08 | `a08_backup_verify_restore_crash_reopen_recover` | g9-evaluation | `tests/recovery/a08_recovery_drill.rs` |
| A08 | `a08_crash_mid_transaction_rolls_back` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A08 | `a08_dry_run_migration_on_copy` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A08 | `a08_migration_checksum_tamper_detected` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A08 | `a08_schema_newer_than_app_refused` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A08 | `a08_snapshot_refuses_overwrite_and_live_path` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A08 | `a08_snapshot_verify_restore_roundtrip` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A08 | `a08_vacuum_into_export_verifies` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A09 | `a09_all_deterministic_commands_work_with_null_runtime` | g9-app | `crates/g9-app/tests/gates.rs` |
| A09 | `a09_backend_unavailable_yields_error_not_panic` | g9-skills | `crates/g9-skills/tests/orchestrator_tests.rs` |
| A09 | `a09_cancelled_response_is_discarded` | g9-ai-adapter | `crates/g9-ai-adapter/tests/gates.rs` |
| A09 | `a09_deterministic_commands_work_and_skills_fail_closed_with_the_model_down` | g9-evaluation | `tests/integration/a09_model_down.rs` |
| A09 | `a09_harness_model_down_option_builds_the_null_runtime` | g9-evaluation | `tests/integration/a09_model_down.rs` |
| A09 | `a09_null_runtime_reports_model_unavailable_as_resource_unavailable` | g9-ai-adapter | `crates/g9-ai-adapter/tests/gates.rs` |
| A10 | `a10_approved_deletion_redacts_note_and_versions_with_tombstone` | g9-evaluation | `tests/integration/a10_export_deletion.rs` |
| A10 | `a10_deletion_request_gated_and_tombstones_content` | g9-app | `crates/g9-app/tests/gates.rs` |
| A10 | `a10_deletion_request_redacts_and_tombstones` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A10 | `a10_export_requires_an_approval_bound_to_the_selection_and_is_recorded_once` | g9-evaluation | `tests/integration/a10_export_deletion.rs` |
| A10 | `a10_redaction_leaves_tombstone_and_releases_content_pointer` | g9-evidence | `crates/g9-evidence/tests/gates.rs` |
| A10 | `a10_reflection_erasure_and_rejected_requests` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A10 | `a10_study_session_erasure_is_explicitly_not_implemented` | g9-storage | `crates/g9-storage/tests/gates.rs` |
| A11 | `a11_budget_and_cancellation_boundaries` | g9-evaluation | `tests/integration/a11_output_validation.rs` |
| A11 | `a11_invalid_structured_output_rejected_with_paths` | g9-ai-adapter | `crates/g9-ai-adapter/tests/gates.rs` |
| A11 | `a11_schema_invalid_output_rejected` | g9-skills | `crates/g9-skills/tests/orchestrator_tests.rs` |
| A11 | `a11_valid_output_against_composed_envelope` | g9-ai-adapter | `crates/g9-ai-adapter/tests/gates.rs` |
| A11 | `a11_valid_output_is_advisory_and_invalid_output_is_rejected` | g9-evaluation | `tests/integration/a11_output_validation.rs` |
| B02 | `b02_transfer_checks_separate_from_item_reviews` | g9-retention | crates/g9-retention/src (unit, `transfer::tests`) |
| B05 | `b05_rejected_hypothesis_invalidates_dependents` | g9-competency | crates/g9-competency/src (unit, `memory::tests`) |
| ST01 | `st01_note_with_embedded_instructions_is_data` | g9-evaluation | `tests/adversarial/st_adversarial_catalogue.rs` |
| ST02 | `st02_tampered_skill_package_is_refused` | g9-evaluation | `tests/adversarial/st_adversarial_catalogue.rs` |
| ST03 | `st03_forged_model_provenance_observation_is_rejected` | g9-evaluation | `tests/adversarial/st_adversarial_catalogue.rs` |
| ST04 | `st04_skill_output_requesting_execute_is_rejected` | g9-evaluation | `tests/adversarial/st_adversarial_catalogue.rs` |
| ST05 | `st05_private_export_requires_approval` | g9-evaluation | `tests/adversarial/st_adversarial_catalogue.rs` |
| ST06 | `st06_modified_model_artifact_is_refused` | g9-evaluation | `tests/adversarial/st_adversarial_catalogue.rs` |
| ST07 | `st07_markup_payloads_are_stored_as_data` | g9-evaluation | `tests/adversarial/st_adversarial_catalogue.rs` |
| ST08 | `st08_oversized_context_and_over_budget_reply_are_refused` | g9-evaluation | `tests/adversarial/st_adversarial_catalogue.rs` |
| ST09 | `st09_replayed_stale_and_forged_approvals_are_denied_with_reasons` | g9-evaluation | `tests/adversarial/st_adversarial_catalogue.rs` |
| ST10 | `st10_corrupted_backup_fails_verification_and_restore_is_refused` | g9-evaluation | `tests/adversarial/st_adversarial_catalogue.rs` |
| ST11 | `st11_cross_domain_access_is_denied` | g9-evaluation | `tests/adversarial/st_adversarial_catalogue.rs` |
| ST12 | `st12_late_response_after_cancellation_is_discarded` | g9-evaluation | `tests/adversarial/st_adversarial_catalogue.rs` |

## 6. NOT RUN

| Item | Reason |
|---|---|
| `cargo check/clippy/test -p g9-mentor-desktop` (Tauri `shell` module, `generate_context!`, `tauri-build`) | `gdk-3.0`/`webkit2gtk-4.1` system libraries are missing on this host (§4) |
| `cargo test --workspace` | stops at the same `gdk-sys` build script; default members were run instead |
| Tauri bundle (`deb`) and release build | needs the GTK/WebKit libraries and `src-tauri/icons/` |
| `cargo audit`, `npm audit` (A12) | `cargo-audit` is not installed; `npm audit` needs registry network access (not attempted) |
| Accessibility (axe) and Playwright e2e (A12) | `tests/e2e/` is empty (PROPOSED in `tests/README.md`) |
| B01, B03, B04, B06 effectiveness gates | no executable tests exist (`docs/evaluations/effectiveness-gates.md`) |
| Native confirmation dialog for protected operations (ADR-0014, Addendum A.7) | not implemented; no dialog plugin is registered (PROPOSED, Stage 03) |
| `G9_WRITE_EVIDENCE=1` regeneration of `docs/evaluations/gate-evidence.md` | not run; the rendering assertions ran inside `gate_evidence_document` without writing |

## 7. Deviations from ARCHITECTURE.md (observed in code; none introduced by this verifier)

| # | ARCHITECTURE / ADR | Observed | Label |
|---|---|---|---|
| D1 | Addendum A.1 / ADR-0012: PASS requires total ≥ 80 **and** every criterion ≥ 50 % of its weight **and** no gate failure | `g9_assessment::grading::score_breakdown` computes `pass_recommendation = sum ≥ 80·weight && critical_gates_passed`, with no per-criterion minimum. `finalize` uses that value, and no g9-app code adds the minimum. The rule is therefore still compensatory for non-gate criteria (LRN-10). It is a wave-1 crate, and no failing test points at it, so it was not changed here. | TESTED (code read) / open |
| D2 | Addendum A.2 / ADR-0005 addendum: SELF_ADJUDICATED decisions yield at most `Independent` + `provisional` | g9-app records `reliability_class` (DETERMINISTIC when every non-advisory result is from the checker, otherwise SELF_ADJUDICATED; EXTERNAL_REVIEW is never assigned). `competency.projection` always returns `provisional: false`, and the derivation does not read the class. The path is not currently exploitable because `assessment.approve_final` derives no observations (warning `competency.observations_not_derived`). | INFERRED / PROPOSED |
| D3 | §5, ADR-0014: protected operations get a native confirmation | absent (see §6) | PROPOSED |
| D4 | §2 rule 6: "the Tauri layer exposes only g9-app commands" | the shell exposes one command (`g9_command` → `g9_app::dispatch`). It also depends on g9-storage, g9-policy and g9-skills, but only for bootstrap types (`Database::open`, `BindingKey`, `Registry::load`). The layering gate allows this explicitly; it is not an exposure. | TESTED (layering gate) |
| D5 | §3 M12/M13/M14/M15: teaching, analytics, executive, research | 15 IPC operations (teaching 4, analytics 3, executive 4, research 4) return `PRECONDITION_FAILED <area>.not_implemented` from `g9_app::dispatch`. `calendar.propose` intents `FILL_CURRICULUM`/`RESCHEDULE` return `calendar.*.not_implemented`. `export.request` records the request but writes no file (warning `export.execution_not_implemented`). Study-session erasure is `deletion.study_session.not_implemented`. | PROPOSED (explicit, not presented as done) |
| D6 | §8 attempt difficulty | `g9-app` rehydrates `Attempt` with a placeholder `Difficulty(1)`, because assessment versions store no difficulty. `finalize` does not read it and no observation is derived (`crates/g9-app/src/commands/assessment.rs` `domain_attempt`). | TESTED (code read) / latent risk |
| D7 | ADR-0010 layering test "wave 2" | was missing; added in §3 | TESTED |

Dependency law (§2): checked with `cargo metadata` and by `layering_gate`. Every internal edge is allowed; g9-ai-adapter depends only on g9-domain and has no `rusqlite`; g9-skills does not depend on g9-storage; g9-app contains no SQL text (grep for `SELECT/INSERT/UPDATE/DELETE/CREATE`, `.execute(` and `.prepare(` found nothing).

## 8. Wave-1 API gaps recorded by g9-evaluation (`crates/g9-evaluation/src/lib.rs` `WAVE1_GAPS`)

| # | Gap | Gate |
|---|---|---|
| G1 | `notes.create` does not run `g9_evidence::classify_text` or persist `content_classifications`; the instruction-like flag is not recorded | ST01 |
| G2 | no IPC cancellation operation; late-response discard is exercised at the orchestrator boundary | ST12 |
| G3 | no IPC path creates study sessions, competencies, assessment definitions or observations; fixtures insert through the repositories | A04/A05 |
| G4 | a schema-invalid model reply surfaces as `INTERNAL skills.backend.runtime` instead of `PRECONDITION_FAILED skills.output_rejected` (the reply is still rejected and nothing is persisted) | A11 |
| G5 | no model-load IPC operation; ST06 is checked at `g9_ai_adapter::verify_checksum` only | ST06 |
| G6 | `backup.verify`/`restore_test` know only backups created by the same process (no manifest reader in g9-storage) | A08 |
| G7 | a LEARNING-only skill invoked in BUSINESS scope is allowed by policy and refused by the orchestrator preflight; the refusal is not a policy decision row | ST11 |
| G8 | no IPC path ingests observations, so ST03 contradiction flagging is evidenced only as a storage refusal | ST03 |

## 9. Files changed by wave-2 engineers outside their stated paths (INFERRED from file modification times and engineer reports; `git` not run)

| File | Engineer | Note |
|---|---|---|
| `crates/g9-storage/src/migrations/0010_intervention_review_and_reliability.sql` (new), `crates/g9-storage/src/migrations.rs`, `crates/g9-storage/src/backup.rs`, `crates/g9-storage/tests/gates.rs` | g9-storage (assigned `src/repos`) | 0010 adds columns and triggers only. Applied migrations 0001–0009 are not edited, and the 0007 CHECK is unchanged. |
| `schemas/ipc/curriculum.promote.{request,response}.schema.json` | g9-app | registered in `operations.ts`; UI parity passes |
| `data/fixtures/*.json`, `data/fixtures/schemas/*.schema.json` | g9-evaluation | synthetic fixtures, schema-validated before load |
| `apps/desktop/src-tauri/Cargo.toml` (`keyring` crate removed, `getrandom = "=0.4.3"`) | g9-mentor-desktop | `cargo-deny` advisories/licenses ok |

## 10. Open risks

1. **D1 (pass rule).** Until the per-criterion minimum is implemented in `g9-assessment` (or applied in `g9-app` before finalize), a final grade can be PASS with a criterion below 50 % of its weight. ADR-0012 named tests do not exist.
2. **Desktop shell unverified.** The Tauri wiring and the missing `icons/` directory have to be built on a prepared Ubuntu 26.04 host with `libwebkit2gtk-4.1-dev` and `libgtk-3-dev`.
3. **G4 error mapping.** The UI receives `INTERNAL` for an invalid model reply. That is the right fail-closed behaviour but the wrong category under ADR-0013.
4. **Disk pressure on the build host.** The per-engineer `target/<crate>` directories total about 27 GB. Concurrent full builds exhausted the disk once during this verification.
5. **Approval binding key** lives only in a `0600` file (`<data>/binding.key`); the OS keyring was dropped for the A12 advisory/licence gate. `system.status` reports a WARNING on every start (desktop engineer report; the keyring tests pass in the harness).
