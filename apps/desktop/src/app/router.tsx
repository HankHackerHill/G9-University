/**
 * The 16 screens (ARCHITECTURE §12) as lazy routes under the application shell.
 * `SCREENS` is the single registry used by navigation, tests and `docs/ui/screens.md`.
 */
import type { RouteObject } from 'react-router-dom';
import { Shell } from './layout/Shell';

export interface ScreenDef {
  id: string;
  path: string;
  title: string;
  load: () => Promise<{ Component: React.ComponentType }>;
}

export const SCREENS: readonly ScreenDef[] = [
  { id: 'mission-control', path: '/', title: 'Mission Control', load: () => import('./screens/MissionControl').then((m) => ({ Component: m.MissionControl })) },
  { id: 'daily-classroom', path: '/classroom', title: 'Daily Classroom', load: () => import('./screens/DailyClassroom').then((m) => ({ Component: m.DailyClassroom })) },
  { id: 'calendar', path: '/calendar', title: 'Calendar', load: () => import('./screens/CalendarScreen').then((m) => ({ Component: m.CalendarScreen })) },
  { id: 'curriculum', path: '/curriculum', title: 'Curriculum Navigator', load: () => import('./screens/CurriculumNavigator').then((m) => ({ Component: m.CurriculumNavigator })) },
  { id: 'session', path: '/session', title: 'Session Tracker', load: () => import('./screens/SessionTracker').then((m) => ({ Component: m.SessionTracker })) },
  { id: 'competency', path: '/competency', title: 'Competency Graph', load: () => import('./screens/CompetencyGraph').then((m) => ({ Component: m.CompetencyGraph })) },
  { id: 'assessments', path: '/assessments', title: 'Assessments', load: () => import('./screens/Assessments').then((m) => ({ Component: m.Assessments })) },
  { id: 'retention', path: '/retention', title: 'Retention Center', load: () => import('./screens/RetentionCenter').then((m) => ({ Component: m.RetentionCenter })) },
  { id: 'interventions', path: '/interventions', title: 'Intervention Center', load: () => import('./screens/InterventionCenter').then((m) => ({ Component: m.InterventionCenter })) },
  { id: 'journal', path: '/journal', title: 'Learning Journal', load: () => import('./screens/LearningJournal').then((m) => ({ Component: m.LearningJournal })) },
  { id: 'performance', path: '/performance', title: 'Performance Dashboard', load: () => import('./screens/PerformanceDashboard').then((m) => ({ Component: m.PerformanceDashboard })) },
  { id: 'executive', path: '/executive', title: 'Executive Workspace', load: () => import('./screens/ExecutiveWorkspace').then((m) => ({ Component: m.ExecutiveWorkspace })) },
  { id: 'memory', path: '/memory', title: 'Mentor Memory Explorer', load: () => import('./screens/MentorMemoryExplorer').then((m) => ({ Component: m.MentorMemoryExplorer })) },
  { id: 'research', path: '/research', title: 'Research Center', load: () => import('./screens/ResearchCenter').then((m) => ({ Component: m.ResearchCenter })) },
  { id: 'approvals', path: '/approvals', title: 'Approvals/Audit', load: () => import('./screens/ApprovalsAudit').then((m) => ({ Component: m.ApprovalsAudit })) },
  { id: 'settings', path: '/settings', title: 'Settings/Recovery', load: () => import('./screens/SettingsRecovery').then((m) => ({ Component: m.SettingsRecovery })) },
];

export const routes: RouteObject[] = [
  {
    path: '/',
    element: <Shell />,
    children: SCREENS.map((s): RouteObject => (s.path === '/' ? { index: true, lazy: s.load } : { path: s.path.slice(1), lazy: s.load })),
  },
];
