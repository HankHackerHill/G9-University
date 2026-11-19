"""Black-box tests of scripts/check_ignored_files.py (audit 2026-11-19, GIT-1).

Each case builds a throw-away git repository in a temporary directory, runs the gate with
`--root <tmp>` through a subprocess (`python3 -I`) and asserts the exit status and the message.
The repository itself is never written. Run from the repository root:

    python3 -I -m unittest discover -s scripts/tests -p 'test_*.py'
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "check_ignored_files.py"


def run(root: Path) -> tuple[int, str]:
    proc = subprocess.run(
        [sys.executable, "-I", str(SCRIPT), "--root", str(root)],
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=60,
        check=False,
    )
    return proc.returncode, proc.stdout + proc.stderr


class IgnoredFiles(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        subprocess.run(["git", "-C", str(self.root), "init", "-q"], check=True)
        (self.root / ".gitignore").write_text("*.log\ntarget\nnode_modules\n", encoding="utf-8")

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def put(self, relative: str, text: str = "x") -> None:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def test_a_fixture_log_that_gitignore_excludes_is_reported_with_its_rule(self) -> None:
        self.put("docs/course/fixtures/d03/journal.log")
        code, out = run(self.root)
        self.assertEqual(code, 1, out)
        self.assertIn("docs/course/fixtures/d03/journal.log", out)
        self.assertIn(".gitignore:1:*.log", out)

    def test_build_output_and_caches_are_allowed(self) -> None:
        self.put("target/debug/a.o")
        self.put("apps/cli/target/x.log")
        self.put("apps/desktop/node_modules/pkg/index.js")
        self.put("scripts/__pycache__/a.cpython-313.pyc")
        code, out = run(self.root)
        self.assertEqual(code, 0, out)
        self.assertIn("ok", out)

    def test_a_tree_with_no_ignored_file_passes(self) -> None:
        self.put("docs/readme.md")
        code, out = run(self.root)
        self.assertEqual(code, 0, out)
        self.assertIn("0 ignored file(s)", out)

    def test_a_log_next_to_a_cache_dir_name_is_still_reported(self) -> None:
        # "target" is a cache only as a directory component, never as part of a file name.
        self.put("docs/target.log")
        code, out = run(self.root)
        self.assertEqual(code, 1, out)

    def test_a_directory_that_is_not_a_git_work_tree_is_an_environment_error(self) -> None:
        with tempfile.TemporaryDirectory() as bare:
            code, out = run(Path(bare))
        self.assertEqual(code, 2, out)

    def test_the_repository_itself_passes(self) -> None:
        code, out = run(REPO)
        self.assertEqual(code, 0, out)


if __name__ == "__main__":
    unittest.main()
