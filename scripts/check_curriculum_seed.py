#!/usr/bin/env python3
"""check_curriculum_seed.py — gate A01 for curriculum manifests.

Validates one or more manifest files against schemas/curriculum/manifest.schema.json
(JSON Schema draft 2020-12) and then recomputes every importer proof obligation
(handoff section 8 "Structural contract", Appendix A "Importer proof obligations",
corrections C01/C02) independently of the schema, using the same error codes and
JSON-pointer paths as crates/g9-curriculum/src/validate.rs:

  weeks = 12, days = 72, primary objectives = 216, weekday 420 / Saturday 300,
  2,400 min/week, 28,800 min total, Sunday absent, day ids D01..D72 in order
  (week w holds D(6w-5)..D(6w)), day_of_week 1..6 strictly increasing, objective
  ids <day>-O<seq>, blocks sum to planned minutes and match the canonical lists,
  non-empty deliverables, Saturday is the assessment day, no duplicate ids
  (duplicate objective text is a warning), no non-positive durations, well-formed
  competency ids, versioned external resources, provenance PROPOSED_SEED unless a
  complete reconciliation block is present.

It also prints the canonical checksum (SHA-256 over key-sorted compact JSON,
the same construction as g9_domain::hashing::sha256_json) so the value can be
compared with g9_curriculum::manifest_checksum.

Usage:
    python3 -I scripts/check_curriculum_seed.py data/curriculum/g9-ct9-480.seed.v0.json
    python3 -I scripts/check_curriculum_seed.py <manifest.json>... [--schema PATH] [--json]

Exit codes: 0 every manifest valid (warnings allowed); 1 any schema error,
manifest error or invariant violation; 2 usage/environment error.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path
from typing import Any

try:
    from jsonschema import Draft202012Validator
except ImportError:  # pragma: no cover
    sys.stderr.write("error: the `jsonschema` package (>= 4.18) is required\n")
    sys.exit(2)

REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_SCHEMA = REPO_ROOT / "schemas" / "curriculum" / "manifest.schema.json"

# ---- g9_domain::CURRICULUM_INVARIANTS -------------------------------------
INV = {
    "weeks": 12,
    "days_per_week": 6,
    "objectives_per_day": 3,
    "weekday_minutes": 420,
    "saturday_minutes": 300,
    "weekly_minutes": 2400,
    "total_minutes": 28800,
    "total_days": 72,
    "total_objectives": 216,
}
WEEKDAY_BLOCKS = [
    ("recall", 15),
    ("theory", 45),
    ("guided_lab", 120),
    ("independent_lab", 120),
    ("defense_verification", 60),
    ("evidence_reporting", 60),
]
SATURDAY_BLOCKS = [
    ("blind_practical", 120),
    ("remediation", 60),
    ("report", 60),
    ("performance_review", 60),
]
ID_RE = re.compile(r"^[A-Za-z0-9._:-]{1,128}$")
HEX64_RE = re.compile(r"^[0-9a-f]{64}$")
SCHEMA_VERSION_RE = re.compile(r"^([0-9]+)\.([0-9]+)\.([0-9]+)$")

# ---- codes (must stay identical to g9_curriculum::validate::codes) ---------
C = {
    "SCHEMA_VERSION_UNSUPPORTED": "manifest.schema_version.unsupported",
    "CURRICULUM_ID_INVALID": "manifest.curriculum_id.invalid",
    "NAME_EMPTY": "manifest.name.empty",
    "VERSION_EMPTY": "manifest.version.empty",
    "STRUCTURE_MISMATCH": "structure.mismatch",
    "STRUCTURE_SUNDAY": "structure.sunday.not_unscheduled",
    "WEEKDAY_BLOCKS_CANONICAL": "blocks.weekday.canonical_mismatch",
    "SATURDAY_BLOCKS_CANONICAL": "blocks.saturday.canonical_mismatch",
    "WEEK_COUNT": "week.count",
    "WEEK_NUMBER_SEQUENCE": "week.number.out_of_sequence",
    "WEEK_TITLE_EMPTY": "week.title.empty",
    "WEEK_DAYS_COUNT": "week.days.count",
    "DAY_OF_WEEK_RANGE": "day.day_of_week.out_of_range",
    "DAY_SUNDAY_PRESENT": "day.sunday_present",
    "DAY_OF_WEEK_ORDER": "day.day_of_week.not_increasing",
    "DAY_ID_MISMATCH": "day.id.mismatch",
    "DAY_ID_DUPLICATE": "day.id.duplicate",
    "DAY_PLANNED_MINUTES": "day.planned_minutes.mismatch",
    "DAY_OBJECTIVES_COUNT": "day.objectives.count",
    "DAY_EVIDENCE_EMPTY": "day.evidence_artifact.empty",
    "DAY_BLOCKS_SUM": "day.blocks.sum_mismatch",
    "DAY_BLOCKS_CANONICAL": "day.blocks.canonical_mismatch",
    "DAY_ASSESSMENT_FLAG": "day.assessment_flag.mismatch",
    "OBJECTIVE_SEQ": "objective.seq.mismatch",
    "OBJECTIVE_ID_MISMATCH": "objective.id.mismatch",
    "OBJECTIVE_ID_DUPLICATE": "objective.id.duplicate",
    "OBJECTIVE_TEXT_EMPTY": "objective.text.empty",
    "COMPETENCY_ID_INVALID": "objective.competency_id.invalid",
    "BLOCK_SEQ": "block.seq.mismatch",
    "DURATION_NON_POSITIVE": "duration.non_positive",
    "RESOURCE_REF_EMPTY": "resource.ref.empty",
    "RESOURCE_VERSION_REQUIRED": "resource.version.required_for_external",
    "PROVENANCE_RECONCILIATION_REQUIRED": "provenance.reconciliation_required",
    "RECONCILIATION_INCOMPLETE": "reconciliation.incomplete",
    "WARN_OBJECTIVE_TEXT_DUPLICATE": "objective.text.duplicate",
    "WARN_COMPETENCY_ID_DUPLICATE": "objective.competency_id.duplicate",
    "WARN_RECONCILIATION_IGNORED": "provenance.reconciliation_ignored",
}


def canonical_checksum(obj: Any) -> str:
    text = json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def pointer(parts: list[Any]) -> str:
    return "/" + "/".join(str(p).replace("~", "~0").replace("/", "~1") for p in parts)


class Report:
    def __init__(self, path: Path) -> None:
        self.path = path
        self.schema_errors: list[dict[str, str]] = []
        self.errors: list[dict[str, str]] = []
        self.warnings: list[dict[str, str]] = []
        self.violations: list[dict[str, Any]] = []
        self.counts: dict[str, int] = {}
        self.checksum = "-"
        self.header: dict[str, Any] = {}

    def error(self, code: str, path: str, message: str) -> None:
        self.errors.append({"code": code, "path": path, "message": message})

    def warn(self, code: str, path: str, message: str) -> None:
        self.warnings.append({"code": code, "path": path, "message": message})

    @property
    def ok(self) -> bool:
        return not (self.schema_errors or self.errors or self.violations)


def is_int(v: Any) -> bool:
    return isinstance(v, int) and not isinstance(v, bool)


def check_block_list(path: str, lst: Any, canonical: list[tuple[str, int]], code: str, r: Report) -> None:
    if not isinstance(lst, list):
        r.error(code, path, "expected a list")
        return
    for i, b in enumerate(lst):
        if isinstance(b, dict) and is_int(b.get("minutes")) and b["minutes"] <= 0:
            r.error(C["DURATION_NON_POSITIVE"], f"{path}/{i}/minutes", f"block `{b.get('block')}` has non-positive minutes {b['minutes']}")
    actual = [(b.get("block"), b.get("minutes")) if isinstance(b, dict) else None for b in lst]
    if actual != canonical:
        r.error(code, path, "expected exactly [" + ", ".join(f"{n} {m}" for n, m in canonical) + "]")


def recompute(m: Any, r: Report) -> None:
    if not isinstance(m, dict):
        r.error("manifest.not_object", "", "manifest root must be an object")
        return
    # Header ------------------------------------------------------------
    sv = m.get("manifest_schema_version")
    mt = SCHEMA_VERSION_RE.match(sv) if isinstance(sv, str) else None
    if not mt or int(mt.group(1)) != 1 or int(mt.group(2)) > 0:
        r.error(C["SCHEMA_VERSION_UNSUPPORTED"], "/manifest_schema_version", "expected a MAJOR.MINOR.PATCH version accepted by 1.0")
    cid = m.get("curriculum_id")
    if not isinstance(cid, str) or not ID_RE.match(cid):
        r.error(C["CURRICULUM_ID_INVALID"], "/curriculum_id", "contains characters outside [A-Za-z0-9._:-] or bad length")
    if not isinstance(m.get("name"), str) or not m["name"].strip():
        r.error(C["NAME_EMPTY"], "/name", "name must not be empty")
    if not isinstance(m.get("version"), str) or not m["version"].strip():
        r.error(C["VERSION_EMPTY"], "/version", "version must not be empty")
    # Structure ---------------------------------------------------------
    s = m.get("structure") if isinstance(m.get("structure"), dict) else {}
    for name in ("weeks", "days_per_week", "objectives_per_day", "weekday_minutes", "saturday_minutes", "weekly_minutes", "total_minutes"):
        if s.get(name) != INV[name]:
            r.error(C["STRUCTURE_MISMATCH"], f"/structure/{name}", f"expected {INV[name]}, declared {s.get(name)}")
    if s.get("sunday") != "unscheduled":
        r.error(C["STRUCTURE_SUNDAY"], "/structure/sunday", "Sunday must be declared `unscheduled`")
    check_block_list("/weekday_blocks", m.get("weekday_blocks"), WEEKDAY_BLOCKS, C["WEEKDAY_BLOCKS_CANONICAL"], r)
    check_block_list("/saturday_blocks", m.get("saturday_blocks"), SATURDAY_BLOCKS, C["SATURDAY_BLOCKS_CANONICAL"], r)
    # Weeks -------------------------------------------------------------
    counts = {"weeks": 0, "days": 0, "objectives": 0, "total_minutes": 0, "weekday_days": 0, "saturday_days": 0}
    weeks = m.get("weeks")
    if not isinstance(weeks, list):
        weeks = []
        r.error(C["WEEK_COUNT"], "/weeks", "weeks must be a list")
    elif len(weeks) != INV["weeks"]:
        r.error(C["WEEK_COUNT"], "/weeks", f"expected {INV['weeks']} weeks, found {len(weeks)}")
    seen_day: dict[str, str] = {}
    seen_obj: dict[str, str] = {}
    seen_text: dict[str, str] = {}
    for wi, week in enumerate(weeks):
        wpath = f"/weeks/{wi}"
        counts["weeks"] += 1
        if not isinstance(week, dict):
            r.error(C["WEEK_DAYS_COUNT"], wpath, "week must be an object")
            continue
        if week.get("week_number") != wi + 1:
            r.error(C["WEEK_NUMBER_SEQUENCE"], f"{wpath}/week_number", f"expected week_number {wi + 1}, found {week.get('week_number')}")
        if not isinstance(week.get("title"), str) or not week["title"].strip():
            r.error(C["WEEK_TITLE_EMPTY"], f"{wpath}/title", "week title must not be empty")
        days = week.get("days") if isinstance(week.get("days"), list) else []
        if len(days) != INV["days_per_week"]:
            r.error(C["WEEK_DAYS_COUNT"], f"{wpath}/days", f"expected {INV['days_per_week']} days, found {len(days)}")
        prev_dow = None
        for di, day in enumerate(days):
            dpath = f"{wpath}/days/{di}"
            counts["days"] += 1
            if not isinstance(day, dict):
                r.error(C["DAY_ID_MISMATCH"], dpath, "day must be an object")
                continue
            check_day(day, wi, di, dpath, prev_dow, seen_day, seen_obj, seen_text, counts, r)
            prev_dow = day.get("day_of_week")
    r.counts = counts
    # Arithmetic invariants (g9_domain::CurriculumCounts::violations) ----
    for code, expected, actual in (
        ("curriculum.weeks", INV["weeks"], counts["weeks"]),
        ("curriculum.days", INV["total_days"], counts["days"]),
        ("curriculum.objectives", INV["total_objectives"], counts["objectives"]),
        ("curriculum.total_minutes", INV["total_minutes"], counts["total_minutes"]),
        ("curriculum.weekday_days", INV["weeks"] * 5, counts["weekday_days"]),
        ("curriculum.saturday_days", INV["weeks"], counts["saturday_days"]),
    ):
        if expected != actual:
            r.violations.append({"code": code, "expected": expected, "actual": actual})
    # Provenance --------------------------------------------------------
    prov = m.get("provenance")
    rec = m.get("reconciliation")
    if prov == "PROPOSED_SEED":
        if rec is not None:
            r.warn(C["WARN_RECONCILIATION_IGNORED"], "/reconciliation", "a PROPOSED_SEED manifest carries a reconciliation block; it has no effect until provenance is RECONCILED_CANDIDATE")
            check_reconciliation(rec, r)
    elif rec is None:
        r.error(C["PROVENANCE_RECONCILIATION_REQUIRED"], "/provenance", f"provenance {prov} requires a complete reconciliation block")
    else:
        check_reconciliation(rec, r)


def check_day(day: dict, wi: int, di: int, dpath: str, prev_dow: Any, seen_day: dict, seen_obj: dict, seen_text: dict, counts: dict, r: Report) -> None:
    dow_raw = day.get("day_of_week")
    dow = None
    if dow_raw == 7:
        r.error(C["DAY_SUNDAY_PRESENT"], f"{dpath}/day_of_week", "Sunday (7) is unscheduled and must not appear")
    elif is_int(dow_raw) and 1 <= dow_raw <= 6:
        dow = dow_raw
    else:
        r.error(C["DAY_OF_WEEK_RANGE"], f"{dpath}/day_of_week", f"day_of_week.out_of_range: expected 1..=6 (Mon..Sat), actual {dow_raw}")
    if is_int(prev_dow) and is_int(dow_raw) and dow_raw <= prev_dow:
        r.error(C["DAY_OF_WEEK_ORDER"], f"{dpath}/day_of_week", f"day_of_week {dow_raw} does not follow {prev_dow}")
    expected_id = f"D{wi * INV['days_per_week'] + di + 1:02d}"
    day_id = day.get("day_id")
    if day_id != expected_id:
        r.error(C["DAY_ID_MISMATCH"], f"{dpath}/day_id", f"expected `{expected_id}`, found `{day_id}`")
    if isinstance(day_id, str):
        if day_id in seen_day:
            r.error(C["DAY_ID_DUPLICATE"], f"{dpath}/day_id", f"day id `{day_id}` already used at {seen_day[day_id]}")
        else:
            seen_day[day_id] = dpath
    planned = day.get("planned_minutes")
    if not is_int(planned):
        planned = 0
        r.error(C["DURATION_NON_POSITIVE"], f"{dpath}/planned_minutes", "planned_minutes must be an integer")
    elif planned <= 0:
        r.error(C["DURATION_NON_POSITIVE"], f"{dpath}/planned_minutes", f"planned_minutes {planned} is not positive")
    if dow is not None:
        expected_planned = INV["saturday_minutes"] if dow == 6 else INV["weekday_minutes"]
        if planned != expected_planned:
            r.error(C["DAY_PLANNED_MINUTES"], f"{dpath}/planned_minutes", f"expected {expected_planned} for day_of_week {dow}, found {planned}")
        counts["saturday_days" if dow == 6 else "weekday_days"] += 1
        if day.get("is_assessment_day") != (dow == 6):
            r.error(C["DAY_ASSESSMENT_FLAG"], f"{dpath}/is_assessment_day", f"is_assessment_day must be {str(dow == 6).lower()} on day_of_week {dow}")
    counts["total_minutes"] += max(planned, 0)
    # Objectives --------------------------------------------------------
    objectives = day.get("objectives") if isinstance(day.get("objectives"), list) else []
    if len(objectives) != INV["objectives_per_day"]:
        r.error(C["DAY_OBJECTIVES_COUNT"], f"{dpath}/objectives", f"expected {INV['objectives_per_day']} objectives, found {len(objectives)}")
    for oi, obj in enumerate(objectives):
        opath = f"{dpath}/objectives/{oi}"
        counts["objectives"] += 1
        if not isinstance(obj, dict):
            r.error(C["OBJECTIVE_ID_MISMATCH"], opath, "objective must be an object")
            continue
        if obj.get("seq") != oi + 1:
            r.error(C["OBJECTIVE_SEQ"], f"{opath}/seq", f"expected seq {oi + 1}, found {obj.get('seq')}")
        expected_oid = f"{day_id}-O{oi + 1}"
        oid = obj.get("objective_id")
        if oid != expected_oid:
            r.error(C["OBJECTIVE_ID_MISMATCH"], f"{opath}/objective_id", f"expected `{expected_oid}`, found `{oid}`")
        if isinstance(oid, str):
            if oid in seen_obj:
                r.error(C["OBJECTIVE_ID_DUPLICATE"], f"{opath}/objective_id", f"objective id `{oid}` already used at {seen_obj[oid]}")
            else:
                seen_obj[oid] = opath
        text = obj.get("text")
        if not isinstance(text, str) or not text.strip():
            r.error(C["OBJECTIVE_TEXT_EMPTY"], f"{opath}/text", "objective text must not be empty")
        else:
            key = text.strip().lower()
            if key in seen_text:
                r.warn(C["WARN_OBJECTIVE_TEXT_DUPLICATE"], f"{opath}/text", f"objective text duplicates {seen_text[key]}")
            else:
                seen_text[key] = opath
        seen_comp: dict[str, int] = {}
        for ci, cidv in enumerate(obj.get("competency_ids") or []):
            cpath = f"{opath}/competency_ids/{ci}"
            if not isinstance(cidv, str) or not ID_RE.match(cidv):
                r.error(C["COMPETENCY_ID_INVALID"], cpath, "contains characters outside [A-Za-z0-9._:-] or bad length")
            if isinstance(cidv, str):
                if cidv in seen_comp:
                    r.warn(C["WARN_COMPETENCY_ID_DUPLICATE"], cpath, f"competency id repeats entry {seen_comp[cidv]} of this objective")
                else:
                    seen_comp[cidv] = ci
    # Deliverable -------------------------------------------------------
    ev = day.get("evidence_artifact")
    if not isinstance(ev, str) or not ev.strip():
        r.error(C["DAY_EVIDENCE_EMPTY"], f"{dpath}/evidence_artifact", "every day needs a deliverable")
    # Blocks ------------------------------------------------------------
    blocks = day.get("blocks") if isinstance(day.get("blocks"), list) else []
    total = 0
    actual = []
    for bi, b in enumerate(blocks):
        bpath = f"{dpath}/blocks/{bi}"
        if not isinstance(b, dict):
            r.error(C["BLOCK_SEQ"], bpath, "block must be an object")
            actual.append(None)
            continue
        if b.get("seq") != bi + 1:
            r.error(C["BLOCK_SEQ"], f"{bpath}/seq", f"expected seq {bi + 1}, found {b.get('seq')}")
        minutes = b.get("minutes")
        if not is_int(minutes) or minutes <= 0:
            r.error(C["DURATION_NON_POSITIVE"], f"{bpath}/minutes", f"block `{b.get('block')}` has non-positive minutes {minutes}")
        if is_int(minutes):
            total += minutes
        actual.append((b.get("block"), minutes))
    if total != planned:
        r.error(C["DAY_BLOCKS_SUM"], f"{dpath}/blocks", f"blocks sum to {total}, planned_minutes is {planned}")
    if dow is not None:
        canonical = SATURDAY_BLOCKS if dow == 6 else WEEKDAY_BLOCKS
        if actual != canonical:
            r.error(C["DAY_BLOCKS_CANONICAL"], f"{dpath}/blocks", "expected exactly [" + ", ".join(f"{n} {m}" for n, m in canonical) + "]")
    # Resources ---------------------------------------------------------
    for ri, res in enumerate(day.get("resources") or []):
        rpath = f"{dpath}/resources/{ri}"
        if not isinstance(res, dict):
            r.error(C["RESOURCE_REF_EMPTY"], rpath, "resource must be an object")
            continue
        ref = res.get("ref")
        if not isinstance(ref, str) or not ref.strip():
            r.error(C["RESOURCE_REF_EMPTY"], f"{rpath}/ref", "resource ref must not be empty")
        ver = res.get("version")
        versioned = isinstance(ver, str) and bool(ver.strip())
        if res.get("kind") == "EXTERNAL" and not versioned:
            r.error(C["RESOURCE_VERSION_REQUIRED"], f"{rpath}/version", "external resources must carry a version")


def check_reconciliation(rec: Any, r: Report) -> None:
    if not isinstance(rec, dict):
        r.error(C["RECONCILIATION_INCOMPLETE"], "/reconciliation", "reconciliation must be an object")
        return
    for field in ("original_checksum", "diff_checksum"):
        v = rec.get(field)
        if not isinstance(v, str) or not HEX64_RE.match(v):
            r.error(C["RECONCILIATION_INCOMPLETE"], f"/reconciliation/{field}", f"{field} must be a 64-character lowercase hex SHA-256")
    ab = rec.get("approved_by")
    if not isinstance(ab, str) or not ID_RE.match(ab):
        r.error(C["RECONCILIATION_INCOMPLETE"], "/reconciliation/approved_by", "approved_by must be an actor id")
    at = rec.get("approved_at")
    if not is_int(at) or at <= 0:
        r.error(C["RECONCILIATION_INCOMPLETE"], "/reconciliation/approved_at", "approved_at must be a positive UTC epoch millisecond instant")


def check_file(path: Path, validator: Draft202012Validator) -> Report:
    r = Report(path)
    try:
        with path.open("r", encoding="utf-8") as fh:
            manifest = json.load(fh)
    except (OSError, ValueError) as exc:
        r.schema_errors.append({"path": "", "message": f"cannot read/parse JSON: {exc.__class__.__name__}"})
        return r
    r.checksum = canonical_checksum(manifest)
    if isinstance(manifest, dict):
        r.header = {k: manifest.get(k) for k in ("curriculum_id", "name", "version", "provenance", "manifest_schema_version")}
    for err in sorted(validator.iter_errors(manifest), key=lambda e: (list(e.absolute_path), e.message)):
        r.schema_errors.append({"path": pointer(list(err.absolute_path)), "message": err.message[:200]})
    recompute(manifest, r)
    return r


def print_report(r: Report) -> None:
    print(f"== {r.path}")
    for k, v in r.header.items():
        print(f"   {k:<24} {v}")
    print(f"   {'canonical_checksum':<24} {r.checksum}")
    if r.counts:
        c = r.counts
        print(
            f"   counts: weeks={c['weeks']}/{INV['weeks']} days={c['days']}/{INV['total_days']} "
            f"objectives={c['objectives']}/{INV['total_objectives']} total_minutes={c['total_minutes']}/{INV['total_minutes']} "
            f"weekday_days={c['weekday_days']}/{INV['weeks'] * 5} saturday_days={c['saturday_days']}/{INV['weeks']}"
        )
    for e in r.schema_errors:
        print(f"   SCHEMA  {e['path'] or '/'}: {e['message']}")
    for v in r.violations:
        print(f"   VIOLATION {v['code']}: expected {v['expected']}, actual {v['actual']}")
    for e in r.errors:
        print(f"   ERROR   {e['code']} {e['path']}: {e['message']}")
    for w in r.warnings:
        print(f"   WARNING {w['code']} {w['path']}: {w['message']}")
    status = "PASS" if r.ok else "FAIL"
    print(
        f"   result: {status} (schema_errors={len(r.schema_errors)} errors={len(r.errors)} "
        f"violations={len(r.violations)} warnings={len(r.warnings)})"
    )


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Validate curriculum manifests (gate A01).")
    ap.add_argument("manifests", nargs="+", type=Path)
    ap.add_argument("--schema", type=Path, default=DEFAULT_SCHEMA)
    ap.add_argument("--json", action="store_true", help="emit a JSON report instead of text")
    args = ap.parse_args(argv)
    try:
        with args.schema.open("r", encoding="utf-8") as fh:
            schema = json.load(fh)
        Draft202012Validator.check_schema(schema)
        validator = Draft202012Validator(schema)
    except (OSError, ValueError) as exc:
        sys.stderr.write(f"error: cannot load schema {args.schema}: {exc.__class__.__name__}\n")
        return 2
    except Exception as exc:  # jsonschema.SchemaError
        sys.stderr.write(f"error: schema is not a valid draft 2020-12 schema: {exc.__class__.__name__}\n")
        return 2
    reports = [check_file(p, validator) for p in args.manifests]
    if args.json:
        print(
            json.dumps(
                [
                    {
                        "file": str(r.path),
                        "header": r.header,
                        "canonical_checksum": r.checksum,
                        "counts": r.counts,
                        "schema_errors": r.schema_errors,
                        "violations": r.violations,
                        "errors": r.errors,
                        "warnings": r.warnings,
                        "ok": r.ok,
                    }
                    for r in reports
                ],
                indent=1,
            )
        )
    else:
        for r in reports:
            print_report(r)
        failed = sum(1 for r in reports if not r.ok)
        print(f"== {len(reports)} manifest(s) checked, {failed} failed")
    return 0 if all(r.ok for r in reports) else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
