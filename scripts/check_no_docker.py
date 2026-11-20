#!/usr/bin/env python3
"""check_no_docker.py - no container engine anywhere in the project's code (owner instruction, 2026-11-20).

The owner's rule: the container engine is not used in this repository, and not run by anyone working
on it, unless the owner lifts the rule explicitly. This gate keeps the code side of that rule true:

    python3 -I scripts/check_no_docker.py [--root DIR]

It reads every tracked file (`git ls-files`) and fails when
  1. a file is named like a container build or composition file (the engine's build file, its
     compose files and ignore file, a Containerfile, a dev-container definition, or the files of
     the other container engines);
  2. a code or configuration file (any tracked file outside `docs/` that is not Markdown) names a
     container engine or its tools (the engine itself, its daemon, its compose tool, and the other
     container engines and build tools listed in ENGINE_TOKENS);
  3. a GitHub workflow uses an Action that is not on the allow-list below, or uses a `container:`
     or `services:` key. An Action can run in a container without any word of ours saying so (the
     cargo-deny Action did: its action.yml says `runs.using` is a container image), so a new Action
     is added to ALLOWED_ACTIONS only after its action.yml has been read and its `runs.using` is
     `node*` or `composite`.
Documents under `docs/` and Markdown files are prose: they may describe the owner's host or quote the
owner's plan, and are not read here.

Exit status: 0 when clean; 1 when a rule fails (each finding is printed with file and line); 2 on a
usage or environment error (not a git work tree, git missing). Read-only. This file names the engine
only in pieces, so that it does not match its own rule.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

_E = "dock" + "er"
ENGINE_TOKENS = [_E, _E + "d", _E + "file", _E + "-compose", "pod" + "man", "container" + "d", "build" + "ah", "nerd" + "ctl"]
TOKEN_RE = re.compile(r"(?i)(?<![A-Za-z0-9_])(" + "|".join(re.escape(t) for t in ENGINE_TOKENS) + r")(?![A-Za-z0-9_])")

# Basenames (lower-case prefixes or exact names) of container build and composition files.
NAME_PREFIXES = (_E + "file", _E + "-compose", "compose.y", "container" + "file", "pod" + "man")
NAME_SUFFIXES = ("." + _E + "file",)
NAME_EXACT = ("." + _E + "ignore", "devcontainer.json")

# Actions whose action.yml was read: runs.using is a JavaScript runtime or composite, never a container.
ALLOWED_ACTIONS = frozenset(
    {
        "actions/checkout",
        "actions/setup-node",
        "actions/upload-artifact",
        "dtolnay/rust-toolchain",
        "Swatinem/rust-cache",
    }
)

USES_RE = re.compile(r"^\s*(?:-\s*)?uses:\s*([^\s#]+)")
CONTAINER_KEY_RE = re.compile(r"^\s*(?:-\s*)?(container|services)\s*:")


def git(root: Path, *args: str) -> bytes:
    proc = subprocess.run(["git", "-C", str(root), *args], capture_output=True, check=False)
    if proc.returncode != 0:
        raise RuntimeError(proc.stderr.decode("utf-8", "replace").strip() or "git failed")
    return proc.stdout


def is_prose(path: str) -> bool:
    return path.startswith("docs/") or path.lower().endswith(".md")


def name_findings(path: str) -> list[str]:
    base = path.rsplit("/", 1)[-1].lower()
    parts = path.lower().split("/")
    if (
        base.startswith(NAME_PREFIXES)
        or base.endswith(NAME_SUFFIXES)
        or base in NAME_EXACT
        or ".devcontainer" in parts
    ):
        return [f"{path}  is named like a container build or composition file"]
    return []


def content_findings(root: Path, path: str) -> list[str]:
    try:
        text = (root / path).read_bytes().decode("utf-8")
    except (OSError, UnicodeDecodeError):
        return []
    found = []
    for n, line in enumerate(text.splitlines(), 1):
        match = TOKEN_RE.search(line)
        if match:
            found.append(f"{path}:{n}  names a container engine or tool ({match.group(1).lower()})")
    return found


def workflow_findings(root: Path, path: str) -> list[str]:
    try:
        text = (root / path).read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError):
        return []
    found = []
    for n, line in enumerate(text.splitlines(), 1):
        uses = USES_RE.match(line)
        if uses:
            action = uses.group(1).split("@", 1)[0]
            if action not in ALLOWED_ACTIONS:
                found.append(
                    f"{path}:{n}  uses the Action {action!r}, which is not on the allow-list of Actions known not to "
                    "run in a container (read its action.yml: runs.using must be node* or composite)"
                )
        if CONTAINER_KEY_RE.match(line):
            found.append(f"{path}:{n}  uses a {CONTAINER_KEY_RE.match(line).group(1)}: key (a container)")
    return found


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--root", default=str(Path(__file__).resolve().parents[1]), help="repository root")
    args = ap.parse_args(argv)
    root = Path(args.root)
    try:
        listing = git(root, "ls-files", "-z")
    except (RuntimeError, OSError) as exc:
        sys.stderr.write(f"error: {exc}\n")
        return 2
    tracked = sorted(p for p in listing.decode("utf-8", "surrogateescape").split("\0") if p)
    findings: list[str] = []
    for path in tracked:
        findings += name_findings(path)
        if not is_prose(path):
            findings += content_findings(root, path)
        if path.startswith(".github/workflows/") and path.endswith((".yml", ".yaml")):
            findings += workflow_findings(root, path)
    for item in findings:
        print(f"FAIL  {item}")
    if findings:
        print(f"\n{len(findings)} finding(s): the container engine is not used in this repository (owner instruction, 2026-11-20).")
        return 1
    print(f"ok    {len(tracked)} tracked file(s): no container engine in code, configuration or workflows")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
