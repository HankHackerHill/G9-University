"""Black-box tests for scripts/build_curriculum_v2.py (the builder of the v2.0 courseware, ADR-0023).

The script resolves the repository root from its own location, so every test runs a byte-for-byte copy of it at
<tmp>/scripts/build_curriculum_v2.py next to a copy of the plan and of the competency packs and a synthetic courseware
written by scripts/tests/_v2_corpus.py. No real courseware is written or read (the repository's WEEKS/ folder is not
an input: other authors write it while these tests run). --write is never run on the repository: run_gate() refuses it
for the repository script, and the write tests below run it on temporary copies.

Two templates are built once per run: ONE (a complete, valid week 1) and TWELVE (all 12 weeks, with the generated
files written by --write). Every test works on its own copy of one of them.

Rules, one case each (CASES: label, mutation, message the rule names):
  seed          a damaged day table of section 5 of the plan: a day row missing, an empty cell, a weekday that is not the
                calendar's, a week heading missing, malformed, out of order, a section 5 heading missing, a fifth cell, a
                row twice, a seed text over 500 characters; Markdown emphasis removed and nothing else changed
  files         every authored file present; no unexpected file; a week that is not written
  day guide     title, the seven metadata lines, the sections (known, once, in order), the block headings, the objectives
                (three, the seed text of the column, competency ids), the hand-in, the resources table (ids in a registry,
                block letters, one to eight rows, the hands-on rule), words per block and per file, the Friday references,
                the commands table, the mapping table, the quiz (identifiers, counts, one paragraph), the hint ladder, the
                seven troubleshooting lines, the evidence labels, the expert extension, the spaced-recall lines, the
                reflection lines, hidden characters, links, HTML, fences, a second title
  day key       title, notice, the eight entries with Answer, Marking, Source, the three sections, a source id in a registry
  assessment    metadata (code, kind, competency, difficulty and its band, title), the sections, 20 questions with their
                identifiers and kinds, the mix, the multiple-choice count and options, INTERPRET with a sample, the task
                statement length, no link or HTML
  assessment key  20 entries, Answer and Why, Distractors for multiple choice (each other option named), the option letter,
                the practical section with the four score parts, the gate section
  week guide, labs, fixtures, evidence checklist
  registry      JSON, the keys, the id, the https address, the enums, a duplicate id, the week of the file, a fallback id
  leaks         a line that begins with an answer marker; the answer text of a key in a learner file; a multiple-choice
                answer is not a leak (the learner file lists the options by design)
  corpus        an identifier used twice; two questions with the same text
  generated     every generated file current (changed, missing, orphaned, source edited without regenerating)
Not reachable through the inputs: the check "identifiers are unique across the corpus" cannot be reached with a valid
identifier in a second week (an identifier carries its week); it is reached here by a repeated identifier inside a day.
The seed parser's "week heading is out of order" and the "five day rows" rules are reached through the plan copy.
HIDDEN is scripts/build_curriculum.py's set (Sanity.test_hidden_matches_the_v1_builder pins that).
"""

from __future__ import annotations

import ast
import datetime
import importlib.util
import json
import os
import re
import shutil
import tempfile
import unittest
from pathlib import Path
from typing import Callable

import _v2_corpus as corpus
from _gate_helpers import (
    NEEDS_JSONSCHEMA,
    REPO,
    GateTestCase,
    InputGuard,
    copy_into,
    digest_inputs,
    edit_json,
    load_json,
    output_of,
    read_text,
    replace_once,
    run_gate,
    stage_script,
    write_text,
)

SCRIPT = "build_curriculum_v2.py"
PLAN_REL = corpus.PLAN_REL
PACKS_REL = corpus.PACKS_REL
WEEKS_REL = corpus.WEEKS_REL
DATA = "data/curriculum"
SEED_JSON = f"{DATA}/g9-cyber-mastery-480.seed.v2.json"
DRAFT_JSON = f"{DATA}/g9-cyber-mastery-480.draft.v2.json"
SHEETS_JSON = f"{DATA}/g9-cyber-mastery-480.sheets.v2.json"
KEYS_JSON = f"{DATA}/g9-cyber-mastery-480.keys.v2.json"
ASSESSMENTS = f"{DATA}/assessments-v2"
V2 = "docs/curriculum/v2"
SCHEDULE = f"{V2}/SCHEDULE_60_DAYS.md"
MAP = f"{V2}/CURRICULUM_MAP.md"
REGISTRY_MD = f"{V2}/RESOURCE_REGISTRY.md"
BANK = f"{V2}/ASSESSMENTS/QUESTION_BANK.md"
ANSWERS = f"{V2}/ASSESSMENTS/ANSWER_KEY.md"
RECALL = f"{V2}/ASSESSMENTS/SPACED_RECALL.md"
GENERATED_FILES = 4 + 12 + 60 + 6  # seed, draft, sheets, keys; assessments; quizzes; six Markdown indexes

# The repository inputs these tests read. WEEKS/ is not among them: other authors write it while the tests run.
_GUARD = InputGuard([PLAN_REL, f"{V2}/AUTHORING.md", PACKS_REL])

PLAN = corpus.read_plan(REPO)
SEEDS = PLAN["seeds"]
_STATE: dict[str, Path] = {}
_HOLDER: tempfile.TemporaryDirectory | None = None


def _base_tree(dst: Path) -> None:
    copy_into(dst, [PLAN_REL, PACKS_REL])
    stage_script(dst, SCRIPT)


def setUpModule() -> None:
    global _HOLDER
    _GUARD.snapshot()
    _HOLDER = tempfile.TemporaryDirectory(prefix="g9-v2-templates-")
    base = Path(_HOLDER.name).resolve()
    one = base / "one"
    _base_tree(one)
    corpus.write_week(one, PLAN, 1)
    twelve = base / "twelve"
    _base_tree(twelve)
    corpus.write_corpus(twelve, PLAN)
    proc = run_gate(twelve / "scripts" / SCRIPT, "--write", cwd=twelve)
    if proc.returncode != 0:
        raise AssertionError(f"the synthetic 12-week corpus does not build:\n{output_of(proc)[-3000:]}")
    _STATE["one"] = one
    _STATE["twelve"] = twelve


def tearDownModule() -> None:
    _GUARD.verify()
    if _HOLDER is not None:
        _HOLDER.cleanup()


# ---------------------------------------------------------------------------------------------------
# a temporary copy of a template and the edits a case makes to it
# ---------------------------------------------------------------------------------------------------


class Tree:
    """Mutations of one temporary copy. Paths of the helpers are relative to WEEKS/W01 unless said otherwise;
    every edit raises if its target text is absent, so a mutation never silently does nothing."""

    def __init__(self, root: Path) -> None:
        self.root = root

    def at(self, rel: str, week: int = 1) -> Path:
        return self.root / WEEKS_REL / f"W{week:02d}" / rel

    def repo(self, rel: str) -> Path:
        return self.root / rel

    def sub(self, rel: str, old: str, new: str, after: str | None = None, week: int = 1) -> None:
        replace_once(self.at(rel, week), old, new, after=after)

    def append(self, rel: str, extra: str, week: int = 1) -> None:
        path = self.at(rel, week)
        write_text(path, read_text(path) + extra)

    def delete(self, rel: str, week: int = 1) -> None:
        self.at(rel, week).unlink()

    def put(self, rel: str, text: str, week: int = 1) -> None:
        write_text(self.at(rel, week), text)

    def _lines(self, rel: str, week: int) -> tuple[Path, list[str]]:
        path = self.at(rel, week)
        return path, read_text(path).split("\n")

    def set_line(self, rel: str, prefix: str, new: str, week: int = 1) -> None:
        """Replace the first whole line that starts with `prefix`."""
        path, lines = self._lines(rel, week)
        idx = next((i for i, line in enumerate(lines) if line.startswith(prefix)), None)
        if idx is None:
            raise AssertionError(f"no line starts with {prefix!r} in {path.name}")
        lines[idx] = new
        write_text(path, "\n".join(lines))

    def del_line(self, rel: str, prefix: str, week: int = 1) -> None:
        path, lines = self._lines(rel, week)
        idx = next((i for i, line in enumerate(lines) if line.startswith(prefix)), None)
        if idx is None:
            raise AssertionError(f"no line starts with {prefix!r} in {path.name}")
        del lines[idx]
        write_text(path, "\n".join(lines))

    def insert_after(self, rel: str, prefix: str, new: str, week: int = 1) -> None:
        path, lines = self._lines(rel, week)
        idx = next((i for i, line in enumerate(lines) if line.startswith(prefix)), None)
        if idx is None:
            raise AssertionError(f"no line starts with {prefix!r} in {path.name}")
        lines.insert(idx + 1, new)
        write_text(path, "\n".join(lines))

    def set_section(self, rel: str, heading: str, body: str, week: int = 1) -> None:
        """Replace everything between the line `heading` and the next level-2 heading (or the end)."""
        path = self.at(rel, week)
        text = read_text(path)
        start = text.index(heading + "\n") + len(heading) + 1
        end = text.find("\n## ", start)
        if end < 0:
            end = len(text)
        write_text(path, text[:start] + body + "\n" + text[end:])

    def cut(self, rel: str, start: str, end: str, week: int = 1) -> None:
        """Delete the text from `start` up to (not including) `end`."""
        path = self.at(rel, week)
        text = read_text(path)
        i = text.index(start)
        j = text.index(end, i + len(start))
        write_text(path, text[:i] + text[j:])

    def registry(self, fn: Callable[[dict], None], week: int = 1) -> None:
        edit_json(self.at("RESOURCES.json", week), fn)

    def plan(self, fn: Callable[[str], str]) -> None:
        path = self.repo(PLAN_REL)
        write_text(path, fn(read_text(path)))


class TreeCase(GateTestCase):
    """A fresh copy of a template per test."""

    template = "one"

    def setUp(self) -> None:
        super().setUp()
        self.root = self.tmp / "tree"
        shutil.copytree(_STATE[self.template], self.root)
        self.script = self.root / "scripts" / SCRIPT
        self.tree = Tree(self.root)

    def build(self, *args: str):
        return run_gate(self.script, *args, cwd=self.root)

    def run_case(self, mutate: Callable[[Tree], None], needles: list[str], args: tuple[str, ...] = ("--week", "1")) -> None:
        mutate(self.tree)
        proc = self.build(*args)
        self.assertExit(proc, 1)
        out = output_of(proc)
        for needle in needles:
            self.assertTrue(needle in out, f"{needle!r} missing; output tail:\n{out[-3500:]}")

    def read(self, rel: str) -> str:
        return read_text(self.root / rel)


# ---------------------------------------------------------------------------------------------------
# anchors in the synthetic week 1 (they follow scripts/tests/_v2_corpus.py)
# ---------------------------------------------------------------------------------------------------

D2 = "DAY_02.md"
D3 = "DAY_03.md"
D5 = "DAY_05.md"
K2 = "ANSWER_KEYS/DAY_02_KEY.md"
AS = "ASSESSMENT.md"
AK = "ANSWER_KEYS/ASSESSMENT_KEY.md"
ROW_DOCS = "| w01-synthetic-docs | Read the canonical page and keep its title and date as evidence | A |"
ROW_MODULE = "| `w01-synthetic-module` | Do the exercise the module names and keep the completion record | C |"
BLOCK_A = "## A — Synthetic block A (120 min)"
BLOCK_B = "## B — Synthetic block B (120 min)"
BLOCK_D = "## D — Synthetic block D (60 min)"
CMD_ROW_1 = "| 1 | `ls -l /srv/lab1` |"
LAB_AUTH = "- Lab authorization: the synthetic range 10.77.10.0/24 RED only, owned guests, written scope SCOPE-W01-02"


def seed(n: int, k: int) -> str:
    return SEEDS[n][k - 1]


def obj_prefix(n: int, k: int) -> str:
    return f"{k}. {seed(n, k)} — "


def obj_line(n: int, k: int, competencies: str) -> str:
    return f"{obj_prefix(n, k)}the learner shows one dated observation number {k} for day {n}. [{competencies}]"


def swap_headings(t: Tree) -> None:
    t.sub(D2, "## Hint ladder", "## PLACEHOLDER")
    t.sub(D2, "## Quiz", "## Hint ladder")
    t.sub(D2, "## PLACEHOLDER", "## Quiz")


def add_resource_rows(t: Tree) -> None:
    rows = [f"| w01-synthetic-docs | use {b} | {b} |" for b in "RBCD"] + [f"| w01-synthetic-module | use {b} | {b} |" for b in "ABD"]
    t.insert_after(D2, ROW_MODULE, "\n".join(rows))


def drop_commands_row(t: Tree) -> None:
    t.del_line(D2, "| 3 | `ls -l /srv/lab3`")


def local_lab_only(t: Tree) -> None:
    t.sub(D2, LAB_AUTH, LAB_AUTH + "; this day is local-lab only")
    t.sub(D2, ROW_MODULE, ROW_MODULE.replace("| C |", "| A |"))


def words_too_few(t: Tree) -> None:
    for heading in (BLOCK_A, BLOCK_B, "## C — Synthetic block C (150 min)"):
        t.set_section(D2, heading, "Too short.")


def blocks_without_friday_reference(t: Tree) -> None:
    t.sub(D5, " The knowledge exam is in ASSESSMENT.md, Part 1.", "")


def quiz_two_paragraphs(t: Tree) -> None:
    t.sub(D2, "[W01D02-R1] " + corpus.recall_text(2, 1), "[W01D02-R1] " + corpus.recall_text(2, 1) + "\n\nA second paragraph of the same question.")


def quiz_text_outside(t: Tree) -> None:
    t.sub(D2, "## Quiz\n", "## Quiz\nA stray sentence before the first subsection.\n")


def key_drop_entry(t: Tree) -> None:
    t.cut(K2, "### [W01D02-R3]", "### [W01D02-R4]")


def assessment_drop_q20(t: Tree) -> None:
    t.cut(AS, "20. [W01X-Q20]", "\n## Part 2")


def assessment_swap_kind(old: str, new: str, q: int) -> Callable[[Tree], None]:
    return lambda t: t.sub(AS, f"{q}. [W01X-Q{q:02d}] ({old})", f"{q}. [W01X-Q{q:02d}] ({new})")


def assessment_q1_without_options(t: Tree) -> None:
    t.sub(AS, corpus.exam_options(1, 1), "")


def assessment_too_long(t: Tree) -> None:
    t.set_section(AS, "## Rules", corpus.prose("long", 40) + "\n\n" + ("x" * 99 + " ") * 195)


def akey_drop_distractors_q01(t: Tree) -> None:
    t.del_line(AK, "- Distractors:")


def akey_incomplete_distractors(t: Tree) -> None:
    t.sub(AK, "C is wrong because it skips the record; ", "", after="### [W01X-Q01]")


def akey_drop_q10(t: Tree) -> None:
    t.cut(AK, "### [W01X-Q10]", "### [W01X-Q11]")


def akey_drop_gate(t: Tree) -> None:
    t.sub(AK, "## Safety and integrity gate", "## Something else")


def week_guide_drop_accelerator(t: Tree) -> None:
    t.cut("WEEK_GUIDE.md", "**Accelerator.**", "## Goals")


def lab_independent_with_steps(t: Tree) -> None:
    t.append("LABS/INDEPENDENT_LAB.md", "\n## Steps\n1. A step list that an independent lab must not give.\n")


def fixture_stale_hash(t: Tree) -> None:
    path = t.at("LABS/fixtures/MANIFEST.md")
    text = read_text(path)
    digest = re.search(r"[0-9a-f]{64}", text).group(0)
    write_text(path, text.replace(digest, "0" * 64))


def fixture_binary(t: Tree) -> None:
    path = t.at("LABS/fixtures/blob.bin")
    path.write_bytes(b"\xff\xfe\x00\x01")


def swap_texts(rel: str, first: str, second: str) -> Callable[[Tree], None]:
    """Exchange two headings or labels of a file, so that both are present and their order is wrong."""

    def mutate(t: Tree) -> None:
        t.sub(rel, first, "@@PLACEHOLDER@@")
        t.sub(rel, second, first)
        t.sub(rel, "@@PLACEHOLDER@@", second)

    return mutate


def registry_entry(fn: Callable[[dict], None]) -> Callable[[Tree], None]:
    return lambda t: t.registry(lambda doc: fn(doc["entries"][0]))


def registry_duplicate_id(t: Tree) -> None:
    def fn(doc: dict) -> None:
        doc["entries"][1]["id"] = doc["entries"][0]["id"]

    t.registry(fn)


def registry_drop_key(t: Tree) -> None:
    t.registry(lambda doc: doc["entries"][0].pop("fallback"))


def registry_hidden_escape(t: Tree) -> None:
    def fn(doc: dict) -> None:
        doc["entries"][0]["notes"] = "Hidden​ character written as a JSON escape."

    t.registry(fn)


def registry_fallback_unknown(t: Tree) -> None:
    t.registry(lambda doc: doc["entries"][1].update({"fallback": "w01-no-such-entry"}))


def registry_invalid_json(t: Tree) -> None:
    t.put("RESOURCES.json", '{"week": 1,')


def registry_not_object(t: Tree) -> None:
    t.put("RESOURCES.json", "[]\n")


def registry_no_entries(t: Tree) -> None:
    t.registry(lambda doc: doc.update({"entries": []}))


def leak_answer_text_in_day(t: Tree) -> None:
    t.sub(D3, "\n## Commands proposed for review", "\n\n" + corpus.day_answer(2, "W01D02-R1") + "\n\n## Commands proposed for review")


def leak_exam_answer_in_checklist(t: Tree) -> None:
    t.append("EVIDENCE_CHECKLIST.md", "\n" + corpus.exam_answer(1, 13) + "\n")


def quiz_same_text(t: Tree) -> None:
    t.sub(D2, "[W01D02-R2] " + corpus.recall_text(2, 2), "[W01D02-R2] " + corpus.recall_text(2, 1))


def quiz_duplicate_id(t: Tree) -> None:
    t.sub(D2, "[W01D02-R2]", "[W01D02-R1]")


# (name, mutation, messages the rule names); every case expects exit 1 for `--week 1`
CASES: list[tuple[str, Callable[[Tree], None], list[str]]] = [
    # -- files ---------------------------------------------------------------------------------------------
    ("file_missing_day", lambda t: t.delete(D2), ["W01: missing file 'DAY_02.md'"]),
    ("file_missing_key", lambda t: t.delete("ANSWER_KEYS/DAY_03_KEY.md"), ["W01: missing file 'ANSWER_KEYS/DAY_03_KEY.md'"]),
    ("file_missing_assessment", lambda t: t.delete(AS), ["W01: missing file 'ASSESSMENT.md'"]),
    ("file_missing_registry", lambda t: t.delete("RESOURCES.json"), ["W01: missing file 'RESOURCES.json'"]),
    ("file_missing_lab", lambda t: t.delete("LABS/GUIDED_LAB.md"), ["W01: missing file 'LABS/GUIDED_LAB.md'"]),
    ("file_unexpected", lambda t: t.put("NOTES.md", "Notes.\n"), ["W01/NOTES.md: unexpected file"]),
    # -- day guide: title and metadata ---------------------------------------------------------------------------
    ("title_form", lambda t: t.sub(D2, "# G9 W01 / DAY_02 — ", "# G9 W01 / DAY_2 — "), ["W01/DAY_02.md:1: line 1 must be '# G9 W01 / DAY_02 — <topic>'"]),
    ("metadata_status", lambda t: t.sub(D2, "- Status: PROPOSED", "- Status: DONE"), ["metadata line 1 must be '- Status: PROPOSED'"]),
    ("metadata_date", lambda t: t.sub(D2, "(Tuesday)", "(Wednesday)"), ["metadata line 3 must be '- Original calendar date: 2026-11-17 (Tuesday)'"]),
    ("metadata_duration", lambda t: t.sub(D2, "D 1 h)", "D 2 h)"), ["metadata line 4 must be '- Duration: 8 instructional hours"]),
    ("metadata_line_missing", lambda t: t.del_line(D2, "- Dependencies:"), ["metadata: exactly 7 lines stand between the title and '## Objectives'"]),
    ("metadata_lab_authorization_empty", lambda t: t.set_line(D2, "- Lab authorization:", "- Lab authorization:"), ["metadata line 5 must be '- Lab authorization: <the exact synthetic range"]),
    ("metadata_provider_form", lambda t: t.sub(D2, " | Secondary: none", ""), ["metadata line 7 must be '- Primary provider: <one provider> | Secondary: <one provider or none>'"]),
    # -- day guide: sections ---------------------------------------------------------------------------------------
    ("section_unknown_and_missing", lambda t: t.sub(D2, "## Hand-in", "## Handin"), ["unknown section '## Handin'", "missing section '## Hand-in'"]),
    ("section_twice", lambda t: t.append(D2, "\n## Reflection\n- Again.\n"), ["section '## Reflection' appears twice"]),
    ("sections_out_of_order", swap_headings, ["sections are out of order"]),
    ("block_minutes", lambda t: t.sub(D2, BLOCK_A, "## A — Synthetic block A (90 min)"), ["block A must be '## A — <title> (120 min)'"]),
    ("block_recall_title", lambda t: t.sub(D2, "## R — Active recall (30 min)", "## R — Warm up (30 min)"), ["block R must be '## R — Active recall (30 min)'"]),
    ("block_title_too_long", lambda t: t.sub(D2, BLOCK_A, "## A — " + "T" * 65 + " (120 min)"), ["block A title is 65 characters (maximum 64)"]),
    # -- day guide: objectives and hand-in -----------------------------------------------------------------------
    ("objective_count", lambda t: t.del_line(D2, obj_prefix(2, 3)), ["2 objective lines (exactly 3 are required)"]),
    ("objective_seed_prefix", lambda t: t.set_line(D2, obj_prefix(2, 1)[:-3], f"1. {seed(2, 1)}. The learner shows one dated observation. []"), [f"objective 1 must start with the seed text of column A '{seed(2, 1)} — '"]),
    ("objective_numbering", lambda t: t.set_line(D2, obj_prefix(2, 2), obj_line(2, 2, "").replace("2. ", "3. ", 1)), ["objective line 2 is numbered 3"]),
    ("objective_too_long", lambda t: t.set_line(D2, obj_prefix(2, 1), f"{obj_prefix(2, 1)}{'y ' * 260}[]"), ["objective 1 is ", "characters (maximum 500)"]),
    ("objective_line_format", lambda t: t.set_line(D2, obj_prefix(2, 3), f"{obj_prefix(2, 3)}no brackets at the end"), ["objective line 3 is not '3. <seed text of column C>"]),
    ("objective_unknown_competency", lambda t: t.set_line(D2, obj_prefix(2, 2), obj_line(2, 2, "lab.no-such-competency")), ["objective 2: unknown competency 'lab.no-such-competency'"]),
    ("objective_four_competencies", lambda t: t.set_line(D2, obj_prefix(2, 2), obj_line(2, 2, "lab.authorization, linux.filesystem, linux.processes-services, python.secure-parsing")), ["objective 2: zero to three distinct competencies"]),
    ("objective_duplicate_competency", lambda t: t.set_line(D2, obj_prefix(2, 2), obj_line(2, 2, "lab.authorization, lab.authorization")), ["objective 2: zero to three distinct competencies"]),
    ("hand_in_prefix", lambda t: t.sub(D2, f"{seed(2, 4)} — one file", f"{seed(2, 4)}. One file"), [f"hand-in must start with the seed text of column D '{seed(2, 4)} — '"]),
    ("hand_in_two_paragraphs", lambda t: t.sub(D2, "## Resources", "A second paragraph of the hand-in.\n\n## Resources"), ["'## Hand-in' is one paragraph", "found 2 paragraphs"]),
    ("hand_in_too_long", lambda t: t.sub(D2, f"{seed(2, 4)} — one file", f"{seed(2, 4)} — {'z' * 520} one file"), ["hand-in is ", "characters (maximum 500)"]),
    # -- day guide: resources --------------------------------------------------------------------------------------
    ("resource_unknown_id", lambda t: t.sub(D2, ROW_DOCS, ROW_DOCS.replace("w01-synthetic-docs", "w01-no-such-entry")), ["resources row 1: 'w01-no-such-entry' is not an entry of a RESOURCES.json"]),
    ("resource_block_letter", lambda t: t.sub(D2, ROW_DOCS, ROW_DOCS[:-3] + "Z |"), ["resources row 1: block 'Z' is not one of R, A, B, C, D"]),
    ("resource_empty_cell", lambda t: t.sub(D2, ROW_DOCS, "| w01-synthetic-docs |  | A |"), ["resources row 1: empty cell 'Use in this day'"]),
    ("resource_row_repeated", lambda t: t.insert_after(D2, ROW_DOCS, ROW_DOCS), ["resources row 2 repeats 'w01-synthetic-docs' for block A"]),
    ("resource_no_rows", lambda t: t.cut(D2, ROW_DOCS, "\n## R — "), ["resources: 0 rows (1 to 8 are required)"]),
    ("resource_nine_rows", add_resource_rows, ["resources: 9 rows (1 to 8 are required)"]),
    ("resource_no_hands_on", lambda t: t.sub(D2, ROW_MODULE, ROW_MODULE.replace("| C |", "| A |")), ["resources: no hands-on exercise: at least one row of block B or C names an entry of kind MODULE, ROOM, LAB, WARGAME, MACHINE or CHALLENGE"]),
    ("resource_header", lambda t: t.sub(D2, "| Id | Use in this day | Block |", "| Identifier | Use in this day | Block |"), ["resources: the section holds the table '| Id | Use in this day | Block |'"]),
    # -- day guide: blocks ---------------------------------------------------------------------------------------
    ("block_too_short", lambda t: t.set_section(D2, BLOCK_B, "Too short to count."), ["block B has 4 words (minimum 120)"]),
    ("block_recall_too_short", lambda t: t.set_section(D2, "## R — Active recall (30 min)", "Warm up with three tasks."), ["block R has 5 words (minimum 60)"]),
    ("friday_block_reference", blocks_without_friday_reference, ["block A of a Friday refers to 'ASSESSMENT.md' and 'Part 1'"]),
    ("friday_block_c_reference", lambda t: t.sub(D5, " The scored practical is in ASSESSMENT.md, Parts 2 and 3.", ""), ["block C of a Friday refers to 'ASSESSMENT.md' and 'Part 2'"]),
    ("friday_block_d_reference", lambda t: t.sub(D5, " Scoring and the report are in ASSESSMENT.md, Part 4.", ""), ["block D of a Friday refers to 'ASSESSMENT.md' and 'Part 4'"]),
    # -- day guide: commands and mapping ---------------------------------------------------------------------------
    ("commands_two_rows", drop_commands_row, ["commands: 2 rows (at least 3 are required on a technical day"]),
    ("commands_empty_cell", lambda t: t.sub(D2, "none needed (read-only) |", " |", after=CMD_ROW_1), ["commands row 1: empty cell 'Rollback'"]),
    ("commands_cell_count", lambda t: t.sub(D2, " | RED guest 1 |", " |", after=CMD_ROW_1), ["commands row 1 has 8 cells (9 are required"]),
    ("commands_row_number", lambda t: t.sub(D2, "| 2 | `ls -l /srv/lab2`", "| 5 | `ls -l /srv/lab2`"), ["commands row 2: the '#' cell must be 2; found '5'"]),
    ("commands_header", lambda t: t.sub(D2, "| # | Command | Target |", "| # | Cmd | Target |"), ["commands: the section holds the table '| # | Command | Target |"]),
    ("mapping_no_rows", lambda t: t.cut(D2, "| a permission left open on guest 2", "\n## Quiz"), ["mapping: at least one row is required"]),
    ("mapping_not_applicable_reason", lambda t: t.sub(D2, "| a permission left open on guest 2 |", "| not applicable |"), ["mapping row 1: a day without an offensive mechanism writes 'not applicable: <reason>' in the first cell"]),
    # -- day guide: quiz -----------------------------------------------------------------------------------------
    ("quiz_four_recall", lambda t: t.del_line(D2, "5. [W01D02-R5]"), ["quiz: recall has 4 questions (exactly 5 are required)"]),
    ("quiz_two_applied", lambda t: t.del_line(D2, "3. [W01D02-A3]"), ["quiz: applied has 2 questions (exactly 3 are required)"]),
    ("quiz_identifier", lambda t: t.sub(D2, "[W01D02-R2]", "[W01D02-R9]"), ["quiz: recall question 2 must be '2. [W01D02-R2] <question>'; found '2. [W01D02-R9]'"]),
    ("quiz_hint_in_question", lambda t: t.sub(D2, corpus.applied_text(2, 1), corpus.applied_text(2, 1) + " hint: list the directory"), ["quiz: W01D02-A1: a question holds no answer and no 'hint:'"]),
    ("quiz_two_paragraphs", quiz_two_paragraphs, ["quiz: a question is one paragraph 'K. [id] <question>'"]),
    ("quiz_text_outside", quiz_text_outside, ["quiz: text outside the subsections '### Recall (closed book)' and '### Applied'"]),
    ("quiz_subsection_names", lambda t: t.sub(D2, "### Applied", "### Applied questions"), ["quiz: the subsections are '### Recall (closed book)' then '### Applied'"]),
    ("quiz_text_too_long", lambda t: t.sub(D2, corpus.recall_text(2, 1), corpus.recall_text(2, 1) + " " + "w" * 2100), ["quiz: question W01D02-R1 is "]),
    # -- day guide: hint ladder, troubleshooting, evidence, extension, spaced recall, reflection -------------------------
    ("hint_line_four", lambda t: t.set_line(D2, "4. Instructor explanation", "4. Instructor explanation: the answer is B."), ["hint ladder line 4 must be '4. Instructor explanation: in the answer key, after the attempt is submitted.'"]),
    ("hint_label", lambda t: t.sub(D2, "2. Conceptual hint: ", "2. Hint: "), ["hint ladder line 2 must be '2. Conceptual hint: <text>'"]),
    ("hint_three_lines", lambda t: t.del_line(D2, "3. Scoped technical hint:"), ["hint ladder: four numbered lines are required"]),
    ("troubleshooting_order", swap_texts(D2, "- Restoration: ", "- Time exceeded: "), ["troubleshooting: this line must be '- Restoration: <text>'"]),
    ("troubleshooting_six_lines", lambda t: t.del_line(D2, "- Missing dependency:"), ["troubleshooting: the seven lines"]),
    ("evidence_label_missing", lambda t: t.del_line(D2, "- Negative test:"), ["evidence: no '- Negative test: ' line (at least one line of each of the three labels)"]),
    ("expert_extension_empty", lambda t: t.set_section(D2, "## Expert extension (optional)", ""), ["'## Expert extension (optional)' is empty"]),
    ("spaced_identifier_twice", lambda t: t.set_line(D2, "- 14 days:", "- 14 days: W01D02-R4"), ["spaced recall: W01D02-R4 appears 2 times", "spaced recall: W01D02-R5 appears 0 times"]),
    ("spaced_first_line_one", lambda t: t.set_line(D2, "- 1 day:", "- 1 day: W01D02-R1"), ["spaced recall: '- 1 day: ' names 2 recall identifiers; found 1"]),
    ("spaced_foreign_identifier", lambda t: t.set_line(D2, "- 7 days:", "- 7 days: W01D03-R4"), ["spaced recall: 'W01D03-R4' is not a recall identifier of the day"]),
    ("spaced_line_missing", lambda t: t.del_line(D2, "- 7 days:"), ["spaced recall: the four lines"]),
    ("reflection_lines", lambda t: t.sub(D2, "- What was not proven?", "- What was proven?"), ["reflection: the section holds the three lines"]),
    # -- day guide: file-level -----------------------------------------------------------------------------------
    ("words_too_few", words_too_few, ["words (1,400 to 3,200 are required in the whole file)"]),
    ("words_too_many", lambda t: t.set_section(D2, BLOCK_D, corpus.prose("big", 2200)), ["words (1,400 to 3,200 are required in the whole file)"]),
    ("hidden_zero_width_space", lambda t: t.sub(D2, "Without notes", "Without​ notes"), ["W01/DAY_02.md:", "hidden or control character U+200B"]),
    ("hidden_tag_character", lambda t: t.sub(D2, "Without notes", "Without\U000e0041 notes"), ["hidden or control character U+E0041"]),
    ("hidden_control_character", lambda t: t.sub(D2, "Without notes", "Without\x07 notes"), ["hidden or control character U+0007"]),
    ("markdown_link", lambda t: t.sub(D2, "Without notes", "See [the page](https://example.com/x) without notes"), ["Markdown link or image '[the page](https://example.com/x)'"]),
    ("markdown_image", lambda t: t.sub(D2, "Without notes", "![diagram](https://example.com/x.png) Without notes"), ["Markdown link or image '![diagram](https://example.com/x.png)'"]),
    ("html_tag", lambda t: t.sub(D2, "Without notes", "<b>Without</b> notes"), ["HTML or an angle-bracket placeholder outside inline code '<b>'"]),
    ("placeholder_outside_code", lambda t: t.sub(D2, "Without notes", "Use <synthetic-password> and without notes"), ["HTML or an angle-bracket placeholder outside inline code '<synthetic-password>'"]),
    ("fence_not_closed", lambda t: t.sub(D2, "## Reflection", "## Reflection\n```\n"), ["a code fence is opened and never closed"]),
    ("second_title", lambda t: t.sub(D2, "## Reflection", "# A second title\n\n## Reflection"), ["a second '# ' heading: a file has one title, on line 1"]),
    # -- day key -------------------------------------------------------------------------------------------------
    ("key_title", lambda t: t.sub(K2, "# Instructor key — G9 W01 / DAY_02", "# Instructor key — G9 W01 / DAY_03"), ["line 1 must be '# Instructor key — G9 W01 / DAY_02'"]),
    ("key_notice_missing", lambda t: t.del_line(K2, "Instructor only."), ["the line 'Instructor only. Not part of the learner's view.' stands alone between the title and '## Quiz key'"]),
    ("key_entry_missing", key_drop_entry, ["quiz key: the eight '### [id]' headings are W01D02-R1 to W01D02-R5 and W01D02-A1 to W01D02-A3, in this order"]),
    ("key_marking_missing", lambda t: t.del_line(K2, "- Marking:"), ["quiz key [W01D02-R1]: the three lines '- Answer: ', '- Marking: ' and '- Source: ' are required"]),
    ("key_source_unknown", lambda t: t.sub(K2, "- Source: w01-synthetic-docs", "- Source: w01-no-such-entry"), ["source names 'w01-no-such-entry', which is not an entry of a RESOURCES.json"]),
    ("key_section_missing", lambda t: t.sub(K2, "## Overclaims to reject", "## Claims"), ["missing section '## Overclaims to reject'"]),
    ("key_section_empty", lambda t: t.set_section(K2, "## Hint 4: instructor explanation", ""), ["'## Hint 4: instructor explanation' is empty"]),
    # -- assessment ------------------------------------------------------------------------------------------------
    ("assessment_code", lambda t: t.sub(AS, "- Code: V2-W01-EXAM", "- Code: V2-W01-QUIZ"), ["metadata line 1 must be '- Code: V2-W01-EXAM'"]),
    ("assessment_kind", lambda t: t.sub(AS, "- Kind: BLIND_PRACTICAL", "- Kind: CAPSTONE"), ["metadata line 2 must be '- Kind: BLIND_PRACTICAL'"]),
    ("assessment_competency_unknown", lambda t: t.set_line(AS, "- Competency:", "- Competency: lab.no-such"), ["unknown competency 'lab.no-such'"]),
    ("assessment_difficulty_not_integer", lambda t: t.set_line(AS, "- Difficulty:", "- Difficulty: three"), ["difficulty 'three' must be an integer 1..5"]),
    ("assessment_difficulty_outside_band", lambda t: t.set_line(AS, "- Difficulty:", "- Difficulty: 5"), ["difficulty 5 is outside the difficulty band 1..2 of 'net.layers-addressing'"]),
    ("assessment_title_too_long", lambda t: t.set_line(AS, "- Title:", "- Title: " + "t" * 201), ["title is 201 characters (maximum 200)"]),
    ("assessment_metadata_line_missing", lambda t: t.del_line(AS, "- Title:"), ["metadata: exactly five lines stand between the title and '## Rules'"]),
    ("assessment_title_line", lambda t: t.sub(AS, "# G9 W01 — Friday assessment (day 05)", "# G9 W01 — Friday assessment (day 5)"), ["line 1 must be '# G9 W01 — Friday assessment (day 05)'"]),
    ("assessment_section_missing", lambda t: t.sub(AS, "## Submission", "## Hand over"), ["missing section '## Submission'"]),
    ("assessment_section_order", swap_texts(AS, "## Remediation track", "## Submission"), ["sections are out of order"]),
    ("assessment_section_thin", lambda t: t.set_section(AS, "## Rules", "Short rules."), ["section '## Rules' has 2 words (minimum 20)"]),
    ("assessment_19_questions", assessment_drop_q20, ["Part 1: 19 questions (exactly 20 are required, W01X-Q01 to W01X-Q20)"]),
    ("assessment_question_identifier", lambda t: t.sub(AS, "5. [W01X-Q05]", "5. [W01X-Q15]"), ["Part 1: question 5 must be '5. [W01X-Q05] ...'; found '5. [W01X-Q15]'"]),
    ("assessment_question_kind", lambda t: t.sub(AS, "5. [W01X-Q05] (CONCEPT)", "5. [W01X-Q05] (EASY)"), ["Part 1: question 5 must be '5. [W01X-Q05] (CONCEPT|INTERPRET|CONTROL|SYNTHESIS) <question>'"]),
    ("assessment_mix_interpret", assessment_swap_kind("INTERPRET", "CONCEPT", 9), ["question mix: 5 INTERPRET questions (at least 6 are required"]),
    ("assessment_mix_control", assessment_swap_kind("CONTROL", "SYNTHESIS", 13), ["question mix: 3 CONTROL questions (at least 4 are required"]),
    ("assessment_mcq_count", assessment_q1_without_options, ["Part 1: 7 multiple-choice questions (at least 8 are required)"]),
    ("assessment_mcq_options", lambda t: t.sub(AS, "   D. Option delta for question 2 of week 1", "   E. Option delta for question 2 of week 1"), ["Part 1: W01X-Q02: a multiple-choice question lists the options 'A.' to 'D.' in order"]),
    ("assessment_interpret_sample", lambda t: t.sub(AS, "9. [W01X-Q09] (INTERPRET) Sample (synthetic):", "9. [W01X-Q09] (INTERPRET) A line"), ["Part 1: W01X-Q09: an INTERPRET question gives what is to be interpreted in the question, marked 'Sample (synthetic):'"]),
    ("assessment_hint_in_question", lambda t: t.sub(AS, "13. [W01X-Q13] (CONTROL) ", "13. [W01X-Q13] (CONTROL) hint: think. "), ["Part 1: W01X-Q13: a question holds no answer and no 'hint:'"]),
    ("assessment_task_statement_too_long", assessment_too_long, ["task statement ('## Rules' to the end of Part 4) is ", "characters (maximum 19000)"]),
    ("assessment_link", lambda t: t.sub(AS, "## Remediation track\n", "## Remediation track\nSee [this](https://example.com).\n"), ["W01/ASSESSMENT.md:", "Markdown link or image"]),
    # -- assessment key ----------------------------------------------------------------------------------------------
    ("akey_distractors_missing", akey_drop_distractors_q01, ["[W01X-Q01]: the key of a multiple-choice question needs '- Distractors: '"]),
    ("akey_distractors_incomplete", akey_incomplete_distractors, ["[W01X-Q01]: '- Distractors: ' says why each other option is wrong; not named: C"]),
    ("akey_answer_letter", lambda t: t.sub(AK, "- Answer: B. The statement bravo for question 1", "- Answer: The statement bravo for question 1"), ["[W01X-Q01]: the answer of a multiple-choice question starts with its option letter (A to D)"]),
    ("akey_why_missing", lambda t: t.del_line(AK, "- Why:"), ["[W01X-Q01]: '- Answer: ' and '- Why: ' are required, in this order"]),
    ("akey_entry_missing", akey_drop_q10, ["the twenty '### [id]' headings are W01X-Q01 to W01X-Q20, in this order"]),
    ("akey_practical_missing", lambda t: t.sub(AK, "## Practical: fixture map and scoring", "## Practical"), ["missing section '## Practical: fixture map and scoring'"]),
    ("akey_practical_numbers", lambda t: t.sub(AK, "evidence and reporting 15 points", "evidence and reporting"), ["gives the four score parts (20, 40, 25 and 15 points) and their bands; not named: 15"]),
    ("akey_gate_missing", akey_drop_gate, ["missing section '## Safety and integrity gate'"]),
    ("akey_sections_order", swap_texts(AK, "## Practical: fixture map and scoring", "## Safety and integrity gate"), ["sections are out of order: '## Safety and integrity gate' stands before '## Practical: fixture map and scoring'"]),
    # -- week guide, labs, fixtures, support files --------------------------------------------------------------------
    ("week_guide_paragraph_missing", week_guide_drop_accelerator, ["W01/WEEK_GUIDE.md: missing the '**Accelerator.**' paragraph before '## Goals'"]),
    ("week_guide_title", lambda t: t.sub("WEEK_GUIDE.md", f"# G9 W01 — {PLAN['titles'][1]}", "# G9 W01 — Another title"), [f"line 1 must be '# G9 W01 — {PLAN['titles'][1]}' (the week title of the plan)"]),
    ("week_guide_section_missing", lambda t: t.sub("WEEK_GUIDE.md", "## Tools", "## Instruments"), ["W01/WEEK_GUIDE.md: missing section '## Tools'"]),
    ("week_guide_section_empty", lambda t: t.set_section("WEEK_GUIDE.md", "## Goals", ""), ["section '## Goals' is empty"]),
    ("week_guide_paragraph_order", swap_texts("WEEK_GUIDE.md", "**Purpose.**", "**Lab stack.**"), ["the paragraphs stand in the order Purpose, Lab stack, Competencies, Accelerator; found ['Lab stack', 'Purpose', 'Competencies', 'Accelerator']"]),
    ("week_guide_paragraph_twice", lambda t: t.sub("WEEK_GUIDE.md", "**Accelerator.**", "**Lab stack.** Moved.\n\n**Accelerator.**"), ["the '**Lab stack.**' paragraph appears twice"]),
    ("lab_section_missing", lambda t: t.sub("LABS/GUIDED_LAB.md", "## Evidence", "## Proof"), ["W01/LABS/GUIDED_LAB.md: missing section '## Evidence'"]),
    ("lab_steps_missing", lambda t: t.sub("LABS/GUIDED_LAB.md", "## Steps", "## Procedure"), ["W01/LABS/GUIDED_LAB.md: missing section '## Steps'"]),
    ("lab_steps_in_independent", lab_independent_with_steps, ["'## Steps' belongs to the guided lab only"]),
    ("lab_section_empty", lambda t: t.set_section("LABS/FRIDAY_PRACTICAL.md", "## Rollback and cleanup", ""), ["section '## Rollback and cleanup' is empty"]),
    ("labs_key_without_lab_heading", lambda t: t.sub("ANSWER_KEYS/LABS_KEY.md", "## FRIDAY_PRACTICAL: ", "## Practical: "), ["no heading names the lab 'FRIDAY_PRACTICAL.md'"]),
    ("fixture_stale_hash", fixture_stale_hash, ["is not the SHA-256 of a file under LABS/fixtures/"]),
    ("fixture_unlisted", lambda t: t.put("LABS/fixtures/extra.log", "Sample (synthetic): one more line\n"), ["does not list the fixture 'extra.log'"]),
    ("fixture_not_text", fixture_binary, ["W01/LABS/fixtures/blob.bin: not UTF-8 text"]),
    ("evidence_checklist_thin", lambda t: t.put("EVIDENCE_CHECKLIST.md", "# Checklist\n\nOne line.\n"), ["words (minimum 40): expected artifacts, allowed redactions, pass or fail"]),
    # -- registry ---------------------------------------------------------------------------------------------------
    ("registry_invalid_json", registry_invalid_json, ["W01/RESOURCES.json: not valid JSON"]),
    ("registry_not_object", registry_not_object, ["the document must be an object"]),
    ("registry_no_entries", registry_no_entries, ["'entries' must be a non-empty list"]),
    ("registry_week", lambda t: t.registry(lambda d: d.update({"week": 2})), ["'week' is 2; the file is the registry of week 1"]),
    ("registry_checked_on", lambda t: t.registry(lambda d: d.update({"checked_on": "2026-13-40"})), ["'checked_on' must be a date YYYY-MM-DD"]),
    ("registry_unknown_key", lambda t: t.registry(lambda d: d.update({"extra": 1})), ["unknown key 'extra'"]),
    ("registry_entry_key_missing", registry_drop_key, ["missing key 'fallback'"]),
    ("registry_id_prefix", registry_entry(lambda e: e.update({"id": "w02-synthetic-docs"})), ["'id' must start with 'w01-' (the week of the file)"]),
    ("registry_id_form", registry_entry(lambda e: e.update({"id": "W01_Docs"})), ["'id' must be 'wNN-' and lower-case words joined by hyphens"]),
    ("registry_id_twice", registry_duplicate_id, ["id 'w01-synthetic-docs' appears twice"]),
    ("registry_url_not_https", registry_entry(lambda e: e.update({"url": "http://example.com/x"})), ["'url' must be an https address without spaces"]),
    ("registry_kind", registry_entry(lambda e: e.update({"kind": "VIDEO"})), ["'kind' is 'VIDEO'; it must be one of"]),
    ("registry_access", registry_entry(lambda e: e.update({"access": "FREEISH"})), ["'access' is 'FREEISH'; it must be one of"]),
    ("registry_verification", registry_entry(lambda e: e.update({"verification": "VERIFIED"})), ["'verification' is 'VERIFIED'; it must be one of"]),
    ("registry_suggested_days", registry_entry(lambda e: e.update({"suggested_days": ["D61"]})), ["'suggested_days' must be a list of day ids D01 to D60"]),
    ("registry_estimate", registry_entry(lambda e: e.update({"estimate_minutes": 0})), ["'estimate_minutes' must be null or a whole number of minutes from 1 to 2400"]),
    ("registry_hidden_escape", registry_hidden_escape, ["hidden or control character U+200B in a text"]),
    ("registry_fact_check_result", lambda t: t.registry(lambda d: d["plan_fact_checks"][0].update({"result": "MAYBE"})), ["'result' is 'MAYBE'; it must be one of"]),
    ("registry_gaps", lambda t: t.registry(lambda d: d.update({"gaps": [""]})), ["'gaps' must be a list of non-empty texts"]),
    ("registry_fallback_unknown_id", registry_fallback_unknown, ["'fallback' names 'w01-no-such-entry', which is not an entry of a RESOURCES.json"]),
    # -- leak rules ---------------------------------------------------------------------------------------------------
    ("leak_answer_line", lambda t: t.append("WEEK_GUIDE.md", "\nAnswer: B\n"), ["W01/WEEK_GUIDE.md:", "begins with 'Answer:', '- Answer:' or 'Correct answer'"]),
    ("leak_dash_answer_line", lambda t: t.append("LABS/GUIDED_LAB.md", "\n- Answer: the second guest\n"), ["W01/LABS/GUIDED_LAB.md:", "begins with 'Answer:', '- Answer:' or 'Correct answer'"]),
    ("leak_correct_answer_line", lambda t: t.append("EVIDENCE_CHECKLIST.md", "\nCorrect answer is C.\n"), ["W01/EVIDENCE_CHECKLIST.md:", "begins with 'Answer:', '- Answer:' or 'Correct answer'"]),
    ("leak_emphasised_answer_line", lambda t: t.append("EVIDENCE_CHECKLIST.md", "\n**Answer:** C\n"), ["begins with 'Answer:', '- Answer:' or 'Correct answer'"]),
    ("leak_day_answer_text", leak_answer_text_in_day, ["W01/DAY_03.md: holds the answer text of W01D02-R1 (W01/ANSWER_KEYS/DAY_02_KEY.md)"]),
    ("leak_exam_answer_text", leak_exam_answer_in_checklist, ["W01/EVIDENCE_CHECKLIST.md: holds the answer text of W01X-Q13 (W01/ANSWER_KEYS/ASSESSMENT_KEY.md)"]),
    # -- corpus rules -------------------------------------------------------------------------------------------------
    ("identifier_used_twice", quiz_duplicate_id, ["'W01D02-R1' is used twice"]),
    ("question_text_twice", quiz_same_text, ["W01D02-R1 and W01D02-R2 have the same text: no two questions of the corpus are the same"]),
]


class OneWeek(TreeCase):
    """`--week 1` on a synthetic week 1: the control passes; every case of CASES breaks one rule."""

    template = "one"

    def test_control_passes(self) -> None:
        proc = self.build("--week", "1")
        self.assertExit(proc, 0)
        out = output_of(proc)
        self.assertRegex(out, r"ok    v2 courseware valid: 1 week\(s\) \[1\]; 5 days; 40 questions; 20 exam questions; 3 resources \(2 cited\); \d+ words in the day guides")
        self.assertIn("partial run (weeks [1]): the generated files were not compared and nothing was written", out)

    def test_control_writes_nothing(self) -> None:
        before = _tree_digest(self.root)
        self.assertExit(self.build("--week", "1"), 0)
        self.assertEqual(before, _tree_digest(self.root))

    def test_week_mode_does_not_need_the_other_weeks(self) -> None:
        self.assertFalse((self.root / WEEKS_REL / "W02").exists())
        self.assertExit(self.build("--week", "1"), 0)

    def test_week_that_is_not_written_fails(self) -> None:
        proc = self.build("--week", "3")
        self.assertRule(proc, "W03: the week is not written: no day guide (DAY_11.md to DAY_15.md)")

    def test_default_mode_needs_all_twelve_weeks(self) -> None:
        proc = self.build()
        self.assertRule(proc, "W02: the week is not written")
        self.assertIn("W12: the week is not written", output_of(proc))

    def test_week_flag_is_repeatable(self) -> None:
        corpus.write_week(self.root, PLAN, 2)
        proc = self.build("--week", "1", "--week", "2")
        self.assertExit(proc, 0)
        self.assertIn("2 week(s) [1, 2]; 10 days; 80 questions; 40 exam questions", output_of(proc))

    def test_available_checks_only_weeks_with_a_day_guide(self) -> None:
        corpus.write_week(self.root, PLAN, 2)
        (self.root / WEEKS_REL / "W03").mkdir()
        write_text(self.root / WEEKS_REL / "W03" / "RESOURCES.json", "not json at all")
        proc = self.build("--available")
        self.assertExit(proc, 0)
        self.assertIn("2 week(s) [1, 2]", output_of(proc))
        self.assertIn("partial run (weeks [1, 2])", output_of(proc))

    def test_available_still_rejects_a_broken_week(self) -> None:
        self.tree.sub(D2, "- Status: PROPOSED", "- Status: DONE")
        self.assertRule(self.build("--available"), "metadata line 1 must be '- Status: PROPOSED'")

    def test_available_with_no_week_passes(self) -> None:
        shutil.rmtree(self.root / WEEKS_REL / "W01")
        proc = self.build("--available")
        self.assertExit(proc, 0)
        self.assertIn("0 week(s) []", output_of(proc))

    def test_available_write_writes_nothing_without_all_weeks(self) -> None:
        before = _tree_digest(self.root)
        proc = self.build("--available", "--write")
        self.assertExit(proc, 0)
        self.assertEqual(before, _tree_digest(self.root))

    def test_write_without_all_weeks_fails_and_writes_nothing(self) -> None:
        before = _tree_digest(self.root)
        proc = self.build("--write")
        self.assertRule(proc, "W02: the week is not written")
        self.assertEqual(before, _tree_digest(self.root))
        self.assertFalse((self.root / DATA).exists())

    def test_day_in_another_weeks_registry_may_be_cited(self) -> None:
        corpus.put(self.root / WEEKS_REL / "W02" / "RESOURCES.json", json_text(corpus.registry(2)))
        self.tree.sub(D2, ROW_DOCS, ROW_DOCS.replace("w01-synthetic-docs", "w02-synthetic-docs"))
        self.assertExit(self.build("--week", "1"), 0)

    def test_day_in_another_weeks_registry_fails_when_that_registry_is_absent(self) -> None:
        self.tree.sub(D2, ROW_DOCS, ROW_DOCS.replace("w01-synthetic-docs", "w02-synthetic-docs"))
        self.assertRule(self.build("--week", "1"), "resources row 1: 'w02-synthetic-docs' is not an entry of a RESOURCES.json")

    def test_local_lab_only_day_needs_no_hands_on_exercise(self) -> None:
        local_lab_only(self.tree)
        self.assertExit(self.build("--week", "1"), 0)

    def test_not_applicable_with_a_reason_is_a_mapping_row(self) -> None:
        self.tree.sub(D2, "| a permission left open on guest 2 |", "| not applicable: a reading day with no offensive mechanism |")
        self.assertExit(self.build("--week", "1"), 0)

    def test_friday_may_have_no_command(self) -> None:
        self.assertNotIn("| 1 |", self.read(f"{WEEKS_REL}/W01/{D5}").split("## Commands proposed for review")[1].split("## Offense")[0])
        self.assertExit(self.build("--week", "1"), 0)

    def test_answer_of_a_multiple_choice_question_in_a_learner_file_is_not_a_leak(self) -> None:
        self.tree.append("EVIDENCE_CHECKLIST.md", "\n" + corpus.exam_answer(1, 1) + "\n")
        self.assertExit(self.build("--week", "1"), 0)

    def test_short_answer_text_is_not_a_leak(self) -> None:
        # the rule compares answers of 40 normalized characters or more
        self.tree.sub(K2, "- Answer: " + corpus.day_answer(2, "W01D02-R1"), "- Answer: a dated listing")
        self.tree.append("EVIDENCE_CHECKLIST.md", "\na dated listing\n")
        self.assertExit(self.build("--week", "1"), 0)

    def test_registry_of_a_week_that_is_not_selected_is_not_judged(self) -> None:
        write_text(self.root / WEEKS_REL / "W07" / "RESOURCES.json", "{broken")
        self.assertExit(self.build("--week", "1"), 0)

    def test_an_unreadable_environment_is_exit_2(self) -> None:
        (self.root / PLAN_REL).unlink()
        self.assertRule(self.build("--week", "1"), "error: master plan (docs/curriculum/v2/MASTER_PLAN_V2.0.md) or competency packs (data/competency-packs) not found", code=2)

    def test_missing_packs_are_exit_2(self) -> None:
        shutil.rmtree(self.root / PACKS_REL)
        self.assertRule(self.build("--week", "1"), "competency packs (data/competency-packs) not found", code=2)

    def test_usage_errors_are_exit_2(self) -> None:
        for args in (("--week", "13"), ("--week", "0"), ("--week", "x"), ("--seed-text", "D61"), ("--seed-text", "d01"), ("--seed-text", "D00"), ("--week", "1", "--write"), ("--week", "1", "--available"), ("--seed-text", "D01", "--week", "1"), ("--seed-text", "D01", "--write"), ("--seed-text", "D01", "--print-seed"), ("--no-such-flag",)):
            with self.subTest(args=args):
                self.assertExit(self.build(*args), 2)

    def test_seed_text_prints_the_four_texts(self) -> None:
        proc = self.build("--seed-text", "D11")
        self.assertExit(proc, 0)
        out = output_of(proc)
        self.assertIn("D11  week 3  2026-11-30 (Monday)", out)
        for k, label in enumerate(("Objective 1 (column A)", "Objective 2 (column B)", "Objective 3 (column C)", "Hand-in (column D)"), start=1):
            self.assertIn(f"{label}: {seed(11, k)}", out)

    def test_seed_text_works_without_any_week(self) -> None:
        shutil.rmtree(self.root / WEEKS_REL)
        proc = self.build("--seed-text", "D60")
        self.assertExit(proc, 0)
        self.assertIn("D60  week 12  2027-02-05 (Friday)", output_of(proc))


class SeedParser(TreeCase):
    """The day tables of section 5 of the plan are the seed: damaged tables fail before anything else is checked."""

    template = "one"

    def plan_case(self, mutate: Callable[[str], str], needles: list[str]) -> None:
        self.tree.plan(mutate)
        proc = self.build("--week", "1")
        self.assertExit(proc, 1)
        out = output_of(proc)
        for needle in needles:
            self.assertIn(needle, out)
        self.assertIn("in the day tables of the plan; nothing else was checked", out)

    def test_day_row_missing(self) -> None:
        def drop(text: str) -> str:
            return "\n".join(line for line in text.split("\n") if not line.startswith("| **07 Tue 24**"))

        self.plan_case(drop, ["day row 08 is out of order (day 07 is expected)", "week 02 has 4 day rows (exactly 5 are required)", "59 day rows in section 5 (exactly 60 are required)"])

    def test_empty_cell(self) -> None:
        def blank(text: str) -> str:
            return re.sub(r"^(\| \*\*03 Wed 18\*\* \|[^|]*\|)[^|]*(\|.*)$", r"\1 \2", text, flags=re.M)

        self.plan_case(blank, ["a day row of section 5 is '| **NN Www DD** | A | B | C | D |' with four non-empty cells"])

    def test_fifth_cell(self) -> None:
        self.plan_case(lambda text: re.sub(r"^(\| \*\*04 Thu 19\*\* \|.*)\|$", r"\1| a fifth cell |", text, flags=re.M), ["with four non-empty cells"])

    def test_weekday_name(self) -> None:
        self.plan_case(lambda text: text.replace("| **02 Tue 17** |", "| **02 Wed 17** |"), ["day 02 is 'Wed 17' in the plan; the calendar gives 'Tue 17' (2026-11-17)"])

    def test_date_number(self) -> None:
        self.plan_case(lambda text: text.replace("| **02 Tue 17** |", "| **02 Tue 18** |"), ["day 02 is 'Tue 18' in the plan; the calendar gives 'Tue 17' (2026-11-17)"])

    def test_week_heading_missing(self) -> None:
        def drop(text: str) -> str:
            return "\n".join(line for line in text.split("\n") if not line.startswith("## WEEK 12 —"))

        self.plan_case(drop, ["11 week headings in section 5 (exactly 12 are required)", "day 56 is not a day of week 11 (days 51 to 55)"])

    def test_week_heading_malformed(self) -> None:
        self.plan_case(lambda text: re.sub(r"^## WEEK 05 — ", "## WEEK 5 - ", text, flags=re.M), ["a week heading of section 5 is '## WEEK NN — <title>'"])

    def test_week_heading_out_of_order(self) -> None:
        self.plan_case(lambda text: text.replace("## WEEK 03 — ", "## WEEK 04 — "), ["week heading 04 is out of order (week 03 is expected)"])

    def test_section_five_heading_missing(self) -> None:
        self.plan_case(lambda text: text.replace("# 5. COMPLETE 60-DAY", "# 5 COMPLETE 60-DAY"), ["no '# 5. ' heading: the day tables of section 5 were not found"])

    def test_day_row_twice(self) -> None:
        def twice(text: str) -> str:
            row = next(line for line in text.split("\n") if line.startswith("| **03 Wed 18**"))
            return text.replace(row, row + "\n" + row)

        self.plan_case(twice, ["day row 03 is out of order (day 04 is expected)"])

    def test_seed_text_too_long(self) -> None:
        self.plan_case(lambda text: text.replace("| **05 Fri 20** | Closed-book Linux knowledge test", "| **05 Fri 20** | " + "x" * 501 + " Closed-book Linux knowledge test"), ["seed text of column A is 5", "characters (maximum 500)"])

    def test_row_outside_section_five_is_not_a_seed_row(self) -> None:
        # a table row of another section that looks like a day row is not read
        proc = self.build("--print-seed")
        self.assertExit(proc, 0)
        before = proc.stdout
        self.tree.plan(lambda text: re.sub(r"^(# 6\. .*)$", r"\1\n\n| **03 Wed 18** | a | b | c | d |", text, count=1, flags=re.M))
        proc = self.build("--print-seed")
        self.assertExit(proc, 0)
        self.assertEqual(before, proc.stdout)

    def test_emphasis_is_removed_and_nothing_else_changes(self) -> None:
        self.tree.plan(
            lambda text: text.replace(
                "| **01 Mon 16** | Entry diagnostic; Linux privilege model; UEFI, kernel/process concepts |",
                "| **01 Mon 16** | **Entry** diagnostic;   Linux *privilege*  model;\tUEFI, kernel/process concepts |",
            )
        )
        proc = self.build("--print-seed")
        self.assertExit(proc, 0)
        doc = json.loads(proc.stdout)
        self.assertEqual(doc["weeks"][0]["days"][0]["objectives"][0]["text"], "Entry diagnostic; Linux privilege model; UEFI, kernel/process concepts")

    def test_print_seed_equals_the_independent_parse(self) -> None:
        proc = self.build("--print-seed")
        self.assertExit(proc, 0)
        doc = json.loads(proc.stdout)
        self.assertEqual(doc["manifest_schema_version"], "1.1.0")
        self.assertEqual(doc["version"], "2.0.0-seed")
        self.assertEqual(doc["provenance"], "PROPOSED_SEED")
        self.assertEqual([w["title"] for w in doc["weeks"]], [PLAN["titles"][w] for w in range(1, 13)])
        days = [d for w in doc["weeks"] for d in w["days"]]
        self.assertEqual([d["day_id"] for d in days], [f"D{n:02d}" for n in range(1, 61)])
        for n, day in enumerate(days, start=1):
            self.assertEqual([o["text"] for o in day["objectives"]], SEEDS[n][:3], f"D{n:02d}")
            self.assertEqual(day["evidence_artifact"], SEEDS[n][3], f"D{n:02d}")
            self.assertNotIn("resources", day)
            self.assertTrue(all("competency_ids" not in o for o in day["objectives"]))

    def test_print_seed_shape(self) -> None:
        doc = json.loads(self.build("--print-seed").stdout)
        self.assertEqual(
            doc["structure"],
            {"weeks": 12, "days_per_week": 5, "objectives_per_day": 3, "weekday_minutes": 480, "saturday_minutes": 480, "weekly_minutes": 2400, "total_minutes": 28800, "sunday": "unscheduled", "assessment_day_of_week": 5},
        )
        self.assertEqual(doc["weekday_blocks"], [{"block": b, "minutes": m} for b, m in (("recall", 30), ("theory", 120), ("guided_lab", 120), ("independent_lab", 150), ("defense_verification", 60))])
        self.assertEqual(doc["saturday_blocks"], [{"block": b, "minutes": m} for b, m in (("recall", 30), ("theory", 120), ("performance_review", 120), ("blind_practical", 150), ("report", 60))])
        for week in doc["weeks"]:
            for day in week["days"]:
                n = int(day["day_id"][1:])
                self.assertEqual(day["day_of_week"], (n - 1) % 5 + 1)
                self.assertEqual(day["planned_minutes"], 480)
                self.assertEqual(sum(b["minutes"] for b in day["blocks"]), 480)
                self.assertEqual([b["seq"] for b in day["blocks"]], [1, 2, 3, 4, 5])
                self.assertEqual(day["is_assessment_day"], n % 5 == 0)
                self.assertEqual(day["blocks"][2]["block"], "performance_review" if n % 5 == 0 else "guided_lab")
                self.assertEqual([o["objective_id"] for o in day["objectives"]], [f"{day['day_id']}-O{k}" for k in (1, 2, 3)])


def _make_case(mutate: Callable[[Tree], None], needles: list[str]) -> Callable[[OneWeek], None]:
    def test(self: OneWeek) -> None:
        self.run_case(mutate, needles)

    return test


for _name, _mutate, _needles in CASES:
    _test = _make_case(_mutate, _needles)
    _test.__name__ = f"test_rule_{_name}"
    _test.__doc__ = f"expects {_needles[0]!r}"
    setattr(OneWeek, _test.__name__, _test)


# ---------------------------------------------------------------------------------------------------
# a complete 12-week corpus: the generated files
# ---------------------------------------------------------------------------------------------------


def json_text(doc: object) -> str:
    return json.dumps(doc, indent=2) + "\n"


def _tree_digest(root: Path) -> dict[str, tuple[int, int]]:
    out = {}
    for dirpath, _dirs, files in os.walk(root):
        for f in files:
            p = Path(dirpath) / f
            st = p.stat()
            out[str(p.relative_to(root))] = (st.st_size, st.st_mtime_ns)
    return out


def _generated(root: Path) -> dict[str, bytes]:
    """Every generated file of a tree: relative path -> bytes."""
    rels = [SEED_JSON, DRAFT_JSON, SHEETS_JSON, KEYS_JSON, SCHEDULE, MAP, REGISTRY_MD, BANK, ANSWERS, RECALL]
    out = {rel: (root / rel).read_bytes() for rel in rels}
    out.update({p.relative_to(root).as_posix(): p.read_bytes() for p in sorted((root / ASSESSMENTS).glob("*.json"))})
    out.update({p.relative_to(root).as_posix(): p.read_bytes() for p in sorted((root / WEEKS_REL).glob("W*/QUIZZES/*.md"))})
    return out


def edit_source(rel: str, old: str, new: str) -> Callable[[Tree], None]:
    return lambda t: replace_once(t.repo(rel), old, new)


def append_to(rel: str, extra: str = "\n") -> Callable[[Tree], None]:
    return lambda t: write_text(t.repo(rel), read_text(t.repo(rel)) + extra)


def delete_file(rel: str) -> Callable[[Tree], None]:
    return lambda t: t.repo(rel).unlink()


def stale(rel: str) -> str:
    return f"FAIL  {rel} is out of date; run: python3 -I scripts/build_curriculum_v2.py --write"


W3 = f"{WEEKS_REL}/W03"
W7 = f"{WEEKS_REL}/W07"
QUIZ_D1 = f"{WEEKS_REL}/W01/QUIZZES/DAY_01_QUIZ.md"

# (name, change to the corpus after --write, messages)
STALE_CASES: list[tuple[str, Callable[[Tree], None], list[str]]] = [
    ("seed_edited_by_hand", append_to(SEED_JSON), [stale(SEED_JSON)]),
    ("draft_edited_by_hand", append_to(DRAFT_JSON), [stale(DRAFT_JSON)]),
    ("sheets_edited_by_hand", append_to(SHEETS_JSON), [stale(SHEETS_JSON)]),
    ("keys_edited_by_hand", append_to(KEYS_JSON), [stale(KEYS_JSON)]),
    ("assessment_payload_edited", append_to(f"{ASSESSMENTS}/V2-W05-EXAM.json"), [stale(f"{ASSESSMENTS}/V2-W05-EXAM.json")]),
    ("capstone_payload_edited", append_to(f"{ASSESSMENTS}/V2-W12-CAPSTONE.json"), [stale(f"{ASSESSMENTS}/V2-W12-CAPSTONE.json")]),
    ("quiz_edited_by_hand", append_to(QUIZ_D1, "\nA hand-written line.\n"), [stale(QUIZ_D1)]),
    ("schedule_edited_by_hand", append_to(SCHEDULE), [stale(SCHEDULE)]),
    ("map_edited_by_hand", append_to(MAP), [stale(MAP)]),
    ("registry_index_edited_by_hand", append_to(REGISTRY_MD), [stale(REGISTRY_MD)]),
    ("bank_edited_by_hand", append_to(BANK), [stale(BANK)]),
    ("answer_key_edited_by_hand", append_to(ANSWERS), [stale(ANSWERS)]),
    ("recall_edited_by_hand", append_to(RECALL), [stale(RECALL)]),
    ("sheets_missing", delete_file(SHEETS_JSON), [stale(SHEETS_JSON)]),
    ("keys_missing", delete_file(KEYS_JSON), [stale(KEYS_JSON)]),
    ("seed_missing", delete_file(SEED_JSON), [stale(SEED_JSON)]),
    ("quiz_missing", delete_file(QUIZ_D1), [stale(QUIZ_D1)]),
    ("index_missing", delete_file(SCHEDULE), [stale(SCHEDULE)]),
    ("assessment_payload_missing", delete_file(f"{ASSESSMENTS}/V2-W07-EXAM.json"), [stale(f"{ASSESSMENTS}/V2-W07-EXAM.json")]),
    ("assessment_payload_orphan", lambda t: write_text(t.repo(f"{ASSESSMENTS}/V2-W99-EXTRA.json"), "{}\n"), [stale(f"{ASSESSMENTS}/V2-W99-EXTRA.json")]),
    (
        "question_edited_without_regenerating",
        edit_source(f"{W7}/DAY_32.md", corpus.recall_text(32, 2), corpus.recall_text(32, 2) + " Add one more observation."),
        [stale(SHEETS_JSON), stale(f"{W7}/QUIZZES/DAY_32_QUIZ.md"), stale(BANK), stale(RECALL)],
    ),
    (
        "key_edited_without_regenerating",
        edit_source(f"{W3}/ANSWER_KEYS/DAY_12_KEY.md", corpus.day_answer(12, "W03D12-R1"), "A different reference answer for the first question of the day twelve."),
        [stale(KEYS_JSON), stale(ANSWERS)],
    ),
    (
        "registry_access_note_edited_without_regenerating",
        edit_source(f"{W3}/RESOURCES.json", "A free account is assumed for this synthetic entry.", "No account is needed for this synthetic entry."),
        [stale(DRAFT_JSON)],
    ),
    (
        "registry_title_edited_without_regenerating",
        edit_source(f"{W3}/RESOURCES.json", '"title": "Synthetic synthetic-docs of week 3"', '"title": "A renamed synthetic page of week 3"'),
        [stale(REGISTRY_MD), stale(SHEETS_JSON)],
    ),
    (
        "week_title_source_edited",
        edit_source(f"{W3}/ASSESSMENT.md", "- Title: Week 3 synthetic assessment", "- Title: Week 3 renamed assessment"),
        [stale(f"{ASSESSMENTS}/V2-W03-EXAM.json"), stale(MAP)],
    ),
    (
        "week_guide_edited_without_regenerating",
        lambda t: t.sub("WEEK_GUIDE.md", "**Purpose.** ", "**Purpose.** An added sentence. ", week=3),
        [stale(SHEETS_JSON)],
    ),
]


class FullCorpus(TreeCase):
    """All 12 weeks written by the helper and the generated files written by --write (template TWELVE)."""

    template = "twelve"

    def test_check_passes_and_the_generated_files_are_current(self) -> None:
        proc = self.build()
        self.assertExit(proc, 0)
        out = output_of(proc)
        self.assertRegex(out, r"ok    v2 courseware valid: 12 week\(s\) \[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12\]; 60 days; 480 questions; 240 exam questions; 36 resources \(24 cited\); \d+ words in the day guides")
        self.assertIn(f"ok    generated files are current ({GENERATED_FILES} files)", out)

    def test_available_equals_the_default_check_when_all_weeks_exist(self) -> None:
        proc = self.build("--available")
        self.assertExit(proc, 0)
        self.assertIn(f"ok    generated files are current ({GENERATED_FILES} files)", output_of(proc))

    def test_second_write_changes_nothing(self) -> None:
        before = _generated(self.root)
        proc = self.build("--write")
        self.assertExit(proc, 0)
        self.assertIn("nothing to write: generated files are current", output_of(proc))
        self.assertEqual(before, _generated(self.root))

    def test_write_repairs_a_stale_copy_and_removes_an_orphan(self) -> None:
        pristine = _generated(self.root)
        append_to(SHEETS_JSON)(self.tree)
        delete_file(QUIZ_D1)(self.tree)
        orphan = self.root / ASSESSMENTS / "V2-W99-EXTRA.json"
        write_text(orphan, "{}\n")
        proc = self.build("--write")
        self.assertExit(proc, 0)
        self.assertIn("wrote 3 file(s)", output_of(proc))
        self.assertFalse(orphan.exists())
        self.assertEqual(pristine, _generated(self.root))
        self.assertExit(self.build(), 0)

    def test_available_write_reports_and_writes_nothing(self) -> None:
        append_to(SHEETS_JSON)(self.tree)
        before = _generated(self.root)
        proc = self.build("--available", "--write")
        self.assertExit(proc, 0)
        out = output_of(proc)
        self.assertIn("--available: nothing was written; 1 generated file(s) would be written or removed", out)
        self.assertIn(SHEETS_JSON, out)
        self.assertEqual(before, _generated(self.root))

    def test_available_compares_when_all_weeks_exist(self) -> None:
        append_to(SHEETS_JSON)(self.tree)
        self.assertRule(self.build("--available"), stale(SHEETS_JSON))

    def test_week_mode_does_not_compare_generated_files(self) -> None:
        append_to(SHEETS_JSON)(self.tree)
        proc = self.build("--week", "3")
        self.assertExit(proc, 0)
        self.assertIn("partial run (weeks [3])", output_of(proc))

    def test_write_is_refused_when_one_week_is_missing(self) -> None:
        shutil.rmtree(self.root / WEEKS_REL / "W12")
        before = _generated_or_none(self.root)
        proc = self.build("--write")
        self.assertRule(proc, "W12: the week is not written")
        self.assertEqual(before, _generated_or_none(self.root))

    def test_default_check_fails_when_one_week_is_missing(self) -> None:
        shutil.rmtree(self.root / WEEKS_REL / "W09")
        self.assertRule(self.build(), "W09: the week is not written")

    def test_every_week_is_checked_in_default_mode(self) -> None:
        self.tree.sub("DAY_55.md", "- Status: PROPOSED", "- Status: DONE", week=11)
        self.assertRule(self.build(), "W11/DAY_55.md: metadata line 1 must be '- Status: PROPOSED'")

    def test_generated_markdown_says_that_it_is_generated(self) -> None:
        files = _generated(self.root)
        markdown = [rel for rel in files if rel.endswith(".md")]
        self.assertEqual(len(markdown), 66)
        for rel in markdown:
            first = files[rel].decode("utf-8").split("\n", 1)[0]
            self.assertTrue(first.startswith("Generated by `scripts/build_curriculum_v2.py` from "), f"{rel}: {first}")
            self.assertIn("Do not edit by hand; run `python3 -I scripts/build_curriculum_v2.py --write`.", first, rel)

    def test_json_is_deterministic_text(self) -> None:
        for rel, data in _generated(self.root).items():
            if rel.endswith(".json"):
                text = data.decode("utf-8")
                self.assertTrue(text.endswith("}\n") and not text.endswith("\n\n"), rel)
                self.assertEqual(text, json.dumps(json.loads(text), indent=2, ensure_ascii=False) + "\n", rel)

    def test_a_second_corpus_built_elsewhere_generates_the_same_bytes(self) -> None:
        other = self.tmp / "other"
        _base_tree(other)
        corpus.write_corpus(other, PLAN)
        self.assertExit(run_gate(other / "scripts" / SCRIPT, "--write", cwd=other), 0)
        self.assertEqual(_generated(self.root), _generated(other))

    def test_quiz_files_hold_the_questions_and_no_key(self) -> None:
        text = self.read(f"{WEEKS_REL}/W02/QUIZZES/DAY_07_QUIZ.md")
        for k in range(1, 6):
            self.assertIn(f"{k}. [W02D07-R{k}] {corpus.recall_text(7, k)}", text)
        for k in range(1, 4):
            self.assertIn(f"{k}. [W02D07-A{k}] {corpus.applied_text(7, k)}", text)
        self.assertNotIn("Reference answer", text)
        self.assertNotIn("Marking", text)

    def test_only_the_instructor_files_hold_answers(self) -> None:
        files = {rel: data.decode("utf-8") for rel, data in _generated(self.root).items()}
        holders = sorted(rel for rel, text in files.items() if "Reference answer for" in text)
        self.assertEqual(holders, sorted([KEYS_JSON, ANSWERS]))
        learner = list((self.root / WEEKS_REL).glob("W*/*.md")) + list((self.root / WEEKS_REL).glob("W*/LABS/**/*"))
        for path in learner:
            if path.is_file():
                self.assertNotIn("Reference answer for", read_text(path), path.name)

    def test_bank_counts(self) -> None:
        text = self.read(BANK)
        for row in ("| Daily recall | 300 |", "| Daily applied | 180 |", "| Exam CONCEPT | 72 |", "| Exam INTERPRET | 72 |", "| Exam CONTROL | 48 |", "| Exam SYNTHESIS | 48 |", "| Total | 720 |", "| Of the exam questions, multiple choice | 96 |"):
            self.assertIn(row, text)
        self.assertNotIn("Reference answer", text)

    def test_schedule_has_one_row_per_day(self) -> None:
        rows = [line for line in self.read(SCHEDULE).split("\n") if re.match(r"^\| D\d\d \|", line)]
        self.assertEqual(len(rows), 60)
        self.assertTrue(rows[0].startswith("| D01 | 2026-11-16 | Monday | 1 | Synthetic topic 01 |"))
        self.assertIn(f"| {seed(1, 1)} | {seed(1, 2)} | {seed(1, 3)} | {seed(1, 4)} | no |", rows[0])
        self.assertTrue(rows[59].startswith("| D60 | 2027-02-05 | Friday | 12 |"))
        self.assertTrue(rows[59].endswith("| `V2-W12-CAPSTONE` |"))
        self.assertTrue(rows[4].endswith("| `V2-W01-EXAM` |"))

    def test_map_lists_every_competency_and_every_day(self) -> None:
        text = self.read(MAP)
        packs = {c for p in (self.root / PACKS_REL).glob("*/pack.json") for c in (x["id"] for x in json.loads(read_text(p))["competencies"])}
        for c in packs:
            self.assertIn(f"`{c}`", text)
        self.assertEqual(len([line for line in text.split("\n") if re.match(r"^\| D\d\d \|", line)]), 60)
        self.assertIn("| 12 | D60 | `V2-W12-CAPSTONE` | CAPSTONE |", text)

    def test_registry_index_lists_every_entry_with_its_label_and_date(self) -> None:
        text = self.read(REGISTRY_MD)
        for w in range(1, 13):
            for name in ("synthetic-docs", "synthetic-module", "synthetic-lab"):
                self.assertRegex(text, rf"\| `w{w:02d}-{name}` \| .* \| FREE_ACCOUNT \| SEARCH_CHECKED \| 2026-11-18 \|")
        self.assertIn("### Plan fact checks", text)
        self.assertIn("### Gaps", text)
        self.assertIn("| `w01-synthetic-lab` |", text)
        self.assertIn("A description of a local substitute.", text)

    def test_the_week_11_friday_exception_is_accepted(self) -> None:
        # day 55: block A is a lesson and the exam sits in block D (the helper writes it so; the full corpus passed above)
        text = self.read(f"{WEEKS_REL}/W11/DAY_55.md")
        a_block = text.split("## A — ")[1].split("## B — ")[0]
        d_block = text.split("## D — ")[1].split("## Commands")[0]
        self.assertNotIn("Part 1", a_block)
        self.assertIn("Part 1", d_block)

    def test_the_draft_manifest(self) -> None:
        draft = load_json_file(self.root / DRAFT_JSON)
        seed_doc = load_json_file(self.root / SEED_JSON)
        self.assertEqual((draft["version"], seed_doc["version"]), ("2.0.0-draft1", "2.0.0-seed"))
        for doc in (draft, seed_doc):
            self.assertEqual(doc["manifest_schema_version"], "1.1.0")
            self.assertEqual(doc["curriculum_id"], "g9-cyber-mastery-480")
            self.assertEqual(doc["name"], "G9 Cybersecurity Mastery v2.0 (480 hours)")
            self.assertEqual(doc["provenance"], "PROPOSED_SEED")
            self.assertEqual(doc["structure"]["assessment_day_of_week"], 5)
            self.assertEqual(len(doc["weeks"]), 12)
        days = [d for w in draft["weeks"] for d in w["days"]]
        self.assertEqual(len(days), 60)
        for n, day in enumerate(days, start=1):
            self.assertEqual(day["day_id"], f"D{n:02d}")
            self.assertEqual(day["planned_minutes"], 480)
            for k, objective in enumerate(day["objectives"], start=1):
                self.assertTrue(objective["text"].startswith(SEEDS[n][k - 1] + " — "), f"D{n:02d}-O{k}")
                self.assertLessEqual(len(objective["text"]), 500)
            self.assertTrue(day["evidence_artifact"].startswith(SEEDS[n][3] + " — "))
            self.assertLessEqual(len(day["evidence_artifact"]), 500)
            self.assertEqual([o.get("competency_ids", []) for o in day["objectives"]][2], [])
            kinds = [r["kind"] for r in day["resources"]]
            self.assertEqual(kinds, ["EXTERNAL", "EXTERNAL", "INTERNAL"])
            for r in day["resources"][:2]:
                self.assertEqual(set(r), {"ref", "kind", "version", "license_note"})
                self.assertTrue(r["ref"].startswith("https://"))
                self.assertEqual(r["version"], "synthetic 1.0; checked 2026-11-18")
                self.assertLessEqual(len(r["license_note"]), 300)
                self.assertTrue(r["license_note"].startswith("FREE_ACCOUNT: "))
            self.assertEqual(day["resources"][2], {"ref": f"docs/curriculum/v2/WEEKS/W{(n - 1) // 5 + 1:02d}/DAY_{n:02d}.md", "kind": "INTERNAL"})

    def test_the_sheets(self) -> None:
        sheets = load_json_file(self.root / SHEETS_JSON)
        draft = load_json_file(self.root / DRAFT_JSON)
        self.assertEqual((sheets["sheets_schema_version"], sheets["curriculum_id"], sheets["built_for_version"]), ("1.1.0", "g9-cyber-mastery-480", "2.0.0-draft1"))
        self.assertEqual(sorted(sheets["weeks"], key=int), [str(w) for w in range(1, 13)])
        for w, week in sheets["weeks"].items():
            self.assertEqual(sorted(week), ["accelerator", "competencies", "lab_stack", "purpose", "title"])
            self.assertEqual(week["title"], PLAN["titles"][int(w)])
            self.assertEqual(week["purpose"], corpus.prose(f"purpose{w}", 30))
        self.assertEqual(list(sheets["days"]), [f"D{n:02d}" for n in range(1, 61)])
        drafts = {d["day_id"]: d for wk in draft["weeks"] for d in wk["days"]}
        for ident, day in sheets["days"].items():
            n = int(ident[1:])
            friday = n % 5 == 0
            self.assertEqual(day["objectives"], [o["text"] for o in drafts[ident]["objectives"]])
            self.assertEqual(day["title"], f"Synthetic topic {n:02d}")
            self.assertEqual([s["key"] for s in day["sections"]], ["recall", "theory", "performance_review", "blind_practical", "report"] if friday else ["recall", "theory", "guided_lab", "independent_lab", "defense_verification"])
            self.assertEqual([s["minutes"] for s in day["sections"]], [30, 120, 120, 150, 60])
            self.assertEqual([s["name"] for s in day["sections"]], ["Active recall"] + [f"Synthetic block {b}" for b in "ABCD"])
            self.assertEqual(day["sections"][1]["markdown"], corpus.block_body(n, "A", friday))
            self.assertIsNone(day["watch_for"])
            self.assertEqual(day["extend"], f"Extend the exercise of day {n} with a second guest; drop it without loss.")
            self.assertEqual(day["hand_in"], drafts[ident]["evidence_artifact"])
            self.assertEqual(sorted(day["header"]), ["date", "dependencies", "lab_authorization", "provider", "status", "weekday"])
            self.assertEqual(day["header"]["date"], corpus.day_date(n).isoformat())
            self.assertEqual(("commands" in day), not friday)
            self.assertEqual(len(day["hint_ladder"].split("\n")), 3)
            self.assertFalse(day["hint_ladder"].startswith("4."))
            self.assertNotIn("Instructor explanation", day["hint_ladder"])
            self.assertEqual(len(day["troubleshooting"].split("\n")), 7)
            self.assertEqual([q["id"] for q in day["quiz"]["recall"]], corpus.recall_ids(n))
            self.assertEqual([q["id"] for q in day["quiz"]["applied"]], corpus.applied_ids(n))
            resources = day["resources"].split("\n")
            self.assertEqual(len(resources), 2)
            self.assertRegex(resources[0], r"^- `https://example\.com/g9-test/w\d\d/synthetic-docs`: Synthetic synthetic-docs of week \d+\. Provider: Synthetic Provider \d+\. Access: FREE_ACCOUNT\. Verification: SEARCH_CHECKED \(2026-11-18\)\. Use: block A: ")
        self.assertIn("| # | Command | Target |", sheets["days"]["D02"]["commands"])
        self.assertIn("| Offensive mechanism |", sheets["days"]["D02"]["mapping"])

    def test_the_keys(self) -> None:
        keys = load_json_file(self.root / KEYS_JSON)
        self.assertEqual((keys["keys_schema_version"], keys["curriculum_id"], keys["built_for_version"]), ("1.0.0", "g9-cyber-mastery-480", "2.0.0-draft1"))
        self.assertEqual(list(keys["days"]), [f"D{n:02d}" for n in range(1, 61)])
        self.assertEqual(list(keys["exams"]), [f"V2-W{w:02d}-EXAM" for w in range(1, 12)] + ["V2-W12-CAPSTONE"])
        day = keys["days"]["D17"]
        self.assertEqual(list(day["quiz"]), corpus.recall_ids(17) + corpus.applied_ids(17))
        self.assertEqual(sorted(day["quiz"]["W04D17-R2"]), ["answer", "marking", "source"])
        self.assertEqual(day["quiz"]["W04D17-R2"]["answer"], corpus.day_answer(17, "W04D17-R2"))
        self.assertEqual(day["quiz"]["W04D17-R2"]["source"], "w04-synthetic-docs")
        self.assertTrue(day["challenge"].startswith("The reference observations of day 17"))
        self.assertTrue(day["hint4"].startswith("The mode on the directory of day 17"))
        self.assertTrue(day["overclaims"].startswith("- Claiming"))
        exam = keys["exams"]["V2-W04-EXAM"]
        self.assertEqual(sorted(exam), ["gate", "practical", "questions"])
        self.assertEqual(list(exam["questions"]), corpus.exam_ids(4))
        self.assertEqual(sorted(exam["questions"]["W04X-Q01"]), ["answer", "distractors", "why"])
        self.assertIsNotNone(exam["questions"]["W04X-Q01"]["distractors"])
        self.assertIsNone(exam["questions"]["W04X-Q09"]["distractors"])

    def test_the_assessment_payloads(self) -> None:
        names = sorted(p.name for p in (self.root / ASSESSMENTS).glob("*.json"))
        self.assertEqual(names, sorted([f"V2-W{w:02d}-EXAM.json" for w in range(1, 12)] + ["V2-W12-CAPSTONE.json"]))
        payload = load_json_file(self.root / ASSESSMENTS / "V2-W12-CAPSTONE.json")
        self.assertEqual(sorted(payload), ["code", "competency_id", "difficulty", "kind", "task_statement", "title"])
        self.assertEqual((payload["code"], payload["kind"], payload["title"]), ("V2-W12-CAPSTONE", "CAPSTONE", "Week 12 synthetic assessment"))
        text = read_text(self.root / WEEKS_REL / "W12" / "ASSESSMENT.md")
        start = text.index("## Rules")
        end = text.index("## Remediation track")
        self.assertEqual(payload["task_statement"], text[start:end].strip())
        self.assertIn("## Part 4 — Evidence and reporting (15 points)", payload["task_statement"])
        self.assertNotIn("## Remediation track", payload["task_statement"])
        week_one = load_json_file(self.root / ASSESSMENTS / "V2-W01-EXAM.json")
        self.assertEqual((week_one["kind"], week_one["competency_id"], week_one["difficulty"]), ("BLIND_PRACTICAL", "net.layers-addressing", 1))

    def test_spaced_recall_follows_the_plan_calendar(self) -> None:
        sheets = load_json_file(self.root / SHEETS_JSON)
        offsets = {1: 1, 2: 1, 3: 3, 4: 7, 5: 14}
        expected: dict[int, set[str]] = {n: set() for n in range(1, 61)}
        after: list[tuple[str, str]] = []
        for n in range(1, 61):
            for k, offset in offsets.items():
                due = corpus.day_date(n) + datetime.timedelta(days=offset)
                target = next((m for m in range(1, 61) if corpus.day_date(m) >= due), None)
                ident = corpus.recall_ids(n)[k - 1]
                if target is None:
                    after.append((due.isoformat(), ident))
                else:
                    expected[target].add(ident)
        for n in range(1, 61):
            got = {q["id"] for q in sheets["days"][f"D{n:02d}"]["recall_queue"]}
            self.assertEqual(got, expected[n], f"D{n:02d}")
        # explicit anchors, calendar by calendar: a Friday's 1-day question waits for Monday; the 14-day question of day 1 falls on day 11
        self.assertIn("W01D05-R1", expected[6])
        self.assertIn("W01D01-R5", expected[11])
        self.assertIn("W01D01-R4", expected[6])
        for item in sheets["days"]["D06"]["recall_queue"]:
            self.assertEqual(item["from_day"], item["id"][3:6])
            self.assertEqual(item["text"], corpus.recall_text(int(item["from_day"][1:]), int(item["id"][-1])))
        text = self.read(RECALL)
        post = text.split("## Post-course review", 1)[1]
        self.assertEqual(sorted(re.findall(r"^\| (\d{4}-\d{2}-\d{2}) \| (W\d\dD\d\d-R\d) \|", post, flags=re.M)), sorted(after))
        self.assertIn("W12D60-R1", post)
        self.assertNotIn("W12D56-R1", post)  # day 56 (Mon 2027-02-01) + 1 falls on day 57
        self.assertIn("W12D56-R4", post)  # +7 falls on 2027-02-08, after day 60
        for n in (1, 30, 60):
            self.assertIn(f"## D{n:02d} — {corpus.day_date(n).isoformat()}", text)

    def test_summary_counts_words(self) -> None:
        out = output_of(self.build())
        m = re.search(r"(\d+) words in the day guides", out)
        assert m is not None
        words = 0
        for n in range(1, 61):
            words += len(read_text(self.root / WEEKS_REL / f"W{(n - 1) // 5 + 1:02d}" / f"DAY_{n:02d}.md").split())
        self.assertEqual(int(m.group(1)), words)


def _generated_or_none(root: Path) -> dict[str, bytes] | None:
    try:
        return _generated(root)
    except FileNotFoundError:
        return None


def load_json_file(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def _make_stale(mutate: Callable[[Tree], None], needles: list[str]) -> Callable[[FullCorpus], None]:
    def test(self: FullCorpus) -> None:
        mutate(self.tree)
        proc = self.build()
        self.assertExit(proc, 1)
        out = output_of(proc)
        for needle in needles:
            self.assertTrue(needle in out, f"{needle!r} missing; output tail:\n{out[-3000:]}")

    return test


for _name, _mutate, _needles in STALE_CASES:
    _test = _make_stale(_mutate, _needles)
    _test.__name__ = f"test_stale_{_name}"
    _test.__doc__ = f"expects {_needles[0]!r}"
    setattr(FullCorpus, _test.__name__, _test)


class FullCorpusRules(TreeCase):
    """Rules that only a complete corpus shows."""

    template = "twelve"

    def test_a_broken_day_in_week_nine_is_found_in_default_mode(self) -> None:
        self.tree.sub("DAY_44.md", "- Status: PROPOSED", "- Status: DONE", week=9)
        proc = self.build()
        self.assertRule(proc, "W09/DAY_44.md: metadata line 1 must be '- Status: PROPOSED'")
        self.assertNotIn("generated files are current", output_of(proc))

    def test_a_day_that_cites_an_entry_of_a_later_week(self) -> None:
        row = "| w02-synthetic-docs | Read the canonical page and keep its title and date as evidence | A |"
        self.tree.sub("DAY_02.md", ROW_DOCS, row)
        self.assertExit(self.build("--week", "1"), 0)

    def test_registry_id_of_another_week_is_found_across_files(self) -> None:
        self.tree.registry(lambda d: d["entries"][0].update({"id": "w05-synthetic-docs"}), week=2)
        self.assertRule(self.build("--week", "2"), "'id' must start with 'w02-' (the week of the file)")

    def test_identifier_used_in_two_weeks(self) -> None:
        # a question identifier repeated in another week is refused by the corpus rule (and by the identifier rule of its day)
        self.tree.sub("DAY_06.md", "[W02D06-R1]", "[W01D01-R1]", week=2)
        proc = self.build()
        self.assertExit(proc, 1)
        self.assertIn("'W01D01-R1' is used twice", output_of(proc))

    def test_same_question_text_in_two_weeks(self) -> None:
        self.tree.sub("DAY_06.md", "[W02D06-R1] " + corpus.recall_text(6, 1), "[W02D06-R1] " + corpus.recall_text(1, 1), week=2)
        proc = self.build()
        self.assertExit(proc, 1)
        self.assertIn("W01D01-R1 and W02D06-R1 have the same text", output_of(proc))

    def test_day_55_sits_the_exam_in_block_d(self) -> None:
        # the plan's exception (week 11): block A is a lesson and the knowledge exam sits in block D
        self.tree.sub("DAY_55.md", " The knowledge exam is in ASSESSMENT.md, Part 1; scoring and the report are in ASSESSMENT.md, Part 4.", " Scoring and the report are in ASSESSMENT.md, Part 4.", week=11)
        proc = self.build("--week", "11")
        self.assertRule(proc, "block D of a Friday refers to 'ASSESSMENT.md' and 'Part 4'")
        self.assertIn("missing: ['Part 1']", output_of(proc))

    def test_other_fridays_keep_the_exam_in_block_a(self) -> None:
        self.tree.sub("DAY_50.md", " The knowledge exam is in ASSESSMENT.md, Part 1.", " The exam is in the file.", week=10)
        self.assertRule(self.build("--week", "10"), "block A of a Friday refers to 'ASSESSMENT.md' and 'Part 1'")

    def test_fallback_to_an_entry_of_another_week(self) -> None:
        self.tree.registry(lambda d: d["entries"][1].update({"fallback": "w09-synthetic-docs"}), week=2)
        self.assertExit(self.build("--week", "2"), 0)

    def test_leak_of_a_week_two_answer_in_week_two_only(self) -> None:
        # the rule is per week: an answer of week 1 in a learner file of week 2 is not a leak of week 2
        self.tree.append("EVIDENCE_CHECKLIST.md", "\n" + corpus.day_answer(2, "W01D02-R1") + "\n", week=2)
        self.assertExit(self.build("--week", "1", "--week", "2"), 0)


# ---------------------------------------------------------------------------------------------------
# schemas
# ---------------------------------------------------------------------------------------------------


@NEEDS_JSONSCHEMA
class Schemas(TreeCase):
    template = "twelve"

    def validator(self, rel: str):
        from jsonschema import Draft202012Validator

        return Draft202012Validator(json.loads(read_text(REPO / rel)))

    def errors(self, rel: str, doc: object) -> list[str]:
        return sorted(e.message[:120] for e in self.validator(rel).iter_errors(doc))

    def test_schema_files_pass_the_house_rules(self) -> None:
        holder = self.tmp / "schemas"
        holder.mkdir()
        for name in ("lesson-sheets", "answer-keys", "resource-registry"):
            shutil.copy2(REPO / "schemas" / "curriculum" / f"{name}.schema.json", holder / f"{name}.schema.json")
        proc = run_gate(REPO / "scripts" / "check_schemas.py", holder, cwd=REPO)
        self.assertExit(proc, 0)
        self.assertIn("3 valid, 0 invalid", output_of(proc))

    def test_the_v1_sheets_file_still_validates(self) -> None:
        doc = json.loads(read_text(REPO / DATA / "g9-ct9-480.sheets.v1.json"))
        self.assertEqual(self.errors("schemas/curriculum/lesson-sheets.schema.json", doc), [])

    def test_the_v2_sheets_file_validates(self) -> None:
        doc = load_json_file(self.root / SHEETS_JSON)
        self.assertEqual(self.errors("schemas/curriculum/lesson-sheets.schema.json", doc), [])

    def test_the_v2_keys_file_validates(self) -> None:
        self.assertEqual(self.errors("schemas/curriculum/answer-keys.schema.json", load_json_file(self.root / KEYS_JSON)), [])

    def test_the_assessment_payloads_validate(self) -> None:
        for path in sorted((self.root / ASSESSMENTS).glob("*.json")):
            self.assertEqual(self.errors("schemas/ipc/assessment.define.request.schema.json", load_json_file(path)), [], path.name)

    def test_the_synthetic_registries_validate(self) -> None:
        for w in range(1, 13):
            path = self.root / WEEKS_REL / f"W{w:02d}" / "RESOURCES.json"
            self.assertEqual(self.errors("schemas/curriculum/resource-registry.schema.json", load_json_file(path)), [], path.parent.name)

    def test_the_builder_refuses_every_real_registry_that_the_schema_refuses(self) -> None:
        # Authors write these files while the tests run. The builder checks the schema's rules and more (the week of the
        # file, unique ids, a fallback id that exists): a registry the schema refuses must be refused by the builder too.
        # A registry that both refuse is the author's to fix, not a failure of the gate.
        for path in sorted((REPO / WEEKS_REL).glob("W*/RESOURCES.json")):
            w = int(path.parent.name[1:])
            try:
                doc = json.loads(read_text(path))
            except ValueError:
                continue
            if not self.errors("schemas/curriculum/resource-registry.schema.json", doc):
                continue
            holder = self.tmp / f"real-{w}"
            _base_tree(holder)
            corpus.write_week(holder, PLAN, w)
            shutil.copy(path, holder / WEEKS_REL / path.parent.name / "RESOURCES.json")
            proc = run_gate(holder / "scripts" / SCRIPT, "--week", str(w), cwd=holder)
            self.assertExit(proc, 1)
            self.assertIn(f"W{w:02d}/RESOURCES.json", output_of(proc), path.parent.name)

    def test_sheets_schema_refuses_what_it_should(self) -> None:
        doc = load_json_file(self.root / SHEETS_JSON)
        refused = "schemas/curriculum/lesson-sheets.schema.json"

        def case(mutate: Callable[[dict], None]) -> list[str]:
            copy = json.loads(json.dumps(doc))
            mutate(copy)
            return self.errors(refused, copy)

        self.assertTrue(case(lambda d: d.update({"sheets_schema_version": "1.0.0"})), "1.0.0 holds 72 days")
        self.assertTrue(case(lambda d: d.update({"sheets_schema_version": "1.2.0"})), "unknown version")
        self.assertTrue(case(lambda d: d["days"].pop("D60")), "1.1.0 holds exactly 60 days")
        self.assertTrue(case(lambda d: d["days"].update({"D61": d["days"]["D01"]})), "no D61")
        self.assertTrue(case(lambda d: d["days"]["D01"].pop("quiz")), "1.1.0 needs the quiz")
        self.assertTrue(case(lambda d: d["days"]["D01"].pop("recall_queue")), "1.1.0 needs the recall queue")
        self.assertTrue(case(lambda d: d["days"]["D01"].pop("header")), "1.1.0 needs the header")
        self.assertEqual(case(lambda d: d["days"]["D05"].pop("commands", None)), [], "commands are optional (a Friday has none)")
        self.assertTrue(case(lambda d: d["days"]["D01"]["quiz"]["recall"].pop()), "five recall questions")
        self.assertTrue(case(lambda d: d["days"]["D01"]["quiz"]["applied"].append({"id": "W01D01-A3", "text": "x"})), "three applied questions")
        self.assertTrue(case(lambda d: d["days"]["D01"]["quiz"]["recall"][0].update({"answer": "x"})), "no key in a question")
        self.assertTrue(case(lambda d: d["days"]["D01"]["quiz"]["recall"][0].update({"id": "W01D01-R6"})), "R1 to R5 only")
        self.assertTrue(case(lambda d: d["days"]["D01"].update({"answer_key": "x"})), "no unknown field")
        self.assertTrue(case(lambda d: d["days"]["D01"]["header"].update({"status": "APPROVED"})), "the header status is PROPOSED")
        self.assertTrue(case(lambda d: d["days"]["D01"]["header"].update({"weekday": "Saturday"})), "no Saturday")
        self.assertTrue(case(lambda d: d["days"]["D01"]["sections"][0].update({"name": "n" * 65})), "section names stay at 64 characters")
        self.assertTrue(case(lambda d: d["days"]["D01"]["recall_queue"].append({"id": "W01D01-R1", "from_day": "D61", "offset_days": 1, "text": "x"})), "no D61")
        self.assertTrue(case(lambda d: d["days"]["D01"]["recall_queue"].append({"id": "W01D01-R1", "from_day": "D01", "offset_days": 15, "text": "x"})), "offset at most 14")

    def test_the_v1_schema_rules_still_hold_for_a_v1_file(self) -> None:
        doc = json.loads(read_text(REPO / DATA / "g9-ct9-480.sheets.v1.json"))
        refused = "schemas/curriculum/lesson-sheets.schema.json"
        for mutate in (
            lambda d: d["days"].pop("D72"),
            lambda d: d.update({"sheets_schema_version": "1.1.0"}),
            lambda d: d["days"]["D01"].pop("watch_for"),
            lambda d: d["days"]["D01"].update({"unknown": 1}),
            lambda d: d["weeks"].pop("12"),
        ):
            copy = json.loads(json.dumps(doc))
            mutate(copy)
            self.assertTrue(self.errors(refused, copy))

    def test_keys_schema_refuses_what_it_should(self) -> None:
        doc = load_json_file(self.root / KEYS_JSON)
        refused = "schemas/curriculum/answer-keys.schema.json"
        for name, mutate in (
            ("version", lambda d: d.update({"keys_schema_version": "2.0.0"})),
            ("59 days", lambda d: d["days"].pop("D01")),
            ("seven answers", lambda d: d["days"]["D01"]["quiz"].pop("W01D01-A3")),
            ("a ninth answer", lambda d: d["days"]["D01"]["quiz"].update({"W01D01-R6": {"answer": "x", "marking": "x", "source": "x"}})),
            ("a missing marking", lambda d: d["days"]["D01"]["quiz"]["W01D01-R1"].pop("marking")),
            ("an extra field", lambda d: d["days"]["D01"]["quiz"]["W01D01-R1"].update({"flag": "x"})),
            ("an empty hint4", lambda d: d["days"]["D01"].update({"hint4": ""})),
            ("19 exam questions", lambda d: d["exams"]["V2-W01-EXAM"]["questions"].pop("W01X-Q20")),
            ("distractors as a number", lambda d: d["exams"]["V2-W01-EXAM"]["questions"]["W01X-Q01"].update({"distractors": 3})),
            ("a missing gate", lambda d: d["exams"]["V2-W01-EXAM"].pop("gate")),
            ("a capstone in week 1", lambda d: d["exams"].update({"V2-W13-EXAM": d["exams"]["V2-W01-EXAM"]})),
            ("eleven exams", lambda d: d["exams"].pop("V2-W12-CAPSTONE")),
        ):
            copy = json.loads(json.dumps(doc))
            mutate(copy)
            self.assertTrue(self.errors(refused, copy), name)

    def test_registry_schema_refuses_what_it_should(self) -> None:
        doc = load_json_file(self.root / WEEKS_REL / "W01" / "RESOURCES.json")
        refused = "schemas/curriculum/resource-registry.schema.json"
        for name, mutate in (
            ("week 13", lambda d: d.update({"week": 13})),
            ("a date", lambda d: d.update({"checked_on": "yesterday"})),
            ("no entries", lambda d: d.update({"entries": []})),
            ("an unknown key", lambda d: d.update({"extra": 1})),
            ("a kind", lambda d: d["entries"][0].update({"kind": "VIDEO"})),
            ("http", lambda d: d["entries"][0].update({"url": "http://example.com"})),
            ("an id", lambda d: d["entries"][0].update({"id": "synthetic-docs"})),
            ("a missing key", lambda d: d["entries"][0].pop("notes")),
            ("a day id", lambda d: d["entries"][0].update({"suggested_days": ["D1"]})),
            ("a verification label", lambda d: d["entries"][0].update({"verification": "VERIFIED"})),
            ("a fact result", lambda d: d["plan_fact_checks"][0].update({"result": "MAYBE"})),
            ("an empty gap", lambda d: d.update({"gaps": [""]})),
        ):
            copy = json.loads(json.dumps(doc))
            mutate(copy)
            self.assertTrue(self.errors(refused, copy), name)
        copy = json.loads(json.dumps(doc))
        copy["entries"][0].update({"difficulty": 3, "estimate_minutes": None, "fallback": None})
        self.assertEqual(self.errors(refused, copy), [], "difficulty may be a number; estimate and fallback may be null")


# ---------------------------------------------------------------------------------------------------
# the repository
# ---------------------------------------------------------------------------------------------------


def _weeks_with_a_day_guide() -> list[int]:
    out = []
    for w in range(1, 13):
        folder = REPO / WEEKS_REL / f"W{w:02d}"
        if folder.is_dir() and any(re.fullmatch(r"DAY_\d{2}\.md", p.name) for p in folder.iterdir()):
            out.append(w)
    return out


class RealTree(GateTestCase):
    def test_available_on_the_repository(self) -> None:
        written = _weeks_with_a_day_guide()
        proc = run_gate(REPO / "scripts" / SCRIPT, "--available", cwd=REPO)
        out = output_of(proc)
        self.assertNotIn("Traceback", out)
        if not written:
            # the state this test was written for: no week is written yet, the check passes and compares nothing
            self.assertExit(proc, 0)
            self.assertIn("ok    v2 courseware valid: 0 week(s) []; 0 days; 0 questions; 0 exam questions; 0 resources (0 cited); 0 words in the day guides", out)
            self.assertIn("partial run (weeks [])", out)
        else:
            # weeks exist: the run reports on them; while authors are writing, a rule may be broken, but never an environment error
            self.assertIn(proc.returncode, (0, 1), out[-2000:])
            self.assertTrue("ok    v2 courseware valid" in out or "problem(s); checked:" in out, out[-2000:])

    def test_seed_text_on_the_repository(self) -> None:
        proc = run_gate(REPO / "scripts" / SCRIPT, "--seed-text", "D30", cwd=REPO)
        self.assertExit(proc, 0)
        out = output_of(proc)
        self.assertIn("D30  week 6  2026-12-25 (Friday)", out)
        self.assertIn(f"Hand-in (column D): {seed(30, 4)}", out)

    def test_print_seed_on_the_repository_matches_the_plan(self) -> None:
        proc = run_gate(REPO / "scripts" / SCRIPT, "--print-seed", cwd=REPO)
        self.assertExit(proc, 0)
        doc = json.loads(proc.stdout)
        days = [d for w in doc["weeks"] for d in w["days"]]
        self.assertEqual(len(days), 60)
        for n, day in enumerate(days, start=1):
            self.assertEqual([o["text"] for o in day["objectives"]], SEEDS[n][:3])
            self.assertEqual(day["evidence_artifact"], SEEDS[n][3])

    def test_the_default_check_on_the_repository_fails_until_every_week_is_written(self) -> None:
        if len(_weeks_with_a_day_guide()) == 12:
            self.skipTest("all 12 weeks exist; the default check is the gate")
        proc = run_gate(REPO / "scripts" / SCRIPT, cwd=REPO)
        self.assertExit(proc, 1)
        self.assertIn("the week is not written", output_of(proc))

    def test_ci_runs_the_builder(self) -> None:
        workflow = read_text(REPO / ".github" / "workflows" / "ci.yml")
        preflight = read_text(REPO / "scripts" / "ci_preflight.sh")
        self.assertRegex(workflow, r"run: \$PY -I scripts/build_curriculum_v2\.py")
        self.assertIn("python3 -I scripts/build_curriculum_v2.py", preflight)
        self.assertIn("scripts/build_curriculum_v2.py", workflow.split("content-gates:")[1].split("sbom:")[0])
        # the workflow stays manual (owner's decision, task 42)
        triggers = workflow.split("\njobs:")[0]
        self.assertIn("workflow_dispatch", triggers)
        for forbidden in ("push:", "pull_request:", "schedule:"):
            self.assertNotIn(forbidden, triggers)

    def test_the_package_of_the_builder_is_standard_library_only(self) -> None:
        imports: set[str] = set()
        for node in ast.walk(ast.parse(read_text(REPO / "scripts" / SCRIPT))):
            if isinstance(node, ast.Import):
                imports |= {alias.name.split(".")[0] for alias in node.names}
            elif isinstance(node, ast.ImportFrom) and node.module:
                imports.add(node.module.split(".")[0])
        stdlib = {"__future__", "argparse", "datetime", "hashlib", "json", "re", "sys", "pathlib", "typing"}
        self.assertEqual(imports - stdlib, set())


class Sanity(unittest.TestCase):
    def test_case_names_unique(self) -> None:
        for cases in (CASES, STALE_CASES):
            names = [c[0] for c in cases]
            self.assertEqual(len(names), len(set(names)))

    def test_there_are_enough_rule_cases(self) -> None:
        self.assertGreaterEqual(len(CASES) + len(STALE_CASES), 45)

    def test_repository_inputs_present(self) -> None:
        missing = [k for k, v in digest_inputs([PLAN_REL, f"{V2}/AUTHORING.md", PACKS_REL, "schemas/curriculum/lesson-sheets.schema.json", "schemas/curriculum/answer-keys.schema.json", "schemas/curriculum/resource-registry.schema.json"]).items() if v == "<missing>"]
        self.assertEqual(missing, [])

    def test_hidden_matches_the_v1_builder(self) -> None:
        def load(name: str):
            spec = importlib.util.spec_from_file_location(name.replace(".py", "_under_test"), REPO / "scripts" / name)
            assert spec is not None and spec.loader is not None
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
            return module

        self.assertEqual(set(load(SCRIPT).HIDDEN), set(load("build_curriculum.py").HIDDEN))

    def test_the_synthetic_corpus_helper_is_deterministic(self) -> None:
        self.assertEqual(corpus.prose("a", 40), corpus.prose("a", 40))
        self.assertNotEqual(corpus.prose("a", 40), corpus.prose("b", 40))
        self.assertEqual(len(corpus.prose("a", 140).split()), 140)

    def test_the_generator_and_the_independent_parse_agree_on_the_calendar(self) -> None:
        self.assertEqual(corpus.day_date(1), datetime.date(2026, 11, 16))
        self.assertEqual(corpus.day_date(5), datetime.date(2026, 11, 20))
        self.assertEqual(corpus.day_date(6), datetime.date(2026, 11, 23))
        self.assertEqual(corpus.day_date(60), datetime.date(2027, 2, 5))
        self.assertEqual(len(SEEDS), 60)
        self.assertEqual(len(PLAN["titles"]), 12)


if __name__ == "__main__":
    unittest.main()
