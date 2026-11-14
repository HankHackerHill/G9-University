/**
 * Markdown -> sanitized HTML. `marked` produces HTML, DOMPurify reduces it to a strict
 * allow-list. Everything rendered through here is treated as untrusted data (notes, imported
 * documents, model output), so:
 *
 * - no script/style/iframe/object/embed/svg/math/form controls/img/link/meta/base;
 * - no `on*` handlers, no `style`, no `class` (classes could imitate VERIFIED badges),
 *   no data-* or aria-* attributes;
 * - no links. A webview follows a link by itself: a click, a middle click, the Enter key,
 *   a drag or "Open Link" in the native context menu all navigate the window, and no Tauri
 *   capability is involved (an earlier version of this comment said one was needed; that was
 *   wrong, security audit 2026-11-13). A link in a note could therefore replace the
 *   application with a page from another origin. The words of a link are kept; a web or mail
 *   address is shown next to them as text the operator can copy; any other address
 *   (in-document, root-relative, `javascript:`, `data:`, `file:`) is dropped. The shell also
 *   refuses every navigation away from the application (`may_load` in
 *   `src-tauri/src/lib.rs`) and the CSP has `form-action 'none'`.
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

/** The only addresses shown (as text, never as a link): absolute http(s) and mailto. */
export const ALLOWED_URI_REGEXP = /^(?:https?:\/\/|mailto:)/i;

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
  // A fragment, so that links can be taken out of the tree before anything is serialized.
  RETURN_DOM_FRAGMENT: true,
});

/**
 * Replace every link by its own content, followed by its address as text when the address
 * survived the allow-list (http(s) or mailto) and is not already the words of the link.
 */
function unlink(fragment: DocumentFragment): void {
  for (const anchor of Array.from(fragment.querySelectorAll('a'))) {
    const href = anchor.getAttribute('href') ?? '';
    const words = anchor.textContent.trim();
    const parts: Node[] = Array.from(anchor.childNodes);
    if (ALLOWED_URI_REGEXP.test(href) && words !== href && `mailto:${words}` !== href) {
      const address = anchor.ownerDocument.createElement('code');
      address.textContent = href;
      parts.push(anchor.ownerDocument.createTextNode(' ('), address, anchor.ownerDocument.createTextNode(')'));
    }
    anchor.replaceWith(...parts);
  }
}

/** Sanitize already-rendered HTML with the house allow-list. The result holds no link. */
export function sanitizeHtml(html: string): string {
  const fragment = purifier.sanitize(html) as unknown as DocumentFragment;
  unlink(fragment);
  const holder = fragment.ownerDocument.createElement('div');
  holder.appendChild(fragment);
  return holder.innerHTML;
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
