import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { ERROR_CATEGORIES } from '../../ipc/types';
import { ERROR_PRESENTATION } from '../../lib/error-text';
import { ErrorDisplay } from './ErrorDisplay';

describe('ErrorDisplay', () => {
  it.each(ERROR_CATEGORIES)('renders operator text for %s', (category) => {
    const { unmount } = render(<ErrorDisplay error={{ category, code: 'test.code', message: 'test message' }} />);
    const alert = screen.getByRole('alert');
    expect(alert).toHaveAttribute('data-category', category);
    expect(alert).toHaveTextContent(ERROR_PRESENTATION[category].title);
    expect(alert).toHaveTextContent(ERROR_PRESENTATION[category].guidance);
    expect(alert).toHaveTextContent('test.code');
    unmount();
  });

  it('shows the approval id and bound hash for APPROVAL_REQUIRED', () => {
    render(
      <ErrorDisplay
        error={{
          category: 'APPROVAL_REQUIRED',
          code: 'policy.approval_required',
          message: 'needs approval',
          approval_id: 'ap-77',
          details: { parameters_sha256: 'a'.repeat(64) },
        }}
      />,
    );
    expect(screen.getByRole('alert')).toHaveTextContent('ap-77');
    expect(screen.getByRole('alert')).toHaveTextContent('a'.repeat(64));
  });
});
