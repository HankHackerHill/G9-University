import { useEffect, useRef } from 'react';
import { ProvenanceBadge, StateBadge } from '../components/Badge';
import { Meter } from '../components/Meter';
import { Note } from '../components/Note';
import { NoData } from '../components/Panel';
import { openDialog } from './CommandPalette';
import '../theme/screens/overlays.css';

/**
 * The key to the interface: what each kind of mark means. Opened with `?` or from the button
 * beside the search. It is drawn with the real components and classes (stamps, swatches, a
 * meter, a note, a "No data" tile, a selected line), so the key cannot drift from what the
 * screens draw. Its only control is the Close button, which therefore takes the focus.
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
        <p className="muted">Every mark says where a thing stands. Its name is written beside it, so colour is never the only signal.</p>
        <div className="key-cols">
          <div>
            <h3>Five standings</h3>
            <dl className="key-list">
              <dt>
                <span className="swatch" data-mark="posted" aria-hidden="true" />
                <ProvenanceBadge label="VERIFIED" />
              </dt>
              <dd>
                <strong>Posted.</strong> Solid ink. On the record: verified records, credited time, final grades.
              </dd>
              <dt>
                <span className="swatch" data-mark="doubt" aria-hidden="true" />
                <ProvenanceBadge label="PROPOSED" />
              </dt>
              <dd>
                <strong>In doubt.</strong> Hatch. Not settled: time that could not be vouched for, proposals, grades that are not final, warnings. It waits for your decision or
                your correction.
              </dd>
              <dt>
                <span className="swatch" data-mark="pencil" aria-hidden="true" />
                <StateBadge state="PLANNED" />
              </dt>
              <dd>
                <strong>Planned.</strong> An outline. Planned and not started.
              </dd>
              <dt>
                <span className="swatch" data-mark="owed" aria-hidden="true" />
                <StateBadge state="FAILED" />
              </dt>
              <dd>
                <strong>Owed.</strong> Red ink. Overdue, failed or refused.
              </dd>
              <dt>
                <span className="key-no-swatch" aria-hidden="true" />
                <ProvenanceBadge label="RECOMMENDATION" />
              </dt>
              <dd>
                <strong>Advice.</strong> Dashed, in the mentor’s hand. A model or a skill suggested it. Advice can propose; it cannot post.
                <Note label="Example of advice">Advice is written like this.</Note>
              </dd>
              <dt>
                <Meter posted={2} doubted={1} of={4} label="Example: half posted, a quarter in doubt, a quarter still planned" />
              </dt>
              <dd>
                <strong>A bar</strong> reads the same way: the solid part is posted, the hatch is in doubt, the outline is still planned.
              </dd>
            </dl>
            <NoData reason="A value that is missing, or that could not be read, is said in words with the reason. It is never drawn as zero." />
          </div>
          <div>
            <h3>The pen in hand</h3>
            <ul className="rows" aria-label="Example of a selected line">
              <li data-selected="true">
                <div className="row-main">
                  <span className="row-title">The selected line</span>
                </div>
              </li>
            </ul>
            <p className="muted">One colour marks where you are: the selected line, the focused control, today, the current step. It is never a status.</p>
            <h3>Three voices</h3>
            <dl className="key-list">
              <dt>Record</dt>
              <dd>Everything you read and press.</dd>
              <dt>
                <span className="machine">Machine</span>
              </dt>
              <dd>
                Measured values and identifiers only: <span className="num">0:40:00</span>, <code>D15-O1</code>.
              </dd>
              <dt>
                <span className="mentor">Mentor</span>
              </dt>
              <dd>Advice only. A record is never written in it.</dd>
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
              <dd>Close an open form, the search or this key</dd>
            </dl>
          </div>
        </div>
        <form method="dialog">
          <button type="submit" className="primary">
            Close
          </button>
        </form>
      </div>
    </dialog>
  );
}
