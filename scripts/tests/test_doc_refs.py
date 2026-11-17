"""Black-box tests of scripts/check_doc_refs.py (plan step DOC-2).

The gate runs in a subprocess (`python3 -I`) with `--root` set to a temporary tree: either a small
fixture written by the test, or a copy of the repository (build outputs and local copies left out).
The repository itself is never modified. Run from the repository root:

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

REPO = Path(__file__).resolve().parents[2]
SCRIPT = REPO / "scripts" / "check_doc_refs.py"
COPY_SKIP_NAMES = {".git", "target", "node_modules", "dist", "coverage", "__pycache__"}
COPY_SKIP_PATHS = {".claude/worktrees", "apps/desktop/src-tauri/gen"}

LIB_RS = """\
//! A crate for the fixture. fn in_doc_comment() is not a definition.
pub fn alpha() {}
pub struct Beta;
pub(crate) const GAMMA: u32 = 1;
macro_rules! delta { () => {} }
/* fn in_block_comment() {} /* nested */ fn still_in_comment() {} */
pub const TEXT: &str = "fn in_string() {}";
pub const RAW: &str = r#"struct InRawString;"#;
pub fn lifetimes<'a>(x: &'a str) -> &'a str { x }
pub fn after_lifetimes() {}
"""

TEST_RS = """\
#[test]
fn a_named_test() {}
"""

TOOL_PY = """\
# def in_comment(): pass
def run():
    pass


class Thing:
    pass
"""


def run_gate(root: Path, *args: str) -> subprocess.CompletedProcess:
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


class Fixture(unittest.TestCase):
    """A small tree: one crate, one Python script, a .gitignore with a build output, one document."""

    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory(prefix="g9-docrefs-")
        self.root = Path(self._tmp.name)
        self.write(".gitignore", "/target\nnode_modules\n")
        self.write("README.md", "# Fixture\n\nSee `crates/x/src/lib.rs`.\n")
        self.write("crates/x/src/lib.rs", LIB_RS)
        self.write("crates/x/tests/named.rs", TEST_RS)
        self.write("scripts/tool.py", TOOL_PY)
        self.write("target/debug/out.bin", "build output\n")
        (self.root / "crates/x/empty").mkdir(parents=True)

    def tearDown(self) -> None:
        self._tmp.cleanup()

    def write(self, rel: str, text: str) -> Path:
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8")
        return path

    def doc(self, *lines: str, rel: str = "docs/guide.md") -> None:
        self.write(rel, "\n".join(("# Guide", *lines)) + "\n")

    def allow(self, *entries: dict) -> None:
        self.write("scripts/doc_refs_allow.json", json.dumps({"entries": list(entries)}, indent=2))

    def assertPasses(self, proc: subprocess.CompletedProcess) -> None:
        self.assertEqual(proc.returncode, 0, output(proc))
        self.assertIn("doc refs: OK", proc.stdout)

    def assertFailsWith(self, proc: subprocess.CompletedProcess, *needles: str) -> None:
        self.assertEqual(proc.returncode, 1, output(proc))
        self.assertIn("doc refs: FAIL", proc.stdout)
        for needle in needles:
            self.assertIn(needle, proc.stdout)


class PathRules(Fixture):
    def test_positive_control_fixture(self) -> None:
        self.doc(
            "`crates/x/src/lib.rs` and `crates/x/` and `scripts/tool.py:3` and `README.md`",
            "```",
            "python3 -I scripts/tool.py --flag=crates/x/src/lib.rs",
            "```",
        )
        proc = run_gate(self.root)
        self.assertPasses(proc)
        self.assertIn("7 references checked in 2 documents: 7 resolve", proc.stdout)

    def test_broken_path_fails_with_doc_line_token_reason(self) -> None:
        self.doc("intro", "`crates/x/src/missing.rs`")
        proc = run_gate(self.root)
        self.assertFailsWith(proc, "docs/guide.md:3  crates/x/src/missing.rs  no such file or directory")

    def test_broken_path_in_a_fenced_block_fails(self) -> None:
        self.doc("```sh", "cat scripts/gone.py", "```")
        self.assertFailsWith(run_gate(self.root), "docs/guide.md:3  scripts/gone.py  no such file or directory")

    def test_tree_entries_in_a_fenced_block_are_joined_to_their_directory(self) -> None:
        self.doc("```", "crates/", "  x/", "    src/", "      lib.rs   the library", "      gone.rs  removed", "```")
        proc = run_gate(self.root)
        self.assertFailsWith(proc, "docs/guide.md:7  crates/x/src/gone.rs  no such file or directory")
        self.assertNotIn("crates/x/src/lib.rs  no such", proc.stdout)

    def test_trailing_slash_needs_a_directory(self) -> None:
        self.doc("`crates/x/src/lib.rs/`")
        self.assertFailsWith(run_gate(self.root), "crates/x/src/lib.rs/  a file, not a directory")

    def test_ignored_build_output_does_not_exist_for_the_gate(self) -> None:
        self.assertTrue((self.root / "target/debug/out.bin").is_file())
        self.doc("`target/debug/out.bin`")
        self.assertFailsWith(run_gate(self.root), "target/debug/out.bin  not in the repository: .gitignore ignores it")

    def test_empty_directory_does_not_exist_for_the_gate(self) -> None:
        self.doc("`crates/x/empty/`")
        self.assertFailsWith(run_gate(self.root), "crates/x/empty/  no file in this directory")

    def test_path_leaving_the_repository_fails(self) -> None:
        self.doc("`crates/../../outside.md`")
        self.assertFailsWith(run_gate(self.root), "crates/../../outside.md  outside the repository")

    def test_brace_form_is_expanded_and_each_member_checked(self) -> None:
        self.doc("`crates/x/src/{lib,gone}.rs`")
        proc = run_gate(self.root)
        self.assertFailsWith(proc, "crates/x/src/gone.rs (from crates/x/src/{lib,gone}.rs)  no such file or directory")
        self.assertIn("3 references checked in 2 documents: 2 resolve", proc.stdout)  # lib.rs here and in README.md

    def test_placeholders_are_skipped_and_counted(self) -> None:
        self.doc(
            "`crates/x/<name>.rs` `docs/week-NN.md` `crates/*` `crates/x/{name}.rs` `crates/.../lib.rs` `target/$CRATE`",
        )
        proc = run_gate(self.root)
        self.assertPasses(proc)
        self.assertIn("6 placeholders", proc.stdout)

    def test_words_that_do_not_start_with_a_top_level_name_are_not_references(self) -> None:
        self.doc("`src/lib.rs` `/etc/hosts` `\"crates/x/quoted.rs\"` `crates` `https://example.org/crates/x`")
        proc = run_gate(self.root)
        self.assertPasses(proc)
        self.assertIn("1 references checked in 2 documents", proc.stdout)  # the one in README.md

    def test_documents_outside_the_scanned_set_are_not_read(self) -> None:
        broken = "`crates/x/src/missing.rs`\n"
        self.write("docs/audit/findings/f.md", broken)
        self.write("docs/node_modules/pkg/README.md", broken)
        self.write("crates/x/NOTES.md", broken)
        self.write("apps/a/README.md", "`crates/x/src/lib.rs`\n")
        proc = run_gate(self.root)
        self.assertPasses(proc)
        self.assertIn("in 2 documents", proc.stdout)

    def test_every_readme_is_read_wherever_it_is(self) -> None:
        # A README is read where it lies (data/fixtures/README.md was missed while only docs/ and
        # apps/*/README.md were): a broken reference in a nested README fails.
        broken = "`crates/x/src/missing.rs`\n"
        self.write("crates/x/README.md", broken)
        self.write("apps/a/sub/README.md", broken)
        proc = run_gate(self.root)
        self.assertEqual(proc.returncode, 1, output(proc))
        self.assertIn("crates/x/README.md:1  crates/x/src/missing.rs", proc.stdout)
        self.assertIn("apps/a/sub/README.md:1  crates/x/src/missing.rs", proc.stdout)

    def test_no_document_is_a_usage_error(self) -> None:
        (self.root / "README.md").unlink()
        proc = run_gate(self.root)
        self.assertEqual(proc.returncode, 2, output(proc))
        self.assertIn("no document found", proc.stderr)


class GitignoreRules(Fixture):
    """The tree as git holds it: nested .gitignore files, `!pattern` re-includes, last match wins."""

    def test_nested_negation_re_includes_a_file_and_leaves_its_sibling_ignored(self) -> None:
        self.write(".gitignore", "/target\nnode_modules\n*.bin\n")
        self.write("data/.gitignore", "# kept on purpose\n!keep*.bin\n")
        self.write("data/keep-1.bin", "fixture\n")
        self.write("data/other.bin", "local\n")
        self.doc("`data/keep-1.bin`")
        self.assertPasses(run_gate(self.root))
        self.doc("`data/keep-1.bin` `data/other.bin`")
        proc = run_gate(self.root)
        self.assertFailsWith(proc, "docs/guide.md:2  data/other.bin  not in the repository: .gitignore ignores it")
        self.assertNotIn("data/keep-1.bin  not in", proc.stdout)

    def test_last_matching_rule_wins_and_deeper_files_come_after_inherited_rules(self) -> None:
        self.write(".gitignore", "/target\nnode_modules\n*.bin\n")
        self.write("data/.gitignore", "!keep*.bin\nkeep-old.bin\n")
        self.write("data/sub/.gitignore", "!keep-old.bin\n")
        for rel in ("data/keep-1.bin", "data/keep-old.bin", "data/sub/keep-old.bin"):
            self.write(rel, "fixture\n")
        self.doc("`data/keep-1.bin` `data/sub/keep-old.bin` `data/keep-old.bin`")
        proc = run_gate(self.root)
        self.assertFailsWith(proc, "data/keep-old.bin  not in the repository: .gitignore ignores it")
        self.assertIn("4 references checked in 2 documents: 3 resolve", proc.stdout)  # keep-1, sub/keep-old, README.md
        self.assertNotIn("data/sub/keep-old.bin  not in", proc.stdout)

    def test_negation_does_not_re_include_below_an_ignored_directory(self) -> None:
        self.write(".gitignore", "/target\nnode_modules\n!/target/debug/out.bin\n")
        self.doc("`target/debug/out.bin`")
        self.assertFailsWith(run_gate(self.root), "target/debug/out.bin  not in the repository: .gitignore ignores it")

    def test_escaped_pattern_is_a_usage_error(self) -> None:
        self.write("data/.gitignore", "\\#literal-hash\n")
        proc = run_gate(self.root)
        self.assertEqual(proc.returncode, 2, output(proc))
        self.assertIn("data/.gitignore:1: escaped patterns are not supported", proc.stderr)


class NameAndLineRules(Fixture):
    def test_names_defined_in_the_file_resolve(self) -> None:
        refs = [f"`crates/x/src/lib.rs::{n}`" for n in ("alpha", "Beta", "GAMMA", "delta", "after_lifetimes")]
        refs += ["`crates/x/tests/named.rs::a_named_test`", "`scripts/tool.py::run`", "`scripts/tool.py::Thing`"]
        refs += ["`crates/x::alpha`", "`crates/x/src/lib.rs::Beta::alpha`"]
        self.doc(" ".join(refs))
        proc = run_gate(self.root)
        self.assertPasses(proc)
        self.assertIn(f"{len(refs) + 1} resolve", proc.stdout)  # + the one in README.md

    def test_missing_name_fails(self) -> None:
        self.doc("`crates/x/src/lib.rs::epsilon`")
        self.assertFailsWith(run_gate(self.root), "crates/x/src/lib.rs::epsilon  `epsilon` is not defined in crates/x/src/lib.rs")

    def test_a_renamed_test_fails_the_gate(self) -> None:
        self.doc("`crates/x/tests/named.rs::a_named_test`")
        self.assertPasses(run_gate(self.root))
        self.write("crates/x/tests/named.rs", TEST_RS.replace("a_named_test", "a_renamed_test"))
        self.assertFailsWith(run_gate(self.root), "`a_named_test` is not defined in crates/x/tests/named.rs")

    def test_names_in_comments_and_strings_are_not_definitions(self) -> None:
        for name in ("in_doc_comment", "in_block_comment", "still_in_comment", "in_string", "InRawString"):
            with self.subTest(name=name):
                self.doc(f"`crates/x/src/lib.rs::{name}`")
                self.assertFailsWith(run_gate(self.root), f"`{name}` is not defined in crates/x/src/lib.rs")

    def test_python_comment_is_not_a_definition(self) -> None:
        self.doc("`scripts/tool.py::in_comment`")
        self.assertFailsWith(run_gate(self.root), "`in_comment` is not defined in scripts/tool.py (def or class)")

    def test_missing_name_below_a_directory_fails(self) -> None:
        self.doc("`crates/x::nowhere`")
        self.assertFailsWith(run_gate(self.root), "`nowhere` is not defined in any .rs file below crates/x/")

    def test_name_on_a_file_type_without_definitions_fails(self) -> None:
        self.doc("`README.md::alpha`")
        self.assertFailsWith(run_gate(self.root), "README.md::alpha  `::name` can be checked in a .rs or .py file")

    def test_line_within_the_file_resolves_and_beyond_the_end_fails(self) -> None:
        lines = LIB_RS.count("\n")
        self.doc(f"`crates/x/src/lib.rs:{lines}` `crates/x/src/lib.rs:2-{lines}`")
        self.assertPasses(run_gate(self.root))
        self.doc(f"`crates/x/src/lib.rs:3,{lines + 1}`")
        self.assertFailsWith(run_gate(self.root), f"line {lines + 1} is beyond the end of crates/x/src/lib.rs ({lines} lines)")

    def test_line_on_a_directory_fails(self) -> None:
        self.doc("`crates/x:3`")
        self.assertFailsWith(run_gate(self.root), "crates/x:3  a line number needs a file")


class AllowList(Fixture):
    def entry(self, ref: str, reason: str = "planned", docs: list[str] | None = None) -> dict:
        return {"ref": ref, "reason": reason, "docs": docs or ["docs/guide.md"], "note": "a reason in words"}

    def test_allow_listed_absent_reference_passes_and_is_counted(self) -> None:
        self.doc("`scripts/planned.py` and `target/debug/out.bin`")
        self.allow(self.entry("scripts/planned.py"), self.entry("target/debug/out.bin", "external"))
        proc = run_gate(self.root)
        self.assertPasses(proc)
        self.assertIn("2 absent and allow-listed (planned 1, historical 0, external 1)", proc.stdout)

    def test_entry_whose_reference_resolves_now_is_stale(self) -> None:
        self.doc("`scripts/planned.py`")
        self.allow(self.entry("scripts/planned.py"))
        self.assertPasses(run_gate(self.root))
        self.write("scripts/planned.py", "def main():\n    pass\n")
        self.assertFailsWith(run_gate(self.root), "scripts/planned.py  stale entry: the reference resolves now")

    def test_entry_that_no_document_refers_to_is_stale(self) -> None:
        self.doc("nothing here")
        self.allow(self.entry("scripts/planned.py"))
        self.assertFailsWith(
            run_gate(self.root), "scripts/planned.py  stale entry: docs/guide.md does not hold this reference any more"
        )

    def test_entry_naming_a_document_that_is_not_scanned_is_stale(self) -> None:
        self.doc("`scripts/planned.py`")
        self.allow(self.entry("scripts/planned.py", docs=["docs/guide.md", "docs/gone.md"]))
        self.assertFailsWith(run_gate(self.root), "scripts/planned.py  stale entry: docs/gone.md is not a scanned document")

    def test_entry_covers_only_the_documents_it_lists(self) -> None:
        self.doc("`scripts/planned.py`")
        self.doc("`scripts/planned.py`", rel="docs/other.md")
        self.allow(self.entry("scripts/planned.py"))
        self.assertFailsWith(
            run_gate(self.root),
            "docs/other.md:2  scripts/planned.py  no such file or directory; the allow-list has it as planned for other documents only",
        )

    def test_reason_outside_the_closed_set_fails(self) -> None:
        self.doc("`scripts/planned.py`")
        self.allow(self.entry("scripts/planned.py", reason="later"))
        self.assertFailsWith(run_gate(self.root), "reason must be one of planned, historical, external")

    def test_ref_must_be_written_in_canonical_form(self) -> None:
        self.doc("`scripts/planned/`")
        self.allow(self.entry("scripts/planned/"))
        self.assertFailsWith(run_gate(self.root), "write the ref as scripts/planned")

    def test_entry_without_a_note_fails(self) -> None:
        self.doc("`scripts/planned.py`")
        self.allow({"ref": "scripts/planned.py", "reason": "planned", "docs": ["docs/guide.md"], "note": " "})
        self.assertFailsWith(run_gate(self.root), "note must say why")

    def test_explicit_allow_file_that_does_not_exist_is_a_usage_error(self) -> None:
        self.doc("`crates/x/src/lib.rs`")
        proc = run_gate(self.root, "--allow", str(self.root / "nope.json"))
        self.assertEqual(proc.returncode, 2, output(proc))
        self.assertIn("no such file", proc.stderr)


class RealTree(unittest.TestCase):
    """The repository as it is, copied: the positive control and fail-first cases on real documents."""

    @classmethod
    def setUpClass(cls) -> None:
        cls._tmp = tempfile.TemporaryDirectory(prefix="g9-docrefs-real-")
        cls.root = Path(cls._tmp.name) / "repo"
        copy_repository(cls.root)

    @classmethod
    def tearDownClass(cls) -> None:
        cls._tmp.cleanup()

    def test_positive_control_real_tree(self) -> None:
        proc = run_gate(self.root)
        self.assertEqual(proc.returncode, 0, output(proc))
        self.assertRegex(proc.stdout, r"doc refs: OK \(\d+ references checked in \d+ documents")
        self.assertIn(", 0 broken;", proc.stdout)

    def test_real_renamed_item_and_removed_file_fail(self) -> None:
        listed = run_gate(self.root, "--list")
        self.assertEqual(listed.returncode, 0, output(listed))
        listing = listed.stdout
        named = re.search(r"^(\S+):\d+  ((crates|apps|tests)/[^\s:]+\.rs)::([A-Za-z_][A-Za-z0-9_]*)  OK$", listing, re.M)
        self.assertIsNotNone(named, "no resolving file.rs::name reference in the documents")
        assert named is not None
        doc, path, name = named.group(1), named.group(2), named.group(4)
        with tempfile.TemporaryDirectory(prefix="g9-docrefs-real-neg-") as tmp:
            root = Path(tmp) / "repo"
            copy_repository(root)
            source = root / path
            text = source.read_text(encoding="utf-8")
            renamed = re.sub(rf"(?<![A-Za-z0-9_]){re.escape(name)}(?![A-Za-z0-9_])", f"{name}_renamed", text)
            source.write_text(renamed, encoding="utf-8")
            proc = run_gate(root)
            self.assertEqual(proc.returncode, 1, output(proc))
            self.assertIn(f"`{name}` is not defined in {path}", proc.stdout)
            self.assertIn(f"{doc}:", proc.stdout)
            source.unlink()
            proc = run_gate(root)
            self.assertEqual(proc.returncode, 1, output(proc))
            self.assertIn(f"{path}::{name}  no such file or directory", proc.stdout)

    def test_real_allow_list_entry_for_an_existing_file_is_stale(self) -> None:
        allow = json.loads((self.root / "scripts/doc_refs_allow.json").read_text(encoding="utf-8"))
        planned = next(e for e in allow["entries"] if e["reason"] == "planned" and "." in e["ref"].rsplit("/", 1)[-1])
        with tempfile.TemporaryDirectory(prefix="g9-docrefs-real-stale-") as tmp:
            root = Path(tmp) / "repo"
            copy_repository(root)
            created = root / planned["ref"]
            created.parent.mkdir(parents=True, exist_ok=True)
            created.write_text("created\n", encoding="utf-8")
            proc = run_gate(root)
            self.assertEqual(proc.returncode, 1, output(proc))
            self.assertIn(f"{planned['ref']}  stale entry: the reference resolves now", proc.stdout)


if __name__ == "__main__":
    unittest.main()
