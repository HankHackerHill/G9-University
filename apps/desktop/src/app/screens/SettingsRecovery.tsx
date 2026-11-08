import { useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { BackupView } from '../../ipc/operations';
import type { G9Error } from '../../ipc/types';
import { formatDateTime } from '../../lib/format';
import { BACKUP_MACHINE, can } from '../../lib/state-machines';
import { warningsOf } from '../../lib/warnings';
import { ApprovalCard } from '../components/ApprovalCard';
import { Badge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Loading, NoData, Panel } from '../components/Panel';
import { Table } from '../components/Table';
import { Warnings } from '../components/Warnings';
import { useApprovalFlow } from '../hooks/useApprovalFlow';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';
import { useContrast } from '../theme/useContrast';

export function SettingsRecovery() {
  const today = useToday();
  const client = useIpc();
  const [contrast, toggleContrast] = useContrast();
  const [backups, setBackups] = useState<BackupView[]>([]);
  const [error, setError] = useState<G9Error | null>(null);
  const [busy, setBusy] = useState(false);
  const [backupWarnings, setBackupWarnings] = useState<readonly string[]>([]);
  const exportFlow = useApprovalFlow<'export.request'>();

  const upsert = (b: BackupView) => {
    setBackups((prev) => [b, ...prev.filter((x) => x.backup_id !== b.backup_id)]);
  };

  const create = async () => {
    setBusy(true);
    const r = await client.invoke('backup.create', { label: 'manual' });
    setBackupWarnings(warningsOf(r));
    if (isG9Error(r)) {
      setError(r);
    } else {
      setError(null);
      upsert(r.result);
    }
    setBusy(false);
    today.reload();
  };

  const step = async (op: 'backup.verify' | 'backup.restore_test', b: BackupView) => {
    setBusy(true);
    const r = await client.invoke(op, { backup_id: b.backup_id }, { expectedRevision: b.revision });
    setBackupWarnings(warningsOf(r));
    if (isG9Error(r)) {
      setError(r);
    } else {
      setError(null);
      upsert(r.result);
    }
    setBusy(false);
    today.reload();
  };

  const requestExport = async () => {
    await exportFlow.run((approvalId) =>
      client.invoke('export.request', { domains: ['LEARNING'], format: 'JSON', include_private: false }, approvalId === undefined ? {} : { approvalId }),
    );
  };

  const s = today.status;
  // `today.status` is null both while loading and after a failed read (useToday); only the
  // first is "loading". A failed read is reported as unavailable, never as still loading.
  const statusPending = (what: string) =>
    today.error === null ? <Loading what={what} /> : <NoData reason={`${what} unavailable: system.status failed (${today.error.code})`} />;

  return (
    <div>
      <h1>Settings / Recovery</h1>
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      {error !== null ? <ErrorDisplay error={error} /> : null}
      <Panel title="Host and storage warnings">
        {s === null ? (
          statusPending('status warnings')
        ) : today.warnings.length === 0 ? (
          <p className="muted" data-testid="status-warnings-none">
            system.status reported no warnings.
          </p>
        ) : (
          <Warnings warnings={today.warnings} label="Reported by system.status (binding-key storage, time zone, skills registry, storage state)" testId="settings-status-warnings" />
        )}
      </Panel>
      <div className="grid-2">
        <Panel title="Model runtime">
          {s === null ? (
            statusPending('runtime status')
          ) : s.model.available ? (
            <p>
              <Badge tone="verified">AVAILABLE</Badge> {s.model.model_id ?? ''}
            </p>
          ) : (
            <p data-testid="runtime-none">
              <Badge tone="off">NONE CONFIGURED</Badge> <span className="muted">reason {s.model.reason ?? 'UNKNOWN'}</span>
              <br />
              <span className="muted">
                No model is pre-selected; loading one requires MODEL_LOAD (operator-only) and a checksum match against the registry (ARCHITECTURE §10).
              </span>
            </p>
          )}
        </Panel>
        <Panel title="Appearance">
          <button type="button" onClick={toggleContrast} aria-pressed={contrast === 'high'}>
            High contrast: {contrast === 'high' ? 'on' : 'off'}
          </button>
          <p className="muted">Persisted in localStorage (`g9.contrast`). Both themes target WCAG 2.2 AA.</p>
        </Panel>
      </div>
      <Panel title="Backups">
        <p className="muted">
          Backup machine: REQUESTED → SNAPSHOT_CREATED → CHECKED → RESTORE_TESTED → RETAINED (→ EXPIRED); FAILED from any non-terminal state. A backup counts only after a
          restore test into a disposable copy (M17).
        </p>
        <div className="row">
          <button
            type="button"
            className="primary"
            disabled={busy}
            onClick={() => {
              void create();
            }}
          >
            Create backup
          </button>
          {s?.last_backup !== null && s?.last_backup !== undefined ? (
            <span className="muted">
              Last: <code>{s.last_backup.backup_id}</code> <StateBadge state={s.last_backup.state} /> {formatDateTime(s.last_backup.created_at, today.zone)}
            </span>
          ) : (
            <span className="muted">No backup recorded</span>
          )}
        </div>
        <Warnings warnings={backupWarnings} />
        {backups.length === 0 ? (
          <NoData reason="no backups created in this session (backup.list is PROPOSED)" />
        ) : (
          <Table
            caption="Backups in this session"
            headers={['Backup', 'State', 'Integrity', 'Created', 'Restore tested', 'Manifest', 'Actions']}
            rows={backups.map((b) => ({
              key: b.backup_id,
              cells: [
                <code>{b.backup_id}</code>,
                <StateBadge state={b.state} />,
                b.integrity,
                formatDateTime(b.created_at, today.zone),
                b.restore_tested_at === null ? '—' : formatDateTime(b.restore_tested_at, today.zone),
                <span className="hash">{b.manifest_sha256 === null ? '—' : `${b.manifest_sha256.slice(0, 16)}…`}</span>,
                <span className="row">
                  <button
                    type="button"
                    disabled={busy || !can(BACKUP_MACHINE, b.state, 'CHECK')}
                    onClick={() => {
                      void step('backup.verify', b);
                    }}
                  >
                    Verify
                  </button>
                  <button
                    type="button"
                    disabled={busy || !can(BACKUP_MACHINE, b.state, 'TEST_RESTORE')}
                    onClick={() => {
                      void step('backup.restore_test', b);
                    }}
                  >
                    Restore test
                  </button>
                </span>,
              ],
            }))}
          />
        )}
      </Panel>
      <Panel title="Export">
        <p className="muted">Exports need a DATA_EXPORT approval bound to the exact domains and format; private content is excluded unless approved.</p>
        <button
          type="button"
          disabled={exportFlow.pending}
          onClick={() => {
            void requestExport();
          }}
        >
          Request LEARNING export (JSON)
        </button>
        {exportFlow.error !== null ? <ErrorDisplay error={exportFlow.error} /> : null}
        {exportFlow.approval !== null ? (
          <ApprovalCard
            approval={exportFlow.approval}
            zone={today.zone}
            now={today.now ?? undefined}
            disabled={exportFlow.pending}
            onDecide={(d) => {
              void exportFlow.decide(d);
            }}
            onApply={() => {
              void exportFlow.applyWithApproval();
            }}
          />
        ) : null}
        {exportFlow.result !== null && !isG9Error(exportFlow.result) ? (
          <p role="status">
            Export <code>{exportFlow.result.result.export_id}</code> <StateBadge state={exportFlow.result.result.state} />
          </p>
        ) : null}
        <Warnings warnings={exportFlow.warnings} />
      </Panel>
      <Panel title="Transport">
        {s === null ? statusPending('transport status') : <p>Active transport: <StateBadge state={s.transport.toUpperCase()} /> {s.transport === 'mock' ? <span className="muted">— fixture data, no storage.</span> : null}</p>}
      </Panel>
    </div>
  );
}
