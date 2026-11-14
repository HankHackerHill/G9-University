import { describe, expect, it } from 'vitest';
import { renderMarkdown, sanitizeHtml } from './markdown';

describe('markdown sanitizer', () => {
  it('strips <script> blocks and inline event handlers', () => {
    const html = renderMarkdown('hello <script>alert(1)</script> world\n\n<p onclick="alert(2)">x</p>');
    expect(html).not.toContain('<script');
    expect(html).not.toContain('alert(1)');
    expect(html).not.toContain('onclick');
    expect(html).toContain('hello');
  });

  it('removes <img onerror> entirely (no images, no handlers)', () => {
    const html = renderMarkdown('<img src=x onerror=alert(1)>\n\n![alt](https://example.org/a.png)');
    expect(html).not.toContain('<img');
    expect(html).not.toContain('onerror');
  });

  // Security audit 2026-11-13: a webview follows a link by itself (context menu, middle
  // click, keyboard, drag) and no Tauri capability is involved, so a link in a note could
  // take the window to another origin. Rendered Markdown therefore has no links at all.
  it('renders no link whatever the form: the text stays and a web or mail address is shown as text', () => {
    const forms = [
      '[c](https://example.org/path?q=1)',
      '<https://example.org/auto>',
      'bare https://example.org/bare address',
      '[ref][r]\n\n[r]: https://example.org/ref "t"',
      '<a href="https://x.y" target="_blank">raw</a>',
      '[in app](#/settings)',
      '[rooted](/index.html)',
      '[scheme-relative](//evil.example/x)',
      '[backslash](/\\evil.example/x)',
      '[js](javascript:alert(1))',
      '[data](data:text/html;base64,PHNjcmlwdD4=)',
      '[mail](mailto:someone@example.org)',
      '[file](file:///etc/passwd)',
      '[nested **bold** text](https://example.org/n)',
    ];
    for (const source of forms) {
      const html = renderMarkdown(source);
      expect(html, source).not.toMatch(/<a[\s>]/i);
      expect(html, source).not.toMatch(/href|target=|rel=/i);
      expect(html, source).not.toContain('javascript:');
      expect(html, source).not.toContain('data:text');
    }
    // The words of the link stay; a web or mail address is shown next to them, as text to copy.
    expect(renderMarkdown('[c](https://example.org/path?q=1)')).toBe('<p>c (<code>https://example.org/path?q=1</code>)</p>\n');
    expect(renderMarkdown('[nested **bold** text](https://example.org/n)')).toBe('<p>nested <strong>bold</strong> text (<code>https://example.org/n</code>)</p>\n');
    expect(renderMarkdown('[mail](mailto:someone@example.org)')).toBe('<p>mail (<code>mailto:someone@example.org</code>)</p>\n');
    // An address that is its own text is shown once.
    expect(renderMarkdown('<https://example.org/auto>')).toBe('<p>https://example.org/auto</p>\n');
    // Anything that is not a web or mail address keeps its words and loses the address.
    expect(renderMarkdown('[in app](#/settings)')).toBe('<p>in app</p>\n');
    expect(renderMarkdown('[rooted](/index.html)')).toBe('<p>rooted</p>\n');
    expect(renderMarkdown('[scheme-relative](//evil.example/x)')).toBe('<p>scheme-relative</p>\n');
    expect(renderMarkdown('[js](javascript:alert(1))')).toBe('<p>js</p>\n');
  });

  it('shows the address of a link as text that cannot become markup', () => {
    const html = sanitizeHtml('<a href="https://example.org/?a=&lt;b&gt;&amp;c=&quot;d&quot;">x</a>');
    expect(html).toBe('x (<code>https://example.org/?a=&lt;b&gt;&amp;c="d"</code>)');
  });

  it('removes <svg onload>, <iframe>, <object>, <style> and <form> controls', () => {
    const html = sanitizeHtml(
      '<svg onload="alert(1)"><circle r="1"/></svg><iframe src="https://x"></iframe><object data="x"></object><style>body{display:none}</style><form><input name="a"><button>go</button></form>',
    );
    expect(html).not.toContain('<svg');
    expect(html).not.toContain('onload');
    expect(html).not.toContain('<iframe');
    expect(html).not.toContain('<object');
    expect(html).not.toContain('<style');
    expect(html).not.toContain('<input');
    expect(html).not.toContain('<button');
  });

  it('keeps headings, lists, code blocks and tables', () => {
    const html = renderMarkdown('# Title\n\n- one\n- two\n\n1. first\n\n```ts\nconst x = 1;\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n');
    expect(html).toContain('<h1>Title</h1>');
    expect(html).toContain('<ul>');
    expect(html).toContain('<li>one</li>');
    expect(html).toContain('<ol>');
    expect(html).toContain('<pre><code>');
    expect(html).toContain('const x = 1;');
    expect(html).toContain('<table>');
    expect(html).toContain('<td>2</td>');
  });

  it('strips class/style/id attributes so untrusted content cannot imitate VERIFIED badges', () => {
    const html = sanitizeHtml('<p class="badge badge-verified" style="color:red" id="main" data-x="1" aria-label="VERIFIED">text</p>');
    expect(html).toBe('<p>text</p>');
  });
});
