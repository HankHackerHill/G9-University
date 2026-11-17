"""Black-box tests of scripts/check_backlog.py (plan step DOC-1).

The positive control runs the gate on the repository as it is. The other tests of `BacklogGate` copy the
five inputs (defect register, RTM, improvement backlog, plan, backlog map) and the ADR files into a
temporary tree with the same layout, edit the copies, and run the gate with `--root <tmp>` through a
subprocess (`python3 -I`); the repository inputs are never written. A negative case builds the rows,
plan step and map entries it needs inside the copy (by cloning a real row of the same table and
changing its id and status), so it never depends on what the map holds today and can never turn into
a skip. Each negative case asserts exit status 1 and the name of the rule that must fail.
`MinimalTree` runs the gate on a small synthetic tree in which every open row is mapped, to test the
done condition of DOC-1 ("0 orphans") and the will-not-do ADR path. Run from the repository root:

    python3 -I -m unittest discover -s scripts/tests -p 'test_*.py'
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Callable

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "check_backlog.py"
REGISTER = "docs/audit/defect-register.md"
RTM = "docs/requirements/rtm.md"
IMPROVEMENTS = "docs/audit/improvement-backlog.md"
PLAN = "docs/release/path-to-10.md"
MAP = "docs/release/backlog-map.json"
INPUTS = (REGISTER, RTM, IMPROVEMENTS, PLAN, MAP)

# The plan step the tests add to the copy of the plan, and the phrases a covering note quotes.
TEST_STEP = "TST-1"
TEST_STEP_TEXT = "Close the rows that the backlog test adds to the copied documents"
TEST_DONE_TEXT = "The backlog test reads every added row as covered"
COVER_NOTE = f'{TEST_STEP}: "{TEST_STEP_TEXT}"'


def run(*args: str, cwd: Path) -> tuple[int, str]:
    proc = subprocess.run(
        [sys.executable, "-I", str(SCRIPT), *args],
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=120,
        cwd=str(cwd),
        check=False,
    )
    return proc.returncode, proc.stdout + proc.stderr


def raw_cells(line: str) -> list[str]:
    """Cells of a table line as written (an escaped '\\|' stays escaped), so a row can be rewritten."""
    text = line.strip()
    if not (text.startswith("|") and text.endswith("|")):
        raise AssertionError(f"not a table row: {line!r}")
    return [c.strip() for c in re.split(r"(?<!\\)\|", text[1:-1])]


def join_cells(cells: list[str]) -> str:
    return "| " + " | ".join(cells) + " |"


class RealTree(unittest.TestCase):
    def test_positive_control_on_the_repository(self) -> None:
        code, out = run("--root", str(REPO), cwd=REPO)
        self.assertEqual(code, 0, out)
        self.assertIn("backlog: OK", out)
        for doc in (REGISTER, RTM, IMPROVEMENTS):
            self.assertRegex(out, re.escape(doc) + r"\s+open\s+\d+\s+mapped\s+\d+\s+unmapped\s+\d+")
        self.assertRegex(out, r"DOC-1 done condition '0 orphans': (met|NOT MET, \d+ orphan)")

    def test_done_condition_agrees_with_the_summary(self) -> None:
        """--require-zero-orphans fails exactly when the summary says the done condition is not met,
        and names one orphan line per unmapped entry."""
        _code, out = run("--root", str(REPO), cwd=REPO)
        m = re.search(r"unmapped\s+(\d+)\s+\(baseline", out)
        self.assertIsNotNone(m, out)
        unmapped = int(m.group(1)) if m else -1
        code, strict = run("--root", str(REPO), "--require-zero-orphans", cwd=REPO)
        orphans = [line for line in strict.splitlines() if "FAIL orphan:" in line]
        self.assertEqual(len(orphans), unmapped, strict)
        if unmapped:
            self.assertEqual(code, 1, strict)
            self.assertIn(f"NOT MET, {unmapped} orphan", out)
        else:
            self.assertEqual(code, 0, strict)
            self.assertIn("DOC-1 done condition '0 orphans': met", out)


class BacklogGate(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory(prefix="g9-backlog-")
        self.root = Path(self._tmp.name)
        for rel in INPUTS:
            dest = self.root / rel
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(REPO / rel, dest)
        (self.root / "docs" / "adr").mkdir(parents=True, exist_ok=True)
        for adr in (REPO / "docs" / "adr").glob("*.md"):
            shutil.copyfile(adr, self.root / "docs" / "adr" / adr.name)
        self.add_plan_step()

    def tearDown(self) -> None:
        self._tmp.cleanup()

    # helpers -----------------------------------------------------------------------------------

    def gate(self, *extra: str) -> tuple[int, str]:
        return run("--root", str(self.root), *extra, cwd=self.root)

    def assert_passes(self, *extra: str) -> str:
        code, out = self.gate(*extra)
        self.assertEqual(code, 0, out)
        self.assertIn("backlog: OK", out)
        return out

    def assert_fails(self, rule: str, *needles: str, extra: tuple[str, ...] = ()) -> str:
        code, out = self.gate(*extra)
        self.assertEqual(code, 1, out)
        self.assertIn(f"FAIL {rule}:", out)
        failing = [line for line in out.splitlines() if f"FAIL {rule}:" in line]
        for needle in needles:
            self.assertTrue(any(needle in line for line in failing), f"{rule} line naming {needle!r}:\n{out}")
        return out

    def text(self, rel: str) -> str:
        return (self.root / rel).read_text(encoding="utf-8")

    def write(self, rel: str, text: str) -> None:
        (self.root / rel).write_text(text, encoding="utf-8")

    def load_map(self) -> dict:
        return json.loads(self.text(MAP))

    def save_map(self, data: dict) -> None:
        self.write(MAP, json.dumps(data, indent=2))

    def update_entry(self, rid: str, **fields: object) -> None:
        data = self.load_map()
        data["entries"][rid].update(fields)
        self.save_map(data)

    def section_range(self, lines: list[str], prefix: str) -> range:
        start = next(i for i, line in enumerate(lines) if line.startswith(prefix))
        end = next((i for i in range(start + 1, len(lines)) if re.match(r"^#{1,2}\s", lines[i])), len(lines))
        return range(start + 1, end)

    def assert_new_id(self, rid: str) -> None:
        for rel in (REGISTER, RTM, IMPROVEMENTS):
            self.assertNotRegex(self.text(rel), rf"(?m)^\| {re.escape(rid)} \|", f"{rid} already in {rel}")
        self.assertNotIn(rid, self.load_map()["entries"])

    def clone_row(self, rel: str, span: range | None, id_pattern: str, edit: Callable[[list[str]], None]) -> None:
        """Insert, after the last row whose id matches `id_pattern` (within `span` if given), a copy of
        that row changed by `edit`. Cloning keeps the cell count of the real table."""
        lines = self.text(rel).split("\n")
        where = span if span is not None else range(len(lines))
        last = max(i for i in where if re.match(rf"^\| {id_pattern} \|", lines[i]))
        cells = raw_cells(lines[last])
        edit(cells)
        lines.insert(last + 1, join_cells(cells))
        self.write(rel, "\n".join(lines))

    def add_plan_step(self) -> None:
        """Add step TST-1 to the first 'ID | Who | Step | Done when' table of section 5 of the copy."""
        lines = self.text(PLAN).split("\n")
        span = self.section_range(lines, "## 5.")
        header = next(
            i for i in span if lines[i].startswith("|") and raw_cells(lines[i]) == ["ID", "Who", "Step", "Done when"]
        )
        lines.insert(header + 2, join_cells([TEST_STEP, "A", TEST_STEP_TEXT, TEST_DONE_TEXT]))
        self.write(PLAN, "\n".join(lines))

    def add_register_row(self, rid: str, status: str, section: str = "## B.", middle: str | None = None) -> None:
        self.assert_new_id(rid)
        lines = self.text(REGISTER).split("\n")
        pattern = r"C\d{2}" if section == "## A." else r"[A-Z]{3}-\d{2}"

        def edit(cells: list[str]) -> None:
            cells[0], cells[-1] = rid, status
            if middle is not None:
                cells[1] = middle

        self.clone_row(REGISTER, self.section_range(lines, section), pattern, edit)

    def add_section_e_row(self, rid: str, status: str) -> None:
        lines = self.text(REGISTER).split("\n")

        def edit(cells: list[str]) -> None:
            cells[0], cells[1] = rid, status

        self.clone_row(REGISTER, self.section_range(lines, "## E."), r"[A-Z]{3}-\d{2}", edit)

    def add_rtm_row(self, rid: str, status: str) -> None:
        self.assert_new_id(rid)

        def edit(cells: list[str]) -> None:
            cells[0], cells[-1] = rid, status

        self.clone_row(RTM, None, r"R-\d{3}", edit)

    def add_improvement_row(self, rid: str) -> None:
        def edit(cells: list[str]) -> None:
            cells[0] = rid

        self.clone_row(IMPROVEMENTS, None, r"IMP-M\d{2}", edit)

    def add_entry(self, rid: str, source: str, status: str | None, mapped: bool, note: str | None = None) -> None:
        """A map entry for `rid`: mapped to TST-1 with a covering quote, or unmapped (the baseline then
        grows by one, so the entry alone does not trip the ratchet)."""
        data = self.load_map()
        data["entries"][rid] = {
            "source": source,
            "status": status,
            "state": "mapped" if mapped else "unmapped",
            "steps": [TEST_STEP] if mapped else [],
            "adr": None,
            "note": note or (COVER_NOTE if mapped else "no plan step covers this row added by the test"),
        }
        if not mapped:
            data["unmapped_baseline"] += 1
        self.save_map(data)

    def set_status(self, rel: str, rid: str, status: str, sections: tuple[str, ...] = ("## A.", "## B.", "## E.")) -> None:
        """Rewrite the status of a row: the last cell, or the second cell in section E of the register."""
        lines = self.text(rel).split("\n")
        spans = [(p, self.section_range(lines, p)) for p in sections] if rel == REGISTER else [("", range(len(lines)))]
        changed = 0
        for prefix, span in spans:
            for i in span:
                if lines[i].startswith(f"| {rid} |"):
                    cells = raw_cells(lines[i])
                    cells[1 if prefix == "## E." else -1] = status
                    lines[i] = join_cells(cells)
                    changed += 1
        self.assertGreater(changed, 0, f"row {rid} not found in {rel}")
        self.write(rel, "\n".join(lines))

    # positive control on the copy --------------------------------------------------------------

    def test_copy_passes(self) -> None:
        self.assert_passes()

    def test_new_open_rows_with_covering_entries_pass(self) -> None:
        self.add_register_row("TST-01", "OPEN")
        self.add_entry("TST-01", REGISTER, "OPEN", mapped=True)
        self.add_rtm_row("R-901", "PROPOSED")
        self.add_entry("R-901", RTM, "PROPOSED", mapped=True)
        self.add_improvement_row("IMP-M91")
        self.add_entry("IMP-M91", IMPROVEMENTS, None, mapped=False)
        self.assert_passes()

    def test_new_closed_row_needs_no_entry(self) -> None:
        self.add_register_row("TST-02", "IMPLEMENTED")
        self.add_rtm_row("R-902", "IMPLEMENTED")
        self.assert_passes()

    # missing-entry: a new open row -------------------------------------------------------------

    def test_new_open_register_row_without_entry_fails(self) -> None:
        self.add_register_row("TST-03", "OPEN")
        self.assert_fails("missing-entry", "TST-03")

    def test_new_open_section_a_row_without_entry_fails(self) -> None:
        self.add_register_row("C98", "PARTIAL", section="## A.")
        self.assert_fails("missing-entry", "C98")

    def test_new_open_rtm_row_without_entry_fails(self) -> None:
        self.add_rtm_row("R-903", "PROPOSED")
        self.assert_fails("missing-entry", "R-903")

    def test_new_improvement_row_without_entry_fails(self) -> None:
        self.add_improvement_row("IMP-M92")
        self.assert_fails("missing-entry", "IMP-M92")

    def test_rtm_mixed_cell_is_open(self) -> None:
        self.add_rtm_row("R-904", "IMPLEMENTED (records), PARTIAL (one part added by the test)")
        self.assert_fails("missing-entry", "R-904")

    def test_rtm_stage_deferral_is_open(self) -> None:
        self.add_rtm_row("R-905", "IMPLEMENTED (ledger), UI flow Stage 05")
        self.assert_fails("missing-entry", "R-905")

    def test_escaped_pipe_does_not_split_a_cell(self) -> None:
        self.add_register_row("TST-04", "PARTIAL", middle=r"a cell with an escaped \| pipe")
        out = self.assert_fails("missing-entry", "TST-04")
        self.assertNotIn("FAIL bad-row", out)

    # unknown-step ------------------------------------------------------------------------------

    def test_unknown_step_id_fails(self) -> None:
        self.add_register_row("TST-05", "OPEN")
        self.add_entry("TST-05", REGISTER, "OPEN", mapped=True)
        self.update_entry("TST-05", steps=[TEST_STEP, "NOPE-1"])
        self.assert_fails("unknown-step", "TST-05", "NOPE-1")

    def test_step_removed_from_the_plan_fails(self) -> None:
        self.add_rtm_row("R-906", "PARTIAL")
        self.add_entry("R-906", RTM, "PARTIAL", mapped=True)
        lines = [line for line in self.text(PLAN).split("\n") if not line.startswith(f"| {TEST_STEP} |")]
        self.write(PLAN, "\n".join(lines))
        self.assert_fails("unknown-step", "R-906", TEST_STEP)

    def test_register_ids_are_not_plan_steps(self) -> None:
        """SEC-nn and LRN-nn are register rows; SECUR-n and LEARN-n are plan steps."""
        self.add_register_row("TST-06", "OPEN")
        self.add_entry("TST-06", REGISTER, "OPEN", mapped=True)
        self.update_entry("TST-06", steps=[TEST_STEP, "SEC-19"])
        self.assert_fails("unknown-step", "TST-06", "SEC-19")

    # stale-entry -------------------------------------------------------------------------------

    def test_closed_register_row_makes_its_entry_stale(self) -> None:
        self.add_register_row("TST-07", "PARTIAL")
        self.add_entry("TST-07", REGISTER, "PARTIAL", mapped=True)
        self.assert_passes()
        self.set_status(REGISTER, "TST-07", "IMPLEMENTED")
        self.assert_fails("stale-entry", "TST-07")

    def test_section_e_overrides_section_b(self) -> None:
        self.add_register_row("TST-08", "PARTIAL")
        self.add_entry("TST-08", REGISTER, "PARTIAL", mapped=True)
        self.assert_passes()
        self.add_section_e_row("TST-08", "IMPLEMENTED")
        out = self.assert_fails("stale-entry", "TST-08")
        self.assertIn("section E is used", out)

    def test_closed_rtm_row_makes_its_entry_stale(self) -> None:
        self.add_rtm_row("R-907", "PARTIAL")
        self.add_entry("R-907", RTM, "PARTIAL", mapped=True)
        self.assert_passes()
        self.set_status(RTM, "R-907", "IMPLEMENTED")
        self.assert_fails("stale-entry", "R-907")

    def test_removed_row_makes_its_entry_stale(self) -> None:
        self.add_improvement_row("IMP-M93")
        self.add_entry("IMP-M93", IMPROVEMENTS, None, mapped=True)
        self.assert_passes()
        lines = [line for line in self.text(IMPROVEMENTS).split("\n") if not line.startswith("| IMP-M93 |")]
        self.write(IMPROVEMENTS, "\n".join(lines))
        self.assert_fails("stale-entry", "IMP-M93", "no longer exists")

    # ratchet: unmapped above the baseline ------------------------------------------------------

    def test_lower_baseline_than_unmapped_count_fails(self) -> None:
        self.add_register_row("TST-09", "OPEN")
        self.add_entry("TST-09", REGISTER, "OPEN", mapped=False)
        data = self.load_map()
        data["unmapped_baseline"] -= 1
        self.save_map(data)
        self.assert_fails("ratchet", "exceed unmapped_baseline")

    def test_one_more_unmapped_entry_fails(self) -> None:
        self.add_rtm_row("R-908", "PROPOSED")
        self.add_entry("R-908", RTM, "PROPOSED", mapped=True)
        self.assert_passes()
        self.update_entry("R-908", state="unmapped", steps=[], note="unmapped by the test")
        self.assert_fails("ratchet", "exceed unmapped_baseline")

    def test_unmapped_below_baseline_is_a_note(self) -> None:
        data = self.load_map()
        data["unmapped_baseline"] += 1
        self.save_map(data)
        out = self.assert_passes()
        self.assertIn("below unmapped_baseline", out)

    # orphans: the done condition of DOC-1 ------------------------------------------------------

    def test_require_zero_orphans_names_each_unmapped_entry(self) -> None:
        self.add_register_row("TST-10", "OPEN")
        self.add_entry("TST-10", REGISTER, "OPEN", mapped=False)
        self.assert_passes()
        self.assert_fails("orphan", "TST-10", extra=("--require-zero-orphans",))

    # entry rules -------------------------------------------------------------------------------

    def test_mapped_entry_without_step_or_adr_fails(self) -> None:
        self.add_register_row("TST-11", "OPEN")
        self.add_entry("TST-11", REGISTER, "OPEN", mapped=True)
        self.update_entry("TST-11", steps=[], adr=None)
        self.assert_fails("no-cover", "TST-11")

    def test_mapped_entry_must_quote_its_step(self) -> None:
        self.add_improvement_row("IMP-M94")
        self.add_entry("IMP-M94", IMPROVEMENTS, None, mapped=True, note='covered by "a phrase that no plan step contains"')
        self.assert_fails("quote-missing", "IMP-M94", TEST_STEP)

    def test_a_one_word_quote_is_not_coverage(self) -> None:
        """The reviewer's case: an unrelated step and the quote "A" (it matches the Who cell)."""
        self.add_register_row("TST-12", "OPEN")
        self.add_entry("TST-12", REGISTER, "OPEN", mapped=True, note='covered by "A"')
        self.update_entry("TST-12", steps=["DOC-1"])
        self.assert_fails("quote-missing", "TST-12", "DOC-1")

    def test_a_short_phrase_of_the_step_is_not_coverage(self) -> None:
        self.add_register_row("TST-13", "OPEN")
        self.add_entry("TST-13", REGISTER, "OPEN", mapped=True, note='covered by "the rows"')
        self.assert_fails("quote-missing", "TST-13", TEST_STEP)

    def test_a_quote_across_the_id_and_who_cells_is_not_coverage(self) -> None:
        self.add_register_row("TST-14", "OPEN")
        self.add_entry("TST-14", REGISTER, "OPEN", mapped=True, note=f'covered by "{TEST_STEP} | A | Close the rows"')
        self.assert_fails("quote-missing", "TST-14", TEST_STEP)

    def test_a_quote_of_the_done_when_cell_is_coverage(self) -> None:
        self.add_register_row("TST-15", "OPEN")
        self.add_entry("TST-15", REGISTER, "OPEN", mapped=True, note=f'{TEST_STEP}: "{TEST_DONE_TEXT}"')
        self.assert_passes()

    def test_every_named_step_needs_its_own_quote(self) -> None:
        self.add_register_row("TST-16", "OPEN")
        self.add_entry("TST-16", REGISTER, "OPEN", mapped=True)
        self.update_entry("TST-16", steps=[TEST_STEP, "DOC-1"])
        out = self.assert_fails("quote-missing", "TST-16", "DOC-1")
        self.assertNotIn(f"names {TEST_STEP},", out)

    def test_adr_needs_its_own_quote(self) -> None:
        adr = sorted((self.root / "docs" / "adr").glob("ADR-0*.md"))[0].name[:8]
        self.add_register_row("TST-17", "OPEN")
        self.add_entry("TST-17", REGISTER, "OPEN", mapped=True)
        self.update_entry("TST-17", adr=adr)
        self.assert_fails("quote-missing", "TST-17", adr)

    def test_a_line_citation_in_a_note_fails(self) -> None:
        self.add_register_row("TST-18", "OPEN")
        self.add_entry("TST-18", REGISTER, "OPEN", mapped=False, note="missing: see crates/g9-storage/src/backup.rs:342")
        self.assert_fails("line-citation", "TST-18", "backup.rs:342")

    def test_unmapped_entry_with_steps_fails(self) -> None:
        self.add_register_row("TST-19", "OPEN")
        self.add_entry("TST-19", REGISTER, "OPEN", mapped=False)
        self.update_entry("TST-19", steps=[TEST_STEP])
        self.assert_fails("bad-unmapped", "TST-19")

    def test_unknown_adr_fails(self) -> None:
        self.add_register_row("TST-20", "OPEN")
        self.add_entry("TST-20", REGISTER, "OPEN", mapped=True)
        self.update_entry("TST-20", adr="ADR-9999")
        self.assert_fails("unknown-adr", "TST-20", "ADR-9999")

    def test_status_drift_fails(self) -> None:
        self.add_register_row("TST-21", "PARTIAL")
        self.add_entry("TST-21", REGISTER, "PARTIAL", mapped=True)
        self.assert_passes()
        self.set_status(REGISTER, "TST-21", "OPEN")
        self.assert_fails("status-drift", "TST-21")

    def test_wrong_source_fails(self) -> None:
        self.add_register_row("TST-22", "OPEN")
        self.add_entry("TST-22", RTM, "OPEN", mapped=False)
        self.assert_fails("wrong-source", "TST-22")

    def test_rules_version_mismatch_fails(self) -> None:
        data = self.load_map()
        data["parsing_rules_version"] = "0.0.0"
        self.save_map(data)
        self.assert_fails("rules-version", "0.0.0")

    # document rules ----------------------------------------------------------------------------

    def test_unknown_status_word_fails(self) -> None:
        self.add_register_row("TST-23", "DONE")
        self.assert_fails("unknown-status", "TST-23", "DONE")

    def test_improvement_status_without_backlog_row_fails(self) -> None:
        self.add_register_row("TST-24", "IMPROVEMENT")
        self.assert_fails("orphan-improvement", "TST-24")

    def test_row_open_in_two_documents_fails(self) -> None:
        self.add_register_row("TST-25", "OPEN")
        self.add_entry("TST-25", REGISTER, "OPEN", mapped=True)

        def edit(cells: list[str]) -> None:
            cells[0] = "TST-25"

        self.clone_row(IMPROVEMENTS, None, r"[A-Z]{3}-\d{2}", edit)
        self.assert_fails("duplicate-row", "TST-25")

    # inputs ------------------------------------------------------------------------------------

    def test_missing_input_exits_2(self) -> None:
        (self.root / MAP).unlink()
        code, out = self.gate()
        self.assertEqual(code, 2, out)
        self.assertIn("ERROR no map file", out)

    def test_invalid_json_exits_2(self) -> None:
        self.write(MAP, "{ not json")
        code, out = self.gate()
        self.assertEqual(code, 2, out)
        self.assertIn("not valid JSON", out)

    def test_explicit_input_overrides(self) -> None:
        other = self.root / "elsewhere"
        other.mkdir()
        shutil.copyfile(self.root / MAP, other / "map.json")
        self.add_register_row("TST-26", "OPEN")
        code, out = run(
            "--register", str(self.root / REGISTER), "--rtm", str(self.root / RTM),
            "--improvements", str(self.root / IMPROVEMENTS), "--plan", str(self.root / PLAN),
            "--map", str(other / "map.json"), "--adr-dir", str(self.root / "docs" / "adr"),
            cwd=self.root,
        )
        self.assertEqual(code, 1, out)
        self.assertIn("FAIL missing-entry", out)
        self.assertIn("TST-26", out)


MINIMAL_REGISTER = """# Defect register

## A. Corrections

| ID | Defect | Status |
|---|---|---|
| C01 | a correction | IMPLEMENTED |

## B. Auditor findings

| ID | Sev | Defect | Status |
|---|---|---|---|
| TEC-01 | LOW | a defect the plan fixes | OPEN |
| TEC-02 | LOW | a defect fixed already | OPEN |
| TEC-03 | LOW | a defect kept as an improvement | IMPROVEMENT |

## E. Status evidence

| ID | Status | Evidence | Remaining gap |
|---|---|---|---|
| TEC-02 | IMPLEMENTED | a test | none |
"""

MINIMAL_RTM = """# RTM

| Req | Requirement | Status |
|---|---|---|
| R-001 | a requirement the plan finishes | IMPLEMENTED (rules), PARTIAL (wiring) |
| R-002 | a requirement met | IMPLEMENTED |

| Gate | Requirement(s) | Status |
|---|---|---|
| A01 | R-002 | PASS |
"""

MINIMAL_IMPROVEMENTS = """# Improvement backlog

| ID | Priority | Improvement |
|---|---|---|
| IMP-M01 | LOW | an improvement nobody will build |
| TEC-03 | LOW | the improvement of register row TEC-03 |
"""

MINIMAL_PLAN = """# Plan

## 5. Plans per area

| ID | Who | Step | Done when |
|---|---|---|---|
| FIX-1 | A | Fix the defect of register row TEC-01 with a fail-first test | The register row reads IMPLEMENTED |
| REQ-1 | A | Wire requirement R-001 through every command | The RTM row reads IMPLEMENTED |
| IMP-1 | A | Build the improvement of register row TEC-03 | Built and tested |

## 6. After
"""

MINIMAL_ADR = """# ADR-0001: Will not do IMP-M01

Decision: we will not build the improvement IMP-M01, because nobody uses it.
"""


def minimal_map(unmapped: tuple[str, ...] = ()) -> dict:
    entries = {
        "TEC-01": {"source": REGISTER, "status": "OPEN", "steps": ["FIX-1"], "adr": None,
                   "note": 'FIX-1: "Fix the defect of register row TEC-01"'},
        "R-001": {"source": RTM, "status": "IMPLEMENTED (rules), PARTIAL (wiring)", "steps": ["REQ-1"], "adr": None,
                  "note": 'REQ-1: "Wire requirement R-001 through every command"'},
        "IMP-M01": {"source": IMPROVEMENTS, "status": None, "steps": [], "adr": "ADR-0001",
                    "note": 'ADR-0001: "we will not build the improvement IMP-M01"'},
        "TEC-03": {"source": IMPROVEMENTS, "status": None, "steps": ["IMP-1"], "adr": None,
                   "note": 'IMP-1: "Build the improvement of register row TEC-03"'},
    }
    for rid, e in entries.items():
        if rid in unmapped:
            e.update(steps=[], adr=None, note="no step and no ADR in the synthetic tree")
        e["state"] = "unmapped" if rid in unmapped else "mapped"
    return {
        "parsing_rules_version": "1.1.0",
        "unmapped_baseline": len(unmapped),
        "entries": {rid: {k: e[k] for k in ("source", "status", "state", "steps", "adr", "note")} for rid, e in entries.items()},
    }


class MinimalTree(unittest.TestCase):
    """A synthetic tree in which every open row has a step or a will-not-do ADR (0 orphans)."""

    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory(prefix="g9-backlog-min-")
        self.root = Path(self._tmp.name)
        files = {
            REGISTER: MINIMAL_REGISTER,
            RTM: MINIMAL_RTM,
            IMPROVEMENTS: MINIMAL_IMPROVEMENTS,
            PLAN: MINIMAL_PLAN,
            "docs/adr/ADR-0001-will-not-do-imp-m01.md": MINIMAL_ADR,
        }
        for rel, text in files.items():
            (self.root / rel).parent.mkdir(parents=True, exist_ok=True)
            (self.root / rel).write_text(text, encoding="utf-8")
        self.write_map(minimal_map())

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def write_map(self, data: dict) -> None:
        (self.root / MAP).parent.mkdir(parents=True, exist_ok=True)
        (self.root / MAP).write_text(json.dumps(data, indent=2), encoding="utf-8")

    def test_zero_orphans_meets_the_done_condition(self) -> None:
        code, out = run("--root", str(self.root), "--require-zero-orphans", cwd=self.root)
        self.assertEqual(code, 0, out)
        self.assertIn("DOC-1 done condition '0 orphans': met", out)
        self.assertRegex(out, r"total\s+open\s+4\s+mapped\s+4\s+unmapped\s+0")

    def test_one_orphan_passes_the_ratchet_but_not_the_done_condition(self) -> None:
        self.write_map(minimal_map(unmapped=("R-001",)))
        code, out = run("--root", str(self.root), cwd=self.root)
        self.assertEqual(code, 0, out)
        self.assertIn("NOT MET, 1 orphan", out)
        code, out = run("--root", str(self.root), "--require-zero-orphans", cwd=self.root)
        self.assertEqual(code, 1, out)
        orphans = [line for line in out.splitlines() if "FAIL orphan:" in line]
        self.assertEqual(len(orphans), 1, out)
        self.assertIn("R-001", orphans[0])

    def test_a_closed_row_needs_no_entry_and_a_stale_one_fails(self) -> None:
        data = minimal_map()
        data["entries"]["TEC-02"] = dict(data["entries"]["TEC-01"], status="IMPLEMENTED")
        self.write_map(data)
        code, out = run("--root", str(self.root), cwd=self.root)
        self.assertEqual(code, 1, out)
        self.assertIn("FAIL stale-entry: entry TEC-02", out)


if __name__ == "__main__":
    unittest.main()
