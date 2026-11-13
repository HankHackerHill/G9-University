/**
 * The UI describes the approval control that exists, no more and no less: the page only asks, and
 * the desktop shell's native confirmation grants (Addendum A.7, ADR-0014). A cancelled
 * confirmation is the operator's answer: nothing is approved and the request stays pending.
 */
import { fireEvent, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isG9Error } from '../../ipc/client';
import { createHarness, renderWithIpc } from '../../test/render';
import { ApprovalsAudit } from './ApprovalsAudit';

async function pendingApproval(harness: ReturnType<typeof createHarness>) {
  const r = await harness.client.invoke('assessment.approve_final', { attempt_id: 'mock-attempt-prev' });
  expect(isG9Error(r) && r.category === 'APPROVAL_REQUIRED').toBe(true);
}

describe('Approvals/Audit native confirmation', () => {
  it('says the grant happens in the application window, not on this page', async () => {
    const harness = createHarness();
    await pendingApproval(harness);
    renderWithIpc(<ApprovalsAudit />, harness);
    const note = await screen.findByTestId('native-confirmation-note');
    expect(note).toHaveTextContent(/confirmation window drawn by the application itself/);
    expect(note).toHaveTextContent(/cannot grant one/);
    expect(document.body).not.toHaveTextContent(/NOT IMPLEMENTED/);
    const card = await screen.findByTestId('approval-card');
    expect(within(card).getByTestId('approval-native-confirmation')).toHaveTextContent(/granted only when you confirm there/);
  });

  it('a cancelled confirmation approves nothing and keeps the request pending', async () => {
    const harness = createHarness({ confirmApproval: () => false });
    await pendingApproval(harness);
    renderWithIpc(<ApprovalsAudit />, harness);
    const card = await screen.findByTestId('approval-card');
    fireEvent.click(within(card).getByRole('button', { name: /approve/i }));
    expect(await screen.findByTestId('confirmation-cancelled')).toHaveTextContent('Nothing was approved');
    const listed = await harness.client.invoke('approvals.list', { state: 'PENDING' });
    expect(isG9Error(listed)).toBe(false);
    if (!isG9Error(listed)) {
      expect(listed.result.approvals.length).toBe(1);
    }
  });
});
