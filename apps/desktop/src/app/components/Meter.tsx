import type { CSSProperties } from 'react';

function ratio(part: number, whole: number): number {
  if (!Number.isFinite(part) || !Number.isFinite(whole) || whole <= 0) {
    return 0;
  }
  return Math.min(1, Math.max(0, part / whole));
}

/**
 * Planned against posted against in doubt, drawn with the fill grammar: solid ink is posted
 * (credited, verified), amber hatch is in doubt, the empty outline is what remains planned.
 * The values come from the backend; the label says them in words, so the picture is never the
 * only carrier. `owed` inks the bar red (a deficit, an overdue queue).
 */
export function Meter({ posted, doubted = 0, of, label, owed = false }: { posted: number; doubted?: number; of: number; label: string; owed?: boolean }) {
  const p = ratio(posted, of);
  const d = Math.min(ratio(doubted, of), 1 - p);
  const style = { '--posted': p, '--doubted': d } as CSSProperties;
  return <span className="meter" role="img" aria-label={label} data-owed={owed} style={style} />;
}
