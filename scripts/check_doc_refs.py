#!/usr/bin/env python3
"""check_doc_refs.py - every repository path a document names exists (drift gate, step DOC-2).

Documents scanned (relative to the repository root):
    README.md, GOVERNANCE.md, SECURITY.md, AGENTS.md, LICENSES.md, apps/*/README.md, docs/**/*.md
    never below a directory named node_modules or target, never below docs/audit/findings/.

Where references are read
  * inline code spans (text between two runs of the same number of backticks on one line)
  * the lines of fenced code blocks (``` or ~~~). In a block, an indented line under a line whose
    first word is a directory (`crates/`) is an entry of a tree and is joined to that directory.
  Text is cut into words at white space and at `=`. The wrapping characters ( [ ` and ) ] ` , ; . :
  and a final `()` are removed from the ends of a word. A quoted word ("data/x.json") is sample
  data, not a reference.

What a reference is
  A word whose first path component is a top-level name of the repository: a top-level directory
  followed by `/`, or a top-level file. A bare directory name (`data`, `tests`) is a word, not a path.
  Top-level names are the entries of the root that .gitignore does not ignore, plus the root-anchored
  literal patterns of the root .gitignore (`/target`), so that a reference to a build output is
  recognised on every machine. Shape:

      path[::name[::name...]][:LINE[-LINE][,LINE...][:COLUMN]][#fragment]

  A brace form is expanded first: `crates/x/src/{a,b}.rs` is two references.

What is checked
  * the path exists. "Exists" means: a file in the tree that .gitignore does not ignore, or a
    directory that holds at least one such file. Build outputs and empty directories therefore do
    not exist for this gate, whatever the machine holds: the result is the same in a working tree
    and in a clean export of the same revision (untracked files apart). A trailing `/` needs a directory.
    Every .gitignore of the tree is read, as git reads it: the last matching rule wins, deeper files
    after the inherited rules; `!pattern` re-includes, except below a directory that is ignored.
    An escaped pattern (`\\#`, `\\!`) or a `**` other than a leading `**/name` is a usage error (exit 2).
  * `file.rs::name`: the last name is defined in that file as fn, struct, enum, const, static, mod,
    trait, type or macro_rules (comments and string literals are not searched; an item made by a
    macro call is not seen). For a directory (`crates/g9-app::pipeline`) the definition may be in
    any .rs file below it. For `file.py::name`: def or class. No other file type can carry `::name`.
  * `file:LINE`: the file has at least that many lines (the highest number of a range or list).
  * `#fragment` and `:COLUMN` are not checked.

What is not checked, and counted
  * placeholders: a word with `<`, `>`, `*`, `?`, `$`, `…`, `...`, a run of two or more `N` that
    does not touch another capital letter (`week-NN.md`), or a brace group without a comma (`{name}`)
  * words that begin like a reference and do not have the shape above (shown by --list)

Allow-list (scripts/doc_refs_allow.json): references that are absent for a stated reason.
    {"description": "<optional text>",
     "entries": [{"ref": "<path[::name][:LINE]: braces expanded, no trailing slash, no fragment>",
                  "reason": "planned" | "historical" | "external",
                  "docs": ["<document that may hold it>", ...],
                  "note": "<why>"}]}
  planned     a file a PROPOSED document says will be created
  historical  a dated record describes the file, the name or the line as it was then
  external    a path on another machine or in another tree, or a build output
  An entry covers the reference in the listed documents only. The gate fails on a stale entry:
  the reference resolves after all, or a listed document does not hold it any more.

Usage (repository root; standard library only; no network):
    python3 -I scripts/check_doc_refs.py [--root DIR] [--allow FILE] [--list]

    --root DIR    tree to check (default: the repository this script is in)
    --allow FILE  allow-list (default: <root>/scripts/doc_refs_allow.json; without that file the list is empty)
    --list        also print every word that was read as a reference, with its result

Exit codes: 0 every reference resolves or is allow-listed and no entry is stale (prints the number
of references checked); 1 otherwise, one line per problem (`doc:line  token  reason`);
2 usage or environment error (no document found, a .gitignore line this script cannot read).
"""
from __future__ import annotations

import argparse
import fnmatch
import json
import os
import posixpath
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ROOT_DOCS = ("README.md", "GOVERNANCE.md", "SECURITY.md", "AGENTS.md", "LICENSES.md")
DOC_SKIP_COMPONENTS = frozenset({"node_modules", "target"})
DOC_SKIP_PREFIXES = ("docs/audit/findings/",)
ALLOW_DEFAULT = "scripts/doc_refs_allow.json"
REASONS = ("planned", "historical", "external")
ENTRY_KEYS = frozenset({"ref", "reason", "docs", "note"})

FENCE = re.compile(r"^[ \t]*(`{3,}|~{3,})")
SPLIT = re.compile(r"[\s=]+")
LEAD = "([`"
TRAIL = ")]`,;.:"
TREE_NAME = re.compile(r"^[A-Za-z0-9_.<>*{}…-][A-Za-z0-9_.<>*{},…/+@-]*$")
PLACEHOLDER = re.compile(r"[<>*?$…]|\.\.\.|(?<![A-Z])N{2,}(?![A-Z])|\{[^{},]*\}")
BRACES = re.compile(r"\{([^{}]*)\}")
SHAPE = re.compile(
    r"^(?P<path>[A-Za-z0-9_.+@~%/-]+)"
    r"(?P<names>(?:::[A-Za-z_][A-Za-z0-9_]*)+)?"
    r"(?P<lines>:\d+(?:[-,]\d+)*)?(?::\d+)?"
    r"(?P<fragment>#\S*)?$"
)
RUST_KINDS = "fn, struct, enum, const, static, mod, trait, type or macro_rules"
PY_KINDS = "def or class"

# Rust lexer pieces: what starts a comment or a literal, and a char literal (a quote that does not
# close one character later starts a lifetime or a label).
RUST_TOKEN = re.compile(r"//|/\*|b?r#*\"|\"|'")
RUST_CHAR = re.compile(r"'(?:\\(?:x[0-9A-Fa-f]{2}|u\{[0-9A-Fa-f_]{1,8}\}|.)|[^\\'\n])'")
IDENT_CHAR = re.compile(r"[A-Za-z0-9_]")


class Unsupported(Exception):
    """A .gitignore line this gate does not implement."""


# ---------------------------------------------------------------- Rust and Python source text

def rust_code(text: str) -> str:
    """`text` with comments removed and string and char literals emptied (nested block comments,
    raw strings `r#"..."#` and byte strings included). What is left is code."""
    out: list[str] = []
    i, n = 0, len(text)
    while True:
        m = RUST_TOKEN.search(text, i)
        if m is None:
            out.append(text[i:])
            break
        tok, s = m.group(0), m.start()
        if tok[0] in "br" and s > 0 and IDENT_CHAR.match(text[s - 1]):
            # `r"` or `br"` glued to an identifier is not a raw string prefix: take the quote alone.
            q = text.index('"', s)
            out.append(text[i:q])
            i = q
            continue
        out.append(text[i:s])
        if tok == "//":
            end = text.find("\n", s)
            i = n if end < 0 else end
        elif tok == "/*":
            depth, j = 1, s + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            out.append(" ")
            i = j
        elif tok == "'":
            c = RUST_CHAR.match(text, s)
            if c:
                out.append("' '")
                i = c.end()
            else:
                out.append("'")
                i = s + 1
        elif tok == '"':
            j = s + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            out.append('""')
            i = j + 1
        else:  # raw string: r"...", r#"..."#, br"..."
            close = '"' + "#" * tok.count("#")
            end = text.find(close, m.end())
            out.append('""')
            i = n if end < 0 else end + len(close)
    return "".join(out)


def python_code(text: str) -> str:
    """`text` without its comment lines (enough to find `def` and `class` statements)."""
    return re.sub(r"(?m)^[ \t]*#.*$", "", text)


def rust_defines(code: str, name: str) -> bool:
    n = re.escape(name)
    return (
        re.search(
            r"(?<![A-Za-z0-9_])(?:fn|struct|enum|const|static|mod|trait|type)\s+(?:mut\s+)?%s(?![A-Za-z0-9_])" % n,
            code,
        )
        is not None
        or re.search(r"(?<![A-Za-z0-9_])macro_rules!\s*%s(?![A-Za-z0-9_])" % n, code) is not None
    )


def python_defines(code: str, name: str) -> bool:
    return re.search(r"(?m)^[ \t]*(?:async[ \t]+)?(?:def|class)[ \t]+%s(?![A-Za-z0-9_])" % re.escape(name), code) is not None


# ---------------------------------------------------------------- the tree, as git would hold it

def parse_gitignore(text: str, base: str, where: str) -> list[tuple[str, tuple[str, ...], bool, bool, bool]]:
    """Rules as (base directory, pattern segments, anchored, directories only, negated), in file
    order. A subset of the syntax: `!pattern` re-includes; an escaped pattern is not supported."""
    rules = []
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.rstrip()
        if not line or line.startswith("#"):
            continue
        if "\\" in line:
            raise Unsupported(f"{where}:{number}: escaped patterns are not supported")
        negated = line.startswith("!")
        if negated:
            line = line[1:]
        dir_only = line.endswith("/")
        line = line.rstrip("/")
        anchored = False
        if line.startswith("**/"):
            line = line[3:]
            if "/" in line:
                raise Unsupported(f"{where}:{number}: `**/` before more than one segment is not supported")
        elif line.startswith("/"):
            line, anchored = line[1:], True
        segments = tuple(line.split("/"))
        if "**" in line or not all(segments):
            raise Unsupported(f"{where}:{number}: pattern not supported: {raw!r}")
        rules.append((base, segments, anchored or len(segments) > 1, dir_only, negated))
    return rules


def is_ignored(rules: list, rel: str, is_dir: bool) -> bool:
    """As git decides for `rel` whose parent directory is not ignored: the last rule that matches
    wins (`rules` holds the inherited rules first, then those of deeper .gitignore files, each in
    file order); a negated rule re-includes. A file below an ignored directory is never reached,
    so it cannot be re-included, as in git."""
    name = rel.rsplit("/", 1)[-1]
    for base, segments, anchored, dir_only, negated in reversed(rules):
        if dir_only and not is_dir:
            continue
        if not anchored:
            if fnmatch.fnmatchcase(name, segments[0]):
                return not negated
            continue
        parts = (rel[len(base) + 1:] if base else rel).split("/")
        if len(parts) == len(segments) and all(fnmatch.fnmatchcase(p, s) for p, s in zip(parts, segments)):
            return not negated
    return False


class Tree:
    """The files of the tree that git would hold, and the questions the gate asks about them."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.files: set[str] = set()
        self.rules_at: dict[str, list] = {}
        self._walk(root, "", [])
        self.dirs: set[str] = set()
        for f in self.files:
            parts = f.split("/")
            for i in range(1, len(parts)):
                self.dirs.add("/".join(parts[:i]))
        self.top_files = {f for f in self.files if "/" not in f}
        self.top_dirs = {d for d in self.dirs if "/" not in d}
        # A root-anchored literal pattern of the root .gitignore names a build output (`/target`).
        for base, segments, anchored, _dir_only, negated in self.rules_at[""]:
            if not negated and base == "" and anchored and len(segments) == 1 and not re.search(r"[*?\[]", segments[0]):
                self.top_dirs.add(segments[0])
        self._code: dict[str, str] = {}
        self._lines: dict[str, int] = {}

    def _walk(self, directory: Path, rel: str, inherited: list) -> None:
        rules = inherited
        ignore_file = directory / ".gitignore"
        if ignore_file.is_file():
            where = f"{rel}/.gitignore" if rel else ".gitignore"
            rules = inherited + parse_gitignore(ignore_file.read_text(encoding="utf-8"), rel, where)
        self.rules_at[rel] = rules
        with os.scandir(directory) as it:
            entries = sorted(it, key=lambda e: e.name)
        for entry in entries:
            if not rel and entry.name == ".git":
                continue
            child = f"{rel}/{entry.name}" if rel else entry.name
            is_dir = entry.is_dir(follow_symlinks=False)
            if is_ignored(rules, child, is_dir):
                continue
            if is_dir:
                self._walk(Path(entry.path), child, rules)
            else:
                self.files.add(child)

    def exists(self, path: str) -> bool:
        return path in self.files or path in self.dirs

    def why_absent(self, path: str) -> str:
        """Reason for a path that is not in the tree."""
        parts = path.split("/")
        rules = self.rules_at[""]
        for i in range(1, len(parts) + 1):
            prefix = "/".join(parts[:i])
            # Every prefix but the last is a directory; the last one is unless it is a file on disk.
            if is_ignored(rules, prefix, i < len(parts) or not (self.root / prefix).is_file()):
                return "not in the repository: .gitignore ignores it (a build output or a local file)"
            rules = self.rules_at.get(prefix, rules)
        if (self.root / path).is_dir():
            return "no file in this directory (git does not store an empty directory)"
        return "no such file or directory"

    def text(self, path: str) -> str:
        try:
            return (self.root / path).read_text(encoding="utf-8", errors="replace")
        except OSError:
            return ""

    def code(self, path: str) -> str:
        """Source text without comments (and, for Rust, without literals)."""
        if path not in self._code:
            text = self.text(path)
            self._code[path] = rust_code(text) if path.endswith(".rs") else python_code(text)
        return self._code[path]

    def line_count(self, path: str) -> int:
        if path not in self._lines:
            try:
                data = (self.root / path).read_bytes()
            except OSError:
                data = b""
            self._lines[path] = data.count(b"\n") + (1 if data and not data.endswith(b"\n") else 0)
        return self._lines[path]

    def documents(self) -> list[str]:
        found = [d for d in ROOT_DOCS if d in self.files]
        for f in sorted(self.files):
            parts = f.split("/")
            if DOC_SKIP_COMPONENTS.intersection(parts[:-1]) or f.startswith(DOC_SKIP_PREFIXES):
                continue
            if (len(parts) > 1 and parts[-1] == "README.md") or (
                parts[0] == "docs" and len(parts) > 1 and f.endswith(".md")
            ):
                found.append(f)
        return found


# ---------------------------------------------------------------- reading a document

def code_spans(line: str) -> list[str]:
    """Inline code spans of one line: text between two runs of the same number of backticks."""
    spans, i, n = [], 0, len(line)
    while i < n:
        if line[i] != "`":
            i += 1
            continue
        j = i
        while j < n and line[j] == "`":
            j += 1
        run, k, close = j - i, j, -1
        while k < n:
            if line[k] != "`":
                k += 1
                continue
            e = k
            while e < n and line[e] == "`":
                e += 1
            if e - k == run:
                close = k
                break
            k = e
        if close < 0:
            i = j
            continue
        spans.append(line[j:close])
        i = close + run
    return spans


def words(text: str) -> list[str]:
    out = []
    for word in SPLIT.split(text):
        word = word.lstrip(LEAD).rstrip(TRAIL)
        if word.endswith("()"):
            word = word[:-2]
        if word:
            out.append(word)
    return out


def candidates(text: str) -> list[tuple[int, str]]:
    """(line number, word) for every word of a code span or of a fenced block; tree entries joined."""
    out: list[tuple[int, str]] = []
    fence: tuple[str, int] | None = None
    parents: list[tuple[int, str]] = []
    for number, line in enumerate(text.split("\n"), 1):
        mark = FENCE.match(line)
        if fence is None:
            if mark:
                fence, parents = (mark.group(1)[0], len(mark.group(1))), []
            else:
                for span in code_spans(line):
                    out.extend((number, w) for w in words(span))
            continue
        if mark and mark.group(1)[0] == fence[0] and len(mark.group(1)) >= fence[1] and not line[mark.end():].strip():
            fence = None
            continue
        parts = line.split(None, 1)
        if not parts:
            continue
        indent = len(line) - len(line.lstrip(" \t"))
        while parents and parents[-1][0] >= indent:
            parents.pop()
        first = parts[0]
        if parents and TREE_NAME.match(first):
            joined = parents[-1][1] + first
            if first.endswith("/"):
                parents.append((indent, joined))
            out.extend((number, w) for w in words(joined))
            out.extend((number, w) for w in words(parts[1] if len(parts) > 1 else ""))
            continue
        if first.endswith("/") and TREE_NAME.match(first):
            parents.append((indent, first))
        out.extend((number, w) for w in words(line))
    return out


def expand(word: str) -> list[str]:
    """Brace expansion. Every group has a comma here: a group without one is a placeholder."""
    group = BRACES.search(word)
    if not group:
        return [word]
    out: list[str] = []
    for option in group.group(1).split(","):
        out.extend(expand(word[: group.start()] + option + word[group.end():]))
    return out


def canonical(ref: str) -> str | None:
    """The form an allow-list entry is written in, or None when `ref` has no reference shape."""
    m = SHAPE.match(ref)
    if m is None:
        return None
    return posixpath.normpath(m.group("path")) + (m.group("names") or "") + (m.group("lines") or "")


class Checker:
    def __init__(self, tree: Tree) -> None:
        self.tree = tree
        self._defined: dict[tuple[str, str], bool] = {}

    def is_reference(self, word: str) -> bool:
        head, slash, _ = word.partition("/")
        if slash:
            return head in self.tree.top_dirs
        return re.split(r"[:#]", word, maxsplit=1)[0] in self.tree.top_files

    def _defines(self, path: str, name: str) -> bool:
        key = (path, name)
        if key not in self._defined:
            code = self.tree.code(path)
            self._defined[key] = rust_defines(code, name) if path.endswith(".rs") else python_defines(code, name)
        return self._defined[key]

    def check(self, ref: str) -> str | None:
        """None when the reference resolves, else the reason. `ref` has the SHAPE."""
        m = SHAPE.match(ref)
        if m is None:
            return "not a reference"
        tree = self.tree
        written = m.group("path")
        path = posixpath.normpath(written)
        if path.startswith("../") or path in ("..", "."):
            return "outside the repository"
        if not tree.exists(path):
            return tree.why_absent(path)
        is_file = path in tree.files
        if written.endswith("/") and is_file:
            return "a file, not a directory"
        if m.group("names"):
            name = m.group("names").rsplit("::", 1)[1]
            if is_file:
                ext = posixpath.splitext(path)[1]
                if ext not in (".rs", ".py"):
                    return "`::name` can be checked in a .rs or .py file, or in a directory of .rs files, only"
                if not self._defines(path, name):
                    kinds = RUST_KINDS if ext == ".rs" else PY_KINDS
                    return f"`{name}` is not defined in {path} ({kinds})"
            else:
                sources = sorted(f for f in tree.files if f.startswith(path + "/") and f.endswith(".rs"))
                if not any(self._defines(f, name) for f in sources):
                    return f"`{name}` is not defined in any .rs file below {path}/ ({RUST_KINDS})"
        if m.group("lines"):
            if not is_file:
                return "a line number needs a file"
            last = max(int(x) for x in re.findall(r"\d+", m.group("lines")))
            have = tree.line_count(path)
            if last < 1 or last > have:
                return f"line {last} is beyond the end of {path} ({have} lines)"
        return None


# ---------------------------------------------------------------- allow-list

def load_allow(path: Path) -> tuple[list[dict], list[str]]:
    """(entries, problems)."""
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        return [], [f"not readable as JSON: {exc}"]
    if not isinstance(doc, dict) or not isinstance(doc.get("entries"), list):
        return [], ['the root must be an object with an "entries" array']
    problems = [f"unknown top-level key {k!r}" for k in sorted(set(doc) - {"entries", "description"})]
    entries, seen = [], set()
    for i, entry in enumerate(doc["entries"]):
        where = f"entries[{i}]"
        if not isinstance(entry, dict) or set(entry) != ENTRY_KEYS:
            problems.append(f"{where}: an entry has exactly the keys {', '.join(sorted(ENTRY_KEYS))}")
            continue
        ref, reason, docs, note = entry["ref"], entry["reason"], entry["docs"], entry["note"]
        if not isinstance(ref, str) or canonical(ref) is None:
            problems.append(f"{where}: ref is not a reference (path[::name][:LINE], braces expanded)")
            continue
        where = f"{where} {ref}"
        if canonical(ref) != ref:
            problems.append(f"{where}: write the ref as {canonical(ref)}")
        elif reason not in REASONS:
            problems.append(f"{where}: reason must be one of {', '.join(REASONS)}")
        elif not isinstance(docs, list) or not docs or not all(isinstance(d, str) for d in docs) or len(set(docs)) != len(docs):
            problems.append(f"{where}: docs must be a non-empty list of distinct document paths")
        elif not isinstance(note, str) or not note.strip():
            problems.append(f"{where}: note must say why, in words")
        elif ref in seen:
            problems.append(f"{where}: second entry for the same ref")
        else:
            seen.add(ref)
            entries.append(entry)
    return entries, problems


# ---------------------------------------------------------------- main

def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Check that every repository path named in the documents exists.")
    ap.add_argument("--root", default=str(ROOT), help="tree to check (default: this repository)")
    ap.add_argument("--allow", default=None, help=f"allow-list (default: <root>/{ALLOW_DEFAULT})")
    ap.add_argument("--list", action="store_true", help="also print every word read as a reference, with its result")
    args = ap.parse_args(argv)

    root = Path(args.root).resolve()
    if not root.is_dir():
        sys.stderr.write(f"error: --root {args.root}: not a directory\n")
        return 2
    try:
        tree = Tree(root)
    except Unsupported as exc:
        sys.stderr.write(f"error: {exc}\n")
        return 2
    documents = tree.documents()
    if not documents:
        sys.stderr.write(f"error: no document found under {root}; nothing was checked\n")
        return 2
    allow_path = Path(args.allow).resolve() if args.allow else root / ALLOW_DEFAULT
    if args.allow and not allow_path.is_file():
        sys.stderr.write(f"error: --allow {args.allow}: no such file\n")
        return 2
    try:
        allow_name = allow_path.relative_to(root).as_posix()
    except ValueError:
        allow_name = str(allow_path)
    entries, allow_problems = load_allow(allow_path) if allow_path.is_file() else ([], [])
    allowed = {e["ref"]: e for e in entries}

    checker = Checker(tree)
    resolve = placeholders = shapes = 0
    by_reason = dict.fromkeys(REASONS, 0)
    broken: list[str] = []
    listing: list[str] = []
    absent_in: dict[str, set[str]] = {}  # canonical reference -> documents in which it does not resolve

    for doc in documents:
        for number, word in candidates(tree.text(doc)):
            if not checker.is_reference(word):
                continue
            if PLACEHOLDER.search(word):
                placeholders += 1
                listing.append(f"{doc}:{number}  {word}  NOT CHECKED placeholder")
                continue
            for ref in expand(word):
                shown = ref if ref == word else f"{ref} (from {word})"
                key = canonical(ref)
                if key is None:
                    shapes += 1
                    listing.append(f"{doc}:{number}  {shown}  NOT CHECKED shape not recognised")
                    continue
                reason = checker.check(ref)
                if reason is None:
                    resolve += 1
                    listing.append(f"{doc}:{number}  {shown}  OK")
                    continue
                absent_in.setdefault(key, set()).add(doc)
                entry = allowed.get(key)
                if entry is not None and doc in entry["docs"]:
                    by_reason[entry["reason"]] += 1
                    listing.append(f"{doc}:{number}  {shown}  ALLOWED {entry['reason']} ({reason})")
                    continue
                if entry is not None:
                    reason += f"; the allow-list has it as {entry['reason']} for other documents only"
                broken.append(f"{doc}:{number}  {shown}  {reason}")
                listing.append(f"{doc}:{number}  {shown}  BROKEN {reason}")

    for entry in entries:
        ref = entry["ref"]
        if checker.check(ref) is None:
            allow_problems.append(f"{ref}  stale entry: the reference resolves now")
            continue
        for doc in entry["docs"]:
            if doc not in documents:
                allow_problems.append(f"{ref}  stale entry: {doc} is not a scanned document")
            elif doc not in absent_in.get(ref, ()):
                allow_problems.append(f"{ref}  stale entry: {doc} does not hold this reference any more")

    if args.list:
        print("\n".join(listing))
    n_allowed = sum(by_reason.values())
    total = resolve + n_allowed + len(broken)
    counts = (
        f"{total} references checked in {len(documents)} documents: {resolve} resolve, "
        f"{n_allowed} absent and allow-listed ("
        + ", ".join(f"{r} {by_reason[r]}" for r in REASONS)
        + f"), {len(broken)} broken; not checked: {placeholders} placeholders, "
        f"{shapes} words with a shape that is not recognised; allow-list: {len(entries)} entries"
    )
    if broken or allow_problems:
        print(f"doc refs: FAIL ({counts}; {len(allow_problems)} allow-list problem(s))")
        print("\n".join(broken + [f"{allow_name}  {p}" for p in allow_problems]))
        return 1
    print(f"doc refs: OK ({counts})")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
