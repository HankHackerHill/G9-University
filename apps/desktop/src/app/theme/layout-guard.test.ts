// @vitest-environment node
/**
 * Source guard for the reflow defect found by measuring the running app: `grid-template-columns: 1fr 1fr`
 * means `minmax(auto, 1fr)`, so a wide table made the Assessments and Session Tracker columns wider than
 * their container (1132 px of columns in 772 px). jsdom cannot measure layout, so this only checks that the
 * rules stay in the safe form; the real check is the browser measurement recorded in the stage report.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const css = readFileSync(fileURLToPath(new URL('./components.css', import.meta.url)), 'utf8');

/** Body of the first top-level rule for `selector` inside the components layer. */
function rule(selector: string): string {
  const pattern = new RegExp(`\\n\\s*${selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\s*\\{([^}]*)\\}`);
  const body = pattern.exec(css)?.[1];
  if (body === undefined) {
    throw new Error(`rule ${selector} not found`);
  }
  return body;
}

describe('components.css layout guard', () => {
  it('multi-column grids use minmax(0, …) tracks so wide content cannot widen them', () => {
    for (const sel of ['.grid-2', '.grid-3', '.split', '.with-margin']) {
      const body = rule(sel);
      expect(body, sel).toMatch(/grid-template-columns:\s*minmax\(0,/);
      expect(body, sel).not.toMatch(/grid-template-columns:\s*\d*fr\s/);
    }
  });

  it('regions contain their own horizontal overflow', () => {
    const body = rule('.panel');
    expect(body).toMatch(/min-width:\s*0/);
    expect(body).toMatch(/overflow-x:\s*auto/);
  });
});
