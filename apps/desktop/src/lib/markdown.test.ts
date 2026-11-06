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

  it('drops javascript: and data: URLs but keeps https links with safe rel and no target', () => {
    const html = renderMarkdown('[a](javascript:alert(1)) [b](data:text/html;base64,PHNjcmlwdD4=) [c](https://example.org) <a href="https://x.y" target="_blank">d</a>');
    expect(html).not.toContain('javascript:');
    expect(html).not.toContain('data:');
    expect(html).toContain('href="https://example.org"');
    expect(html).not.toContain('target=');
    expect(html).toContain('rel="noopener noreferrer nofollow"');
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
