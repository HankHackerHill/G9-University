/**
 * The official baseline rubric and its anchored level scale, mirrored from the backend:
 *   - criteria: `g9_domain::assessment::BaselineRubric::CRITERIA` (crates/g9-domain/src/assessment.rs)
 *   - pass cut: `g9_domain::assessment::PASS_THRESHOLD`
 *   - level scale and floor: `g9_assessment::rubric::LevelScale::STANDARD`, `CRITERION_FLOOR_PERCENT`
 *     (crates/g9-assessment/src/rubric.rs; defect register LRN-10 option a, ADR-0012)
 *
 * The backend rejects any criterion code that is not part of the attempt's rubric
 * (`INVALID_INPUT grade.criterion_unknown`), so the UI must send exactly these codes.
 * `src/lib/rubric.test.ts` parses the Rust sources and fails when the two drift.
 * Carrying the rubric in the `assessment.list` response instead of mirroring it is PROPOSED
 * (needs a schema and `g9-app` view change outside apps/desktop).
 */
import type { AnchoredLevel } from '../ipc/operations';

export interface RubricCriterionDef {
  readonly code: string;
  readonly title: string;
  /** Percent of the total; the six weights sum to 100. */
  readonly weight: number;
  readonly machine_checkable: boolean;
  /** Missing the per-criterion floor on a critical criterion is a gate failure. */
  readonly critical_gate: boolean;
}

export const BASELINE_RUBRIC: readonly RubricCriterionDef[] = [
  { code: 'independent_execution', title: 'Independent execution', weight: 25, machine_checkable: true, critical_gate: false },
  { code: 'root_cause_reasoning', title: 'Root-cause reasoning', weight: 20, machine_checkable: false, critical_gate: false },
  { code: 'remediation_verification', title: 'Remediation and verification', weight: 20, machine_checkable: true, critical_gate: false },
  { code: 'detection_visibility', title: 'Detection and visibility', weight: 15, machine_checkable: false, critical_gate: false },
  { code: 'evidence_documentation', title: 'Evidence/documentation', weight: 10, machine_checkable: false, critical_gate: false },
  { code: 'scope_compliance_reporting', title: 'Scope compliance and reporting', weight: 10, machine_checkable: false, critical_gate: true },
];

/** Total (percent) at or above which an attempt passes, together with the floor and the gates. */
export const PASS_THRESHOLD = 80;

/** Every criterion must reach at least this percent of its weight. */
export const CRITERION_FLOOR_PERCENT = 50;

/** Anchored level 0..4 → criterion score (percent of the criterion weight). */
export const LEVEL_SCALE: readonly [number, number, number, number, number] = [0, 25, 50, 80, 100];

export const ANCHORED_LEVELS: readonly AnchoredLevel[] = [0, 1, 2, 3, 4];

/** Parses an operator-entered level. Only a single digit 0–4 counts; blank or anything else is `null`. */
export function parseLevel(raw: string | undefined): AnchoredLevel | null {
  if (raw === undefined) {
    return null;
  }
  const t = raw.trim();
  return /^[0-4]$/.test(t) ? (Number(t) as AnchoredLevel) : null;
}

export function scoreForLevel(level: AnchoredLevel): number {
  return LEVEL_SCALE[level];
}
