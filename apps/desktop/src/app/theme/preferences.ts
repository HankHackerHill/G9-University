/**
 * Appearance preferences: kept in localStorage, read through one external store so every
 * control that shows a preference (the shell tools, Settings) stays in step. When storage is
 * unavailable the choice is held in memory for the life of the window.
 */
import { useCallback, useSyncExternalStore } from 'react';

const listeners = new Set<() => void>();
const memory = new Map<string, string>();

function readRaw(key: string): string | null {
  try {
    return typeof localStorage === 'undefined' ? (memory.get(key) ?? null) : localStorage.getItem(key);
  } catch {
    return memory.get(key) ?? null;
  }
}

function writeRaw(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    memory.set(key, value);
  }
}

function subscribe(notify: () => void): () => void {
  listeners.add(notify);
  window.addEventListener('storage', notify);
  return () => {
    listeners.delete(notify);
    window.removeEventListener('storage', notify);
  };
}

export function parsePreference<T extends string>(raw: string | null, allowed: readonly T[], fallback: T): T {
  return allowed.find((a) => a === raw) ?? fallback;
}

export function readPreference<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
  return parsePreference(readRaw(key), allowed, fallback);
}

export function usePreference<T extends string>(key: string, allowed: readonly T[], fallback: T): [T, (next: T) => void] {
  const value = useSyncExternalStore(
    subscribe,
    () => readPreference(key, allowed, fallback),
    () => fallback,
  );
  const set = useCallback(
    (next: T) => {
      writeRaw(key, next);
      for (const notify of listeners) {
        notify();
      }
    },
    [key],
  );
  return [value, set];
}
