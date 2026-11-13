/**
 * Backend disclosures must be announced when they first appear and must be navigable
 * afterwards (NEW-7). A live region inserted together with its content is not announced, so
 * the live region is always mounted and the content lives in a labelled region landmark.
 */
import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { Warnings } from './Warnings';

const EXPORT = 'export.attachments_missing: 1 attachment file(s) were missing or damaged in the store and are listed in manifest.json';
const READ_ONLY = 'WARNING storage.migration_blocked: pre-migration backup failed; storage is read-only';

describe('Warnings', () => {
  it('keeps an empty polite live region mounted while there is nothing to disclose, and no visible region', () => {
    render(<Warnings warnings={[]} />);
    const live = screen.getByRole('status');
    expect(live).toHaveAttribute('aria-live', 'polite');
    expect(live).toHaveAttribute('aria-atomic', 'true');
    expect(live).toHaveTextContent('');
    expect(screen.queryByRole('region')).toBeNull();
    expect(screen.queryByTestId('outcome-warnings')).toBeNull();
  });

  it('announces the first appearance through the pre-existing live region and lists the texts in a labelled region landmark', () => {
    const { rerender } = render(<Warnings warnings={[]} />);
    const live = screen.getByRole('status');
    rerender(<Warnings warnings={[EXPORT, READ_ONLY, EXPORT]} />);
    // Same element as before the warnings arrived (not re-mounted), now carrying the summary.
    expect(screen.getByRole('status')).toBe(live);
    expect(live).toHaveTextContent('2 backend warnings: export.attachments_missing, storage.migration_blocked');
    const region = screen.getByRole('region', { name: 'Backend warnings' });
    expect(region).toHaveAttribute('data-testid', 'outcome-warnings');
    expect(within(region).getByText(EXPORT)).toBeInTheDocument();
    expect(within(region).getByText(READ_ONLY)).toBeInTheDocument();
    expect(within(region).getAllByRole('listitem')).toHaveLength(2);
    // The verbatim list is not itself a live region: it is content, re-readable by navigation.
    expect(region).not.toHaveAttribute('role', 'status');
    expect(region).not.toHaveAttribute('aria-live');
  });

  it('uses the caller label and test id for both the region and its live region', () => {
    render(<Warnings warnings={[READ_ONLY]} label="System status warnings" testId="status-warnings" />);
    expect(screen.getByRole('region', { name: 'System status warnings' })).toHaveAttribute('data-testid', 'status-warnings');
    expect(screen.getByTestId('status-warnings-live')).toHaveTextContent('1 backend warning: storage.migration_blocked');
  });
});
