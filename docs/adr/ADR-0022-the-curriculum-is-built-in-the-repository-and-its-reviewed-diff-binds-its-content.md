# ADR-0022: The curriculum is built in the repository, and a reviewed diff binds the content it reviewed

**Status:** Accepted (2026-11-14). Carries out row 8 of ADR-0020 (the twelve-week curriculum is built with the owner, in the repository, from the Appendix A skeleton) and corrects a gap in the reconciliation block found while testing it.

## Context

The shipped curriculum was the Appendix A transcription: 72 day ids, three objective titles per day, a deliverable name per day and the standard time allocation, with no lesson, no success criterion, no resource and no link to the competency packs. The owner asked for the twelve-week curriculum to be built with them. The application plans from a manifest (`g9-curriculum`, schema 1.0.0); a manifest becomes `APPROVED_CANONICAL` only through `curriculum.promote`, which needs a reconciliation against an original and an operator approval.

While testing the promotion of the built curriculum against the seed, a candidate whose reviewed objective was reworded after the review still matched its reconciliation block. `ReconciliationReport::diff_checksum` hashed `per_day` and `summary`, and a changed day records the names of the fields that differ (`objectives[1].text`), not their values. The block therefore bound the shape of a diff, not what the candidate says.

## Decision

1. **One source, generated artifacts.** The curriculum is written as 12 Markdown files (`docs/curriculum/weeks/week-NN.md`: purpose, lab stack, competencies, accelerator and six day sheets each). `scripts/build_curriculum.py` checks them and generates `data/curriculum/g9-ct9-480.draft.v1.json` (`PROPOSED_SEED`, version `1.0.0-draft1`), 12 assessment definitions (`data/curriculum/assessments/`) and `docs/curriculum/coverage.md`. Nothing generated is edited by hand; the default run fails when a generated file is stale.
2. **An enrichment, not a replacement.** Every Appendix A objective keeps its wording as the start of its text and gains a success criterion; every artifact name starts its hand-in; ids, week titles, minutes, blocks and the Saturday assessment flags are the seed's. The builder, `scripts/check_curriculum_seed.py` and `crates/g9-curriculum/tests/draft_v1.rs` (the application's own parser, validator and diff) all check this.
3. **The diff checksum binds the candidate's content.** `reconcile` adds `candidate_content_checksum` to the report: the checksum of the candidate without its provenance fields (`manifest_schema_version`, `version`, `provenance`, `provenance_note`, `reconciliation`). `diff_checksum` now hashes `per_day`, `summary` and that checksum. Relabelling a candidate (a new version, `RECONCILED_CANDIDATE`, the block) leaves it unchanged, so the block can be written before it exists in the manifest; any edit to the content changes it, so the block stops matching.
4. **A tool for the review.** `g9 reconcile` validates two manifest files, prints the diff (days grouped by the fields that changed) and the checksums, and, only with explicit `--candidate-version`, `--approved-by` and `--approved-at`, writes the `RECONCILED_CANDIDATE` manifest and the payloads of `curriculum.import` and `curriculum.promote`. Nothing defaults: the block says that this operator reviewed this diff, and the command never says that itself. It reads two files, opens no data directory, never overwrites a file and re-checks what it wrote.
5. **Promotion stays the operator's.** The block records a review; `curriculum.promote` and its approval are the authority. No test, script or agent promotes the curriculum. The draft stays `PROPOSED_SEED` until the operator runs the steps of `docs/curriculum/promotion.md`.
6. **The mapping to competencies is carried, not stored.** Each objective lists one to three competency ids (validated against the packs by the builder, validated as slugs by the application). The importer does not persist them: `objective_competencies` needs a dimension and a weight that the manifest does not carry and that would be invented numbers. Open (PROPOSED): a manifest field for them.

## Alternatives considered

| Alternative | Why not |
|---|---|
| Hand-edit the manifest JSON | 296 competency links, 256 resources and 216 objective texts cannot be reviewed as JSON; the sheets would drift from the data |
| Leave `diff_checksum` as it was and document it | The block is the record that a diff was reviewed; a record that survives an edit of what was reviewed is a false record. The fix is one hash input and a test |
| Put the manifest checksum into the block | The checksum covers the block: circular |
| Bind each changed day's digest in `per_day` | Same effect for days, but top-level fields (`name`, week titles) would stay unbound |
| Promote from the application window | The window has no promotion action; adding one is a separate change with its own confirmation design (PROPOSED) |

## Consequences

- `diff_checksum` values computed before this change differ. No block had been written for any real manifest (the seed was never promoted), so nothing in use is invalidated. The mock transport computes its own value and is unchanged.
- `ReconciliationReport` gains a field (`candidate_content_checksum`); the IPC responses are unchanged.
- The owner reviews 72 day sheets, not a diff of field names: the report's grouping shows that every day changed in the same eight fields, which is what an enrichment looks like, and the content is read in `docs/curriculum/weeks/`.
- Every external resource is `ASSUMED` (not opened) unless its licence note says `CHECKED <date> by search result only, page not opened`. Three hosts that the authoring session's network did not allow (`genai.owasp.org`, `modelcontextprotocol.io`, `attack.mitre.org`) could not be opened even for a spot check; allowing them in the session's network settings lets those resources be checked.

## Verification

| Check | State |
|---|---|
| `crates/g9-curriculum/tests/a01_gate.rs::a01_reviewed_diff_is_bound_to_the_candidate_content` fails before the change and passes after (the same edit in an already-changed field and in a changed top-level field) | TESTED 2026-11-14 |
| `crates/g9-curriculum/tests/draft_v1.rs` (4 tests): valid and non-authoritative; same structure and wording as the seed; the diff touches only the eight enrichment fields; promotion needs a block bound to this diff | TESTED 2026-11-14 |
| `apps/cli/src/commands/reconcile.rs` (8 unit tests) and `apps/cli/tests/curriculum_promotion.rs` (3 tests): the route through the binary in a throwaway data directory, including the refusal of an edited candidate and of a tampered payload | TESTED 2026-11-14 |
| The route on the owner's data directory | NOT RUN |
