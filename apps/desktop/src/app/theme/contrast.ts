/**
 * Colour arithmetic for the theme palettes: OKLCH to sRGB, WCAG 2 relative luminance and
 * contrast ratio, and the list of pairs every palette must meet. Used by the theme test (every
 * theme is checked on every run) and by the theme gallery, which shows the measured minimum.
 *
 * Fills are drawn in CSS as `color-mix(in oklch, <colour> N%, transparent)` over the surface,
 * which browsers composite in gamma-encoded sRGB; `composite` reproduces that.
 */

export type Oklch = readonly [l: number, c: number, h: number];
export type Rgb = readonly [r: number, g: number, b: number];

/** Linear-light sRGB, unclamped (values outside 0..1 are outside the sRGB gamut). */
export function oklchToLinear([l, c, h]: Oklch): Rgb {
  const a = c * Math.cos((h * Math.PI) / 180);
  const b = c * Math.sin((h * Math.PI) / 180);
  const l_ = l + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = l - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = l - 0.0894841775 * a - 1.291485548 * b;
  const L = l_ ** 3;
  const M = m_ ** 3;
  const S = s_ ** 3;
  return [
    4.0767416621 * L - 3.3077115913 * M + 0.2309699292 * S,
    -1.2684380046 * L + 2.6097574011 * M - 0.3413193965 * S,
    -0.0041960863 * L - 0.7034186147 * M + 1.707614701 * S,
  ];
}

/**
 * True when the colour is inside sRGB, within a rounding tolerance. Out-of-gamut colours are
 * gamut-mapped by the browser, which would change the contrast that was measured here.
 */
export function inGamut(colour: Oklch, tolerance = 0.002): boolean {
  return oklchToLinear(colour).every((v) => v >= -tolerance && v <= 1 + tolerance);
}

function encode(v: number): number {
  const x = Math.min(1, Math.max(0, v));
  return x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055;
}

function decode(v: number): number {
  return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
}

/** Gamma-encoded sRGB, each channel 0..1. */
export function toSrgb(colour: Oklch): Rgb {
  const [r, g, b] = oklchToLinear(colour);
  return [encode(r), encode(g), encode(b)];
}

export function luminance([r, g, b]: Rgb): number {
  return 0.2126 * decode(r) + 0.7152 * decode(g) + 0.0722 * decode(b);
}

/** WCAG 2 contrast ratio, 1..21. */
export function contrast(a: Rgb, b: Rgb): number {
  const la = luminance(a);
  const lb = luminance(b);
  return (Math.max(la, lb) + 0.05) / (Math.min(la, lb) + 0.05);
}

/** `fg` at `alpha` over an opaque `bg`, in gamma-encoded sRGB (what browsers draw). */
export function composite(fg: Rgb, alpha: number, bg: Rgb): Rgb {
  return [alpha * fg[0] + (1 - alpha) * bg[0], alpha * fg[1] + (1 - alpha) * bg[1], alpha * fg[2] + (1 - alpha) * bg[2]];
}

export function cssOklch([l, c, h]: Oklch): string {
  return `oklch(${l} ${c} ${h})`;
}

/** The colours of one theme in one scheme. Names match the CSS tokens (tokens.css). */
export interface Palette {
  readonly scheme: 'light' | 'dark';
  readonly paper: Oklch;
  readonly sheet: Oklch;
  readonly sheet2: Oklch;
  readonly rule: Oklch;
  readonly rule2: Oklch;
  readonly ink: Oklch;
  readonly ink2: Oklch;
  readonly ink3: Oklch;
  readonly owed: Oklch;
  readonly doubt: Oklch;
  readonly advice: Oklch;
  readonly live: Oklch;
  /** Page titles, the active page marker and the wordmark. Defaults to `ink`. */
  readonly accent?: Oklch;
  /** Opaque fill for things in doubt; derived from `doubt` when absent. */
  readonly doubtFill?: Oklch;
  /** Background of text fields. Defaults to `paper`. */
  readonly field?: Oklch;
  /** Title bars of the window-style regions, and their text. */
  readonly bar?: Oklch;
  readonly onBar?: Oklch;
}

/** Alpha of each translucent fill (tokens.css). */
export const FILL_ALPHA = { doubt: 0.15, owed: 0.13, advice: 0.11, live: 0.14 } as const;

const TEXT_TOKENS = ['ink', 'ink2', 'ink3', 'owed', 'doubt', 'advice', 'live', 'accent'] as const;
const SURFACES = ['paper', 'sheet', 'sheet2'] as const;

export interface ContrastCheck {
  pair: string;
  ratio: number;
  minimum: number;
}

/**
 * Every pair a palette must meet. Text: 4.5:1 (WCAG 2.2 SC 1.4.3), including text on the
 * standing fills. Control borders: 3:1 against the surfaces they sit on (SC 1.4.11).
 */
export function paletteChecks(p: Palette): ContrastCheck[] {
  const colour = (o: Oklch) => toSrgb(o);
  const text: Record<(typeof TEXT_TOKENS)[number], Rgb> = {
    ink: colour(p.ink),
    ink2: colour(p.ink2),
    ink3: colour(p.ink3),
    owed: colour(p.owed),
    doubt: colour(p.doubt),
    advice: colour(p.advice),
    live: colour(p.live),
    accent: colour(p.accent ?? p.ink),
  };
  const surface: Record<(typeof SURFACES)[number], Rgb> = { paper: colour(p.paper), sheet: colour(p.sheet), sheet2: colour(p.sheet2) };
  const checks: ContrastCheck[] = [];
  const need = (pair: string, a: Rgb, b: Rgb, minimum: number) => {
    checks.push({ pair, ratio: contrast(a, b), minimum });
  };
  for (const t of TEXT_TOKENS) {
    for (const s of SURFACES) {
      need(`${t} on ${s}`, text[t], surface[s], 4.5);
    }
  }
  for (const [fill, alpha] of Object.entries(FILL_ALPHA) as [keyof typeof FILL_ALPHA, number][]) {
    for (const s of ['paper', 'sheet'] as const) {
      const bg = fill === 'doubt' && p.doubtFill !== undefined ? colour(p.doubtFill) : composite(colour(p[fill]), alpha, surface[s]);
      need(`ink on ${fill} fill over ${s}`, text.ink, bg, 4.5);
      need(`ink2 on ${fill} fill over ${s}`, text.ink2, bg, 4.5);
    }
  }
  need('owed on owed fill', text.owed, composite(text.owed, FILL_ALPHA.owed, surface.paper), 4.5);
  need('paper on ink (primary buttons, selection)', surface.paper, text.ink, 4.5);
  need('rule2 on paper (control borders)', colour(p.rule2), surface.paper, 3);
  need('rule2 on sheet (control borders)', colour(p.rule2), surface.sheet, 3);
  if (p.field !== undefined) {
    need('ink on field', text.ink, colour(p.field), 4.5);
    need('rule2 on field (control borders)', colour(p.rule2), colour(p.field), 3);
  }
  if (p.bar !== undefined) {
    need('onBar on bar', colour(p.onBar ?? p.paper), colour(p.bar), 4.5);
  }
  return checks;
}

/** Lowest text contrast of a palette (the figure the gallery shows). */
export function lowestTextContrast(p: Palette): number {
  return Math.min(...paletteChecks(p).filter((c) => c.minimum >= 4.5).map((c) => c.ratio));
}

/** `color-mix(in oklch, colour N%, white | black)`: an achromatic colour has no hue to contribute, so the hue is kept. */
function mixWith(colour: Oklch, share: number, target: 'white' | 'black'): Oklch {
  const [l, c, h] = colour;
  return [share * l + (1 - share) * (target === 'white' ? 1 : 0), share * c, h];
}

/** Share of ink in each step of the ordinal ramp, by scheme (tokens.css `--ramp-1 … --ramp-4`). */
export const RAMP_SHARES = { light: [0.5, 0.66, 0.83, 1], dark: [0.58, 0.72, 0.86, 1] } as const;

/** The raised surface charts sit on: paper mixed 38 % with white in a light scheme, `sheet` in a dark one (tokens.css `--raised`). */
export function raisedOf(p: Palette): Oklch {
  return p.scheme === 'light' ? mixWith(p.paper, 0.38, 'white') : p.sheet;
}

/** The four steps of the ordinal ramp of a palette, from the step nearest the surface to full ink. */
export function rampOf(p: Palette): Oklch[] {
  const target = p.scheme === 'light' ? 'white' : 'black';
  return RAMP_SHARES[p.scheme].map((share) => mixWith(p.ink, share, target));
}

