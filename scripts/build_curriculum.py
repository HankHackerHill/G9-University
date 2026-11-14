#!/usr/bin/env python3
"""build_curriculum.py: the authored 12-week curriculum, checked, and turned into its importable forms.

Source of truth: one Markdown file per week, `docs/curriculum/weeks/week-NN.md` (lessons the learner reads
and the data the application imports, in one place). Generated from it:

  data/curriculum/g9-ct9-480.draft.v1.json     the manifest (schema 1.0.0), provenance PROPOSED_SEED
  data/curriculum/assessments/WNN-*.json        one `assessment.define` payload per Saturday
  docs/curriculum/coverage.md                   competency x week matrix, prerequisite order, changes against the seed

Usage (stdlib only; run from anywhere):
    python3 -I scripts/build_curriculum.py                    check: validate the sources, require the generated files to be current
    python3 -I scripts/build_curriculum.py --write            validate and (re)write the generated files
    python3 -I scripts/build_curriculum.py --allow-partial    validate only the weeks that exist (authoring aid; writes nothing)

Exit codes: 0 valid (and current); 1 a rule is broken or a generated file is out of date; 2 usage or environment error.

The structure that the application enforces (12 weeks, 6 days, 3 objectives, 420/300 minutes) comes from the seed
manifest, not from this file. What this file adds to the seed is the content of each objective (the Appendix A
wording stays as the start of the text), the competency links, the resources and the hand-in criteria. Nothing is
approved by building it: the output is PROPOSED until the operator promotes it (docs/curriculum/provenance-and-reconciliation.md).
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
WEEKS_DIR = REPO / "docs" / "curriculum" / "weeks"
SEED = REPO / "data" / "curriculum" / "g9-ct9-480.seed.v0.json"
PACKS = REPO / "data" / "competency-packs"
OUT_MANIFEST = REPO / "data" / "curriculum" / "g9-ct9-480.draft.v1.json"
OUT_ASSESSMENTS = REPO / "data" / "curriculum" / "assessments"
OUT_COVERAGE = REPO / "docs" / "curriculum" / "coverage.md"

VERSION = "1.0.0-draft1"
PROVENANCE_NOTE = (
    "Built in the repository from the Appendix A skeleton (seed 0.1.0-seed): every Appendix A objective keeps its wording as the start "
    "of its text and gains an observable success criterion; each objective is linked to the competencies it builds; each day gains "
    "hand-in criteria, versioned resources and a lesson (docs/curriculum/weeks/). Not reconciled and not approved: PROPOSED until the "
    "operator promotes it."
)

WEEKDAY_SECTIONS = [
    ("Recall (15 min)", "recall"),
    ("Theory (45 min)", "theory"),
    ("Guided lab (120 min)", "guided_lab"),
    ("Independent lab (120 min)", "independent_lab"),
    ("Defence and verification (60 min)", "defense_verification"),
    ("Evidence and report (60 min)", "evidence_reporting"),
]
SATURDAY_SECTIONS = [
    ("Blind practical (120 min)", "blind_practical"),
    ("Remediation (60 min)", "remediation"),
    ("Report (60 min)", "report"),
    ("Performance review (60 min)", "performance_review"),
]
OPTIONAL_SECTIONS = ["Watch for", "Extend"]
ASSESSMENT_KINDS = {"BLIND_PRACTICAL", "CAPSTONE", "TRANSFER_CHECK"}

# A block that is one or two lines is a stub, not a lesson.
MIN_WORDS_BLOCK = 25
MAX_OBJECTIVE_CHARS = 500
MAX_ARTIFACT_CHARS = 500
MAX_TASK_STATEMENT_CHARS = 20000
MAX_RESOURCE_VERSION_CHARS = 128  # schemas/curriculum/manifest.schema.json, resource.version
MAX_RESOURCE_NOTE_CHARS = 1000  # resource.license_note
MIN_TARGETS_PER_COMPETENCY = 2

WEEK_RE = re.compile(r"^# Week (\d{1,2}) — (.+?)\s*$")
DAY_RE = re.compile(r"^## (D\d{2}) — (.+?)\s*$")
SECTION_RE = re.compile(r"^### (.+?)\s*$")
HAND_IN_RE = re.compile(r"^\*\*Hand in\.\*\* (.+?)\s*$")
OBJECTIVE_RE = re.compile(r"^(\d)\. (.+?) \[([a-z0-9][a-z0-9.\-]*(?:, ?[a-z0-9][a-z0-9.\-]*)*)\]\s*$")
ASSESSMENT_CODE_RE = re.compile(r"^[A-Z0-9][A-Z0-9-]{0,31}$")

# Characters that must never appear in a source file: controls, soft hyphen, zero-width, bidi, BOM.
HIDDEN = (
    {c for c in range(0x00, 0x20) if c not in (0x09, 0x0A)}
    | {0x7F, 0xAD, 0x061C, 0x180E, 0xFEFF}
    | set(range(0x80, 0xA0))
    | set(range(0x200B, 0x2010))
    | set(range(0x202A, 0x202F))
    | set(range(0x2060, 0x2070))
)


class Problems:
    def __init__(self) -> None:
        self.items: list[str] = []

    def add(self, where: str, message: str) -> None:
        self.items.append(f"{where}: {message}")


def load_seed() -> dict:
    return json.loads(SEED.read_text(encoding="utf-8"))


def load_competencies() -> tuple[dict[str, dict], dict[str, list[str]]]:
    """id -> {name, pack}; id -> prerequisite ids (pack prefixes stripped)."""
    info: dict[str, dict] = {}
    prerequisites: dict[str, list[str]] = {}
    for path in sorted(PACKS.glob("*/pack.json")):
        pack = json.loads(path.read_text(encoding="utf-8"))
        for c in pack["competencies"]:
            info[c["id"]] = {"name": c["name"], "pack": pack["pack_id"], "band": (c["difficulty_band"]["min"], c["difficulty_band"]["max"])}
            prerequisites[c["id"]] = [p.split(":", 1)[-1] for p in c.get("prerequisites", [])]
    return info, prerequisites


def parse_week(path: Path, problems: Problems) -> dict | None:
    text = path.read_text(encoding="utf-8")
    where = path.name
    for i, ch in enumerate(text):
        if ord(ch) in HIDDEN:
            line = text.count("\n", 0, i) + 1
            problems.add(f"{where}:{line}", f"hidden or control character U+{ord(ch):04X}")
            return None
    week: dict | None = None
    day: dict | None = None
    section: str | None = None
    for number, raw in enumerate(text.split("\n"), start=1):
        line = raw.rstrip()
        m = WEEK_RE.match(line)
        if m:
            if week is not None:
                problems.add(f"{where}:{number}", "more than one week heading")
            week = {"number": int(m.group(1)), "title": m.group(2), "days": [], "file": path.name}
            day = None
            section = None
            continue
        m = DAY_RE.match(line)
        if m:
            if week is None:
                problems.add(f"{where}:{number}", "a day before the week heading")
                return None
            day = {"id": m.group(1), "title": m.group(2), "hand_in": None, "sections": {}, "order": [], "line": number}
            week["days"].append(day)
            section = None
            continue
        if day is None:
            continue
        m = HAND_IN_RE.match(line)
        if m and section is None:
            day["hand_in"] = m.group(1)
            continue
        m = SECTION_RE.match(line)
        if m:
            section = m.group(1)
            if section in day["sections"]:
                problems.add(f"{where}:{number}", f"{day['id']}: section '{section}' appears twice")
            day["sections"][section] = []
            day["order"].append(section)
            continue
        if section is not None:
            day["sections"][section].append(line)
    if week is None:
        problems.add(where, "no '# Week N — title' heading")
    return week


def words(lines: list[str]) -> int:
    return len(" ".join(lines).split())


def check_week(week: dict, seed: dict, info: dict[str, dict], problems: Problems) -> dict:
    """Validate one parsed week; return the manifest week and the assessment payloads."""
    n = week["number"]
    where = week["file"]
    seed_week = seed["weeks"][n - 1] if 1 <= n <= 12 else None
    out_days: list[dict] = []
    assessments: list[dict] = []
    if seed_week is None:
        problems.add(where, f"week number {n} is outside 1..12")
        return {"week_number": n, "title": week["title"], "days": []}
    expected_file = f"week-{n:02d}.md"
    if week["file"] != expected_file:
        problems.add(where, f"holds week {n}; the file must be named {expected_file}")
    if week["title"] != seed_week["title"]:
        problems.add(where, f"week title '{week['title']}' differs from Appendix A '{seed_week['title']}'")
    ids = [d["id"] for d in week["days"]]
    seed_ids = [d["day_id"] for d in seed_week["days"]]
    if ids != seed_ids:
        problems.add(where, f"days are {ids}; expected {seed_ids}")
        return {"week_number": n, "title": week["title"], "days": []}
    block_names_weekday = [b["block"] for b in seed["weekday_blocks"]]
    block_names_saturday = [b["block"] for b in seed["saturday_blocks"]]
    if [b for _, b in WEEKDAY_SECTIONS] != block_names_weekday or [b for _, b in SATURDAY_SECTIONS] != block_names_saturday:
        problems.add("seed", "the seed's canonical block lists differ from the section names this script expects")
    for day, seed_day in zip(week["days"], seed_week["days"]):
        d = day["id"]
        at = f"{where} {d}"
        saturday = seed_day["is_assessment_day"]
        sections = day["sections"]
        wanted = SATURDAY_SECTIONS if saturday else WEEKDAY_SECTIONS
        required = ["Objectives"] + [name for name, _ in wanted] + (["Assessment"] if saturday else []) + ["Resources"]
        for name in required:
            if name not in sections:
                problems.add(at, f"missing section '{name}'")
        unknown = [s for s in sections if s not in required and s not in OPTIONAL_SECTIONS]
        if unknown:
            problems.add(at, f"unknown section(s) {unknown}")
        order = [s for s in day["order"] if s in required]
        if order != [s for s in required if s in sections]:
            problems.add(at, f"sections are out of order: {day['order']}")
        for name, _ in wanted:
            if name in sections and words(sections[name]) < MIN_WORDS_BLOCK:
                problems.add(at, f"section '{name}' has {words(sections[name])} words (minimum {MIN_WORDS_BLOCK})")
        # hand-in: starts with the Appendix A artifact name
        artifact = day["hand_in"]
        seed_artifact = seed_day["evidence_artifact"]
        if not artifact:
            problems.add(at, "missing '**Hand in.**' line")
            artifact = seed_artifact
        else:
            if not artifact.startswith(seed_artifact + " — "):
                problems.add(at, f"hand-in must start with the Appendix A artifact '{seed_artifact} — '")
            if len(artifact) > MAX_ARTIFACT_CHARS:
                problems.add(at, f"hand-in is {len(artifact)} characters (maximum {MAX_ARTIFACT_CHARS})")
        # objectives
        objectives: list[dict] = []
        lines = [ln for ln in sections.get("Objectives", []) if ln.strip()]
        if len(lines) != 3:
            problems.add(at, f"{len(lines)} objective lines (exactly 3 are required)")
        for k, ln in enumerate(lines[:3], start=1):
            m = OBJECTIVE_RE.match(ln)
            if not m:
                problems.add(at, f"objective line {k} is not 'N. text [competency, ...]': {ln[:80]}")
                continue
            if int(m.group(1)) != k:
                problems.add(at, f"objective line {k} is numbered {m.group(1)}")
            text = m.group(2)
            ids_ = [c.strip() for c in m.group(3).split(",")]
            seed_text = seed_day["objectives"][k - 1]["text"]
            if not text.startswith(seed_text + " — "):
                problems.add(at, f"objective {k} must start with the Appendix A wording '{seed_text} — '")
            if len(text) > MAX_OBJECTIVE_CHARS:
                problems.add(at, f"objective {k} is {len(text)} characters (maximum {MAX_OBJECTIVE_CHARS})")
            if len(ids_) > 3 or len(set(ids_)) != len(ids_):
                problems.add(at, f"objective {k}: at most 3 distinct competencies, got {ids_}")
            for c in ids_:
                if c not in info:
                    problems.add(at, f"objective {k}: unknown competency '{c}'")
            objectives.append({"seq": k, "objective_id": f"{d}-O{k}", "text": text, "competency_ids": ids_})
        # resources
        resources: list[dict] = []
        for ln in sections.get("Resources", []):
            if not ln.strip():
                continue
            if not ln.startswith("- "):
                problems.add(at, f"resource line does not start with '- ': {ln[:60]}")
                continue
            parts = [p.strip() for p in ln[2:].split(" | ")]
            if len(parts) < 2 or parts[1] not in ("External", "Internal"):
                problems.add(at, f"resource line needs 'ref | External|Internal | version | licence note': {ln[:80]}")
                continue
            ref, kind = parts[0], parts[1]
            version = parts[2] if len(parts) > 2 and parts[2] else None
            note = parts[3] if len(parts) > 3 and parts[3] else None
            if version is not None and len(version) > MAX_RESOURCE_VERSION_CHARS:
                problems.add(at, f"resource version is {len(version)} characters (maximum {MAX_RESOURCE_VERSION_CHARS}); move the explanation to the licence note: {ref[:60]}")
            if note is not None and len(note) > MAX_RESOURCE_NOTE_CHARS:
                problems.add(at, f"resource licence note is {len(note)} characters (maximum {MAX_RESOURCE_NOTE_CHARS}): {ref[:60]}")
            if kind == "External":
                if not ref.startswith("https://") or " " in ref or len(ref) > 2048:
                    problems.add(at, f"external resource must be an https URL without spaces: {ref[:80]}")
                if version is None:
                    problems.add(at, f"external resource needs a version: {ref[:80]}")
            res: dict = {"ref": ref, "kind": kind.upper()}
            if note:
                res["license_note"] = note
            if version:
                res["version"] = version
            resources.append(res)
        if not any(r["kind"] == "EXTERNAL" for r in resources):
            problems.add(at, "at least one external resource is required")
        # the day's own sheet is always a resource of the day; the fragment is a locator
        # (the day id), so check that the file exists and holds that day's heading
        sheet = WEEKS_DIR / week["file"]
        if not sheet.is_file() or not any(l.startswith(f"## {d} — ") for l in sheet.read_text(encoding="utf-8").split("\n")):
            problems.add(at, f"the day sheet locator {week['file']}#{d.lower()} does not resolve to a '## {d} — ' heading")
        resources.append({"ref": f"docs/curriculum/weeks/{week['file']}#{d.lower()}", "kind": "INTERNAL"})
        # saturday assessment
        if saturday:
            payload = parse_assessment(sections.get("Assessment", []), at, info, objectives, problems)
            if payload:
                assessments.append(payload)
        out_days.append(
            {
                "day_id": d,
                "day_of_week": seed_day["day_of_week"],
                "planned_minutes": seed_day["planned_minutes"],
                "objectives": objectives,
                "evidence_artifact": artifact,
                "blocks": seed_day["blocks"],
                "is_assessment_day": seed_day["is_assessment_day"],
                "resources": resources,
            }
        )
    return {"week_number": n, "title": week["title"], "days": out_days, "_assessments": assessments}


def parse_assessment(lines: list[str], at: str, info: dict[str, dict], objectives: list[dict], problems: Problems) -> dict | None:
    fields: dict[str, str] = {}
    body_from = None
    for i, ln in enumerate(lines):
        if ln.strip() == "Task statement:":
            body_from = i + 1
            break
        m = re.match(r"^- (Code|Title|Kind|Competency|Difficulty): (.+)$", ln)
        if m:
            fields[m.group(1)] = m.group(2).strip()
    for key in ("Code", "Title", "Kind", "Competency", "Difficulty"):
        if key not in fields:
            problems.add(at, f"assessment: missing '- {key}:'")
    if body_from is None:
        problems.add(at, "assessment: missing 'Task statement:'")
        return None
    task = "\n".join(lines[body_from:]).strip()
    if not task:
        problems.add(at, "assessment: empty task statement")
        return None
    if len(task) > MAX_TASK_STATEMENT_CHARS:
        problems.add(at, f"assessment: task statement is {len(task)} characters (maximum {MAX_TASK_STATEMENT_CHARS})")
    if any(k not in fields for k in ("Code", "Title", "Kind", "Competency", "Difficulty")):
        return None
    if not ASSESSMENT_CODE_RE.match(fields["Code"]):
        problems.add(at, f"assessment: code '{fields['Code']}' does not match ^[A-Z0-9][A-Z0-9-]{{0,31}}$")
    if fields["Kind"] not in ASSESSMENT_KINDS:
        problems.add(at, f"assessment: kind '{fields['Kind']}' is not one of {sorted(ASSESSMENT_KINDS)}")
    if fields["Competency"] not in info:
        problems.add(at, f"assessment: unknown competency '{fields['Competency']}'")
    elif not any(fields["Competency"] in o["competency_ids"] for o in objectives):
        problems.add(at, f"assessment: competency '{fields['Competency']}' is not linked from any objective of its day")
    try:
        difficulty = int(fields["Difficulty"])
        if not 1 <= difficulty <= 5:
            raise ValueError
    except ValueError:
        problems.add(at, f"assessment: difficulty '{fields['Difficulty']}' must be an integer 1..5")
        difficulty = 3
    band = info.get(fields["Competency"], {}).get("band")
    if band and not band[0] <= difficulty <= band[1]:
        problems.add(at, f"assessment: difficulty {difficulty} is outside the difficulty band {band[0]}..{band[1]} of '{fields['Competency']}'")
    if len(fields["Title"]) > 200:
        problems.add(at, "assessment: title longer than 200 characters")
    return {
        "code": fields["Code"],
        "title": fields["Title"],
        "kind": fields["Kind"],
        "competency_id": fields["Competency"],
        "task_statement": task,
        "difficulty": difficulty,
    }


def build_manifest(seed: dict, weeks: list[dict]) -> dict:
    return {
        "manifest_schema_version": seed["manifest_schema_version"],
        "curriculum_id": seed["curriculum_id"],
        "name": seed["name"],
        "version": VERSION,
        "provenance": "PROPOSED_SEED",
        "provenance_note": PROVENANCE_NOTE,
        "structure": seed["structure"],
        "weekday_blocks": seed["weekday_blocks"],
        "saturday_blocks": seed["saturday_blocks"],
        "weeks": [{"week_number": w["week_number"], "title": w["title"], "days": w["days"]} for w in weeks],
    }


def cross_checks(manifest: dict, info: dict[str, dict], prerequisites: dict[str, list[str]], problems: Problems) -> dict:
    """Coverage and prerequisite order over the whole programme. Returns the data coverage.md needs."""
    targets: dict[str, list[tuple[int, int, int]]] = {c: [] for c in info}
    for w in manifest["weeks"]:
        for day_index, day in enumerate(w["days"]):
            absolute = (w["week_number"] - 1) * 6 + day_index
            for o in day["objectives"]:
                for c in o["competency_ids"]:
                    if c in targets:
                        targets[c].append((w["week_number"], absolute, o["seq"]))
    for c, hits in sorted(targets.items()):
        if len(hits) < MIN_TARGETS_PER_COMPETENCY:
            problems.add("coverage", f"competency '{c}' is targeted by {len(hits)} objective(s); at least {MIN_TARGETS_PER_COMPETENCY} are required")
    for c, hits in sorted(targets.items()):
        if not hits:
            continue
        first_c = min((h[1], h[2]) for h in hits)
        for p in prerequisites.get(c, []):
            ph = targets.get(p, [])
            if not ph:
                continue
            first_p = min((h[1], h[2]) for h in ph)
            if first_p > first_c:
                problems.add("prerequisites", f"'{c}' is first taught at day index {first_c[0] + 1} before its prerequisite '{p}' (day index {first_p[0] + 1})")
    return targets


def coverage_markdown(manifest: dict, seed: dict, info: dict[str, dict], targets: dict[str, list[tuple[int, int, int]]], assessments: list[dict]) -> str:
    lines = [
        "# Curriculum coverage (generated)",
        "",
        "Generated by `python3 -I scripts/build_curriculum.py --write` from `docs/curriculum/weeks/`. Do not edit by hand.",
        "",
        f"Manifest `{manifest['curriculum_id']}` version `{manifest['version']}`, provenance `{manifest['provenance']}` (nothing here is approved).",
        "",
        "## Competency by week",
        "",
        "Each cell counts the objectives of that week that build the competency. Every competency must be built by at least two objectives, and no competency may be first taught before its prerequisites (both checked by the build).",
        "",
        "| Pack | Competency | " + " | ".join(f"W{n}" for n in range(1, 13)) + " | Total |",
        "|---|---|" + "|".join(["---"] * 12) + "|---|",
    ]
    for c in sorted(info, key=lambda k: (info[k]["pack"], k)):
        per_week = [sum(1 for h in targets[c] if h[0] == n) for n in range(1, 13)]
        cells = " | ".join(str(x) if x else "" for x in per_week)
        lines.append(f"| {info[c]['pack']} | `{c}` | {cells} | {sum(per_week)} |")
    lines += ["", "## Saturday assessments", "", "| Day | Code | Kind | Competency | Difficulty |", "|---|---|---|---|---|"]
    for w in manifest["weeks"]:
        for day in w["days"]:
            if day["is_assessment_day"]:
                a = next((x for x in assessments if x["_day"] == day["day_id"]), None)
                if a:
                    lines.append(f"| {day['day_id']} | `{a['code']}` | {a['kind']} | `{a['competency_id']}` | {a['difficulty']} |")
    seed_objectives = [o for w in seed["weeks"] for d in w["days"] for o in d["objectives"]]
    new_objectives = [o for w in manifest["weeks"] for d in w["days"] for o in d["objectives"]]
    links = sum(len(o["competency_ids"]) for o in new_objectives)
    external = sum(1 for w in manifest["weeks"] for d in w["days"] for r in d["resources"] if r["kind"] == "EXTERNAL")
    distinct = {r["ref"] for w in manifest["weeks"] for d in w["days"] for r in d["resources"] if r["kind"] == "EXTERNAL"}
    extended = sum(1 for a, b in zip(seed_objectives, new_objectives) if a["text"] != b["text"])
    lines += [
        "",
        "## Changes against the Appendix A seed (0.1.0-seed)",
        "",
        "| What | Count |",
        "|---|---|",
        f"| Weeks, days, objectives (unchanged structure) | {len(manifest['weeks'])}, {sum(len(w['days']) for w in manifest['weeks'])}, {len(new_objectives)} |",
        f"| Objectives whose text gained a success criterion (the Appendix A wording is kept as its start) | {extended} |",
        f"| Objective to competency links | {links} |",
        f"| Day hand-ins with criteria | {sum(1 for a, b in zip((d for w in seed['weeks'] for d in w['days']), (d for w in manifest['weeks'] for d in w['days'])) if a['evidence_artifact'] != b['evidence_artifact'])} |",
        f"| External resource references, distinct URLs | {external}, {len(distinct)} |",
        "",
        "External resources carry the version or edition the author believed current and a licence note; every one is **ASSUMED** until the operator verifies it (`docs/curriculum/provenance-and-reconciliation.md`, resource register).",
        "",
    ]
    return "\n".join(lines)


def dump(obj: object) -> str:
    return json.dumps(obj, indent=2, ensure_ascii=False) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--write", action="store_true", help="write the generated files")
    ap.add_argument("--allow-partial", action="store_true", help="validate only the weeks that exist; write nothing")
    args = ap.parse_args()
    if not SEED.is_file() or not PACKS.is_dir():
        sys.stderr.write("error: seed manifest or competency packs not found\n")
        return 2
    seed = load_seed()
    info, prerequisites = load_competencies()
    problems = Problems()
    files = sorted(WEEKS_DIR.glob("week-*.md")) if WEEKS_DIR.is_dir() else []
    weeks: list[dict] = []
    for path in files:
        parsed = parse_week(path, problems)
        if parsed is not None:
            weeks.append(check_week(parsed, seed, info, problems))
    weeks.sort(key=lambda w: w["week_number"])
    numbers = [w["week_number"] for w in weeks]
    full = numbers == list(range(1, 13))
    if not full and not args.allow_partial:
        problems.add("weeks", f"weeks present: {numbers}; all 12 are required (use --allow-partial while authoring)")
    # attach the day id to each assessment (one per Saturday)
    assessments = []
    for w in weeks:
        saturday = next((d for d in w["days"] if d["is_assessment_day"]), None)
        for a in w.get("_assessments", []):
            assessments.append(dict(a, _day=saturday["day_id"] if saturday else ""))
    codes = [a["code"] for a in assessments]
    if len(set(codes)) != len(codes):
        problems.add("assessments", f"duplicate assessment codes: {sorted(c for c in set(codes) if codes.count(c) > 1)}")
    manifest = build_manifest(seed, weeks)
    if full:
        targets = cross_checks(manifest, info, prerequisites, problems)
    else:
        targets = {c: [] for c in info}
        # informative only: how many objectives of the weeks present link each competency
        for w in weeks:
            for d in w["days"]:
                for o in d["objectives"]:
                    for c in o["competency_ids"]:
                        if c in targets:
                            targets[c].append((w["week_number"], 0, o["seq"]))
    if problems.items:
        for p in problems.items:
            print(f"FAIL  {p}")
        print(f"\n{len(problems.items)} problem(s) in {len(weeks)} week file(s).")
        return 1
    print(f"ok    {len(weeks)} week file(s) valid: weeks {numbers}; {sum(len(w['days']) for w in weeks)} days; {sum(len(d['objectives']) for w in weeks for d in w['days'])} objectives; {len(assessments)} assessment(s)")
    if not full:
        linked = sum(1 for c in targets if targets[c])
        print(f"      partial run: {linked} of {len(info)} competencies linked so far; nothing written")
        return 0
    outputs: dict[Path, str] = {OUT_MANIFEST: dump(manifest)}
    for a in assessments:
        payload = {k: v for k, v in a.items() if not k.startswith("_")}
        outputs[OUT_ASSESSMENTS / f"{a['code']}.json"] = dump(payload)
    outputs[OUT_COVERAGE] = coverage_markdown(manifest, seed, info, targets, assessments)
    stale = []
    for path, content in outputs.items():
        current = path.read_text(encoding="utf-8") if path.is_file() else None
        if current != content:
            stale.append(path)
            if args.write:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")
    # an assessment file for a code that no longer exists is stale too
    if OUT_ASSESSMENTS.is_dir():
        for path in sorted(OUT_ASSESSMENTS.glob("*.json")):
            if path not in outputs:
                stale.append(path)
                if args.write:
                    path.unlink()
    if args.write:
        print(f"wrote {len(stale)} file(s)" if stale else "nothing to write: generated files are current")
        return 0
    if stale:
        for p in stale:
            print(f"FAIL  {p.relative_to(REPO)} is out of date; run: python3 -I scripts/build_curriculum.py --write")
        return 1
    print("ok    generated files are current")
    return 0


if __name__ == "__main__":
    sys.exit(main())
