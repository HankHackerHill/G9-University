import { useId, useRef, useState } from 'react';
import { isG9Error } from '../../ipc/client';
import type { BackupView, ExportFormat, IntegrityResult } from '../../ipc/operations';
import { DATA_DOMAINS, type DataDomain, type G9Error } from '../../ipc/types';
import { domainLabel } from '../../lib/copy';
import { formatDateTime, sentenceCase, shortHash } from '../../lib/format';
import { BACKUP_MACHINE, can } from '../../lib/state-machines';
import { mergeWarnings, warningsOf } from '../../lib/warnings';
import { ApprovalCard, ConfirmationCancelled } from '../components/ApprovalCard';
import { Badge, StateBadge } from '../components/Badge';
import { ErrorDisplay } from '../components/ErrorDisplay';
import { Margin, MarginNote } from '../components/Margin';
import { Loading, NoData, Panel } from '../components/Panel';
import { PageHead } from '../components/PageHead';
import { ProgramSetup } from '../components/ProgramSetup';
import { Table } from '../components/Table';
import { ThemeGallery } from '../components/ThemeGallery';
import { Warnings } from '../components/Warnings';
import { useApprovalFlow } from '../hooks/useApprovalFlow';
import { useCommand } from '../hooks/useCommand';
import { useToday } from '../hooks/useToday';
import { useIpc } from '../ipc-context';
import { SKINS, fixedScheme, skinById } from '../theme/skins';
import { useContrast } from '../theme/useContrast';
import { useSkin } from '../theme/useSkin';
import { THEME_MODES, useTheme, type ThemeMode } from '../theme/useTheme';

type BackupStep = 'backup.create' | 'backup.verify' | 'backup.restore_test';

/** The sections of the page, in the order they protect the operator's records. */
const SECTIONS = ['Backups', 'Export your data', 'Program', 'Model', 'Appearance', 'About this window'] as const;
const EXPORT_FORMATS: readonly ExportFormat[] = ['JSON', 'CSV'];
const SCHEME_WORDS: Readonly<Record<ThemeMode, string>> = { system: 'System', light: 'Light', dark: 'Dark' };
const INTEGRITY_WORDS: Readonly<Record<IntegrityResult, string>> = { UNKNOWN: 'Not checked', OK: 'Passed', FAILED: 'Failed' };
/** What a backup step posted, in the words of its button. */
const STEP_POSTED: Readonly<Record<BackupStep, string>> = {
  'backup.create': 'Backup created',
  'backup.verify': 'Backup verified',
  'backup.restore_test': 'Restore tested',
};

/** The nearest ancestor whose content scrolls (in the application, the shell's main region). */
function scrollParent(el: HTMLElement): HTMLElement | null {
  for (let parent = el.parentElement; parent !== null; parent = parent.parentElement) {
    const overflowY = getComputedStyle(parent).overflowY;
    if (overflowY === 'auto' || overflowY === 'scroll') {
      return parent;
    }
  }
  return null;
}

/** The scheme a Ledger preview is drawn in: the setting, or the desktop's when it follows the system. */
function previewScheme(mode: ThemeMode): 'light' | 'dark' {
  if (mode !== 'system') {
    return mode;
  }
  return typeof window !== 'undefined' && typeof window.matchMedia === 'function' && window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

/**
 * Settings/Recovery, ordered by what protects the operator's records: backups and their restore
 * tests, exports, then the model, the look of the window and the facts about the host. Every
 * fact is `system.status`'s; when that read fails each section that depends on it says so.
 */
export function SettingsRecovery() {
  const today = useToday();
  const client = useIpc();
  const [contrast, toggleContrast] = useContrast();
  const [skin, setSkin] = useSkin();
  const [scheme, setScheme] = useTheme();
  // The backups recorded in the database (`backup.list`), with the ones this window just made or stepped
  // overlaid: the answer of a step is newer than the list read before it.
  const stored = useCommand('backup.list', {});
  const program = useCommand('curriculum.program', {});
  const [fresh, setBackups] = useState<BackupView[]>([]);
  const backups = [...fresh, ...(stored.data?.backups ?? []).filter((b) => !fresh.some((x) => x.backup_id === b.backup_id))].sort(
    (a, b) => b.created_at - a.created_at || (a.backup_id < b.backup_id ? 1 : -1),
  );
  const [error, setError] = useState<G9Error | null>(null);
  const [busy, setBusy] = useState(false);
  const [backupWarnings, setBackupWarnings] = useState<readonly string[]>([]);
  // The backup as the last step returned it, for the receipt.
  const [posted, setPosted] = useState<{ step: BackupStep; backup: BackupView } | null>(null);
  const exportFlow = useApprovalFlow<'export.request'>();
  // The export form. The defaults are the narrowest request: learning records, no private ones.
  const [areas, setAreas] = useState<readonly DataDomain[]>(['LEARNING']);
  const [format, setFormat] = useState<ExportFormat>('JSON');
  const [includePrivate, setIncludePrivate] = useState(false);
  const mainRef = useRef<HTMLDivElement>(null);
  const base = useId();

  const upsert = (b: BackupView) => {
    setBackups((prev) => [b, ...prev.filter((x) => x.backup_id !== b.backup_id)]);
  };

  const create = async () => {
    setBusy(true);
    const r = await client.invoke('backup.create', { label: 'manual' });
    setBackupWarnings(warningsOf(r));
    if (isG9Error(r)) {
      setError(r);
      setPosted(null);
    } else {
      setError(null);
      upsert(r.result);
      setPosted({ step: 'backup.create', backup: r.result });
    }
    setBusy(false);
    today.reload();
    stored.reload();
  };

  const step = async (op: 'backup.verify' | 'backup.restore_test', b: BackupView) => {
    setBusy(true);
    const r = await client.invoke(op, { backup_id: b.backup_id }, { expectedRevision: b.revision });
    setBackupWarnings(warningsOf(r));
    if (isG9Error(r)) {
      setError(r);
      setPosted(null);
    } else {
      setError(null);
      upsert(r.result);
      setPosted({ step: op, backup: r.result });
    }
    setBusy(false);
    today.reload();
    stored.reload();
  };

  const exported = exportFlow.result !== null && !isG9Error(exportFlow.result) ? exportFlow.result.result : null;
  // Only a finished export is posted as a receipt; any other state is said by its stamp.
  const exportWritten = exported?.state === 'READY';
  // The areas are sent in the backend's own order, whatever order they were ticked in.
  const chosenAreas = DATA_DOMAINS.filter((d) => areas.includes(d));
  const canRequest = !exportFlow.pending && chosenAreas.length > 0;

  const requestExport = async () => {
    // The approval is bound to exactly these values: the same closure is sent again with the approval.
    const payload = { domains: chosenAreas, format, include_private: includePrivate };
    await exportFlow.run((approvalId) => client.invoke('export.request', payload, approvalId === undefined ? {} : { approvalId }));
  };

  /**
   * "On this page": the route lives in the URL hash, so a section is reached by scrolling to it
   * and moving focus to its heading, never by an anchor. Only the page's own scroll container is
   * moved: `scrollIntoView` also shifts the shell behind it (regions that clip with
   * `overflow: hidden` stay scrollable by script) when the section is the last one on the page.
   */
  const jumpTo = (title: string) => {
    const headings = mainRef.current?.querySelectorAll<HTMLElement>('.panel > .panel-head > h2') ?? [];
    const heading = Array.from(headings).find((h) => h.textContent === title);
    if (heading === undefined) {
      return;
    }
    const section = heading.closest<HTMLElement>('.panel') ?? heading;
    const scroller = scrollParent(section);
    if (scroller !== null) {
      scroller.scrollTop += section.getBoundingClientRect().top - scroller.getBoundingClientRect().top;
    }
    heading.tabIndex = -1;
    heading.focus({ preventScroll: true });
  };

  const s = today.status;
  // `today.status` is null both while loading and after a failed read (useToday); only the
  // first is "loading". A failed read is reported with its code, never as still loading.
  const statusPending = (what: string, subject: string) =>
    today.error === null ? <Loading what={what} /> : <NoData reason={`${subject} could not be read (${today.error.code}).`} />;
  const storageAlert = s !== null && (s.storage.state === 'READ_ONLY' || s.storage.state === 'UNAVAILABLE') ? s.storage.state : null;
  const fixed = fixedScheme(skinById(skin));
  const privateHint = `${base}-private`;

  return (
    <div className="stack">
      <PageHead title="Settings / Recovery" lede="Back up your records and test that a backup restores, export your data, and set how this window looks." />
      {today.error !== null ? <ErrorDisplay error={today.error} onRetry={today.reload} /> : null}
      {storageAlert !== null ? (
        <div className="banner" role="alert" data-testid="storage-alert">
          <strong>{storageAlert === 'READ_ONLY' ? 'Storage is read-only.' : 'Storage is unavailable.'}</strong>{' '}
          {storageAlert === 'READ_ONLY' ? 'Changes are refused until it is writable again.' : 'Records cannot be read or saved until it is back.'}
          {today.warnings.length > 0 ? ' The warnings below say what the backend found.' : ''}
        </div>
      ) : null}
      <Warnings warnings={today.warnings} label="Host and storage warnings" testId="settings-status-warnings" />
      <Warnings warnings={mergeWarnings(stored.warnings, program.warnings)} label="Read warnings" testId="settings-read-warnings" />

      <div className="with-margin">
        <div className="stack" ref={mainRef}>
          <Panel
            title="Backups"
            note="A backup is a snapshot of the database with a copy of the attached files. It counts only after you verify it and test a restore into a disposable copy."
          >
            <div className="stack-sm">
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
                {s === null ? (
                  today.error === null ? (
                    <Loading what="the last backup" />
                  ) : (
                    <span className="muted small">The last tested backup could not be read ({today.error.code}).</span>
                  )
                ) : s.last_backup === null ? (
                  <span className="muted small">No backup has been verified and restore-tested yet.</span>
                ) : (
                  <span className="muted small" data-testid="last-backup">
                    Last verified and restore-tested backup: <code>{s.last_backup.backup_id}</code> <StateBadge state={s.last_backup.state} /> {formatDateTime(s.last_backup.created_at, today.zone)}
                  </span>
                )}
              </div>
              {error !== null ? <ErrorDisplay error={error} /> : null}
              {posted === null ? null : posted.backup.state === 'FAILED' ? (
                <div className="banner" role="alert" data-testid="backup-failed">
                  <strong>The backup failed at this step.</strong> <code>{posted.backup.backup_id}</code> stays in the list as failed and cannot be used. Create a new backup.
                </div>
              ) : (
                <p className="receipt" role="status" data-testid="backup-receipt">
                  {STEP_POSTED[posted.step]}: <code>{posted.backup.backup_id}</code>, <StateBadge state={posted.backup.state} />.
                </p>
              )}
              <Warnings warnings={backupWarnings} label="Backup warnings" />
              {stored.error !== null ? <ErrorDisplay error={stored.error} onRetry={stored.reload} /> : null}
              {backups.length === 0 ? (
                stored.data === null && stored.error === null ? (
                  <Loading what="the backups" />
                ) : stored.error !== null ? (
                  <NoData reason={`The stored backups could not be read (${stored.error.code}).`} />
                ) : (
                  <NoData title="No backups yet" reason="No backup is recorded. Create one before you change anything you cannot redo." />
                )
              ) : (
                <>
                  <Table
                    caption="Backups"
                    headers={['Backup', 'State', 'Integrity', 'Steps']}
                    widths={[undefined, '9.5rem', '7.5rem', '7.5rem']}
                    rows={backups.map((b) => {
                      const rowId = `${base}-${b.backup_id}`;
                      return {
                        key: b.backup_id,
                        cells: [
                          <span className="row-main">
                            <code id={rowId}>{b.backup_id}</code>
                            <span className="row-sub">Created {formatDateTime(b.created_at, today.zone)}</span>
                            <span className="row-sub">
                              {b.restore_tested_at === null ? 'Restore not tested' : `Restore tested ${formatDateTime(b.restore_tested_at, today.zone)}`}
                            </span>
                          </span>,
                          <StateBadge state={b.state} />,
                          <span className="row-main">
                            <span title={b.integrity}>{INTEGRITY_WORDS[b.integrity]}</span>
                            {b.manifest_sha256 === null ? (
                              <span className="row-sub">No fingerprint</span>
                            ) : (
                              <span className="hash" title={`Fingerprint of the manifest (SHA-256): ${b.manifest_sha256}`}>
                                {shortHash(b.manifest_sha256)}
                              </span>
                            )}
                          </span>,
                          <span className="stack-sm">
                            <button
                              type="button"
                              className="sm"
                              aria-describedby={rowId}
                              disabled={busy || !can(BACKUP_MACHINE, b.state, 'CHECK')}
                              onClick={() => {
                                void step('backup.verify', b);
                              }}
                            >
                              Verify
                            </button>
                            <button
                              type="button"
                              className="sm"
                              aria-describedby={rowId}
                              disabled={busy || !can(BACKUP_MACHINE, b.state, 'TEST_RESTORE')}
                              onClick={() => {
                                void step('backup.restore_test', b);
                              }}
                            >
                              Test restore
                            </button>
                          </span>,
                        ],
                      };
                    })}
                  />
                  <p className="field-hint">Only the step a backup can take now is enabled. The backend checks every step again.</p>
                </>
              )}
              <div>
                <h3>Restoring a backup</h3>
                <p className="panel-note" data-testid="restore-guidance">
                  Restoring a kept backup over the live database is not available in this window; it is not built yet. To restore, close G9 Mentor and run <code>g9 restore</code>{' '}
                  from the command line. Without <code>--confirm-replace-live-database</code> it only prints the plan. With it, the verified snapshot replaces the database, and the
                  old one is kept in <code>quarantine/</code>, never deleted. The steps are in <code>docs/runbooks/backup-restore.md</code>.
                </p>
              </div>
            </div>
          </Panel>

          <Panel
            title="Export your data"
            note={
              <>
                The backend writes an export to a new folder on this machine: <code>data.json</code> (or one CSV file per table), copies of the attached files, and{' '}
                <code>manifest.json</code>, written last, which lists every file with its SHA-256. The manifest is not signed. Erased content is left out.
              </>
            }
          >
            <div className="stack-sm">
              <form
                aria-label="Export your data"
                onSubmit={(e) => {
                  e.preventDefault();
                  if (canRequest) {
                    void requestExport();
                  }
                }}
              >
                <div className="form-grid">
                  <fieldset className="field wide">
                    <legend>Areas</legend>
                    <div className="row">
                      {DATA_DOMAINS.map((d) => (
                        <label className="check" key={d}>
                          <input
                            type="checkbox"
                            checked={areas.includes(d)}
                            onChange={(e) => {
                              setAreas(e.target.checked ? [...areas.filter((x) => x !== d), d] : areas.filter((x) => x !== d));
                            }}
                          />
                          {domainLabel(d)}
                        </label>
                      ))}
                    </div>
                    {chosenAreas.length === 0 ? (
                      <span className="field-error" role="alert">
                        Tick at least one area.
                      </span>
                    ) : (
                      <span className="field-hint">The records of every area you tick are exported.</span>
                    )}
                  </fieldset>
                  <div className="field wide">
                    <label className="check">
                      <input
                        type="checkbox"
                        checked={includePrivate}
                        aria-describedby={privateHint}
                        onChange={(e) => {
                          setIncludePrivate(e.target.checked);
                        }}
                      />
                      Include private records
                    </label>
                    <span className="field-hint" id={privateHint}>
                      Private records are left out unless you tick this. Secret records are never exported.
                    </span>
                  </div>
                  <fieldset className="field">
                    <legend>Format</legend>
                    <div className="seg">
                      {EXPORT_FORMATS.map((f) => (
                        <button
                          type="button"
                          key={f}
                          aria-pressed={format === f}
                          onClick={() => {
                            setFormat(f);
                          }}
                        >
                          {f}
                        </button>
                      ))}
                    </div>
                  </fieldset>
                </div>
                <div className="form-actions">
                  <button type="submit" disabled={!canRequest}>
                    Request export
                  </button>
                  <span className="form-note">Nothing is written until you approve the request.</span>
                </div>
              </form>
              {exportFlow.error !== null ? <ErrorDisplay error={exportFlow.error} /> : null}
              {exportFlow.cancelled ? <ConfirmationCancelled /> : null}
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
              {exported !== null ? (
                <div className="stack-sm" role="status" data-testid="export-result">
                  <p className={exportWritten ? 'receipt' : undefined}>
                    {exportWritten ? 'Export written' : 'Export requested'}: {exported.domains.map(domainLabel).join(', ')} as {exported.format},{' '}
                    <StateBadge state={exported.state} />.
                  </p>
                  <dl className="pairs">
                    <dt>Folder</dt>
                    <dd>
                      <code data-testid="export-location">{exported.location}</code>
                    </dd>
                    <dt>Rows</dt>
                    <dd className="num">{exported.rows}</dd>
                    <dt>Files</dt>
                    <dd className="num">{exported.files} (manifest.json included)</dd>
                    <dt>Attached files copied</dt>
                    <dd className="num">{exported.attachments}</dd>
                    <dt title="SHA-256 of manifest.json, which lists every file of the export with its own SHA-256.">Manifest fingerprint</dt>
                    <dd>
                      <span className="hash" data-testid="export-manifest-sha256">
                        {exported.manifest_sha256}
                      </span>
                    </dd>
                    <dt>Requested</dt>
                    <dd>{formatDateTime(exported.requested_at, today.zone)}</dd>
                    <dt>Export</dt>
                    <dd>
                      <code>{exported.export_id}</code>
                    </dd>
                  </dl>
                  <p className="field-hint">
                    The folder is outside this window: open it in your file manager. To check the export later, compare the SHA-256 of manifest.json with the fingerprint above,
                    then each file with the SHA-256 the manifest lists.
                  </p>
                </div>
              ) : null}
              <Warnings warnings={exportFlow.warnings} label="Export warnings" />
            </div>
          </Panel>

          <ProgramSetup
            heading="Program"
            program={program}
            zone={today.zone}
            now={today.now}
            todayIso={today.isoDate}
            onChanged={today.reload}
          />

          <Panel title="Model">
            {s === null ? (
              statusPending('the model setting', 'The model setting')
            ) : s.model.available ? (
              <p>
                <Badge tone="verified" title="AVAILABLE">
                  Available
                </Badge>{' '}
                {s.model.model_id === undefined ? 'The backend did not name the model.' : <code>{s.model.model_id}</code>}
              </p>
            ) : (
              <div className="stack-sm" data-testid="runtime-none">
                {s.model.reason === undefined || s.model.reason === 'NO_RUNTIME_CONFIGURED' ? (
                  <p>
                    <Badge tone="off" title={s.model.reason ?? 'UNKNOWN'}>
                      None configured
                    </Badge>{' '}
                    <span className="muted">No model runtime is set up. This is a setting, not a fault.</span>
                  </p>
                ) : (
                  <p>
                    <Badge tone="off" title={s.model.reason}>
                      Not available
                    </Badge>{' '}
                    <span className="muted">
                      The backend gives the reason as <span title={s.model.reason}>{sentenceCase(s.model.reason).toLowerCase()}</span>.
                    </span>
                  </p>
                )}
                <p className="panel-note">
                  Planning, notes, timers and reviews work without a model; only advice from the mentor needs one. No model is chosen for you: loading one is your decision alone,
                  and its file must match the checksum in the registry.
                </p>
              </div>
            )}
          </Panel>

          <Panel
            title="Appearance"
            note={`${SKINS.length} themes. Every one keeps the same marks: posted is solid, pencilled is outlined, in doubt is hatched, advice is dashed, owed is red, and every mark keeps its name in words. Each palette is checked for contrast on every build.`}
          >
            <div className="stack-sm">
              <div className="form-grid">
                {fixed === null ? (
                  <fieldset className="field">
                    <legend>Colour scheme</legend>
                    <div className="seg">
                      {THEME_MODES.map((mode) => (
                        <button
                          type="button"
                          key={mode}
                          aria-pressed={scheme === mode}
                          onClick={() => {
                            setScheme(mode);
                          }}
                        >
                          {SCHEME_WORDS[mode]}
                        </button>
                      ))}
                    </div>
                  </fieldset>
                ) : (
                  <div className="field">
                    <p className="field-hint">
                      {skinById(skin).name} is a {fixed} theme; the light or dark setting applies to Ledger.
                    </p>
                  </div>
                )}
                <fieldset className="field">
                  <legend>Contrast</legend>
                  <div className="seg">
                    <button
                      type="button"
                      aria-pressed={contrast !== 'high'}
                      onClick={() => {
                        if (contrast === 'high') {
                          toggleContrast();
                        }
                      }}
                    >
                      Theme colours
                    </button>
                    <button type="button" aria-pressed={contrast === 'high'} onClick={toggleContrast}>
                      High contrast
                    </button>
                  </div>
                </fieldset>
              </div>
              <p className="field-hint">
                High contrast keeps the theme’s type and shapes and replaces its colours with pure ink on pure paper. Your choices are kept in this window’s local storage (
                <code>g9.skin</code>, <code>g9.theme</code>, <code>g9.contrast</code>).
              </p>
              <ThemeGallery value={skin} onChange={setSkin} scheme={previewScheme(scheme)} />
            </div>
          </Panel>

          <Panel title="About this window" note="What the backend reports about this installation. These are settings, not faults.">
            {s === null ? (
              statusPending('the facts about this window', 'The facts about this window')
            ) : (
              <dl className="pairs">
                <dt>Transport</dt>
                <dd>
                  {s.transport === 'mock' ? 'Built-in mock backend: fixture data, nothing is stored.' : 'Desktop backend.'} <code>{s.transport}</code>
                </dd>
                <dt>Storage</dt>
                <dd>
                  <StateBadge state={s.storage.state} />
                </dd>
                <dt>Backend clock</dt>
                <dd>
                  <time dateTime={new Date(s.clock.now).toISOString()}>{formatDateTime(s.clock.now, s.clock.zone)}</time>, as this page last read it
                </dd>
                <dt>Time zone</dt>
                <dd>{s.clock.zone}</dd>
                <dt>Application version</dt>
                <dd className="num">{s.app_version}</dd>
                <dt>Host warnings</dt>
                {today.warnings.length === 0 ? (
                  <dd data-testid="status-warnings-none">None reported</dd>
                ) : (
                  <dd>{today.warnings.length} reported, shown at the top of this page</dd>
                )}
              </dl>
            )}
          </Panel>
        </div>

        <div className="sticky">
          <Margin label="Notes on this page">
            <nav className="margin-note" aria-label="On this page">
              <h3>On this page</h3>
              <div className="toolbar">
                {SECTIONS.map((title) => (
                  <button
                    type="button"
                    className="quiet sm"
                    key={title}
                    onClick={() => {
                      jumpTo(title);
                    }}
                  >
                    {title}
                  </button>
                ))}
              </div>
            </nav>
            <MarginNote title="Keep a copy elsewhere">
              <p>
                A backup is stored on the same disk as the database, so one failed disk takes both. Copy each kept backup to a USB drive or a NAS from the command line:{' '}
                <code>g9 copy-backup --backup-id &lt;id&gt; --to &lt;folder&gt;</code>.
              </p>
            </MarginNote>
            <MarginNote title="What an approval is for">
              <p>
                An export takes records out of the application, so it waits for your approval. The approval covers exactly the areas, the format and the choice about private records
                you asked for, and it is used once.
              </p>
            </MarginNote>
          </Margin>
        </div>
      </div>
    </div>
  );
}
