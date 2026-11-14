import { useEffect, useState } from 'react';
import { loadLessonSheets, type LessonSheets } from '../../lib/lesson-sheets';

export interface LessonSheetsState {
  /** Null until the sheets are read, and when they could not be. */
  sheets: LessonSheets | null;
  /** True when the sheets that ship with the application could not be loaded. */
  failed: boolean;
}

/** Reads the lesson sheets once `enabled` (a day with blocks is on screen). */
export function useLessonSheets(enabled: boolean): LessonSheetsState {
  const [state, setState] = useState<LessonSheetsState>({ sheets: null, failed: false });
  const have = state.sheets !== null;
  useEffect(() => {
    if (!enabled || have) {
      return undefined;
    }
    let live = true;
    loadLessonSheets().then(
      (sheets) => {
        if (live) {
          setState({ sheets, failed: false });
        }
      },
      () => {
        if (live) {
          setState({ sheets: null, failed: true });
        }
      },
    );
    return () => {
      live = false;
    };
  }, [enabled, have]);
  return state;
}
