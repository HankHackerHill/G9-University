#!/usr/bin/env python3
"""collect_gate_evidence.py — run every repository check and record executed evidence per gate.

Runs (from the repository root):
  * cargo test --all-targets            (default workspace members)
  * cargo test -p g9-retention --features fsrs
  * cargo fmt --all --check / cargo clippy --all-targets -- -D warnings
  * npm run typecheck / lint / vitest (JSON reporter) / build   in apps/desktop
  * the four content validators in scripts/
  * cargo-deny (if a binary is given with --cargo-deny)

Parses each individual test result from the real output, maps test names to acceptance gates
(A01..A12, B01..B06) and adversarial scenarios (ST01..ST12) by name prefix (`a01_`, `b02_`, `st07_`),
and writes:
  docs/release/gate-evidence.json   machine-readable results
  docs/release/gate-evidence.md     tables used by docs/release/stage-report.md

Nothing is inferred: a gate with no executed test is NOT RUN; a gate with any failing test is FAIL.

Usage:
  python3 -I scripts/collect_gate_evidence.py [--cargo-deny PATH] [--skip-npm]
"""
from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# Reuse an existing build directory instead of compiling the workspace twice:
#   G9_EVIDENCE_TARGET=/abs/path/to/target-dir python3 -I scripts/collect_gate_evidence.py ...
TARGET = Path(os.environ["G9_EVIDENCE_TARGET"]) if os.environ.get("G9_EVIDENCE_TARGET") else ROOT / "target" / "evidence"
TEST_LINE = re.compile(r"^test (\S+) \.\.\. (ok|FAILED|ignored)\b")
RUNNING = re.compile(r"^\s*Running (?:unittests )?(\S+) \(.*?deps/([A-Za-z0-9_]+)-[0-9a-f]+\)")
GATE = re.compile(r"(?:^|(?<=[^a-z0-9]))(a(?:0[1-9]|1[0-2])|b0[1-6]|st(?:0[1-9]|1[0-2]))(?=_)", re.IGNORECASE)

GATES = {
    **{f"A{i:02d}": "" for i in range(1, 13)},
    **{f"B{i:02d}": "" for i in range(1, 7)},
    **{f"ST{i:02d}": "" for i in range(1, 13)},
}
GATE_TITLES = {
    "A01": "Curriculum invariants and provenance", "A02": "Recurrence, DST, conflicts",
    "A03": "Timer survives suspend/crash; uncertain time not credited", "A04": "Finalized grade immutability; retake is new attempt",
    "A05": "Competency projection replayable; correction", "A06": "Policy denies unapproved actions",
    "A07": "Malicious note/skill cannot escalate", "A08": "Backup, restore, migrations",
    "A09": "Core works with model disabled", "A10": "Export and deletion propagation",
    "A11": "Model/skill output validated", "A12": "SBOM, licences, pins, accessibility",
    "B01": "Rubric validity", "B02": "Retention vs transfer separated", "B03": "Intervention vs baseline",
    "B04": "Effect size and uncertainty", "B05": "Correction propagates", "B06": "Planning overhead",
}


def run(cmd: list[str], cwd: Path, env: dict[str, str] | None = None, timeout: int = 3600) -> tuple[int, str]:
    e = dict(os.environ)
    e["CARGO_TARGET_DIR"] = str(TARGET)
    e["CARGO_TERM_COLOR"] = "never"
    e["NO_COLOR"] = "1"
    if env:
        e.update(env)
    try:
        p = subprocess.run(cmd, cwd=cwd, env=e, capture_output=True, text=True, timeout=timeout)
        return p.returncode, (p.stdout or "") + (p.stderr or "")
    except FileNotFoundError as exc:
        return 127, f"not found: {exc}"
    except subprocess.TimeoutExpired:
        return 124, "timeout"


def parse_cargo(out: str) -> list[dict]:
    tests, binary = [], "?"
    for line in out.splitlines():
        m = RUNNING.match(line)
        if m:
            binary = m.group(2)
            continue
        m = TEST_LINE.match(line.strip())
        if m:
            tests.append({"suite": "cargo", "binary": binary, "name": m.group(1), "status": m.group(2)})
    return tests


def parse_vitest(json_path: Path) -> list[dict]:
    if not json_path.is_file():
        return []
    doc = json.loads(json_path.read_text(encoding="utf-8"))
    tests = []
    for f in doc.get("testResults", []):
        rel = os.path.relpath(f.get("name", "?"), ROOT)
        for a in f.get("assertionResults", []):
            st = {"passed": "ok", "failed": "FAILED"}.get(a.get("status"), a.get("status", "?"))
            tests.append({"suite": "vitest", "binary": rel, "name": " > ".join(a.get("ancestorTitles", []) + [a.get("title", "")]), "status": st})
    return tests


def summary_line(out: str, pattern: str) -> str:
    lines = [l.strip() for l in out.splitlines() if re.search(pattern, l)]
    return lines[-1] if lines else ""


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-deny", default=None)
    ap.add_argument("--skip-npm", action="store_true")
    args = ap.parse_args(argv)

    checks: list[dict] = []
    tests: list[dict] = []

    def check(name: str, cmd: list[str], cwd: Path = ROOT, **kw) -> tuple[int, str]:
        code, out = run(cmd, cwd, **kw)
        checks.append({"check": name, "command": " ".join(cmd), "cwd": os.path.relpath(cwd, ROOT), "exit": code})
        return code, out

    check("cargo fmt --check", ["cargo", "fmt", "--all", "--check"])
    code, out = check("cargo clippy -D warnings", ["cargo", "clippy", "--all-targets", "--", "-D", "warnings"])
    checks[-1]["summary"] = summary_line(out, r"^(warning|error)") or "clean"
    code, out = check("cargo test (default members)", ["cargo", "test", "--all-targets", "--no-fail-fast"])
    tests += parse_cargo(out)
    code, out = check("cargo test g9-retention --features fsrs", ["cargo", "test", "-p", "g9-retention", "--features", "fsrs", "--no-fail-fast"])
    tests += [dict(t, binary=t["binary"] + "[fsrs]") for t in parse_cargo(out)]
    code, out = check("cargo check g9-mentor-desktop (Tauri shell)", ["cargo", "check", "-p", "g9-mentor-desktop"])
    checks[-1]["summary"] = "ok" if code == 0 else (summary_line(out, r"(webkit2gtk|gtk|pkg-config|error)") or "failed")[:300]

    if not args.skip_npm:
        app = ROOT / "apps" / "desktop"
        for name, cmd in [("npm typecheck", ["npm", "run", "typecheck"]), ("npm lint", ["npm", "run", "lint"]), ("npm build", ["npm", "run", "build"])]:
            check(name, cmd, cwd=app)
        vjson = TARGET / "vitest.json"
        TARGET.mkdir(parents=True, exist_ok=True)
        check("vitest", ["npx", "vitest", "run", "--reporter=json", f"--outputFile={vjson}"], cwd=app)
        tests += parse_vitest(vjson)
        code, out = check("npm audit --audit-level=high", ["npm", "audit", "--audit-level=high"], cwd=app)
        checks[-1]["summary"] = summary_line(out, r"vulnerabilit") or "no vulnerabilities"

    for name, cmd in [
        ("curriculum seed (A01 static)", ["python3", "-I", "scripts/check_curriculum_seed.py", "data/curriculum/g9-ct9-480.seed.v0.json"]),
        ("skill packages (37)", ["python3", "-I", "scripts/check_skill_packages.py", ".agents/skills", "--expect-count", "37"]),
        ("competency packs (8)", ["python3", "-I", "scripts/check_competency_packs.py", "data/competency-packs", "--expect-count", "8"]),
        ("JSON schemas", ["python3", "-I", "scripts/check_schemas.py", "schemas", ".agents/skills"]),
    ]:
        code, out = check(name, cmd)
        checks[-1]["summary"] = summary_line(out, r"(Summary|RESULT|result:|PASS|FAIL)")

    if args.cargo_deny:
        code, out = check("cargo-deny (bans licenses sources advisories)", [args.cargo_deny, "--all-features", "check"])
        checks[-1]["summary"] = summary_line(out, r"(advisories|bans|licenses|sources) (ok|FAILED)")

    gate_tests: dict[str, list[dict]] = {g: [] for g in GATES}
    for t in tests:
        leaf = t["name"].split("::")[-1]
        for m in GATE.finditer(leaf.lower()):
            g = m.group(1).upper()
            if g in gate_tests:
                gate_tests[g].append(t)

    def gate_status(ts: list[dict]) -> str:
        ran = [t for t in ts if t["status"] in ("ok", "FAILED")]
        if not ran:
            return "NOT RUN"
        return "FAIL" if any(t["status"] == "FAILED" for t in ran) else "PASS"

    now = dt.datetime.now(dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    result = {
        "collected_at": now,
        "checks": checks,
        "totals": {
            "cargo_passed": sum(1 for t in tests if t["suite"] == "cargo" and t["status"] == "ok"),
            "cargo_failed": sum(1 for t in tests if t["suite"] == "cargo" and t["status"] == "FAILED"),
            "cargo_ignored": sum(1 for t in tests if t["suite"] == "cargo" and t["status"] == "ignored"),
            "vitest_passed": sum(1 for t in tests if t["suite"] == "vitest" and t["status"] == "ok"),
            "vitest_failed": sum(1 for t in tests if t["suite"] == "vitest" and t["status"] == "FAILED"),
        },
        "gates": {g: {"status": gate_status(ts), "tests": [f"{t['binary']}::{t['name']} [{t['status']}]" for t in ts]} for g, ts in gate_tests.items()},
        "failed_tests": [f"{t['suite']}:{t['binary']}::{t['name']}" for t in tests if t["status"] == "FAILED"],
    }
    out_dir = ROOT / "docs" / "release"
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "gate-evidence.json").write_text(json.dumps(result, indent=1) + "\n", encoding="utf-8")

    tot = result["totals"]
    md = [f"# Gate evidence (executed {now})", "",
          "Generated by `python3 -I scripts/collect_gate_evidence.py`. Every row comes from executed output in this run; "
          "a gate without an executed test is **NOT RUN**. Test-to-gate mapping is by name prefix (`a01_`, `b02_`, `st07_`).", "",
          "## Checks", "", "| Check | Command | Exit | Summary |", "|---|---|---|---|"]
    for c in checks:
        md.append(f"| {c['check']} | `{c['command']}` ({c['cwd']}) | {c['exit']} | {c.get('summary', '').replace('|', '/')} |")
    md += ["", "## Test totals", "", "| Suite | Passed | Failed | Ignored |", "|---|---|---|---|",
           f"| cargo (Rust) | {tot['cargo_passed']} | {tot['cargo_failed']} | {tot['cargo_ignored']} |",
           f"| vitest (UI) | {tot['vitest_passed']} | {tot['vitest_failed']} | — |", "",
           "## Gates", "", "| Gate | Title | Status | Executed tests (count) | Examples |", "|---|---|---|---|---|"]
    for g, v in result["gates"].items():
        ex = "; ".join(x.split("::")[-1].split(" [")[0] for x in v["tests"][:3])
        md.append(f"| {g} | {GATE_TITLES.get(g, 'adversarial scenario')} | {v['status']} | {len(v['tests'])} | {ex} |")
    if result["failed_tests"]:
        md += ["", "## Failed tests", ""] + [f"- `{t}`" for t in result["failed_tests"]]
    (out_dir / "gate-evidence.md").write_text("\n".join(md) + "\n", encoding="utf-8")
    print(json.dumps({"totals": tot, "gates": {g: v["status"] for g, v in result["gates"].items()}, "checks": {c["check"]: c["exit"] for c in checks}}, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
