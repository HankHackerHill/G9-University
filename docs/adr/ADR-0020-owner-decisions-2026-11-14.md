# ADR-0020: Owner decisions of 2026-11-14

**Status:** Accepted (2026-11-14). Records the owner's answer to the remediation report of 2026-11-14 (commit `6063ac4`, `docs/release/stage-report.md`, "Update 2026-11-14 (later)").

## Context

The report ended with decisions taken without the owner, two open owner decisions and a list of what was left. The owner answered: "All your recommendations and help me build the 12 week cybersecurity curriculum". This ADR records what that accepts. Where the report named a decision for the owner without a recommendation, the recommendation recorded here is the engineering one; the owner can reverse any row.

## Decisions

| # | Decision | Recommendation accepted | Where it lives | Reversal |
|---|---|---|---|---|
| 1 | An approved erasure also removes the attachment files of the erased note | Yes (ADR-0017 stands, now confirmed) | `crates/g9-app/src/commands/privacy.rs`, ADR-0017 | one argument, ADR-0017 "Consequences" |
| 2 | A declined native confirmation leaves one metadata-only audit row | Yes | `pipeline::record_declined_confirmation`, one call in `dispatch_confirming` | delete the call |
| 3 | A production bundle without the Tauri runtime refuses to run on the mock | Yes | `chooseTransport(…, production)`, `apps/desktop/src/ipc/index.ts` | the third argument |
| 4 | The backup operations are exempt from the EXAM derivation of the interaction mode | Yes (ADR-0019) | `OpSpec::exam_exempt` in `crates/g9-app/src/ops.rs` | clear the flag on the three operations |
| 5 | `g9-skills` depends directly on `libc =0.2.191` on Unix | Yes (platform constants for `O_NOFOLLOW`; the version `Cargo.lock` already held) | `crates/g9-skills/Cargo.toml` | read package files without `O_NOFOLLOW` (the type and identity checks stay) |
| 6 | The hashes of whole requests in the immutable ledgers are keyed (HMAC-SHA-256 with the binding key) | Yes. It changes what every approval is bound to (a pending approval at the time of the upgrade expires unused), so it ships as its own change and its own ADR | ADR-0021 | see ADR-0021 |
| 7 | `core:default` in the shell's capability file is reduced to an explicit list | Yes, **on the host, after one run on a display**. It is not done blind: a wrong capability list stops the window from starting, and the authoring sandbox has no GTK or WebKit. ARCHITECTURE.md names `core:default` and needs the same edit by its owner | host checklist, `docs/audit/security-audit-2026-11-13.md` section 5 | n/a (not changed) |
| 8 | The twelve-week curriculum is built with the owner, in the repository, starting from the Appendix A skeleton | Yes | `docs/curriculum/`, ADR-0022 | n/a |

## Not decided here

- Promotion of any curriculum version to `APPROVED_CANONICAL`: that is an operator approval bound to a checksum and a reviewed diff (`curriculum.promote`), given in the application, never by this record.
- The PNG-plus-archive polyglot, the `already_stored` oracle, the CLI's default data directory (INFO, unchanged).
