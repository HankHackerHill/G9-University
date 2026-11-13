import type { ReactNode } from 'react';
import type { ProvenanceLabel } from '../../ipc/types';
import { stateLabel } from '../../lib/copy';

export type BadgeTone = 'verified' | 'recommendation' | 'proposed' | 'state' | 'off';

/**
 * A stamp: the standing of a thing, as a mark and a word. The tone is the standing
 * (components.css): `verified` is inked, `proposed` is hatched, `recommendation` is dashed,
 * `off` is dotted. `state` is a backend state; when its name is passed as `state` the
 * stylesheet gives it its own standing. `title` carries the backend's exact name.
 */
export function Badge({ tone, children, label, state, title }: { tone: BadgeTone; children: ReactNode; label?: string; state?: string; title?: string }) {
  return (
    <span className={`badge badge-${tone}`} aria-label={label} data-state={state} title={title}>
      {children}
    </span>
  );
}

/**
 * Every piece of text shown to the operator carries its provenance. `VERIFIED` is an
 * operator/system-deterministic record; `RECOMMENDATION` is model or skill output that can
 * only propose; `PROPOSED` is a hypothesis/plan awaiting an operator decision.
 */
export function ProvenanceBadge({ label }: { label: ProvenanceLabel }) {
  const tone: BadgeTone = label === 'VERIFIED' ? 'verified' : label === 'RECOMMENDATION' ? 'recommendation' : 'proposed';
  return (
    <Badge tone={tone} label={`Provenance: ${stateLabel(label).toLowerCase()}`} title={label}>
      {stateLabel(label)}
    </Badge>
  );
}

/** A backend state in the operator's words; the exact name is the `title` and `data-state`. */
export function StateBadge({ state }: { state: string }) {
  return (
    <Badge tone="state" label={`State: ${stateLabel(state)}`} state={state} title={state}>
      {stateLabel(state)}
    </Badge>
  );
}
