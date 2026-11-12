/**
 * Colour scheme preference. `system` follows the desktop (no attribute, `color-scheme: dark light`);
 * `light` and `dark` set `data-theme` on <html>, which pins `color-scheme` and therefore every
 * `light-dark()` token (tokens.css).
 */
import { useEffect } from 'react';
import { parsePreference, usePreference } from './preferences';

export type ThemeMode = 'system' | 'light' | 'dark';
export const THEME_STORAGE_KEY = 'g9.theme';
export const THEME_MODES: readonly ThemeMode[] = ['system', 'light', 'dark'];

export function readStoredTheme(storage: Pick<Storage, 'getItem'> | null): ThemeMode {
  try {
    return parsePreference(storage?.getItem(THEME_STORAGE_KEY) ?? null, THEME_MODES, 'system');
  } catch {
    return 'system';
  }
}

export function applyTheme(mode: ThemeMode, root: HTMLElement | null = typeof document === 'undefined' ? null : document.documentElement): void {
  if (root === null) {
    return;
  }
  if (mode === 'system') {
    delete root.dataset.theme;
  } else {
    root.dataset.theme = mode;
  }
}

export function useTheme(): [ThemeMode, (next: ThemeMode) => void] {
  const [mode, setMode] = usePreference<ThemeMode>(THEME_STORAGE_KEY, THEME_MODES, 'system');

  useEffect(() => {
    applyTheme(mode);
  }, [mode]);

  return [mode, setMode];
}
