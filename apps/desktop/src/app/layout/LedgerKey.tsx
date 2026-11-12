import { useEffect, useRef } from 'react';
import { Badge, StateBadge } from '../components/Badge';
import { Meter } from '../components/Meter';
import { Note } from '../components/Note';
import { openDialog } from './CommandPalette';

/**
 * The key to the interface: what each kind of mark means. Opened with `?` or from the tools.
 * It uses the real components, so the key cannot drift from what the screens draw.
 */
export function LedgerKey({ onClose }: { onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const d = dialog.current;
    if (d === null) {
      return;
    }
    openDialog(d);
    const onBackdrop = (e: MouseEvent) => {
      if (e.target === d) {
        onClose();
      }
    };
    d.addEventListener('mousedown', onBackdrop);
    return () => {
      d.removeEventListener('mousedown', onBackdrop);
    };
  }, [onClose]);

  return (
    <dialog className="key-dialog" ref={dialog} aria-labelledby="ledger-key-title" onClose={onClose}>
      <div className="key-body">
        <h2 id="ledger-key-title">How to read the ledger</h2>
        <p className="muted">
          G9 Mentor keeps books on your training. Every mark on screen says how far a thing can be trusted. Each one also carries its
          name in words, so colour is never the only signal.
        </p>
        <dl className="key-list">
          <dt>
            <Badge tone="verified">VERIFIED</Badge>
          </dt>
          <dd>
            <strong>Posted.</strong> Solid ink. The backend has recorded it: verified records, credited time, approved grades.{' '}
            <Meter posted={3} of={4} label="Example: three quarters posted" />
          </dd>
          <dt>
            <StateBadge state="PLANNED" />
          </dt>
          <dd>
            <strong>Pencilled.</strong> An outline. Planned and not yet started. <Meter posted={0} of={4} label="Example: nothing posted yet" />
          </dd>
          <dt>
            <Badge tone="proposed">PROPOSED</Badge>
          </dt>
          <dd>
            <strong>In doubt.</strong> Amber hatch. Not settled: uncertain time, proposed plans, provisional results, warnings. It waits
            for your decision or a correction. <Meter posted={1} doubted={2} of={4} label="Example: one quarter posted, two quarters in doubt" />
          </dd>
          <dt>
            <Badge tone="recommendation">RECOMMENDATION</Badge>
          </dt>
          <dd>
            <strong>Advice.</strong> Violet, dashed, in a different hand. A model or a skill suggested it. Advice can propose; it cannot
            post.
            <Note label="Example of advice">Written like this, in the margin.</Note>
          </dd>
          <dt>
            <StateBadge state="FAILED" />
          </dt>
          <dd>
            <strong>Owed.</strong> Red ink. Overdue, failed, refused or broken.
          </dd>
          <dt>
            <span className="machine">NO DATA</span>
          </dt>
          <dd>
            <strong>Missing.</strong> An empty line with the reason written on it. A missing value is never shown as zero.
          </dd>
        </dl>
        <h3>Keys</h3>
        <dl className="pairs">
          <dt>
            <kbd>Ctrl</kbd> <kbd>K</kbd>
          </dt>
          <dd>Search screens and commands</dd>
          <dt>
            <kbd>?</kbd>
          </dt>
          <dd>Open this key</dd>
          <dt>
            <kbd>Esc</kbd>
          </dt>
          <dd>Close a dialog</dd>
        </dl>
        <form method="dialog">
          <button type="submit" className="primary">
            Close
          </button>
        </form>
      </div>
    </dialog>
  );
}
