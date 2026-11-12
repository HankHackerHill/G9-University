// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { SCREENS } from '../router';
import { NAV_GROUPS } from './nav';

describe('table of contents', () => {
  it('places every screen of ARCHITECTURE §12 in exactly one group', () => {
    const grouped = NAV_GROUPS.flatMap((g) => g.screens);
    expect([...grouped].sort()).toEqual(SCREENS.map((s) => s.id).sort());
    expect(new Set(grouped).size).toBe(grouped.length);
  });

  it('names no screen that is not registered', () => {
    const known = new Set(SCREENS.map((s) => s.id));
    for (const id of NAV_GROUPS.flatMap((g) => g.screens)) {
      expect(known.has(id), id).toBe(true);
    }
  });
});
