# Promoting the curriculum draft to APPROVED_CANONICAL

The draft `data/curriculum/g9-ct9-480.draft.v1.json` is built from `docs/curriculum/weeks/` and is `PROPOSED_SEED`: the application plans from it and labels every plan "Proposed". It becomes `APPROVED_CANONICAL` only through the steps below. Every step that changes the installation is an operator approval; this document never replaces one. Procedure rules: `provenance-and-reconciliation.md`.

## The four manifests

| Manifest | File | Provenance | Role |
|---|---|---|---|
| Appendix A seed | `data/curriculum/g9-ct9-480.seed.v0.json` (`0.1.0-seed`) | `PROPOSED_SEED` | the **original** in the reconciliation: the skeleton the draft was built from |
| Draft | `data/curriculum/g9-ct9-480.draft.v1.json` (`1.0.0-draft1`) | `PROPOSED_SEED` | the **candidate**; generated, never edited by hand (`python3 -I scripts/build_curriculum.py --write`) |
| Reconciled candidate | written by `g9 reconcile` (you name the file and the version, for example `1.0.0-rc1`) | `RECONCILED_CANDIDATE` | the draft plus a block that records who reviewed the diff against the seed and when |
| Canonical | a new row in the database, made by `curriculum.promote` | `APPROVED_CANONICAL` | what the plans stop labelling "Proposed" for; earlier rows are never rewritten |

## What only you can do

| Review | Where | Why it is not done for you |
|---|---|---|
| Read the 72 day sheets and the 12 Saturday assessments | `docs/curriculum/weeks/week-01.md` to `week-12.md` | the approval says the content is acceptable to teach from |
| Read the coverage matrix and the list of changes against the seed | `docs/curriculum/coverage.md` | shows which objective builds which of the 64 competencies |
| Settle the open items | `docs/curriculum/design.md` "Not decided by this document" | for example the source of each Saturday blind practical |
| Check the resources you will rely on | the `### Resources` lines of each day | every external resource is `ASSUMED` unless its note says `CHECKED`; none was opened, so terms, versions and availability are unverified |

## Steps

Close the window first: the CLI and the window must not write the same data directory at once. `G9` is the `g9` binary; `D` is the data directory of your installation (`docs/runbooks/first-run.md`).

| # | Command | You should see |
|---|---|---|
| 0 | `python3 -I scripts/build_curriculum.py` and `python3 -I scripts/check_curriculum_seed.py data/curriculum/g9-ct9-480.draft.v1.json` | `generated files are current`; `PASS` with canonical checksum `836f989e…a121fc` |
| 1 | `$G9 reconcile --original data/curriculum/g9-ct9-480.seed.v0.json --candidate data/curriculum/g9-ct9-480.draft.v1.json --human` | `days: 0 unchanged, 72 changed, 0 missing, 0 added`; `metadata_changes: []`; one group of 72 days, each changed in the same eight fields (the three objective texts, the three competency lists, `evidence_artifact`, `resources`); `binding: ABSENT`; `promotion_allowed: false`. Note the three checksums |
| 2 | `$G9 reconcile --original <seed> --candidate <draft> --write-candidate <dir>/rc1.json --candidate-version 1.0.0-rc1 --approved-by <your actor id> --approved-at now` | `binding: MATCHED` and three new files: `rc1.json`, `rc1.import.json`, `rc1.promote.json`. The three review flags have no default: the block says that you reviewed this diff. An existing file is never overwritten |
| 3 | `$G9 call curriculum.import --data-dir "$D" --payload-file <dir>/rc1.import.json` | `APPROVAL_REQUIRED` and an approval id |
| 4 | `$G9 approvals --data-dir "$D" --state PENDING`, then `$G9 approve <id> --data-dir "$D"` | the request you are approving; then the same command as step 3 with `--approval <id>` answers `provenance: RECONCILED_CANDIDATE` |
| 5 | `$G9 call curriculum.promote --data-dir "$D" --payload-file <dir>/rc1.promote.json`, approve as in step 4, repeat with `--approval <id>` | `provenance: APPROVED_CANONICAL`, the original, candidate and diff checksums of steps 1 and 2 |
| 6 | open the window | Classroom and Navigator read "Verified"; a plan carries no `curriculum.non_canonical` warning |

The window has no promote action yet (PROPOSED). Step 5 is the only way to promote.

## What each value binds

| Value | Binds |
|---|---|
| `original_checksum` | the exact original (the seed): any change to it, a different original, or a different curriculum id blocks promotion |
| `candidate content checksum` | the candidate's curriculum content without its provenance fields (`manifest_schema_version`, `version`, `provenance`, `provenance_note`, `reconciliation`); unchanged when the candidate is relabelled, changed by any edit |
| `diff_checksum` (the block) | which days differ in which fields, **and** the candidate content checksum: rewording an objective after the review breaks it, even in a field that already differed (before 2026-11-14 it bound only the field names; `ADR-0022`) |
| `candidate_checksum` (`manifest_sha256`) | the whole candidate manifest as stored: `curriculum.promote` refuses a candidate that does not hash to the stored version |
| the approval | the exact request, payload included (ADR-0003); an approval for another payload is refused |

The block records the review; the approval of `curriculum.promote` is the authority. A reconciliation block alone never promotes.

## Refusals

| Code | Meaning | Do |
|---|---|---|
| `curriculum.invariant_violated` | a manifest fails the structural contract (`g9 reconcile` names which one) | rebuild with `build_curriculum.py` |
| `reconcile.target_exists` | a file `--write-candidate` would create exists | choose a new name |
| `curriculum.version_exists` | that version label is already imported | use a new `--candidate-version` |
| `curriculum.provenance_mismatch` | the payload's `provenance` differs from the manifest's | use the payload `g9 reconcile` wrote |
| `curriculum.candidate_checksum_mismatch` | the promote payload's candidate is not the imported one | use the `.promote.json` written with that `.json` |
| `curriculum.promotion_blocked` | the block does not match this original and this diff, or the ids differ; the message names which | the candidate changed after the review: reconcile again and review the new diff |

## Changing the curriculum afterwards

A later edit is a new version: edit the week files, rebuild, choose a new draft version label in `scripts/build_curriculum.py`, then run steps 1 to 5 with the **previous canonical manifest as the original**. Nothing already promoted is rewritten. Plans already made keep the structure they were made from.

## Verification state

| Claim | State |
|---|---|
| The route (reconcile, write, import, promote) runs on the real draft and the real seed in a throwaway data directory, including the refusals above | TESTED 2026-11-14: `apps/cli/tests/curriculum_promotion.rs`, `crates/g9-curriculum/tests/draft_v1.rs`, `a01_gate.rs` |
| The same route on your data directory | NOT RUN: it is your approval |
| The window cannot promote | PROPOSED: a Settings panel is not built |
| Resources and licences of the external references | ASSUMED, see above |
