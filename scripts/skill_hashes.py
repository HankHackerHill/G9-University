#!/usr/bin/env python3
"""Skill package hashes: one definition, checked against every place that records it.

The hash is exactly `g9_skills::package::package_sha256` (crates/g9-skills/src/package.rs):
for every regular file under the package directory, recursively, sorted by its
`/`-joined relative path (byte order), the hasher absorbs

    path bytes | 0x00 | file length as 8 big-endian bytes | file bytes

Symlinks and every other entry that is not a regular file or a directory (FIFO, socket,
device) are refused, as the loader refuses them. The result is the lowercase hex SHA-256.

Recorded in two places, both of which this script checks:

* `apps/desktop/src-tauri/skill-pins.json` - the trust anchor compiled into the
  desktop binary (`Registry::load_pinned`); full 64-hex hashes.
* `docs/skills/registry.md` - the "Package hash" column; first 16 hex.

Usage (run from the repository root; stdlib only; no network):

    python3 -I scripts/skill_hashes.py --check    # CI gate: exit 1 on any mismatch
    python3 -I scripts/skill_hashes.py --write    # deliberate regeneration after a reviewed package change

`--write` blesses whatever is on disk. Run it only after reviewing the package
diff; a changed pin manifest is a change to the runtime trust anchor and needs
the same review as a change to the packages themselves.
"""
import hashlib
import json
import os
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKILLS = ROOT / ".agents" / "skills"
PINS = ROOT / "apps" / "desktop" / "src-tauri" / "skill-pins.json"
REGISTRY = ROOT / "docs" / "skills" / "registry.md"
ALGORITHM = "g9_skills::package::package_sha256"


def package_sha256(directory: Path) -> str:
    files = []
    for base, dirs, names in os.walk(directory):
        base_path = Path(base)
        for name in dirs + names:
            if (base_path / name).is_symlink():
                raise SystemExit(f"symlink in skill package: {(base_path / name).relative_to(directory)}")
        for name in names:
            if not (base_path / name).is_file():
                raise SystemExit(f"not a regular file in skill package: {(base_path / name).relative_to(directory)}")
            files.append((base_path / name).relative_to(directory).as_posix())
    files.sort(key=lambda rel: rel.encode("utf-8"))
    digest = hashlib.sha256()
    for rel in files:
        data = (directory / rel).read_bytes()
        digest.update(rel.encode("utf-8"))
        digest.update(b"\x00")
        digest.update(len(data).to_bytes(8, "big"))
        digest.update(data)
    return digest.hexdigest()


def computed() -> dict:
    if not SKILLS.is_dir():
        raise SystemExit(f"missing {SKILLS}")
    return {d.name: package_sha256(d) for d in sorted(SKILLS.iterdir()) if d.is_dir()}


ROW = re.compile(r"^(\|\s*[A-Z]\d{2}\s*\|\s*)(g9-[a-z0-9-]+)(\s*\|.*\|\s*)([0-9a-f]{16})(\s*\|\s*)$")


def registry_rows() -> list:
    return REGISTRY.read_text(encoding="utf-8").splitlines()


def check() -> int:
    want = computed()
    problems = []
    pins = json.loads(PINS.read_text(encoding="utf-8"))
    if pins.get("algorithm") != ALGORITHM:
        problems.append(f"{PINS.name}: algorithm is {pins.get('algorithm')!r}, expected {ALGORITHM!r}")
    if set(pins) != {"algorithm", "pins"}:
        problems.append(f"{PINS.name}: unexpected keys {sorted(set(pins) - {'algorithm', 'pins'})}")
    have = pins.get("pins", {})
    for name in sorted(set(want) | set(have)):
        if name not in have:
            problems.append(f"{PINS.name}: package {name} has no pin")
        elif name not in want:
            problems.append(f"{PINS.name}: pin {name} has no package directory")
        elif have[name] != want[name]:
            problems.append(f"{PINS.name}: {name} pin {have[name][:16]} != computed {want[name][:16]}")
    seen = set()
    for line in registry_rows():
        m = ROW.match(line)
        if not m:
            continue
        name, short = m.group(2), m.group(4)
        seen.add(name)
        if name not in want:
            problems.append(f"{REGISTRY.name}: row {name} has no package directory")
        elif short != want[name][:16]:
            problems.append(f"{REGISTRY.name}: {name} hash {short} != computed {want[name][:16]}")
    for name in sorted(set(want) - seen):
        problems.append(f"{REGISTRY.name}: package {name} has no row")
    if problems:
        print(f"skill hash check: FAIL ({len(problems)} problem(s))")
        for p in problems:
            print("  -", p)
        return 1
    print(f"skill hash check: OK ({len(want)} packages; pins and registry agree with {ALGORITHM})")
    return 0


def write() -> int:
    want = computed()
    PINS.write_text(json.dumps({"algorithm": ALGORITHM, "pins": want}, indent=2) + "\n", encoding="utf-8")
    out = []
    for line in registry_rows():
        m = ROW.match(line)
        if m and m.group(2) in want:
            line = f"{m.group(1)}{m.group(2)}{m.group(3)}{want[m.group(2)][:16]}{m.group(5)}"
        out.append(line)
    REGISTRY.write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"wrote {PINS.relative_to(ROOT)} ({len(want)} pins) and the hash column of {REGISTRY.relative_to(ROOT)}")
    return check()


def main(argv: list) -> int:
    if argv[1:] == ["--check"]:
        return check()
    if argv[1:] == ["--write"]:
        return write()
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
