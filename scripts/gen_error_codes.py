#!/usr/bin/env python3
"""Error-code catalogue: generator and completeness gate (OBS-5).

``python3 -I scripts/gen_error_codes.py`` checks; ``--write`` regenerates
``docs/runbooks/error-codes.md``. Standard library only, offline, deterministic:
the output depends on the files under ``--root`` and on nothing else (no dates,
no commit ids, no environment, no randomness except the seeded ``--spot-check``).

What is read (all relative to ``--root``):

* production Rust: ``crates/*/src``, ``apps/cli/src``, ``apps/desktop/src-tauri/src``.
  ``tests/`` directories are outside these roots and skipped if nested; items
  under ``#[cfg(test)]`` (any predicate that holds only with ``test``) and
  ``#[test]``, files that start with ``#![cfg(test)]`` and the files of a
  ``#[cfg(test)] mod x;`` are removed at token level before anything is read;
* frontend TypeScript: ``apps/desktop/src`` without ``*.test.*``, ``*.d.ts``, the
  ``test/`` and ``fixtures/`` directories and the mock transport
  (``ipc/transports/mock*.ts``, a development double a production bundle never
  selects; its codes are compared, not catalogued);
* ``crates/g9-app/src/ops.rs`` (operation names, to tell them from codes);
* ``docs/runbooks/error-codes.actions.json`` (operator actions, written meanings,
  the allow-list for document references);
* the documents checked for dead references (``DOC_PATHS``).

How a code is recognised: by position, never by spelling. A string is an error
code only where it reaches the ``code`` of a ``G9Error``: the code argument of a
constructor of ``impl G9Error`` (``crates/g9-domain/src/envelope.rs``), an arm of
``fn code(&self)`` of an error enum whose value is passed to such a constructor,
an argument of a function that forwards it (found by a fixpoint over function
summaries), a ``format!`` whose parts are known, a JSON envelope literal with a
``category`` of the closed set and a ``code``, or a TypeScript object literal with
both. Operation names such as ``notes.create`` look the same but never reach
that position, so they are not codes.

Exit status: 0 current and complete, 1 a rule failed (each failure line names
its rule and the code or reference), 2 the tool could not run.
"""

import argparse
import itertools
import json
import os
import random
import re
import sys

CATALOGUE_REL = 'docs/runbooks/error-codes.md'
ACTIONS_REL = 'docs/runbooks/error-codes.actions.json'
OPS_REL = 'crates/g9-app/src/ops.rs'
DOC_PATHS = (
    'docs/ui/ux-copy.md',
    'docs/runbooks/first-run.md',
    'docs/runbooks/backup-restore.md',
    'docs/curriculum/promotion.md',
)
RUST_ROOTS = ('crates/*/src', 'apps/cli/src', 'apps/desktop/src-tauri/src')
HARNESS_PREFIX = 'crates/g9-evaluation/'
TS_ROOT = 'apps/desktop/src'
TS_SKIP_DIRS = frozenset(('test', '__tests__', '__mocks__', 'fixtures', 'node_modules', 'dist'))
TS_MOCK = re.compile(r'(^|/)ipc/transports/mock[^/]*\.tsx?$')
VARIES = 'VARIES'
DOTTED = re.compile(r'^[a-z][a-z0-9_]*(?:\.[a-z0-9_]+)+$')
RAW_DOTTED_LITERAL = re.compile(r'"([a-z][a-z0-9_]*(?:\.[a-z0-9_]+)+)"')
# A dotted token in a document: not part of a path, a URL, a file name with a dash, or a longer word.
DOC_TOKEN = re.compile(r'(?<![A-Za-z0-9_./:<>\-{$])[a-z][a-z0-9_]*(?:\.[a-z0-9_{}]+)+(?![A-Za-z0-9_/\-(])')
WARNING_PREFIX = re.compile(r'^(?:WARNING )?([a-z][a-z0-9_]*(?:\.[a-z0-9_]+)+): (.*)$', re.S)
WARNING_HELPER = re.compile(r'^WARNING \{([a-z_]+)\}: ')
FILE_SUFFIXES = frozenset((
    'rs', 'ts', 'tsx', 'js', 'cjs', 'mjs', 'json', 'jsonl', 'md', 'py', 'sh', 'toml', 'lock', 'sql',
    'sqlite', 'key', 'html', 'css', 'txt', 'yml', 'yaml', 'png', 'svg', 'partial', 'quarantine',
))
OPEN = {'(': ')', '[': ']', '{': '}'}
CLOSE = frozenset(')]}')
PATTERN_TEXT = (
    ('constructor', 'a string literal passed to a constructor of `G9Error` (`crates/g9-domain/src/envelope.rs`)'),
    ('helper', 'a literal passed to a function that forwards its argument to a constructor'),
    ('enum', 'an arm of `fn code(&self)` of an error type whose value reaches a constructor '
             '(`impl From<..> for G9Error` and similar)'),
    ('format', 'a `format!` whose parts are known: a literal, a constant or the literal payload of an enum variant'),
    ('choice', 'one of the literals of a `match` or `if` passed as the code'),
    ('json', 'a JSON error envelope literal (the serialization fallback of the bridge and the CLI)'),
    ('frontend', 'an object literal with `category` and `code` in `apps/desktop/src` (the IPC client and transport)'),
)


VARIES_ACTION = ('The guidance of the category the error carries (the table of that category above; '
                 '`docs/ui/ux-copy.md` section 5).')
HARNESS_NOTE = ('Raised only by `crates/g9-evaluation`, the offline gate harness (its `Cargo.toml`: nothing depends '
                'on it), never by the desktop window or the `g9` CLI. A row here is a failed gate scenario, not '
                'something the operator sees, so it has no operator action.')


class ToolError(Exception):
    """The generator cannot run (exit 2)."""


# --------------------------------------------------------------------------
# Rust lexer
# --------------------------------------------------------------------------

_RE_STR_START = re.compile(r'(br|r|b|c)?(#*)"')
_RE_IDENT = re.compile(r'[A-Za-z_][A-Za-z0-9_]*')
_RE_NUM = re.compile(r'[0-9][0-9A-Za-z_]*')
_RE_LIFETIME = re.compile(r"'[A-Za-z_][A-Za-z0-9_]*")
_SIMPLE_ESCAPES = {'n': '\n', 't': '\t', 'r': '\r', '0': '\0', '\\': '\\', '"': '"', "'": "'"}


def _decode_escapes(src, j, n):
    """Decode a non-raw string body starting at ``j``; returns ``(value, index after the quote)``."""
    buf = []
    while j < n and src[j] != '"':
        ch = src[j]
        if ch != '\\':
            buf.append(ch)
            j += 1
            continue
        nx = src[j + 1] if j + 1 < n else ''
        if nx in _SIMPLE_ESCAPES:
            buf.append(_SIMPLE_ESCAPES[nx])
            j += 2
        elif nx == 'x':
            try:
                buf.append(chr(int(src[j + 2:j + 4], 16)))
            except ValueError:
                buf.append('?')
            j += 4
        elif nx == 'u':
            k = src.find('}', j)
            try:
                buf.append(chr(int(src[j + 3:k].replace('_', ''), 16)))
            except ValueError:
                buf.append('?')
            j = k + 1 if k > 0 else n
        elif nx == '\n':
            j += 2
            while j < n and src[j] in ' \t\r\n':
                j += 1
        else:
            buf.append(nx)
            j += 2
    return ''.join(buf), j + 1


def lex_rust(src):
    """Tokens as ``(kind, value, line)``; kinds: id, str, num, char, life, p.

    Comments (line, nested block, doc) are dropped; string values are decoded.
    Macro bodies are lexed like any other code (no expansion).
    """
    toks = []
    i, n, line = 0, len(src), 1
    while i < n:
        c = src[i]
        if c == '\n':
            line += 1
            i += 1
            continue
        if c in ' \t\r':
            i += 1
            continue
        if c == '/' and src.startswith('//', i):
            j = src.find('\n', i)
            i = n if j < 0 else j
            continue
        if c == '/' and src.startswith('/*', i):
            depth, j = 1, i + 2
            while j < n and depth:
                if src.startswith('/*', j):
                    depth += 1
                    j += 2
                elif src.startswith('*/', j):
                    depth -= 1
                    j += 2
                else:
                    if src[j] == '\n':
                        line += 1
                    j += 1
            i = j
            continue
        if c in 'rbc"':
            m = _RE_STR_START.match(src, i)
            prefix = (m.group(1) or '') if m else ''
            if m and (not m.group(2) or 'r' in prefix):
                hashes = m.group(2)
                if 'r' in prefix:
                    end = src.find('"' + hashes, m.end())
                    end = n if end < 0 else end
                    value, j = src[m.end():end], end + 1 + len(hashes)
                else:
                    value, j = _decode_escapes(src, m.end(), n)
                toks.append(('str', value, line))
                line += src.count('\n', i, j)
                i = j
                continue
        if c == "'":
            if i + 1 < n and src[i + 1] == '\\':
                j = src.find("'", i + 3)
                j = n - 1 if j < 0 else j
                toks.append(('char', src[i:j + 1], line))
                i = j + 1
                continue
            if i + 2 < n and src[i + 2] == "'":
                toks.append(('char', src[i:i + 3], line))
                i += 3
                continue
            m = _RE_LIFETIME.match(src, i)
            if m:
                toks.append(('life', m.group(0), line))
                i = m.end()
                continue
        m = _RE_IDENT.match(src, i)
        if m:
            toks.append(('id', m.group(0), line))
            i = m.end()
            continue
        m = _RE_NUM.match(src, i)
        if m:
            toks.append(('num', m.group(0), line))
            i = m.end()
            continue
        two = src[i:i + 2]
        if two in ('::', '=>', '->'):
            toks.append(('p', two, line))
            i += 2
        else:
            toks.append(('p', c, line))
            i += 1
    return toks


def match_close(toks, i):
    """Index of the bracket that closes the one at ``toks[i]``."""
    depth = 0
    for j in range(i, len(toks)):
        kind, value, _ = toks[j]
        if kind == 'p':
            if value in OPEN:
                depth += 1
            elif value in CLOSE:
                depth -= 1
                if depth == 0:
                    return j
    return len(toks) - 1


def split_top(toks, lo, hi, sep=',', angles=False):
    """Split ``toks[lo:hi]`` at separators outside every bracket (and ``<..>`` if asked)."""
    parts, depth, angle, start = [], 0, 0, lo
    for j in range(lo, hi):
        kind, value, _ = toks[j]
        if kind != 'p':
            continue
        if value in OPEN:
            depth += 1
        elif value in CLOSE:
            depth -= 1
        elif angles and value == '<':
            angle += 1
        elif angles and value == '>':
            angle = max(0, angle - 1)
        elif value == sep and depth == 0 and angle == 0:
            parts.append((start, j))
            start = j + 1
    if start < hi:
        parts.append((start, hi))
    return parts


def scan_to(toks, j, hi, stops):
    """First index from ``j`` whose punctuation is in ``stops`` outside ``()`` and ``[]``."""
    depth = 0
    while j < hi:
        kind, value, _ = toks[j]
        if kind == 'p':
            if value in '([':
                depth += 1
            elif value in ')]':
                depth -= 1
            elif value in stops and depth == 0:
                return j
        j += 1
    return hi


def text_of(toks, lo, hi, limit=100):
    out = []
    for kind, value, _ in toks[lo:hi]:
        out.append(json.dumps(value, ensure_ascii=False) if kind == 'str' else value)
    text = ' '.join(out)
    text = re.sub(r'\s+([,;.)\]?])', r'\1', text)
    text = re.sub(r'([(\[.!&])\s+', r'\1', text)
    text = text.replace(' :: ', '::').replace(' (', '(')
    return text if len(text) <= limit else text[:limit - 1] + '…'


def normalise_message(text):
    """One line, single spaces, format specs dropped (``{x:?}`` -> ``{x}``)."""
    text = re.sub(r'\s+', ' ', text).strip()
    return re.sub(r'\{([A-Za-z0-9_.]*):[^{}]*\}', r'{\1}', text)


# --------------------------------------------------------------------------
# Test code removal
# --------------------------------------------------------------------------

def cfg_requires_test(toks):
    """True when a ``cfg(...)`` predicate holds only in test builds."""
    def parse(i):
        kind, value, _ = toks[i]
        if kind == 'id' and i + 1 < len(toks) and toks[i + 1][1] == '(':
            end = match_close(toks, i + 1)
            kids, a = [], i + 2
            while a < end:
                result, a = parse(a)
                kids.append(result)
                if a < end and toks[a][1] == ',':
                    a += 1
            if value == 'all':
                return any(kids), end + 1
            if value == 'any':
                return bool(kids) and all(kids), end + 1
            return False, end + 1
        if kind == 'id' and value == 'test':
            return True, i + 1
        j = i + 1
        while j < len(toks) and toks[j][1] != ',':
            if toks[j][1] in OPEN:
                j = match_close(toks, j)
            j += 1
        return False, j
    return bool(toks) and parse(0)[0]


def strip_test_items(toks):
    """Remove test-only items. Returns ``(tokens, test_module_names, whole_file_is_test)``."""
    out, test_mods = [], []
    i, n = 0, len(toks)
    while i < n:
        kind, value, _ = toks[i]
        if kind == 'p' and value == '#':
            inner = i + 1 < n and toks[i + 1][1] == '!'
            b = i + 2 if inner else i + 1
            if b < n and toks[b][1] == '[':
                e = match_close(toks, b)
                body = toks[b + 1:e]
                is_test = False
                if len(body) > 2 and body[0][1] == 'cfg' and body[1][1] == '(':
                    is_test = cfg_requires_test(body[2:-1])
                elif len(body) == 1 and body[0][1] == 'test':
                    is_test = True
                if is_test and inner:
                    return [], test_mods, True
                if is_test:
                    j = e + 1
                    while j + 1 < n and toks[j][1] == '#' and toks[j + 1][1] == '[':
                        j = match_close(toks, j + 1) + 1
                    start, depth = j, 0
                    while j < n:
                        k2, v2, _ = toks[j]
                        if k2 == 'p':
                            if v2 in OPEN:
                                depth += 1
                            elif v2 in CLOSE:
                                depth -= 1
                                if depth < 0:
                                    break
                                if depth == 0 and v2 == '}':
                                    j += 1
                                    if j < n and toks[j][1] == 'else':
                                        continue
                                    if j < n and toks[j][1] == ';':
                                        j += 1
                                    break
                            elif v2 == ';' and depth == 0:
                                j += 1
                                break
                        j += 1
                    names = [t[1] for t in toks[start:j]]
                    if 'mod' in names:
                        m = names.index('mod')
                        if m + 2 < len(names) and names[m + 2] == ';':
                            test_mods.append(names[m + 1])
                    i = j
                    continue
                out.extend(toks[i:e + 1])
                i = e + 1
                continue
        out.append(toks[i])
        i += 1
    return out, test_mods, False


# --------------------------------------------------------------------------
# Rust structure: enums, functions, impls, constants
# --------------------------------------------------------------------------

class Variant:
    __slots__ = ('name', 'tuple_types', 'field_types', 'error_text', 'transparent')

    def __init__(self, name):
        self.name = name
        self.tuple_types = []      # one list of type identifiers per tuple field
        self.field_types = {}      # field name -> list of type identifiers
        self.error_text = None     # thiserror #[error("...")] text
        self.transparent = False


class EnumDef:
    __slots__ = ('crate', 'name', 'file', 'variants', 'code_map', 'cat_map', 'code_file')

    def __init__(self, crate, name, file):
        self.crate, self.name, self.file = crate, name, file
        self.variants = {}      # name -> Variant (declaration order)
        self.code_map = None    # variant or '*' -> ('lits', [..]) | ('delegate', binding) | ('unknown', text)
        self.cat_map = None     # variant or '*' -> ('cat', NAME) | ('delegate', binding) | ('unknown', text)
        self.code_file = None


class Fn:
    __slots__ = ('file', 'crate', 'name', 'impl_type', 'impl_trait', 'params', 'open', 'close', 'toks',
                 'in_impl')

    def __init__(self, file, crate, name, toks):
        self.file, self.crate, self.name, self.toks = file, crate, name, toks
        self.impl_type = None
        self.impl_trait = None
        self.params = []        # [(name or None, [type identifiers])]
        self.open = self.close = 0
        self.in_impl = False

    def key(self):
        return (self.file, self.name, self.impl_type or '', self.open)


def crate_of(rel):
    parts = rel.split('/')
    if parts[0] == 'crates':
        return parts[1]
    if rel.startswith('apps/desktop/src-tauri/'):
        return 'g9-desktop'
    if rel.startswith('apps/cli/'):
        return 'g9-cli'
    return parts[0]


def _ident_before_generics(toks, lo, hi):
    """Last path identifier of a type, ignoring generic arguments."""
    name, angle = None, 0
    for kind, value, _ in toks[lo:hi]:
        if kind == 'p' and value == '<':
            angle += 1
        elif kind == 'p' and value == '>':
            angle = max(0, angle - 1)
        elif kind == 'id' and value == 'where' and angle == 0:
            break
        elif kind == 'id' and angle == 0 and value not in ('dyn', 'mut', 'const', 'for', 'impl'):
            name = value
    return name


def _idents(toks, lo, hi):
    return [v for k, v, _ in toks[lo:hi] if k == 'id']


def _skip_attrs(toks, a, b):
    while a + 1 < b and toks[a][1] == '#' and toks[a + 1][1] == '[':
        a = match_close(toks, a + 1) + 1
    return a


def _skip_generics(toks, i):
    """``toks[i]`` is ``<``: index just after the matching ``>``."""
    depth = 0
    while i < len(toks):
        value = toks[i][1]
        if toks[i][0] == 'p':
            if value == '<':
                depth += 1
            elif value == '>':
                depth -= 1
                if depth == 0:
                    return i + 1
            elif value in OPEN:
                i = match_close(toks, i)
        i += 1
    return i


def parse_enum(toks, i, crate, rel):
    """``toks[i]`` is ``enum``. Returns ``(EnumDef or None, next index)``."""
    name = toks[i + 1][1]
    j = i + 2
    if j < len(toks) and toks[j][1] == '<':
        j = _skip_generics(toks, j)
    while j < len(toks) and toks[j][1] not in ('{', ';'):
        j += 1
    if j >= len(toks) or toks[j][1] != '{':
        return None, j + 1
    close = match_close(toks, j)
    enum = EnumDef(crate, name, rel)
    k = j + 1
    while k < close:
        error_text, transparent = None, False
        while k + 1 < close and toks[k][1] == '#' and toks[k + 1][1] == '[':
            e = match_close(toks, k + 1)
            if toks[k + 2][1] == 'error' and k + 3 < e and toks[k + 3][1] == '(':
                inner = toks[k + 4:e - 1]
                if inner and inner[0][0] == 'str':
                    error_text = inner[0][1]
                elif inner and inner[0][1] == 'transparent':
                    transparent = True
            k = e + 1
        if k >= close or toks[k][0] != 'id':
            k += 1
            continue
        variant = Variant(toks[k][1])
        variant.error_text, variant.transparent = error_text, transparent
        k += 1
        if k < close and toks[k][1] == '(':
            e = match_close(toks, k)
            for a, b in split_top(toks, k + 1, e, angles=True):
                a = _skip_attrs(toks, a, b)
                variant.tuple_types.append(_idents(toks, a, b))
            k = e + 1
        elif k < close and toks[k][1] == '{':
            e = match_close(toks, k)
            for a, b in split_top(toks, k + 1, e, angles=True):
                a = _skip_attrs(toks, a, b)
                if a < b and toks[a][1] == 'pub':
                    a += 1
                    if a < b and toks[a][1] == '(':
                        a = match_close(toks, a) + 1
                if a + 1 < b and toks[a][0] == 'id' and toks[a + 1][1] == ':':
                    variant.field_types[toks[a][1]] = _idents(toks, a + 2, b)
            k = e + 1
        while k < close and toks[k][1] != ',':
            if toks[k][1] in OPEN:
                k = match_close(toks, k)
            k += 1
        k += 1
        enum.variants[variant.name] = variant
    return enum, close + 1


def error_attr_before(toks, i):
    """The ``#[error("...")]`` text among the attributes in front of the item keyword at ``toks[i]``."""
    j = i - 1
    while j >= 0 and toks[j][1] in ('pub', 'crate', 'super', 'in', '(', ')'):
        j -= 1
    while j >= 0 and toks[j][1] == ']':
        depth, k = 0, j
        while k >= 0:
            if toks[k][1] == ']':
                depth += 1
            elif toks[k][1] == '[':
                depth -= 1
                if depth == 0:
                    break
            k -= 1
        if k < 1 or toks[k - 1][1] != '#':
            return None
        if k + 3 < j and toks[k + 1][1] == 'error' and toks[k + 2][1] == '(' and toks[k + 3][0] == 'str':
            return toks[k + 3][1]
        j = k - 2
    return None


def parse_file_structure(rel, crate, toks):
    """One pass: functions (with their impl), enums, error structs, string constants."""
    fns, enums, consts = [], [], {}
    scopes = []     # (close index, kind, payload)
    n = len(toks)
    i = 0
    while i < n:
        while scopes and i > scopes[-1][0]:
            scopes.pop()
        kind, value, _ = toks[i]
        if kind != 'id':
            i += 1
            continue
        prev = toks[i - 1][1] if i else ''
        if value == 'impl' and prev in ('', '}', ';', ']', '{', 'unsafe', 'default'):
            j = i + 1
            if j < n and toks[j][1] == '<':
                j = _skip_generics(toks, j)
            start = j
            j = scan_to(toks, j, n, '{;')
            if j >= n or toks[j][1] != '{':
                i = j + 1
                continue
            for_at, angle = None, 0
            for t in range(start, j):
                v = toks[t][1]
                if toks[t][0] == 'p' and v == '<':
                    angle += 1
                elif toks[t][0] == 'p' and v == '>':
                    angle = max(0, angle - 1)
                elif toks[t][0] == 'id' and v == 'for' and angle == 0:
                    for_at = t
                    break
            if for_at is None:
                impl_type, impl_trait = _ident_before_generics(toks, start, j), None
            else:
                impl_type = _ident_before_generics(toks, for_at + 1, j)
                impl_trait = _ident_before_generics(toks, start, for_at)
            scopes.append((match_close(toks, j), 'impl', (impl_type, impl_trait)))
            i = j + 1
            continue
        if value == 'fn' and i + 1 < n and toks[i + 1][0] == 'id':
            fn = Fn(rel, crate, toks[i + 1][1], toks)
            j = i + 2
            if j < n and toks[j][1] == '<':
                j = _skip_generics(toks, j)
            if j >= n or toks[j][1] != '(':
                i += 1
                continue
            pe = match_close(toks, j)
            for a, b in split_top(toks, j + 1, pe, angles=True):
                a = _skip_attrs(toks, a, b)
                colon = next((t for t in range(a, b) if toks[t][1] == ':'), None)
                pat_end = colon if colon is not None else b
                names = [v for k, v, _ in toks[a:pat_end] if k == 'id' and v not in ('mut', 'ref')]
                if 'self' in names:
                    fn.params.append(('self', []))
                elif len(names) == 1 and pat_end - a <= 2:
                    fn.params.append((names[0], _idents(toks, pat_end + 1, b) if colon is not None else []))
                else:
                    fn.params.append((None, []))
            j = scan_to(toks, pe + 1, n, '{;')
            if j >= n or toks[j][1] != '{':
                i = j + 1
                continue
            fn.open, fn.close = j, match_close(toks, j)
            for _, skind, payload in reversed(scopes):
                if skind == 'impl':
                    fn.impl_type, fn.impl_trait = payload
                    fn.in_impl = scopes[-1][1] == 'impl'
                    break
            fns.append(fn)
            scopes.append((fn.close, 'fn', fn))
            i = j + 1
            continue
        if value == 'struct' and prev not in ('::', '.') and i + 1 < n and toks[i + 1][0] == 'id':
            text = error_attr_before(toks, i)
            if text is not None:
                consts.setdefault(('struct', toks[i + 1][1]), set()).add(text)
            i += 2
            continue
        if value == 'enum' and prev not in ('::', '.') and i + 1 < n and toks[i + 1][0] == 'id':
            enum, nxt = parse_enum(toks, i, crate, rel)
            if enum is not None:
                enums.append(enum)
            i = nxt
            continue
        if value in ('const', 'static') and i + 2 < n and toks[i + 1][0] == 'id' and toks[i + 2][1] == ':':
            name = toks[i + 1][1]
            j, depth, eq = i + 3, 0, None
            while j < n:
                v = toks[j][1]
                if toks[j][0] == 'p':
                    if v in OPEN:
                        depth += 1
                    elif v in CLOSE:
                        depth -= 1
                    elif v == '=' and depth == 0 and eq is None:
                        eq = j
                    elif v == ';' and depth == 0:
                        break
                j += 1
            if eq is not None and j == eq + 2 and toks[eq + 1][0] == 'str':
                consts.setdefault(name, set()).add(toks[eq + 1][1])
            i = j + 1
            continue
        i += 1
    return fns, enums, consts


# --------------------------------------------------------------------------
# Rust analysis
# --------------------------------------------------------------------------

def screaming(name):
    return re.sub(r'(?<!^)(?=[A-Z])', '_', name).upper()


def _to_comma(toks, k, hi):
    depth = 0
    while k < hi:
        kind, value, _ = toks[k]
        if kind == 'p':
            if value in OPEN:
                depth += 1
            elif value in CLOSE:
                depth -= 1
            elif value == ',' and depth == 0:
                break
        k += 1
    return k


def _to_semicolon(toks, k, hi):
    depth = 0
    while k < hi:
        kind, value, _ = toks[k]
        if kind == 'p':
            if value in OPEN:
                depth += 1
            elif value in CLOSE:
                depth -= 1
            elif value == ';' and depth == 0:
                break
        k += 1
    return k


def parse_match_arms(toks, open_i, close_i):
    """Arms of a ``match`` body: ``[(pat_lo, pat_hi, body_lo, body_hi)]``."""
    arms, i = [], open_i + 1
    while i < close_i:
        i = _skip_attrs(toks, i, close_i)
        j, depth = i, 0
        while j < close_i:
            kind, value, _ = toks[j]
            if kind == 'p':
                if value in OPEN:
                    depth += 1
                elif value in CLOSE:
                    depth -= 1
                elif value == '=>' and depth == 0:
                    break
            j += 1
        if j >= close_i:
            break
        body_lo = j + 1
        if body_lo < close_i and toks[body_lo][1] == '{':
            body_hi = match_close(toks, body_lo) + 1
            nxt = body_hi
            if nxt < close_i and toks[nxt][1] == ',':
                nxt += 1
            elif nxt < close_i and toks[nxt][1] in ('.', '?'):
                k = _to_comma(toks, nxt, close_i)
                body_hi, nxt = k, k + 1
        else:
            k = _to_comma(toks, body_lo, close_i)
            body_hi, nxt = k, k + 1
        arms.append((i, j, body_lo, body_hi))
        i = nxt
    return arms


def find_matches(toks, lo, hi):
    """Every ``match`` in ``toks[lo:hi]``: ``[(scrutinee_lo, scrutinee_hi, open, close)]``."""
    found = []
    for i in range(lo, hi):
        if toks[i][0] == 'id' and toks[i][1] == 'match' and (i == 0 or toks[i - 1][1] not in ('.', '::')):
            j = scan_to(toks, i + 1, hi, '{;')
            if j < hi and toks[j][1] == '{':
                found.append((i + 1, j, j, match_close(toks, j)))
    return found


def pattern_bindings(toks, lo, hi):
    """Identifiers a pattern binds (lowercase names that are not path segments or field names)."""
    names = []
    for i in range(lo, hi):
        kind, value, _ = toks[i]
        if kind != 'id' or value in ('mut', 'ref', 'if', '_', 'Some', 'None', 'Ok', 'Err', 'self'):
            continue
        nxt = toks[i + 1][1] if i + 1 < hi else ''
        prv = toks[i - 1][1] if i > lo else ''
        if nxt == '::' or prv == '::' or nxt in ('(', '{'):
            continue
        if nxt == ':' and i + 2 < hi:
            continue
        if value[0].islower() or value[0] == '_':
            names.append(value)
    return names


class Analysis:
    """Everything extracted from the production Rust sources."""

    def __init__(self, root):
        self.root = root
        self.files = {}             # rel -> tokens (test items removed)
        self.excluded = []          # (rel, reason)
        self.fns = []
        self.fns_by_file = {}
        self.fn_index = {}
        self.enums = {}             # (crate, name) -> EnumDef
        self.enums_by_name = {}
        self.consts = {}            # rel -> {name: set(values)}
        self.categories = []        # SCREAMING names, declared order
        self.cat_variants = {}      # CamelCase -> SCREAMING
        self.summaries = {}         # fn key -> [template]
        self.summary_names = set()
        self.ctor_keys = set()
        self.emissions = []         # dicts
        self.unresolved = []        # dicts
        self.blind = []             # (kind, where, text): constructs the analysis could not follow
        self.sink_calls = 0
        self.warnings = {}          # code -> {'files': set, 'texts': set, 'owners': set}
        self.reasons = {}           # message-prefix tags inside error constructors
        self.literals = {}          # dotted literal -> set(files), test items removed
        self.operations = set()
        self.called = set()         # keys of summarised functions with a call site
        self.unresolved_calls = set()
        self.ambiguous = []

    # ---- loading -------------------------------------------------------

    def rust_files(self):
        rels = []
        for pattern in RUST_ROOTS:
            if '/*/' in pattern:
                head, tail = pattern.split('/*/')
                base = os.path.join(self.root, head)
                tops = [head + '/' + d + '/' + tail for d in sorted(os.listdir(base))] if os.path.isdir(base) else []
            else:
                tops = [pattern]
            for top in tops:
                for dirpath, dirnames, filenames in os.walk(os.path.join(self.root, top)):
                    dirnames[:] = sorted(d for d in dirnames if d not in ('tests', 'target', 'node_modules'))
                    for name in sorted(filenames):
                        if name.endswith('.rs'):
                            rels.append(os.path.relpath(os.path.join(dirpath, name), self.root).replace(os.sep, '/'))
        return rels

    def load(self):
        excluded_paths, excluded_dirs = set(), set()
        raw = {}
        rels = self.rust_files()
        if not rels:
            raise ToolError('no Rust source under ' + ', '.join(RUST_ROOTS))
        for rel in rels:
            toks, test_mods, whole = strip_test_items(lex_rust(read_text(self.root, rel)))
            if whole:
                self.excluded.append((rel, '#![cfg(test)]'))
                continue
            raw[rel] = toks
            directory, base = os.path.dirname(rel), os.path.basename(rel)
            for mod in test_mods:
                home = directory if base in ('lib.rs', 'main.rs', 'mod.rs') else directory + '/' + base[:-3]
                excluded_paths.update((home + '/' + mod + '.rs', home + '/' + mod + '/mod.rs'))
                excluded_dirs.add(home + '/' + mod + '/')
        for rel, toks in raw.items():
            if rel in excluded_paths or any(rel.startswith(d) for d in excluded_dirs):
                self.excluded.append((rel, '#[cfg(test)] mod'))
                continue
            self.files[rel] = toks
        for rel in sorted(self.files):
            crate = crate_of(rel)
            fns, enums, consts = parse_file_structure(rel, crate, self.files[rel])
            self.fns.extend(fns)
            self.fns_by_file[rel] = fns
            self.consts[rel] = consts
            for enum in enums:
                self.enums.setdefault((crate, enum.name), enum)
                self.enums_by_name.setdefault(enum.name, []).append(enum)
        cat_enum = self.enums.get(('g9-domain', 'ErrorCategory'))
        if cat_enum is None:
            raise ToolError('enum ErrorCategory not found under crates/g9-domain/src')
        for name in cat_enum.variants:
            self.cat_variants[name] = screaming(name)
            self.categories.append(screaming(name))
        for fn in self.fns:
            self.fn_index.setdefault(fn.name, []).append(fn)
        ops = self.files.get(OPS_REL, [])
        for i in range(len(ops) - 2):
            if ops[i][1] == 'op' and ops[i + 1][1] == '(' and ops[i + 2][0] == 'str':
                self.operations.add(ops[i + 2][1])

    # ---- lookups -------------------------------------------------------

    def resolve_enum(self, crate, idents):
        """The enum a list of type identifiers names (same crate first)."""
        for ident in reversed(idents):
            if (crate, ident) in self.enums:
                return self.enums[(crate, ident)]
        for ident in reversed(idents):
            candidates = self.enums_by_name.get(ident, [])
            with_codes = [e for e in candidates if e.code_map is not None]
            if len(with_codes) == 1:
                return with_codes[0]
            if len(candidates) == 1:
                return candidates[0]
        return None

    def const_value(self, rel, name):
        """A string constant: same file, else unique in the crate, else unique anywhere."""
        values = self.consts.get(rel, {}).get(name)
        if values and len(values) == 1:
            return next(iter(values))
        crate = crate_of(rel)
        pooled = set()
        for other, table in self.consts.items():
            if crate_of(other) == crate and name in table:
                pooled |= table[name]
        if not pooled:
            for table in self.consts.values():
                pooled |= table.get(name, set())
        return next(iter(pooled)) if len(pooled) == 1 else None

    def struct_text(self, crate, idents):
        """The ``#[error]`` text of an error struct named by type identifiers (same crate first)."""
        for ident in reversed(idents):
            same = [t for r, t in self.consts.items() if crate_of(r) == crate and ('struct', ident) in t]
            pool = same or [t for t in self.consts.values() if ('struct', ident) in t]
            texts = set().union(*(t[('struct', ident)] for t in pool)) if pool else set()
            if len(texts) == 1:
                return next(iter(texts))
        return None

    def innermost_fn(self, rel, index):
        best = None
        for fn in self.fns_by_file.get(rel, ()):
            if fn.open < index < fn.close and (best is None or fn.open > best.open):
                best = fn
        return best

    # ---- enum code / category functions --------------------------------

    def build_enum_maps(self):
        for fn in self.fns:
            if not fn.in_impl or fn.impl_trait is not None or fn.name not in ('code', 'category'):
                continue
            if not fn.params or fn.params[0][0] != 'self':
                continue
            enum = self.enums.get((fn.crate, fn.impl_type))
            if enum is None:
                continue
            toks = fn.toks
            matches = [m for m in find_matches(toks, fn.open + 1, fn.close) if 'self' in _idents(toks, m[0], m[1])]
            if not matches:
                self.blind.append(('code-fn-without-match', fn.file, enum.name + '::' + fn.name))
                continue
            _, _, open_i, close_i = matches[0]
            table = {}
            for pat_lo, pat_hi, body_lo, body_hi in parse_match_arms(toks, open_i, close_i):
                variants = self.pattern_variants(enum, toks, pat_lo, pat_hi)
                value = self.arm_value(fn, toks, body_lo, body_hi)
                if value[0] == 'delegate' and value[1] not in pattern_bindings(toks, pat_lo, pat_hi):
                    value = ('unknown', text_of(toks, body_lo, body_hi))
                if value[0] == 'unknown':
                    self.blind.append(('code-arm-not-followed', fn.file, enum.name + '::' + fn.name + ': ' + value[1]))
                for variant in (variants if variants is not None else ['*']):
                    table.setdefault(variant, value)
            if fn.name == 'code':
                enum.code_map, enum.code_file = table, fn.file
            else:
                enum.cat_map = table

    def pattern_variants(self, enum, toks, lo, hi):
        """Variants an arm pattern names, or ``None`` for a catch-all arm."""
        names = []
        for i in range(lo, hi - 2):
            if toks[i][0] == 'id' and toks[i + 1][1] == '::' and toks[i + 2][0] == 'id':
                if toks[i][1] in (enum.name, 'Self') and toks[i + 2][1] in enum.variants:
                    names.append(toks[i + 2][1])
        if names:
            return names
        if any(toks[i][1] == '::' for i in range(lo, hi)):
            return []
        bare = [toks[i][1] for i in range(lo, hi) if toks[i][0] == 'id' and toks[i][1] in enum.variants]
        return bare or None

    def arm_value(self, fn, toks, lo, hi):
        while hi - lo >= 2 and toks[lo][1] == '{' and match_close(toks, lo) == hi - 1:
            lo, hi = lo + 1, hi - 1
        body = toks[lo:hi]
        if len(body) == 1 and body[0][0] == 'str':
            return ('lits', [body[0][1]])
        if len(body) == 1 and body[0][0] == 'id':
            const = self.const_value(fn.file, body[0][1])
            if const is not None:
                return ('lits', [const])
        if len(body) == 3 and body[0][1] == 'ErrorCategory' and body[1][1] == '::':
            return ('cat', self.cat_variants.get(body[2][1], VARIES))
        if (len(body) == 5 and body[0][0] == 'id' and body[1][1] == '.' and body[2][1] in ('code', 'category')
                and body[3][1] == '(' and body[4][1] == ')'):
            return ('delegate', body[0][1])
        lits = [t[1] for t in body if t[0] == 'str']
        if lits and fn.name == 'code':
            return ('lits', lits)
        cats = [self.cat_variants[body[i + 2][1]] for i in range(len(body) - 2)
                if body[i][1] == 'ErrorCategory' and body[i + 1][1] == '::' and body[i + 2][1] in self.cat_variants]
        if len(set(cats)) == 1:
            return ('cat', cats[0])
        return ('unknown', text_of(toks, lo, hi))

    def delegate_enum(self, enum, variant_name, binding):
        variant = enum.variants.get(variant_name)
        if variant is None:
            return None
        if binding in variant.field_types:
            return self.resolve_enum(enum.crate, variant.field_types[binding])
        for types in variant.tuple_types:
            target = self.resolve_enum(enum.crate, types)
            if target is not None:
                return target
        return None

    def enum_codes(self, enum, variant_name, depth=0):
        """``[(code, sub_enum or None, sub_variant or None)]`` for one variant."""
        if enum.code_map is None or depth > 4:
            return []
        value = enum.code_map.get(variant_name, enum.code_map.get('*'))
        if value is None or value[0] == 'unknown':
            return []
        if value[0] == 'lits':
            return [(lit, None, None) for lit in value[1]]
        target = self.delegate_enum(enum, variant_name, value[1])
        if target is None:
            self.blind.append(('code-delegate-unresolved', enum.file, enum.name + '::' + variant_name))
            return []
        out = []
        for sub in target.variants:
            for code, _, _ in self.enum_codes(target, sub, depth + 1):
                out.append((code, target, sub))
        return out

    def enum_category(self, enum, variant_name, sub_enum=None, sub_variant=None, depth=0):
        if enum.cat_map is None or depth > 4:
            return VARIES
        value = enum.cat_map.get(variant_name, enum.cat_map.get('*'))
        if value is None:
            return VARIES
        if value[0] == 'cat':
            return value[1]
        if value[0] == 'delegate':
            target = self.delegate_enum(enum, variant_name, value[1])
            if target is not None and sub_enum is target and sub_variant is not None:
                return self.enum_category(target, sub_variant, depth=depth + 1)
        return VARIES

    def enum_texts(self, enum, variant_name, sub_enum=None, sub_variant=None, depth=0):
        """Display texts of a variant (``#[error]``; through ``#[error(transparent)]`` to the wrapped type)."""
        variant = enum.variants.get(variant_name)
        if variant is None or depth > 4:
            return []
        if not variant.transparent:
            return [variant.error_text] if variant.error_text else []
        if sub_enum is not None and sub_variant is not None:
            return self.enum_texts(sub_enum, sub_variant, depth=depth + 1)
        types = variant.tuple_types[0] if variant.tuple_types else []
        text = self.struct_text(enum.crate, types)
        if text is not None:
            return [text]
        inner = self.resolve_enum(enum.crate, types)
        if inner is None:
            return []
        out = []
        for name in inner.variants:
            out.extend(self.enum_texts(inner, name, depth=depth + 1))
        return out

    # ---- expression values ---------------------------------------------

    def enclosing_arms(self, fn, index):
        """Innermost-first ``[(scrut_lo, scrut_hi, arms, arm_index)]`` around a token."""
        if fn is None:
            return []
        out = []
        toks = fn.toks
        for scrut_lo, scrut_hi, open_i, close_i in find_matches(toks, fn.open + 1, fn.close):
            if not open_i < index < close_i:
                continue
            arms = parse_match_arms(toks, open_i, close_i)
            for n, (pat_lo, _, _, body_hi) in enumerate(arms):
                if pat_lo <= index < body_hi:
                    out.append((scrut_lo, scrut_hi, arms, n))
                    break
        out.sort(key=lambda item: -item[0])
        return out

    def arm_variant_set(self, fn, index, var, enum):
        """Variants of ``enum`` that reach ``index`` through a ``match <var>``; ``None`` = all."""
        toks = fn.toks
        for scrut_lo, scrut_hi, arms, n in self.enclosing_arms(fn, index):
            if var not in _idents(toks, scrut_lo, scrut_hi):
                continue
            named = self.pattern_variants(enum, toks, arms[n][0], arms[n][1])
            if named is None:
                taken = set()
                for m, arm in enumerate(arms):
                    if m == n:
                        continue
                    other = self.pattern_variants(enum, toks, arm[0], arm[1])
                    guarded = any(t[1] == 'if' for t in toks[arm[0]:arm[1]])
                    if other and not guarded:
                        taken.update(other)
                return [v for v in enum.variants if v not in taken]
            return named
        return None

    def var_enum(self, fn, var):
        if fn is None:
            return None
        if var == 'self':
            return self.enums.get((fn.crate, fn.impl_type)) if fn.impl_type else None
        for name, types in fn.params:
            if name == var:
                return self.resolve_enum(fn.crate, types)
        return None

    def find_let(self, fn, index, name):
        """Tokens of the nearest ``let <name> = <expr>;`` before ``index``."""
        toks = fn.toks
        for i in range(index - 1, fn.open, -1):
            if toks[i][0] == 'id' and toks[i][1] == 'let':
                j = i + 1
                if toks[j][1] == 'mut':
                    j += 1
                if toks[j][1] != name:
                    continue
                j += 1
                if toks[j][1] == ':':
                    while j < index and toks[j][1] != '=':
                        j += 1
                if toks[j][1] != '=':
                    continue
                k = _to_semicolon(toks, j + 1, fn.close)
                if k < index:
                    return j + 1, k
        return None

    def closure_param(self, fn, index, name):
        """True when ``name`` is a parameter of a closure that opens before ``index``."""
        toks = fn.toks
        i, depth = index - 1, 0
        while i > fn.open:
            kind, value, _ = toks[i]
            if kind == 'p' and value in CLOSE:
                depth += 1
            elif kind == 'p' and value in OPEN:
                depth -= 1
            elif kind == 'p' and value == '|' and depth <= 0:
                j = i - 1
                while j > fn.open and toks[j][1] != '|' and toks[j][1] not in (';', '{', '}'):
                    j -= 1
                if toks[j][1] == '|' and name in _idents(toks, j, i):
                    return True
                i = j
            i -= 1
        return False

    def value(self, fn, rel, toks, lo, hi, index, depth=0):
        """Classify the expression ``toks[lo:hi]`` used at call site ``index``.

        ``index`` is the use site: a value bound by ``let`` is evaluated with the
        match arms around the use, so ``let code = e.code(); match e { A => ctor(code) }``
        yields only the codes of ``A``.
        """
        while lo < hi and toks[lo][1] in ('&', '*'):
            lo += 1
        while hi - lo >= 2 and toks[lo][1] == '(' and match_close(toks, lo) == hi - 1:
            lo, hi = lo + 1, hi - 1
        while (hi - lo >= 5 and toks[hi - 1][1] == ')' and toks[hi - 2][1] == '(' and toks[hi - 4][1] == '.'
               and toks[hi - 3][1] in ('into', 'to_owned', 'clone', 'as_str', 'as_ref', 'to_string')
               and (toks[hi - 3][1] != 'to_string' or toks[hi - 5][0] == 'str')):
            hi -= 4
        body = toks[lo:hi]
        if not body:
            return ('unknown', '')
        if len(body) == 1 and body[0][0] == 'str':
            return ('lit', body[0][1])
        if (len(body) == 6 and body[0][1] == 'String' and body[1][1] == '::' and body[2][1] == 'from'
                and body[3][1] == '(' and body[4][0] == 'str'):
            return ('lit', body[4][1])
        if len(body) == 3 and body[1][1] == '::' and body[0][1] == 'ErrorCategory':
            return ('cat', self.cat_variants.get(body[2][1], VARIES))
        if len(body) >= 4 and body[0][1] == 'format' and body[1][1] == '!' and body[2][1] in OPEN:
            end = match_close(toks, lo + 2)
            if end == hi - 1:
                parts = split_top(toks, lo + 3, end)
                if parts and parts[0][1] - parts[0][0] == 1 and toks[parts[0][0]][0] == 'str':
                    template = toks[parts[0][0]][1]
                    mapping, positional = {}, 0
                    for a, b in parts[1:]:
                        if b - a >= 3 and toks[a][0] == 'id' and toks[a + 1][1] == '=':
                            mapping[toks[a][1]] = self.value(fn, rel, toks, a + 2, b, index, depth + 1)
                        else:
                            mapping[positional] = self.value(fn, rel, toks, a, b, index, depth + 1)
                            positional += 1
                    for name in re.findall(r'\{([A-Za-z_][A-Za-z0-9_]*)(?::[^}]*)?\}', template):
                        if name not in mapping:
                            mapping[name] = self.name_value(fn, rel, toks, name, index, depth + 1)
                    return ('fmt', template, mapping, rel)
        if len(body) == 1 and body[0][0] == 'id':
            return self.name_value(fn, rel, toks, body[0][1], index, depth)
        if (len(body) == 5 and body[0][0] == 'id' and body[1][1] == '.' and body[3][1] == '('
                and body[4][1] == ')' and body[2][1] in ('code', 'category', 'to_string')):
            var = body[0][1]
            enum = self.var_enum(fn, var)
            if enum is None and fn is not None:
                for scrut_lo, scrut_hi, arms, n in self.enclosing_arms(fn, index):
                    if var in pattern_bindings(toks, arms[n][0], arms[n][1]):
                        for name in _idents(toks, scrut_lo, scrut_hi):
                            enum = enum or self.var_enum(fn, name)
            if enum is not None and (enum.code_map is not None or body[2][1] != 'code'):
                what = {'code': 'code', 'category': 'category', 'to_string': 'display'}[body[2][1]]
                return ('enum', what, enum, self.arm_variant_set(fn, index, var, enum))
            if body[2][1] == 'to_string' and fn is not None:
                types = next((t for name, t in fn.params if name == var), [])
                text = self.struct_text(fn.crate, types)
                if text is not None:
                    return ('lit', text)
            return ('unknown', text_of(toks, lo, hi))
        if len(body) == 3 and body[0][0] == 'id' and body[1][1] == '.' and body[2][1] in ('code', 'category', 'message'):
            return ('field', text_of(toks, lo, hi))
        if body[0][1] in ('match', 'if'):
            arms = []
            if body[0][1] == 'match':
                found = find_matches(toks, lo, hi)
                if found and found[0][0] == lo + 1:
                    for _, _, body_lo, body_hi in parse_match_arms(toks, found[0][2], found[0][3]):
                        arms.append(self.value(fn, rel, toks, body_lo, body_hi, index, depth + 1))
            if arms and all(a[0] == 'lit' for a in arms):
                return ('choice', arms)
            lits = [t[1] for t in body if t[0] == 'str']
            if lits and body[0][1] == 'if':
                return ('choice', [('lit', lit) for lit in lits])
        if (len(body) >= 9 and body[0][0] == 'id' and body[1][1] == '.' and body[2][1] == 'split'
                and body[3][1] == '(' and body[4][0] == 'char' and body[4][1] == "'.'" and body[5][1] == ')'
                and body[6][1] == '.' and body[7][1] == 'next'):
            return ('first_seg', self.name_value(fn, rel, toks, body[0][1], index, depth + 1))
        if len(body) == 3 and body[0][0] == 'id' and body[1][1] == '::' and body[2][0] == 'id':
            const = self.const_value(rel, body[2][1])
            if const is not None:
                return ('lit', const)
        return ('unknown', text_of(toks, lo, hi))

    def name_value(self, fn, rel, toks, name, index, depth=0):
        if depth > 6:
            return ('unknown', name)
        if fn is not None:
            for scrut_lo, scrut_hi, arms, n in self.enclosing_arms(fn, index):
                pat_lo, pat_hi = arms[n][0], arms[n][1]
                if name in pattern_bindings(toks, pat_lo, pat_hi):
                    for scrut_name in _idents(toks, scrut_lo, scrut_hi):
                        enum = self.var_enum(fn, scrut_name)
                        if enum is None:
                            continue
                        variants = self.pattern_variants(enum, toks, pat_lo, pat_hi) or []
                        if len(variants) == 1:
                            return ('payload', enum, variants[0], name)
                    return ('unknown', name)
            let = self.find_let(fn, index, name)
            if let is not None:
                return self.value(fn, rel, toks, let[0], let[1], index, depth + 1)
            for position, (pname, _) in enumerate(fn.params):
                if pname == name:
                    if self.closure_param(fn, index, name):
                        break
                    return ('param', position, name)
        if name.upper() == name and any(c.isalpha() for c in name):
            const = self.const_value(rel, name)
            if const is not None:
                return ('lit', const)
        return ('unknown', name)

    # ---- sinks and wrappers ---------------------------------------------

    def seed_constructors(self):
        """``G9Error::new`` is the root; the other constructors are found as its wrappers."""
        root = None
        for fn in self.fns:
            if fn.crate == 'g9-domain' and fn.impl_type == 'G9Error' and fn.impl_trait is None and fn.in_impl:
                self.ctor_keys.add(fn.key())
                if fn.name == 'new':
                    root = fn
        if root is None:
            raise ToolError('impl G9Error { fn new } not found under crates/g9-domain/src')
        names = [p[0] for p in root.params]
        if not {'category', 'code', 'message'} <= set(names):
            raise ToolError('G9Error::new(category, code, message) not recognised in ' + root.file)
        self.summaries[root.key()] = [{
            'cat': ('param', names.index('category'), 'category'),
            'code': ('param', names.index('code'), 'code'),
            'msg': ('param', names.index('message'), 'message'),
            'via': [], 'ctor': True,
        }]
        self.summary_names.add('new')

    def callee(self, rel, crate, toks, i, fn):
        """The function a call at ``toks[i]`` (its name token) refers to, or ``None``.

        Sets ``self.ambiguous`` to the candidates when several functions of that name could be meant.
        """
        self.ambiguous = []
        prev = toks[i - 1][1] if i else ''
        if prev in ('fn', '.'):
            return None
        candidates = self.fn_index.get(toks[i][1], ())
        if not candidates:
            return None
        found = self._callee(rel, crate, toks, i, fn, prev, candidates)
        if isinstance(found, list):
            self.ambiguous = found if len(found) > 1 else []
            return found[0] if len(found) == 1 else None
        return found

    def _callee(self, rel, crate, toks, i, fn, prev, candidates):
        qualifier = toks[i - 2][1] if prev == '::' and i >= 2 else None
        if qualifier is not None:
            if qualifier == 'Self' and fn is not None:
                qualifier = fn.impl_type
            typed = [c for c in candidates if c.impl_type == qualifier and c.in_impl]
            if typed:
                return [c for c in typed if c.crate == crate] or typed
            free = [c for c in candidates if not c.in_impl]
            pool = [c for c in free if c.crate == qualifier.replace('_', '-')]
            if not pool and qualifier in ('crate', 'super', 'self'):
                pool = [c for c in free if c.crate == crate]
            if not pool:
                pool = [c for c in free if c.crate == crate and os.path.basename(c.file)[:-3] == qualifier]
            if not pool:
                pool = [c for c in free if os.path.basename(c.file)[:-3] == qualifier
                        or os.path.basename(os.path.dirname(c.file)) == qualifier]
            return pool
        free = [c for c in candidates if not c.in_impl]
        local = [c for c in free if c.file == rel]
        if local:
            if len(local) == 1:
                return local
            return [c for c in local if fn is not None and fn.open < c.open < fn.close] or local
        return [c for c in free if c.crate == crate]

    @staticmethod
    def depends(value):
        if value[0] == 'param':
            return True
        if value[0] == 'fmt':
            return any(Analysis.depends(v) for v in value[2].values())
        if value[0] == 'choice':
            return any(Analysis.depends(v) for v in value[1])
        if value[0] == 'first_seg':
            return Analysis.depends(value[1])
        return False

    def substitute(self, value, args):
        if value[0] == 'param':
            return args[value[1]] if value[1] < len(args) else ('unknown', value[2])
        if value[0] == 'fmt':
            return ('fmt', value[1], {k: self.substitute(v, args) for k, v in value[2].items()}) + value[3:]
        if value[0] == 'choice':
            return ('choice', [self.substitute(v, args) for v in value[1]])
        if value[0] == 'first_seg':
            return ('first_seg', self.substitute(value[1], args))
        return value

    def analyse(self):
        self.seed_constructors()
        self.build_enum_maps()
        ground = {}
        for _ in range(10):
            changed = False
            for rel in sorted(self.files):
                toks = self.files[rel]
                crate = crate_of(rel)
                for i in range(1, len(toks) - 1):
                    if toks[i][0] != 'id' or toks[i + 1][1] != '(' or toks[i][1] not in self.summary_names:
                        continue
                    fn = self.innermost_fn(rel, i)
                    target = self.callee(rel, crate, toks, i, fn)
                    if target is None and any(c.key() in self.summaries for c in self.ambiguous):
                        self.unresolved_calls.add((rel, toks[i][2], 'call of %s(..) matches several functions '
                                                   'and one of them raises a code' % toks[i][1]))
                    if target is None or target.key() not in self.summaries:
                        continue
                    self.called.add(target.key())
                    close = match_close(toks, i + 1)
                    args = [self.value(fn, rel, toks, a, b, i) for a, b in split_top(toks, i + 2, close)]
                    if target.params and target.params[0][0] == 'self':
                        args = [('unknown', 'self')] + args
                    for template in list(self.summaries[target.key()]):
                        emission = {
                            'cat': self.substitute(template['cat'], args),
                            'code': self.substitute(template['code'], args),
                            'msg': self.substitute(template['msg'], args),
                            'via': template['via'] + [target.name],
                            'ctor': template['ctor'] and target.key() in self.ctor_keys,
                            'file': rel, 'line': toks[i][2], 'fn': fn.name if fn else '',
                        }
                        # A constructor of G9Error is summarised whatever it passes, so that its
                        # callers (not envelope.rs) are the raising sites of a fixed code.
                        summarise = fn is not None and (
                            self.depends(emission['code']) or self.depends(emission['cat'])
                            or fn.key() in self.ctor_keys)
                        if summarise:
                            entry = {k: emission[k] for k in ('cat', 'code', 'msg', 'via', 'ctor')}
                            bucket = self.summaries.setdefault(fn.key(), [])
                            if entry not in bucket:
                                bucket.append(entry)
                                self.summary_names.add(fn.name)
                                changed = True
                        else:
                            ground[(rel, i, repr(template))] = emission
            if not changed:
                break
        self.sink_calls = len({(k[0], k[1]) for k in ground})
        for key in sorted(ground, key=lambda k: (k[0], k[1], k[2])):
            self.finalise(ground[key])
        # A function that builds a code from its parameters but is never called from Rust (a Tauri
        # command, an entry point) still raises that code: its pattern is a row, at its own file.
        for fn in sorted(self.fns, key=Fn.key):
            if fn.key() in self.called or fn.key() in self.ctor_keys:
                continue
            for template in self.summaries.get(fn.key(), ()):
                if template['code'][0] in ('fmt', 'first_seg', 'choice'):
                    self.finalise(dict(template, file=fn.file, line=fn.toks[fn.open][2], fn=fn.name))
        for rel, line, text in sorted(self.unresolved_calls):
            self.unresolved.append({'file': rel, 'line': line, 'category': VARIES, 'expr': text})
        for fn in sorted(self.fns, key=Fn.key):
            if fn.key() in self.ctor_keys or not fn.params or fn.params[0][0] != 'self':
                continue
            if any(t['code'][0] == 'param' for t in self.summaries.get(fn.key(), ())):
                self.unresolved.append({'file': fn.file, 'line': fn.toks[fn.open][2], 'category': VARIES,
                                        'expr': 'method %s passes a code parameter on; calls `.%s(..)` are not '
                                                'followed' % (fn.name, fn.name)})
        self.scan_json_objects()
        self.scan_prefixed_literals()
        for rel in sorted(self.files):
            for kind, value, _ in self.files[rel]:
                if kind == 'str' and DOTTED.match(value):
                    self.literals.setdefault(value, set()).add(rel)

    # ---- from values to rows ---------------------------------------------

    def payload_instances(self, enum, variant):
        """String literals a variant is constructed with: ``[(literal, file)]``."""
        cache = self.__dict__.setdefault('_payload_cache', {})
        key = (enum.crate, enum.name, variant)
        if key not in cache:
            found = []
            for rel in sorted(self.files):
                toks = self.files[rel]
                for i in range(len(toks) - 5):
                    if (toks[i][1] == enum.name and toks[i + 1][1] == '::' and toks[i + 2][1] == variant
                            and toks[i + 3][1] == '(' and toks[i + 4][0] == 'str'
                            and toks[i + 5][1] in (')', '.', ',')):
                        found.append((toks[i + 4][1], rel))
            cache[key] = found
        return cache[key]

    def expand_text(self, value):
        """``[(text, extra_files, resolved)]`` for a code or message value."""
        kind = value[0]
        if kind == 'lit':
            return [(value[1], (), True)]
        if kind == 'choice':
            out = []
            for item in value[1]:
                out.extend(self.expand_text(item))
            return out
        if kind == 'first_seg':
            return [(text.split('.')[0], files, True) if ok else ('{first segment}', files, False)
                    for text, files, ok in self.expand_text(value[1])]
        if kind == 'payload':
            instances = self.payload_instances(value[1], value[2])
            if instances:
                return [(lit, (rel,), True) for lit, rel in instances]
            return [('{' + value[3] + '}', (), False)]
        if kind == 'fmt':
            template, mapping = value[1], value[2]
            slots, positional = [], 0
            for piece in re.split(r'(\{\{|\}\}|\{[^{}]*\})', template):
                if piece in ('{{', '}}'):
                    slots.append([(piece[0], (), True)])
                elif piece.startswith('{') and piece.endswith('}'):
                    name = piece[1:-1].split(':')[0].strip()
                    if name == '':
                        key = positional
                        positional += 1
                    elif name.isdigit():
                        key = int(name)
                    else:
                        key = name
                    label = '{' + (name if name and not name.isdigit() else '') + '}'
                    sub = mapping.get(key)
                    if sub is None:
                        slots.append([(label, (), False)])
                        continue
                    options = [(text, files, True) if ok else (label, files, False)
                               for text, files, ok in self.expand_text(sub)]
                    slots.append(options or [(label, (), False)])
                elif piece:
                    slots.append([(piece, (), True)])
            out = []
            for combo in itertools.islice(itertools.product(*slots), 512):
                out.append((''.join(p[0] for p in combo), tuple(f for p in combo for f in p[1]),
                            all(p[2] for p in combo)))
            return out
        if kind == 'param':
            return [('{' + value[2] + '}', (), False)]
        return [('{…}', (), False)]

    def message_texts(self, msg):
        """Normalised call-site messages a message value can take (empty: built at run time)."""
        if msg[0] == 'lit':
            return [normalise_message(msg[1])]
        if msg[0] == 'fmt':
            texts = self.expand_text(msg)
            if len(texts) <= 3:
                return sorted({normalise_message(t[0]) for t in texts})
            return [normalise_message(msg[1])]
        if msg[0] == 'choice':
            return sorted({normalise_message(t) for t, _, ok in self.expand_text(msg) if ok})
        if msg[0] == 'enum' and msg[1] == 'display':
            names = msg[3] if msg[3] is not None else list(msg[2].variants)
            return sorted({normalise_message(t) for v in names for t in self.enum_texts(msg[2], v)})
        return []

    def finalise(self, emission):
        code, cat, msg = emission['code'], emission['cat'], emission['msg']
        rel = emission['file']
        if code[0] == 'field':
            return      # an existing error's code passed on (wrapping), not a new code
        if code[0] == 'enum' and code[1] == 'code':
            self.finalise_enum(emission)
            return
        category = cat[1] if cat[0] == 'cat' else VARIES
        if code[0] == 'unknown':
            self.unresolved.append({'file': rel, 'line': emission['line'], 'category': category, 'expr': code[1]})
            return
        if code[0] == 'lit':
            pattern = 'constructor' if emission['ctor'] else 'helper'
        elif code[0] == 'choice':
            pattern = 'choice'
        else:
            pattern = 'format'
        origin = (code[3],) if code[0] == 'fmt' and len(code) > 3 else ()
        for text, files, resolved in self.expand_text(code):
            files = tuple(files) + origin
            messages = self.message_texts(msg)
            if msg[0] == 'fmt' and code[0] == 'fmt' and resolved:
                paired = self.paired_message(code, msg, text)
                messages = [paired] if paired else messages
            self.emissions.append({
                'code': text, 'category': category, 'messages': messages, 'pattern': pattern,
                'dynamic': not resolved, 'files': sorted(set((rel,) + tuple(files))), 'line': emission['line'],
            })

    def paired_message(self, code, msg, code_text):
        """Message of a formatted code when both share one placeholder value."""
        for name, sub in code[2].items():
            if name not in msg[2]:
                continue
            for text, _, ok in self.expand_text(sub):
                if not ok:
                    continue
                trial = self.expand_text(('fmt', code[1], dict(code[2], **{name: ('lit', text)})))
                if any(t == code_text for t, _, _ in trial):
                    filled = self.expand_text(('fmt', msg[1], dict(msg[2], **{name: ('lit', text)})))
                    if filled:
                        return normalise_message(filled[0][0])
        return None

    def finalise_enum(self, emission):
        _, _, enum, variants = emission['code']
        cat, msg = emission['cat'], emission['msg']
        rel = emission['file']
        for variant in (variants if variants is not None else list(enum.variants)):
            for code, sub_enum, sub_variant in self.enum_codes(enum, variant):
                if cat[0] == 'cat':
                    category = cat[1]
                elif cat[0] == 'enum' and cat[1] == 'category':
                    category = self.enum_category(cat[2], variant, sub_enum, sub_variant)
                else:
                    category = VARIES
                if msg[0] == 'enum':
                    messages = sorted({normalise_message(t) for t in self.enum_texts(enum, variant, sub_enum, sub_variant)})
                else:
                    messages = self.message_texts(msg)
                files = {rel, enum.code_file}
                source = sub_enum if sub_enum is not None else enum
                files.add(source.code_file)
                self.emissions.append({
                    'code': code, 'category': category, 'messages': messages, 'pattern': 'enum',
                    'dynamic': False, 'files': sorted(f for f in files if f), 'line': emission['line'],
                })

    # ---- other construction forms ----------------------------------------

    def scan_json_objects(self):
        """``json!({"category": "<CATEGORY>", "code": "<code>", ...})`` envelopes."""
        for rel in sorted(self.files):
            toks = self.files[rel]
            for i in range(len(toks) - 2):
                if not (toks[i][0] == 'str' and toks[i][1] == 'category' and toks[i + 1][1] == ':'
                        and toks[i + 2][0] == 'str' and toks[i + 2][1] in self.categories):
                    continue
                depth, j = 0, i
                while j > 0:
                    kind, value, _ = toks[j]
                    if kind == 'p' and value in CLOSE:
                        depth += 1
                    elif kind == 'p' and value in OPEN:
                        if depth == 0:
                            break
                        depth -= 1
                    j -= 1
                if toks[j][1] != '{':
                    continue
                fields = {}
                for a, b in split_top(toks, j + 1, match_close(toks, j)):
                    if b - a == 3 and toks[a][0] == 'str' and toks[a + 1][1] == ':' and toks[a + 2][0] == 'str':
                        fields[toks[a][1]] = toks[a + 2][1]
                if 'code' in fields:
                    self.emissions.append({
                        'code': fields['code'], 'category': fields['category'],
                        'messages': [normalise_message(fields['message'])] if 'message' in fields else [],
                        'pattern': 'json', 'dynamic': False, 'files': [rel], 'line': toks[i][2],
                    })

    def enclosing_constructor(self, toks, i, error_enums):
        """``Enum::Variant`` or ``G9Error::ctor`` whose arguments hold ``toks[i]``."""
        depth, j = 0, i - 1
        while j > 0:
            kind, value, _ = toks[j]
            if kind == 'p' and value in CLOSE:
                depth += 1
            elif kind == 'p' and value in OPEN:
                if depth == 0:
                    if value == '(' and j >= 3 and toks[j - 2][1] == '::' and toks[j - 1][0] == 'id':
                        owner = toks[j - 3][1]
                        if owner in error_enums or owner == 'G9Error':
                            return owner + '::' + toks[j - 1][1]
                    if value == '{':
                        return None
                else:
                    depth -= 1
            elif kind == 'p' and value == ';' and depth == 0:
                return None
            j -= 1
        return None

    def scan_prefixed_literals(self):
        """``[WARNING ]<code>: <sentence>`` literals: warnings, or reason tags inside an error."""
        error_enums = {name for name, defs in self.enums_by_name.items() if any(d.code_map is not None for d in defs)}
        for rel in sorted(self.files):
            toks = self.files[rel]
            for i, (kind, value, _) in enumerate(toks):
                if kind != 'str':
                    continue
                m = WARNING_PREFIX.match(value)
                if not m or m.group(1).split('.')[-1] in FILE_SUFFIXES:
                    continue
                owner = self.enclosing_constructor(toks, i, error_enums)
                table = self.reasons if owner else self.warnings
                entry = table.setdefault(m.group(1), {'files': set(), 'texts': set(), 'owners': set()})
                entry['files'].add(rel)
                entry['texts'].add(normalise_message(m.group(2)))
                if owner:
                    entry['owners'].add(owner)
        # helpers that build "WARNING {code}: {message}" from their arguments (g9_app::keyfile::warning)
        for fn in self.fns:
            names = [p[0] for p in fn.params]
            for i in range(fn.open, fn.close):
                if fn.toks[i][0] != 'str':
                    continue
                m = WARNING_HELPER.match(fn.toks[i][1])
                if not m or m.group(1) not in names:
                    continue
                position = names.index(m.group(1))
                for rel in sorted(self.files):
                    other = self.files[rel]
                    for j in range(1, len(other) - 1):
                        if other[j][1] != fn.name or other[j + 1][1] != '(' or other[j - 1][1] in ('fn', '.'):
                            continue
                        parts = split_top(other, j + 2, match_close(other, j + 1))
                        if position >= len(parts):
                            continue
                        caller = self.innermost_fn(rel, j)
                        val = self.value(caller, rel, other, parts[position][0], parts[position][1], j)
                        if val[0] != 'lit' or not DOTTED.match(val[1]):
                            continue
                        entry = self.warnings.setdefault(val[1], {'files': set(), 'texts': set(), 'owners': set()})
                        entry['files'].add(rel)
                        if position + 1 < len(parts):
                            text = self.value(caller, rel, other, parts[position + 1][0], parts[position + 1][1], j)
                            for t in self.message_texts(text):
                                entry['texts'].add(t)


# --------------------------------------------------------------------------
# Frontend (TypeScript)
# --------------------------------------------------------------------------

def ts_files(root):
    prod, mock = [], []
    for dirpath, dirnames, filenames in os.walk(os.path.join(root, TS_ROOT)):
        dirnames[:] = sorted(d for d in dirnames if d not in TS_SKIP_DIRS)
        for name in sorted(filenames):
            if not name.endswith(('.ts', '.tsx')) or name.endswith('.d.ts') or re.search(r'\.(test|spec)\.tsx?$', name):
                continue
            rel = os.path.relpath(os.path.join(dirpath, name), root).replace(os.sep, '/')
            (mock if TS_MOCK.search(rel) else prod).append(rel)
    return prod, mock


def ts_objects(src):
    """``[(open, close)]`` of every brace pair, skipping strings, templates and comments."""
    pairs, stack, modes = [], [], ['code']
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if modes[-1] == 'template':
            if c == '\\':
                i += 2
                continue
            if c == '`':
                modes.pop()
            elif src.startswith('${', i):
                modes.append('code')
                stack.append(('interp', i + 1))
                i += 2
                continue
            i += 1
            continue
        if c in ('"', "'"):
            j = i + 1
            while j < n and src[j] != c and src[j] != '\n':
                j += 2 if src[j] == '\\' else 1
            i = j + 1
            continue
        if c == '`':
            modes.append('template')
            i += 1
            continue
        if src.startswith('//', i):
            j = src.find('\n', i)
            i = n if j < 0 else j
            continue
        if src.startswith('/*', i):
            j = src.find('*/', i + 2)
            i = n if j < 0 else j + 2
            continue
        if c == '{':
            stack.append(('brace', i))
        elif c == '}' and stack:
            kind, start = stack.pop()
            if kind == 'interp':
                modes.pop()
            else:
                pairs.append((start, i))
        i += 1
    return pairs


def scan_typescript(root, categories):
    """``(rows, mock_codes)``: G9Error object literals in production TS; ``fail(...)`` codes of the mock."""
    prod, mock = ts_files(root)
    rows = []
    cats = '|'.join(map(re.escape, categories))
    cat_re = re.compile(r"\bcategory\s*:\s*['\"](%s)['\"]" % cats)
    for rel in prod:
        src = read_text(root, rel)
        pairs = None
        for m in cat_re.finditer(src):
            pairs = ts_objects(src) if pairs is None else pairs
            inner = [(a, b) for a, b in pairs if a < m.start() < b]
            if not inner:
                continue
            a, b = max(inner)
            body = src[a:b + 1]
            code = re.search(r"\bcode\s*:\s*['\"]([^'\"]+)['\"]", body)
            if not code:
                continue
            msg = re.search(r"\bmessage\s*:\s*(?:'([^']*)'|\"([^\"]*)\"|`([^`]*)`)", body)
            messages = []
            if msg:
                text = next(g for g in msg.groups() if g is not None)
                messages = [normalise_message(re.sub(r'\$\{\s*([A-Za-z_][A-Za-z0-9_.]*)\s*\}', r'{\1}', text))]
            rows.append({'code': code.group(1), 'category': m.group(1), 'messages': messages,
                         'pattern': 'frontend', 'dynamic': False, 'files': [rel],
                         'line': src.count('\n', 0, m.start()) + 1})
    mock_codes = {}
    fail_re = re.compile(r"\bfail\(\s*'(%s)'\s*,\s*'([^']+)'" % cats)
    for rel in mock:
        for m in fail_re.finditer(read_text(root, rel)):
            mock_codes.setdefault(m.group(2), set()).add(m.group(1))
    return rows, mock_codes


# --------------------------------------------------------------------------
# Catalogue
# --------------------------------------------------------------------------

def read_text(root, rel):
    try:
        with open(os.path.join(root, rel), encoding='utf-8') as handle:
            return handle.read()
    except (OSError, UnicodeDecodeError) as exc:
        raise ToolError('cannot read %s: %s' % (rel, exc)) from exc


def load_actions(root):
    path = os.path.join(root, ACTIONS_REL)
    if not os.path.isfile(path):
        raise ToolError('missing ' + ACTIONS_REL)
    try:
        with open(path, encoding='utf-8') as handle:
            data = json.load(handle)
    except (OSError, ValueError) as exc:
        raise ToolError('cannot read %s: %s' % (ACTIONS_REL, exc)) from exc
    if not isinstance(data, dict):
        raise ToolError(ACTIONS_REL + ' is not a JSON object')
    return data


def build(root):
    """Everything the catalogue and the checks need, from the files under ``root``."""
    analysis = Analysis(root)
    analysis.load()
    analysis.analyse()
    ts_rows, mock_codes = scan_typescript(root, analysis.categories)
    rows = {}
    for emission in analysis.emissions + ts_rows:
        key = (emission['category'], emission['code'])
        row = rows.setdefault(key, {'code': emission['code'], 'category': emission['category'],
                                    'files': set(), 'messages': {}, 'patterns': set(), 'dynamic': False})
        row['files'].update(emission['files'])
        row['patterns'].add(emission['pattern'])
        row['dynamic'] = row['dynamic'] or emission['dynamic']
        for message in emission['messages']:
            row['messages'][message] = row['messages'].get(message, 0) + 1
    for row in rows.values():
        row['scope'] = 'harness' if all(f.startswith(HARNESS_PREFIX) for f in row['files']) else 'shipped'
    return analysis, rows, mock_codes


def row_meaning(row, actions):
    entry = actions.get('codes', {}).get(row['code'], {})
    if isinstance(entry, dict) and entry.get('meaning'):
        return entry['meaning'] + ' (written)', 'written'
    messages = sorted(row['messages'], key=lambda m: (-row['messages'][m], m))
    if not messages:
        return 'No fixed message: it is built at run time (see the raising file).', 'none'
    shown = ' / '.join('"%s"' % m for m in messages[:3])
    if len(messages) > 3:
        shown += ' (and %d more)' % (len(messages) - 3)
    return shown, 'message'


def action_text(value):
    return ' '.join(value) if isinstance(value, list) else (value or '')


def row_action(row, actions):
    entry = actions.get('codes', {}).get(row['code'], {})
    if isinstance(entry, dict) and entry.get('action'):
        return '%s (`%s`)' % (action_text(entry['action']), entry.get('source', '')), 'code'
    if row['category'] == VARIES:
        return VARIES_ACTION, 'default'
    default = actions.get('category_defaults', {}).get(row['category'], {})
    if isinstance(default, dict) and default.get('action'):
        return action_text(default['action']), 'default'
    return '', 'missing'


def cell(text):
    return text.replace('\\', '\\\\').replace('|', '\\|').replace('\n', ' ')


def files_cell(files):
    return ', '.join('`%s`' % f for f in sorted(files))


def render(analysis, rows, actions):
    """The catalogue as Markdown (deterministic)."""
    categories = analysis.categories
    shipped = [r for r in rows.values() if r['scope'] == 'shipped']
    harness = sorted((r for r in rows.values() if r['scope'] == 'harness'), key=lambda r: (r['code'], r['category']))
    by_cat = {c: sorted((r for r in shipped if r['category'] == c and not r['dynamic']), key=lambda r: r['code'])
              for c in categories + [VARIES]}
    dynamic = sorted((r for r in shipped if r['dynamic']), key=lambda r: (r['code'], r['category']))
    out = []
    w = out.append

    def table(rows_, with_category=False):
        if with_category:
            w('| Code | Category | Meaning | What the operator does | Raised in |')
            w('|---|---|---|---|---|')
        else:
            w('| Code | Meaning | What the operator does | Raised in |')
            w('|---|---|---|---|')
        for r in rows_:
            middle = ' `%s` |' % r['category'] if with_category else ''
            w('| `%s` |%s %s | %s | %s |' % (r['code'], middle, cell(row_meaning(r, actions)[0]),
                                           cell(row_action(r, actions)[0]), files_cell(r['files'])))

    w('# Error-code catalogue')
    w('')
    w('Generated by `python3 -I scripts/gen_error_codes.py --write`; do not edit by hand. '
      'Every error that reaches the operator is a `G9Error` (`crates/g9-domain/src/envelope.rs`): one of '
      'the %d closed categories and a dotted code. Sources: the production Rust of `crates/*/src`, '
      '`apps/cli/src` and `apps/desktop/src-tauri/src` (test items removed) and the TypeScript of '
      '`apps/desktop/src` (tests and the mock transport excluded). Operator actions and written meanings: '
      '`docs/runbooks/error-codes.actions.json`.' % len(categories))
    w('')
    w('`python3 -I scripts/gen_error_codes.py` (no flag) exits 1 when this file differs from a regeneration '
      '(a code without a row, a row without a code), when the actions file names a code the source does not '
      'have or quotes a document that does not say it, and when %s names a code the source does not have.'
      % ', '.join('`%s`' % p for p in DOC_PATHS))
    w('')
    w('## Summary')
    w('')
    w('| Category | Rows | Action written for the code | Action from the category default |')
    w('|---|---|---|---|')
    totals = [0, 0]
    for cat in categories + [VARIES]:
        own = sum(1 for r in by_cat[cat] if row_action(r, actions)[1] == 'code')
        label = '`%s`' % cat if cat != VARIES else 'Category decided at run time'
        w('| %s | %d | %d | %d |' % (label, len(by_cat[cat]), own, len(by_cat[cat]) - own))
        totals = [totals[0] + len(by_cat[cat]), totals[1] + own]
    own = sum(1 for r in dynamic if row_action(r, actions)[1] == 'code')
    w('| Code patterns (part of the code is filled at run time) | %d | %d | %d |' % (len(dynamic), own, len(dynamic) - own))
    totals = [totals[0] + len(dynamic), totals[1] + own]
    w('| Total | %d | %d | %d |' % (totals[0], totals[1], totals[0] - totals[1]))
    w('')
    w('%d distinct codes reach the operator (a code raised with two categories has a row in each table); '
      '%d more are raised only by the evaluation harness; %d warning codes are listed apart.'
      % (len({r['code'] for r in shipped}), len({r['code'] for r in harness}), len(analysis.warnings)))
    w('')
    w('| How the code reaches `G9Error` | Rows | Rule |')
    w('|---|---|---|')
    for name, text in PATTERN_TEXT:
        w('| %s | %d | %s |' % (name, sum(1 for r in shipped if name in r['patterns']), text))
    w('')
    w('A row counts once for each way it is raised. **Meaning**: the call-site message, normalised '
      '(whitespace collapsed, `{x:?}` shown as `{x}`; a `{x}` is filled at run time), up to three messages, '
      'the most frequent first. A meaning marked "(written)" was written after reading the call site, because '
      'the message is built at run time or says nothing beyond the category. **What the operator does**: '
      'quoted from the document named after it; a row without a code-specific quote carries the guidance of '
      'its category from `docs/ui/ux-copy.md` section 5. **Raised in**: the files whose code produces the '
      'error (the call site; for an enum also the file of its `fn code`).')
    w('')
    for cat in categories:
        w('## `%s`' % cat)
        w('')
        if by_cat[cat]:
            table(by_cat[cat])
        else:
            w('No code in this category.')
        w('')
    w('## Category decided at run time')
    w('')
    w('The source passes on the category of the error that caused this one.')
    w('')
    if by_cat[VARIES]:
        table(by_cat[VARIES])
    else:
        w('None.')
    w('')
    w('## Code patterns')
    w('')
    w('Part of the code is filled at run time (`{x}`), typically `<area>.not_implemented` for an operation '
      'or an option that is PROPOSED. The concrete codes the source spells out are rows of the tables above.')
    w('')
    if dynamic:
        table(dynamic, with_category=True)
    else:
        w('None.')
    w('')
    w('## Warning codes')
    w('')
    w('Not errors: a successful result carries them in `warnings` as `<code>: <sentence>`, or as '
      '`WARNING <code>: <sentence>` from the bridge and the CLI. Listed so that the documents naming them can '
      'be checked; the operator reads the sentence (`docs/ui/ux-copy.md` section 6).')
    w('')
    w('| Code | Backend sentence | Raised in |')
    w('|---|---|---|')
    for code in sorted(analysis.warnings):
        entry = analysis.warnings[code]
        texts = sorted(entry['texts'])
        sentence = ' / '.join('"%s"' % t for t in texts[:2]) if texts else 'Built at run time.'
        w('| `%s` | %s | %s |' % (code, cell(sentence), files_cell(entry['files'])))
    w('')
    w('## Evaluation harness codes')
    w('')
    w(HARNESS_NOTE)
    w('')
    if harness:
        w('| Code | Category | Meaning | Raised in |')
        w('|---|---|---|---|')
        for r in harness:
            w('| `%s` | `%s` | %s | %s |' % (r['code'], r['category'], cell(row_meaning(r, actions)[0]),
                                            files_cell(r['files'])))
    else:
        w('None.')
    w('')
    w('## Method and blind spots')
    w('')
    w('| Item | Detail |')
    w('|---|---|')
    w('| Extraction | Rust is lexed (strings decoded, comments dropped). `G9Error::new` is the root sink; a function '
      'that passes a parameter on as the code or the category becomes a sink too (fixpoint over the call graph), '
      'and every call of a sink with a known code is a row. `let code = e.code(); match e { A => ctor(code) }` '
      'gives only the codes of `A` |')
    w('| Test code | `tests/` directories are not read; `#[cfg(test)]` and `#[test]` items, `#![cfg(test)]` files '
      'and the files of `#[cfg(test)] mod x;` are removed before extraction (%d files removed whole) |'
      % len(analysis.excluded))
    w('| Operation names | `notes.create` and the like are first arguments of `op(..)` in `crates/g9-app/src/ops.rs` '
      'and never reach a code position, so they are not rows (%d operation names read) |' % len(analysis.operations))
    w('| Unresolved, fails the check | a code position the analysis cannot evaluate: a closure parameter, a '
      'collection element, a call that matches several functions, a method that passes a code parameter on '
      '(calls `x.m(..)` are not followed) (%d now) |' % len(analysis.unresolved))
    w('| Not seen | `fn code` in a trait impl (only inherent impls are read); a `G9Error` decoded from stored JSON '
      '(an idempotent replay returns a code the source raised before); macros other than `format!` and `json!` '
      'that build the code |')
    w('| Wrapping | `G9Error { details, ..e }` and an `e.code` passed on keep an existing code and add no row |')
    w('| Unsupported, fails the check | a `G9Error { code: .. }` struct literal or an assignment `x.code = ..` in '
      'production code: the analysis reads constructors and code functions only, so such a code would be missing; '
      'build the error with a constructor |')
    w('| Frontend | object literals with `category` and `code` only; the mock transport (`ipc/transports/mock*.ts`) '
      'is a development double and is not catalogued |')
    w('')
    return '\n'.join(out) + '\n'


# --------------------------------------------------------------------------
# Checks
# --------------------------------------------------------------------------

def catalogue_rows(text):
    """``{key: set(row lines)}`` of a rendered catalogue. A row's key is its code, except in the
    "Summary" section, where it is ``summary of <CATEGORY>`` (a count row, not a code)."""
    rows = {}
    section = ''
    for line in text.splitlines():
        if line.startswith('## '):
            section = line[3:].strip()
            continue
        m = re.match(r'^\| `([^`]+)` \|', line)
        if m:
            key = 'summary of ' + m.group(1) if section == 'Summary' else m.group(1)
            rows.setdefault(key, set()).add(line)
    return rows


def stale_failures(current, fresh):
    if current == fresh:
        return []
    if current is None:
        return ['stale: %s does not exist; run with --write' % CATALOGUE_REL]
    old, new = catalogue_rows(current), catalogue_rows(fresh)
    failures = []
    for code in sorted(set(new) - set(old)):
        failures.append('stale: code %s has no row in %s; run with --write' % (code, CATALOGUE_REL))
    for code in sorted(set(old) - set(new)):
        failures.append('stale: %s has a row for %s, which the source no longer has; run with --write'
                        % (CATALOGUE_REL, code))
    for code in sorted(set(old) & set(new)):
        if old[code] != new[code]:
            if code.startswith('summary of '):
                failures.append('stale: the %s count differs from the source; run with --write' % code)
            else:
                failures.append('stale: the row of %s differs from the source; run with --write' % code)
    if not failures:
        failures.append('stale: %s differs from a regeneration outside the rows; run with --write' % CATALOGUE_REL)
    return failures


def normalise_doc(text):
    return re.sub(r'\s+', ' ', text.replace('**', '')).strip()


def actions_failures(root, analysis, rows, actions):
    failures = []
    codes = {r['code'] for r in rows.values()}
    defaults = actions.get('category_defaults')
    if not isinstance(defaults, dict):
        return ['actions: %s has no "category_defaults" object' % ACTIONS_REL]
    for cat in analysis.categories:
        if not isinstance(defaults.get(cat), dict) or not defaults[cat].get('action'):
            failures.append('actions: no category default for %s in %s' % (cat, ACTIONS_REL))
    for cat in sorted(set(defaults) - set(analysis.categories)):
        failures.append('actions: category default for %s, which is not a category' % cat)
    quoted = [('category ' + cat, entry) for cat, entry in sorted(defaults.items()) if isinstance(entry, dict)]
    for r in sorted(rows.values(), key=lambda r: (r['code'], r['category'])):
        if r['scope'] == 'shipped' and row_action(r, actions)[1] == 'missing':
            failures.append('actions: code %s (%s) has no operator action' % (r['code'], r['category']))
    entries = actions.get('codes', {})
    if not isinstance(entries, dict):
        return failures + ['actions: "codes" is not an object']
    for code, entry in sorted(entries.items()):
        if code not in codes:
            failures.append('actions: %s names %s, which the source does not have' % (ACTIONS_REL, code))
            continue
        if not isinstance(entry, dict) or not (entry.get('action') or entry.get('meaning')):
            failures.append('actions: entry %s has neither "action" nor "meaning"' % code)
            continue
        if entry.get('action'):
            quoted.append((code, entry))
            source = entry.get('source', '')
            if source and os.path.isfile(os.path.join(root, source)) and code not in read_text(root, source):
                failures.append('actions: the action of %s quotes %s, which does not mention %s' % (code, source, code))
        if entry.get('meaning'):
            basis = entry.get('basis', '')
            if not basis or not os.path.isfile(os.path.join(root, basis)):
                failures.append('actions: the written meaning of %s names no readable "basis" file' % code)
    for label, entry in quoted:
        if not isinstance(entry, dict) or not entry.get('action'):
            continue
        source = entry.get('source', '')
        if not source or not os.path.isfile(os.path.join(root, source)):
            failures.append('actions: the action of %s names no readable "source" document' % label)
            continue
        text = read_text(root, source)
        doc = normalise_doc(text)
        units = doc_units(text)
        for part in (entry['action'] if isinstance(entry['action'], list) else [entry['action']]):
            quote = normalise_doc(part)
            if quote not in doc:
                failures.append('actions: the action of %s is not a quotation of %s: "%s"' % (label, source, part))
            elif not label.startswith('category ') and not any(label in u and quote in u for u in units):
                failures.append('actions: the action of %s is quoted from %s, but not from the row or paragraph '
                                'that names %s: "%s"' % (label, source, label, part))
    return failures


def doc_units(text):
    """The units a quotation must come from, normalised: each table row on its own, every other block of
    text between blank lines as one paragraph."""
    units = []
    for block in re.split(r'\n\s*\n', text):
        lines = block.strip().splitlines()
        if lines and all(line.lstrip().startswith('|') for line in lines):
            units.extend(normalise_doc(line) for line in lines)
        else:
            units.append(normalise_doc(block))
    return units


def doc_tokens(text):
    """``[(line, token, backticked_span or None)]`` of dotted tokens in a Markdown document."""
    found = []
    for number, line in enumerate(text.splitlines(), 1):
        spans = [(m.start(), m.end(), m.group(1)) for m in re.finditer(r'`([^`]+)`', line)]
        for m in DOC_TOKEN.finditer(line):
            token = m.group(0).rstrip('.')
            if '{' in token or '}' in token or all(len(seg) == 1 for seg in token.split('.')):
                continue        # a placeholder, or an abbreviation such as e.g
            span = next((s[2] for s in spans if s[0] < m.start() < s[1]), None)
            found.append((number, token, span))
    return found


def proposed_areas(analysis):
    """First segments of the operation names in every ``PROPOSED_OPERATIONS`` list: the only areas a
    ``<area>.not_implemented`` refusal can name."""
    areas = set()
    for toks in analysis.files.values():
        for i in range(len(toks) - 1):
            if toks[i][:2] == ('id', 'PROPOSED_OPERATIONS') and toks[i + 1][:2] == ('p', ':'):
                j = i + 2
                while j < len(toks) and toks[j][:2] != ('p', ';'):
                    if toks[j][0] == 'str' and '.' in toks[j][1]:
                        areas.add(toks[j][1].split('.')[0])
                    j += 1
    return sorted(areas)


def dynamic_patterns(rows, areas=None):
    """Regular expressions of the dynamic codes. A placeholder named ``area`` matches only the areas of
    ``areas`` (when given); any other placeholder matches one or more code segments."""
    out = []
    for r in rows.values():
        if not r['dynamic']:
            continue
        def fill(m):
            if areas and 'area' in m.group(0):
                return '(?:' + '|'.join(re.escape(a) for a in areas) + ')'
            return '[a-z0-9_.]+'
        out.append(re.compile('^' + re.sub(r'\\\{[^}]*\\\}', fill, re.escape(r['code'])) + '$'))
    return out


def classify_reference(token, analysis, codes, dynamic_res, allow):
    if token in codes:
        return 'code'
    if token in analysis.warnings:
        return 'warning'
    if token in analysis.operations:
        return 'operation'
    if token.split('.')[-1] in FILE_SUFFIXES:
        return 'file'
    if any(rx.match(token) for rx in dynamic_res):
        return 'pattern'
    if token in allow:
        return 'allowed'
    return None


def reference_failures(root, analysis, rows, actions):
    """Dead references and category contradictions in ``DOC_PATHS``: ``(failures, findings)``."""
    failures, findings = [], []
    allow = actions.get('doc_reference_allow', {})
    if not isinstance(allow, dict):
        return ['actions: "doc_reference_allow" is not an object'], findings
    dynamic_res = dynamic_patterns(rows, proposed_areas(analysis))
    by_code = {}
    for r in rows.values():
        by_code.setdefault(r['code'], set()).add(r['category'])
    seen = set()
    for rel in DOC_PATHS:
        if not os.path.isfile(os.path.join(root, rel)):
            failures.append('dead-reference: %s is missing' % rel)
            continue
        for number, token, span in doc_tokens(read_text(root, rel)):
            kind = classify_reference(token, analysis, by_code, dynamic_res, allow)
            seen.add(token)
            if kind is None:
                failures.append('dead-reference: %s:%d names %s, which the source does not have (not a code, '
                                'warning, operation or file; if it is not a code, allow-list it with a reason in %s)'
                                % (rel, number, token, ACTIONS_REL))
                continue
            m = re.match(r'^([A-Z_]+) ([a-z][a-z0-9_.]+)$', (span or '').strip())
            if (m and m.group(2) == token and kind == 'code' and m.group(1) in analysis.categories
                    and not by_code[token] & {m.group(1), VARIES}):
                failures.append('category-contradiction: %s:%d says %s is %s; the source raises it as %s'
                                % (rel, number, token, m.group(1), ', '.join(sorted(by_code[token]))))
            findings.append((rel, number, token, kind))
    for token, reason in sorted(allow.items()):
        if not isinstance(reason, str) or not reason.strip():
            failures.append('actions: doc_reference_allow entry %s has no reason' % token)
        elif token not in seen:
            failures.append('actions: doc_reference_allow entry %s is not named in any checked document' % token)
        elif classify_reference(token, analysis, by_code, dynamic_res, {}) is not None:
            failures.append('actions: doc_reference_allow entry %s is now resolved by the source; remove it' % token)
    return failures, findings


def unresolved_failures(analysis):
    return ['unresolved: %s:%d passes a code the analysis cannot follow (%s)' % (u['file'], u['line'], u['expr'])
            for u in analysis.unresolved] + construction_failures(analysis)


def construction_failures(analysis):
    """A code set outside a constructor: a ``G9Error { code: .. }`` struct literal, or an assignment
    ``x.code = ..``. The analysis reads constructors and code functions only, so these would make a row
    silently missing; they fail the check instead (write the error with a constructor)."""
    failures = []
    for rel in sorted(analysis.files):
        toks = analysis.files[rel]
        for i in range(len(toks) - 3):
            kind, value, line = toks[i]
            if kind == 'id' and value == 'G9Error' and toks[i + 1][:2] == ('p', '{') \
                    and not (i > 0 and toks[i - 1][1] in ('impl', 'struct', 'enum', 'for', '>')):
                close = match_close(toks, i + 1)
                depth = 0
                for j in range(i + 1, close):
                    k2, v2, l2 = toks[j]
                    if k2 == 'p' and v2 in OPEN:
                        depth += 1
                    elif k2 == 'p' and v2 in CLOSE:
                        depth -= 1
                    elif depth == 1 and k2 == 'id' and v2 == 'code' and toks[j + 1][:2] == ('p', ':') \
                            and toks[j + 2][:2] != ('p', ':'):
                        failures.append('unsupported: %s:%d sets a code in a G9Error struct literal, which the '
                                        'catalogue cannot see; build the error with a constructor' % (rel, l2))
            if kind == 'p' and value == '.' and toks[i + 1][:2] == ('id', 'code') \
                    and toks[i + 2][:2] == ('p', '=') and toks[i + 3][:2] != ('p', '='):
                failures.append('unsupported: %s:%d assigns .code, which the catalogue cannot see; build the '
                                'error with a constructor' % (rel, line))
    return failures


# --------------------------------------------------------------------------
# Reports: stats and spot check
# --------------------------------------------------------------------------

def stats(analysis, rows, mock_codes, actions):
    shipped = [r for r in rows.values() if r['scope'] == 'shipped']
    all_codes = {}
    for r in rows.values():
        all_codes.setdefault(r['code'], set()).add(r['category'])
    out = {
        'files_read': len(analysis.files),
        'files_removed_as_test': sorted(r for r, _ in analysis.excluded),
        'sink_call_sites': analysis.sink_calls,
        'operations_read': len(analysis.operations),
        'rows_shipped': len(shipped),
        'distinct_codes_shipped': len({r['code'] for r in shipped}),
        'rows_harness': sum(1 for r in rows.values() if r['scope'] == 'harness'),
        'distinct_codes_harness': len({r['code'] for r in rows.values() if r['scope'] == 'harness'}),
        'warning_codes': len(analysis.warnings),
        'message_tags': sorted(analysis.reasons),
        'rows_per_category': {}, 'rows_per_pattern': {}, 'rows_on_category_default': 0,
        'meanings': {'message': 0, 'written': 0, 'none': 0},
        'codes_with_several_categories': sorted(c for c, cats in all_codes.items() if len(cats) > 1),
        'dynamic_patterns': sorted(r['code'] for r in shipped if r['dynamic']),
        'unresolved': analysis.unresolved,
        'blind_spots': sorted(set(analysis.blind)),
        'mock_only_codes': sorted(c for c in mock_codes if c not in all_codes),
        'mock_category_differs': sorted('%s: mock %s, source %s' % (c, ','.join(sorted(cats)),
                                                                     ','.join(sorted(all_codes[c])))
                                        for c, cats in mock_codes.items()
                                        if c in all_codes and not cats <= all_codes[c]),
        'non_dotted_codes': sorted(c for c in all_codes if not DOTTED.match(c) and '{' not in c),
    }
    for r in shipped:
        out['rows_per_category'][r['category']] = out['rows_per_category'].get(r['category'], 0) + 1
        for p in r['patterns']:
            out['rows_per_pattern'][p] = out['rows_per_pattern'].get(p, 0) + 1
        if row_action(r, actions)[1] != 'code':
            out['rows_on_category_default'] += 1
        out['meanings'][row_meaning(r, actions)[1]] += 1
    out['rows_per_category'] = dict(sorted(out['rows_per_category'].items()))
    out['rows_per_pattern'] = dict(sorted(out['rows_per_pattern'].items()))
    return out


def spot_check(root, analysis, rows, seed, count):
    """Sample dotted string literals from a raw text search of the production Rust files."""
    found = {}
    for rel in analysis.rust_files():
        for number, line in enumerate(read_text(root, rel).splitlines(), 1):
            for m in RAW_DOTTED_LITERAL.finditer(line):
                found.setdefault(m.group(1), []).append((rel, number, line.strip()))
    values = sorted(found)
    sample = random.Random(seed).sample(values, min(count, len(values)))
    by_code = {}
    for r in rows.values():
        by_code.setdefault(r['code'], set()).add(r['category'])
    result = []
    for value in sample:
        rel, number, line = found[value][0]
        if value in by_code:
            verdict = 'catalogued: ' + ', '.join(sorted(by_code[value]))
        elif value in analysis.warnings:
            verdict = 'warning code (Warning codes table)'
        elif value in analysis.operations:
            verdict = 'operation name (crates/g9-app/src/ops.rs)'
        elif value in analysis.reasons:
            verdict = 'message tag inside ' + ', '.join(sorted(analysis.reasons[value]['owners']))
        elif value.split('.')[-1] in FILE_SUFFIXES:
            verdict = 'file name'
        elif value not in analysis.literals:
            verdict = 'only in test items or comments (removed before extraction)'
        else:
            verdict = 'not at a code position (see the line)'
        result.append({'literal': value, 'verdict': verdict, 'first_seen': '%s:%d' % (rel, number),
                       'line': line[:160], 'occurrences': len(found[value])})
    return {'seed': seed, 'population': len(values), 'sample': result}


# --------------------------------------------------------------------------
# Main
# --------------------------------------------------------------------------

def main(argv=None):
    parser = argparse.ArgumentParser(description='Generate or check docs/runbooks/error-codes.md.')
    parser.add_argument('--root', default=os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                        help='repository root (default: the parent of scripts/)')
    parser.add_argument('--write', action='store_true', help='regenerate the catalogue')
    parser.add_argument('--stats', action='store_true', help='print extraction statistics as JSON and exit 0')
    parser.add_argument('--spot-check', action='store_true',
                        help='print a seeded sample of dotted literals with their classification and exit 0')
    parser.add_argument('--seed', type=int, default=5023)
    parser.add_argument('--count', type=int, default=25)
    args = parser.parse_args(argv)
    root = os.path.abspath(args.root)
    try:
        actions = load_actions(root)
        analysis, rows, mock_codes = build(root)
        if args.stats:
            print(json.dumps(stats(analysis, rows, mock_codes, actions), indent=2, sort_keys=True))
            return 0
        if args.spot_check:
            print(json.dumps(spot_check(root, analysis, rows, args.seed, args.count), indent=2, ensure_ascii=False))
            return 0
        fresh = render(analysis, rows, actions)
        failures = unresolved_failures(analysis) + actions_failures(root, analysis, rows, actions)
        failures += reference_failures(root, analysis, rows, actions)[0]
        target = os.path.join(root, CATALOGUE_REL)
        if args.write:
            with open(target, 'w', encoding='utf-8', newline='\n') as handle:
                handle.write(fresh)
            print('wrote %s: %d rows' % (CATALOGUE_REL, len(rows)))
        else:
            current = read_text(root, CATALOGUE_REL) if os.path.isfile(target) else None
            failures = stale_failures(current, fresh) + failures
    except ToolError as exc:
        print('error: %s' % exc, file=sys.stderr)
        return 2
    if failures:
        for line in failures:
            print('FAIL ' + line)
        print('%d failure(s)' % len(failures))
        return 1
    if not args.write:
        shipped = sum(1 for r in rows.values() if r['scope'] == 'shipped')
        print('PASS: %s is current (%d rows reaching the operator, %d harness rows, %d warning codes); every '
              'row has an action; no dead reference in %d documents'
              % (CATALOGUE_REL, shipped, len(rows) - shipped, len(analysis.warnings), len(DOC_PATHS)))
    return 0


if __name__ == '__main__':
    sys.exit(main())
