/**
 * Markdown -> sanitized HTML. `marked` produces HTML, DOMPurify reduces it to a strict
 * allow-list. Everything rendered through here is treated as untrusted data (notes, imported
 * documents, model output), so:
 *
 * - no script/style/iframe/object/embed/svg/math/form controls/img/link/meta/base;
 * - no `on*` handlers, no `style`, no `class` (classes could imitate VERIFIED badges),
 *   no data-* or aria-* attributes;
 * - URLs limited to http(s), mailto and in-document anchors; `javascript:` and `data:` are
 *   dropped (we do not allow data: images either: the Tauri CSP keeps `img-src` to self);
 * - `target` is removed and `rel="noopener noreferrer nofollow"` is forced. Opening
 *   external URLs would need the Tauri shell capability, which is NOT enabled.
 */
import DOMPurify from 'dompurify';
import { marked } from 'marked';

export const ALLOWED_TAGS: readonly string[] = [
  'p',
  'br',
  'hr',
  'h1',
  'h2',
  'h3',
  'h4',
  'h5',
  'h6',
  'ul',
  'ol',
  'li',
  'blockquote',
  'pre',
  'code',
  'em',
  'strong',
  'del',
  'sup',
  'sub',
  'a',
  'table',
  'thead',
  'tbody',
  'tr',
  'th',
  'td',
];

export const ALLOWED_ATTR: readonly string[] = ['href', 'title', 'start'];

/** http(s), mailto, in-document anchors and root-relative paths only. */
export const ALLOWED_URI_REGEXP = /^(?:https?:|mailto:|#|\/(?!\/))/i;

const purifier = DOMPurify();

purifier.setConfig({
  ALLOWED_TAGS: [...ALLOWED_TAGS],
  ALLOWED_ATTR: [...ALLOWED_ATTR],
  ALLOWED_URI_REGEXP,
  ALLOW_DATA_ATTR: false,
  ALLOW_ARIA_ATTR: false,
  ALLOW_UNKNOWN_PROTOCOLS: false,
  KEEP_CONTENT: true,
  FORBID_TAGS: ['style', 'script', 'iframe', 'object', 'embed', 'svg', 'math', 'form', 'input', 'button', 'img', 'link', 'meta', 'base'],
  FORBID_ATTR: ['style', 'target', 'src', 'srcset', 'class', 'id', 'name'],
  RETURN_DOM: false,
  RETURN_DOM_FRAGMENT: false,
});

purifier.addHook('afterSanitizeAttributes', (node) => {
  if (node.nodeName === 'A') {
    node.removeAttribute('target');
    node.setAttribute('rel', 'noopener noreferrer nofollow');
  }
});

/** Sanitize already-rendered HTML with the house allow-list. */
export function sanitizeHtml(html: string): string {
  return purifier.sanitize(html);
}

/** Render markdown to sanitized HTML. Never throws on untrusted input; falls back to text. */
export function renderMarkdown(source: string): string {
  let html: string;
  try {
    html = marked.parse(source, { async: false, gfm: true, breaks: false });
  } catch {
    html = `<pre>${escapeText(source)}</pre>`;
  }
  return sanitizeHtml(html);
}

export function escapeText(text: string): string {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}
