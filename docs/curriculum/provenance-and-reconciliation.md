# Curriculum provenance and reconciliation (gate A01, corrections C01/C02)

## Provenance states

| State | Meaning | May drive official scheduling/assessment? |
|---|---|---|
| `PROPOSED_SEED` | Appendix A transcription (`data/curriculum/g9-ct9-480.seed.v0.json`) or any unreconciled manifest | No |
| `RECONCILED_CANDIDATE` | Compared day-by-day against the original approved plan; `ReconciliationReport` attached; diff reviewed | No |
| `APPROVED_CANONICAL` | Operator approval bound to the candidate checksum (`Capability::CurriculumPromote`) | Yes |

The seed is **not** the original approved curriculum: it carries the 72 day ids, 3 objectives per day, deliverables and the standard weekly time allocation, but not verbatim lessons, exact resources or lab instructions.

## Structural contract (enforced by `g9-curriculum::validate` and `scripts/check_curriculum_seed.py`)

12 weeks · 6 days/week · 72 day ids `D01..D72` in order · 3 objectives/day = 216 · weekdays 420 min (recall 15, theory 45, guided lab 120, independent lab 120, defense/verification 60, evidence/reporting 60) · Saturday 300 min (blind practical 120, remediation 60, report 60, performance review 60) · 2,400 min/week · 28,800 min = 480 h · Sunday unscheduled · Saturday is the assessment day · unique ids · no missing deliverables · no negative durations · competency links well-formed · external resources versioned.

## Reconciliation procedure

1. Operator supplies the original approved plan as a manifest in the same format (or a transcription with its own provenance note and source checksum).
2. `curriculum.validate` on both; both must pass the structural contract (deviations in the original are reported, not auto-fixed).
3. `g9_curriculum::reconcile(original, candidate)` (on the command line: `g9 reconcile --original <file> --candidate <file>`) produces: both checksums; the candidate's content checksum (the manifest without its provenance fields); per-day status (unchanged / changed fields / missing / added); summary counts; a human-readable diff; the diff checksum, which binds the per-day statuses, the summary **and the candidate's content checksum** (so an edit after the review changes it, ADR-0022); `promotion_allowed`.
4. Operator reviews the diff (the report of `g9 reconcile`, and the content it points to: for the built curriculum, `docs/curriculum/weeks/` and `docs/curriculum/coverage.md`); any changed reference must carry a version; replaced resources (e.g. a free alternative to a paid lab) are recorded with license notes in `resource_references`. The Curriculum Navigator shows the plan the application derives from the imported version, not the diff.
5. `curriculum.promote` command: requires an approval bound to the candidate checksum and the report hash; writes a new immutable `curriculum_versions` row with provenance `APPROVED_CANONICAL`; earlier versions are never rewritten (historical planned totals preserved). The candidate must first be stored as `RECONCILED_CANDIDATE`: `g9 reconcile --write-candidate …` (with the explicit review flags) writes that manifest and the payloads of `curriculum.import` and `curriculum.promote`; the steps, each approved by the operator, are in `promotion.md`.
6. Accelerated pathways for an expert learner (C22) are **overlays**: a diagnostic calibration may mark objectives as `SKIP_PROPOSED` with evidence; the operator opts in per objective; the canonical structure and planned totals are unchanged.

## Resource and license register (ASSUMED until verified per item)

PortSwigger Web Security Academy, Hack The Box (free tier), CyberDefenders, OverTheWire, Splunk BOTS datasets, LetsDefend — each must be verified for current free-tier terms and redistribution/embedding rights before being referenced from an `APPROVED_CANONICAL` version. Verification outcome is recorded per `resource_references` row (`verification: VERIFIED|ASSUMED`, `verified_at`, `license_note`).
