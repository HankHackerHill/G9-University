import type { ReactNode } from 'react';
import type { ProvenanceLabel } from '../../ipc/types';

export type BadgeTone = 'verified' | 'recommendation' | 'proposed' | 'state' | 'off';

/**
 * A stamp. The tone is the standing of what it marks (components.css): `verified` is inked,
 * `proposed` is hatched, `recommendation` is dashed, `off` is dotted. `state` is a backend
 * state name; when the name is passed as `state` the stylesheet gives it its own standing.
 */
export function Badge({ tone, children, label, state }: { tone: BadgeTone; children: ReactNode; label?: string; state?: string }) {
  return (
    <span className={`badge badge-${tone}`} aria-label={label} data-state={state}>
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
    <Badge tone={tone} label={`Provenance: ${label.toLowerCase()}`}>
      {label}
    </Badge>
  );
}

export function StateBadge({ state }: { state: string }) {
  return (
    <Badge tone="state" label={`State: ${state}`} state={state}>
      {state}
    </Badge>
  );
}
