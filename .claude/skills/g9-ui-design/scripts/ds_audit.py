#!/usr/bin/env python3
"""Design-system audit of apps/desktop: token coverage, undefined and unused classes, inline styles.

Usage: python3 ds_audit.py <repo>/apps/desktop/src            # JSON report on stdout
       python3 ds_audit.py <repo>/apps/desktop/src --check    # exit 1 on a finding that must be fixed

A finding that must be fixed: a class used in a component that no stylesheet defines, a hex or
rgb()/hsl() colour outside tokens.css, or a font size that is neither a token nor relative.
Unused classes and raw px values are reported for review; they do not fail the check."""
import re, sys, os, json, collections
root = sys.argv[1]
check = '--check' in sys.argv[2:]
theme = os.path.join(root, 'app', 'theme')
css_files = []
for d, _, fs in os.walk(theme):
    for f in fs:
        if f.endswith('.css'):
            css_files.append(os.path.join(d, f))
tsx_files = []
for d, _, fs in os.walk(root):
    for f in fs:
        if f.endswith('.tsx') and not f.endswith('.test.tsx'):
            tsx_files.append(os.path.join(d, f))

def strip_comments(s):
    return re.sub(r'/\*.*?\*/', '', s, flags=re.S)

css = {f: strip_comments(open(f).read()) for f in css_files}
defined = collections.Counter()
for f, s in css.items():
    for sel_block in re.findall(r'([^{}]+)\{', s):
        for c in re.findall(r'\.([a-zA-Z][a-zA-Z0-9_-]*)', sel_block):
            if not re.match(r'^\d', c):
                defined[c] += 1

used = collections.Counter()
used_where = collections.defaultdict(set)
for f in tsx_files:
    s = open(f).read()
    for m in re.finditer(r'className=(?:"([^"]*)"|\{`([^`]*)`\}|\{\'([^\']*)\'\}|\{([^}]*)\})', s):
        raw = next(g for g in m.groups() if g is not None)
        # string literals inside expressions
        lits = re.findall(r"'([^']*)'|\"([^\"]*)\"|`([^`]*)`", raw) if m.group(4) is not None else [(raw, '', '')]
        for lit in lits:
            text = next((x for x in lit if x), '') if isinstance(lit, tuple) else lit
            text = re.sub(r'\$\{[^}]*\}', ' ', text)
            for c in text.split():
                # A class name here is lower-case; an ALL_CAPS token in a className expression is an enum value being compared.
                if re.match(r'^[a-z][a-zA-Z0-9_-]*$', c) and not c.isupper():
                    used[c] += 1
                    used_where[c].add(os.path.relpath(f, root))

undefined = sorted(c for c in used if c not in defined)
unused = sorted(c for c in defined if c not in used)

# token coverage in non-token stylesheets
def count(pattern, s):
    return len(re.findall(pattern, s))
report = {'css_files': len(css_files), 'tsx_files': len(tsx_files), 'classes_defined': len(defined), 'classes_used': len(used)}
per_file = {}
for f, s in css.items():
    name = os.path.relpath(f, theme)
    if name in ('tokens.css',):
        continue
    hexes = re.findall(r'#[0-9a-fA-F]{3,8}\b', s)
    raw_colour = re.findall(r'\b(?:rgb|rgba|hsl|hsla)\(', s)
    oklch = re.findall(r'oklch\(', s)
    px = [m for m in re.findall(r'(?<![\w.-])(\d+(?:\.\d+)?)px', s)]
    px_big = [p for p in px if float(p) > 3]
    font_sizes = re.findall(r'font-size:\s*([^;]+);', s)
    font_raw = [v for v in font_sizes if 'var(' not in v and v.strip() not in ('inherit', '100%', '1em')]
    per_file[name] = {'hex': len(hexes), 'rgb_hsl': len(raw_colour), 'oklch_literals': len(oklch), 'px_over_3': len(px_big), 'font_size_raw': font_raw, 'var_uses': count(r'var\(--', s)}
inline = {}
for f in tsx_files:
    s = open(f).read()
    n = len(re.findall(r'style=\{', s))
    if n:
        inline[os.path.relpath(f, root)] = n
report['per_file'] = per_file
report['inline_style_props'] = inline
report['undefined_classes'] = {c: sorted(used_where[c]) for c in undefined}
report['unused_classes'] = unused
print(json.dumps(report, indent=1))
if check:
    # `badge-` is the prefix of a computed class name (`badge-${tone}`), not a class; `badge-state` is the
    # tone of a state stamp, which is styled by `[data-state]` and has no rule of its own.
    undefined_real = [c for c in undefined if not c.endswith('-') and c not in ('badge-state',)]
    colours = sum(v['hex'] + v['rgb_hsl'] for v in per_file.values())
    if undefined_real or colours:
        sys.stderr.write(f"design-system check failed: undefined classes {undefined_real}, colours outside tokens {colours}\n")
        sys.exit(1)
