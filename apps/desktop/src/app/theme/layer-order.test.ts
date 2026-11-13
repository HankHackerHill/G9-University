// @vitest-environment node
/**
 * The order of the cascade layers is fixed by the first stylesheet that names a layer. A screen
 * stylesheet that reached the document before `theme.css` made `screens` the lowest layer, and
 * every screen rule then lost to the component rule it was meant to adjust (found by measuring
 * the running app: `.cal-event` stayed `nowrap`). Two guards, both checked here:
 *   1. `main.tsx` imports `theme.css` before anything else, so the shared order comes first;
 *   2. every stylesheet under `theme/screens/` states the same order itself before its rules,
 *      so the result does not depend on which chunk loads first.
 * jsdom does not apply layers, so this is a source guard; the measurement is in
 * docs/ui/design-critique-2026-11-13.md.
 */
import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const ORDER = '@layer reset, tokens, base, layout, components, screens, skins, utilities;';
const read = (relative: string) => readFileSync(fileURLToPath(new URL(relative, import.meta.url)), 'utf8');
const withoutComments = (css: string) => css.replace(/\/\*[\s\S]*?\*\//g, '').trim();

describe('cascade layer order', () => {
  it('theme.css states the order before any rule or import', () => {
    expect(withoutComments(read('./theme.css')).startsWith(ORDER)).toBe(true);
  });

  it('main.tsx imports theme.css before any other module', () => {
    const firstImport = /^\s*import\s[^\n]*$/m.exec(read('../../main.tsx'))?.[0].trim();
    expect(firstImport).toBe("import './app/theme/theme.css';");
  });

  const screensDir = fileURLToPath(new URL('./screens/', import.meta.url));
  const sheets = readdirSync(screensDir).filter((f) => f.endsWith('.css'));

  it('finds the screen stylesheets', () => {
    expect(sheets.length).toBeGreaterThan(0);
  });

  it.each(sheets)('screens/%s states the order first and puts every rule in the screens layer', (file) => {
    const css = withoutComments(read(`./screens/${file}`));
    expect(css.startsWith(ORDER), `${file} must start with the layer order statement`).toBe(true);
    // After the statement, the file is one or more `@layer screens { … }` blocks and nothing else at the top level.
    const rest = css.slice(ORDER.length).trim();
    expect(rest.startsWith('@layer screens {'), `${file}: rules must sit in @layer screens`).toBe(true);
    let depth = 0;
    let topLevel = '';
    for (const ch of rest) {
      if (ch === '{') {
        depth += 1;
      } else if (ch === '}') {
        depth -= 1;
      } else if (depth === 0) {
        topLevel += ch;
      }
    }
    expect(topLevel.replace(/@layer screens/g, '').trim(), `${file}: nothing outside @layer screens`).toBe('');
  });
});
