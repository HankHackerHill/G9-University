import { useMemo } from 'react';
import { renderMarkdown } from '../../lib/markdown';

/**
 * Sanitized markdown preview. The HTML comes from `renderMarkdown` (allow-list DOMPurify),
 * which renders no link: a webview follows a link by itself, so the words of a link are
 * shown and its address is text to copy (see `lib/markdown.ts`).
 */
export function MarkdownPreview({ source, label = 'Preview' }: { source: string; label?: string }) {
  const html = useMemo(() => renderMarkdown(source), [source]);

  // The only `dangerouslySetInnerHTML` in the app; input is always passed through the sanitizer.
  return <div className="md" aria-label={label} data-testid="markdown-preview" dangerouslySetInnerHTML={{ __html: html }} />;
}
