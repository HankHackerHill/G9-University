"""Black-box tests for scripts/build_curriculum.py in its default (check) mode (step CLI-2).

The script resolves the repository root from its own location, so every test runs a byte-for-byte copy
of it at <tmp>/scripts/build_curriculum.py next to copies of its inputs (week files, seed, competency
packs) and of the files it generates (draft manifest, lesson sheets, assessment payloads, coverage.md).
--write is never run on the repository: run_gate() refuses it for the repository script, and the one
write test below runs it on the temporary copy.

Rules enumerated from the script, one case each in CASES (label: where the message comes from):
  parse_week     hidden/control character; second week heading; day before the week heading;
                 preamble field twice; section twice in a day; file without a week heading
  check_week     preamble paragraph missing; week number outside 1..12; file name; week title against
                 the seed; day ids against the seed; seed block lists against the section names;
                 required/unknown/out-of-order sections; words per block section; hand-in present,
                 prefix and length; exactly 3 objective lines; objective line format, numbering,
                 Appendix A prefix, length, <= 3 distinct competencies, known competencies; resource
                 line prefix, format, version length, licence-note length, https URL, version for
                 external resources; >= 1 external resource per day
  assessment     the five fields; Task statement present, non-empty, length; code pattern; kind;
                 competency known and linked from the day; difficulty integer 1..5 and inside the band;
                 title length
  main           all 12 weeks; unique assessment codes; every competency targeted >= 2 times;
                 no competency before its prerequisite; generated files current (changed, missing,
                 orphaned); environment error (exit 2)
Not reachable through the inputs: the day-sheet locator check ("does not resolve to a '## Dnn — '
heading") re-reads the file that was just parsed and every parsed day heading satisfies it.
Not encoded as rules here: U+000D is in HIDDEN but never reaches the scan (Path.read_text() turns CR
and CRLF into LF), which matches crates/g9-domain/tests/source_text.rs (CR is file-format whitespace);
HIDDEN is narrower than g9_domain::text::is_hidden (it lacks U+2028/U+2029, U+FFF9..U+FFFB and the tag
characters U+E0000..U+E007F), which that Rust scan applies to every text file, week files included.
"""

from __future__ import annotations

import os
import shutil
import time
import unittest
from pathlib import Path
from typing import Callable

from _gate_helpers import (
    REPO,
    GateTestCase,
    InputGuard,
    copy_into,
    digest_inputs,
    edit_json,
    output_of,
    read_text,
    replace_once,
    run_gate,
    stage_script,
    write_text,
)

SCRIPT = "build_curriculum.py"
WEEKS = "docs/curriculum/weeks"
SEED = "data/curriculum/g9-ct9-480.seed.v0.json"
PACKS = "data/competency-packs"
MANIFEST = "data/curriculum/g9-ct9-480.draft.v1.json"
SHEETS = "data/curriculum/g9-ct9-480.sheets.v1.json"
ASSESSMENTS = "data/curriculum/assessments"
COVERAGE = "docs/curriculum/coverage.md"
INPUTS = [WEEKS, SEED, PACKS, MANIFEST, SHEETS, ASSESSMENTS, COVERAGE]
LINUX_PACK = f"{PACKS}/linux-python-systems/pack.json"

_GUARD = InputGuard(INPUTS)


def setUpModule() -> None:
    _GUARD.snapshot()


def tearDownModule() -> None:
    _GUARD.verify()


class Tree:
    """Mutations of one temporary copy; every edit raises if its target text is absent."""

    def __init__(self, root: Path) -> None:
        self.root = root

    def week(self, n: int) -> Path:
        return self.root / WEEKS / f"week-{n:02d}.md"

    def sub(self, n: int, old: str, new: str, after: str | None = None) -> None:
        replace_once(self.week(n), old, new, after=after)

    def cut_between(self, n: int, start: str, end: str, after: str) -> None:
        """Delete the text after `start` up to (not including) `end`, both searched after `after`."""
        path = self.week(n)
        text = read_text(path)
        base = text.index(after)
        i = text.index(start, base) + len(start)
        j = text.index(end, i)
        write_text(path, text[:i] + "\n" + text[j:])

    def drop_line(self, n: int, prefix: str) -> None:
        """Delete the first whole line of week n that starts with `prefix`."""
        path = self.week(n)
        lines = read_text(path).split("\n")
        idx = next(i for i, line in enumerate(lines) if line.startswith(prefix))
        del lines[idx]
        write_text(path, "\n".join(lines))

    def append(self, rel: str, extra: str) -> None:
        path = self.root / rel
        write_text(path, read_text(path) + extra)

    def pack(self, fn: Callable[[dict], None]) -> None:
        edit_json(self.root / LINUX_PACK, fn)


D06 = "## D06 — "
RECALL_D01 = (
    "Nothing to recall yet, so take the entry check instead. Without notes, write one line each: What is the difference "
    "between authorization and scope? What makes a lab isolated rather than merely separate? Name three facts you would "
    "record to prove that a system returned to a known state."
)
ACCELERATOR = (
    "**Accelerator.** An experienced administrator can take the Day 1 diagnostic first and treat days 2 and 3 as review; "
    "the Saturday assessment still happens, unchanged.\n\n"
)


def swap_d01_headings(t: Tree) -> None:
    t.sub(1, "### Recall (15 min)", "### PLACEHOLDER")
    t.sub(1, "### Theory (45 min)", "### Recall (15 min)")
    t.sub(1, "### PLACEHOLDER", "### Theory (45 min)")


def swap_seed_weekday_blocks(t: Tree) -> None:
    def fn(seed: dict) -> None:
        b = seed["weekday_blocks"]
        b[0], b[1] = b[1], b[0]

    edit_json(t.root / SEED, fn)


def add_untaught_competency(t: Tree) -> None:
    t.pack(lambda p: p["competencies"].append({"id": "linux.untaught", "name": "Untaught", "difficulty_band": {"min": 1, "max": 5}, "prerequisites": []}))


def taught_before_prerequisite(t: Tree) -> None:
    t.pack(lambda p: p["competencies"][0].update({"prerequisites": ["linux.blind-troubleshooting"]}))


# (name, mutation, expected substrings); every case expects exit 1
CASES: list[tuple[str, Callable[[Tree], None], list[str]]] = [
    # -- parse_week --------------------------------------------------------------------------
    ("hidden_zero_width_space", lambda t: t.sub(1, "Build the laboratory", "Build the\u200b laboratory"), ["week-01.md:3: hidden or control character U+200B"]),
    ("second_week_heading", lambda t: t.sub(1, "## D02 — ", "# Week 1 — Linux, Python and secure lab engineering\n\n## D02 — "), ["more than one week heading"]),
    ("day_before_week_heading", lambda t: t.sub(1, "# Week 1 — ", "## D01 — Too early\n\n# Week 1 — "), ["week-01.md:1: a day before the week heading"]),
    ("preamble_field_twice", lambda t: t.sub(1, "**Accelerator.**", "**Accelerator.** Once.\n\n**Accelerator.**"), ["'Accelerator' appears twice before the first day"]),
    ("section_twice", lambda t: t.sub(1, "### Watch for\n", "### Watch for\nFirst copy.\n\n### Watch for\n"), ["D01: section 'Watch for' appears twice"]),
    ("file_without_week_heading", lambda t: write_text(t.root / WEEKS / "week-00.md", "Notes without any heading.\n"), ["week-00.md: no '# Week N — title' heading"]),
    # -- check_week: week level ------------------------------------------------------------------
    ("preamble_paragraph_missing", lambda t: t.sub(1, ACCELERATOR, ""), ["week-01.md: missing the '**Accelerator.**' paragraph before the first day"]),
    ("week_number_outside_range", lambda t: t.sub(1, "# Week 1 — ", "# Week 13 — "), ["week-01.md: week number 13 is outside 1..12"]),
    ("file_name_mismatch", lambda t: t.week(1).rename(t.week(1).with_name("week-1.md")), ["week-1.md: holds week 1; the file must be named week-01.md"]),
    ("week_title_differs", lambda t: t.sub(1, "# Week 1 — Linux, Python and secure lab engineering", "# Week 1 — Linux and Python"), ["week title 'Linux and Python' differs from Appendix A"]),
    ("day_ids_differ", lambda t: t.sub(1, "## D02 — ", "## D08 — "), ["week-01.md: days are ['D01', 'D08', 'D03', 'D04', 'D05', 'D06']; expected"]),
    ("seed_block_lists_differ", swap_seed_weekday_blocks, ["seed: the seed's canonical block lists differ from the section names this script expects"]),
    # -- check_week: sections ------------------------------------------------------------------------
    ("section_missing", lambda t: t.sub(1, "### Theory (45 min)\n", ""), ["week-01.md D01: missing section 'Theory (45 min)'"]),
    ("section_unknown", lambda t: t.sub(1, "### Watch for", "### Watch out"), ["week-01.md D01: unknown section(s) ['Watch out']"]),
    ("sections_out_of_order", swap_d01_headings, ["week-01.md D01: sections are out of order"]),
    ("section_too_short", lambda t: t.sub(1, RECALL_D01, "Too short to count."), ["week-01.md D01: section 'Recall (15 min)' has 4 words (minimum 25)"]),
    # -- check_week: hand-in ---------------------------------------------------------------------------
    ("hand_in_missing", lambda t: t.sub(1, "**Hand in.** Lab design", "Hand in: Lab design"), ["week-01.md D01: missing '**Hand in.**' line"]),
    ("hand_in_prefix", lambda t: t.sub(1, "**Hand in.** Lab design + baseline — ", "**Hand in.** Lab design and baseline — "), ["week-01.md D01: hand-in must start with the Appendix A artifact 'Lab design + baseline — '"]),
    ("hand_in_too_long", lambda t: t.sub(1, "Lab design + baseline — a dated", "Lab design + baseline — " + "x" * 500 + " a dated"), ["week-01.md D01: hand-in is ", "characters (maximum 500)"]),
    # -- check_week: objectives ------------------------------------------------------------------------
    ("objective_lines_count", lambda t: t.drop_line(1, "3. Establish diagnostic baseline"), ["week-01.md D01: 2 objective lines (exactly 3 are required)"]),
    ("objective_line_format", lambda t: t.sub(1, "stop condition [lab.authorization]", "stop condition"), ["week-01.md D01: objective line 1 is not 'N. text [competency, ...]'"]),
    ("objective_numbering", lambda t: t.sub(1, "2. Map host/guest", "4. Map host/guest"), ["week-01.md D01: objective line 2 is numbered 4"]),
    ("objective_appendix_wording", lambda t: t.sub(1, "1. Define lab authorization and boundaries — ", "1. Define lab authorisation and boundaries — "), ["week-01.md D01: objective 1 must start with the Appendix A wording 'Define lab authorization and boundaries — '"]),
    ("objective_too_long", lambda t: t.sub(1, "stop condition [lab.authorization]", "stop condition" + " y" * 250 + " [lab.authorization]"), ["week-01.md D01: objective 1 is ", "characters (maximum 500)"]),
    ("objective_more_than_three_competencies", lambda t: t.sub(1, "[lab.authorization, linux.filesystem]", "[lab.authorization, linux.filesystem, linux.processes-services, python.secure-parsing]"), ["week-01.md D01: objective 3: at most 3 distinct competencies"]),
    ("objective_duplicate_competency", lambda t: t.sub(1, "stop condition [lab.authorization]", "stop condition [lab.authorization, lab.authorization]"), ["week-01.md D01: objective 1: at most 3 distinct competencies"]),
    ("objective_unknown_competency", lambda t: t.sub(1, "stop condition [lab.authorization]", "stop condition [lab.no-such-competency]"), ["week-01.md D01: objective 1: unknown competency 'lab.no-such-competency'"]),
    # -- check_week: resources -------------------------------------------------------------------------
    ("resource_line_prefix", lambda t: t.sub(1, "- https://csrc.nist.gov/pubs/sp/800/115/final", "* https://csrc.nist.gov/pubs/sp/800/115/final"), ["week-01.md D01: resource line does not start with '- '"]),
    ("resource_line_format", lambda t: t.sub(1, "| External | SP 800-115", "| Externally | SP 800-115"), ["week-01.md D01: resource line needs 'ref | External|Internal | version | licence note'"]),
    ("resource_version_too_long", lambda t: t.sub(1, "SP 800-115 (September 2008)", "v" * 129), ["week-01.md D01: resource version is 129 characters (maximum 128)"]),
    ("resource_note_too_long", lambda t: t.sub(1, "NIST publication, freely available", "n" * 1001), ["week-01.md D01: resource licence note is 1001 characters (maximum 1000)"]),
    ("external_resource_not_https", lambda t: t.sub(1, "- https://csrc.nist.gov/pubs/sp/800/115/final", "- http://csrc.nist.gov/pubs/sp/800/115/final"), ["week-01.md D01: external resource must be an https URL without spaces"]),
    ("external_resource_without_version", lambda t: t.sub(1, "| External | SP 800-115 (September 2008) |", "| External |  |"), ["week-01.md D01: external resource needs a version"]),
    ("day_without_external_resource", lambda t: t.sub(1, "| External |", "| Internal |", after=D06), ["week-01.md D06: at least one external resource is required"]),
    # -- assessment ------------------------------------------------------------------------------------
    ("assessment_field_missing", lambda t: t.sub(1, "- Kind: BLIND_PRACTICAL\n", ""), ["week-01.md D06: assessment: missing '- Kind:'"]),
    ("assessment_task_statement_missing", lambda t: t.sub(1, "Task statement:", "Task:"), ["week-01.md D06: assessment: missing 'Task statement:'"]),
    ("assessment_task_statement_empty", lambda t: t.cut_between(1, "Task statement:\n", "### Resources", after=D06), ["week-01.md D06: assessment: empty task statement"]),
    ("assessment_task_statement_too_long", lambda t: t.sub(1, "Task statement:\n", "Task statement:\n" + "z" * 20001 + "\n"), ["week-01.md D06: assessment: task statement is ", "characters (maximum 20000)"]),
    ("assessment_code_pattern", lambda t: t.sub(1, "- Code: W01-BLIND", "- Code: w01-blind"), ["week-01.md D06: assessment: code 'w01-blind' does not match"]),
    ("assessment_kind", lambda t: t.sub(1, "- Kind: BLIND_PRACTICAL", "- Kind: ORAL_EXAM"), ["week-01.md D06: assessment: kind 'ORAL_EXAM' is not one of"]),
    ("assessment_unknown_competency", lambda t: t.sub(1, "- Competency: linux.blind-troubleshooting", "- Competency: linux.no-such"), ["week-01.md D06: assessment: unknown competency 'linux.no-such'"]),
    ("assessment_competency_not_linked", lambda t: t.sub(1, "- Competency: linux.blind-troubleshooting", "- Competency: linux.hardening-baseline"), ["assessment: competency 'linux.hardening-baseline' is not linked from any objective of its day"]),
    ("assessment_difficulty_not_integer", lambda t: t.sub(1, "- Difficulty: 3", "- Difficulty: three"), ["week-01.md D06: assessment: difficulty 'three' must be an integer 1..5"]),
    ("assessment_difficulty_out_of_scale", lambda t: t.sub(1, "- Difficulty: 3", "- Difficulty: 6"), ["week-01.md D06: assessment: difficulty '6' must be an integer 1..5"]),
    ("assessment_difficulty_outside_band", lambda t: t.sub(1, "- Difficulty: 3", "- Difficulty: 2"), ["assessment: difficulty 2 is outside the difficulty band 3..5 of 'linux.blind-troubleshooting'"]),
    ("assessment_title_too_long", lambda t: t.sub(1, "- Title: Week 1 blind practical", "- Title: " + "t" * 201 + " Week 1 blind practical"), ["week-01.md D06: assessment: title longer than 200 characters"]),
    # -- main: programme-wide ----------------------------------------------------------------------------
    ("week_file_missing", lambda t: t.week(12).unlink(), ["weeks: weeks present: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]; all 12 are required"]),
    ("duplicate_assessment_code", lambda t: t.sub(2, "- Code: W02-BLIND", "- Code: W01-BLIND"), ["assessments: duplicate assessment codes: ['W01-BLIND']"]),
    ("competency_targeted_too_rarely", add_untaught_competency, ["coverage: competency 'linux.untaught' is targeted by 0 objective(s); at least 2 are required"]),
    ("taught_before_prerequisite", taught_before_prerequisite, ["prerequisites: 'lab.authorization' is first taught at day index 1 before its prerequisite 'linux.blind-troubleshooting'"]),
    # -- main: generated files current --------------------------------------------------------------------
    ("stale_manifest", lambda t: t.append(MANIFEST, "\n"), [f"FAIL  {MANIFEST} is out of date; run: python3 -I scripts/build_curriculum.py --write"]),
    ("stale_sheets", lambda t: t.append(SHEETS, "\n"), [f"FAIL  {SHEETS} is out of date"]),
    ("stale_assessment_payload", lambda t: t.append(f"{ASSESSMENTS}/W01-BLIND.json", "\n"), [f"FAIL  {ASSESSMENTS}/W01-BLIND.json is out of date"]),
    ("stale_coverage", lambda t: t.append(COVERAGE, "\nhand edit\n"), [f"FAIL  {COVERAGE} is out of date"]),
    ("missing_coverage", lambda t: (t.root / COVERAGE).unlink(), [f"FAIL  {COVERAGE} is out of date"]),
    ("missing_assessment_payload", lambda t: (t.root / ASSESSMENTS / "W05-BLIND.json").unlink(), [f"FAIL  {ASSESSMENTS}/W05-BLIND.json is out of date"]),
    ("orphan_assessment_payload", lambda t: write_text(t.root / ASSESSMENTS / "W99-EXTRA.json", "{}\n"), [f"FAIL  {ASSESSMENTS}/W99-EXTRA.json is out of date"]),
    ("source_edited_without_regenerating", lambda t: t.sub(1, "Keeping evidence inside the machine you are testing.", "Keeping the evidence inside the machine under test."), [f"FAIL  {SHEETS} is out of date"]),
]


class RealTree(GateTestCase):
    def test_positive_control_real_tree(self) -> None:
        start = time.monotonic()
        proc = run_gate(REPO / "scripts" / SCRIPT, cwd=REPO)
        elapsed = time.monotonic() - start
        self.assertExit(proc, 0)
        out = output_of(proc)
        self.assertIn("ok    12 week file(s) valid: weeks [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]; 72 days; 216 objectives; 12 assessment(s)", out)
        self.assertIn("ok    generated files are current", out)
        self.assertLess(elapsed, 30.0, "build_curriculum.py check mode took unexpectedly long")


class Mutations(GateTestCase):
    def setUp(self) -> None:
        super().setUp()
        copy_into(self.tmp, INPUTS)
        self.script = stage_script(self.tmp, SCRIPT)
        self.tree = Tree(self.tmp)

    def run_copy(self, *args: str):
        return run_gate(self.script, *args, cwd=self.tmp)

    def run_case(self, mutate: Callable[[Tree], None], needles: list[str]) -> None:
        mutate(self.tree)
        proc = self.run_copy()
        self.assertExit(proc, 1)
        out = output_of(proc)
        for needle in needles:
            self.assertTrue(needle in out, f"{needle!r} missing; output tail:\n{out[-3000:]}")

    def test_copy_control_passes(self) -> None:
        proc = self.run_copy()
        self.assertExit(proc, 0)
        self.assertIn("ok    generated files are current", output_of(proc))

    def test_missing_seed_is_environment_error(self) -> None:
        (self.tmp / SEED).unlink()
        self.assertRule(self.run_copy(), "error: seed manifest or competency packs not found", code=2)

    def test_missing_packs_is_environment_error(self) -> None:
        shutil.rmtree(self.tmp / PACKS)
        self.assertRule(self.run_copy(), "error: seed manifest or competency packs not found", code=2)

    def test_allow_partial_accepts_missing_week_and_writes_nothing(self) -> None:
        self.tree.week(12).unlink()
        before = _tree_digest(self.tmp)
        proc = self.run_copy("--allow-partial")
        self.assertExit(proc, 0)
        self.assertIn("partial run:", output_of(proc))
        self.assertEqual(before, _tree_digest(self.tmp))

    def test_allow_partial_still_rejects_a_broken_week(self) -> None:
        self.tree.week(12).unlink()
        self.tree.sub(1, "- Kind: BLIND_PRACTICAL", "- Kind: ORAL_EXAM")
        self.assertRule(self.run_copy("--allow-partial"), "assessment: kind 'ORAL_EXAM' is not one of")

    def test_write_regenerates_the_copy(self) -> None:
        # Writing mode on the temporary copy only: stale sheets and an orphan payload are repaired,
        # the regenerated files equal the repository's, and the check passes afterwards.
        self.tree.append(SHEETS, "\n")
        orphan = self.tmp / ASSESSMENTS / "W99-EXTRA.json"
        write_text(orphan, "{}\n")
        proc = self.run_copy("--write")
        self.assertExit(proc, 0)
        self.assertIn("wrote 2 file(s)", output_of(proc))
        self.assertFalse(orphan.exists())
        self.assertEqual((self.tmp / SHEETS).read_bytes(), (REPO / SHEETS).read_bytes())
        self.assertExit(self.run_copy(), 0)


def _tree_digest(root: Path) -> dict[str, tuple[int, int]]:
    out = {}
    for dirpath, _dirs, files in os.walk(root):
        for f in files:
            p = Path(dirpath) / f
            st = p.stat()
            out[str(p.relative_to(root))] = (st.st_size, st.st_mtime_ns)
    return out


def _make(mutate: Callable[[Tree], None], needles: list[str]) -> Callable[[Mutations], None]:
    def test(self: Mutations) -> None:
        self.run_case(mutate, needles)

    return test


for _name, _mutate, _needles in CASES:
    _test = _make(_mutate, _needles)
    _test.__name__ = f"test_{_name}"
    _test.__doc__ = f"expects {_needles[0]!r}"
    setattr(Mutations, _test.__name__, _test)


class Sanity(unittest.TestCase):
    def test_case_names_unique(self) -> None:
        names = [c[0] for c in CASES]
        self.assertEqual(len(names), len(set(names)))

    def test_repository_inputs_present(self) -> None:
        missing = [k for k, v in digest_inputs(INPUTS).items() if v == "<missing>"]
        self.assertEqual(missing, [])


if __name__ == "__main__":
    unittest.main()
