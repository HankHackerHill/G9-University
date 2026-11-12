/**
 * The table of contents. The 16 screens of ARCHITECTURE §12 are grouped by the question the
 * operator is asking, in the order of a working day: what do I do now, what is planned, what
 * has been proven, what should change, what else is on the desk, who decides.
 *
 * Screen ids are those of `SCREENS` (router.tsx); `nav.test.ts` checks that every screen is in
 * exactly one group.
 */
export interface NavGroup {
  id: string;
  title: string;
  screens: readonly string[];
}

export const NAV_GROUPS: readonly NavGroup[] = [
  { id: 'today', title: 'Today', screens: ['mission-control', 'daily-classroom', 'session'] },
  { id: 'plan', title: 'Plan', screens: ['calendar', 'curriculum'] },
  { id: 'prove', title: 'Prove', screens: ['assessments', 'competency', 'retention'] },
  { id: 'improve', title: 'Improve', screens: ['performance', 'interventions', 'journal'] },
  { id: 'work', title: 'Work', screens: ['executive', 'research', 'memory'] },
  { id: 'control', title: 'Control', screens: ['approvals', 'settings'] },
];
