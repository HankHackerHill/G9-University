/**
 * The UI describes the approval control that exists, no more and no less: the page only asks, and
 * the desktop shell's native confirmation grants (Addendum A.7, ADR-0014). A cancelled
 * confirmation is the operator's answer: nothing is approved and the request stays pending.
 *
 * The rest of the screen is the record: what waits, what was decided, and the audit trail with
 * the chain status exactly as the backend reports it (never a claim the backend did not make).
 */
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { isG9Error } from '../../ipc/client';
import type { ResponseOf } from '../../ipc/operations';
import { STORAGE_BUSY, addWarnings, createInterceptHarness } from '../../test/intercept';
import { createHarness, renderWithIpc, type Harness } from '../../test/render';
import { ApprovalsAudit } from './ApprovalsAudit';

async function pendingApproval(harness: Harness) {
  const r = await harness.client.invoke('assessment.approve_final', { attempt_id: 'mock-attempt-prev' });
  expect(isG9Error(r) && r.category === 'APPROVAL_REQUIRED').toBe(true);
}

/** The stat tile with the given label (the label also names a panel and a stamp, so the tile is found by its class). */
function stat(label: string): HTMLElement {
  const tile = screen
    .getAllByText(label)
    .map((el) => el.closest<HTMLElement>('.stat'))
    .find((el) => el !== null);
  if (tile === undefined) {
    throw new Error(`stat tile ${label} not found`);
  }
  return tile;
}

/** A harness whose `audit.recent` answers with the given chain status and extra warnings. */
function auditHarness(chainOk: boolean | null, warnings: readonly string[] = []) {
  return createInterceptHarness(async (op, _payload, next) => {
    const r = await next();
    if (op !== 'audit.recent' || typeof r !== 'object' || r === null || !('result' in r)) {
      return r;
    }
    return addWarnings({ ...r, result: { ...(r.result as ResponseOf<'audit.recent'>), chain_ok: chainOk } }, warnings);
  });
}

const busyAlerts = () => screen.queryAllByRole('alert').filter((a) => a.getAttribute('data-category') === 'RETRYABLE_STORAGE');

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
    // The cancellation is not an error and not a decision: no alert, no receipt, and the voucher still waits.
    expect(screen.queryAllByRole('alert').filter((a) => a.hasAttribute('data-category'))).toHaveLength(0);
    expect(screen.queryByTestId('decision-receipt')).toBeNull();
    expect(await screen.findByTestId('approval-card')).toBeInTheDocument();
    expect(stat('Waiting for you')).toHaveAttribute('data-standing', 'doubt');
  });
});

describe('Approvals/Audit requests', () => {
  it('with nothing waiting says so in a sentence, with a posted zero the backend counted', async () => {
    renderWithIpc(<ApprovalsAudit />);
    expect(await screen.findByText('Nothing waits for your approval.')).toBeInTheDocument();
    expect(stat('Waiting for you')).toHaveAttribute('data-standing', 'posted');
    expect(stat('Waiting for you')).toHaveTextContent('0approvals to decide');
    expect(screen.queryByTestId('approval-card')).toBeNull();
    expect(within(screen.getByRole('region', { name: 'Decided' })).getByText('No decided requests yet')).toBeInTheDocument();
  });

  it('a granted approval leaves the vouchers, is listed under Decided with what it covered, and a receipt says so', async () => {
    const harness = createInterceptHarness((_op, _payload, next) => next());
    await pendingApproval(harness);
    renderWithIpc(<ApprovalsAudit />, harness);
    const card = await screen.findByTestId('approval-card');
    expect(stat('Waiting for you')).toHaveAttribute('data-standing', 'doubt');
    expect(stat('Waiting for you')).toHaveTextContent('1approval to decide');
    const fingerprint = within(card).getByTestId('approval-hash').textContent;
    fireEvent.click(within(card).getByRole('button', { name: 'Approve' }));

    expect(await screen.findByTestId('decision-receipt')).toHaveTextContent('Request approved: Make a grade final. It is listed under Decided.');
    // The page sent the decision for this request and nothing else.
    const sent = harness.calls.filter((c) => c.op === 'approvals.decide').map((c) => c.payload);
    expect(sent).toHaveLength(1);
    expect(sent[0]).toMatchObject({ decision: 'APPROVE' });
    await waitFor(() => {
      expect(screen.queryByTestId('approval-card')).toBeNull();
    });
    expect(stat('Waiting for you')).toHaveAttribute('data-standing', 'posted');
    expect(screen.getByText('Nothing waits for your approval.')).toBeInTheDocument();

    const decided = screen.getByRole('region', { name: 'Decided' });
    expect(within(decided).getByText('Make a grade final')).toBeInTheDocument();
    expect(within(decided).getByLabelText('State: Approved')).toBeInTheDocument();
    expect(within(decided).getByText(/^1 request/)).toHaveTextContent('1 request: 1 approved');
    expect(within(decided).getByText(/^Decided 2026-11-18 09:30, not used yet, expires 2026-11-18 09:40\.$/)).toBeInTheDocument();
    // Exactly what the approval covered stays with the record: the same fingerprint the voucher showed, and the exact operation.
    expect(within(decided).getByText('Show what it covered')).toBeInTheDocument();
    expect(fingerprint).toMatch(/^[0-9a-f]{64}$/);
    expect(within(decided).getByText(fingerprint)).toBeInTheDocument();
    expect(within(decided).getByText('assessment.approve_final')).toBeInTheDocument();
  });

  it('a rejected request is listed under Decided as rejected', async () => {
    const harness = createHarness();
    await pendingApproval(harness);
    renderWithIpc(<ApprovalsAudit />, harness);
    const card = await screen.findByTestId('approval-card');
    fireEvent.click(within(card).getByRole('button', { name: 'Reject' }));
    expect(await screen.findByTestId('decision-receipt')).toHaveTextContent('Request rejected: Make a grade final.');
    const decided = screen.getByRole('region', { name: 'Decided' });
    expect(await within(decided).findByLabelText('State: Rejected')).toBeInTheDocument();
    expect(screen.queryByTestId('confirmation-cancelled')).toBeNull();
  });
});

describe('Approvals/Audit audit trail', () => {
  it('lists the entries with the exact operation, the actor in words, the outcome and the full hash one step away', async () => {
    const harness = createHarness();
    await pendingApproval(harness);
    renderWithIpc(<ApprovalsAudit />, harness);
    const table = await screen.findByRole('table', { name: 'Audit entries, newest first' });
    const seed = within(table).getByText('mock.seed').closest('tr');
    const refused = within(table).getByText('assessment.approve_final').closest('tr');
    if (seed === null || refused === null) {
      throw new Error('audit rows not found');
    }
    expect(within(seed).getByTitle('SYSTEM system')).toHaveTextContent('The application');
    expect(within(seed).getByLabelText('Outcome: done')).toHaveTextContent('Done');
    expect(within(seed).getByText('ALLOW')).toBeInTheDocument();
    expect(within(seed).getByTitle(/^[0-9a-f]{64}$/)).toHaveTextContent(/^[0-9a-f]{12}…$/);
    // The refused request is in the record too, with the backend's decision.
    expect(within(refused).getByTitle('OPERATOR operator')).toHaveTextContent('You');
    expect(within(refused).getByLabelText('Outcome: not done')).toHaveTextContent('Not done');
    expect(within(refused).getByText('APPROVAL_REQUIRED')).toBeInTheDocument();
    expect(stat('Audit entries shown')).toHaveTextContent('2The most recent entries, 50 at most.');
  });

  it('an intact chain is stamped Verified; a chain the backend did not verify or found broken is never called intact', async () => {
    const intact = renderWithIpc(<ApprovalsAudit />, auditHarness(true));
    await waitFor(() => {
      expect(stat('Audit chain')).toHaveTextContent('Intact');
    });
    expect(stat('Audit chain')).toHaveAttribute('data-standing', 'posted');
    expect(within(screen.getByRole('region', { name: 'Audit trail' })).getByLabelText('Provenance: verified')).toBeInTheDocument();
    intact.unmount();

    const unverified = renderWithIpc(<ApprovalsAudit />, auditHarness(null));
    await waitFor(() => {
      expect(stat('Audit chain')).toHaveTextContent('Not verified');
    });
    expect(stat('Audit chain')).toHaveAttribute('data-standing', 'missing');
    expect(stat('Audit chain')).toHaveTextContent('The backend returned no result for the check of the chain.');
    expect(screen.queryByLabelText('Provenance: verified')).toBeNull();
    expect(screen.queryByText('Intact')).toBeNull();
    unverified.unmount();

    renderWithIpc(<ApprovalsAudit />, auditHarness(false));
    await waitFor(() => {
      expect(stat('Audit chain')).toHaveTextContent('Broken');
    });
    expect(stat('Audit chain')).toHaveAttribute('data-standing', 'owed');
    expect(screen.getByTestId('chain-broken')).toHaveTextContent('verify a backup in Settings/Recovery');
    expect(screen.getByTestId('chain-broken')).toHaveAttribute('role', 'alert');
    expect(screen.queryByLabelText('Provenance: verified')).toBeNull();
    expect(screen.queryByText('Intact')).toBeNull();
  });

  it('says the list covers this run only when the backend discloses it, and shows the disclosure', async () => {
    const SCOPED = 'audit.recent.process_scoped: rows appended before this process started are verified (chain_ok) but not listed';
    const scoped = renderWithIpc(<ApprovalsAudit />, auditHarness(true, [SCOPED]));
    await waitFor(() => {
      expect(stat('Audit entries shown')).toHaveTextContent('Since the application started.');
    });
    expect(within(screen.getByTestId('outcome-warnings')).getByText(SCOPED)).toBeInTheDocument();
    // The scope disclosure says the chain was checked: the listed entries keep their stamp.
    expect(screen.getByLabelText('Provenance: verified')).toBeInTheDocument();
    scoped.unmount();

    const OTHER = 'audit.recent.something_else: a disclosure this window does not know';
    renderWithIpc(<ApprovalsAudit />, auditHarness(true, [OTHER]));
    expect(await within(await screen.findByTestId('outcome-warnings')).findByText(OTHER)).toBeInTheDocument();
    expect(stat('Audit entries shown')).toHaveTextContent('The most recent entries, 50 at most.');
    // An unknown disclosure about the read: nothing is claimed beyond what the backend said.
    expect(screen.queryByLabelText('Provenance: verified')).toBeNull();
  });

  it('failed reads show one error each and "No data" with the code, never a zero, "nothing waits" or an empty table', async () => {
    const harness = createInterceptHarness(async (op, _payload, next) => (op === 'approvals.list' || op === 'audit.recent' ? STORAGE_BUSY : next()));
    renderWithIpc(<ApprovalsAudit />, harness);
    await waitFor(() => {
      expect(busyAlerts()).toHaveLength(2);
    });
    for (const label of ['Waiting for you', 'Audit chain', 'Audit entries shown']) {
      expect(stat(label)).toHaveAttribute('data-standing', 'missing');
      expect(stat(label)).toHaveTextContent('No data');
      expect(stat(label)).toHaveTextContent('storage.busy');
    }
    expect(screen.queryByText('Nothing waits for your approval.')).toBeNull();
    expect(screen.queryByText('No decided requests yet')).toBeNull();
    expect(screen.queryByRole('table')).toBeNull();
    expect(screen.queryByLabelText('Provenance: verified')).toBeNull();
    expect(screen.getByText(/Approval requests could not be read \(storage\.busy\), so nothing can be decided here/)).toBeInTheDocument();
    expect(screen.getByText(/The audit trail could not be read \(storage\.busy\), so no entry can be shown/)).toBeInTheDocument();
  });
});
