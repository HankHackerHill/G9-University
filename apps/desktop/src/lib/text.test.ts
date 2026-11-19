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

describe('names closed by the audit of 2026-11-19 (NAME-1)', () => {
  it('refuses fillers, blanks, stray selectors, odd spaces and runs of spaces, naming the code point', () => {
    const cases: [string, string][] = [
      ['invoice\u3164.exe', 'U+3164'],
      ['\u2800x', 'U+2800'],
      ['a\uFFA0b', 'U+FFA0'],
      ['a\u034Fb', 'U+034F'],
      ['a\u17B5b', 'U+17B5'],
      ['a\uFFFCb', 'U+FFFC'],
      ['a\u{E0100}b', 'U+E0100'],
      ['a\uFE0Fb', 'U+FE0F'],
      ['\uFE0F', 'U+FE0F'],
      ['a\uE000b', 'U+E000'],
      ['a\uFDD0b', 'U+FDD0'],
      ['a\u00A0b', 'U+00A0'],
      ['report.pdf\u3000\u3000.exe', 'U+3000'],
      ['a\u202Fb', 'U+202F'],
      ['\u0301abc', 'U+0301'],
      ['report   final', 'two or more spaces'],
    ];
    for (const [hostile, why] of cases) {
      expect(nameDefect(hostile), JSON.stringify(hostile)).toContain(why);
    }
  });

  it('keeps ordinary text, emoji with their selector, and trimmed edge spaces valid', () => {
    for (const ok of ['Dvo\u0159\u00E1k \u2013 Symphony No. 9', '\u5C71\u7530\u592A\u90CE', 'Plan \u2764\uFE0F', '1\uFE0F\u20E3 first', '  Dentist  ', 'e\u0301clair']) {
      expect(nameDefect(ok), JSON.stringify(ok)).toBeNull();
    }
  });

  it('applies the same blank-character rule to imported free text', () => {
    expect(hiddenCharacter('report\u3164')).toBe('3164');
    expect(hiddenCharacter('a\u00A0b')).toBeNull(); // free text may keep a no-break space
    expect(hiddenCharacter('a\u{E0100}b')).toBe('E0100');
  });
});

describe('a review prompt is shown as a row title and in sentences (P2A-03)', () => {
  it('is refused at the boundary when it holds a hidden or blank character, and may keep layout and spaces', () => {
    expect(firstUnprintableName({ prompt: 'Recall the flag \u202E.tpircs', answer: 'a' })).toMatchObject({ field: 'prompt' });
    expect(firstUnprintableName({ prompt: 'x\u3164' })).toMatchObject({ field: 'prompt' });
    expect(firstUnprintableName({ prompt: 'What does\u00A0HSTS prevent?\nName one.', answer: 'a\u202E' })).toBeNull();
  });
});
