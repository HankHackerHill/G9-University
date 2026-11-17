"""Black-box tests of scripts/gen_metrics.py (plan step DOC-2).

The generator runs in a subprocess (`python3 -I`) with `--root` set to a temporary tree: a small
fixture whose counts are known by construction, or a copy of the repository (build outputs and local
copies left out). `--write` only ever writes into a temporary tree; the repository is never modified.
Run from the repository root:

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
SCRIPT = REPO / "scripts" / "gen_metrics.py"
OUTPUT = "docs/release/metrics.md"
COPY_SKIP_NAMES = {".git", "target", "node_modules", "dist", "coverage", "__pycache__"}
COPY_SKIP_PATHS = {".claude/worktrees", "apps/desktop/src-tauri/gen"}

FIXTURE = {
    "Cargo.toml": """\
[workspace]
members = ["crates/g9-app", "crates/g9-storage", "crates/g9-evaluation", "apps/cli"]
default-members = ["crates/g9-app", "crates/g9-storage", "crates/g9-evaluation"]
""",
    "crates/g9-app/Cargo.toml": '[package]\nname = "g9-app"\n',
    "crates/g9-app/src/lib.rs": "pub mod ops;\npub mod dispatch;\npub mod teaching;\npub mod research;\n",
    "crates/g9-app/src/ops.rs": """\
pub const OPERATIONS: &[OpSpec] = &[
    op("system.status", false),
    exam_exempt(op("backup.create", true)),
    // op("commented.out", true),
];

#[cfg(test)]
mod tests {
    #[test]
    fn count() {
        assert_eq!(OPERATIONS.len(), 2);
    }
}
""",
    "crates/g9-app/src/dispatch.rs": """\
fn route(op: &str) {
    for area in [teaching::PROPOSED_OPERATIONS, research::PROPOSED_OPERATIONS] {
        let _ = (area, op);
    }
}
""",
    "crates/g9-app/src/teaching.rs": 'pub const PROPOSED_OPERATIONS: &[&str] = &["teaching.lesson", "teaching.quiz"];\n',
    "crates/g9-app/src/research/mod.rs": 'pub const PROPOSED_OPERATIONS: &[&str] = &[\n    "research.search",\n];\n',
    "crates/g9-app/tests/t.rs": '#[test]\nfn a() {}\n// #[test] in a comment\nconst S: &str = "#[test]";\n/* #[test] */\n',
    "crates/g9-storage/Cargo.toml": '[package]\nname = "g9-storage"\n',
    "crates/g9-storage/src/lib.rs": "pub mod migrations;\n",
    "crates/g9-storage/src/migrations.rs": 'const A: &str = include_str!("migrations/0001_a.sql");\nconst B: &str = include_str!("migrations/0002_b.sql");\n',
    "crates/g9-storage/src/migrations/0001_a.sql": "-- CREATE TABLE ghost (x TEXT);\nCREATE TABLE a (x TEXT) STRICT;\nCREATE TABLE IF NOT EXISTS b (y TEXT);\n",
    "crates/g9-storage/src/migrations/0002_b.sql": "DROP TABLE b;\nALTER TABLE a RENAME TO a2;\nCREATE TABLE c (note TEXT DEFAULT 'CREATE TABLE fake');\n",
    "crates/g9-evaluation/Cargo.toml": '[package]\nname = "g9-evaluation"\n\n[[test]]\nname = "gate"\npath = "../../tests/integration/gate.rs"\n',
    "crates/g9-evaluation/src/lib.rs": "\n",
    "tests/integration/gate.rs": "#[test]\nfn g() {}\n#[test]\nfn h() {}\n",
    "tests/integration/orphan.rs": "fn never_built() {}\n",
    "apps/cli/Cargo.toml": '[package]\nname = "g9-cli"\n\n[[example]]\nname = "ex"\npath = "examples/ex.rs"\n',
    "apps/cli/src/main.rs": "fn main() {}\n",
    "apps/cli/examples/ex.rs": "fn main() {}\n\n",
    "apps/cli/src/commands/ops.rs": "fn run() {\n    let _ = [g9_app::teaching::PROPOSED_OPERATIONS, g9_app::research::PROPOSED_OPERATIONS];\n}\n",
    "apps/desktop/src/main.tsx": "export {};\n",
    "apps/desktop/src/lib/a.ts": "export const a = 1;\nexport const b = 2;\n",
    "apps/desktop/src/lib/a.test.ts": "import { a } from './a';\n",
    "apps/desktop/src/test/setup.ts": "export {};\n",
    "apps/desktop/vite.config.ts": "export default {};\n",
    "apps/desktop/src/theme.css": ":root {}\nbody {}\n",
    "apps/desktop/node_modules/pkg/index.ts": "not counted\n",
    "docs/adr/ADR-0001-x.md": "# ADR\n",
    "docs/adr/ADR-0002-y.md": "# ADR\n",
    "docs/adr/README.md": "# not an ADR\n",
    "schemas/ipc/system.status.request.schema.json": "{}\n",
    "schemas/ipc/system.status.response.schema.json": "{}\n",
    "schemas/ipc/backup.create.request.schema.json": "{}\n",
    "schemas/ipc/command-meta.schema.json": "{}\n",
    "schemas/other.schema.json": "{}\n",
    ".agents/skills/g9-one/SKILL.md": "# one\n",
    ".agents/skills/g9-two/SKILL.md": "# two\n",
    ".agents/skills/not-a-package/README.md": "# no SKILL.md\n",
    "data/competency-packs/p1/pack.json": "{}\n",
    "scripts/a.py": "\n",
    "scripts/b.sh": "\n",
    "scripts/e2e/tour.cjs": "\n",
    "scripts/tests/test_a.py": "\n",
    "scripts/tests/helper.py": "\n",
}


def run_gen(root: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, "-I", str(SCRIPT), "--root", str(root), *args],
        cwd=str(root),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=120,
        check=False,
    )


def output(proc: subprocess.CompletedProcess) -> str:
    return (proc.stdout or "") + (proc.stderr or "")


def copy_repository(dst: Path) -> None:
    """Copy the repository to `dst` without build outputs, dependencies and local worktrees."""

    def ignore(directory: str, names: list[str]) -> set[str]:
        rel = Path(directory).resolve().relative_to(REPO.resolve()).as_posix()
        skipped = set()
        for name in names:
            child = name if rel == "." else f"{rel}/{name}"
            if name in COPY_SKIP_NAMES or child in COPY_SKIP_PATHS:
                skipped.add(name)
        return skipped

    shutil.copytree(REPO, dst, ignore=ignore, symlinks=True)


class FixtureTree(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory(prefix="g9-metrics-")
        self.root = Path(self._tmp.name)
        for rel, text in FIXTURE.items():
            self.write(rel, text)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def write(self, rel: str, text: str) -> None:
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")

    def generated(self) -> str:
        proc = run_gen(self.root, "--write")
        self.assertEqual(proc.returncode, 0, output(proc))
        return (self.root / OUTPUT).read_text(encoding="utf-8")

    def assertCrossCheckFails(self, rule: str) -> None:
        proc = run_gen(self.root, "--write")
        self.assertEqual(proc.returncode, 1, output(proc))
        self.assertIn(f"metrics: FAIL cross-check {rule}", proc.stdout)
        self.assertFalse((self.root / OUTPUT).exists(), "nothing is written when a cross-check fails")

    def test_counts_follow_the_rules(self) -> None:
        text = self.generated()
        expected_rows = [
            "| Workspace members (crates) | 4 (3 under `crates/`; 3 default members) |",
            "| Routed IPC operations | 2 |",
            "| PROPOSED IPC operations | 3 (research 1, teaching 2) |",
            "| Migrations | 2 (0001 to 0002) |",
            "| `CREATE TABLE` statements in the migrations | 3 |",
            "| Tables after the last migration | 2 (1 `DROP TABLE`, 1 renames) |",
            "| Migrations embedded by `crates/g9-storage/src/migrations.rs` | 2 |",
            "| ADRs | 2 |",
            "| JSON schemas | 5 (4 in `schemas/ipc/`, 1 elsewhere) |",
            "| Skill packages | 2 |",
            "| Competency packs | 1 |",
            "| Frontend TS/TSX | 5 files, 6 lines; 1 test files |",
            "| CSS | 1 files, 2 lines |",
            "| Scripts | 2 in `scripts/`, 1 in `scripts/e2e/`, 1 test modules in `scripts/tests/` |",
            # src: lib 4 + ops 13 + dispatch 5 + teaching 1 + research 3; tests: t.rs 5; #[test]: ops 1 + t.rs 1
            "| g9-app | `crates/g9-app` | yes | 6 | 26 | 5 | 0 | 2 |",
            # the [[test]] path outside the member directory counts as its tests
            "| g9-evaluation | `crates/g9-evaluation` | yes | 2 | 1 | 4 | 0 | 2 |",
            "| g9-cli | `apps/cli` | no | 3 | 4 | 0 | 2 | 0 |",
            "| `research` | 0 | 1 |",
            "| `system` | 1 | 0 |",
            "Routed operations with a request and a response schema in `schemas/ipc/`: 1 of 2. Schemas in `schemas/ipc/`: 2 request, 1 response, 1 shared.",
            "- `tests/integration/orphan.rs`",
        ]
        for row in expected_rows:
            with self.subTest(row=row):
                self.assertIn(row, text)

    def test_output_is_reproducible_and_holds_no_date_or_commit(self) -> None:
        first = self.generated()
        second = self.generated()
        self.assertEqual(first, second)
        self.assertIsNone(re.search(r"\b20\d\d-\d\d-\d\d\b", first))
        self.assertIsNone(re.search(r"\b[0-9a-f]{7,40}\b", first))
        proc = run_gen(self.root)
        self.assertEqual(proc.returncode, 0, output(proc))
        self.assertIn("metrics: OK", proc.stdout)

    def test_missing_file_fails(self) -> None:
        proc = run_gen(self.root)
        self.assertEqual(proc.returncode, 1, output(proc))
        self.assertIn(f"{OUTPUT} is missing", proc.stdout)

    def test_stale_file_fails_with_a_diff(self) -> None:
        self.generated()
        self.write("crates/g9-app/tests/t2.rs", "#[test]\nfn b() {}\n")
        proc = run_gen(self.root)
        self.assertEqual(proc.returncode, 1, output(proc))
        self.assertIn(f"metrics: FAIL {OUTPUT} is stale", proc.stdout)
        self.assertIn("+| g9-app | `crates/g9-app` | yes | 7 | 26 | 7 | 0 | 3 |", proc.stdout)

    def test_hand_edited_number_is_stale(self) -> None:
        text = self.generated()
        self.write(OUTPUT, text.replace("| Routed IPC operations | 2 |", "| Routed IPC operations | 3 |"))
        proc = run_gen(self.root)
        self.assertEqual(proc.returncode, 1, output(proc))
        self.assertIn("is stale", proc.stdout)

    def test_cross_check_name_both_routed_and_proposed(self) -> None:
        self.write("crates/g9-app/src/teaching.rs", 'pub const PROPOSED_OPERATIONS: &[&str] = &["system.status"];\n')
        self.assertCrossCheckFails("operations.disjoint")

    def test_cross_check_duplicate_routed_name(self) -> None:
        ops = FIXTURE["crates/g9-app/src/ops.rs"].replace('op("backup.create", true)', 'op("system.status", true)')
        self.write("crates/g9-app/src/ops.rs", ops)
        self.assertCrossCheckFails("operations.unique")

    def test_cross_check_cli_lists_other_areas(self) -> None:
        self.write("apps/cli/src/commands/ops.rs", "fn run() { let _ = [g9_app::teaching::PROPOSED_OPERATIONS]; }\n")
        self.assertCrossCheckFails("operations.cli-areas")

    def test_cross_check_test_assertion_disagrees(self) -> None:
        ops = FIXTURE["crates/g9-app/src/ops.rs"].replace("OPERATIONS.len(), 2", "OPERATIONS.len(), 3")
        self.write("crates/g9-app/src/ops.rs", ops)
        self.assertCrossCheckFails("operations.test-assertion")

    def test_cross_check_entry_without_a_literal_name(self) -> None:
        ops = FIXTURE["crates/g9-app/src/ops.rs"].replace('op("system.status", false)', "op(NAME, false)")
        self.write("crates/g9-app/src/ops.rs", ops)
        self.assertCrossCheckFails("operations.parse")

    def test_missing_registry_is_an_environment_error(self) -> None:
        (self.root / "crates/g9-app/src/ops.rs").unlink()
        proc = run_gen(self.root)
        self.assertEqual(proc.returncode, 2, output(proc))
        self.assertIn("crates/g9-app/src/ops.rs: cannot read", proc.stderr)

    def test_module_named_by_dispatch_without_a_file_is_an_environment_error(self) -> None:
        (self.root / "crates/g9-app/src/teaching.rs").unlink()
        proc = run_gen(self.root)
        self.assertEqual(proc.returncode, 2, output(proc))
        self.assertIn("names module `teaching`", proc.stderr)


class RealTree(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls._tmp = tempfile.TemporaryDirectory(prefix="g9-metrics-real-")
        cls.root = Path(cls._tmp.name) / "repo"
        copy_repository(cls.root)

    @classmethod
    def tearDownClass(cls) -> None:
        cls._tmp.cleanup()

    def test_positive_control_real_tree(self) -> None:
        proc = run_gen(self.root)
        self.assertEqual(proc.returncode, 0, output(proc) + "\n(run python3 -I scripts/gen_metrics.py --write after changing the tree)")
        self.assertIn(f"metrics: OK ({OUTPUT} is current)", proc.stdout)

    def test_real_tree_goes_stale_when_a_test_is_added(self) -> None:
        with tempfile.TemporaryDirectory(prefix="g9-metrics-real-neg-") as tmp:
            root = Path(tmp) / "repo"
            copy_repository(root)
            self.assertEqual(run_gen(root, "--write").returncode, 0)
            extra = root / "crates/g9-domain/tests/metrics_probe.rs"
            extra.parent.mkdir(parents=True, exist_ok=True)
            extra.write_text("#[test]\nfn probe() {}\n", encoding="utf-8")
            proc = run_gen(root)
            self.assertEqual(proc.returncode, 1, output(proc))
            self.assertIn("is stale", proc.stdout)
            self.assertRegex(proc.stdout, r"\+\| g9-domain \| `crates/g9-domain` \|")


    def test_a_file_git_ignores_never_changes_a_count(self) -> None:
        # The root .gitignore ignores *.log: a local log in a counted directory is not a file of the
        # tree, so the gate stays green in a working tree exactly as in a clean export.
        with tempfile.TemporaryDirectory(prefix="g9-metrics-real-ign-") as tmp:
            root = Path(tmp) / "repo"
            copy_repository(root)
            self.assertEqual(run_gen(root, "--write").returncode, 0)
            (root / "scripts" / "local.log").write_text("noise\n", encoding="utf-8")
            proc = run_gen(root)
            self.assertEqual(proc.returncode, 0, output(proc))
            # The control: the same file under a name git keeps does change the count.
            (root / "scripts" / "local.log").rename(root / "scripts" / "local_tool.py")
            proc = run_gen(root)
            self.assertEqual(proc.returncode, 1, output(proc))
            self.assertIn("is stale", proc.stdout)

if __name__ == "__main__":
    unittest.main()
