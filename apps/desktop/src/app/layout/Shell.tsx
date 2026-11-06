import { Suspense } from 'react';
import { NavLink, Outlet } from 'react-router-dom';
import { formatDateTime } from '../../lib/format';
import { useCommand } from '../hooks/useCommand';
import { SCREENS } from '../router';
import { useContrast } from '../theme/useContrast';
import { Badge } from '../components/Badge';

export function Shell() {
  const status = useCommand('system.status', {});
  const [contrast, toggleContrast] = useContrast();
  const s = status.data;

  return (
    <div className="shell">
      <a className="skip-link" href="#main">
        Skip to main content
      </a>
      <header className="shell-header">
        <span className="brand" aria-label="G9 Mentor">
          G9 MENTOR
        </span>
        <div className="status-strip" aria-live="polite" aria-label="System status">
          {s === null ? (
            <span className="muted">{status.error === null ? 'Loading status…' : `Status unavailable (${status.error.category})`}</span>
          ) : (
            <>
              <span>
                <span className="muted">Clock </span>
                <time dateTime={new Date(s.clock.now).toISOString()}>{formatDateTime(s.clock.now, s.clock.zone)}</time> <span className="muted">{s.clock.zone}</span>
              </span>
              <span>
                <span className="muted">Model </span>
                {s.model.available ? <Badge tone="verified">AVAILABLE</Badge> : <Badge tone="off">UNAVAILABLE · {s.model.reason ?? 'UNKNOWN'}</Badge>}
              </span>
              <span>
                <span className="muted">Network </span>
                {s.network.egress_allowed ? <Badge tone="proposed">EGRESS ALLOWED</Badge> : <Badge tone="off">OFF</Badge>}
              </span>
              <span>
                <span className="muted">Storage </span>
                <Badge tone="state">{s.storage.state}</Badge>
              </span>
              <span>
                <span className="muted">Transport </span>
                <Badge tone={s.transport === 'mock' ? 'proposed' : 'state'}>{s.transport.toUpperCase()}</Badge>
              </span>
              <span>
                <span className="muted">Approvals pending </span>
                {s.pending_approvals}
              </span>
            </>
          )}
          <button type="button" onClick={toggleContrast} aria-pressed={contrast === 'high'}>
            High contrast
          </button>
        </div>
      </header>
      <nav className="shell-nav" aria-label="Screens">
        <ol>
          {SCREENS.map((screen) => (
            <li key={screen.id}>
              <NavLink to={screen.path} end={screen.path === '/'}>
                {screen.title}
              </NavLink>
            </li>
          ))}
        </ol>
      </nav>
      <main id="main" className="shell-main" tabIndex={-1}>
        <Suspense fallback={<p className="muted">Loading screen…</p>}>
          <Outlet />
        </Suspense>
      </main>
    </div>
  );
}
