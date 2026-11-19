import { describe, expect, it } from 'vitest';
import { firstUnprintableName, hiddenCharacter, nameDefect } from './text';

describe('names are one line of visible text', () => {
  it('accepts ordinary names in any script, padded or not', () => {
    for (const ok of ['Week 3 lab', 'Überblick: Tag 1', '報告書 v2', 'evidence (final).pdf', ' padded ', 'e\u0301clair']) {
      expect(nameDefect(ok), ok).toBeNull();
    }
  });

  it('refuses blank names', () => {
    for (const blank of ['', ' ', '\u00A0\u3000']) {
      expect(nameDefect(blank), JSON.stringify(blank)).toBe('is empty or only spaces');
    }
  });

  it('refuses every hidden character and names its code point', () => {
    const cases: [string, string][] = [
      ['a\nb', 'U+000A'],
      ['a\tb', 'U+0009'],
      ['pin\u202Etxt.exe', 'U+202E'],
      ['a\u200Bb', 'U+200B'],
      ['a\u2028b', 'U+2028'],
      ['a\u007Fb', 'U+007F'],
      ['a\u0085b', 'U+0085'],
      ['a\u{E0041}b', 'U+E0041'],
      ['\uFEFF', 'U+FEFF'],
      ['a\u00ADb', 'U+00AD'],
    ];
    for (const [text, code] of cases) {
      expect(nameDefect(text), JSON.stringify(text)).toContain(code);
    }
  });

  it('finds a name by key at any depth and ignores free text', () => {
    expect(firstUnprintableName({ title: 'fine', body_markdown: 'line one\n\nline two\u202E' })).toBeNull();
    expect(firstUnprintableName({ notes: [{ filename: 'ok.txt' }, { filename: 'bad\u202E.txt' }] })).toMatchObject({ field: 'filename' });
    expect(firstUnprintableName({ nested: { label: '  ' } })).toEqual({ field: 'label', reason: 'is empty or only spaces' });
    expect(firstUnprintableName(null)).toBeNull();
    expect(firstUnprintableName('title')).toBeNull();
  });
});

describe('free text in an imported curriculum', () => {
  it('may hold tab and line feed and refuses every other hidden character, naming its code point', () => {
    expect(hiddenCharacter('a\tb\nc')).toBeNull();
    expect(hiddenCharacter('Überblick 報告書')).toBeNull();
    expect(hiddenCharacter('report\u202Efdp.exe')).toBe('202E');
    expect(hiddenCharacter('a\u200Bb')).toBe('200B');
    expect(hiddenCharacter('a\r\nb')).toBe('000D');
    expect(hiddenCharacter('x\u{E0041}y')).toBe('E0041');
  });
});
