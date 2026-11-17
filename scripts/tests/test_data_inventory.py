"""Black-box tests of scripts/check_data_inventory.py (plan step PRIV-7).

Every test copies the real migrations and the real docs/data-model.md into a temporary directory and
runs the script on the copies through a subprocess (`python3 -I`); the repository inputs are never
modified. Run from the repository root:

    python3 -I -m unittest discover -s scripts/tests -p 'test_*.py'
"""

from __future__ import annotations

import re
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "check_data_inventory.py"
MIGRATIONS = REPO / "crates" / "g9-storage" / "src" / "migrations"
DOC = REPO / "docs" / "data-model.md"
COLUMNS = 9


class DataInventoryGate(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory(prefix="g9-inventory-")
        self.tmp = Path(self._tmp.name)
        self.migrations = self.tmp / "migrations"
        self.migrations.mkdir()
        for sql in sorted(MIGRATIONS.glob("*.sql")):
            shutil.copyfile(sql, self.migrations / sql.name)
        self.doc = self.tmp / "data-model.md"
        shutil.copyfile(DOC, self.doc)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    # helpers -----------------------------------------------------------------------------------

    def run_gate(self, *args: str) -> tuple[int, str]:
        argv = list(args) or ["--migrations", str(self.migrations), "--doc", str(self.doc)]
        proc = subprocess.run(
            [sys.executable, "-I", str(SCRIPT), *argv],
            capture_output=True,
            text=True,
            timeout=60,
            cwd=self.tmp,
        )
        return proc.returncode, proc.stdout + proc.stderr

    def add_migration(self, name: str, sql: str) -> None:
        (self.migrations / name).write_text(sql, encoding="utf-8")

    def doc_lines(self) -> list[str]:
        return self.doc.read_text(encoding="utf-8").split("\n")

    def write_doc(self, lines: list[str]) -> None:
        self.doc.write_text("\n".join(lines), encoding="utf-8")

    def section(self, lines: list[str]) -> range:
        """Line indexes of the section "## Data inventory" (other tables of the document repeat
        table names, so rows are only looked up inside it)."""
        starts = [i for i, line in enumerate(lines) if line.strip() == "## Data inventory"]
        self.assertEqual(len(starts), 1, "the copied document holds one Data inventory section")
        end = next(
            (i for i in range(starts[0] + 1, len(lines)) if re.match(r"^#{1,2}\s", lines[i])), len(lines)
        )
        return range(starts[0] + 1, end)

    def inventory_rows(self, lines: list[str]) -> list[int]:
        """Rows of the inventory table: the table whose header starts with '| table |'."""
        span = self.section(lines)
        header = [i for i in span if lines[i].startswith("| table | data domain |")]
        self.assertEqual(len(header), 1, "the section holds one inventory table")
        rows = []
        for i in range(header[0] + 2, span.stop):
            if not lines[i].startswith("|"):
                break
            rows.append(i)
        return rows

    def row_index(self, lines: list[str], table: str) -> int:
        prefix = f"| `{table}` |"
        hits = [i for i in self.inventory_rows(lines) if lines[i].startswith(prefix)]
        self.assertEqual(len(hits), 1, f"the copied inventory must hold one row for {table}")
        return hits[0]

    def set_cell(self, table: str, column: int, value: str) -> None:
        lines = self.doc_lines()
        i = self.row_index(lines, table)
        cells = lines[i].strip().strip("|").split("|")
        self.assertEqual(len(cells), COLUMNS, "test rows have no '|' inside a cell")
        cells[column] = f" {value} "
        lines[i] = "|" + "|".join(cells) + "|"
        self.write_doc(lines)

    def assert_fails(self, rule: str, *names: str) -> str:
        code, out = self.run_gate()
        self.assertEqual(code, 1, out)
        lines = [line for line in out.splitlines() if f"FAIL {rule}:" in line]
        self.assertTrue(lines, f"no '{rule}' failure in:\n{out}")
        for name in names:
            self.assertTrue(any(name in line for line in lines), f"'{rule}' does not name {name}:\n{out}")
        return out

    # positive controls -------------------------------------------------------------------------

    def test_real_inventory_is_complete(self) -> None:
        code, out = self.run_gate()
        self.assertEqual(code, 0, out)
        self.assertIn("data inventory: OK", out)
        reported = re.search(r"\((\d+) tables in (\d+) migrations", out)
        self.assertIsNotNone(reported, out)
        # Independent counts: the rows of the copied inventory, and the CREATE TABLE lines of the
        # copied migrations. A migration that rebuilds a table (create, copy, DROP TABLE, RENAME TO)
        # changes how many CREATE lines there are without changing how many tables exist, so the
        # line count is compared only while no migration drops or renames a table; the gate's own
        # rules for drops and renames are tested separately.
        created = 0
        rebuilds = False
        for sql in self.migrations.glob("*.sql"):
            text = sql.read_text(encoding="utf-8")
            created += len(re.findall(r"(?im)^\s*CREATE\s+(?:VIRTUAL\s+)?TABLE\b", text))
            rebuilds = rebuilds or bool(re.search(r"(?i)\bDROP\s+TABLE\b|\bRENAME\s+TO\b", text))
        rows = len(self.inventory_rows(self.doc_lines()))
        self.assertEqual(int(reported.group(1)), rows, out)
        self.assertIn(f"{rows} inventory rows", out)
        if not rebuilds:
            self.assertEqual(int(reported.group(1)), created, out)

    def test_root_option_finds_both_inputs(self) -> None:
        root = self.tmp / "root"
        shutil.copytree(self.migrations, root / "crates" / "g9-storage" / "src" / "migrations")
        (root / "docs").mkdir()
        shutil.copyfile(self.doc, root / "docs" / "data-model.md")
        code, out = self.run_gate("--root", str(root))
        self.assertEqual(code, 0, out)
        self.assertIn("data inventory: OK", out)

    def test_create_table_in_a_comment_or_a_string_is_not_a_table(self) -> None:
        self.add_migration(
            "0999_text_only.sql",
            "-- CREATE TABLE probe_in_comment (x TEXT);\n"
            "/* CREATE TABLE probe_in_block (x TEXT); */\n"
            "SELECT 'CREATE TABLE probe_in_string (x TEXT)';\n"
            "CREATE TEMP TABLE probe_temporary (x TEXT);\n",
        )
        code, out = self.run_gate()
        self.assertEqual(code, 0, out)
        self.assertIn("1 temporary not counted", out)

    # missing-row -------------------------------------------------------------------------------

    def test_added_table_without_a_row_fails_and_is_named(self) -> None:
        self.add_migration(
            "0999_probe.sql", "CREATE TABLE IF NOT EXISTS probe_new_table (id TEXT PRIMARY KEY) STRICT;\n"
        )
        out = self.assert_fails("missing-row", "probe_new_table", "0999_probe.sql")
        self.assertNotIn("FAIL orphan-row", out)

    def test_added_virtual_table_without_a_row_fails(self) -> None:
        self.add_migration("0999_fts.sql", "CREATE VIRTUAL TABLE probe_fts USING fts5(body);\n")
        out = self.assert_fails("missing-row", "probe_fts")
        self.assertIn("virtual table `probe_fts`", out)

    def test_quoted_table_names_are_read(self) -> None:
        self.add_migration(
            "0999_quoted.sql",
            'CREATE TABLE "probe quoted" (x TEXT);\n'
            "CREATE TABLE `probe_tick` (x TEXT);\n"
            "CREATE TABLE [probe_bracket] (x TEXT);\n"
            "create table if not exists main.probe_lower (x text);\n",
        )
        self.assert_fails("missing-row", "probe quoted", "probe_tick", "probe_bracket", "probe_lower")

    def test_renamed_table_needs_a_row_under_its_new_name(self) -> None:
        self.add_migration("0999_rename.sql", "ALTER TABLE program_plan RENAME TO program_plans;\n")
        self.assert_fails("missing-row", "program_plans")
        self.assert_fails("orphan-row", "program_plan")

    # orphan-row --------------------------------------------------------------------------------

    def test_row_for_a_missing_table_fails_and_is_named(self) -> None:
        lines = self.doc_lines()
        i = self.row_index(lines, "program_plan")
        lines.insert(i + 1, lines[i].replace("`program_plan`", "`probe_ghost_table`", 1))
        self.write_doc(lines)
        out = self.assert_fails("orphan-row", "probe_ghost_table")
        self.assertNotIn("FAIL missing-row", out)

    def test_row_of_a_dropped_table_fails(self) -> None:
        self.add_migration("0999_drop.sql", "DROP TABLE IF EXISTS program_plan;\n")
        self.assert_fails("orphan-row", "program_plan")

    # empty-cell --------------------------------------------------------------------------------

    def test_empty_cell_fails_and_names_row_and_column(self) -> None:
        self.set_cell("notes", 8, "")
        self.assert_fails("empty-cell", "`notes`", "'retention'")

    def test_placeholder_cell_fails(self) -> None:
        self.set_cell("attachments", 6, "TODO")
        self.assert_fails("empty-cell", "`attachments`", "'export by export.request'")

    def test_empty_table_cell_fails(self) -> None:
        lines = self.doc_lines()
        i = self.row_index(lines, "program_plan")
        lines[i] = lines[i].replace("`program_plan`", "", 1)
        self.write_doc(lines)
        self.assert_fails("empty-cell", "'table'")
        self.assert_fails("missing-row", "program_plan")

    # duplicate-row, bad-row, bad-header, no-section --------------------------------------------

    def test_duplicate_row_fails(self) -> None:
        lines = self.doc_lines()
        i = self.row_index(lines, "notes")
        lines.insert(i + 1, lines[i])
        self.write_doc(lines)
        self.assert_fails("duplicate-row", "`notes`")

    def test_row_with_a_missing_cell_fails(self) -> None:
        lines = self.doc_lines()
        i = self.row_index(lines, "notes")
        cells = lines[i].strip().strip("|").split("|")
        lines[i] = "|" + "|".join(cells[:-1]) + "|"
        self.write_doc(lines)
        self.assert_fails("bad-row", "`notes`", "8 cells")

    def test_changed_header_fails(self) -> None:
        text = self.doc.read_text(encoding="utf-8")
        self.assertEqual(text.count("| in backups | retention |"), 1)
        self.doc.write_text(text.replace("| in backups | retention |", "| in backups |", 1), encoding="utf-8")
        self.assert_fails("bad-header", "exactly one table")

    def test_missing_section_fails(self) -> None:
        text = self.doc.read_text(encoding="utf-8")
        self.doc.write_text(text.replace("## Data inventory", "## Inventory of data", 1), encoding="utf-8")
        self.assert_fails("no-section", "0 sections")

    def test_two_sections_fail(self) -> None:
        text = self.doc.read_text(encoding="utf-8")
        self.doc.write_text(text + "\n## Data inventory\n\nnothing\n", encoding="utf-8")
        self.assert_fails("no-section", "2 sections")

    # inputs ------------------------------------------------------------------------------------

    def test_missing_inputs_exit_2(self) -> None:
        code, out = self.run_gate("--migrations", str(self.tmp / "absent"), "--doc", str(self.doc))
        self.assertEqual(code, 2, out)
        self.assertIn("no *.sql migrations", out)
        code, out = self.run_gate("--migrations", str(self.migrations), "--doc", str(self.tmp / "absent.md"))
        self.assertEqual(code, 2, out)
        self.assertIn("no document", out)


if __name__ == "__main__":
    unittest.main()
