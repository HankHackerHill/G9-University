"""Shared helpers for the gate-script tests in scripts/tests/test_gates_*.py (step CLI-2).

Standard library only. Every gate runs as a black box: `[sys.executable, "-I", script, ...]` in a
subprocess, against a temporary copy of its inputs. Gates that derive the repository root from their
own location (build_curriculum.py, check_source_register.py) are run as a byte-for-byte copy placed at
`<tmp>/scripts/<name>.py`, so the copy resolves its root to the temporary tree and the gate script
itself needs no extra option. Writing modes are refused unless the script runs from a temporary tree
outside the repository.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any, Callable, Iterable

REPO = Path(__file__).resolve().parents[2]
SCRIPTS = REPO / "scripts"
TIMEOUT_S = 120
WRITING_FLAGS = {"--write", "--write-markdown"}


def have_module(name: str) -> bool:
    """True if `name` is importable by this interpreter (the gates run with the same interpreter)."""
    return importlib.util.find_spec(name) is not None


HAVE_JSONSCHEMA = have_module("jsonschema")
HAVE_YAML = have_module("yaml")
# Where the gates run for real (the preflight and CI set it), a missing jsonschema must
# fail the run, not skip most of it: without it the suite would pass after testing
# almost nothing.
if os.environ.get("G9_REQUIRE_JSONSCHEMA") == "1" and not HAVE_JSONSCHEMA:
    raise RuntimeError("G9_REQUIRE_JSONSCHEMA=1 but the python3 module jsonschema is not importable")
NEEDS_JSONSCHEMA = unittest.skipUnless(HAVE_JSONSCHEMA, "python3 module jsonschema is not installed (CI pins jsonschema==4.26.0)")


def _child_env() -> dict[str, str]:
    env = dict(os.environ)
    # Deterministic text I/O in the child; -I already ignores PYTHON* variables.
    env["LC_ALL"] = "C.UTF-8"
    env["LANG"] = "C.UTF-8"
    return env


def _is_within(path: Path, parent: Path) -> bool:
    try:
        path.resolve().relative_to(parent.resolve())
        return True
    except ValueError:
        return False


def run_gate(script: Path, *args: Any, cwd: Path) -> subprocess.CompletedProcess:
    """Run one gate script in isolated mode and capture its output as UTF-8 text."""
    argv = [str(a) for a in args]
    if WRITING_FLAGS.intersection(argv) and _is_within(script, REPO):
        raise AssertionError(f"refusing to run a writing mode of the repository copy of {script.name}")
    return subprocess.run(
        [sys.executable, "-I", str(script), *argv],
        cwd=str(cwd),
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=TIMEOUT_S,
        env=_child_env(),
        check=False,
    )


def output_of(proc: subprocess.CompletedProcess) -> str:
    return (proc.stdout or "") + (proc.stderr or "")


# --------------------------------------------------------------------------
# Repository input guard: hash before (setUpModule) and after (tearDownModule)
# --------------------------------------------------------------------------


def _files_under(path: Path) -> list[Path]:
    if path.is_file():
        return [path]
    if path.is_dir():
        return sorted(p for p in path.rglob("*") if p.is_file() and "__pycache__" not in p.parts)
    return []


def digest_inputs(rel_paths: Iterable[str]) -> dict[str, str]:
    """relative path -> sha256 of every file under the given repository paths (missing paths recorded)."""
    out: dict[str, str] = {}
    for rel in rel_paths:
        path = REPO / rel
        if not path.exists():
            out[rel] = "<missing>"
            continue
        for f in _files_under(path):
            out[f.relative_to(REPO).as_posix()] = hashlib.sha256(f.read_bytes()).hexdigest()
    return out


class InputGuard:
    """Proves that a test module left its repository inputs byte-for-byte unchanged."""

    def __init__(self, rel_paths: Iterable[str]) -> None:
        self.rel_paths = list(rel_paths)
        self.before: dict[str, str] | None = None

    def snapshot(self) -> None:
        self.before = digest_inputs(self.rel_paths)
        if not self.before or all(v == "<missing>" for v in self.before.values()):
            raise AssertionError(f"input guard found none of {self.rel_paths}")

    def verify(self) -> None:
        if self.before is None:
            raise AssertionError("input guard: snapshot() was not called")
        after = digest_inputs(self.rel_paths)
        changed = sorted(k for k in set(self.before) | set(after) if self.before.get(k) != after.get(k))
        if changed:
            raise AssertionError(f"repository inputs changed during the test run: {changed[:20]}")


# --------------------------------------------------------------------------
# Temporary trees and mutations
# --------------------------------------------------------------------------


def copy_into(tmp: Path, rel_paths: Iterable[str]) -> None:
    """Copy repository files or directories to the same relative paths under tmp."""
    for rel in rel_paths:
        src = REPO / rel
        dst = tmp / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        if src.is_dir():
            shutil.copytree(src, dst, ignore=shutil.ignore_patterns("__pycache__"))
        elif src.is_file():
            shutil.copy2(src, dst)
        else:
            raise FileNotFoundError(f"repository input missing: {rel}")


def stage_script(tmp: Path, name: str) -> Path:
    """Copy scripts/<name> byte-for-byte to <tmp>/scripts/<name>; the copy resolves its root to tmp."""
    if _is_within(tmp, REPO):
        raise AssertionError(f"temporary tree {tmp} lies inside the repository")
    dst = tmp / "scripts" / name
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(SCRIPTS / name, dst)
    if dst.read_bytes() != (SCRIPTS / name).read_bytes():
        raise AssertionError(f"staged copy of {name} differs from the repository script")
    return dst


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write_text(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def replace_once(path: Path, old: str, new: str, *, after: str | None = None) -> None:
    """Replace the first occurrence of `old` (searched from the first occurrence of `after`, if given).

    Raises if the anchor or `old` is absent, so a mutation never silently does nothing."""
    text = read_text(path)
    start = 0
    if after is not None:
        start = text.find(after)
        if start < 0:
            raise AssertionError(f"mutation anchor {after!r} not found in {path.name}")
    idx = text.find(old, start)
    if idx < 0:
        raise AssertionError(f"mutation target {old!r} not found in {path.name}")
    write_text(path, text[:idx] + new + text[idx + len(old):])


def load_json(path: Path) -> Any:
    return json.loads(read_text(path))


def dump_json(path: Path, doc: Any) -> None:
    write_text(path, json.dumps(doc, indent=2, ensure_ascii=False) + "\n")


def edit_json(path: Path, fn: Callable[[Any], Any]) -> None:
    """Load, let `fn` mutate the document in place (its return value is ignored), and write it back."""
    doc = load_json(path)
    fn(doc)
    dump_json(path, doc)


class GateTestCase(unittest.TestCase):
    """Base class: a fresh temporary directory per test and assertions on exit code and rule text."""

    maxDiff = None

    def setUp(self) -> None:
        holder = tempfile.TemporaryDirectory(prefix="g9-gate-test-")
        self.addCleanup(holder.cleanup)
        self.tmp = Path(holder.name).resolve()

    def assertExit(self, proc: subprocess.CompletedProcess, code: int) -> None:
        self.assertEqual(proc.returncode, code, f"exit {proc.returncode}, expected {code}; output tail:\n{output_of(proc)[-4000:]}")

    def assertRule(self, proc: subprocess.CompletedProcess, needle: str, code: int = 1) -> None:
        """The gate rejected the input (exit `code`) and named the rule (`needle` in its output)."""
        self.assertExit(proc, code)
        out = output_of(proc)
        self.assertTrue(needle in out, f"rule text {needle!r} missing; output tail:\n{out[-4000:]}")

    def assertKnownGap(
        self,
        mutate: Callable[[], None],
        run: Callable[[], subprocess.CompletedProcess],
        needle: str,
        *,
        kind: str = "accepted",
    ) -> None:
        """Body of an @unittest.expectedFailure test for a known gate defect.

        kind="accepted": the gate should reject the mutation and name `needle`, but exits 0.
        kind="crash":    the gate should name `needle`, but dies with a Python traceback before reporting.

        Only the documented defect fails the test (and so counts as the expected failure). A broken
        mutation, or any other outcome of the gate, ends the test without an exception, which unittest
        reports as an unexpected success and so fails the run: when the defect is fixed, move the case
        into the ordinary rule tests and drop the decorator."""
        if kind not in ("accepted", "crash"):
            raise ValueError(kind)
        try:
            mutate()
            proc = run()
        except Exception as exc:  # noqa: BLE001 - surface a broken harness as an unexpected success
            sys.stderr.write(f"\nknown-gap harness error in {self.id()}: {exc!r}\n")
            return
        out = output_of(proc)
        if kind == "accepted" and proc.returncode == 0:
            self.fail(f"known gap still open: the gate accepted the mutation (exit 0); expected it to name {needle!r}")
        if kind == "crash" and proc.returncode != 0 and "Traceback (most recent call last)" in out and needle not in out:
            self.fail(f"known defect still present: the gate crashed (exit {proc.returncode}) before naming {needle!r}")
        sys.stderr.write(f"\nknown-gap test {self.id()}: outcome changed (exit {proc.returncode}, rule named: {needle in out})\n")
