"""Black-box tests of scripts/check_no_docker.py (owner instruction, 2026-11-20).

Each case builds a throw-away git repository, runs the gate with `--root <tmp>` through a subprocess
(`python3 -I`) and asserts the exit status and the message. The engine's name is assembled from
pieces so that this file does not match the rule it tests. Run from the repository root:

    python3 -I -m unittest discover -s scripts/tests -p 'test_*.py'
"""

from __future__ import annotations

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "check_no_docker.py"
E = "dock" + "er"


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


WORKFLOW_OK = (
    "jobs:\n  a:\n    steps:\n"
    "      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7\n"
    "      - uses: dtolnay/rust-toolchain@683b588c487a69d01f6c6ddf40f036ff682cc2c8\n"
    "      - run: cargo test\n"
)


class NoContainerEngine(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name)
        subprocess.run(["git", "-C", str(self.root), "init", "-q"], check=True)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def add(self, relative: str, text: str = "x\n") -> None:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        subprocess.run(["git", "-C", str(self.root), "add", "-f", relative], check=True)

    def test_a_clean_tree_with_allowed_actions_passes(self) -> None:
        self.add(".github/workflows/ci.yml", WORKFLOW_OK)
        self.add("scripts/build.sh", "cargo build\n")
        code, out = run(self.root)
        self.assertEqual(code, 0, out)
        self.assertIn("ok", out)

    def test_a_script_that_runs_the_engine_is_reported_with_its_line(self) -> None:
        self.add("scripts/run.sh", f"set -e\n{E} run --rm image\n")
        code, out = run(self.root)
        self.assertEqual(code, 1, out)
        self.assertIn("scripts/run.sh:2", out)

    def test_a_build_or_compose_file_is_reported_by_its_name(self) -> None:
        for name in (E.capitalize() + "file", f"{E}-compose.yml", f".{E}ignore", "Containerfile", ".devcontainer/devcontainer.json"):
            with self.subTest(name=name):
                sub = tempfile.TemporaryDirectory()
                try:
                    root = Path(sub.name)
                    subprocess.run(["git", "-C", str(root), "init", "-q"], check=True)
                    (root / name).parent.mkdir(parents=True, exist_ok=True)
                    (root / name).write_text("x\n", encoding="utf-8")
                    subprocess.run(["git", "-C", str(root), "add", "-f", name], check=True)
                    code, out = run(root)
                    self.assertEqual(code, 1, out)
                    self.assertIn("named like a container", out)
                finally:
                    sub.cleanup()

    def test_an_action_that_is_not_on_the_allow_list_is_reported(self) -> None:
        # The cargo-deny Action runs in a container without any word of ours saying so.
        self.add(".github/workflows/ci.yml", WORKFLOW_OK + "      - uses: EmbarkStudios/cargo-deny-action@abc123\n")
        code, out = run(self.root)
        self.assertEqual(code, 1, out)
        self.assertIn("EmbarkStudios/cargo-deny-action", out)
        self.assertIn("allow-list", out)

    def test_a_container_or_services_key_in_a_workflow_is_reported(self) -> None:
        self.add(".github/workflows/ci.yml", "jobs:\n  a:\n    container: ubuntu:26.04\n    steps: []\n")
        code, out = run(self.root)
        self.assertEqual(code, 1, out)
        self.assertIn("container: key", out)

    def test_prose_may_describe_the_owners_host(self) -> None:
        self.add("docs/curriculum/host.md", f"The host runs {E} with bridges 172.17.0.0/16.\n")
        self.add("README.md", f"{E} is not used here.\n")
        code, out = run(self.root)
        self.assertEqual(code, 0, out)

    def test_a_directory_that_is_not_a_git_work_tree_is_an_environment_error(self) -> None:
        with tempfile.TemporaryDirectory() as bare:
            code, out = run(Path(bare))
        self.assertEqual(code, 2, out)

    def test_the_repository_itself_passes(self) -> None:
        code, out = run(REPO)
        self.assertEqual(code, 0, out)


if __name__ == "__main__":
    unittest.main()
