import { useEffect, useMemo, useRef } from 'react';
import { renderMarkdown } from '../../lib/markdown';

/**
 * Sanitized markdown preview. The HTML comes from `renderMarkdown` (allow-list DOMPurify).
 * Link activation is intercepted: opening external URLs is a shell capability this app does
 * not enable, so the URL is shown to the operator instead of navigating.
 */
export function MarkdownPreview({ source, label = 'Preview' }: { source: string; label?: string }) {
  const html = useMemo(() => renderMarkdown(source), [source]);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const el = ref.current;
    if (el === null) {
      return;
    }
    const onClick = (ev: MouseEvent) => {
      const target = ev.target instanceof Element ? ev.target.closest('a') : null;
      if (target === null) {
        return;
      }
      const href = target.getAttribute('href') ?? '';
      if (!href.startsWith('#')) {
        ev.preventDefault();
        target.setAttribute('title', `External link not opened: ${href}`);
      }
    };
    el.addEventListener('click', onClick);
    return () => {
      el.removeEventListener('click', onClick);
    };
  }, []);

  // The only `dangerouslySetInnerHTML` in the app; input is always passed through the sanitizer.
  return <div ref={ref} className="md" aria-label={label} data-testid="markdown-preview" dangerouslySetInnerHTML={{ __html: html }} />;
}
