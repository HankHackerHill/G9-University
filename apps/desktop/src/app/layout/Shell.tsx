import { Suspense, useCallback, useEffect, useMemo, useState } from 'react';
import { NavLink, Outlet, useNavigate } from 'react-router-dom';
import { formatDate, formatTime } from '../../lib/format';
import { useIpc, useMutationEpoch } from '../ipc-context';
import { useCommand } from '../hooks/useCommand';
import { SCREENS } from '../router';
import { useContrast } from '../theme/useContrast';
import { THEME_MODES, useTheme, type ThemeMode } from '../theme/useTheme';
import { fixedScheme, SKIN_GROUPS, SKIN_IDS, SKINS, skinById } from '../theme/skins';
import { useSkin } from '../theme/useSkin';
import { Warnings } from '../components/Warnings';
import { CommandPalette, type PaletteCommand } from './CommandPalette';
import { DayRail } from './DayRail';
import { LedgerKey } from './LedgerKey';
import { NAV_GROUPS } from './nav';

type Overlay = 'none' | 'palette' | 'key';

const THEME_LABEL: Record<ThemeMode, string> = { system: 'System', light: 'Light', dark: 'Dark' };

function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  return target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName);
}

/**
 * The application shell: the table of contents, the day rail with the instrument lamps, and
 * the page. Host facts come from `system.status`; nothing here is decided by the window.
 */
export function Shell() {
  const client = useIpc();
  const epoch = useMutationEpoch();
  const status = useCommand('system.status', {}, undefined, epoch);
  const [contrast, toggleContrast] = useContrast();
  const [theme, setTheme] = useTheme();
  const [skin, setSkin] = useSkin();
  const followsScheme = fixedScheme(skinById(skin)) === null;
  const [overlay, setOverlay] = useState<Overlay>('none');
  const navigate = useNavigate();
  const s = status.data;

  const closeOverlay = useCallback(() => {
    setOverlay('none');
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        setOverlay((current) => (current === 'palette' ? 'none' : 'palette'));
      } else if (e.key === '?' && !isTyping(e.target)) {
        e.preventDefault();
        setOverlay('key');
      }
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, []);

  const commands = useMemo<PaletteCommand[]>(() => {
    const go: PaletteCommand[] = NAV_GROUPS.flatMap((group) =>
      group.screens.flatMap((id) => {
        const screen = SCREENS.find((x) => x.id === id);
        return screen === undefined
          ? []
          : [
              {
                id: `go-${screen.id}`,
                label: screen.title,
                hint: group.title,
                keywords: `go open ${screen.id}`,
                run: () => {
                  void navigate(screen.path, { viewTransition: true });
                },
              },
            ];
      }),
    );
    const themes: PaletteCommand[] = SKINS.filter((x) => x.id !== skin).map((x) => ({
      id: `skin-${x.id}`,
      label: `Theme: ${x.name}`,
      hint: 'Appearance',
      keywords: `theme skin look ${SKIN_GROUPS.find((g) => g.id === x.group)?.title ?? ''} ${x.description}`,
      run: () => {
        setSkin(x.id);
      },
    }));
    const schemes: PaletteCommand[] = followsScheme
      ? THEME_MODES.filter((mode) => mode !== theme).map((mode) => ({
        id: `theme-${mode}`,
        label: mode === 'system' ? 'Follow the system colour scheme' : `Switch to the ${mode} scheme`,
        hint: 'Appearance',
        keywords: 'theme colour color dark light',
        run: () => {
          setTheme(mode);
        },
      }))
      : [];
    const appearance: PaletteCommand[] = [
      ...themes,
      ...schemes,
      {
        id: 'contrast',
        label: contrast === 'high' ? 'Turn high contrast off' : 'Turn high contrast on',
        hint: 'Appearance',
        keywords: 'accessibility contrast',
        run: toggleContrast,
      },
      {
        id: 'key',
        label: 'How to read the ledger',
        hint: 'Help',
        keywords: 'legend key help marks shortcuts',
        run: () => {
          setOverlay('key');
        },
      },
    ];
    return [...go, ...appearance];
  }, [navigate, theme, setTheme, contrast, toggleContrast, skin, setSkin, followsScheme]);

  return (
    <div className="shell">
      <a className="skip-link" href="#main">
        Skip to main content
      </a>
      {client.transportKind === 'mock' ? (
        <div className="mock-banner" role="status" data-testid="mock-banner">
          <strong>SYNTHETIC DATA.</strong> This window is running the built-in mock backend (the Tauri runtime is not present).
          Nothing shown here is read from, or saved to, the G9 database.
        </div>
      ) : null}

      <div className="shell-side">
        <p className="brand">
          <span className="visually-hidden">G9 Mentor</span>
          <span className="brand-unit" aria-hidden="true">
            G9
          </span>
          <span className="brand-role" aria-hidden="true">
            Mentor
          </span>
        </p>
        <div className="shell-search">
          <button
            type="button"
            onClick={() => {
              setOverlay('palette');
            }}
          >
            <span>
              Search<span className="visually-hidden"> and commands</span>
            </span>
            <kbd aria-hidden="true">Ctrl K</kbd>
          </button>
        </div>
        <nav className="shell-nav" aria-label="Screens">
          {NAV_GROUPS.map((group) => (
            <div key={group.id}>
              <h2 id={`nav-${group.id}`}>{group.title}</h2>
              <ul aria-labelledby={`nav-${group.id}`}>
                {group.screens.map((id) => {
                  const screen = SCREENS.find((x) => x.id === id);
                  if (screen === undefined) {
                    return null;
                  }
                  const pending = screen.id === 'approvals' && s !== null && s.pending_approvals > 0 ? s.pending_approvals : undefined;
                  return (
                    <li key={screen.id}>
                      <NavLink
                        to={screen.path}
                        end={screen.path === '/'}
                        viewTransition
                        data-pending={pending}
                        title={pending === undefined ? undefined : `${pending} pending`}
                      >
                        {screen.title}
                      </NavLink>
                    </li>
                  );
                })}
              </ul>
            </div>
          ))}
        </nav>
        <div className="shell-tools" role="group" aria-label="Appearance and help">
          <label className="theme-pick">
            <span>Theme</span>
            <select
              value={skin}
              onChange={(e) => {
                const next = SKIN_IDS.find((id) => id === e.target.value);
                if (next !== undefined) {
                  setSkin(next);
                }
              }}
            >
              {SKIN_GROUPS.map((group) => (
                <optgroup key={group.id} label={group.title}>
                  {SKINS.filter((x) => x.group === group.id).map((x) => (
                    <option key={x.id} value={x.id}>
                      {x.name}
                    </option>
                  ))}
                </optgroup>
              ))}
            </select>
          </label>
          {followsScheme ? (
          <div className="seg" role="group" aria-label="Colour scheme">
            {THEME_MODES.map((mode) => (
              <button
                type="button"
                key={mode}
                aria-pressed={theme === mode}
                onClick={() => {
                  setTheme(mode);
                }}
              >
                {THEME_LABEL[mode]}
              </button>
            ))}
          </div>
          ) : null}
          <button type="button" onClick={toggleContrast} aria-pressed={contrast === 'high'}>
            High contrast
          </button>
          <button
            type="button"
            onClick={() => {
              setOverlay('key');
            }}
          >
            Ledger key
            <kbd aria-hidden="true">?</kbd>
          </button>
        </div>
      </div>

      <header className="shell-header">
        <DayRail status={s} epoch={epoch} />
        <div className="status-strip" role="group" aria-live="polite" aria-label="System status">
          {s === null ? (
            <span className="muted">{status.error === null ? 'Loading status…' : `Status unavailable (${status.error.category})`}</span>
          ) : (
            <>
              <span className="lamps">
              <span className="lamp" data-lamp={s.model.available ? 'on' : 'off'}>
                <span className="lamp-name">Model</span>
                <span className="lamp-value">{s.model.available ? 'AVAILABLE' : <>UNAVAILABLE · {s.model.reason ?? 'UNKNOWN'}</>}</span>
              </span>
              <span className="lamp" data-lamp={s.network.egress_allowed ? 'doubt' : 'off'}>
                <span className="lamp-name">Network</span>
                <span className="lamp-value">{s.network.egress_allowed ? 'EGRESS ALLOWED' : 'OFF'}</span>
              </span>
              <span className="lamp" data-lamp={s.storage.state === 'READY' ? 'on' : s.storage.state === 'UNAVAILABLE' ? 'owed' : 'doubt'}>
                <span className="lamp-name">Storage</span>
                <span className="lamp-value">{s.storage.state}</span>
              </span>
              <span className="lamp" data-lamp={s.transport === 'mock' ? 'doubt' : 'on'}>
                <span className="lamp-name">Transport</span>
                <span className="lamp-value">{s.transport.toUpperCase()}</span>
              </span>
              <span className="lamp" data-lamp={s.pending_approvals > 0 ? 'doubt' : 'off'}>
                <span className="lamp-name">Approvals pending</span>
                <span className="lamp-value">{s.pending_approvals}</span>
              </span>
              </span>
              <span className="lamp-clock">
                <span className="visually-hidden">Backend clock: </span>
                <time dateTime={new Date(s.clock.now).toISOString()}>{formatTime(s.clock.now, s.clock.zone)}</time>
                <span>
                  {formatDate(s.clock.now, s.clock.zone)} {s.clock.zone}
                </span>
              </span>
            </>
          )}
        </div>
        <Warnings warnings={status.warnings} label="System status warnings (host facts reported by system.status)" testId="status-warnings" />
      </header>

      <main id="main" className="shell-main" tabIndex={-1}>
        <Suspense fallback={<p className="muted">Loading screen…</p>}>
          <Outlet />
        </Suspense>
      </main>

      {overlay === 'palette' ? <CommandPalette commands={commands} onClose={closeOverlay} /> : null}
      {overlay === 'key' ? <LedgerKey onClose={closeOverlay} /> : null}
    </div>
  );
}
