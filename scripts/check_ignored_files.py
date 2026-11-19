#!/usr/bin/env python3
"""check_ignored_files.py - no file that git ignores is part of the project (audit 2026-11-19, GIT-1).

A file that `.gitignore` excludes is in no commit: it exists on the machine that wrote it and in no
clone. In 2026-11-19 four synthetic lab fixtures (`*.log`) were excluded this way: the manifest that
lists their SHA-256 passed on the working machine and failed in every clone. This gate lists every
file git ignores in the working tree (`git ls-files --others --ignored --exclude-standard`) and fails
for each one that is not inside a directory known to hold only build output or tool caches.

    python3 -I scripts/check_ignored_files.py [--root DIR]

Exit status: 0 when every ignored file is build output or a cache (or none exists, as in a fresh
clone); 1 when one is not (each is printed with the rule that ignores it); 2 on a usage or
environment error (not a git work tree, git missing). Read-only.
"""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

# Directories (repository-relative, as path prefixes) that hold only build output or tool caches.
# Matching is on whole path components: "target" covers "target/x" and "apps/cli/target/x".
CACHE_DIR_NAMES = frozenset(
    {"target", "node_modules", "dist", "__pycache__", ".ruff_cache", ".pytest_cache", "coverage", ".venv"}
)
CACHE_PATH_PREFIXES = ("apps/desktop/src-tauri/gen/", ".claude/worktrees/")


def git(root: Path, *args: str) -> bytes:
    proc = subprocess.run(["git", "-C", str(root), *args], capture_output=True, check=False)
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.decode("utf-8", "replace").strip() or "git failed")
    return proc.stdout


def is_cache(relative: str) -> bool:
    if relative.startswith(CACHE_PATH_PREFIXES):
        return True
    return any(part in CACHE_DIR_NAMES for part in relative.split("/")[:-1])


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--root", default=str(Path(__file__).resolve().parents[1]), help="repository root")
    args = ap.parse_args(argv)
    root = Path(args.root)
    try:
        listing = git(root, "ls-files", "-z", "--others", "--ignored", "--exclude-standard")
    except (RuntimeError, OSError) as exc:
        sys.stderr.write(f"error: {exc}\n")
        return 2
    ignored = sorted(p for p in listing.decode("utf-8", "surrogateescape").split("\0") if p)
    offenders = [p for p in ignored if not is_cache(p)]
    for path in offenders:
        rule = git(root, "check-ignore", "-v", "--no-index", "--", path).decode("utf-8", "replace").strip()
        print(f"FAIL  {path}  is ignored by git and is not build output or a cache: {rule.split(chr(9))[0]}")
    if offenders:
        print(
            f"\n{len(offenders)} ignored file(s) outside the build and cache directories: they are in no commit. "
            "Track them (add a narrow exception to .gitignore, then git add) or delete them."
        )
        return 1
    print(f"ok    {len(ignored)} ignored file(s), all build output or tool caches")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
