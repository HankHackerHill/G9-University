#!/usr/bin/env python3
"""gen_metrics.py - counts of the repository, generated into docs/release/metrics.md (step DOC-2).

Every number is computed from the files of the tree; the output holds no date and no commit id, so
the same tree gives the same bytes. Default mode compares docs/release/metrics.md with what the tree
gives and fails when it is stale; --write regenerates it.

Usage (repository root; standard library only; no network):
    python3 -I scripts/gen_metrics.py [--root DIR] [--write]

Exit codes: 0 the file is current (or was written); 1 the file is missing or stale, or a cross-check
of the operation registry failed (nothing is written then); 2 usage or environment error (a source
the counts need is missing or cannot be read).

Files: every file below DIR except below a directory named .git, target, node_modules, dist,
coverage or __pycache__, and except apps/desktop/src-tauri/gen and .claude/worktrees (the
.gitignore entries that hold build outputs and local copies). Lines are physical lines: the number
of line ends, plus one for a last line without one; blank and comment lines count.

What is counted, and from where
  crates           members and default-members of the root Cargo.toml [workspace]; package name
                   from each member's Cargo.toml
  Rust lines       per member: src = .rs below <member>/src; tests = .rs below <member>/tests plus
                   the [[test]] paths of its Cargo.toml that lie outside the member directory;
                   other = every other .rs file below the member (examples, benches, build.rs).
                   A .rs file in no member and named by no [[test]], [[bin]], [[example]] or [[bench]]
                   path is listed as not built.
  #[test]          `#[test]` attributes in those files, outside comments and string literals. This is
                   not the number of tests cargo runs (doc tests, ignored tests, generated tests).
  frontend         .ts and .tsx below apps/desktop, not below src-tauri: test files (*.test.ts,
                   *.test.tsx), test support (below apps/desktop/src/test), application source (the rest
                   of apps/desktop/src), build configuration (the rest); .css below apps/desktop
  migrations       crates/g9-storage/src/migrations/*.sql; those embedded by migrations.rs (include_str!)
  tables           CREATE [TEMP] [VIRTUAL] TABLE statements in those files, outside comments and string
                   literals; tables after the last migration = created, less DROP TABLE, with
                   ALTER TABLE ... RENAME TO applied. The Migrator's own schema_migrations table is
                   created by migrations.rs (BOOTSTRAP_SQL), not by a migration, and is not counted
  ADRs             docs/adr/ADR-*.md
  JSON schemas     *.json below schemas/; in schemas/ipc: *.request.schema.json, *.response.schema.json
                   and the shared rest
  skill packages   directories of .agents/skills that hold a SKILL.md
  competency packs directories of data/competency-packs that hold a pack.json
  scripts          files directly in scripts/ (by extension), scripts/e2e, scripts/tests/test_*.py
  operations       routed: the string literals of the `op(...)` entries of `OPERATIONS` in
                   crates/g9-app/src/ops.rs; PROPOSED: the `PROPOSED_OPERATIONS` lists of the modules
                   that crates/g9-app/src/dispatch.rs names (`<module>::PROPOSED_OPERATIONS`), read from
                   crates/g9-app/src/<module>.rs or <module>/mod.rs. All parsing is static: no cargo.

Cross-checks (exit 1 when one fails, named in the message)
  operations.unique          no routed name twice, no PROPOSED name twice
  operations.disjoint        no name both routed and PROPOSED
  operations.cli-areas       apps/cli/src/commands/ops.rs (`g9 ops`) lists the PROPOSED lists of the same
                             modules as dispatch.rs (checked when that file exists)
  operations.test-assertion  the count equals `assert_eq!(OPERATIONS.len(), N)` in ops.rs (checked when
                             that assertion exists)
  operations.parse           every `op(` entry of OPERATIONS yields exactly one name
"""
from __future__ import annotations

import argparse
import difflib
import importlib.util
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = "docs/release/metrics.md"
SKIP_NAMES = frozenset({".git", "target", "node_modules", "dist", "coverage", "__pycache__"})
SKIP_PATHS = frozenset({"apps/desktop/src-tauri/gen", ".claude/worktrees"})
OPS_RS = "crates/g9-app/src/ops.rs"
DISPATCH_RS = "crates/g9-app/src/dispatch.rs"
CLI_OPS_RS = "apps/cli/src/commands/ops.rs"
APP_SRC = "crates/g9-app/src"
MIGRATIONS_DIR = "crates/g9-storage/src/migrations"
MIGRATIONS_RS = "crates/g9-storage/src/migrations.rs"

RUST_TOKEN = re.compile(r"//|/\*|b?r#*\"|\"|'")
RUST_CHAR = re.compile(r"'(?:\\(?:x[0-9A-Fa-f]{2}|u\{[0-9A-Fa-f_]{1,8}\}|.)|[^\\'\n])'")
IDENT_CHAR = re.compile(r"[A-Za-z0-9_]")
STRING_LITERAL = re.compile(r'"((?:[^"\\]|\\.)*)"')
TEST_ATTR = re.compile(r"#\s*\[\s*test\s*\]")


class Fail(Exception):
    """A source the counts need is missing or cannot be read (exit 2)."""


class CrossCheck(Exception):
    """The operation registry disagrees with itself (exit 1)."""


# ---------------------------------------------------------------- files

def _doc_refs_tree():
    """The tree walk of scripts/check_doc_refs.py, loaded from its file next to this one (no sys.path
    entry is added, so `python3 -I` keeps a planted module out)."""
    path = Path(__file__).resolve().with_name("check_doc_refs.py")
    spec = importlib.util.spec_from_file_location("g9_check_doc_refs", path)
    if spec is None or spec.loader is None:
        raise Fail(f"{path}: cannot load the .gitignore-aware tree walk")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.Tree


def walk(root: Path) -> list[str]:
    """Relative POSIX paths of every counted file, sorted: the files git would hold (the root and nested
    .gitignore files apply, as in scripts/check_doc_refs.py), so an ignored local file such as a log
    never changes a count, minus the fixed skips."""
    out: list[str] = []
    for child in sorted(_doc_refs_tree()(root).files):
        parts = child.split("/")
        if SKIP_NAMES.intersection(parts[:-1]) or any(child == p or child.startswith(p + "/") for p in SKIP_PATHS):
            continue
        if (root / child).is_symlink():
            continue
        out.append(child)
    return out


def lines_of(root: Path, rel: str) -> int:
    data = (root / rel).read_bytes()
    return data.count(b"\n") + (1 if data and not data.endswith(b"\n") else 0)


def read(root: Path, rel: str) -> str:
    try:
        return (root / rel).read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as exc:
        raise Fail(f"{rel}: cannot read: {exc}") from exc


# ---------------------------------------------------------------- Rust text

def rust_mask(text: str) -> tuple[str, str]:
    """(code, masked): both as long as `text`. `code` has every comment replaced by blanks (line ends
    kept); `masked` also has the inside of every string and char literal replaced by blanks."""
    code = list(text)
    masked = list(text)
    i, n = 0, len(text)

    def blank(a: int, b: int, *targets: list[str]) -> None:
        for t in targets:
            for k in range(a, min(b, n)):
                if t[k] != "\n":
                    t[k] = " "

    while True:
        m = RUST_TOKEN.search(text, i)
        if m is None:
            break
        tok, s = m.group(0), m.start()
        if tok[0] in "br" and s > 0 and IDENT_CHAR.match(text[s - 1]):
            i = text.index('"', s)
            tok, s = '"', i
        if tok == "//":
            end = text.find("\n", s)
            end = n if end < 0 else end
            blank(s, end, code, masked)
            i = end
        elif tok == "/*":
            depth, j = 1, s + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            blank(s, j, code, masked)
            i = j
        elif tok == "'":
            c = RUST_CHAR.match(text, s)
            if c:
                blank(s + 1, c.end() - 1, masked)
                i = c.end()
            else:
                i = s + 1
        elif tok == '"':
            j = s + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            blank(s + 1, j, masked)
            i = j + 1
        else:  # raw string
            close = '"' + "#" * tok.count("#")
            end = text.find(close, m.end())
            end = n if end < 0 else end
            blank(m.end(), end, masked)
            i = end + len(close)
    return "".join(code), "".join(masked)


def bracket_span(masked: str, open_at: int) -> int:
    """Index just past the `]` that closes the `[` at `open_at` (in comment- and literal-free text)."""
    depth = 0
    for k in range(open_at, len(masked)):
        if masked[k] == "[":
            depth += 1
        elif masked[k] == "]":
            depth -= 1
            if depth == 0:
                return k + 1
    raise Fail("unbalanced brackets")


def const_strings(root: Path, rel: str, decl: str) -> tuple[list[str], str, int, int]:
    """String literals of the array literal that follows `decl` (a regex ending at its `[`)."""
    code, masked = rust_mask(read(root, rel))
    m = re.search(decl, masked)
    if m is None:
        raise Fail(f"{rel}: no `{decl}` found")
    end = bracket_span(masked, m.end() - 1)
    names = [s for s in STRING_LITERAL.findall(code[m.end():end])]
    return names, masked, m.end(), end


def module_file(root: Path, module: str) -> str:
    for rel in (f"{APP_SRC}/{module}.rs", f"{APP_SRC}/{module}/mod.rs"):
        if (root / rel).is_file():
            return rel
    raise Fail(f"{DISPATCH_RS} names module `{module}`, but neither {APP_SRC}/{module}.rs nor {APP_SRC}/{module}/mod.rs exists")


def operations(root: Path) -> dict:
    routed, masked, a, b = const_strings(root, OPS_RS, r"\bOPERATIONS\s*:\s*&\s*\[\s*OpSpec\s*\]\s*=\s*&\s*\[")
    calls = len(re.findall(r"(?<![A-Za-z0-9_])op\s*\(", masked[a:b]))
    if calls != len(routed):
        raise CrossCheck(f"operations.parse: {OPS_RS} OPERATIONS has {calls} `op(` entries but {len(routed)} string literals")
    if len(set(routed)) != len(routed):
        dupes = sorted({n for n in routed if routed.count(n) > 1})
        raise CrossCheck(f"operations.unique: routed more than once in {OPS_RS}: {', '.join(dupes)}")
    asserted = re.search(r"assert_eq!\s*\(\s*OPERATIONS\s*\.\s*len\s*\(\s*\)\s*,\s*(\d+)\s*\)", rust_mask(read(root, OPS_RS))[0])
    if asserted and int(asserted.group(1)) != len(routed):
        raise CrossCheck(
            f"operations.test-assertion: {OPS_RS} asserts OPERATIONS.len() == {asserted.group(1)}, the registry has {len(routed)}"
        )

    _code, dmasked = rust_mask(read(root, DISPATCH_RS))
    modules = sorted(set(re.findall(r"(?<![A-Za-z0-9_])([a-z_][a-z0-9_]*)\s*::\s*PROPOSED_OPERATIONS\b", dmasked)))
    if not modules:
        raise Fail(f"{DISPATCH_RS}: no `<module>::PROPOSED_OPERATIONS` found")
    if (root / CLI_OPS_RS).is_file():
        _c, cmasked = rust_mask(read(root, CLI_OPS_RS))
        cli = sorted(set(re.findall(r"g9_app\s*::\s*([a-z_][a-z0-9_]*)\s*::\s*PROPOSED_OPERATIONS\b", cmasked)))
        if cli != modules:
            raise CrossCheck(
                f"operations.cli-areas: {DISPATCH_RS} routes the PROPOSED lists of {', '.join(modules)}; "
                f"{CLI_OPS_RS} (`g9 ops`) lists {', '.join(cli) or 'none'}"
            )
    proposed: dict[str, list[str]] = {}
    for module in modules:
        rel = module_file(root, module)
        names, *_ = const_strings(root, rel, r"\bPROPOSED_OPERATIONS\s*:\s*&\s*\[\s*&\s*str\s*\]\s*=\s*&\s*\[")
        proposed[module] = names
    flat = [n for names in proposed.values() for n in names]
    if len(set(flat)) != len(flat):
        dupes = sorted({n for n in flat if flat.count(n) > 1})
        raise CrossCheck(f"operations.unique: PROPOSED more than once: {', '.join(dupes)}")
    both = sorted(set(flat) & set(routed))
    if both:
        raise CrossCheck(f"operations.disjoint: both routed and PROPOSED: {', '.join(both)}")
    return {"routed": routed, "proposed": proposed, "asserted": asserted is not None}


# ---------------------------------------------------------------- crates

def load_toml(root: Path, rel: str) -> dict:
    try:
        return tomllib.loads(read(root, rel))
    except tomllib.TOMLDecodeError as exc:
        raise Fail(f"{rel}: not valid TOML: {exc}") from exc


def norm(parts: list[str]) -> str:
    out: list[str] = []
    for p in parts:
        if p in ("", "."):
            continue
        if p == "..":
            if not out:
                raise Fail("a Cargo.toml path leaves the repository")
            out.pop()
        else:
            out.append(p)
    return "/".join(out)


def crates(root: Path, files: list[str]) -> dict:
    if not (root / "Cargo.toml").is_file():
        raise Fail("Cargo.toml: no such file")
    ws = load_toml(root, "Cargo.toml").get("workspace", {})
    members = ws.get("members", [])
    defaults = set(ws.get("default-members", members))
    rs_files = [f for f in files if f.endswith(".rs")]
    rows, claimed = [], set()
    for member in members:
        manifest = load_toml(root, f"{member}/Cargo.toml")
        name = manifest.get("package", {}).get("name", member)
        cat: dict[str, set[str]] = {"src": set(), "tests": set(), "other": set()}
        for f in rs_files:
            if f.startswith(member + "/"):
                inner = f[len(member) + 1:]
                key = "src" if inner.startswith("src/") else "tests" if inner.startswith("tests/") else "other"
                cat[key].add(f)
        for kind, key in (("test", "tests"), ("bin", "other"), ("example", "other"), ("bench", "other")):
            for target in manifest.get(kind, []):
                path = target.get("path")
                if not path:
                    continue
                rel = norm(member.split("/") + path.split("/"))
                if rel in rs_files and not rel.startswith(member + "/"):
                    cat[key].add(rel)
        all_files = cat["src"] | cat["tests"] | cat["other"]
        claimed |= all_files
        tests = 0
        for f in sorted(all_files):
            tests += len(TEST_ATTR.findall(rust_mask(read(root, f))[1]))
        rows.append(
            {
                "name": name,
                "path": member,
                "default": member in defaults,
                "src": sum(lines_of(root, f) for f in cat["src"]),
                "tests": sum(lines_of(root, f) for f in cat["tests"]),
                "other": sum(lines_of(root, f) for f in cat["other"]),
                "files": len(all_files),
                "test_attrs": tests,
            }
        )
    return {
        "rows": rows,
        "members": len(members),
        "defaults": len([m for m in members if m in defaults]),
        "under_crates": len([m for m in members if m.startswith("crates/")]),
        "unbuilt": sorted(set(rs_files) - claimed),
    }


# ---------------------------------------------------------------- other counts

def frontend(root: Path, files: list[str]) -> dict:
    groups = {"source": [], "tests": [], "support": [], "config": []}
    css = []
    for f in files:
        if not f.startswith("apps/desktop/") or f.startswith("apps/desktop/src-tauri/"):
            continue
        if f.endswith(".css"):
            css.append(f)
        if not f.endswith((".ts", ".tsx")):
            continue
        if re.search(r"\.test\.tsx?$", f):
            groups["tests"].append(f)
        elif f.startswith("apps/desktop/src/test/"):
            groups["support"].append(f)
        elif f.startswith("apps/desktop/src/"):
            groups["source"].append(f)
        else:
            groups["config"].append(f)
    out = {k: (len(v), sum(lines_of(root, f) for f in v)) for k, v in groups.items()}
    out["css"] = (len(css), sum(lines_of(root, f) for f in css))
    return out


def strip_sql(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", " ", text, flags=re.S)
    text = re.sub(r"--[^\n]*", " ", text)
    return re.sub(r"'(?:[^']|'')*'", "''", text)


SQL_NAME = r"(?:\"([^\"]+)\"|`([^`]+)`|\[([^\]]+)\]|([A-Za-z_][A-Za-z0-9_$]*))"


def sql_name(m: re.Match, first: int) -> str:
    return next(g for g in m.groups()[first:first + 4] if g is not None).lower()


def migrations(root: Path, files: list[str]) -> dict:
    sql = sorted(f for f in files if f.startswith(MIGRATIONS_DIR + "/") and f.count("/") == MIGRATIONS_DIR.count("/") + 1 and f.endswith(".sql"))
    if not sql:
        raise Fail(f"{MIGRATIONS_DIR}: no .sql file")
    embedded = None
    if (root / MIGRATIONS_RS).is_file():
        code, _masked = rust_mask(read(root, MIGRATIONS_RS))
        embedded = len(set(re.findall(r'include_str!\s*\(\s*"migrations/([^"]+\.sql)"\s*\)', code)))
    statements, tables = 0, set()
    create = re.compile(r"\bCREATE\s+(?:(?:TEMP|TEMPORARY)\s+)?(?:VIRTUAL\s+)?TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?" + SQL_NAME, re.I)
    drop = re.compile(r"\bDROP\s+TABLE\s+(?:IF\s+EXISTS\s+)?" + SQL_NAME, re.I)
    rename = re.compile(r"\bALTER\s+TABLE\s+" + SQL_NAME + r"\s+RENAME\s+TO\s+" + SQL_NAME, re.I)
    drops = renames = 0
    for f in sql:
        text = strip_sql(read(root, f))
        events = [(m.start(), "create", m) for m in create.finditer(text)]
        events += [(m.start(), "drop", m) for m in drop.finditer(text)]
        events += [(m.start(), "rename", m) for m in rename.finditer(text)]
        for _pos, kind, m in sorted(events, key=lambda e: e[0]):
            if kind == "create":
                statements += 1
                tables.add(sql_name(m, 0))
            elif kind == "drop":
                drops += 1
                tables.discard(sql_name(m, 0))
            else:
                renames += 1
                old, new = sql_name(m, 0), sql_name(m, 4)
                tables.discard(old)
                tables.add(new)
    numbers = [re.match(r"(\d+)", f.rsplit("/", 1)[1]) for f in sql]
    first = numbers[0].group(1) if numbers[0] else "?"
    last = numbers[-1].group(1) if numbers[-1] else "?"
    return {
        "files": len(sql),
        "range": f"{first} to {last}",
        "embedded": embedded,
        "create": statements,
        "drop": drops,
        "rename": renames,
        "tables": len(tables),
    }


def simple_counts(files: list[str]) -> dict:
    schemas = [f for f in files if f.startswith("schemas/") and f.endswith(".json")]
    ipc = [f for f in schemas if f.startswith("schemas/ipc/") and f.count("/") == 2]
    req = [f for f in ipc if f.endswith(".request.schema.json")]
    resp = [f for f in ipc if f.endswith(".response.schema.json")]
    scripts_top = [f for f in files if f.startswith("scripts/") and f.count("/") == 1]
    by_ext: dict[str, int] = {}
    for f in scripts_top:
        ext = f.rsplit(".", 1)[1] if "." in f.rsplit("/", 1)[1] else "(none)"
        by_ext[ext] = by_ext.get(ext, 0) + 1
    return {
        "adrs": len([f for f in files if re.fullmatch(r"docs/adr/ADR-[^/]*\.md", f)]),
        "schemas": len(schemas),
        "schemas_ipc": len(ipc),
        "schemas_req": len(req),
        "schemas_resp": len(resp),
        "schemas_ipc_shared": len(ipc) - len(req) - len(resp),
        "schemas_other": len(schemas) - len(ipc),
        "request_ops": {f[len("schemas/ipc/"):-len(".request.schema.json")] for f in req},
        "response_ops": {f[len("schemas/ipc/"):-len(".response.schema.json")] for f in resp},
        "skills": len({f.split("/")[2] for f in files if re.fullmatch(r"\.agents/skills/[^/]+/SKILL\.md", f)}),
        "packs": len({f.split("/")[2] for f in files if re.fullmatch(r"data/competency-packs/[^/]+/pack\.json", f)}),
        "scripts_top": len(scripts_top),
        "scripts_by_ext": dict(sorted(by_ext.items())),
        "scripts_e2e": len([f for f in files if f.startswith("scripts/e2e/") and f.count("/") == 2]),
        "script_tests": len([f for f in files if re.fullmatch(r"scripts/tests/test_[^/]*\.py", f)]),
    }


# ---------------------------------------------------------------- the document

def fmt(n: int) -> str:
    return f"{n:,}"


def render(root: Path) -> str:
    files = walk(root)
    c = crates(root, files)
    ops = operations(root)
    fe = frontend(root, files)
    mig = migrations(root, files)
    s = simple_counts(files)
    rows = c["rows"]
    src = sum(r["src"] for r in rows)
    tst = sum(r["tests"] for r in rows)
    oth = sum(r["other"] for r in rows)
    attrs = sum(r["test_attrs"] for r in rows)
    routed = ops["routed"]
    proposed = ops["proposed"]
    n_prop = sum(len(v) for v in proposed.values())
    areas: dict[str, int] = {}
    for name in routed:
        areas[name.split(".", 1)[0]] = areas.get(name.split(".", 1)[0], 0) + 1
    with_both = [n for n in routed if n in s["request_ops"] and n in s["response_ops"]]
    unrouted_schemas = sorted((s["request_ops"] | s["response_ops"]) - set(routed))
    fe_ts_files = sum(fe[k][0] for k in ("source", "tests", "support", "config"))
    fe_ts_lines = sum(fe[k][1] for k in ("source", "tests", "support", "config"))

    out: list[str] = []
    w = out.append
    w("# Repository metrics")
    w("")
    w("Generated from the files of the tree by `scripts/gen_metrics.py --write`. `python3 -I scripts/gen_metrics.py` fails when this file differs from what the tree gives. No date and no commit id: the same tree gives the same file. Counting rules: the header of `scripts/gen_metrics.py`. Label: TESTED by the generator for the tree that holds this file; a number quoted in another document is not checked against it.")
    w("")
    w("## Summary")
    w("")
    w("| Metric | Value | Counted from |")
    w("|---|---|---|")
    w(f"| Workspace members (crates) | {c['members']} ({c['under_crates']} under `crates/`; {c['defaults']} default members) | `Cargo.toml` |")
    w(f"| Rust lines | {fmt(src + tst + oth)} (src {fmt(src)}, tests {fmt(tst)}, other {fmt(oth)}) | the members' `.rs` files and `[[test]]` paths |")
    w(f"| `#[test]` attributes | {fmt(attrs)} | the same files |")
    w(f"| Routed IPC operations | {len(routed)} | `OPERATIONS` in `{OPS_RS}` |")
    w(f"| PROPOSED IPC operations | {n_prop} ({', '.join(f'{m} {len(v)}' for m, v in proposed.items())}) | `PROPOSED_OPERATIONS` of the modules `{DISPATCH_RS}` names |")
    w(f"| Migrations | {mig['files']} ({mig['range']}) | `{MIGRATIONS_DIR}/` |")
    w(f"| `CREATE TABLE` statements in the migrations | {mig['create']} | the same files |")
    w(f"| Tables after the last migration | {mig['tables']} ({mig['drop']} `DROP TABLE`, {mig['rename']} renames) | the same files |")
    w(f"| ADRs | {s['adrs']} | `docs/adr/` |")
    w(f"| JSON schemas | {s['schemas']} ({s['schemas_ipc']} in `schemas/ipc/`, {s['schemas_other']} elsewhere) | `schemas/` |")
    w(f"| Skill packages | {s['skills']} | `.agents/skills/` |")
    w(f"| Competency packs | {s['packs']} | `data/competency-packs/` |")
    w(f"| Frontend TS/TSX | {fe_ts_files} files, {fmt(fe_ts_lines)} lines; {fe['tests'][0]} test files | `apps/desktop/` |")
    w(f"| CSS | {fe['css'][0]} files, {fmt(fe['css'][1])} lines | `apps/desktop/` |")
    w(f"| Scripts | {s['scripts_top']} in `scripts/`, {s['scripts_e2e']} in `scripts/e2e/`, {s['script_tests']} test modules in `scripts/tests/` | `scripts/` |")
    w("")
    w("## Rust per workspace member")
    w("")
    w("| Crate | Path | Default member | Files | src lines | tests lines | other lines | `#[test]` |")
    w("|---|---|---|---|---|---|---|---|")
    for r in rows:
        w(
            f"| {r['name']} | `{r['path']}` | {'yes' if r['default'] else 'no'} | {r['files']} | {fmt(r['src'])} | "
            f"{fmt(r['tests'])} | {fmt(r['other'])} | {fmt(r['test_attrs'])} |"
        )
    w(f"| Total | | | {sum(r['files'] for r in rows)} | {fmt(src)} | {fmt(tst)} | {fmt(oth)} | {fmt(attrs)} |")
    w("")
    if c["unbuilt"]:
        w("Rust files that no member builds (no member directory, no `[[test]]`, `[[bin]]`, `[[example]]` or `[[bench]]` path):")
        w("")
        for f in c["unbuilt"]:
            w(f"- `{f}`")
    else:
        w("Rust files that no member builds: none.")
    w("")
    w("## IPC operations")
    w("")
    w("| Area | Routed | PROPOSED |")
    w("|---|---|---|")
    all_areas = sorted(set(areas) | {n.split(".", 1)[0] for v in proposed.values() for n in v})
    prop_areas: dict[str, int] = {}
    for v in proposed.values():
        for n in v:
            prop_areas[n.split(".", 1)[0]] = prop_areas.get(n.split(".", 1)[0], 0) + 1
    for a in all_areas:
        w(f"| `{a}` | {areas.get(a, 0)} | {prop_areas.get(a, 0)} |")
    w(f"| Total | {len(routed)} | {n_prop} |")
    w("")
    w(
        f"Routed operations with a request and a response schema in `schemas/ipc/`: {len(with_both)} of {len(routed)}. "
        f"Schemas in `schemas/ipc/`: {s['schemas_req']} request, {s['schemas_resp']} response, {s['schemas_ipc_shared']} shared."
    )
    if unrouted_schemas:
        w("")
        w("Operation schemas without a routed operation: " + ", ".join(f"`{n}`" for n in unrouted_schemas) + ".")
    w("")
    w("Cross-checks passed: operations.parse, operations.unique, operations.disjoint"
      + (", operations.cli-areas" if (root / CLI_OPS_RS).is_file() else "")
      + (", operations.test-assertion" if ops["asserted"] else "") + ".")
    w("")
    w("## Storage")
    w("")
    w("| Metric | Value |")
    w("|---|---|")
    w(f"| Migration files | {mig['files']} |")
    w(f"| Migrations embedded by `{MIGRATIONS_RS}` | {mig['embedded'] if mig['embedded'] is not None else 'file not found'} |")
    w(f"| `CREATE TABLE` statements | {mig['create']} |")
    w(f"| `DROP TABLE` statements | {mig['drop']} |")
    w(f"| `ALTER TABLE ... RENAME TO` statements | {mig['rename']} |")
    w(f"| Tables after the last migration | {mig['tables']} |")
    w("")
    w("## Frontend")
    w("")
    w("| Group | Files | Lines |")
    w("|---|---|---|")
    w(f"| Application source (`apps/desktop/src/`, .ts/.tsx) | {fe['source'][0]} | {fmt(fe['source'][1])} |")
    w(f"| Test files (`*.test.ts`, `*.test.tsx`) | {fe['tests'][0]} | {fmt(fe['tests'][1])} |")
    w(f"| Test support (`apps/desktop/src/test/`) | {fe['support'][0]} | {fmt(fe['support'][1])} |")
    w(f"| Build configuration (.ts outside `apps/desktop/src/`) | {fe['config'][0]} | {fmt(fe['config'][1])} |")
    w(f"| CSS | {fe['css'][0]} | {fmt(fe['css'][1])} |")
    w("")
    w("## Content and scripts")
    w("")
    w("| Metric | Value |")
    w("|---|---|")
    w(f"| ADRs (`docs/adr/`) | {s['adrs']} |")
    w(f"| JSON schemas in `schemas/ipc/` | {s['schemas_ipc']} |")
    w(f"| JSON schemas elsewhere in `schemas/` | {s['schemas_other']} |")
    w(f"| Skill packages (`.agents/skills/`) | {s['skills']} |")
    w(f"| Competency packs (`data/competency-packs/`) | {s['packs']} |")
    w(f"| Files in `scripts/` | {s['scripts_top']} ({', '.join(f'.{k} {v}' for k, v in s['scripts_by_ext'].items())}) |")
    w(f"| Files in `scripts/e2e/` | {s['scripts_e2e']} |")
    w(f"| Test modules `scripts/tests/test_*.py` | {s['script_tests']} |")
    w("")
    return "\n".join(out)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Generate or check docs/release/metrics.md.")
    ap.add_argument("--root", default=str(ROOT), help="tree to count (default: this repository)")
    ap.add_argument("--write", action="store_true", help=f"write <root>/{OUTPUT}")
    args = ap.parse_args(argv)
    root = Path(args.root).resolve()
    if not root.is_dir():
        sys.stderr.write(f"error: --root {args.root}: not a directory\n")
        return 2
    try:
        text = render(root)
    except Fail as exc:
        sys.stderr.write(f"error: {exc}\n")
        return 2
    except CrossCheck as exc:
        print(f"metrics: FAIL cross-check {exc}")
        return 1
    target = root / OUTPUT
    if args.write:
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8", newline="\n")
        print(f"metrics: wrote {OUTPUT}")
        return 0
    if not target.is_file():
        print(f"metrics: FAIL {OUTPUT} is missing; run python3 -I scripts/gen_metrics.py --write")
        return 1
    current = target.read_text(encoding="utf-8")
    if current == text:
        print(f"metrics: OK ({OUTPUT} is current)")
        return 0
    print(f"metrics: FAIL {OUTPUT} is stale; run python3 -I scripts/gen_metrics.py --write")
    diff = list(difflib.unified_diff(current.splitlines(), text.splitlines(), "committed", "from the tree", lineterm="", n=0))
    print("\n".join(diff[:60]))
    if len(diff) > 60:
        print(f"... {len(diff) - 60} more diff lines")
    return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
