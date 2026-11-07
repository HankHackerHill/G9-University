// @vitest-environment node
/**
 * Parity between the UI/mock rubric mirror and the Rust source of truth. The backend rejects any
 * criterion code outside the attempt's rubric (`grade.criterion_unknown`), so drift here breaks
 * grading against the real backend while mock-backed tests keep passing.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { BASELINE_CRITERIA } from '../ipc/transports/mock';
import { BASELINE_RUBRIC, CRITERION_FLOOR_PERCENT, LEVEL_SCALE, PASS_THRESHOLD, parseLevel } from './rubric';

const DOMAIN_RS = fileURLToPath(new URL('../../../../crates/g9-domain/src/assessment.rs', import.meta.url));
const RUBRIC_RS = fileURLToPath(new URL('../../../../crates/g9-assessment/src/rubric.rs', import.meta.url));

function rustBaselineCriteria(): { code: string; title: string; weight: number; machine_checkable: boolean; critical_gate: boolean }[] {
  const src = readFileSync(DOMAIN_RS, 'utf8');
  const block = /pub const CRITERIA: \[RubricCriterion; \d+\] = \[([\s\S]*?)\n {4}\];/.exec(src)?.[1];
  if (block === undefined) {
    throw new Error('BaselineRubric::CRITERIA not found in g9-domain/src/assessment.rs');
  }
  const re =
    /RubricCriterion \{\s*code: "([^"]+)",\s*title: "([^"]+)",\s*weight: (\d+),\s*machine_checkable: (true|false),\s*critical_gate: (true|false),\s*\}/g;
  return [...block.matchAll(re)].map((m) => ({
    code: m[1] ?? '',
    title: m[2] ?? '',
    weight: Number(m[3]),
    machine_checkable: m[4] === 'true',
    critical_gate: m[5] === 'true',
  }));
}

describe('baseline rubric parity with the Rust sources', () => {
  it('matches g9_domain::assessment::BaselineRubric::CRITERIA code for code, in order', () => {
    const rust = rustBaselineCriteria();
    expect(rust).toHaveLength(6);
    expect(BASELINE_RUBRIC).toEqual(rust);
    expect(BASELINE_RUBRIC.reduce((a, c) => a + c.weight, 0)).toBe(100);
    expect(BASELINE_RUBRIC.filter((c) => c.critical_gate).map((c) => c.code)).toEqual(['scope_compliance_reporting']);
  });

  it('the mock grades against exactly the same criteria', () => {
    expect(BASELINE_CRITERIA).toEqual(BASELINE_RUBRIC.map((c) => ({ code: c.code, weight: c.weight, critical: c.critical_gate })));
  });

  it('matches PASS_THRESHOLD, the standard level scale and the per-criterion floor', () => {
    const domain = readFileSync(DOMAIN_RS, 'utf8');
    const rubric = readFileSync(RUBRIC_RS, 'utf8');
    expect(Number(/pub const PASS_THRESHOLD: u8 = (\d+);/.exec(domain)?.[1])).toBe(PASS_THRESHOLD);
    expect(Number(/pub const CRITERION_FLOOR_PERCENT: u8 = (\d+);/.exec(rubric)?.[1])).toBe(CRITERION_FLOOR_PERCENT);
    const scale = /pub const STANDARD: LevelScale = LevelScale \{\s*scores: \[([\d,\s]+)\],/.exec(rubric)?.[1];
    expect(scale?.split(',').map((x) => Number(x.trim()))).toEqual([...LEVEL_SCALE]);
  });

  it('accepts only an explicit single level 0–4 as reviewer input', () => {
    expect(['0', '1', '2', '3', '4', ' 3 '].map(parseLevel)).toEqual([0, 1, 2, 3, 4, 3]);
    expect(['', '5', '-1', '2.5', '3e0', 'x', undefined].map(parseLevel)).toEqual([null, null, null, null, null, null, null]);
  });
});
