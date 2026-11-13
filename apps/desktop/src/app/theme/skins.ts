/**
 * The themes. Ledger is the house style (tokens.css) and follows the light or dark setting;
 * every other theme has one scheme. A theme changes colour, type and the drawing of regions; it
 * never changes the grammar: posted is solid, pencilled is outlined, in doubt is hatched, advice
 * is dashed in the mentor voice, owed is red, and every mark keeps its name in words.
 *
 * Every palette is checked by `skins.test.ts` on every run: text at least 4.5:1 on all three
 * surfaces and on the standing fills, control borders at least 3:1, every colour inside sRGB.
 *
 * Applying a theme sets inline custom properties on <html> through the CSSOM, which the content
 * security policy (`style-src 'self'`) permits; no <style> element or style attribute is written
 * into markup. High contrast keeps the theme's type and shapes but replaces its colours.
 */
import type { CSSProperties } from 'react';
import { cssOklch, FILL_ALPHA, type Palette } from './contrast';

export type SkinId = 'ledger' | 'clean' | 'professional' | 'modern-tech' | 'graphite' | 'midnight' | 'fjord' | 'deep-sea' | 'sakura' | 'notebook' | 'typewriter' | 'archive' | 'e-ink' | 'blueprint' | 'chalkboard' | 'hacker' | 'amber-crt' | 'synthwave' | 'cyberpunk' | 'classic-desktop' | 'pocket-console';
export type FontRole = 'sans' | 'serif' | 'mono';
export type SkinGroup = 'house' | 'studio' | 'paper' | 'retro';

export interface SkinType {
  body: FontRole;
  heading: FontRole;
  /** Recursive CASL axis, 0 (linear) to 1 (casual). */
  bodyCasual: number;
  headingCasual: number;
  headingWeight: number;
  /** Recursive slnt axis, 0 to -15. */
  headingSlant?: number;
  bodyWeight?: number;
}

export interface Skin {
  id: SkinId;
  name: string;
  description: string;
  group: SkinGroup;
  /** Ledger has both; every other theme has exactly one. */
  palettes: { light?: Palette; dark?: Palette };
  type: SkinType;
  radius: 'none' | 'small' | 'default' | 'soft';
  /** How a region of a page is drawn: a rule above it, a card, or a window with a title bar. */
  regions: 'ruled' | 'boxed' | 'window';
  /** One texture or treatment per theme (skins.css); none of them survives high contrast. */
  effect: 'none' | 'glow' | 'scanlines' | 'neon' | 'clipped' | 'bevel' | 'pixel' | 'grid' | 'notebook' | 'chalk' | 'still';
}

export const SKINS: readonly Skin[] = [
  {
    id: 'ledger',
    name: 'Ledger',
    description: 'Eye-ease ledger paper, blue-black ink, one ballpoint blue. The house style; follows the light or dark setting.',
    group: 'house',
    palettes: {
      light: {
        scheme: 'light',
        paper: [0.966, 0.012, 150],
        sheet: [0.94, 0.02, 152],
        sheet2: [0.908, 0.03, 152],
        rule: [0.85, 0.028, 155],
        rule2: [0.6, 0.035, 190],
        ink: [0.235, 0.04, 258],
        ink2: [0.39, 0.04, 255],
        ink3: [0.48, 0.035, 250],
        owed: [0.5, 0.19, 28],
        doubt: [0.5, 0.112, 68],
        advice: [0.45, 0.17, 302],
        live: [0.47, 0.105, 160],
        accent: [0.44, 0.17, 262],
        doubtFill: [0.935, 0.07, 95],
      },
      dark: {
        scheme: 'dark',
        paper: [0.215, 0.022, 258],
        sheet: [0.255, 0.026, 258],
        sheet2: [0.295, 0.03, 258],
        rule: [0.36, 0.028, 258],
        rule2: [0.56, 0.03, 250],
        ink: [0.945, 0.02, 150],
        ink2: [0.83, 0.025, 170],
        ink3: [0.73, 0.03, 200],
        owed: [0.76, 0.143, 28],
        doubt: [0.84, 0.13, 82],
        advice: [0.8, 0.11, 302],
        live: [0.86, 0.15, 158],
        accent: [0.8, 0.1, 258],
      },
    },
    type: { body: 'sans', heading: 'sans', bodyCasual: 0, headingCasual: 0, headingWeight: 860 },
    radius: 'default',
    regions: 'ruled',
    effect: 'none',
  },
  {
    id: 'clean',
    name: 'Clean Crisp',
    description: 'White paper, black ink, one blue. Nothing else.',
    group: 'studio',
    palettes: {
      light: {
        scheme: 'light',
        paper: [1, 0, 0],
        sheet: [0.975, 0.002, 250],
        sheet2: [0.955, 0.004, 250],
        rule: [0.905, 0.004, 250],
        rule2: [0.62, 0.01, 250],
        ink: [0.18, 0.005, 260],
        ink2: [0.4, 0.01, 260],
        ink3: [0.5, 0.01, 260],
        owed: [0.53, 0.2, 27],
        doubt: [0.52, 0.112, 70],
        advice: [0.5, 0.2, 290],
        live: [0.52, 0.128, 155],
        accent: [0.53, 0.2, 260],
        doubtFill: [0.96, 0.06, 95],
      },
    },
    type: { body: 'sans', heading: 'sans', bodyCasual: 0, headingCasual: 0, headingWeight: 680 },
    radius: 'default',
    regions: 'ruled',
    effect: 'none',
  },
  {
    id: 'professional',
    name: 'Professional',
    description: 'Slate and navy cards with report-style serif headings.',
    group: 'studio',
    palettes: {
      light: {
        scheme: 'light',
        paper: [0.98, 0.004, 240],
        sheet: [0.96, 0.008, 240],
        sheet2: [0.935, 0.012, 240],
        rule: [0.87, 0.012, 240],
        rule2: [0.6, 0.02, 245],
        ink: [0.26, 0.035, 258],
        ink2: [0.4, 0.03, 255],
        ink3: [0.48, 0.025, 255],
        owed: [0.5, 0.18, 25],
        doubt: [0.5, 0.108, 70],
        advice: [0.46, 0.15, 290],
        live: [0.48, 0.108, 160],
        accent: [0.4, 0.13, 262],
        doubtFill: [0.95, 0.055, 90],
        field: [1, 0, 0],
      },
    },
    type: { body: 'sans', heading: 'serif', bodyCasual: 0, headingCasual: 0, headingWeight: 700 },
    radius: 'small',
    regions: 'boxed',
    effect: 'none',
  },
  {
    id: 'modern-tech',
    name: 'Modern Tech',
    description: 'Deep navy, a cyan signal and soft cards.',
    group: 'studio',
    palettes: {
      dark: {
        scheme: 'dark',
        paper: [0.2, 0.035, 262],
        sheet: [0.24, 0.04, 262],
        sheet2: [0.28, 0.045, 262],
        rule: [0.35, 0.04, 262],
        rule2: [0.56, 0.06, 258],
        ink: [0.96, 0.012, 255],
        ink2: [0.84, 0.03, 255],
        ink3: [0.74, 0.04, 255],
        owed: [0.73, 0.166, 22],
        doubt: [0.85, 0.13, 82],
        advice: [0.78, 0.125, 295],
        live: [0.86, 0.15, 160],
        accent: [0.83, 0.12, 215],
      },
    },
    type: { body: 'sans', heading: 'sans', bodyCasual: 0, headingCasual: 0, headingWeight: 720 },
    radius: 'soft',
    regions: 'boxed',
    effect: 'none',
  },
  {
    id: 'graphite',
    name: 'Graphite',
    description: 'Monochrome charcoal. Colour only where it carries meaning.',
    group: 'studio',
    palettes: {
      dark: {
        scheme: 'dark',
        paper: [0.22, 0, 0],
        sheet: [0.255, 0, 0],
        sheet2: [0.29, 0, 0],
        rule: [0.36, 0, 0],
        rule2: [0.57, 0, 0],
        ink: [0.95, 0, 0],
        ink2: [0.83, 0, 0],
        ink3: [0.72, 0, 0],
        owed: [0.74, 0.15, 25],
        doubt: [0.85, 0.12, 85],
        advice: [0.8, 0.09, 290],
        live: [0.86, 0.13, 160],
      },
    },
    type: { body: 'sans', heading: 'sans', bodyCasual: 0, headingCasual: 0, headingWeight: 740 },
    radius: 'small',
    regions: 'ruled',
    effect: 'none',
  },
  {
    id: 'midnight',
    name: 'Midnight',
    description: 'Blue-black and lavender, easy on the eyes late at night.',
    group: 'studio',
    palettes: {
      dark: {
        scheme: 'dark',
        paper: [0.17, 0.035, 275],
        sheet: [0.21, 0.04, 275],
        sheet2: [0.25, 0.045, 275],
        rule: [0.32, 0.045, 275],
        rule2: [0.54, 0.06, 280],
        ink: [0.93, 0.03, 285],
        ink2: [0.82, 0.05, 285],
        ink3: [0.72, 0.06, 285],
        owed: [0.73, 0.15, 15],
        doubt: [0.85, 0.11, 80],
        advice: [0.8, 0.1, 220],
        live: [0.84, 0.12, 165],
        accent: [0.8, 0.1, 300],
      },
    },
    type: { body: 'sans', heading: 'sans', bodyCasual: 0, headingCasual: 0, headingWeight: 700 },
    radius: 'soft',
    regions: 'boxed',
    effect: 'none',
  },
  {
    id: 'fjord',
    name: 'Fjord',
    description: 'Arctic slate and glacier blue.',
    group: 'studio',
    palettes: {
      dark: {
        scheme: 'dark',
        paper: [0.3, 0.022, 255],
        sheet: [0.335, 0.024, 255],
        sheet2: [0.37, 0.026, 255],
        rule: [0.43, 0.027, 255],
        rule2: [0.63, 0.03, 250],
        ink: [0.95, 0.01, 250],
        ink2: [0.87, 0.02, 250],
        ink3: [0.8, 0.03, 245],
        owed: [0.79, 0.12, 20],
        doubt: [0.88, 0.1, 88],
        advice: [0.84, 0.07, 305],
        live: [0.86, 0.09, 190],
        accent: [0.82, 0.08, 220],
      },
    },
    type: { body: 'sans', heading: 'sans', bodyCasual: 0, headingCasual: 0, headingWeight: 720 },
    radius: 'default',
    regions: 'boxed',
    effect: 'none',
  },
  {
    id: 'deep-sea',
    name: 'Deep Sea',
    description: 'Dark teal water with aqua highlights.',
    group: 'studio',
    palettes: {
      dark: {
        scheme: 'dark',
        paper: [0.22, 0.04, 220],
        sheet: [0.26, 0.045, 220],
        sheet2: [0.3, 0.05, 220],
        rule: [0.37, 0.05, 218],
        rule2: [0.58, 0.06, 215],
        ink: [0.95, 0.02, 200],
        ink2: [0.84, 0.04, 205],
        ink3: [0.75, 0.05, 205],
        owed: [0.75, 0.15, 25],
        doubt: [0.86, 0.12, 85],
        advice: [0.8, 0.1, 300],
        live: [0.86, 0.12, 165],
        accent: [0.84, 0.11, 185],
      },
    },
    type: { body: 'sans', heading: 'sans', bodyCasual: 0, headingCasual: 0, headingWeight: 720 },
    radius: 'soft',
    regions: 'boxed',
    effect: 'none',
  },
  {
    id: 'sakura',
    name: 'Sakura',
    description: 'Blossom-pink paper and plum ink.',
    group: 'studio',
    palettes: {
      light: {
        scheme: 'light',
        paper: [0.975, 0.012, 350],
        sheet: [0.95, 0.022, 350],
        sheet2: [0.925, 0.03, 350],
        rule: [0.86, 0.035, 350],
        rule2: [0.6, 0.05, 350],
        ink: [0.28, 0.05, 340],
        ink2: [0.42, 0.05, 340],
        ink3: [0.49, 0.045, 340],
        owed: [0.5, 0.18, 25],
        doubt: [0.51, 0.1, 70],
        advice: [0.47, 0.14, 290],
        live: [0.48, 0.1, 165],
        accent: [0.52, 0.17, 355],
        doubtFill: [0.95, 0.06, 92],
      },
    },
    type: { body: 'sans', heading: 'sans', bodyCasual: 0.2, headingCasual: 0.5, headingWeight: 760 },
    radius: 'soft',
    regions: 'boxed',
    effect: 'none',
  },
  {
    id: 'notebook',
    name: 'Notebook',
    description: 'Squared school paper, blue pen and a red margin.',
    group: 'paper',
    palettes: {
      light: {
        scheme: 'light',
        paper: [0.99, 0.004, 240],
        sheet: [0.965, 0.012, 240],
        sheet2: [0.94, 0.02, 240],
        rule: [0.86, 0.04, 240],
        rule2: [0.6, 0.06, 245],
        ink: [0.27, 0.07, 265],
        ink2: [0.4, 0.06, 265],
        ink3: [0.48, 0.05, 265],
        owed: [0.5, 0.19, 27],
        doubt: [0.5, 0.108, 70],
        advice: [0.47, 0.17, 305],
        live: [0.48, 0.108, 160],
        accent: [0.45, 0.15, 265],
        doubtFill: [0.955, 0.09, 105],
      },
    },
    type: { body: 'sans', heading: 'sans', bodyCasual: 0.3, headingCasual: 0.8, headingWeight: 760 },
    radius: 'default',
    regions: 'ruled',
    effect: 'notebook',
  },
  {
    id: 'typewriter',
    name: 'Typewriter',
    description: 'Carbon ribbon on bond paper, red ribbon for what is owed.',
    group: 'paper',
    palettes: {
      light: {
        scheme: 'light',
        paper: [0.965, 0.01, 88],
        sheet: [0.94, 0.013, 85],
        sheet2: [0.915, 0.016, 82],
        rule: [0.84, 0.016, 80],
        rule2: [0.58, 0.015, 75],
        ink: [0.2, 0.008, 60],
        ink2: [0.36, 0.008, 60],
        ink3: [0.46, 0.008, 60],
        owed: [0.48, 0.18, 28],
        doubt: [0.5, 0.1, 75],
        advice: [0.45, 0.12, 290],
        live: [0.47, 0.1, 160],
        doubtFill: [0.93, 0.06, 90],
      },
    },
    type: { body: 'mono', heading: 'mono', bodyCasual: 0, headingCasual: 0, headingWeight: 700 },
    radius: 'none',
    regions: 'ruled',
    effect: 'none',
  },
  {
    id: 'archive',
    name: 'Archive',
    description: 'Sepia card stock and a reading serif for long notes.',
    group: 'paper',
    palettes: {
      light: {
        scheme: 'light',
        paper: [0.94, 0.03, 80],
        sheet: [0.915, 0.034, 78],
        sheet2: [0.89, 0.038, 76],
        rule: [0.8, 0.04, 72],
        rule2: [0.56, 0.05, 65],
        ink: [0.3, 0.04, 55],
        ink2: [0.4, 0.04, 55],
        ink3: [0.46, 0.04, 55],
        owed: [0.46, 0.15, 28],
        doubt: [0.47, 0.099, 75],
        advice: [0.43, 0.1, 300],
        live: [0.45, 0.09, 165],
      },
    },
    type: { body: 'serif', heading: 'serif', bodyCasual: 0, headingCasual: 0, headingWeight: 700 },
    radius: 'small',
    regions: 'ruled',
    effect: 'none',
  },
  {
    id: 'e-ink',
    name: 'E-ink',
    description: 'Grey paper and black type, with no motion and no colour you do not need.',
    group: 'paper',
    palettes: {
      light: {
        scheme: 'light',
        paper: [0.92, 0, 0],
        sheet: [0.895, 0, 0],
        sheet2: [0.87, 0, 0],
        rule: [0.78, 0, 0],
        rule2: [0.5, 0, 0],
        ink: [0.12, 0, 0],
        ink2: [0.28, 0, 0],
        ink3: [0.38, 0, 0],
        owed: [0.42, 0.12, 25],
        doubt: [0.44, 0.07, 75],
        advice: [0.4, 0.09, 290],
        live: [0.42, 0.07, 160],
      },
    },
    type: { body: 'serif', heading: 'serif', bodyCasual: 0, headingCasual: 0, headingWeight: 700 },
    radius: 'none',
    regions: 'ruled',
    effect: 'still',
  },
  {
    id: 'blueprint',
    name: 'Blueprint',
    description: 'White lines on drafting blue, headings in the machine voice.',
    group: 'paper',
    palettes: {
      dark: {
        scheme: 'dark',
        paper: [0.36, 0.11, 258],
        sheet: [0.4, 0.11, 258],
        sheet2: [0.44, 0.105, 258],
        rule: [0.52, 0.09, 255],
        rule2: [0.74, 0.06, 250],
        ink: [0.985, 0.007, 250],
        ink2: [0.93, 0.025, 250],
        ink3: [0.88, 0.035, 250],
        owed: [0.88, 0.063, 25],
        doubt: [0.93, 0.1, 95],
        advice: [0.9, 0.05, 300],
        live: [0.92, 0.08, 175],
      },
    },
    type: { body: 'sans', heading: 'mono', bodyCasual: 0, headingCasual: 0, headingWeight: 760 },
    radius: 'none',
    regions: 'ruled',
    effect: 'grid',
  },
  {
    id: 'chalkboard',
    name: 'Chalkboard',
    description: 'Slate-green board and chalk lettering.',
    group: 'paper',
    palettes: {
      dark: {
        scheme: 'dark',
        paper: [0.31, 0.035, 165],
        sheet: [0.35, 0.037, 165],
        sheet2: [0.39, 0.04, 165],
        rule: [0.46, 0.035, 165],
        rule2: [0.66, 0.03, 160],
        ink: [0.96, 0.012, 100],
        ink2: [0.89, 0.02, 100],
        ink3: [0.83, 0.03, 115],
        owed: [0.83, 0.094, 20],
        doubt: [0.92, 0.11, 95],
        advice: [0.86, 0.07, 285],
        live: [0.89, 0.09, 200],
      },
    },
    type: { body: 'sans', heading: 'sans', bodyCasual: 0.4, headingCasual: 1, headingWeight: 720 },
    radius: 'small',
    regions: 'ruled',
    effect: 'chalk',
  },
  {
    id: 'hacker',
    name: 'Hacker',
    description: 'Phosphor green on black, every character in the machine voice.',
    group: 'retro',
    palettes: {
      dark: {
        scheme: 'dark',
        paper: [0.14, 0.012, 150],
        sheet: [0.18, 0.016, 150],
        sheet2: [0.22, 0.02, 150],
        rule: [0.3, 0.04, 150],
        rule2: [0.52, 0.1, 150],
        ink: [0.9, 0.186, 148],
        ink2: [0.8, 0.15, 148],
        ink3: [0.7, 0.12, 148],
        owed: [0.72, 0.174, 28],
        doubt: [0.88, 0.15, 100],
        advice: [0.8, 0.11, 220],
        live: [0.92, 0.1, 190],
        accent: [0.88, 0.23, 144],
      },
    },
    type: { body: 'mono', heading: 'mono', bodyCasual: 0, headingCasual: 0, headingWeight: 820 },
    radius: 'none',
    regions: 'ruled',
    effect: 'glow',
  },
  {
    id: 'amber-crt',
    name: 'Amber Terminal',
    description: 'A 1980s monochrome monitor: amber phosphor and scan lines.',
    group: 'retro',
    palettes: {
      dark: {
        scheme: 'dark',
        paper: [0.16, 0.02, 65],
        sheet: [0.2, 0.025, 65],
        sheet2: [0.24, 0.03, 65],
        rule: [0.32, 0.05, 65],
        rule2: [0.55, 0.09, 70],
        ink: [0.81, 0.16, 75],
        ink2: [0.75, 0.14, 72],
        ink3: [0.69, 0.12, 70],
        owed: [0.72, 0.17, 32],
        doubt: [0.92, 0.11, 98],
        advice: [0.82, 0.08, 300],
        live: [0.88, 0.1, 160],
        accent: [0.84, 0.15, 80],
      },
    },
    type: { body: 'mono', heading: 'mono', bodyCasual: 0, headingCasual: 0, headingWeight: 760 },
    radius: 'none',
    regions: 'ruled',
    effect: 'scanlines',
  },
  {
    id: 'synthwave',
    name: 'Synthwave',
    description: 'Neon pink and cyan on midnight purple, straight out of 1986.',
    group: 'retro',
    palettes: {
      dark: {
        scheme: 'dark',
        paper: [0.19, 0.07, 300],
        sheet: [0.23, 0.08, 300],
        sheet2: [0.27, 0.09, 300],
        rule: [0.35, 0.09, 305],
        rule2: [0.58, 0.13, 320],
        ink: [0.95, 0.025, 320],
        ink2: [0.85, 0.06, 325],
        ink3: [0.76, 0.08, 320],
        owed: [0.74, 0.159, 18],
        doubt: [0.87, 0.142, 85],
        advice: [0.82, 0.11, 215],
        live: [0.86, 0.15, 175],
        accent: [0.72, 0.199, 355],
      },
    },
    type: { body: 'sans', heading: 'sans', bodyCasual: 0, headingCasual: 0, headingWeight: 920, headingSlant: -10 },
    radius: 'soft',
    regions: 'boxed',
    effect: 'neon',
  },
  {
    id: 'cyberpunk',
    name: 'Cyberpunk',
    description: 'Acid yellow and hot magenta on black, with clipped corners.',
    group: 'retro',
    palettes: {
      dark: {
        scheme: 'dark',
        paper: [0.15, 0.01, 280],
        sheet: [0.19, 0.012, 280],
        sheet2: [0.23, 0.015, 280],
        rule: [0.31, 0.02, 280],
        rule2: [0.55, 0.03, 280],
        ink: [0.95, 0.03, 105],
        ink2: [0.85, 0.04, 105],
        ink3: [0.75, 0.05, 105],
        owed: [0.72, 0.174, 25],
        doubt: [0.8, 0.14, 60],
        advice: [0.74, 0.201, 345],
        live: [0.86, 0.14, 200],
        accent: [0.92, 0.198, 108],
      },
    },
    type: { body: 'sans', heading: 'mono', bodyCasual: 0, headingCasual: 0, headingWeight: 900 },
    radius: 'none',
    regions: 'boxed',
    effect: 'clipped',
  },
  {
    id: 'classic-desktop',
    name: 'Classic Desktop',
    description: 'Grey bevels and navy title bars, like a 1995 desktop.',
    group: 'retro',
    palettes: {
      light: {
        scheme: 'light',
        paper: [0.808, 0, 0],
        sheet: [0.85, 0, 0],
        sheet2: [0.88, 0, 0],
        rule: [0.6, 0, 0],
        rule2: [0.42, 0, 0],
        ink: [0, 0, 0],
        ink2: [0.25, 0, 0],
        ink3: [0.32, 0, 0],
        owed: [0.38, 0.15, 28],
        doubt: [0.4, 0.084, 75],
        advice: [0.35, 0.17, 285],
        live: [0.38, 0.097, 155],
        field: [1, 0, 0],
        bar: [0.2737, 0.1855, 264.05],
        onBar: [1, 0, 0],
      },
    },
    type: { body: 'sans', heading: 'sans', bodyCasual: 0, headingCasual: 0, headingWeight: 700 },
    radius: 'none',
    regions: 'window',
    effect: 'bevel',
  },
  {
    id: 'pocket-console',
    name: 'Pocket Console',
    description: 'Four shades of olive, like a 1989 handheld game.',
    group: 'retro',
    palettes: {
      light: {
        scheme: 'light',
        paper: [0.84, 0.13, 122],
        sheet: [0.8, 0.135, 124],
        sheet2: [0.76, 0.14, 126],
        rule: [0.55, 0.1, 140],
        rule2: [0.42, 0.09, 145],
        ink: [0.27, 0.075, 148],
        ink2: [0.32, 0.08, 148],
        ink3: [0.36, 0.085, 146],
        owed: [0.38, 0.12, 30],
        doubt: [0.38, 0.077, 90],
        advice: [0.34, 0.1, 300],
        live: [0.36, 0.075, 165],
      },
    },
    type: { body: 'mono', heading: 'mono', bodyCasual: 0, headingCasual: 0, headingWeight: 900, bodyWeight: 540 },
    radius: 'none',
    regions: 'boxed',
    effect: 'pixel',
  },
];

export const SKIN_IDS: readonly SkinId[] = SKINS.map((s) => s.id);
export const DEFAULT_SKIN: SkinId = 'ledger';
export const SKIN_STORAGE_KEY = 'g9.skin';

export const SKIN_GROUPS: readonly { id: SkinGroup; title: string }[] = [
  { id: 'house', title: 'House' },
  { id: 'studio', title: 'Studio' },
  { id: 'paper', title: 'Paper and craft' },
  { id: 'retro', title: 'Retro and terminal' },
];

function houseStyle(): Skin {
  const ledger = SKINS.find((s) => s.id === DEFAULT_SKIN);
  if (ledger === undefined) {
    throw new Error('the house theme is missing from SKINS');
  }
  return ledger;
}

export function skinById(id: SkinId): Skin {
  return SKINS.find((s) => s.id === id) ?? houseStyle();
}

/** The palette a theme shows: its only one, or for Ledger the one of the given scheme. */
export function paletteOf(skin: Skin, scheme: 'light' | 'dark'): Palette {
  const p = skin.palettes[scheme] ?? skin.palettes.dark ?? skin.palettes.light;
  if (p === undefined) {
    throw new Error(`theme ${skin.id} has no palette`);
  }
  return p;
}

/** The scheme a single-scheme theme forces, or null for a theme that follows the setting. */
export function fixedScheme(skin: Skin): 'light' | 'dark' | null {
  if (skin.palettes.light !== undefined && skin.palettes.dark !== undefined) {
    return null;
  }
  return skin.palettes.dark !== undefined ? 'dark' : 'light';
}

const FONT: Record<FontRole, string> = { sans: 'var(--font-sans)', serif: 'var(--font-serif)', mono: 'var(--font-sans)' };
const RADII: Record<Skin['radius'], readonly [string, string, string, string]> = {
  none: ['0px', '0px', '0px', '0px'],
  small: ['2px', '3px', '4px', '6px'],
  default: ['3px', '7px', '12px', '18px'],
  soft: ['4px', '9px', '16px', '22px'],
};

/** Custom properties that carry colour: these give way to high contrast. */
export function colourProperties(p: Palette): Record<string, string> {
  return {
    '--paper': cssOklch(p.paper),
    '--sheet': cssOklch(p.sheet),
    '--sheet-2': cssOklch(p.sheet2),
    '--rule': cssOklch(p.rule),
    '--rule-2': cssOklch(p.rule2),
    '--ink': cssOklch(p.ink),
    '--ink-2': cssOklch(p.ink2),
    '--ink-3': cssOklch(p.ink3),
    '--owed': cssOklch(p.owed),
    '--doubt': cssOklch(p.doubt),
    '--advice': cssOklch(p.advice),
    '--live': cssOklch(p.live),
    '--accent': cssOklch(p.accent ?? p.ink),
    '--field': cssOklch(p.field ?? p.paper),
    '--bar': cssOklch(p.bar ?? p.ink),
    '--on-bar': cssOklch(p.onBar ?? p.paper),
    '--doubt-fill': p.doubtFill === undefined ? `color-mix(in oklch, var(--doubt) ${FILL_ALPHA.doubt * 100}%, transparent)` : cssOklch(p.doubtFill),
  };
}

/** Custom properties that carry type and shape: these stay under high contrast. */
export function typeProperties(skin: Skin): Record<string, string> {
  const t = skin.type;
  const [r1, r2, r3, r4] = RADII[skin.radius];
  return {
    '--font': FONT[t.body],
    '--font-heading': FONT[t.heading],
    '--mono': t.body === 'mono' ? '1' : '0',
    '--casl': String(t.bodyCasual),
    '--h-mono': t.heading === 'mono' ? '1' : '0',
    '--h-casl': String(t.headingCasual),
    '--h-slnt': String(t.headingSlant ?? 0),
    '--h-weight': String(t.headingWeight),
    '--body-weight': String(t.bodyWeight ?? 420),
    '--r-1': r1,
    '--r-2': r2,
    '--r-3': r3,
    '--r-4': r4,
  };
}

const ALL_PROPERTIES = [...Object.keys(colourProperties(paletteOf(houseStyle(), 'light'))), ...Object.keys(typeProperties(houseStyle()))];

/**
 * Applies a theme to <html>. Ledger clears every inline property so tokens.css (with its
 * light-dark pairs, the scheme setting and the high-contrast block) decides. Any other theme
 * pins `color-scheme` and sets its type; its colours are set only when high contrast is off.
 */
export function applySkin(skin: Skin, contrast: 'default' | 'high', root: HTMLElement | null = typeof document === 'undefined' ? null : document.documentElement): void {
  if (root === null) {
    return;
  }
  for (const property of ALL_PROPERTIES) {
    root.style.removeProperty(property);
  }
  root.style.removeProperty('color-scheme');
  root.dataset.skin = skin.id;
  root.dataset.regions = skin.regions;
  root.dataset.effect = skin.effect;
  const scheme = fixedScheme(skin);
  if (scheme === null) {
    return;
  }
  root.style.setProperty('color-scheme', scheme);
  const properties = contrast === 'high' ? typeProperties(skin) : { ...typeProperties(skin), ...colourProperties(paletteOf(skin, scheme)) };
  for (const [property, value] of Object.entries(properties)) {
    root.style.setProperty(property, value);
  }
}

/** Inline custom properties that draw a preview of a theme inside a `.skin-scope` element. */
export function previewStyle(skin: Skin, scheme: 'light' | 'dark'): CSSProperties {
  const p = paletteOf(skin, scheme);
  return { colorScheme: p.scheme, ...typeProperties(skin), ...colourProperties(p) };
}
