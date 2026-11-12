/**
 * Contrast preference persisted in localStorage and applied as `data-contrast` on <html>.
 * `high` puts pure ink on pure paper and strengthens every rule and standing colour; patterns
 * and labels are unchanged (tokens.css).
 */
import { useCallback, useEffect } from 'react';
import { parsePreference, usePreference } from './preferences';

export type ContrastMode = 'default' | 'high';
export const CONTRAST_STORAGE_KEY = 'g9.contrast';
export const CONTRAST_MODES: readonly ContrastMode[] = ['default', 'high'];

export function readStoredContrast(storage: Pick<Storage, 'getItem'> | null): ContrastMode {
  try {
    return parsePreference(storage?.getItem(CONTRAST_STORAGE_KEY) ?? null, CONTRAST_MODES, 'default');
  } catch {
    return 'default';
  }
}

export function applyContrast(mode: ContrastMode, root: HTMLElement | null = typeof document === 'undefined' ? null : document.documentElement): void {
  if (root !== null) {
    root.dataset.contrast = mode;
  }
}

export function useContrast(): [ContrastMode, () => void] {
  const [mode, setMode] = usePreference<ContrastMode>(CONTRAST_STORAGE_KEY, CONTRAST_MODES, 'default');

  useEffect(() => {
    applyContrast(mode);
  }, [mode]);

  const toggle = useCallback(() => {
    setMode(mode === 'high' ? 'default' : 'high');
  }, [mode, setMode]);

  return [mode, toggle];
}
