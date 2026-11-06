import { useId, useState, type KeyboardEvent, type ReactNode } from 'react';

export interface TabDef {
  id: string;
  label: string;
  content: ReactNode;
}

/** Minimal WAI-ARIA tabs: arrow keys move between tabs, Home/End jump. */
export function Tabs({ tabs, label }: { tabs: readonly TabDef[]; label: string }) {
  const [activeId, setActiveId] = useState(tabs[0]?.id ?? '');
  const base = useId();
  const index = Math.max(0, tabs.findIndex((t) => t.id === activeId));

  const onKeyDown = (e: KeyboardEvent<HTMLButtonElement>) => {
    let next = index;
    if (e.key === 'ArrowRight') {
      next = (index + 1) % tabs.length;
    } else if (e.key === 'ArrowLeft') {
      next = (index - 1 + tabs.length) % tabs.length;
    } else if (e.key === 'Home') {
      next = 0;
    } else if (e.key === 'End') {
      next = tabs.length - 1;
    } else {
      return;
    }
    e.preventDefault();
    const tab = tabs[next];
    if (tab !== undefined) {
      setActiveId(tab.id);
      document.getElementById(`${base}-tab-${tab.id}`)?.focus();
    }
  };

  return (
    <div>
      <div role="tablist" aria-label={label}>
        {tabs.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            id={`${base}-tab-${t.id}`}
            aria-selected={t.id === activeId}
            aria-controls={`${base}-panel-${t.id}`}
            tabIndex={t.id === activeId ? 0 : -1}
            onKeyDown={onKeyDown}
            onClick={() => {
              setActiveId(t.id);
            }}
          >
            {t.label}
          </button>
        ))}
      </div>
      {tabs.map((t) => (
        <div key={t.id} role="tabpanel" id={`${base}-panel-${t.id}`} aria-labelledby={`${base}-tab-${t.id}`} hidden={t.id !== activeId}>
          {t.content}
        </div>
      ))}
    </div>
  );
}
