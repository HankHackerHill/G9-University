/**
 * `session.plan_day` from a screen: one PLANNED session for each block of the program day on a
 * date that has none. `onPlanned` runs after a successful call so the screen can read its plan
 * and sessions again.
 */
import { useCallback, useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { DayPlanned } from '../../ipc/operations';
import type { G9Error } from '../../ipc/types';
import { useIpc } from '../ipc-context';

export interface PlanDay {
  /** Plan the blocks of the day on `date` (`YYYY-MM-DD`). */
  plan: (date: string) => Promise<void>;
  busy: boolean;
  error: G9Error | null;
  result: DayPlanned | null;
  warnings: readonly string[];
}

export function usePlanDay(onPlanned: () => void): PlanDay {
  const client = useIpc();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<G9Error | null>(null);
  const [result, setResult] = useState<DayPlanned | null>(null);
  const [warnings, setWarnings] = useState<readonly string[]>([]);

  const plan = useCallback(
    async (date: string): Promise<void> => {
      setBusy(true);
      setError(null);
      const r = await client.invoke('session.plan_day', { date });
      if (isG9Error(r)) {
        setError(r);
        setResult(null);
        setWarnings([]);
      } else {
        setResult(r.result);
        setWarnings(r.warnings ?? []);
        onPlanned();
      }
      setBusy(false);
    },
    [client, onPlanned],
  );

  return { plan, busy, error, result, warnings };
}
