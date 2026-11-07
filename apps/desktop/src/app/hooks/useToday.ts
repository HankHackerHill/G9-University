/**
 * The operator's "today" comes from the backend clock (`system.status.clock`), never from the
 * webview's wall clock, so every screen agrees with the backend and tests stay deterministic.
 */
import type { SystemStatus } from '../../ipc/operations';
import type { G9Error, IanaZone, UtcMillis } from '../../ipc/types';
import { civilDateOf, formatIsoDate, type CivilDate } from '../../lib/civil-time';
import { useCommand } from './useCommand';

export interface Today {
  status: SystemStatus | null;
  error: G9Error | null;
  loading: boolean;
  now: UtcMillis | null;
  zone: IanaZone;
  date: CivilDate | null;
  isoDate: string | null;
  /** `system.status` warnings: host facts appended by the bridge (key-file fallback, zone fallback, read-only storage...). */
  warnings: readonly string[];
  reload: () => void;
}

export function useToday(): Today {
  const status = useCommand('system.status', {});
  const s = status.data;
  const zone = s?.clock.zone ?? 'UTC';
  const date = s === null ? null : civilDateOf(s.clock.now, zone);
  return {
    status: s,
    error: status.error,
    loading: status.loading,
    now: s?.clock.now ?? null,
    zone,
    date,
    isoDate: date === null ? null : formatIsoDate(date),
    warnings: status.warnings,
    reload: status.reload,
  };
}
