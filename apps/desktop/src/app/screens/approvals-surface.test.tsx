/**
 * The UI must not claim a security control that does not exist: the native Rust confirmation
 * (Addendum A.7, ADR-0014) is PROPOSED and SEC-03 is OPEN, so approvals are granted from the webview.
 */
import { screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isG9Error } from '../../ipc/client';
import { createHarness, renderWithIpc } from '../../test/render';
import { ApprovalsAudit } from './ApprovalsAudit';

describe('Approvals/Audit confirmation-surface disclosure', () => {
  it('states that the native confirmation is not implemented and never that a trusted surface grants approvals', async () => {
    const harness = createHarness();
    const r = await harness.client.invoke('assessment.approve_final', { attempt_id: 'mock-attempt-prev' });
    expect(isG9Error(r) && r.category === 'APPROVAL_REQUIRED').toBe(true);
    renderWithIpc(<ApprovalsAudit />, harness);
    const note = await screen.findByTestId('confirmation-surface-gap');
    expect(note).toHaveTextContent('NOT IMPLEMENTED');
    expect(note).toHaveTextContent('PROPOSED');
    expect(note).toHaveTextContent('SEC-03');
    expect(document.body).not.toHaveTextContent(/trusted confirmation surface grants/i);
    const card = await screen.findByTestId('approval-card');
    expect(within(card).getByTestId('approval-webview-grant')).toHaveTextContent(/PROPOSED, not implemented \(SEC-03 OPEN\).*directly\s+from the webview/);
  });
});
