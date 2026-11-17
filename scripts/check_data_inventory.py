#!/usr/bin/env python3
"""check_data_inventory.py - every table the migrations create has one complete row in the data
inventory, and every row of the inventory names such a table (plan step PRIV-7,
docs/release/path-to-10.md section 5.15).

Inputs
    the migrations: every *.sql file of a directory, applied in file-name order
        (default: <root>/crates/g9-storage/src/migrations)
    the inventory: the first Markdown table of the section "## Data inventory" of a document
        (default: <root>/docs/data-model.md)

How the migrations are read
    Comments (-- and /* */) are removed and string literals are blanked first, so a CREATE TABLE
    that is only text in a comment or a trigger message is not a table. Then, in order:
      * CREATE [VIRTUAL] TABLE [IF NOT EXISTS] [schema.]name   adds a table (a virtual table, an
        FTS5 table for example, is a table and needs a row; its shadow tables are SQLite's own)
      * DROP TABLE [IF EXISTS] [schema.]name                   removes it
      * ALTER TABLE [schema.]name RENAME TO new_name           renames it
    CREATE TEMP / TEMPORARY TABLE (or a name in the schema temp) is not counted: a temporary table
    is not stored in the database file. Names may be bare, "double-quoted", `back-quoted` or
    [bracketed]; they are compared case-insensitively, as SQLite compares them.

How the inventory is read
    The section must hold exactly one Markdown table whose header is these nine columns (case,
    spacing and backticks ignored); other tables of the section (a legend, notes) are not read:
        table | data domain | classification | operator content | immutable |
        erasure by deletion.request | export by export.request | in backups | retention
    The section ends at the next heading of level 1 or 2.
    The table name of a row is the first `code span` of its first cell (or the whole cell).
    A '|' inside a code span or escaped as '\\|' does not split a cell.

Rules (each failure line names its rule)
    no-section     the document has no section "## Data inventory", or more than one
    bad-header     the section holds no table with the nine columns above, or more than one
    bad-row        a row does not have nine cells
    empty-cell     a cell is empty or only a placeholder (-, ?, TBD, TODO)
    duplicate-row  two rows name the same table
    missing-row    a table the migrations create has no row
    orphan-row     a row names no table the migrations leave in place

Exit status: 0 complete (prints the number of tables), 1 a rule failed, 2 an input is missing.

Usage (standard library only; no network; deterministic):
    python3 -I scripts/check_data_inventory.py [--root DIR] [--migrations DIR] [--doc FILE]
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

HEADER = [
    "table",
    "data domain",
    "classification",
    "operator content",
    "immutable",
    "erasure by deletion.request",
    "export by export.request",
    "in backups",
    "retention",
]
PLACEHOLDERS = {"", "-", "--", "–", "—", "?", "tbd", "todo", "xxx"}
SECTION_RE = re.compile(r"^##\s+Data inventory\s*$", re.IGNORECASE)
HEADING_RE = re.compile(r"^#{1,2}\s")
SEPARATOR_RE = re.compile(r"^\|?\s*:?-{3,}:?\s*(\|\s*:?-{3,}:?\s*)*\|?\s*$")

_IDENT = r'(?:"(?:[^"]|"")+"|`(?:[^`]|``)+`|\[[^\]]+\]|[A-Za-z_][A-Za-z0-9_$]*)'
_QUALIFIED = r"(?:(?P<{p}schema>" + _IDENT + r")\s*\.\s*)?(?P<{p}name>" + _IDENT + r")"
STATEMENT_RE = re.compile(
    r"\bCREATE\s+(?:(?P<temp>TEMP|TEMPORARY)\s+)?(?P<virtual>VIRTUAL\s+)?TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?"
    + _QUALIFIED.format(p="c")
    + r"|\bDROP\s+TABLE\s+(?:IF\s+EXISTS\s+)?"
    + _QUALIFIED.format(p="d")
    + r"|\bALTER\s+TABLE\s+"
    + _QUALIFIED.format(p="r")
    + r"\s+RENAME\s+TO\s+(?P<rnew>"
    + _IDENT
    + r")",
    re.IGNORECASE,
)


def strip_sql(text: str) -> str:
    """Remove comments and blank string literals; keep quoted identifiers."""
    out: list[str] = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c == "-" and text.startswith("--", i):
            j = text.find("\n", i)
            i = n if j < 0 else j
        elif c == "/" and text.startswith("/*", i):
            j = text.find("*/", i + 2)
            i = n if j < 0 else j + 2
            out.append(" ")
        elif c == "'":
            j = i + 1
            while j < n:
                if text[j] == "'":
                    if j + 1 < n and text[j + 1] == "'":
                        j += 2
                        continue
                    break
                j += 1
            out.append("''")
            i = j + 1
        elif c in "\"`[":
            close = "]" if c == "[" else c
            j = i + 1
            while j < n:
                if text[j] == close:
                    if close != "]" and j + 1 < n and text[j + 1] == close:
                        j += 2
                        continue
                    break
                j += 1
            out.append(text[i : j + 1])
            i = j + 1
        else:
            out.append(c)
            i += 1
    return "".join(out)


def unquote(ident: str) -> str:
    if len(ident) >= 2 and ident[0] == ident[-1] and ident[0] in "\"`":
        q = ident[0]
        return ident[1:-1].replace(q + q, q)
    if len(ident) >= 2 and ident[0] == "[" and ident[-1] == "]":
        return ident[1:-1]
    return ident


def read_tables(migrations: Path) -> tuple[dict[str, dict[str, object]], dict[str, int]]:
    """Tables left in place after every migration: key (lower case) -> name, file, virtual."""
    tables: dict[str, dict[str, object]] = {}
    stats = {"files": 0, "created": 0, "virtual": 0, "dropped": 0, "renamed": 0, "temporary": 0}
    for path in sorted(migrations.glob("*.sql")):
        stats["files"] += 1
        text = strip_sql(path.read_text(encoding="utf-8"))
        for m in STATEMENT_RE.finditer(text):
            if m.group("cname"):
                schema = unquote(m.group("cschema") or "").lower()
                if m.group("temp") or schema == "temp":
                    stats["temporary"] += 1
                    continue
                name = unquote(m.group("cname"))
                virtual = bool(m.group("virtual"))
                stats["created"] += 1
                stats["virtual"] += int(virtual)
                tables[name.lower()] = {"name": name, "file": path.name, "virtual": virtual}
            elif m.group("dname"):
                if tables.pop(unquote(m.group("dname")).lower(), None) is not None:
                    stats["dropped"] += 1
            elif m.group("rname"):
                old = tables.pop(unquote(m.group("rname")).lower(), None)
                if old is not None:
                    new = unquote(m.group("rnew"))
                    stats["renamed"] += 1
                    tables[new.lower()] = {
                        "name": new,
                        "file": f"{old['file']}, renamed in {path.name}",
                        "virtual": old["virtual"],
                    }
    return tables, stats


def split_cells(line: str) -> list[str]:
    """Cells of a Markdown table row; '|' inside a code span or escaped does not split."""
    s = line.strip()
    if s.startswith("|"):
        s = s[1:]
    if s.endswith("|") and not s.endswith("\\|"):
        s = s[:-1]
    cells: list[str] = []
    cur: list[str] = []
    in_code = False
    i = 0
    while i < len(s):
        c = s[i]
        if c == "\\" and i + 1 < len(s) and s[i + 1] == "|":
            cur.append("|")
            i += 2
            continue
        if c == "`":
            in_code = not in_code
        if c == "|" and not in_code:
            cells.append("".join(cur).strip())
            cur = []
        else:
            cur.append(c)
        i += 1
    cells.append("".join(cur).strip())
    return cells


def norm_header(cell: str) -> str:
    return " ".join(cell.replace("`", "").split()).lower()


def row_name(cell: str) -> str:
    m = re.search(r"`([^`]+)`", cell)
    return (m.group(1) if m else cell).strip()


def read_inventory(doc: Path, problems: list[str]) -> list[tuple[int, list[str]]]:
    lines = doc.read_text(encoding="utf-8").splitlines()
    starts = [i for i, line in enumerate(lines) if SECTION_RE.match(line)]
    if len(starts) != 1:
        problems.append(
            f"no-section: {doc.name} has {len(starts)} sections '## Data inventory' (exactly one is required)"
        )
        return []
    start = starts[0] + 1
    end = next((i for i in range(start, len(lines)) if HEADING_RE.match(lines[i])), len(lines))
    headers = [
        i
        for i in range(start, end - 1)
        if lines[i].lstrip().startswith("|")
        and not lines[i - 1].lstrip().startswith("|")
        and SEPARATOR_RE.match(lines[i + 1].strip())
    ]
    matching = [i for i in headers if [norm_header(c) for c in split_cells(lines[i])] == HEADER]
    if len(matching) != 1:
        found = "; ".join(
            "line %d: [%s]" % (i + 1, " | ".join(norm_header(c) for c in split_cells(lines[i])))
            for i in headers
        )
        problems.append(
            "bad-header: the section 'Data inventory' must hold exactly one table with the columns "
            "[%s]; it holds %d (tables found: %s)"
            % (" | ".join(HEADER), len(matching), found or "none")
        )
        return []
    header_at = matching[0]
    rows: list[tuple[int, list[str]]] = []
    i = header_at + 2
    while i < end and lines[i].lstrip().startswith("|"):
        rows.append((i + 1, split_cells(lines[i])))
        i += 1
    return rows


def check(migrations: Path, doc: Path) -> tuple[list[str], str]:
    problems: list[str] = []
    tables, stats = read_tables(migrations)
    rows = read_inventory(doc, problems)
    seen: dict[str, int] = {}
    for lineno, cells in rows:
        name = row_name(cells[0]) if cells else ""
        label = f"`{name}`" if name else f"(line {lineno})"
        if len(cells) != len(HEADER):
            problems.append(
                f"bad-row: line {lineno}, row {label} has {len(cells)} cells; the header has {len(HEADER)}"
            )
            continue
        for column, cell in zip(HEADER, cells):
            value = row_name(cell) if column == "table" else cell
            if value.strip().lower() in PLACEHOLDERS:
                problems.append(
                    f"empty-cell: line {lineno}, row {label}, column '{column}' is empty or a placeholder"
                )
        if not name:
            continue
        key = name.lower()
        if key in seen:
            problems.append(
                f"duplicate-row: line {lineno}, table `{name}` already has a row at line {seen[key]}"
            )
            continue
        seen[key] = lineno
        if key not in tables:
            problems.append(
                f"orphan-row: line {lineno}, row `{name}` names no table that "
                f"{migrations.name}/*.sql creates (or it was dropped or renamed)"
            )
    if not any(p.startswith(("no-section", "bad-header")) for p in problems):
        for key, info in tables.items():
            if key not in seen:
                kind = "virtual table" if info["virtual"] else "table"
                problems.append(
                    f"missing-row: {kind} `{info['name']}` ({info['file']}) has no row in the Data inventory"
                )
    summary = (
        f"{len(tables)} tables in {stats['files']} migrations ({stats['created']} CREATE TABLE, "
        f"{stats['virtual']} virtual, {stats['dropped']} dropped, {stats['renamed']} renamed, "
        f"{stats['temporary']} temporary not counted), {len(seen)} inventory rows"
    )
    return problems, summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--migrations", type=Path, default=None, help="directory of *.sql migrations")
    parser.add_argument("--doc", type=Path, default=None, help="document holding '## Data inventory'")
    args = parser.parse_args(argv)
    migrations = args.migrations or args.root / "crates" / "g9-storage" / "src" / "migrations"
    doc = args.doc or args.root / "docs" / "data-model.md"
    if not migrations.is_dir() or not any(migrations.glob("*.sql")):
        print(f"data inventory: ERROR no *.sql migrations in {migrations}", file=sys.stderr)
        return 2
    if not doc.is_file():
        print(f"data inventory: ERROR no document {doc}", file=sys.stderr)
        return 2
    problems, summary = check(migrations, doc)
    if problems:
        for p in problems:
            print(f"data inventory: FAIL {p}")
        print(f"data inventory: FAIL ({len(problems)} problem(s); {summary})")
        return 1
    print(f"data inventory: OK ({summary})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
