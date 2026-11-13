// @vitest-environment node
/**
 * Every theme, every run: colours inside sRGB, text at least 4.5:1 on every surface and fill,
 * control borders at least 3:1 (WCAG 2.2 SC 1.4.3 and 1.4.11, computed from the palette
 * values; textures drawn by skins.css are not part of the computation). The Ledger entry must
 * equal tokens.css, which draws the window before any script runs.
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { contrast, cssOklch, inGamut, lowestTextContrast, paletteChecks, raisedOf, rampOf, toSrgb, type Oklch, type Palette } from './contrast';
import { fixedScheme, paletteOf, SKIN_GROUPS, SKINS, skinById } from './skins';

const tokens = readFileSync(fileURLToPath(new URL('./tokens.css', import.meta.url)), 'utf8');

const TOKEN_NAMES: Record<string, keyof Palette> = {
  paper: 'paper',
  sheet: 'sheet',
  'sheet-2': 'sheet2',
  rule: 'rule',
  'rule-2': 'rule2',
  ink: 'ink',
  'ink-2': 'ink2',
  'ink-3': 'ink3',
  owed: 'owed',
  doubt: 'doubt',
  advice: 'advice',
  live: 'live',
  accent: 'accent',
};

function block(selector: string): string {
  const start = tokens.indexOf(`${selector} {`);
  if (start === -1) {
    throw new Error(`${selector} not found in tokens.css`);
  }
  return tokens.slice(start, tokens.indexOf('\n  }', start));
}

const num = (s: string | undefined) => Number(s);

/** The light and dark values of each colour token declared with light-dark(oklch(), oklch()). */
function lightDark(css: string): { light: Record<string, Oklch>; dark: Record<string, Oklch> } {
  const light: Record<string, Oklch> = {};
  const dark: Record<string, Oklch> = {};
  const re = /--([a-z0-9-]+):\s*light-dark\(oklch\(([\d.]+) ([\d.]+) ([\d.]+)\),\s*oklch\(([\d.]+) ([\d.]+) ([\d.]+)\)\)/g;
  for (const m of css.matchAll(re)) {
    const key = TOKEN_NAMES[m[1] ?? ''];
    if (key !== undefined) {
      light[key] = [num(m[2]), num(m[3]), num(m[4])];
      dark[key] = [num(m[5]), num(m[6]), num(m[7])];
    }
  }
  return { light, dark };
}

const ALL_PALETTES: { name: string; palette: Palette }[] = SKINS.flatMap((skin) =>
  (['light', 'dark'] as const).flatMap((scheme) => {
    const p = skin.palettes[scheme];
    return p === undefined ? [] : [{ name: `${skin.name} (${scheme})`, palette: p }];
  }),
);

describe('theme registry', () => {
  it('offers the house style and twenty more themes, each in one group, with unique ids and names', () => {
    expect(SKINS).toHaveLength(21);
    expect(new Set(SKINS.map((s) => s.id)).size).toBe(21);
    expect(new Set(SKINS.map((s) => s.name)).size).toBe(21);
    const groups = new Set(SKIN_GROUPS.map((g) => g.id));
    for (const skin of SKINS) {
      expect(groups.has(skin.group), skin.id).toBe(true);
      expect(skin.description.length, skin.id).toBeGreaterThan(10);
    }
    expect(skinById('ledger').group).toBe('house');
  });

  it('gives Ledger both schemes and every other theme exactly one, matching the scheme of its palette', () => {
    for (const skin of SKINS) {
      const fixed = fixedScheme(skin);
      if (skin.id === 'ledger') {
        expect(fixed).toBeNull();
      } else {
        expect(fixed, skin.id).not.toBeNull();
        expect(paletteOf(skin, 'light').scheme, skin.id).toBe(fixed);
      }
    }
  });

  it('includes the kinds of theme that were asked for', () => {
    const ids = SKINS.map((s) => s.id);
    for (const wanted of ['hacker', 'amber-crt', 'classic-desktop', 'pocket-console', 'clean', 'modern-tech', 'professional', 'synthwave']) {
      expect(ids).toContain(wanted);
    }
  });
});

describe.each(ALL_PALETTES)('palette $name', ({ palette }) => {
  it('keeps every colour inside the sRGB gamut', () => {
    for (const [key, value] of Object.entries(palette)) {
      if (Array.isArray(value)) {
        expect(inGamut(value as unknown as Oklch), key).toBe(true);
      }
    }
  });

  it('meets every contrast pair (text 4.5:1, control borders 3:1)', () => {
    const failing = paletteChecks(palette)
      .filter((c) => c.ratio < c.minimum)
      .map((c) => `${c.pair}: ${c.ratio.toFixed(2)} < ${c.minimum}`);
    expect(failing).toEqual([]);
    expect(lowestTextContrast(palette)).toBeGreaterThanOrEqual(4.5);
  });
});

describe.each(ALL_PALETTES)('ordinal ramp of $name', ({ palette }) => {
  const ramp = rampOf(palette);

  it('is one hue with lightness moving one way in steps a reader can tell apart', () => {
    const hues = new Set(ramp.map(([, , h]) => h));
    expect(hues.size).toBe(1);
    const steps = ramp.slice(1).map(([l], i) => Math.abs(l - (ramp[i]?.[0] ?? l)));
    // Dataviz rule for a sequential scale: monotone lightness, at least 0.06 between neighbours.
    const direction = Math.sign((ramp[3]?.[0] ?? 0) - (ramp[0]?.[0] ?? 0));
    for (let i = 1; i < ramp.length; i += 1) {
      expect(Math.sign((ramp[i]?.[0] ?? 0) - (ramp[i - 1]?.[0] ?? 0))).toBe(direction);
    }
    expect(Math.min(...steps)).toBeGreaterThanOrEqual(0.06);
  });

  it('stays inside sRGB and keeps its faintest step visible on the raised surface (2:1 or better)', () => {
    for (const step of ramp) {
      expect(inGamut(step), cssOklch(step)).toBe(true);
    }
    const faintest = ramp[0];
    expect(faintest).toBeDefined();
    if (faintest !== undefined) {
      expect(contrast(toSrgb(faintest), toSrgb(raisedOf(palette)))).toBeGreaterThanOrEqual(2);
    }
  });
});

describe('tokens.css and the registry agree', () => {
  it('Ledger light and dark equal the light-dark pairs of tokens.css', () => {
    const { light, dark } = lightDark(block(':root'));
    const ledger = skinById('ledger');
    for (const key of Object.values(TOKEN_NAMES)) {
      expect(light[key], `light ${key}`).toEqual(ledger.palettes.light?.[key]);
      expect(dark[key], `dark ${key}`).toEqual(ledger.palettes.dark?.[key]);
    }
  });

  it('the high-contrast palettes of tokens.css meet the same pairs and reach 7:1 for body text', () => {
    const { light, dark } = lightDark(block(":root[data-contrast='high']"));
    for (const [scheme, values] of [
      ['light', light],
      ['dark', dark],
    ] as const) {
      const palette = { scheme, ...values } as Palette;
      const failing = paletteChecks(palette)
        .filter((c) => c.ratio < c.minimum)
        .map((c) => `${scheme} ${c.pair}: ${c.ratio.toFixed(2)}`);
      expect(failing).toEqual([]);
      for (const surface of [palette.paper, palette.sheet, palette.sheet2]) {
        expect(contrast(toSrgb(palette.ink), toSrgb(surface)), `${scheme} ink`).toBeGreaterThanOrEqual(7);
      }
    }
  });
});
