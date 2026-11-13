import { useEffect, useRef, type ReactNode } from 'react';

/**
 * The form a page head's primary action opens, in place, above the list it adds to. It is a
 * labelled region, takes focus on its first control when it opens, and closes on Escape.
 */
export function Composer({ title, onClose, children }: { title: string; onClose: () => void; children: ReactNode }) {
  const ref = useRef<HTMLElement>(null);

  // Put the caret in the first field once, when the composer opens.
  useEffect(() => {
    ref.current?.querySelector<HTMLElement>('input, select, textarea')?.focus();
  }, []);

  // Escape closes the composer from any control inside it, and goes no further (an open
  // overlay behind it must not close too).
  useEffect(() => {
    const region = ref.current;
    if (region === null) {
      return undefined;
    }
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
      }
    };
    region.addEventListener('keydown', onKey);
    return () => {
      region.removeEventListener('keydown', onKey);
    };
  }, [onClose]);

  return (
    <section className="composer" aria-label={title} ref={ref}>
      <div className="panel-head">
        <h2>{title}</h2>
        <button type="button" className="quiet sm" onClick={onClose}>
          Close
        </button>
      </div>
      {children}
    </section>
  );
}
