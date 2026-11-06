/**
 * Contrast preference persisted in localStorage and applied as `data-contrast` on <html>.
 * Default theme: dark cinematic with restrained gold/cyan/green; `high` raises every pair
 * above WCAG 2.2 AA with pure black/white surfaces.
 */
import { useCallback, useEffect, useState } from 'react';

export type ContrastMode = 'default' | 'high';
export const CONTRAST_STORAGE_KEY = 'g9.contrast';

export function readStoredContrast(storage: Pick<Storage, 'getItem'> | null): ContrastMode {
  try {
    return storage?.getItem(CONTRAST_STORAGE_KEY) === 'high' ? 'high' : 'default';
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
  const [mode, setMode] = useState<ContrastMode>(() => readStoredContrast(typeof localStorage === 'undefined' ? null : localStorage));

  useEffect(() => {
    applyContrast(mode);
    try {
      localStorage.setItem(CONTRAST_STORAGE_KEY, mode);
    } catch {
      // Storage may be unavailable (private mode); the in-memory preference still applies.
    }
  }, [mode]);

  const toggle = useCallback(() => {
    setMode((m) => (m === 'high' ? 'default' : 'high'));
  }, []);

  return [mode, toggle];
}
