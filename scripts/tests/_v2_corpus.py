"""A small synthetic v2.0 courseware, written from scratch for the tests of scripts/build_curriculum_v2.py.

Standard library only. Nothing here is courseware: every text is deterministic filler built from a fixed vocabulary
and the identifiers it must carry, so that a corpus follows docs/curriculum/v2/AUTHORING.md rule by rule and a test
can break exactly one rule at a time. The seed texts of the objectives and hand-ins come from the plan copy in the
temporary tree, read here by a parser of its own (the builder's parser is a thing under test, not a helper).

    plan = read_plan(root)                       # {"seeds": {n: [A, B, C, D]}, "titles": {w: title}}
    write_week(root, plan, 1)                    # a complete, valid week 1 (files of AUTHORING.md, no generated files)
    write_corpus(root, plan)                     # all 12 weeks
"""

from __future__ import annotations

import datetime
import hashlib
import json
import re
from pathlib import Path

START = datetime.date(2026, 11, 16)
WEEKDAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"]
BLOCK_MINUTES = {"R": 30, "A": 120, "B": 120, "C": 150, "D": 60}
PLAN_REL = "docs/curriculum/v2/MASTER_PLAN_V2.0.md"
WEEKS_REL = "docs/curriculum/v2/WEEKS"
PACKS_REL = "data/competency-packs"

VOCAB = (
    "record scope boundary baseline evidence control owner listing account service signal sample compare change reading "
    "limit target guest range network review step result trace mode path value source check claim fact limit group"
).split()

# competency -> (min, max) difficulty band, read from the packs at run time
EXAM_COMPETENCIES = ["lab.authorization", "net.layers-addressing", "win.principals-services", "web.http-semantics"]


def day_date(n: int) -> datetime.date:
    return START + datetime.timedelta(days=7 * ((n - 1) // 5) + (n - 1) % 5)


def prose(tag: str, words: int) -> str:
    """Deterministic filler of exactly `words` words, in sentences; `tag` seeds it so that two texts differ."""
    start = int(hashlib.sha256(tag.encode()).hexdigest()[:8], 16)
    out: list[str] = []
    for i in range(words):
        out.append(VOCAB[(start + 5 * i + i * i // 3) % len(VOCAB)])
    sentences = []
    for k in range(0, words, 11):
        chunk = out[k : k + 11]
        chunk[0] = chunk[0].capitalize()
        sentences.append(" ".join(chunk) + ".")
    return " ".join(sentences)


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


# ---------------------------------------------------------------------------------------------------
# the plan and the packs (read-only inputs, copied into the temporary tree by the test)
# ---------------------------------------------------------------------------------------------------


def read_plan(root: Path) -> dict:
    """The seed texts (columns A to D, emphasis markers removed, whitespace collapsed) and the week titles of section 5."""
    seeds: dict[int, list[str]] = {}
    titles: dict[int, str] = {}
    in_five = False
    for line in (root / PLAN_REL).read_text(encoding="utf-8").split("\n"):
        if line.startswith("# 5. "):
            in_five = True
            continue
        if in_five and line.startswith("# "):
            break
        if not in_five:
            continue
        m = re.match(r"^## WEEK (\d{2}) — (.+?)\s*$", line)
        if m:
            titles[int(m.group(1))] = " ".join(m.group(2).split())
        m = re.match(r"^\| \*\*(\d{2}) [A-Za-z]{3} \d{2}\*\* \|(.*)\|\s*$", line)
        if m:
            seeds[int(m.group(1))] = [" ".join(c.replace("*", "").split()) for c in m.group(2).split("|")]
    assert len(seeds) == 60 and len(titles) == 12, (len(seeds), len(titles))
    return {"seeds": seeds, "titles": titles}


def read_competencies(root: Path) -> dict[str, tuple[int, int]]:
    out: dict[str, tuple[int, int]] = {}
    for path in sorted((root / PACKS_REL).glob("*/pack.json")):
        for c in json.loads(path.read_text(encoding="utf-8"))["competencies"]:
            out[c["id"]] = (c["difficulty_band"]["min"], c["difficulty_band"]["max"])
    return out


# ---------------------------------------------------------------------------------------------------
# identifiers
# ---------------------------------------------------------------------------------------------------


def week_of(n: int) -> int:
    return (n - 1) // 5 + 1


def wn(w: int) -> str:
    return f"W{w:02d}"


def days_of(w: int) -> list[int]:
    return list(range(5 * w - 4, 5 * w + 1))


def recall_ids(n: int) -> list[str]:
    return [f"W{week_of(n):02d}D{n:02d}-R{k}" for k in range(1, 6)]


def applied_ids(n: int) -> list[str]:
    return [f"W{week_of(n):02d}D{n:02d}-A{k}" for k in range(1, 4)]


def exam_ids(w: int) -> list[str]:
    return [f"W{w:02d}X-Q{k:02d}" for k in range(1, 21)]


def exam_code(w: int) -> str:
    return "V2-W12-CAPSTONE" if w == 12 else f"V2-W{w:02d}-EXAM"


# ---------------------------------------------------------------------------------------------------
# the question texts and their keys (unique across the corpus)
# ---------------------------------------------------------------------------------------------------


def recall_text(n: int, k: int) -> str:
    return f"Without notes, explain what observation of day {n} would show that control number {k} of week {week_of(n)} held, and what you would record."


def applied_text(n: int, k: int) -> str:
    return f"Scenario {n}.{k}: a synthetic guest of week {week_of(n)} gives result number {k} on day {n}; state your conclusion and the test that would falsify it."


def day_answer(n: int, ident: str) -> str:
    return f"Reference answer for {ident}: a dated listing of the synthetic guest with owner and mode, kept as the record of day {n}."


def exam_kind(q: int) -> str:
    return "CONCEPT" if q <= 6 else "INTERPRET" if q <= 12 else "CONTROL" if q <= 16 else "SYNTHESIS"


def exam_is_mcq(q: int) -> bool:
    return q <= 8


def exam_question(w: int, q: int) -> str:
    kind = exam_kind(q)
    ident = f"W{w:02d}X-Q{q:02d}"
    if kind == "INTERPRET":
        text = f"Sample (synthetic): `auth failure for synthetic-user number {q} from 10.77.10.{q} in week {w}`. State two facts this line proves and one it does not."
    elif kind == "CONCEPT":
        text = f"Explain concept number {q} of week {w} and state one case in which it does not apply."
    elif kind == "CONTROL":
        text = f"Name the control that would stop the exposure of case {q} in week {w} and the evidence that it works."
    else:
        text = f"Combine the findings of case {q} in week {w} into one prioritised statement for a reader who has not seen the lab."
    return f"{q}. [{ident}] ({kind}) {text}" + (exam_options(w, q) if exam_is_mcq(q) else "")


def exam_options(w: int, q: int) -> str:
    """The four option lines of a multiple-choice question (each starts with a line break)."""
    return "".join(f"\n   {letter}. Option {name} for question {q} of week {w}" for letter, name in zip("ABCD", ["alpha", "bravo", "charlie", "delta"]))


DAY_MARKING = "one point for the record, one for the limit, none for a claim without evidence"
DAY_OVERCLAIM = "Claiming the guest is secure after one successful check."
PRACTICAL_KEY = (
    "Knowledge 20 points, practical 40 points, defensive validation 25 points, evidence and reporting 15 points; "
    "bands are 0 to 49 percent, 50 to 79 percent and 80 to 100 percent of each part."
)
GATE_KEY = "Any action outside the written scope ends the attempt; any altered evidence fails the integrity gate."


def day_challenge(n: int) -> str:
    return f"The reference observations of day {n} are a dated listing and a denied read; score each out of five."


def day_hint4(n: int) -> str:
    return f"The mode on the directory of day {n} decides the read; explain it after the attempt."


def exam_why(w: int, q: int) -> str:
    return f"the owner of the boundary decides, as case {q} of week {w} shows."


def exam_distractors(w: int, q: int) -> str:
    return f"A is wrong because it names the wrong owner in question {q}; C is wrong because it skips the record; D is wrong because it claims too much."


def labs_key_body(w: int, name: str) -> str:
    return prose(f"{name}key{w}", 30)


def exam_answer(w: int, q: int) -> str:
    if exam_is_mcq(q):
        return f"B. The statement bravo for question {q} of week {w} holds because the synthetic control owns the boundary."
    return f"Reference answer for W{w:02d}X-Q{q:02d}: the boundary of case {q} in week {w} belongs to one control, shown by a dated record."


# ---------------------------------------------------------------------------------------------------
# a day guide
# ---------------------------------------------------------------------------------------------------


def day_competencies(n: int, known: list[str]) -> list[list[str]]:
    """Three competency lists of the objectives: three, one and zero ids (all of them exist in the packs)."""
    base = (n * 3) % len(known)
    return [[known[(base + i) % len(known)] for i in range(3)], [known[(base + 3) % len(known)]], []]


def block_body(n: int, letter: str, friday: bool) -> str:
    words = 70 if letter == "R" else 140
    text = prose(f"day{n}{letter}", words)
    if friday:
        if letter == "A" and n != 55:
            text += " The knowledge exam is in ASSESSMENT.md, Part 1."
        elif letter == "C":
            text += " The scored practical is in ASSESSMENT.md, Parts 2 and 3."
        elif letter == "D":
            text += " The knowledge exam is in ASSESSMENT.md, Part 1; scoring and the report are in ASSESSMENT.md, Part 4." if n == 55 else " Scoring and the report are in ASSESSMENT.md, Part 4."
    return text


def day_guide(plan: dict, n: int, known: list[str]) -> str:
    w = week_of(n)
    friday = n % 5 == 0
    s = plan["seeds"][n]
    comps = day_competencies(n, known)
    r_ids, a_ids = recall_ids(n), applied_ids(n)
    lines = [
        f"# G9 {wn(w)} / DAY_{n:02d} — Synthetic topic {n:02d}",
        "",
        "- Status: PROPOSED",
        f"- Day id: D{n:02d}",
        f"- Original calendar date: {day_date(n).isoformat()} ({WEEKDAYS[(n - 1) % 5]})",
        "- Duration: 8 instructional hours (R 0.5 h; A 2 h; B 2 h; C 2.5 h; D 1 h)",
        f"- Lab authorization: the synthetic range 10.77.10.0/24 RED only, owned guests, written scope SCOPE-{wn(w)}-{n:02d}",
        "- Dependencies: no earlier day; a shell and an editor; the free access tier of one provider",
        f"- Primary provider: Synthetic Provider {w} | Secondary: none",
        "",
        "## Objectives",
    ]
    for k in (1, 2, 3):
        lines.append(f"{k}. {s[k - 1]} — the learner shows one dated observation number {k} for day {n}. [{', '.join(comps[k - 1])}]")
    lines += [
        "",
        "## Hand-in",
        f"{s[3]} — one file a stranger can check against three stated acceptance criteria for day {n}.",
        "",
        "## Resources",
        "| Id | Use in this day | Block |",
        "|---|---|---|",
        f"| w{w:02d}-synthetic-docs | Read the canonical page and keep its title and date as evidence | A |",
        f"| `w{w:02d}-synthetic-module` | Do the exercise the module names and keep the completion record | C |",
        "",
    ]
    for letter in "RABCD":
        title = "Active recall" if letter == "R" else f"Synthetic block {letter}"
        lines += [f"## {letter} — {title} ({BLOCK_MINUTES[letter]} min)", block_body(n, letter, friday), ""]
    lines += ["## Commands proposed for review", "| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |", "|---|---|---|---|---|---|---|---|---|"]
    if not friday:
        for k in (1, 2, 3):
            lines.append(f"| {k} | `ls -l /srv/lab{k}` | RED guest {k} | list one directory | Sample (synthetic): a listing | none | user | RED only | none needed (read-only) |")
    lines += [
        "",
        "## Offense, detection, remediation, retest",
        "| Offensive mechanism | Defensive observation | Remediation | Retest |",
        "|---|---|---|---|",
        f"| a permission left open on guest {n} | the mode shows in the listing | set the least privilege | list again and try the denied read |",
        "",
        "## Quiz",
        "### Recall (closed book)",
    ]
    lines += [f"{k}. [{r_ids[k - 1]}] {recall_text(n, k)}" for k in range(1, 6)]
    lines += ["### Applied"]
    lines += [f"{k}. [{a_ids[k - 1]}] {applied_text(n, k)}" for k in range(1, 4)]
    lines += [
        "",
        "## Hint ladder",
        f"1. No hint: try the task alone for 45 minutes and note what you tried on day {n}.",
        "2. Conceptual hint: ask which boundary the task crosses and who owns it.",
        "3. Scoped technical hint: read the mode and the owner of the one directory in scope.",
        "4. Instructor explanation: in the answer key, after the attempt is submitted.",
        "",
        "## Troubleshooting and rollback",
        "- Expected: the listing shows the owner and the mode you set.",
        "- Missing dependency: install nothing; ask for approval and use the hosted fallback.",
        "- No connectivity: stay on the local synthetic guest and record the failure.",
        "- Unexpected result: stop, record the output and do not retry blindly.",
        "- Safety breach: stop at once, record the time and tell the owner.",
        "- Restoration: return the synthetic directory to its baseline mode.",
        "- Time exceeded: stop at the block limit and write down what is left.",
        "",
        "## Evidence and acceptance",
        f"- Evidence: the dated listing of day {n}",
        "- Grading: each acceptance criterion is met or not met",
        "- Negative test: try the denied read as a second user and show it fails",
        "",
        "## Expert extension (optional)",
        f"Extend the exercise of day {n} with a second guest; drop it without loss.",
        "",
        "## Spaced recall",
        f"- 1 day: {r_ids[0]}, {r_ids[1]}",
        f"- 3 days: {r_ids[2]}",
        f"- 7 days: {r_ids[3]}",
        f"- 14 days: {r_ids[4]}",
        "",
        "## Reflection",
        "- What was observed and what was inferred?",
        "- What was not proven?",
        "- Which test would falsify the leading hypothesis?",
        "",
    ]
    return "\n".join(lines)


def day_key(n: int) -> str:
    w = week_of(n)
    lines = [f"# Instructor key — G9 {wn(w)} / DAY_{n:02d}", "", "Instructor only. Not part of the learner's view.", "", "## Quiz key"]
    for ident in recall_ids(n) + applied_ids(n):
        lines += [
            f"### [{ident}]",
            f"- Answer: {day_answer(n, ident)}",
            f"- Marking: {DAY_MARKING}",
            f"- Source: w{w:02d}-synthetic-docs",
            "",
        ]
    lines += [
        "## Independent challenge: reference observations and scoring",
        day_challenge(n),
        "",
        "## Hint 4: instructor explanation",
        day_hint4(n),
        "",
        "## Overclaims to reject",
        f"- {DAY_OVERCLAIM}",
        "",
    ]
    return "\n".join(lines)


# ---------------------------------------------------------------------------------------------------
# the Friday assessment and its key
# ---------------------------------------------------------------------------------------------------


def exam_competency(w: int, bands: dict[str, tuple[int, int]]) -> tuple[str, int]:
    c = EXAM_COMPETENCIES[w % len(EXAM_COMPETENCIES)]
    return c, bands[c][0]


def assessment(w: int, bands: dict[str, tuple[int, int]]) -> str:
    comp, difficulty = exam_competency(w, bands)
    kind = "CAPSTONE" if w == 12 else "BLIND_PRACTICAL"
    lines = [
        f"# G9 {wn(w)} — Friday assessment (day {5 * w:02d})",
        "",
        f"- Code: {exam_code(w)}",
        f"- Kind: {kind}",
        f"- Competency: {comp}",
        f"- Difficulty: {difficulty}",
        f"- Title: Week {w} synthetic assessment",
        "",
        "## Rules",
        prose(f"rules{w}", 40),
        "",
        "## Part 1 — Knowledge exam (20 questions, 20 points)",
    ]
    lines += [exam_question(w, q) for q in range(1, 21)]
    lines += [
        "",
        "## Part 2 — Practical (40 points)",
        prose(f"p2{w}", 60),
        "",
        "## Part 3 — Defensive validation (25 points)",
        prose(f"p3{w}", 60),
        "",
        "## Part 4 — Evidence and reporting (15 points)",
        prose(f"p4{w}", 60),
        "",
        "## Remediation track",
        prose(f"rem{w}", 40),
        "",
        "## Submission",
        prose(f"sub{w}", 40),
        "",
    ]
    return "\n".join(lines)


def assessment_key(w: int) -> str:
    lines = [f"# Instructor key — G9 {wn(w)} Friday assessment", "", "Instructor only. Not part of the learner's view.", ""]
    for q in range(1, 21):
        ident = f"W{w:02d}X-Q{q:02d}"
        lines += [f"### [{ident}]", f"- Answer: {exam_answer(w, q)}", f"- Why: {exam_why(w, q)}"]
        if exam_is_mcq(q):
            lines.append(f"- Distractors: {exam_distractors(w, q)}")
        lines.append("")
    lines += [
        "## Practical: fixture map and scoring",
        PRACTICAL_KEY,
        "",
        "## Safety and integrity gate",
        GATE_KEY,
        "",
    ]
    return "\n".join(lines)


# ---------------------------------------------------------------------------------------------------
# week guide, labs, fixtures, support files, registry
# ---------------------------------------------------------------------------------------------------


def week_guide(plan: dict, w: int) -> str:
    lines = [
        f"# G9 {wn(w)} — {plan['titles'][w]}",
        "",
        f"**Purpose.** {prose(f'purpose{w}', 30)}",
        "",
        f"**Lab stack.** {prose(f'stack{w}', 20)}",
        "",
        f"**Competencies.** {prose(f'comp{w}', 20)}",
        "",
        f"**Accelerator.** {prose(f'acc{w}', 20)}",
        "",
    ]
    for name in ["Goals", "Prerequisites", "Days", "Tools", "Risks and safety", "Pass conditions", "Remediation"]:
        lines += [f"## {name}", prose(f"{name}{w}", 25), ""]
    return "\n".join(lines)


def lab(w: int, name: str) -> str:
    sections = ["Why it matters", "Scope and authorization", "Environment", "Preconditions"]
    if name == "GUIDED_LAB.md":
        sections.append("Steps")
    sections += ["Positive validation", "Negative and safety validation", "Evidence", "Rollback and cleanup"]
    lines = [f"# {wn(w)} {name[:-3].replace('_', ' ').title()}", ""]
    for s in sections:
        lines += [f"## {s}", prose(f"{name}{s}{w}", 25), ""]
    return "\n".join(lines)


def labs_key(w: int) -> str:
    lines = [f"# Instructor key — G9 {wn(w)} labs", "", "Instructor only.", ""]
    for name in ("GUIDED_LAB", "INDEPENDENT_LAB", "FRIDAY_PRACTICAL"):
        lines += [f"## {name}: reference observations and scoring", labs_key_body(w, name), ""]
    return "\n".join(lines)


def evidence_checklist(w: int) -> str:
    return "\n".join([f"# {wn(w)} evidence checklist", "", prose(f"ev{w}", 50), ""])


def registry(w: int) -> dict:
    prefix = f"w{w:02d}"

    def entry(ident: str, kind: str, block: str, fallback: str | None) -> dict:
        return {
            "id": f"{prefix}-{ident}",
            "url": f"https://example.com/g9-test/{prefix}/{ident}",
            "provider": f"Synthetic Provider {w}",
            "title": f"Synthetic {ident} of week {w}",
            "kind": kind,
            "access": "FREE_ACCOUNT",
            "access_note": "A free account is assumed for this synthetic entry.",
            "verification": "SEARCH_CHECKED",
            "evidence": "Synthetic: no search was made; this entry exists for the tests of the builder.",
            "version": "synthetic 1.0",
            "topics": ["synthetic"],
            "suggested_days": [f"D{n:02d}" for n in days_of(w)],
            "block": block,
            "estimate_minutes": 120,
            "difficulty": None,
            "fallback": fallback,
            "notes": "Synthetic entry.",
        }

    return {
        "week": w,
        "checked_on": "2026-11-18",
        "method": "Synthetic registry written by the test helper; no page was opened.",
        "entries": [
            entry("synthetic-docs", "DOCS", "A", None),
            entry("synthetic-module", "MODULE", "C", f"{prefix}-synthetic-docs"),
            entry("synthetic-lab", "LAB", "B", "A description of a local substitute."),
        ],
        "plan_fact_checks": [{"claim": f"Synthetic claim of week {w}.", "result": "CONFIRMED", "evidence": "Synthetic."}],
        "gaps": [f"Synthetic gap of week {w}."],
    }


def fixture_manifest(path: Path) -> str:
    return "\n".join(
        [
            "# Fixtures of the week",
            "",
            "| File | SHA-256 | Description |",
            "|---|---|---|",
            f"| `auth.log` | {sha256_file(path)} | Synthetic failed-login lines. |",
            "",
        ]
    )


# ---------------------------------------------------------------------------------------------------
# writing
# ---------------------------------------------------------------------------------------------------


def put(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def write_week(root: Path, plan: dict, w: int) -> None:
    """Every authored file of week w (AUTHORING.md); the generated QUIZZES/ files are the builder's."""
    known = sorted(read_competencies(root))
    bands = read_competencies(root)
    base = root / WEEKS_REL / wn(w)
    put(base / "RESOURCES.json", json.dumps(registry(w), indent=2) + "\n")
    put(base / "WEEK_GUIDE.md", week_guide(plan, w))
    for n in days_of(w):
        put(base / f"DAY_{n:02d}.md", day_guide(plan, n, known))
        put(base / "ANSWER_KEYS" / f"DAY_{n:02d}_KEY.md", day_key(n))
    for name in ("GUIDED_LAB.md", "INDEPENDENT_LAB.md", "FRIDAY_PRACTICAL.md"):
        put(base / "LABS" / name, lab(w, name))
    fixture = base / "LABS" / "fixtures" / "auth.log"
    put(fixture, f"Sample (synthetic): Nov 16 10:00:00 guest sshd[1]: Failed password for synthetic-user-{w} from 10.77.10.5\n")
    put(base / "LABS" / "fixtures" / "MANIFEST.md", fixture_manifest(fixture))
    put(base / "ASSESSMENT.md", assessment(w, bands))
    put(base / "EVIDENCE_CHECKLIST.md", evidence_checklist(w))
    put(base / "ANSWER_KEYS" / "ASSESSMENT_KEY.md", assessment_key(w))
    put(base / "ANSWER_KEYS" / "LABS_KEY.md", labs_key(w))


def write_corpus(root: Path, plan: dict, weeks: range | list[int] = range(1, 13)) -> None:
    for w in weeks:
        write_week(root, plan, w)
