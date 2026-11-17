#!/usr/bin/env python3
"""check_backlog.py - one backlog: every open row of the defect register, the RTM and the improvement
backlog has an entry in the backlog map, and every entry points at plan steps or an ADR that exist
(plan step DOC-1, docs/release/path-to-10.md section 5.18).

Inputs (defaults are under --root, the repository root)
    --register      docs/audit/defect-register.md
    --rtm           docs/requirements/rtm.md
    --improvements  docs/audit/improvement-backlog.md
    --plan          docs/release/path-to-10.md
    --map           docs/release/backlog-map.json
    --adr-dir       docs/adr

Parsing rules, version 1.1.0 (the map records the version it was built under)
    Tables     Markdown pipe tables: a header line, a separator line, then rows; a table ends at the
               first line that does not start with '|'. Cells split on every '|' not escaped as
               '\\|', as GitHub-flavoured Markdown splits them (a code span does not protect a '|').
               Cells are compared with surrounding spaces removed. Lines inside fenced code blocks
               are not read.
    Sections   a section starts at its heading and ends at the next heading of level 1 or 2.
    Register   section "## A." (rows C01..C22), section "## B." (rows XXX-nn) and section "## E."
               (status re-assessment). In A and B the status is the last cell of the table whose
               header starts with "ID" and ends with "Status"; in E it is the "Status" cell of the
               table "ID | Status | Evidence | Remaining gap". A row of E overrides its row of B (E is
               the later, dated re-assessment); a disagreement is printed as a note. The status word
               is the first word of the cell, e.g. "PARTIAL (2026-11-13)" -> PARTIAL.
    RTM        the table whose header starts with "Req" (rows R-nnn) and the table whose header
               starts with "Gate" (rows An, Bn); the status is the last cell ("Status").
    Backlog    docs/audit/improvement-backlog.md has no status column: every row of a table whose
               header starts with "ID" (rows IMP-Mnn and XXX-nn) is open.
    Plan       step ids are the first cells of the tables headed exactly "ID | Who | Step | Done when"
               inside section "## 5."; the text of a step is its "Step" cell and its "Done when" cell,
               each read on its own (the "ID" and "Who" cells are not step text).

Status classification
    Register   open:   OPEN, PARTIAL, PROPOSED
               closed: IMPLEMENTED, IMPLEMENTED-IN-REPO, CORRECTED-IN-V6.0, ADDRESSED,
                       ADDRESSED-IN-DOCS, REFUTED
               delegated: IMPROVEMENT (tracked as a row of the improvement backlog, counted there)
               any other word: rule unknown-status
    RTM        open if the cell holds any of PARTIAL, PROPOSED, NOT RUN, BLOCKED, FAIL, FAILED,
               SKIPPED or a deferral "Stage nn"; otherwise closed if it holds IMPLEMENTED, TESTED or
               PASS; otherwise rule unknown-status. A mixed cell such as
               "IMPLEMENTED (runtime), BLOCKED (egress test on host NOT RUN)" is open.
    Backlog    every row is open.

Map format (JSON object)
    parsing_rules_version   must equal the version above
    unmapped_baseline       integer: the ratchet; the count of unmapped entries may not exceed it
    about                   optional one-line description
    entries                 {"<row id>": {"source", "status", "state", "steps", "adr", "note"}}
        source  docs/audit/defect-register.md | docs/requirements/rtm.md | docs/audit/improvement-backlog.md
        status  the row's status cell as written (null for the improvement backlog)
        state   "mapped" (steps or adr cover the open part) or "unmapped" (steps [] and adr null)
        steps   plan step ids; adr: "ADR-nnnn" or null
        note    one line, without a "file.ext:NN" line citation (cite a function, test or heading:
                line numbers drift and nothing checks them). A mapped note quotes, in double quotes,
                for every step it names and for its ADR, a phrase of at least 3 words and 15
                characters that appears verbatim in that step's "Step" or "Done when" cell, or in the
                ADR file. Shorter quoted text is not evidence of coverage.

Orphans: an unmapped entry is an open row with neither a step id nor an ADR. The plan's done
condition for DOC-1 is "0 orphans"; the default run allows up to unmapped_baseline of them (the
ratchet) and prints whether the done condition holds. --require-zero-orphans checks the done
condition itself: every unmapped entry is then a failure (rule orphan).

Rules (each failure line names its rule)
    missing-entry       an open row has no entry
    unknown-step        an entry names a step id that section 5 of the plan does not define
    unknown-adr         an entry names an ADR with no file <adr-dir>/<id>-*.md
    stale-entry         an entry's row is closed, delegated, or no longer in its document
    wrong-source        an entry names another document than the one that holds its open row
    status-drift        an entry's status differs from the row's current status cell
    no-cover            a mapped entry has no step and no ADR
    quote-missing       a mapped entry's note quotes no phrase (3 words, 15 characters) of one of
                        its steps or of its ADR
    line-citation       an entry's note cites a line number ("file.ext:NN")
    bad-unmapped        an unmapped entry names steps or an ADR
    orphan              with --require-zero-orphans only: an entry is unmapped
    bad-entry           an entry or the map is malformed
    ratchet             the count of unmapped entries exceeds unmapped_baseline
    rules-version       the map was built under other parsing rules
    unknown-status      a status cell has no word of the vocabulary above
    bad-row             a row has the wrong number of cells or an id of the wrong form
    duplicate-row       a row id occurs twice in a table set, or is open in two documents
    orphan-improvement  a register row with status IMPROVEMENT has no improvement-backlog row

Notes (printed, not failures): a register row whose section B and section E statuses differ; an
unmapped count below unmapped_baseline (lower the baseline: the ratchet only goes down).

Exit status: 0 every rule holds (prints open, mapped and unmapped per document), 1 a rule failed,
2 an input is missing, is not valid JSON, or lacks a section or table named above.

Usage (standard library only; no network; deterministic; reads only):
    python3 -I scripts/check_backlog.py [--root DIR] [--register F] [--rtm F] [--improvements F]
                                        [--plan F] [--map F] [--adr-dir DIR]
                                        [--require-zero-orphans]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

PARSING_RULES_VERSION = "1.1.0"

SRC_REGISTER = "docs/audit/defect-register.md"
SRC_RTM = "docs/requirements/rtm.md"
SRC_IMPROVEMENTS = "docs/audit/improvement-backlog.md"
SOURCES = (SRC_REGISTER, SRC_RTM, SRC_IMPROVEMENTS)

REGISTER_OPEN = frozenset({"OPEN", "PARTIAL", "PROPOSED"})
REGISTER_CLOSED = frozenset(
    {"IMPLEMENTED", "IMPLEMENTED-IN-REPO", "CORRECTED-IN-V6.0", "ADDRESSED", "ADDRESSED-IN-DOCS", "REFUTED"}
)
REGISTER_DELEGATED = frozenset({"IMPROVEMENT"})

RTM_OPEN = re.compile(r"\b(?:PARTIAL|PROPOSED|NOT RUN|BLOCKED|FAIL|FAILED|SKIPPED)\b|\bStage \d+\b")
RTM_CLOSED = re.compile(r"\b(?:IMPLEMENTED|TESTED|PASS)\b")

PLAN_HEADER = ["ID", "Who", "Step", "Done when"]
STEP_ID = re.compile(r"^[A-Z][A-Z0-9]*-\d+$")
AUDITOR_ID = re.compile(r"^[A-Z]{3}-\d{2}$")
CORRECTION_ID = re.compile(r"^C\d{2}$")
REQ_ID = re.compile(r"^R-\d{3}$")
GATE_ID = re.compile(r"^[AB]\d{2}$")
BACKLOG_ID = re.compile(r"^(?:IMP-M\d{2}|[A-Z]{3}-\d{2})$")
ADR_ID = re.compile(r"^ADR-\d{4}$")
ENTRY_KEYS = {"source", "status", "state", "steps", "adr", "note"}
TOP_KEYS = {"parsing_rules_version", "unmapped_baseline", "about", "entries"}
QUOTE = re.compile(r'"([^"]+)"')
MIN_QUOTE_WORDS = 3
MIN_QUOTE_CHARS = 15
LINE_CITATION = re.compile(r"[\w./-]+\.[A-Za-z][A-Za-z0-9]{0,4}:\d+")


class StructureError(Exception):
    """A document lacks a section or table this script reads (exit 2)."""


# --------------------------------------------------------------------------------------------------
# Markdown tables


def split_cells(line: str) -> list[str]:
    """Cells of one table line, split as GitHub-flavoured Markdown splits them: on every '|' that is
    not escaped as '\\|' (a code span does not protect a '|'; the documents escape such pipes)."""
    text = line.strip()
    if text.startswith("|"):
        text = text[1:]
    if text.endswith("|") and not text.endswith("\\|"):
        text = text[:-1]
    cells = re.split(r"(?<!\\)\|", text)
    return [c.replace("\\|", "|").strip() for c in cells]


def is_separator(cells: list[str]) -> bool:
    return bool(cells) and all(re.fullmatch(r":?-{1,}:?", c) for c in cells)


def tables(lines: list[str], start: int, end: int) -> list[tuple[int, list[str], list[tuple[int, list[str]]]]]:
    """Tables between line indexes [start, end): (header line number, header cells, [(line number, cells)])."""
    found = []
    i = start
    while i < end:
        if lines[i].lstrip().startswith("|"):
            block = []
            while i < end and lines[i].lstrip().startswith("|"):
                block.append(i)
                i += 1
            if len(block) >= 2 and is_separator(split_cells(lines[block[1]])):
                header = split_cells(lines[block[0]])
                rows = [(k + 1, split_cells(lines[k])) for k in block[2:]]
                found.append((block[0] + 1, header, rows))
            continue
        i += 1
    return found


def section(lines: list[str], prefix: str, doc: str) -> tuple[int, int]:
    """Line range [start, end) of the one section whose heading starts with `prefix`."""
    starts = [i for i, line in enumerate(lines) if line.startswith(prefix)]
    if len(starts) != 1:
        raise StructureError(f"{doc}: expected one heading starting with '{prefix}', found {len(starts)}")
    s = starts[0]
    end = next((i for i in range(s + 1, len(lines)) if re.match(r"^#{1,2}\s", lines[i])), len(lines))
    return s + 1, end


def read_lines(path: Path) -> list[str]:
    """Lines of a document; lines inside fenced code blocks (``` or ~~~) are blanked, so a heading or
    a table inside a code block is not read. Line numbers are kept."""
    lines = path.read_text(encoding="utf-8").split("\n")
    fence: str | None = None
    for i, line in enumerate(lines):
        m = re.match(r"^\s{0,3}(`{3,}|~{3,})", line)
        if fence is None and m:
            fence = m.group(1)[0] * len(m.group(1))
            lines[i] = ""
        elif fence is not None:
            if re.match(r"^\s{0,3}" + re.escape(fence[0]) + "{" + str(len(fence)) + r",}\s*$", line):
                fence = None
            lines[i] = ""
    return lines


def norm(text: str) -> str:
    return " ".join(text.split())


def long_enough(quote: str) -> bool:
    """A quoted phrase counts as evidence of coverage only from 3 words and 15 characters on: a short
    quote ("A", "DOC-1") matches almost any step and proves nothing."""
    return len(quote) >= MIN_QUOTE_CHARS and len(quote.split()) >= MIN_QUOTE_WORDS


# --------------------------------------------------------------------------------------------------
# Documents


class Row:
    """One row of a status document: its id, document, status as written, state and line number."""

    __slots__ = ("rid", "source", "status", "state", "line")

    def __init__(self, rid: str, source: str, status: str | None, state: str, line: int) -> None:
        self.rid = rid
        self.source = source
        self.status = status
        self.state = state  # open | closed | delegated
        self.line = line


def row_problem(doc: str, lineno: int, cells: list[str], header: list[str], id_re: re.Pattern[str]) -> str | None:
    """The bad-row message for a row with the wrong cell count or an id of the wrong form, else None."""
    if len(cells) != len(header):
        return f"bad-row: {doc}:{lineno} has {len(cells)} cells; its table header has {len(header)}"
    if not id_re.match(cells[0]):
        return f"bad-row: {doc}:{lineno} first cell {cells[0]!r} is not an id of the form {id_re.pattern}"
    return None


def status_word(cell: str) -> str:
    m = re.match(r"[A-Z][A-Z0-9.\-]*[A-Z0-9]", cell.strip())
    return m.group(0) if m else ""


def parse_register(path: Path, problems: list[str], notes: list[str]) -> dict[str, Row]:
    lines = read_lines(path)
    doc = SRC_REGISTER
    rows: dict[str, Row] = {}

    def status_tables(prefix: str, id_re: re.Pattern[str]) -> list[tuple[str, str, int]]:
        start, end = section(lines, prefix, doc)
        picked = [t for t in tables(lines, start, end) if t[1] and t[1][0] == "ID" and t[1][-1] == "Status"]
        if not picked:
            raise StructureError(f"{doc}: section '{prefix}' has no table headed 'ID ... Status'")
        out = []
        for _hline, header, body in picked:
            for lineno, cells in body:
                bad = row_problem(doc, lineno, cells, header, id_re)
                if bad:
                    problems.append(bad)
                    continue
                out.append((cells[0], cells[-1], lineno))
        return out

    a_rows = status_tables("## A.", CORRECTION_ID)
    b_rows = status_tables("## B.", AUDITOR_ID)
    e_start, e_end = section(lines, "## E.", doc)
    e_tables = [t for t in tables(lines, e_start, e_end) if t[1][:2] == ["ID", "Status"]]
    if not e_tables:
        raise StructureError(f"{doc}: section '## E.' has no table headed 'ID | Status | ...'")
    e_status: dict[str, tuple[str, int]] = {}
    for _hline, header, body in e_tables:
        for lineno, cells in body:
            bad = row_problem(doc, lineno, cells, header, AUDITOR_ID)
            if bad:
                problems.append(bad)
                continue
            if cells[0] in e_status:
                problems.append(f"duplicate-row: {doc}:{lineno} repeats section E row {cells[0]}")
            e_status[cells[0]] = (cells[1], lineno)

    seen: set[str] = set()
    for rid, status, lineno in a_rows + b_rows:
        if rid in seen:
            problems.append(f"duplicate-row: {doc}:{lineno} repeats row {rid}")
            continue
        seen.add(rid)
        effective, where = status, lineno
        if rid in e_status:
            e_cell, e_line = e_status[rid]
            if norm(e_cell) != norm(status):
                notes.append(
                    f"{doc}: {rid} has status '{norm(status)}' in section B (line {lineno}) and "
                    f"'{norm(e_cell)}' in section E (line {e_line}); section E is used"
                )
            effective, where = e_cell, e_line
        word = status_word(effective)
        if word in REGISTER_OPEN:
            state = "open"
        elif word in REGISTER_CLOSED:
            state = "closed"
        elif word in REGISTER_DELEGATED:
            state = "delegated"
        else:
            problems.append(f"unknown-status: {doc}:{where} row {rid} has status '{norm(effective)}'")
            continue
        rows[rid] = Row(rid, doc, norm(effective), state, where)
    for rid, (_cell, e_line) in e_status.items():
        if rid not in seen:
            problems.append(f"bad-row: {doc}:{e_line} section E row {rid} has no row in section B")
    if not rows and not problems:
        raise StructureError(f"{doc}: no register rows found")
    return rows


def parse_rtm(path: Path, problems: list[str]) -> dict[str, Row]:
    lines = read_lines(path)
    doc = SRC_RTM
    rows: dict[str, Row] = {}
    found = tables(lines, 0, len(lines))
    for first, id_re in (("Req", REQ_ID), ("Gate", GATE_ID)):
        picked = [t for t in found if t[1] and t[1][0] == first and t[1][-1] == "Status"]
        if len(picked) != 1:
            raise StructureError(f"{doc}: expected one table headed '{first} ... Status', found {len(picked)}")
        _hline, header, body = picked[0]
        for lineno, cells in body:
            bad = row_problem(doc, lineno, cells, header, id_re)
            if bad:
                problems.append(bad)
                continue
            rid, status = cells[0], cells[-1]
            if rid in rows:
                problems.append(f"duplicate-row: {doc}:{lineno} repeats row {rid}")
                continue
            if RTM_OPEN.search(status):
                state = "open"
            elif RTM_CLOSED.search(status):
                state = "closed"
            else:
                problems.append(f"unknown-status: {doc}:{lineno} row {rid} has status '{norm(status)}'")
                continue
            rows[rid] = Row(rid, doc, norm(status), state, lineno)
    return rows


def parse_improvements(path: Path, problems: list[str]) -> dict[str, Row]:
    lines = read_lines(path)
    doc = SRC_IMPROVEMENTS
    rows: dict[str, Row] = {}
    picked = [t for t in tables(lines, 0, len(lines)) if t[1] and t[1][0] == "ID"]
    for _hline, header, body in picked:
        for lineno, cells in body:
            bad = row_problem(doc, lineno, cells, header, BACKLOG_ID)
            if bad:
                problems.append(bad)
                continue
            if cells[0] in rows:
                problems.append(f"duplicate-row: {doc}:{lineno} repeats row {cells[0]}")
                continue
            rows[cells[0]] = Row(cells[0], doc, None, "open", lineno)
    if not rows and not problems:
        raise StructureError(f"{doc}: no table headed 'ID ...' with rows")
    return rows


def parse_plan(path: Path, problems: list[str]) -> dict[str, tuple[str, str]]:
    """Step id -> (Step cell, Done when cell), whitespace-normalised."""
    lines = read_lines(path)
    start, end = section(lines, "## 5.", f"{path}")
    steps: dict[str, tuple[str, str]] = {}
    for _hline, header, body in tables(lines, start, end):
        if header != PLAN_HEADER:
            continue
        for lineno, cells in body:
            bad = row_problem(str(path), lineno, cells, PLAN_HEADER, STEP_ID)
            if bad:
                problems.append(bad)
                continue
            if cells[0] in steps:
                problems.append(f"duplicate-row: {path}:{lineno} repeats step {cells[0]}")
                continue
            steps[cells[0]] = (norm(cells[2]), norm(cells[3]))
    if not steps:
        raise StructureError(f"{path}: section '## 5.' has no table headed '{' | '.join(PLAN_HEADER)}'")
    return steps


def adr_text(adr_dir: Path, adr: str) -> str | None:
    files = sorted(adr_dir.glob(f"{adr}-*.md"))
    if len(files) != 1:
        return None
    return norm(files[0].read_text(encoding="utf-8"))


# --------------------------------------------------------------------------------------------------
# The check


def check(args: argparse.Namespace) -> tuple[list[str], list[str], list[str]]:
    problems: list[str] = []
    notes: list[str] = []
    register = parse_register(args.register, problems, notes)
    rtm = parse_rtm(args.rtm, problems)
    improvements = parse_improvements(args.improvements, problems)
    steps = parse_plan(args.plan, problems)
    try:
        data = json.loads(args.map.read_text(encoding="utf-8"))
    except json.JSONDecodeError as e:
        raise StructureError(f"{args.map}: not valid JSON ({e})") from e

    # Open rows of the three documents, keyed by id.
    every: dict[str, list[Row]] = {}
    for rows in (register, rtm, improvements):
        for row in rows.values():
            every.setdefault(row.rid, []).append(row)
    open_rows: dict[str, Row] = {}
    for rid, found in every.items():
        live = [r for r in found if r.state == "open"]
        if len(live) > 1:
            problems.append(
                f"duplicate-row: {rid} is open in {len(live)} documents ({', '.join(r.source for r in live)})"
            )
        if live:
            open_rows[rid] = live[0]
    delegated = sorted(r.rid for r in register.values() if r.state == "delegated")
    for rid in delegated:
        if rid not in improvements:
            problems.append(
                f"orphan-improvement: {SRC_REGISTER}:{register[rid].line} row {rid} has status IMPROVEMENT "
                f"but no row in {SRC_IMPROVEMENTS}"
            )

    if not isinstance(data, dict):
        raise StructureError(f"{args.map}: the map is not a JSON object")
    extra = set(data) - TOP_KEYS
    for key in sorted(extra):
        problems.append(f"bad-entry: the map has an unknown top-level key {key!r}")
    if data.get("parsing_rules_version") != PARSING_RULES_VERSION:
        problems.append(
            f"rules-version: the map was built under parsing rules {data.get('parsing_rules_version')!r}; "
            f"this script implements {PARSING_RULES_VERSION!r} (re-review the map, then update it)"
        )
    baseline = data.get("unmapped_baseline")
    if not isinstance(baseline, int) or isinstance(baseline, bool) or baseline < 0:
        problems.append(f"bad-entry: unmapped_baseline must be a non-negative integer, not {baseline!r}")
        baseline = None
    entries = data.get("entries")
    if not isinstance(entries, dict):
        raise StructureError(f"{args.map}: 'entries' is not a JSON object")

    adr_cache: dict[str, str | None] = {}
    counts = {s: {"open": 0, "mapped": 0, "unmapped": 0} for s in SOURCES}
    unmapped_total = 0
    for rid, entry in entries.items():
        where = f"entry {rid}"
        if not isinstance(entry, dict) or set(entry) != ENTRY_KEYS:
            keys = sorted(entry) if isinstance(entry, dict) else type(entry).__name__
            problems.append(f"bad-entry: {where} must have exactly the keys {sorted(ENTRY_KEYS)}, has {keys}")
            continue
        source, status, state = entry["source"], entry["status"], entry["state"]
        steps_of, adr, note = entry["steps"], entry["adr"], entry["note"]
        if source not in SOURCES:
            problems.append(f"bad-entry: {where} has source {source!r}, not one of {list(SOURCES)}")
            continue
        if state not in ("mapped", "unmapped"):
            problems.append(f"bad-entry: {where} has state {state!r}, not 'mapped' or 'unmapped'")
            continue
        if not isinstance(steps_of, list) or not all(isinstance(s, str) for s in steps_of):
            problems.append(f"bad-entry: {where} steps must be a list of step ids")
            continue
        if len(set(steps_of)) != len(steps_of):
            problems.append(f"bad-entry: {where} repeats a step id")
        if adr is not None and (not isinstance(adr, str) or not ADR_ID.match(adr)):
            problems.append(f"bad-entry: {where} adr must be null or 'ADR-nnnn', not {adr!r}")
            continue
        if not isinstance(note, str) or not note.strip() or "\n" in note:
            problems.append(f"bad-entry: {where} note must be one non-empty line")
            continue
        cited = LINE_CITATION.search(note)
        if cited:
            problems.append(
                f"line-citation: {where} note cites a line number ({cited.group(0)!r}); line numbers drift "
                f"and nothing checks them: cite a function, test or heading"
            )

        # The row behind the entry.
        row = open_rows.get(rid)
        if state == "unmapped":
            unmapped_total += 1
            if args.require_zero_orphans:
                at = f"{row.source}:{row.line}" if row is not None else source
                problems.append(f"orphan: {where} ({at}) has neither a plan step nor an ADR: {note}")
        if row is None:
            known = every.get(rid)
            if known:
                desc = "; ".join(f"{r.source} status '{r.status}' ({r.state})" for r in known)
                problems.append(f"stale-entry: {where} is no longer open: {desc}")
            else:
                problems.append(f"stale-entry: {where} names a row that no longer exists in {source}")
        elif row.source != source:
            problems.append(f"wrong-source: {where} names {source}, but the open row is in {row.source}")
        else:
            counts[source][state] += 1
            if source == SRC_IMPROVEMENTS:
                if status is not None:
                    problems.append(f"status-drift: {where} has status {status!r}; {SRC_IMPROVEMENTS} has no status column (use null)")
            elif not isinstance(status, str) or norm(status) != row.status:
                problems.append(
                    f"status-drift: {where} records status {status!r}; {source}:{row.line} now reads "
                    f"'{row.status}' (re-review the mapping, then update the status)"
                )

        for step in steps_of:
            if step not in steps:
                problems.append(f"unknown-step: {where} names step {step}, which section 5 of the plan does not define")
        if adr is not None and adr not in adr_cache:
            adr_cache[adr] = adr_text(args.adr_dir, adr)
        if adr is not None and adr_cache[adr] is None:
            problems.append(f"unknown-adr: {where} names {adr}; no single file {args.adr_dir}/{adr}-*.md")

        if state == "mapped":
            if not steps_of and adr is None:
                problems.append(f"no-cover: {where} is mapped but names no step and no ADR")
            else:
                quotes = [q for q in (norm(x) for x in QUOTE.findall(note)) if long_enough(q)]
                covers: list[tuple[str, tuple[str, ...]]] = [(s, steps[s]) for s in steps_of if s in steps]
                if adr is not None and adr_cache.get(adr):
                    covers.append((adr, (adr_cache[adr] or "",)))
                for name, cells in covers:
                    if not any(q in cell for q in quotes for cell in cells):
                        what = "its ADR file" if name == adr else "its 'Step' or 'Done when' cell"
                        problems.append(
                            f"quote-missing: {where} names {name}, but its note quotes no phrase of at least "
                            f"{MIN_QUOTE_WORDS} words and {MIN_QUOTE_CHARS} characters found verbatim in {what}"
                        )
        elif steps_of or adr is not None:
            problems.append(f"bad-unmapped: {where} is unmapped but names steps {steps_of} or ADR {adr!r}")

    for rid, row in sorted(open_rows.items(), key=lambda kv: (SOURCES.index(kv[1].source), kv[1].line)):
        counts[row.source]["open"] += 1
        if rid not in entries:
            status = f"status '{row.status}'" if row.status is not None else "no status column"
            problems.append(f"missing-entry: {row.source}:{row.line} row {rid} ({status}) is open and has no entry")

    if baseline is not None:
        if unmapped_total > baseline:
            problems.append(
                f"ratchet: {unmapped_total} unmapped entries exceed unmapped_baseline {baseline} "
                f"(map the new rows to plan steps, or add a step to the plan)"
            )
        elif unmapped_total < baseline:
            notes.append(f"{unmapped_total} unmapped entries, below unmapped_baseline {baseline}: lower the baseline")

    width = max(len(s) for s in SOURCES)
    summary = [
        f"{s:<{width}}  open {c['open']:>3}  mapped {c['mapped']:>3}  unmapped {c['unmapped']:>3}"
        for s, c in counts.items()
    ]
    total = {k: sum(c[k] for c in counts.values()) for k in ("open", "mapped", "unmapped")}
    summary.append(
        f"{'total':<{width}}  open {total['open']:>3}  mapped {total['mapped']:>3}  unmapped {total['unmapped']:>3}"
        f"  (baseline {baseline}; {len(steps)} plan steps; {len(delegated)} register IMPROVEMENT rows "
        f"counted in {SRC_IMPROVEMENTS})"
    )
    if unmapped_total == 0:
        summary.append("DOC-1 done condition '0 orphans': met")
    else:
        summary.append(
            f"DOC-1 done condition '0 orphans': NOT MET, {unmapped_total} orphan(s) (unmapped entries; "
            f"--require-zero-orphans lists them)"
        )
    return problems, notes, summary


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--register", type=Path, default=None)
    parser.add_argument("--rtm", type=Path, default=None)
    parser.add_argument("--improvements", type=Path, default=None)
    parser.add_argument("--plan", type=Path, default=None)
    parser.add_argument("--map", type=Path, default=None)
    parser.add_argument("--adr-dir", type=Path, default=None)
    parser.add_argument(
        "--require-zero-orphans",
        action="store_true",
        help="check DOC-1's done condition: every unmapped entry is a failure (rule orphan)",
    )
    args = parser.parse_args(argv)
    args.register = args.register or args.root / SRC_REGISTER
    args.rtm = args.rtm or args.root / SRC_RTM
    args.improvements = args.improvements or args.root / SRC_IMPROVEMENTS
    args.plan = args.plan or args.root / "docs" / "release" / "path-to-10.md"
    args.map = args.map or args.root / "docs" / "release" / "backlog-map.json"
    args.adr_dir = args.adr_dir or args.root / "docs" / "adr"
    for name in ("register", "rtm", "improvements", "plan", "map"):
        if not getattr(args, name).is_file():
            print(f"backlog: ERROR no {name} file {getattr(args, name)}", file=sys.stderr)
            return 2
    if not args.adr_dir.is_dir():
        print(f"backlog: ERROR no ADR directory {args.adr_dir}", file=sys.stderr)
        return 2
    try:
        problems, notes, summary = check(args)
    except StructureError as e:
        print(f"backlog: ERROR {e}", file=sys.stderr)
        return 2
    for note in notes:
        print(f"backlog: note: {note}")
    if problems:
        for p in problems:
            print(f"backlog: FAIL {p}")
        print(f"backlog: FAIL ({len(problems)} problem(s))")
        for line in summary:
            print(f"backlog:   {line}")
        return 1
    print(f"backlog: OK (parsing rules {PARSING_RULES_VERSION})")
    for line in summary:
        print(f"backlog:   {line}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
