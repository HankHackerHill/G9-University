"""Tests of the deletion guards of scripts/ci_preflight.sh (audit 2026-11-19, P2D-01).

The script removes its export and, when it created it, its build directory. These tests run it with
paths that used to defeat the guard and assert that it refuses (exit status 2) before it exports or
deletes anything, and that a directory that existed before the run is left alone. Nothing here
starts a CI step: every case exits at argument checking. Run from the repository root:

    python3 -I -m unittest discover -s scripts/tests -p 'test_*.py'
"""

from __future__ import annotations

import subprocess
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "ci_preflight.sh"


def run(*args: str) -> tuple[int, str]:
    proc = subprocess.run(["bash", str(SCRIPT), *args], capture_output=True, text=True, timeout=120, check=False)
    return proc.returncode, proc.stdout + proc.stderr


class PreflightGuards(unittest.TestCase):
    def test_a_target_with_dotdot_components_is_refused_and_nothing_outside_work_is_touched(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            (base / "work").mkdir()
            victim = base / "victim"
            victim.mkdir()
            (victim / "keep.txt").write_text("keep", encoding="utf-8")
            code, out = run("--work", str(base / "work"), "--target", str(base / "work" / ".." / "victim"))
            self.assertEqual(code, 2, out)
            self.assertIn("'..' components", out)
            self.assertTrue((victim / "keep.txt").is_file(), "a directory outside --work was touched")

    def test_a_work_directory_with_dotdot_components_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            code, out = run("--work", str(Path(tmp) / "a" / ".." / "b"))
            self.assertEqual(code, 2, out)
            self.assertIn("'..' components", out)

    def test_an_existing_default_build_directory_is_refused_and_kept(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            work = Path(tmp) / "work"
            (work / "target").mkdir(parents=True)
            (work / "target" / "keep2.txt").write_text("keep", encoding="utf-8")
            code, out = run("--work", str(work))
            self.assertEqual(code, 2, out)
            self.assertIn("existing build directory", out)
            self.assertTrue((work / "target" / "keep2.txt").is_file())


if __name__ == "__main__":
    unittest.main()
