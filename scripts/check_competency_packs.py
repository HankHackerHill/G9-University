#!/usr/bin/env python3
"""check_competency_packs.py — validate data/competency-packs/*/pack.json.

Rules (schemas/competency-pack.schema.json plus cross-record rules):
  * every pack validates against the schema (Draft 2020-12, no external refs)
  * `pack_id` equals the directory name; pack ids unique
  * competency ids unique within a pack; assessment, lab fixture and
    misconception ids unique within a pack
  * `difficulty_band.min <= max`; every assessment difficulty lies in the band
  * BLIND_PRACTICAL and TRANSFER_CHECK assessments carry >= 1 rubric_focus
  * each pack has >= 1 BLIND_PRACTICAL assessment and >= 1 nice_refs entry
  * prerequisites resolve: bare ids within the pack, `pack_id:competency_id`
    to another pack; no self-reference; no cycles across the whole graph
  * APPROVED packs: every source and nice_refs entry is VERIFIED
  * --expect-count N (default 8): number of VALID packs must equal N

Usage:
    python3 -I scripts/check_competency_packs.py data/competency-packs
    python3 -I scripts/check_competency_packs.py data/competency-packs --expect-count 8 --json

Exit codes: 0 ok; 1 failures; 2 usage/environment error.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any

try:
    import jsonschema
    from jsonschema import Draft202012Validator
except ImportError:  # pragma: no cover
    sys.stderr.write("error: the `jsonschema` package (>= 4.18) is required\n")
    sys.exit(2)

DEFAULT_SCHEMA = Path(__file__).resolve().parent.parent / "schemas" / "competency-pack.schema.json"


class PackReport:
    def __init__(self, directory: Path) -> None:
        self.directory = directory
        self.pack_id = "-"
        self.status = "-"
        self.version = "-"
        self.competencies = 0
        self.failures: list[str] = []
        self.checks = 0

    def check(self, cond: bool, msg: str) -> bool:
        self.checks += 1
        if not cond:
            self.failures.append(msg)
        return cond

    @property
    def passed(self) -> bool:
        return not self.failures

    def to_json(self) -> dict[str, Any]:
        return {
            "directory": str(self.directory),
            "pack_id": self.pack_id,
            "status": self.status,
            "version": self.version,
            "competencies": self.competencies,
            "checks": self.checks,
            "pass": self.passed,
            "failures": list(self.failures),
        }


def load_pack(rep: PackReport, validator: Draft202012Validator) -> dict[str, Any] | None:
    path = rep.directory / "pack.json"
    if not rep.check(path.is_file(), "pack.json missing"):
        return None
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        rep.check(False, f"pack.json invalid JSON: {exc.msg} at line {exc.lineno}")
        return None
    except UnicodeDecodeError:
        rep.check(False, "pack.json is not UTF-8")
        return None
    rep.checks += 1
    errors = sorted(validator.iter_errors(doc), key=lambda e: list(e.path))
    if not rep.check(not errors, "schema: " + "; ".join(
            f"{'/'.join(str(p) for p in e.path) or '$'}: {e.message}" for e in errors[:5])
            + (" ..." if len(errors) > 5 else "")):
        return None
    return doc


def check_pack(rep: PackReport, doc: dict[str, Any]) -> None:
    rep.pack_id = doc["pack_id"]
    rep.status = doc["status"]
    rep.version = doc["version"]
    rep.competencies = len(doc["competencies"])
    rep.check(doc["pack_id"] == rep.directory.name, f"pack_id {doc['pack_id']!r} does not match directory {rep.directory.name!r}")

    comp_ids = [c["id"] for c in doc["competencies"]]
    dup = sorted({c for c in comp_ids if comp_ids.count(c) > 1})
    rep.check(not dup, f"duplicate competency ids {dup}")

    assessment_ids: list[str] = []
    fixture_ids: list[str] = []
    blind = 0
    nice_total = 0
    for comp in doc["competencies"]:
        cid = comp["id"]
        band = comp["difficulty_band"]
        rep.check(band["min"] <= band["max"], f"{cid}: difficulty_band.min > max")
        mids = [m["id"] for m in comp["misconceptions"]]
        rep.check(len(set(mids)) == len(mids), f"{cid}: duplicate misconception ids")
        nice_total += len(comp["nice_refs"])
        for a in comp["assessments"]:
            assessment_ids.append(a["id"])
            rep.check(band["min"] <= a["difficulty"] <= band["max"],
                      f"{cid}: assessment {a['id']} difficulty {a['difficulty']} outside band [{band['min']},{band['max']}]")
            if a["kind"] in ("BLIND_PRACTICAL", "TRANSFER_CHECK"):
                rep.check(len(a["rubric_focus"]) >= 1, f"{cid}: assessment {a['id']} ({a['kind']}) needs >= 1 rubric_focus")
            if a["kind"] == "BLIND_PRACTICAL":
                blind += 1
        for f in comp["lab_fixtures"]:
            fixture_ids.append(f["id"])
        rep.check(cid not in comp["prerequisites"], f"{cid}: lists itself as a prerequisite")
        rep.check(f"{doc['pack_id']}:{cid}" not in comp["prerequisites"], f"{cid}: lists itself (qualified) as a prerequisite")
    dup_a = sorted({a for a in assessment_ids if assessment_ids.count(a) > 1})
    rep.check(not dup_a, f"duplicate assessment ids {dup_a}")
    dup_f = sorted({f for f in fixture_ids if fixture_ids.count(f) > 1})
    rep.check(not dup_f, f"duplicate lab fixture ids {dup_f}")
    rep.check(blind >= 1, "pack needs at least one BLIND_PRACTICAL assessment (independent practical assessment)")
    rep.check(nice_total >= 1, "pack needs at least one nice_refs entry with the pinned release identifier")

    if doc["status"] == "APPROVED":
        unverified_sources = [s["title"] for s in doc["sources"] if s["verification"] != "VERIFIED"]
        rep.check(not unverified_sources, f"APPROVED pack has ASSUMED sources {unverified_sources}")
        unverified_nice = [f"{c['id']}:{n['id']}" for c in doc["competencies"] for n in c["nice_refs"] if n["confidence"] != "VERIFIED"]
        rep.check(not unverified_nice, f"APPROVED pack has ASSUMED nice_refs {unverified_nice}")


def check_graph(reports: list[PackReport], docs: dict[str, dict[str, Any]]) -> list[str]:
    """Resolve prerequisites across packs and detect cycles. Returns global failures."""
    failures: list[str] = []
    nodes: dict[str, list[str]] = {}
    by_pack = {pid: {c["id"] for c in d["competencies"]} for pid, d in docs.items()}
    rep_by_pack = {r.pack_id: r for r in reports if r.pack_id in docs}
    for pid, d in docs.items():
        rep = rep_by_pack[pid]
        for comp in d["competencies"]:
            node = f"{pid}:{comp['id']}"
            edges: list[str] = []
            for pre in comp["prerequisites"]:
                if ":" in pre:
                    tpack, tcomp = pre.split(":", 1)
                    if not rep.check(tpack in by_pack, f"{comp['id']}: prerequisite {pre!r} names unknown pack {tpack!r}"):
                        continue
                    if not rep.check(tcomp in by_pack[tpack], f"{comp['id']}: prerequisite {pre!r} not found in pack {tpack!r}"):
                        continue
                    edges.append(f"{tpack}:{tcomp}")
                else:
                    if not rep.check(pre in by_pack[pid], f"{comp['id']}: prerequisite {pre!r} not found in this pack"):
                        continue
                    edges.append(f"{pid}:{pre}")
            nodes[node] = edges

    WHITE, GREY, BLACK = 0, 1, 2
    color = {n: WHITE for n in nodes}
    stack: list[str] = []

    def dfs(n: str) -> str | None:
        color[n] = GREY
        stack.append(n)
        for m in nodes.get(n, []):
            if m not in color:
                continue
            if color[m] == GREY:
                cyc = stack[stack.index(m):] + [m]
                return " -> ".join(cyc)
            if color[m] == WHITE:
                found = dfs(m)
                if found:
                    return found
        stack.pop()
        color[n] = BLACK
        return None

    for n in sorted(nodes):
        if color[n] == WHITE:
            cyc = dfs(n)
            if cyc:
                failures.append(f"prerequisite cycle: {cyc}")
                stack.clear()
    return failures


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Validate competency packs against schemas/competency-pack.schema.json")
    ap.add_argument("root", nargs="?", default="data/competency-packs", help="packs root (default data/competency-packs)")
    ap.add_argument("--schema", default=str(DEFAULT_SCHEMA), help="path to competency-pack.schema.json")
    ap.add_argument("--expect-count", type=int, default=8, help="required number of VALID packs (default 8)")
    ap.add_argument("--json", action="store_true", help="machine-readable JSON output")
    args = ap.parse_args(argv)

    root = Path(args.root)
    if not root.is_dir():
        sys.stderr.write(f"error: {root} is not a directory\n")
        return 2
    schema_path = Path(args.schema)
    try:
        schema = json.loads(schema_path.read_text(encoding="utf-8"))
        Draft202012Validator.check_schema(schema)
    except (OSError, json.JSONDecodeError, jsonschema.SchemaError) as exc:
        sys.stderr.write(f"error: cannot load schema {schema_path}: {exc}\n")
        return 2
    validator = Draft202012Validator(schema)

    dirs = sorted(p for p in root.iterdir() if p.is_dir() and not p.name.startswith("."))
    reports: list[PackReport] = []
    docs: dict[str, dict[str, Any]] = {}
    for d in dirs:
        rep = PackReport(d)
        doc = load_pack(rep, validator)
        if doc is not None:
            check_pack(rep, doc)
            if doc["pack_id"] in docs:
                rep.check(False, f"duplicate pack_id {doc['pack_id']!r}")
            else:
                docs[doc["pack_id"]] = doc
        reports.append(rep)

    global_failures = check_graph(reports, docs)
    stray = sorted(p.name for p in root.iterdir() if not p.is_dir())
    if stray:
        global_failures.append(f"non-directory entries in packs root are not allowed: {stray}")

    valid = sum(1 for r in reports if r.passed)
    count_ok = valid == args.expect_count
    ok = valid == len(reports) and count_ok and not global_failures

    if args.json:
        print(json.dumps({
            "root": str(root),
            "schema": str(schema_path),
            "expected_count": args.expect_count,
            "packs": [r.to_json() for r in reports],
            "global_failures": global_failures,
            "summary": {"packs": len(reports), "valid": valid, "invalid": len(reports) - valid, "expected_count_ok": count_ok},
            "ok": ok,
        }, indent=2))
        return 0 if ok else 1

    print(f"Competency pack check — root: {root} — schema: {schema_path}")
    print(f"{'Pack':<36} {'Status':<9} {'Version':<9} {'Comp':>4} {'Checks':>6} {'Fail':>4}  Result")
    print("-" * 82)
    for r in reports:
        print(f"{r.directory.name:<36} {r.status:<9} {r.version:<9} {r.competencies:>4} {r.checks:>6} {len(r.failures):>4}  "
              f"{'PASS' if r.passed else 'FAIL'}")
    for r in reports:
        if r.failures:
            print(f"\n{r.directory.name}:")
            for f in r.failures:
                print(f"  FAIL  {f}")
    for g in global_failures:
        print(f"\nFAIL  {g}")
    print(f"\nSummary: {len(reports)} pack(s), {valid} valid, {len(reports) - valid} invalid; expected {args.expect_count} valid -> "
          f"{'OK' if count_ok else 'MISMATCH'}")
    print("RESULT: " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
