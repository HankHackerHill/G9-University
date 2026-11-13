// @vitest-environment node
/**
 * Source guard for the reflow defect found by measuring the running app: `grid-template-columns: 1fr 1fr`
 * means `minmax(auto, 1fr)`, so a wide table made the Assessments and Session Tracker columns wider than
 * their container (1132 px of columns in 772 px). jsdom cannot measure layout, so this only checks that the
 * rules stay in a safe form (`minmax(0, …)` or `minmax(min(100%, …), 1fr)`); the real check is the browser
 * measurement recorded in docs/ui/design-critique-2026-11-13.md.
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

/**
 * The `grid-template-columns` value of the first rule that ends in `selector` and sets one (a
 * selector may also close a shared list such as `.split, .with-margin { display: grid }`).
 */
function columns(selector: string): string {
  const pattern = new RegExp(`\\n\\s*${selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\s*\\{([^}]*)\\}`, 'g');
  for (const match of css.matchAll(pattern)) {
    const value = /grid-template-columns:\s*([^;]+);/.exec(match[1] ?? '')?.[1];
    if (value !== undefined) {
      return value;
    }
  }
  throw new Error(`no grid-template-columns in any rule for ${selector}`);
}

describe('components.css layout guard', () => {
  it('multi-column grids give every fr track a definite minimum, so wide content cannot widen them', () => {
    for (const sel of ['.grid-2', '.grid-3', '.split', '.with-margin', '.bento', '.form-grid']) {
      const value = columns(sel);
      // A bare `1fr` means `minmax(auto, 1fr)`: the track grows to its widest content.
      const bare = value.replace(/minmax\((?:[^()]|\([^()]*\))*\)/g, '');
      expect(bare, `${sel}: ${value}`).not.toMatch(/\dfr/);
      expect(value, sel).not.toMatch(/minmax\(\s*auto/);
      expect(value, sel).toMatch(/minmax\(/);
    }
  });

  it('regions cannot be widened by their content, and a table scrolls inside its own wrapper', () => {
    expect(rule('.panel')).toMatch(/min-width:\s*0/);
    expect(rule('.instrument')).toMatch(/min-width:\s*0/);
    // Overflow is contained at the table, not at the region: a region that clipped would cut focus rings and hover readouts.
    expect(rule('.table-wrap')).toMatch(/overflow-x:\s*auto/);
    expect(rule('.panel')).not.toMatch(/overflow/);
  });
});
