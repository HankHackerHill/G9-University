/**
 * Test helper: set up the program on a harness the way the operator does (import the curriculum
 * that ships with the application through the approval, then choose the Monday week 1 starts).
 */
import seedJson from '../../../../data/curriculum/g9-ct9-480.seed.v0.json';
import { isG9Error } from '../ipc/client';
import type { JsonValue } from '../ipc/types';
import type { Harness } from './render';

export async function setUpProgram(harness: Harness, monday = '2026-11-02', dayStart?: string): Promise<void> {
  const manifest = seedJson as JsonValue;
  const asked = await harness.client.invoke('curriculum.import', { manifest, provenance: 'PROPOSED_SEED' });
  if (!isG9Error(asked) || asked.approval_id === undefined) {
    throw new Error('curriculum.import: expected APPROVAL_REQUIRED');
  }
  const decided = await harness.client.invoke('approvals.decide', { approval_id: asked.approval_id, decision: 'APPROVE' });
  if (isG9Error(decided)) {
    throw new Error(`approvals.decide failed: ${decided.code}`);
  }
  const imported = await harness.client.invoke('curriculum.import', { manifest, provenance: 'PROPOSED_SEED' }, { approvalId: asked.approval_id });
  if (isG9Error(imported)) {
    throw new Error(`curriculum.import failed: ${imported.code}`);
  }
  const start = await harness.client.invoke('curriculum.set_program_start', { date: monday, ...(dayStart === undefined ? {} : { day_start: dayStart }) });
  if (isG9Error(start)) {
    throw new Error(`curriculum.set_program_start failed: ${start.code}`);
  }
}
