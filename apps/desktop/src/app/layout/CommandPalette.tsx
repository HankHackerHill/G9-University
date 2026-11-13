import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import '../theme/screens/overlays.css';

export interface PaletteCommand {
  id: string;
  label: string;
  /** Where the command lives or what kind it is, shown at the right. */
  hint: string;
  /** Extra words to match on. */
  keywords?: string;
  run: () => void;
}

/** 0 = no match. Higher is better: start of label, start of a word, substring, letters in order. */
export function matchScore(command: Pick<PaletteCommand, 'label' | 'keywords' | 'hint'>, query: string): number {
  const q = query.trim().toLowerCase();
  if (q === '') {
    return 1;
  }
  const label = command.label.toLowerCase();
  if (label.startsWith(q)) {
    return 4;
  }
  const hay = `${label} ${command.hint.toLowerCase()} ${(command.keywords ?? '').toLowerCase()}`;
  if (hay.split(/[\s/]+/).some((word) => word.startsWith(q))) {
    return 3;
  }
  if (hay.includes(q)) {
    return 2;
  }
  let at = 0;
  for (const ch of q) {
    at = label.indexOf(ch, at);
    if (at === -1) {
      return 0;
    }
    at += 1;
  }
  return 1;
}

export function openDialog(dialog: HTMLDialogElement): void {
  if (dialog.open) {
    return;
  }
  // Older webviews and the test DOM may lack showModal; the dialog still opens, without the top layer.
  const showModal = (dialog as { showModal?: () => void }).showModal;
  if (showModal === undefined) {
    dialog.setAttribute('open', '');
  } else {
    showModal.call(dialog);
  }
}

/**
 * Search and commands (Ctrl K). A combobox over a listbox: typing filters, the arrow keys move
 * the selection, Enter runs it. Mounted only while open, so every opening starts clean.
 */
export function CommandPalette({ commands, onClose }: { commands: readonly PaletteCommand[]; onClose: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [query, setQuery] = useState('');
  const [cursor, setCursor] = useState(0);
  const listId = useId();

  useEffect(() => {
    const d = dialog.current;
    if (d === null) {
      return;
    }
    openDialog(d);
    const onBackdrop = (e: MouseEvent) => {
      if (e.target === d) {
        onClose();
      }
    };
    d.addEventListener('mousedown', onBackdrop);
    return () => {
      d.removeEventListener('mousedown', onBackdrop);
    };
  }, [onClose]);

  const results = useMemo(
    () =>
      commands
        .map((command, order) => ({ command, order, score: matchScore(command, query) }))
        .filter((r) => r.score > 0)
        .sort((a, b) => b.score - a.score || a.order - b.order)
        .map((r) => r.command),
    [commands, query],
  );
  const index = Math.min(cursor, Math.max(0, results.length - 1));
  const active = results[index];

  const run = (command: PaletteCommand) => {
    onClose();
    command.run();
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      setCursor(results.length === 0 ? 0 : (index + 1) % results.length);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setCursor(results.length === 0 ? 0 : (index - 1 + results.length) % results.length);
    } else if (e.key === 'Home') {
      e.preventDefault();
      setCursor(0);
    } else if (e.key === 'End') {
      e.preventDefault();
      setCursor(Math.max(0, results.length - 1));
    } else if (e.key === 'Enter' && active !== undefined) {
      e.preventDefault();
      run(active);
    }
  };

  return (
    <dialog className="palette" ref={dialog} aria-label="Search and commands" onClose={onClose}>
      <input
        type="text"
        role="combobox"
        aria-label="Search screens and commands"
        aria-expanded="true"
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={active === undefined ? undefined : `${listId}-${active.id}`}
        autoComplete="off"
        spellCheck={false}
        placeholder="Search screens and commands…"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setCursor(0);
        }}
        onKeyDown={onKeyDown}
      />
      {results.length === 0 ? (
        <p className="palette-none" role="status">
          Nothing matches “{query}”. Try a screen name such as “session” or “calendar”.
        </p>
      ) : (
        <div role="listbox" id={listId} aria-label="Results">
          {results.map((command, i) => (
            <div
              key={command.id}
              id={`${listId}-${command.id}`}
              role="option"
              aria-selected={i === index}
              tabIndex={-1}
              onMouseMove={() => {
                setCursor(i);
              }}
              onClick={() => {
                run(command);
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  run(command);
                }
              }}
            >
              <span>{command.label}</span>
              <span className="palette-hint">{command.hint}</span>
            </div>
          ))}
        </div>
      )}
      <p className="palette-foot" aria-hidden="true">
        <span>
          <kbd>↑</kbd> <kbd>↓</kbd> move
        </span>
        <span>
          <kbd>Enter</kbd> choose
        </span>
        <span>
          <kbd>Esc</kbd> close
        </span>
      </p>
    </dialog>
  );
}
