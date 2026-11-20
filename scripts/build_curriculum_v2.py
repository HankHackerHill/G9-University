#!/usr/bin/env python3
"""build_curriculum_v2.py: the v2.0 courseware (60 weekdays, ADR-0023), checked, and turned into its importable forms.

Sources (never edited by this script):
  docs/curriculum/v2/MASTER_PLAN_V2.0.md          the owner's plan; the day tables of its section 5 are the seed
  docs/curriculum/v2/WEEKS/Wnn/                   one folder per week, in the formats of docs/curriculum/v2/AUTHORING.md
  data/competency-packs/*/pack.json               the competency ids and difficulty bands

Generated (never edited by hand):
  data/curriculum/g9-cyber-mastery-480.seed.v2.json    the plan's skeleton (manifest format 1.1.0)
  data/curriculum/g9-cyber-mastery-480.draft.v2.json   the enriched manifest, provenance PROPOSED_SEED
  data/curriculum/g9-cyber-mastery-480.sheets.v2.json  the lesson sheets (schemas/curriculum/lesson-sheets.schema.json)
  data/curriculum/g9-cyber-mastery-480.keys.v2.json    the answer keys, instructor material (schemas/curriculum/answer-keys.schema.json)
  data/curriculum/assessments-v2/<code>.json           one `assessment.define` payload per Friday
  docs/curriculum/v2/WEEKS/Wnn/QUIZZES/DAY_NN_QUIZ.md  the eight questions of a day, without keys
  docs/curriculum/v2/SCHEDULE_60_DAYS.md, CURRICULUM_MAP.md, RESOURCE_REGISTRY.md
  docs/curriculum/v2/ASSESSMENTS/QUESTION_BANK.md, ANSWER_KEY.md (instructor), SPACED_RECALL.md

Usage (stdlib only; run from anywhere):
    python3 -I scripts/build_curriculum_v2.py                  check all 12 weeks and require the generated files to be current
    python3 -I scripts/build_curriculum_v2.py --week 3         check week 3 only (repeatable); generated files are not compared
    python3 -I scripts/build_curriculum_v2.py --available      check every week whose folder holds all five day guides; writes nothing
    python3 -I scripts/build_curriculum_v2.py --write          check all 12 weeks and (re)write the generated files
    python3 -I scripts/build_curriculum_v2.py --seed-text D11  print the four seed texts of a day
    python3 -I scripts/build_curriculum_v2.py --print-seed     print the seed manifest (the plan alone) on standard output

Exit codes: 0 valid (and current); 1 a rule is broken or a generated file is out of date; 2 usage or environment error
(a missing plan or pack folder, a competency pack that cannot be read).

Keys stay out of the learner's files (ADR-0023 decision 6): the text of every key field of every week (answer, marking, rationale,
distractors, reference observations, fourth hint, overclaims, practical, gate, lab key) is compared, as runs of eight words, with every
learner file and every string of a registry; a line that begins with an answer label in any of its forms, a key line, and a question that
states or marks its answer are refused too. Not enforced: a key that is paraphrased, a word inserted into every run, an answer in a
table cell ('| Answer | B |'), real host names, organizations or personal names. Credential-like tokens, personal e-mail addresses, URLs
with a password and IPv4 addresses the plan does not name (its lab 10.77.0.0/16 and the host networks it says to avoid; loopback, netmasks,
the documentation ranges and the reserved blocks by prefix are allowed too) are refused in every file of a week.
Generated files are written in place only: a symbolic link, or a folder where a file belongs, stops the run before anything is written.

Nothing is approved by building: the output is PROPOSED until the operator promotes it. Every command in the
courseware is proposed text; this script reads files and writes the generated ones, and runs nothing else.
"""

from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import re
import sys
import unicodedata
from pathlib import Path
from typing import NamedTuple

REPO = Path(__file__).resolve().parent.parent
V2_REL = "docs/curriculum/v2"
V2_DIR = REPO / "docs" / "curriculum" / "v2"
PLAN = V2_DIR / "MASTER_PLAN_V2.0.md"
PLAN_REL = f"{V2_REL}/MASTER_PLAN_V2.0.md"
WEEKS_DIR = V2_DIR / "WEEKS"
PACKS = REPO / "data" / "competency-packs"
DATA = REPO / "data" / "curriculum"
OUT_SEED = DATA / "g9-cyber-mastery-480.seed.v2.json"
OUT_DRAFT = DATA / "g9-cyber-mastery-480.draft.v2.json"
OUT_SHEETS = DATA / "g9-cyber-mastery-480.sheets.v2.json"
OUT_KEYS = DATA / "g9-cyber-mastery-480.keys.v2.json"
OUT_ASSESSMENTS = DATA / "assessments-v2"
OUT_SCHEDULE = V2_DIR / "SCHEDULE_60_DAYS.md"
OUT_MAP = V2_DIR / "CURRICULUM_MAP.md"
OUT_REGISTRY = V2_DIR / "RESOURCE_REGISTRY.md"
OUT_BANK = V2_DIR / "ASSESSMENTS" / "QUESTION_BANK.md"
OUT_ANSWERS = V2_DIR / "ASSESSMENTS" / "ANSWER_KEY.md"
OUT_RECALL = V2_DIR / "ASSESSMENTS" / "SPACED_RECALL.md"
SELF = "scripts/build_curriculum_v2.py"
WRITE_HINT = f"python3 -I {SELF} --write"

CURRICULUM_ID = "g9-cyber-mastery-480"
CURRICULUM_NAME = "G9 Cybersecurity Mastery v2.0 (480 hours)"
MANIFEST_SCHEMA_VERSION = "1.1.0"
SEED_VERSION = "2.0.0-seed"
DRAFT_VERSION = "2.0.0-draft1"
SHEETS_SCHEMA_VERSION = "1.1.0"
KEYS_SCHEMA_VERSION = "1.0.0"
SEED_NOTE = (
    "Generated by scripts/build_curriculum_v2.py from the day tables of section 5 of docs/curriculum/v2/MASTER_PLAN_V2.0.md "
    "(the owner's plan, prepared 2026-11-17): columns A, B and C of a day are its three objectives and column D is its evidence "
    "artifact, with Markdown emphasis removed and nothing else changed. A skeleton, not an approved curriculum."
)
DRAFT_NOTE = (
    "Built in the repository from the v2.0 seed (2.0.0-seed): every seed text keeps its wording as the start of its field and gains "
    "an observable criterion; objectives are linked to the competencies they build; each day cites entries of the weekly resource "
    "registries (checked by search result only, see their labels) and its day guide (docs/curriculum/v2/WEEKS/). Not reconciled and "
    "not approved: PROPOSED until the operator promotes it."
)
SHEETS_NOTE = (
    "Lesson sheets of the v2.0 draft curriculum, generated by scripts/build_curriculum_v2.py from docs/curriculum/v2/WEEKS/. "
    "The application shows a day's sheet only when the day's objective texts equal the ones recorded here. The questions are here; "
    "their keys are in a separate file. Every command is proposed text for the owner's review, never a record of something that ran. "
    "Content of a PROPOSED curriculum: reading, not a record."
)
KEYS_NOTE = (
    "Instructor material: the answer keys of the v2.0 draft curriculum, generated by scripts/build_curriculum_v2.py from "
    "docs/curriculum/v2/WEEKS/Wnn/ANSWER_KEYS/. Kept apart from the lesson sheets so that a key is read only when the operator asks "
    "for it; the separation prevents accidental exposure, it is not access control."
)

# The shape of the v2 plan (ADR-0023, profile MASTERY_V2). "saturday_*" keeps its name and describes the assessment day.
STRUCTURE = {
    "weeks": 12,
    "days_per_week": 5,
    "objectives_per_day": 3,
    "weekday_minutes": 480,
    "saturday_minutes": 480,
    "weekly_minutes": 2400,
    "total_minutes": 28800,
    "sunday": "unscheduled",
    "assessment_day_of_week": 5,
}
BLOCK_LETTERS = ["R", "A", "B", "C", "D"]
BLOCK_MINUTES = {"R": 30, "A": 120, "B": 120, "C": 150, "D": 60}
WEEKDAY_CODES = {"R": "recall", "A": "theory", "B": "guided_lab", "C": "independent_lab", "D": "defense_verification"}
FRIDAY_CODES = {"R": "recall", "A": "theory", "B": "performance_review", "C": "blind_practical", "D": "report"}
RECALL_TITLE = "Active recall"

CALENDAR_START = datetime.date(2026, 11, 16)  # Monday of week 1 (plan section 5)
WEEKDAYS = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday"]
WEEKDAY_ABBR = ["Mon", "Tue", "Wed", "Thu", "Fri"]

# Limits. The contract (AUTHORING.md) gives the first group; the second group is what the generated files' schemas hold.
MIN_WORDS_FILE = 1400
MAX_WORDS_FILE = 3200
MIN_WORDS_BLOCK = 120
MIN_WORDS_RECALL = 60
MIN_WORDS_FRIDAY_BLOCK = 40  # the contract gives the minimum "on an ordinary day"; a Friday block is still not a stub
MAX_OBJECTIVE_CHARS = 500  # AUTHORING.md says 600; the manifest (objective.text) and the lesson sheet (objectives[]) hold 500, so 500 is the rule that keeps the generated files valid
MAX_TASK_STATEMENT_CHARS = 19000
MAX_TITLE_CHARS = 200
MIN_COMMAND_ROWS = 3
MAX_RESOURCE_ROWS = 8
MIN_MULTIPLE_CHOICE = 8
QUESTION_MIX = {"CONCEPT": 6, "INTERPRET": 6, "CONTROL": 4, "SYNTHESIS": 4}
LEAK_SHINGLE_WORDS = 8  # key text in a learner file: any run of this many consecutive words (normalized) of a key field
MIN_LEAK_CHARS = 40  # a key field shorter than one run is compared whole, from this many normalized characters (a shorter one is a word, not a leak)
MAX_LEAK_REPORTS = 5  # key fields named for one learner file; the rest is counted
MIN_WORDS_EXAM_SECTION = 20
MIN_WORDS_SUPPORT_FILE = 40
MIN_WORDS_LABS_KEY = 20  # words under the heading of each lab in LABS_KEY.md

MAX_SEED_TEXT_CHARS = 500  # schemas/curriculum/manifest.schema.json: objective.text, evidence_artifact
MAX_HAND_IN_CHARS = 500  # evidence_artifact
MAX_BLOCK_TITLE_CHARS = 64  # lesson sheets: section.name (schemas/curriculum/lesson-sheets.schema.json)
MAX_BLOCK_CHARS = 20000  # lesson sheets: section.markdown
MAX_RESOURCE_VERSION_CHARS = 128  # manifest: resource.version
MAX_LICENSE_NOTE_CHARS = 300
MAX_SHEET_RESOURCES_CHARS = 8000
MAX_QUESTION_CHARS = 2000
MAX_EXAM_QUESTION_CHARS = 6000
MAX_PARAGRAPH_CHARS = 4000  # week paragraphs, expert extension, hint ladder
MAX_SHORT_CHARS = 2000  # header lines, marking
MAX_PROVIDER_CHARS = 300
MAX_TABLE_CHARS = 20000
MAX_LIST_CHARS = 8000
MAX_KEY_TEXT_CHARS = 4000  # answer, why, distractors
MAX_SOURCE_CHARS = 500
MAX_KEY_SECTION_CHARS = 20000
MAX_PRACTICAL_KEY_CHARS = 40000

REGISTRY_KEYS = ["week", "checked_on", "method", "entries", "plan_fact_checks", "gaps"]
ENTRY_KEYS = [
    "id", "url", "provider", "title", "kind", "access", "access_note", "verification", "evidence", "version",
    "topics", "suggested_days", "block", "estimate_minutes", "difficulty", "fallback", "notes",
]  # fmt: skip
ENTRY_KINDS = ["MODULE", "ROOM", "PATH", "LAB", "WARGAME", "MACHINE", "CHALLENGE", "DOCS", "TOOL", "STANDARD"]
HANDS_ON_KINDS = {"MODULE", "ROOM", "LAB", "WARGAME", "MACHINE", "CHALLENGE"}
ENTRY_ACCESS = ["FREE", "FREE_ACCOUNT", "PAID", "MIXED", "UNKNOWN"]
ENTRY_VERIFICATION = ["SEARCH_CHECKED", "ASSUMED", "UNVERIFIED"]
ENTRY_BLOCKS = ["A", "B", "C", "D", "any"]
FACT_KEYS = ["claim", "result", "evidence"]
FACT_RESULTS = ["CONFIRMED", "NOT_CONFIRMED", "CONTRADICTED"]
REGISTRY_ID_RE = re.compile(r"^w(0[1-9]|1[0-2])-[a-z0-9]+(?:[.-][a-z0-9]+)*$")
REGISTRY_ID_IN_TEXT_RE = re.compile(r"(?<![A-Za-z0-9-])w(?:0[1-9]|1[0-2])-[a-z0-9]+(?:[.-][a-z0-9]+)*")
DAY_ID_RE = re.compile(r"^D(0[1-9]|[1-5][0-9]|60)$")
DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
COMPETENCY_RE = re.compile(r"^[a-z0-9][a-z0-9.\-]*$")
LOCAL_LAB_ONLY_RE = re.compile(r"\blocal[- ]lab[- ]only\b", re.IGNORECASE)

DAY_FIXED_BEFORE = ["Objectives", "Hand-in", "Resources"]
DAY_FIXED_AFTER = [
    "Commands proposed for review",
    "Offense, detection, remediation, retest",
    "Quiz",
    "Hint ladder",
    "Troubleshooting and rollback",
    "Evidence and acceptance",
    "Expert extension (optional)",
    "Spaced recall",
    "Reflection",
]
DAY_ORDER = DAY_FIXED_BEFORE + BLOCK_LETTERS + DAY_FIXED_AFTER
DURATION_LINE = "- Duration: 8 instructional hours (R 0.5 h; A 2 h; B 2 h; C 2.5 h; D 1 h)"
RESOURCES_HEADER = ["Id", "Use in this day", "Block"]
COMMANDS_HEADER = ["#", "Command", "Target", "Purpose", "Expected output", "Side effects", "Privilege", "Safe scope", "Rollback"]
MAPPING_HEADER = ["Offensive mechanism", "Defensive observation", "Remediation", "Retest"]
QUIZ_RECALL = "Recall (closed book)"
QUIZ_APPLIED = "Applied"
HINT_LABELS = ["No hint", "Conceptual hint", "Scoped technical hint", "Instructor explanation"]
HINT_LINE_4 = "4. Instructor explanation: in the answer key, after the attempt is submitted."
TROUBLESHOOTING_LABELS = ["Expected", "Missing dependency", "No connectivity", "Unexpected result", "Safety breach", "Restoration", "Time exceeded"]
EVIDENCE_LABELS = ["Evidence", "Grading", "Negative test"]
SPACED_LINES = [("1 day", 1, 2), ("3 days", 3, 1), ("7 days", 7, 1), ("14 days", 14, 1)]
REFLECTION_LINES = [
    "- What was observed and what was inferred?",
    "- What was not proven?",
    "- Which test would falsify the leading hypothesis?",
]
KEY_NOTICE = "Instructor only. Not part of the learner's view."
DAY_KEY_SECTIONS = ["Quiz key", "Independent challenge: reference observations and scoring", "Hint 4: instructor explanation", "Overclaims to reject"]
DAY_KEY_LABELS = ["Answer", "Marking", "Source"]
EXAM_SECTIONS = [
    "Rules",
    "Part 1 — Knowledge exam (20 questions, 20 points)",
    "Part 2 — Practical (40 points)",
    "Part 3 — Defensive validation (25 points)",
    "Part 4 — Evidence and reporting (15 points)",
    "Remediation track",
    "Submission",
]
EXAM_KEY_PRACTICAL = "Practical: fixture map and scoring"
EXAM_KEY_GATE = "Safety and integrity gate"
SCORE_PARTS = ["20", "40", "25", "15"]  # knowledge, practical, defensive validation, evidence and reporting (plan 9.1)
SAMPLE_MARK = "Sample (synthetic):"
WEEK_PARAGRAPHS = [("Purpose", "purpose"), ("Lab stack", "lab_stack"), ("Competencies", "competencies"), ("Accelerator", "accelerator")]
WEEK_SECTIONS = ["Goals", "Prerequisites", "Days", "Tools", "Risks and safety", "Pass conditions", "Remediation"]
LAB_FILES = [("GUIDED_LAB.md", "guided lab"), ("INDEPENDENT_LAB.md", "independent lab"), ("FRIDAY_PRACTICAL.md", "friday practical")]
LAB_SECTIONS = [
    "Why it matters",
    "Scope and authorization",
    "Environment",
    "Preconditions",
    "Steps",
    "Positive validation",
    "Negative and safety validation",
    "Evidence",
    "Rollback and cleanup",
]
ASSESSMENT_KINDS = {"BLIND_PRACTICAL", "CAPSTONE"}

# Characters that must never appear in a source file: the set scripts/build_curriculum.py defines as HIDDEN
# (g9_domain::text::is_hidden without TAB and LF, the whitespace of a Markdown file).
# scripts/tests/test_gates_build_curriculum_v2.py loads both scripts and fails if the two sets drift apart.
HIDDEN = (
    {c for c in range(0x00, 0x20) if c not in (0x09, 0x0A)}  # C0 controls (Cc)
    | {0x7F}
    | set(range(0x80, 0xA0))  # DEL and C1 controls (Cc)
    | {0xAD, 0x061C, 0x180E}  # soft hyphen, Arabic letter mark, Mongolian vowel separator
    | set(range(0x200B, 0x2010))  # zero-width space, joiners, direction marks
    | set(range(0x2028, 0x202F))  # line and paragraph separators, embeddings, overrides
    | set(range(0x2060, 0x2070))  # word joiner, invisible operators, isolates
    | {0xFEFF}  # byte order mark
    | set(range(0xFFF9, 0xFFFC))  # interlinear annotation
    | set(range(0xE0000, 0xE0080))  # tag characters
)

PLAN_SECTION_RE = re.compile(r"^# 5\. ")
PLAN_WEEK_RE = re.compile(r"^## WEEK (\d{2}) — (.+?)\s*$")
PLAN_ROW_START_RE = re.compile(r"^\|\s*\*\*\s*\d")
PLAN_ROW_RE = re.compile(r"^\| \*\*(\d{2}) (Mon|Tue|Wed|Thu|Fri) (\d{2})\*\* \|(.*)\|$")
FENCE_RE = re.compile(r"^\s{0,3}(`{3,}|~{3,})")
HEADING_RE = re.compile(r"^(#{1,6}) (.*?)\s*$")
BLOCK_HEAD_RE = re.compile(r"^([RABCD]) — (.+?) \((\d+) min\)$")
BLOCK_PREFIX_RE = re.compile(r"^([RABCD]) — ")
OBJECTIVE_RE = re.compile(r"^(\d)\. (.+) \[([^\[\]]*)\]$")
QUIZ_ITEM_RE = re.compile(r"^(\d+)\. \[([^\]\s]+)\] (.+)$")
EXAM_ITEM_START_RE = re.compile(r"^\d{1,2}\. \[")
EXAM_ITEM_RE = re.compile(r"^(\d{1,2})\. \[([^\]\s]+)\] \(([A-Z]+)\) (.+)$")
OPTION_RE = re.compile(r"^\s*([A-D])\. \S")
KEY_HEAD_RE = re.compile(r"^\[([^\]\s]+)\]$")
CODE_SPAN_RE = re.compile(r"(?<!`)(`+)(?!`)(.+?)(?<!`)\1(?!`)")
AUTOLINK_RE = re.compile(r"<[A-Za-z][A-Za-z0-9+.\-]{1,31}:[^\s<>]*>")
HTML_RE = re.compile(r"<(?:/?[A-Za-z][A-Za-z0-9\-]*(?:\s[^<>]*)?/?>|!--|![A-Za-z]|\?[A-Za-z])")
SHA256_RE = re.compile(r"(?<![0-9a-f])[0-9a-f]{64}(?![0-9a-f])")

# Answer labels. A learner file holds no answer, no key line and no marked option (AUTHORING.md line 21; ADR-0023 decision 6).
# A line is first stripped of what decorates a label (list markers, numbering, emphasis, quotes, brackets, NFKC folding of
# full-width forms); the label must then start it and be followed by a colon, '=', a dash or 'is'. 'Answer the question' and
# 'Flags: 0x018' are ordinary lines and stay allowed: a bare 'Answer x' is only an answer label when x is an option letter.
DECOR_RE = re.compile(r"^(?:[\s>#*_+\-\u2022\u00b7`'\"\u201c\u201d\u2018\u2019(\[{]|<!--|\d{1,3}[.)](?=\s))+")
_ADJECTIVE = r"(?:(?:the|this|that|its|model|reference|right|expected|final|official|sample|suggested)\s+)*"
_AFTER = r"[*_`'\")\]]*"
_SEPARATOR = r"(?:\s*[:=]|\s*[\u2013\u2014]|\s+-+\s)"
_SEPARATOR_IS = rf"(?:{_SEPARATOR}|\s+is\b)"
ANSWER_LABEL_RE = re.compile(rf"^{_ADJECTIVE}(?:answers?(?:\s+key)?|ans){_AFTER}{_SEPARATOR_IS}")
SOLUTION_LABEL_RE = re.compile(rf"^{_ADJECTIVE}solutions?{_AFTER}{_SEPARATOR}")  # 'The solution is ...' is ordinary teaching prose
CORRECT_ANSWER_RE = re.compile(r"^(?:the\s+)?correct\s+(?:answers?|options?|choices?|responses?)\b")
FLAG_LABEL_RE = re.compile(rf"^(?:the\s+)?flag{_AFTER}\s*[:=]\s*\S*\{{")
ANSWER_LETTER_RE = re.compile(r"^(?i:answers?|ans|correct|key|solution)\s+\(?[A-D]\)?\W*$")
# in the lines of a question only: the labels that are ordinary words elsewhere
STRICT_LABEL_RE = re.compile(rf"^{_ADJECTIVE}(?:keys?|correct){_AFTER}{_SEPARATOR_IS}")
FIELD_LABEL_RE = re.compile(rf"^(?:marking|source|why|distractors?){_AFTER}{_SEPARATOR}")
EXPECTED_LETTER_RE = re.compile(r"^(?i:expected)[*_`'\")\]]*\s*[:=]\s*\(?[A-D]\)?\W*$")
LETTER_ONLY_RE = re.compile(r"^[A-D][*_\])\.]*$")
LETTER_IS_RE = re.compile(r"^[A-D]\s*(?:[:=\-\u2013\u2014]|\s+is|\s+are)\s*(?:the\s+)?(?:correct|right|true|best|answer|key)\b", re.IGNORECASE)
OPTION_MARK_RE = re.compile(r"(?:[(\[]\s*(?:correct|right|answer|key|incorrect|wrong)\s*[)\]]|\s[-\u2013\u2014]\s*(?:correct|right|wrong|incorrect))\s*[.!]?\s*$", re.IGNORECASE)
TICK_RE = re.compile("[\u2713\u2714\u2705\u2611\u2717\u2718\u274c\u274e\u2612]")
# a key line of an instructor file ('- Marking: ...') in a learner file; a list marker is required (prose may say 'Why: ...')
KEY_LINE_RE = re.compile(r"^\s*(?:>\s*)*(?:[-*+\u2022]|\d{1,3}[.)])\s+[*_`]*(?:marking|source|why|distractors?)[*_`]*\s*:", re.IGNORECASE)
ANSWER_LABEL_MESSAGE = (
    "a line of a learner file begins with 'Answer:', '- Answer:' or 'Correct answer' (or 'Ans:', 'Solution:', 'The answer is', 'Flag: g9{...}' "
    "in any emphasis, numbering, bracket or quote form): answers live only under ANSWER_KEYS/"
)
KEY_LINE_MESSAGE = "a line of a learner file is a key line ('- Marking:', '- Source:', '- Why:', '- Distractors:'): key lines live only under ANSWER_KEYS/"
LEAK_TAIL = "no learner file may hold key text (answers, marking, rationale, reference observations, hints, scoring): reword it"
FLAG_TOKEN_RE = re.compile(r"\b[A-Za-z][A-Za-z0-9_]{1,11}\{([^{}\s]{4,})\}")
FLAG_PLACEHOLDER_RE = re.compile(r"^(?:\.+|\u2026+|[xX*_-]+|<[^>]*>|flag)$")
MCQ_LETTER_RE = re.compile(r"^\W{0,2}[A-D](?![A-Za-z0-9])\W*")
OPTION_VERBS = "is|are|was|were|and|or|would|says|names|claims|treats|skips|ignores|confuses|fails|describes|reverses|assumes|proposes|measures|checks|mistakes|swaps"

# Text that must not be in the courseware (AUTHORING.md: no real credentials, hosts or personal data; lab addresses from the plan).
# Cheap patterns, not a scanner: they catch a pasted token, key, personal address or non-lab IPv4. Real host names, organizations and
# personal names are NOT enforced.
SENSITIVE_TOKENS: list[tuple[re.Pattern, str]] = [
    (re.compile(r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY(?: BLOCK)?-----"), "looks like a private key header"),
    (re.compile(r"\b(?:AKIA|ASIA|AGPA|AIDA|AROA|ANPA|ANVA|AIPA)[A-Z0-9]{16}\b"), "looks like an AWS access key id"),
    (re.compile(r"\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,})"), "looks like a GitHub token"),
    (re.compile(r"\bxox[abprs]-[A-Za-z0-9-]{10,}"), "looks like a Slack token"),
    (re.compile(r"\bAIza[0-9A-Za-z_\-]{35}\b"), "looks like a Google API key"),
    (re.compile(r"\b[sr]k_live_[0-9A-Za-z]{16,}"), "looks like a live payment key"),
]
EMAIL_RE = re.compile(r"(?<![\w.%+\-])([A-Za-z0-9._%+\-]+)@((?:[A-Za-z0-9\-]+\.)+[A-Za-z]{2,})\b")
EMAIL_OK_DOMAINS = {"example.com", "example.org", "example.net", "example.edu"}  # and the local part "git" (git@github.com: a clone address, not a person)
EMAIL_OK_SUFFIXES = (".test", ".example", ".invalid", ".localhost", ".local", ".internal", ".example.com", ".example.org", ".example.net")
URL_USERINFO_RE = re.compile(r"\b[A-Za-z][A-Za-z0-9+.\-]*://([^\s/@:'\"`<>]+):([^\s/@'\"`]+)@")
PASSWORD_PLACEHOLDER_RE = re.compile(r"^(?:<[^>]+>|\{[^}]+\}|\$\{?\w+\}?|\*+|[xX]+|REDACTED|redacted|password|changeme|synthetic[\w-]*)$")
IPV4_RE = re.compile(r"(?<![\d.])(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})(?:/(\d{1,2}))?(?!\d|\.\d)")
IPV4_DOC_NETS = {(192, 0, 2), (198, 51, 100), (203, 0, 113)}  # RFC 5737 documentation ranges
IPV4_BLOCKS = {  # the reserved blocks, named by their prefix when a lesson teaches them (10.0.0.0/8, 192.168.0.0/16 ...)
    ((10, 0, 0, 0), 8), ((172, 16, 0, 0), 12), ((192, 168, 0, 0), 16), ((169, 254, 0, 0), 16), ((100, 64, 0, 0), 10), ((127, 0, 0, 0), 8),
    ((224, 0, 0, 0), 4), ((240, 0, 0, 0), 4), ((0, 0, 0, 0), 8), ((0, 0, 0, 0), 0),
}  # fmt: skip
IPV4_ENDS = {  # the first and last address of those blocks, as a lesson writes a range
    (10, 0, 0, 0), (10, 255, 255, 255), (172, 16, 0, 0), (172, 31, 255, 255), (192, 168, 0, 0), (192, 168, 255, 255), (169, 254, 0, 0), (169, 254, 255, 255),
    (100, 64, 0, 0), (100, 127, 255, 255),
}  # fmt: skip
IPV4_METADATA = {(169, 254, 169, 254), (169, 254, 170, 2), (168, 63, 129, 16), (100, 100, 100, 200)}  # cloud metadata endpoints, link-local, not hosts


class Problems:
    def __init__(self) -> None:
        self.items: list[str] = []

    def add(self, where: str, message: str) -> None:
        self.items.append(f"{where}: {message}")


class Line(NamedTuple):
    no: int
    text: str
    level: int  # heading level outside fenced code, else 0
    code: bool  # inside a fenced code block (the fence lines included)


class Section(NamedTuple):
    title: str
    no: int
    body: list[Line]


class KeyField(NamedTuple):
    """One field of an instructor key: the text no learner file may hold."""

    field: str  # answer, marking, why, distractors, challenge, hint 4, overclaims, practical, gate, lab key
    ident: str  # the question id, the day, the exam code or the lab heading
    where: str  # the key file
    text: str


class LeakIndex(NamedTuple):
    windows: dict[str, KeyField]  # every run of LEAK_SHINGLE_WORDS normalized words of a key field
    wholes: list[tuple[str, KeyField]]  # key fields shorter than one run, from MIN_LEAK_CHARS characters
    flags: dict[str, KeyField]  # a flag (name{value}) written in a key


# ---------------------------------------------------------------------------------------------
# calendar and small helpers
# ---------------------------------------------------------------------------------------------


def day_date(n: int) -> datetime.date:
    """The plan's calendar date of day n (1..60): week 1 starts Monday 2026-11-16, five days a week."""
    return CALENDAR_START + datetime.timedelta(days=7 * ((n - 1) // 5) + (n - 1) % 5)


def week_of(n: int) -> int:
    return (n - 1) // 5 + 1


def is_friday(n: int) -> bool:
    return n % 5 == 0


def day_id(n: int) -> str:
    return f"D{n:02d}"


def week_name(w: int) -> str:
    return f"W{w:02d}"


def exam_code(w: int) -> str:
    return "V2-W12-CAPSTONE" if w == 12 else f"V2-W{w:02d}-EXAM"


def queue_day(n: int, offset: int) -> int | None:
    """The first training day on or after the date of day n plus `offset` calendar days; None after day 60."""
    target = day_date(n) + datetime.timedelta(days=offset)
    for e in range(n + 1, 61):
        if day_date(e) >= target:
            return e
    return None


def normalize(text: str) -> str:
    """Lower case, every run of characters that are not letters or digits as one space: the form two texts are compared in."""
    return " ".join(re.sub(r"[\W_]+", " ", text.lower()).split())


def cut(text: str, limit: int) -> str:
    return text if len(text) <= limit else text[: limit - 3].rstrip() + "..."


def decor_stripped(line: str) -> str:
    """A line without what decorates a label: list markers, numbering, emphasis, quotes, opening brackets; full-width forms folded."""
    return DECOR_RE.sub("", unicodedata.normalize("NFKC", line)).strip()


def answer_label(line: str, strict: bool = False) -> str | None:
    """The answer label a line begins with ('Answer:', '**Answer**:', 'The answer is', 'Solution:', 'Flag: g9{...}' ...), or None.
    `strict` is for the lines of a question, where more is refused: 'Correct: B', 'Key: B', '(B)', 'B is correct', a '- Marking:' line."""
    s = decor_stripped(line)
    low = s.lower()
    m = ANSWER_LABEL_RE.match(low) or SOLUTION_LABEL_RE.match(low) or CORRECT_ANSWER_RE.match(low) or FLAG_LABEL_RE.match(low) or ANSWER_LETTER_RE.match(s)
    if strict and not m:
        m = STRICT_LABEL_RE.match(low) or FIELD_LABEL_RE.match(low) or EXPECTED_LETTER_RE.match(s) or LETTER_ONLY_RE.match(s) or LETTER_IS_RE.match(s)
    return s[: max(m.end(), 1)].strip()[:40] if m else None


def key_line(line: str) -> str | None:
    """'- Marking: ...', '- Source: ...', '- Why: ...', '- Distractors: ...' (the field lines of an instructor key), or None."""
    m = KEY_LINE_RE.match(unicodedata.normalize("NFKC", line))
    return m.group(0).strip()[:40] if m else None


def question_leak(line: str, code: bool = False) -> str | None:
    """What in one line of a question states or marks its answer: a label, a key line, a tick or cross, '(correct)' after an option."""
    label = answer_label(line, strict=not code)
    if label or code:
        return label
    tick = TICK_RE.search(line)
    if tick:
        return tick.group(0)
    mark = OPTION_MARK_RE.search(line)
    return mark.group(0).strip() if mark else None


def option_named(letter: str, text: str) -> bool:
    """Whether a text names the option `letter` ('A:', 'A.', '(A)', 'Option A', 'A is wrong', 'A, C and D'); the English article 'A' does not."""
    return re.search(rf"(?<![A-Za-z0-9])(?:(?i:options?)\s+|\()?{letter}(?:\)|\s*[:.,;/\u2013\u2014\]-]|\s*\(|\s+(?:{OPTION_VERBS})\b|\s*$)", text) is not None


def parts_named(text: str) -> set[int]:
    """The numbers of the parts of ASSESSMENT.md a text names: 'Part 2', 'Parts 2 and 3', 'Parts 2, 3', 'Parts 2 to 4', 'Parts 2-3'."""
    out: set[int] = set()
    for m in re.finditer(r"\bParts?\s+(\d(?:\s*(?:,|and|&|to|-|\u2013|\u2014)\s*(?:Part\s+)?\d)*)", text):
        previous: int | None = None
        span = False
        for token in re.findall(r"\d|to|-|\u2013|\u2014", m.group(1)):
            if token.isdigit():
                k = int(token)
                out.update(range(previous, k + 1) if span and previous is not None else [k])
                previous, span = k, False
            else:
                span = True
    return out


_PLAN_ADDRESSES: set[tuple[int, ...]] | None = None


def plan_addresses() -> set[tuple[int, ...]]:
    """The IPv4 addresses the owner's plan names: the lab topology (10.77.x.x) and the host networks it says to avoid (its container bridges
    and the host LAN). A lesson on isolation has to name them; they are the plan's facts, not hosts of the lesson."""
    global _PLAN_ADDRESSES
    if _PLAN_ADDRESSES is None:
        text = PLAN.read_bytes().decode("utf-8", errors="replace") if PLAN.is_file() else ""
        found = {tuple(int(x) for x in m.groups()[:4]) for m in IPV4_RE.finditer(text)}
        _PLAN_ADDRESSES = {octets for octets in found if max(octets) <= 255}
    return _PLAN_ADDRESSES


def ipv4_allowed(octets: tuple[int, int, int, int], prefix: int | None) -> bool:
    """The addresses a lesson may write: the lab (10.77.0.0/16) and the other addresses the plan names, loopback, the unspecified and
    broadcast addresses, netmasks and wildcards, the documentation ranges, the reserved blocks by their prefix, and the cloud metadata
    endpoints."""
    a, b, c, _d = octets
    n = (a << 24) | (b << 16) | (c << 8) | octets[3]
    inverse = ~n & 0xFFFFFFFF
    return (
        (a, b) == (10, 77)
        or a == 127
        or n in (0, 0xFFFFFFFF)
        or (a, b, c) in IPV4_DOC_NETS
        or (inverse & (inverse + 1)) == 0  # a netmask: ones, then zeros
        or (n & (n + 1)) == 0  # a wildcard mask: zeros, then ones
        or (prefix is not None and (octets, prefix) in IPV4_BLOCKS)
        or octets in IPV4_ENDS
        or octets in IPV4_METADATA
        or octets in plan_addresses()
    )


def sensitive_hits(text: str, ipv4: bool = True) -> list[tuple[int, str]]:
    """(line, what) for each credential-like token, personal e-mail address, URL with a password and non-lab IPv4 address; once per kind and address."""
    hits: dict[str, int] = {}

    def add(offset: int, what: str) -> None:
        hits.setdefault(what, text.count("\n", 0, offset) + 1)

    for pattern, what in SENSITIVE_TOKENS:
        for m in pattern.finditer(text):
            if not (what == "looks like an AWS access key id" and m.group(0).endswith("EXAMPLE")):
                add(m.start(), what)
    for m in URL_USERINFO_RE.finditer(text):
        if not PASSWORD_PLACEHOLDER_RE.match(m.group(2)):
            add(m.start(), "a URL with a user name and password: use a synthetic placeholder such as <synthetic-password>")
    for m in EMAIL_RE.finditer(text):
        domain = m.group(2).lower()
        if m.group(1).lower() != "git" and domain not in EMAIL_OK_DOMAINS and not domain.endswith(EMAIL_OK_SUFFIXES):
            add(m.start(), f"an e-mail address at {domain}: personal data stays out of the courseware (write student@corp.g9.test or an example.com address)")
    if ipv4:
        for m in IPV4_RE.finditer(text):
            octets = tuple(int(x) for x in m.groups()[:4])
            if max(octets) > 255 or (m.group(5) is not None and int(m.group(5)) > 32):
                continue
            if not ipv4_allowed(octets, None if m.group(5) is None else int(m.group(5))):  # type: ignore[arg-type]
                add(m.start(), f"IPv4 address {'.'.join(map(str, octets))} is not a lab address: lab addresses come from 10.77.0.0/16 and the networks the plan names (the documentation ranges are allowed)")
    return sorted((no, what) for what, no in hits.items())


def cell(text: str) -> str:
    """A text as one table cell: one line, a pipe written as \\|."""
    return re.sub(r"(?<!\\)\|", r"\\|", " ".join(text.split()))


def dump(obj: object) -> str:
    return json.dumps(obj, indent=2, ensure_ascii=False) + "\n"


def rel(path: Path) -> str:
    return path.relative_to(REPO).as_posix()


def limit(problems: Problems, where: str, what: str, text: str, maximum: int) -> None:
    if len(text) > maximum:
        problems.add(where, f"{what} is {len(text)} characters (maximum {maximum})")


# ---------------------------------------------------------------------------------------------
# reading Markdown
# ---------------------------------------------------------------------------------------------


def read_source(path: Path, where: str, problems: Problems) -> str | None:
    """The text of a source file; a hidden or control character is reported once, with its line. The bytes are decoded as they are:
    reading in text mode would turn a CR into a line break before the check could see it."""
    try:
        text = path.read_bytes().decode("utf-8")
    except UnicodeDecodeError:
        problems.add(where, "not valid UTF-8")
        return None
    for i, ch in enumerate(text):
        if ord(ch) in HIDDEN:
            line = text.count("\n", 0, i) + 1
            problems.add(f"{where}:{line}", f"hidden or control character U+{ord(ch):04X}")
            break
    return text


def scan(text: str) -> tuple[list[Line], bool]:
    """The lines of a Markdown text with their heading level; headings are not read inside fenced code.
    The second value is True when a fence is opened and never closed."""
    out: list[Line] = []
    fence: str | None = None
    for no, raw in enumerate(text.split("\n"), start=1):
        line = raw.rstrip()
        m = FENCE_RE.match(line)
        if fence is None:
            if m:
                fence = m.group(1)
                out.append(Line(no, line, 0, True))
                continue
            h = HEADING_RE.match(line)
            out.append(Line(no, line, len(h.group(1)) if h else 0, False))
        else:
            out.append(Line(no, line, 0, True))
            if m and m.group(1)[0] == fence[0] and len(m.group(1)) >= len(fence) and line.strip() == m.group(1):
                fence = None
    return out, fence is not None


def heading_text(line: Line) -> str:
    m = HEADING_RE.match(line.text)
    return m.group(2) if m else ""


def split_sections(lines: list[Line], level: int) -> tuple[list[Line], list[Section]]:
    """The lines before the first heading of `level`, then one Section per heading of that level."""
    preamble: list[Line] = []
    sections: list[Section] = []
    for line in lines:
        if line.level == level:
            sections.append(Section(heading_text(line), line.no, []))
        elif sections:
            sections[-1].body.append(line)
        else:
            preamble.append(line)
    return preamble, sections


def nonblank(body: list[Line]) -> list[Line]:
    return [ln for ln in body if ln.text.strip()]


def markdown_of(body: list[Line]) -> str:
    """The lines of a section as one Markdown text (blank lines inside are kept, the edges are trimmed)."""
    return "\n".join(ln.text for ln in body).strip("\n")


def words_of(body: list[Line]) -> int:
    return len(" ".join(ln.text for ln in body).split())


def paragraphs(body: list[Line]) -> list[str]:
    """The paragraphs of a section, each as one line of text."""
    out: list[str] = []
    current: list[str] = []
    for ln in body:
        if ln.text.strip():
            current.append(ln.text.strip())
        elif current:
            out.append(" ".join(current))
            current = []
    if current:
        out.append(" ".join(current))
    return out


def split_row(text: str) -> list[str] | None:
    """The cells of a table row; a pipe inside a cell is written \\|."""
    s = text.strip()
    if len(s) < 2 or not s.startswith("|") or not s.endswith("|"):
        return None
    return [c.strip() for c in re.split(r"(?<!\\)\|", s[1:-1])]


def parse_table(body: list[Line], header: list[str], where: str, name: str, problems: Problems) -> list[list[str]] | None:
    """The rows of a section that holds exactly one table with the given header. None when the table is not there."""
    lines = nonblank(body)
    head = "| " + " | ".join(header) + " |"
    cells = split_row(lines[0].text) if lines else None
    separator = split_row(lines[1].text) if len(lines) > 1 else None
    if cells != header or separator is None or len(separator) != len(header) or not all(re.fullmatch(r":?-{3,}:?", c) for c in separator):
        problems.add(where, f"{name}: the section holds the table '{head}' (header, then the '|---|' line, then the rows) and nothing else")
        return None
    rows: list[list[str]] = []
    for k, ln in enumerate(lines[2:], start=1):
        row = split_row(ln.text)
        if row is None:
            problems.add(f"{where}:{ln.no}", f"{name}: text outside the table: {ln.text[:60]}")
            continue
        if len(row) != len(header):
            problems.add(f"{where}:{ln.no}", f"{name} row {k} has {len(row)} cells ({len(header)} are required; write a pipe inside a cell as \\|)")
            continue
        for column, value in zip(header, row):
            if not value:
                problems.add(f"{where}:{ln.no}", f"{name} row {k}: empty cell '{column}'")
        rows.append(row)
    return rows


def labelled_fields(body: list[Line], labels: list[str]) -> tuple[list[tuple[str, str]], list[Line]]:
    """The '- Label: text' fields of a key entry in the order written (a following line continues the field),
    and the lines that belong to no field."""
    fields: list[tuple[str, list[str]]] = []
    stray: list[Line] = []
    for ln in nonblank(body):
        label = next((lb for lb in labels if ln.text.startswith(f"- {lb}:")), None)
        if label is not None:
            fields.append((label, [ln.text[len(label) + 3 :].strip()]))
        elif fields:
            fields[-1][1].append(ln.text.strip())
        else:
            stray.append(ln)
    return [(label, "\n".join(parts).strip()) for label, parts in fields], stray


LINK_RE = re.compile(r"!?\[[^\]]*\]\([^)\n]*\)")  # the text of a link may wrap over lines of one paragraph
REF_LINK_RE = re.compile(r"!?\[[^\]\n]+\]\[[^\]\n]*\]")
REF_DEFINITION_RE = re.compile(r"(?m)^ {0,3}\[[^\]\n]+\]:\s*\S+")


def check_plain_markdown(lines: list[Line], where: str, problems: Problems) -> None:
    """No link, image or HTML outside code: the window renders none of them (AUTHORING.md, rules for every file).
    A link is found in a paragraph, so that a text wrapped over two lines or a reference-style link does not pass."""
    paragraph: list[Line] = []

    def flush() -> None:
        if not paragraph:
            return
        stripped = [CODE_SPAN_RE.sub(" ", ln.text) for ln in paragraph]
        joined = "\n".join(stripped)
        for pattern, kind in ((LINK_RE, ""), (REF_LINK_RE, " (reference style)"), (REF_DEFINITION_RE, " (reference definition)")):
            m = pattern.search(joined)
            if m:
                no = paragraph[joined.count("\n", 0, m.start())].no
                problems.add(f"{where}:{no}", f"Markdown link or image{kind} '{' '.join(m.group(0).split())[:60]}': write the address as inline code and list it in '## Resources'")
                break
        for ln, text in zip(paragraph, stripped):
            m = AUTOLINK_RE.search(text)
            if m:
                problems.add(f"{where}:{ln.no}", f"Markdown link or image '{m.group(0)[:60]}': write the address as inline code and list it in '## Resources'")
            m = HTML_RE.search(text)
            if m:
                problems.add(f"{where}:{ln.no}", f"HTML or an angle-bracket placeholder outside inline code '{m.group(0)[:40]}': no HTML; write a placeholder such as `<synthetic-password>` as inline code")
        paragraph.clear()

    for ln in lines:
        if ln.code or not ln.text.strip():
            flush()
        else:
            paragraph.append(ln)
    flush()


def check_structure(lines: list[Line], unclosed: bool, where: str, problems: Problems) -> None:
    if unclosed:
        problems.add(where, "a code fence is opened and never closed")
    for ln in lines:
        if ln.level == 1 and ln.no != 1:
            problems.add(f"{where}:{ln.no}", "a second '# ' heading: a file has one title, on line 1")


def check_order(sections: list[Section], keys: list[str | None], order: list[str], where: str, problems: Problems, show: dict[str, str] | None = None) -> dict[str, Section]:
    """Every heading is known, appears once and in the contract's order. Returns the sections by key."""
    show = show or {}
    found: dict[str, Section] = {}
    sequence: list[str] = []
    for section, key in zip(sections, keys):
        if key is None:
            problems.add(f"{where}:{section.no}", f"unknown section '## {section.title}'")
            continue
        if key in found:
            problems.add(f"{where}:{section.no}", f"section '## {section.title}' appears twice")
            continue
        found[key] = section
        sequence.append(key)
    for key in order:
        if key not in found:
            problems.add(where, f"missing section '## {show.get(key, key)}'")
    if sequence != [k for k in order if k in found]:
        problems.add(where, f"sections are out of order: {[show.get(k, k) for k in sequence]}")
    return found


# ---------------------------------------------------------------------------------------------
# the plan: seed texts
# ---------------------------------------------------------------------------------------------


def clean_cell(text: str) -> str:
    """A plan cell as seed text: Markdown emphasis markers removed, whitespace collapsed, nothing else changed."""
    return " ".join(text.replace("*", "").split())


def parse_plan(text: str, problems: Problems) -> list[dict]:
    """The 12 weeks and 60 days of section 5 of the master plan."""
    where = "MASTER_PLAN_V2.0.md"
    lines = text.split("\n")
    start = next((i for i, ln in enumerate(lines) if PLAN_SECTION_RE.match(ln)), None)
    if start is None:
        problems.add(where, "no '# 5. ' heading: the day tables of section 5 were not found")
        return []
    end = next((i for i in range(start + 1, len(lines)) if lines[i].startswith("# ")), len(lines))
    weeks: list[dict] = []
    expected = 1
    for i in range(start + 1, end):
        line = lines[i].rstrip()
        at = f"{where}:{i + 1}"
        if line.startswith("## WEEK"):
            m = PLAN_WEEK_RE.match(line)
            if not m:
                problems.add(at, "a week heading of section 5 is '## WEEK NN — <title>'")
                continue
            if int(m.group(1)) != len(weeks) + 1:
                problems.add(at, f"week heading {m.group(1)} is out of order (week {len(weeks) + 1:02d} is expected)")
            title = " ".join(m.group(2).split())
            limit(problems, at, "week title", title, MAX_TITLE_CHARS)
            weeks.append({"number": len(weeks) + 1, "title": title, "days": []})
            continue
        if not PLAN_ROW_START_RE.match(line):
            continue
        m = PLAN_ROW_RE.match(line)
        cells = [clean_cell(c) for c in re.split(r"(?<!\\)\|", m.group(4))] if m else []
        if not m or len(cells) != 4 or not all(cells):
            problems.add(at, "a day row of section 5 is '| **NN Www DD** | A | B | C | D |' with four non-empty cells")
            continue
        if not weeks:
            problems.add(at, "a day row before the first '## WEEK NN — <title>' heading")
            continue
        n = int(m.group(1))
        if not 1 <= n <= 60:
            problems.add(at, f"day row {n:02d} is outside days 01 to 60 (the plan has exactly 60 days)")
            continue
        if n != expected:
            problems.add(at, f"day row {n:02d} is out of order (day {expected:02d} is expected)")
        expected = n + 1
        w = weeks[-1]["number"]
        if week_of(n) != w:
            problems.add(at, f"day {n:02d} is not a day of week {w:02d} (days {5 * w - 4:02d} to {5 * w:02d})")
        date = day_date(n)
        if m.group(2) != WEEKDAY_ABBR[(n - 1) % 5] or int(m.group(3)) != date.day:
            problems.add(at, f"day {n:02d} is '{m.group(2)} {m.group(3)}' in the plan; the calendar gives '{WEEKDAY_ABBR[(n - 1) % 5]} {date.day:02d}' ({date.isoformat()})")
        for column, value in zip("ABCD", cells):
            limit(problems, at, f"seed text of column {column}", value, MAX_SEED_TEXT_CHARS)
        weeks[-1]["days"].append({"n": n, "texts": cells})
    if len(weeks) != 12:
        problems.add(where, f"{len(weeks)} week headings in section 5 (exactly 12 are required)")
    for w in weeks:
        if len(w["days"]) != 5:
            problems.add(where, f"week {w['number']:02d} has {len(w['days'])} day rows (exactly 5 are required)")
    total = sum(len(w["days"]) for w in weeks)
    if total != 60:
        problems.add(where, f"{total} day rows in section 5 (exactly 60 are required)")
    return weeks


def manifest_blocks(n: int) -> list[dict]:
    codes = FRIDAY_CODES if is_friday(n) else WEEKDAY_CODES
    return [{"seq": i, "block": codes[letter], "minutes": BLOCK_MINUTES[letter]} for i, letter in enumerate(BLOCK_LETTERS, start=1)]


def manifest_day(n: int, objectives: list[dict], artifact: str, resources: list[dict] | None) -> dict:
    day: dict = {
        "day_id": day_id(n),
        "day_of_week": (n - 1) % 5 + 1,
        "planned_minutes": STRUCTURE["weekday_minutes"],
        "objectives": objectives,
        "evidence_artifact": artifact,
        "blocks": manifest_blocks(n),
        "is_assessment_day": is_friday(n),
    }
    if resources:
        day["resources"] = resources
    return day


def manifest(version: str, note: str, weeks: list[dict]) -> dict:
    return {
        "manifest_schema_version": MANIFEST_SCHEMA_VERSION,
        "curriculum_id": CURRICULUM_ID,
        "name": CURRICULUM_NAME,
        "version": version,
        "provenance": "PROPOSED_SEED",
        "provenance_note": note,
        "structure": STRUCTURE,
        "weekday_blocks": [{"block": WEEKDAY_CODES[x], "minutes": BLOCK_MINUTES[x]} for x in BLOCK_LETTERS],
        "saturday_blocks": [{"block": FRIDAY_CODES[x], "minutes": BLOCK_MINUTES[x]} for x in BLOCK_LETTERS],
        "weeks": weeks,
    }


def build_seed(plan: list[dict]) -> dict:
    weeks = []
    for w in plan:
        days = []
        for d in w["days"]:
            objectives = [{"seq": k, "objective_id": f"{day_id(d['n'])}-O{k}", "text": d["texts"][k - 1]} for k in (1, 2, 3)]
            days.append(manifest_day(d["n"], objectives, d["texts"][3], None))
        weeks.append({"week_number": w["number"], "title": w["title"], "days": days})
    return manifest(SEED_VERSION, SEED_NOTE, weeks)


def seed_text_lines(plan: list[dict], n: int) -> list[str]:
    d = next(d for w in plan for d in w["days"] if d["n"] == n)
    date = day_date(n)
    return [
        f"{day_id(n)}  week {week_of(n)}  {date.isoformat()} ({WEEKDAYS[(n - 1) % 5]})",
        f"Objective 1 (column A): {d['texts'][0]}",
        f"Objective 2 (column B): {d['texts'][1]}",
        f"Objective 3 (column C): {d['texts'][2]}",
        f"Hand-in (column D): {d['texts'][3]}",
    ]


# ---------------------------------------------------------------------------------------------
# competencies and resource registries
# ---------------------------------------------------------------------------------------------


class PackError(Exception):
    """A competency pack that cannot be read: an environment error (exit 2), not a rule of the courseware."""


def load_competencies() -> dict[str, dict]:
    """id -> {name, pack, band}. A pack that is not valid JSON, lacks 'pack_id', 'competencies' or a difficulty band raises PackError."""
    info: dict[str, dict] = {}
    for path in sorted(PACKS.glob("*/pack.json")):
        try:
            pack = json.loads(path.read_bytes().decode("utf-8"))
            pack_id = pack["pack_id"]
            for c in pack["competencies"]:
                low, high = c["difficulty_band"]["min"], c["difficulty_band"]["max"]
                if not isinstance(c["id"], str) or not isinstance(c["name"], str) or not all(isinstance(x, int) and not isinstance(x, bool) for x in (low, high)):
                    raise ValueError("a competency needs a text id, a text name and a difficulty band of two whole numbers")
                info[c["id"]] = {"name": c["name"], "pack": pack_id, "band": (low, high)}
        except (OSError, ValueError, KeyError, TypeError, AttributeError) as exc:
            raise PackError(f"competency pack {rel(path)} is not usable ({type(exc).__name__}: {str(exc)[:80]})") from exc
    return info


def strings_of(node: object) -> list[str]:
    if isinstance(node, str):
        return [node]
    if isinstance(node, dict):
        return [s for k, v in node.items() for s in [k, *strings_of(v)]]
    if isinstance(node, list):
        return [s for v in node for s in strings_of(v)]
    return []


def exact_keys(node: dict, keys: list[str], where: str, problems: Problems) -> bool:
    ok = True
    for key in keys:
        if key not in node:
            problems.add(where, f"missing key '{key}'")
            ok = False
    for key in node:
        if key not in keys:
            problems.add(where, f"unknown key '{key}'")
            ok = False
    return ok


def text_field(node: dict, key: str, where: str, problems: Problems, maximum: int, empty: bool = False) -> None:
    value = node.get(key)
    if not isinstance(value, str) or (not empty and not value.strip()):
        problems.add(where, f"'{key}' must be {'a text' if empty else 'a non-empty text'}")
    elif len(value) > maximum:
        problems.add(where, f"'{key}' is {len(value)} characters (maximum {maximum})")


def load_registry(w: int, problems: Problems) -> dict | None:
    """Read and check WEEKS/Wnn/RESOURCES.json (schemas/curriculum/resource-registry.schema.json, checked here by hand:
    the script uses the standard library only). Returns the registry with the entries that can be cited."""
    where = f"{week_name(w)}/RESOURCES.json"
    path = WEEKS_DIR / week_name(w) / "RESOURCES.json"
    if not path.is_file():
        return None
    text = read_source(path, where, problems)
    if text is None:
        return None
    try:
        doc = json.loads(text)
    except json.JSONDecodeError as exc:
        problems.add(where, f"not valid JSON: {exc.msg} (line {exc.lineno})")
        return None
    if not isinstance(doc, dict):
        problems.add(where, "the document must be an object")
        return None
    for s in strings_of(doc):
        hidden = next((ch for ch in s if ord(ch) in HIDDEN), None)
        if hidden is not None:
            problems.add(where, f"hidden or control character U+{ord(hidden):04X} in a text (written as a JSON escape)")
            break
    exact_keys(doc, REGISTRY_KEYS, where, problems)
    if doc.get("week") != w or isinstance(doc.get("week"), bool):
        problems.add(where, f"'week' is {doc.get('week')!r}; the file is the registry of week {w}")
    checked_on = doc.get("checked_on")
    if not isinstance(checked_on, str) or not valid_date(checked_on):
        problems.add(where, "'checked_on' must be a date YYYY-MM-DD")
        checked_on = "unknown"
    text_field(doc, "method", where, problems, 2000)
    entries = doc.get("entries")
    usable: list[dict] = []
    if not isinstance(entries, list) or not entries:
        problems.add(where, "'entries' must be a non-empty list")
        entries = []
    seen: set[str] = set()
    for i, entry in enumerate(entries):
        at = f"{where} entries[{i}]"
        if not isinstance(entry, dict):
            problems.add(at, "an entry must be an object")
            continue
        ident = entry.get("id")
        if isinstance(ident, str) and ident:
            at = f"{where} {ident}"
        before = len(problems.items)
        exact_keys(entry, ENTRY_KEYS, at, problems)
        if not isinstance(ident, str) or not REGISTRY_ID_RE.match(ident) or len(ident) > 80:
            problems.add(at, "'id' must be 'wNN-' and lower-case words joined by hyphens (at most 80 characters)")
        else:
            if not ident.startswith(f"w{w:02d}-"):
                problems.add(at, f"'id' must start with 'w{w:02d}-' (the week of the file)")
            if ident in seen:
                problems.add(at, f"id '{ident}' appears twice")
            seen.add(ident)
        url = entry.get("url")
        if not isinstance(url, str) or not url.startswith("https://") or len(url) > 2048 or re.search(r"\s", url) or len(url) <= len("https://"):
            problems.add(at, "'url' must be an https address without spaces")
        text_field(entry, "provider", at, problems, 100)
        text_field(entry, "title", at, problems, 300)
        for key, allowed in (("kind", ENTRY_KINDS), ("access", ENTRY_ACCESS), ("verification", ENTRY_VERIFICATION), ("block", ENTRY_BLOCKS)):
            if entry.get(key) not in allowed or not isinstance(entry.get(key), str):
                problems.add(at, f"'{key}' is {entry.get(key)!r}; it must be one of {allowed}")
        text_field(entry, "access_note", at, problems, 2000, empty=True)
        text_field(entry, "evidence", at, problems, 4000, empty=True)
        text_field(entry, "version", at, problems, 200)
        text_field(entry, "notes", at, problems, 4000, empty=True)
        topics = entry.get("topics")
        if not isinstance(topics, list) or not all(isinstance(t, str) and t.strip() and len(t) <= 100 for t in topics):
            problems.add(at, "'topics' must be a list of non-empty texts (at most 100 characters each)")
        days = entry.get("suggested_days")
        if not isinstance(days, list) or not all(isinstance(d, str) and DAY_ID_RE.match(d) for d in days):
            problems.add(at, "'suggested_days' must be a list of day ids D01 to D60")
        minutes = entry.get("estimate_minutes")
        if minutes is not None and (isinstance(minutes, bool) or not isinstance(minutes, int) or not 1 <= minutes <= 2400):
            problems.add(at, "'estimate_minutes' must be null or a whole number of minutes from 1 to 2400")
        difficulty = entry.get("difficulty")
        if difficulty is not None and (isinstance(difficulty, bool) or not isinstance(difficulty, (str, int)) or (isinstance(difficulty, str) and not 1 <= len(difficulty) <= 200)):
            problems.add(at, "'difficulty' must be null, a text or a whole number")
        fallback = entry.get("fallback")
        if fallback is not None and (not isinstance(fallback, str) or not fallback.strip() or len(fallback) > 1000):
            problems.add(at, "'fallback' must be null or a non-empty text (a registry id or a description)")
        if len(problems.items) == before:
            usable.append(entry)
    checks = doc.get("plan_fact_checks")
    if not isinstance(checks, list):
        problems.add(where, "'plan_fact_checks' must be a list")
        checks = []
    for i, check in enumerate(checks):
        at = f"{where} plan_fact_checks[{i}]"
        if not isinstance(check, dict):
            problems.add(at, "a fact check must be an object")
            continue
        exact_keys(check, FACT_KEYS, at, problems)
        text_field(check, "claim", at, problems, 2000)
        text_field(check, "evidence", at, problems, 4000, empty=True)
        if check.get("result") not in FACT_RESULTS or not isinstance(check.get("result"), str):
            problems.add(at, f"'result' is {check.get('result')!r}; it must be one of {FACT_RESULTS}")
    gaps = doc.get("gaps")
    if not isinstance(gaps, list) or not all(isinstance(g, str) and g.strip() and len(g) <= 2000 for g in gaps):
        problems.add(where, "'gaps' must be a list of non-empty texts")
        gaps = []
    return {
        "week": w,
        "checked_on": checked_on,
        "method": doc.get("method") if isinstance(doc.get("method"), str) else "",
        "entries": usable,
        "plan_fact_checks": [c for c in checks if isinstance(c, dict)],
        "gaps": gaps,
    }


def valid_date(text: str) -> bool:
    if not DATE_RE.match(text):
        return False
    try:
        datetime.date.fromisoformat(text)
    except ValueError:
        return False
    return True


# ---------------------------------------------------------------------------------------------
# a day guide and its key
# ---------------------------------------------------------------------------------------------


def quiz_ids(n: int) -> tuple[list[str], list[str]]:
    prefix = f"W{week_of(n):02d}D{n:02d}"
    return [f"{prefix}-R{k}" for k in range(1, 6)], [f"{prefix}-A{k}" for k in range(1, 4)]


def parse_quiz_part(body: list[Line], expected: list[str], part: str, where: str, problems: Problems) -> list[dict]:
    """The numbered questions of one quiz subsection: 'K. [id] text', one paragraph each."""
    items: list[dict] = []
    gap = False
    for ln in body:
        if not ln.text.strip():
            gap = True
            continue
        m = QUIZ_ITEM_RE.match(ln.text)
        if m and not ln.code:
            items.append({"number": int(m.group(1)), "id": m.group(2), "text": m.group(3).strip(), "no": ln.no, "lines": [(ln.no, m.group(3).strip())]})
        elif items and not gap and not ln.code and not re.match(r"^\d+\. ", ln.text):
            items[-1]["text"] += " " + ln.text.strip()
            items[-1]["lines"].append((ln.no, ln.text.strip()))
        else:
            problems.add(f"{where}:{ln.no}", f"quiz: a question is one paragraph 'K. [id] <question>'; found: {ln.text[:60]}")
        gap = False
    if len(items) != len(expected):
        problems.add(where, f"quiz: {part} has {len(items)} questions (exactly {len(expected)} are required)")
    out: list[dict] = []
    for k, item in enumerate(items[: len(expected)], start=1):
        if item["number"] != k or item["id"] != expected[k - 1]:
            problems.add(f"{where}:{item['no']}", f"quiz: {part} question {k} must be '{k}. [{expected[k - 1]}] <question>'; found '{item['number']}. [{item['id']}]'")
        lowered = item["text"].lower()
        leaks = [(no, k_line, leak) for k_line, (no, text) in enumerate(item["lines"], start=1) if (leak := question_leak(text))]
        if leaks:
            no, k_line, leak = leaks[0]
            problems.add(f"{where}:{no}", f"quiz: {item['id']}: line {k_line} of the question is an answer or key line ('{leak}'): answers and key lines live only under ANSWER_KEYS/")
        elif "hint:" in lowered or "answer:" in lowered:
            problems.add(f"{where}:{item['no']}", f"quiz: {item['id']}: a question holds no answer and no 'hint:'")
        limit(problems, f"{where}:{item['no']}", f"quiz: question {item['id']}", item["text"], MAX_QUESTION_CHARS)
        out.append({"id": item["id"], "text": item["text"]})
    return out


def check_day(n: int, plan_day: dict, info: dict[str, dict], registry_index: dict[str, dict], problems: Problems) -> dict | None:
    """Validate one day guide; return what the generated files need from it."""
    w = week_of(n)
    wname = week_name(w)
    where = f"{wname}/DAY_{n:02d}.md"
    text = read_source(WEEKS_DIR / wname / f"DAY_{n:02d}.md", where, problems)
    if text is None:
        return None
    lines, unclosed = scan(text)
    check_structure(lines, unclosed, where, problems)
    check_plain_markdown(lines, where, problems)
    friday = is_friday(n)
    date = day_date(n)
    weekday = WEEKDAYS[(n - 1) % 5]
    recall_ids, applied_ids = quiz_ids(n)
    day: dict = {
        "n": n,
        "week": w,
        "where": where,
        "text": text,
        "topic": "",
        "header": {},
        "objectives": [],
        "hand_in": plan_day["texts"][3],
        "resource_rows": [],
        "blocks": {},
        "commands": None,
        "mapping": "",
        "quiz": {"recall": [], "applied": []},
        "hint_ladder": "",
        "troubleshooting": "",
        "evidence": "",
        "extend": "",
        "spaced": {},
        "reflection": "",
        "words": len(text.split()),
    }

    # title
    title_form = f"# G9 {wname} / DAY_{n:02d} — "
    first = lines[0].text if lines else ""
    if not first.startswith(title_form) or not first[len(title_form) :].strip():
        problems.add(f"{where}:1", f"line 1 must be '{title_form}<topic>'")
    else:
        day["topic"] = first[len(title_form) :].strip()
        limit(problems, where, "topic", day["topic"], MAX_TITLE_CHARS)

    preamble, sections = split_sections(lines[1:], 2)

    # metadata lines
    meta = nonblank(preamble)
    expected_meta = [
        ("- Status: PROPOSED", None),
        (f"- Day id: {day_id(n)}", None),
        (f"- Original calendar date: {date.isoformat()} ({weekday})", None),
        (DURATION_LINE, None),
        ("- Lab authorization: ", "<the exact synthetic range, platform and rules this day works under>"),
        ("- Dependencies: ", "<earlier days, tools, access tier>"),
        ("- Primary provider: ", "<one provider> | Secondary: <one provider or none>"),
    ]
    values: list[str] = []
    if len(meta) != len(expected_meta):
        problems.add(where, f"metadata: exactly {len(expected_meta)} lines stand between the title and '## Objectives' (Status, Day id, Original calendar date, Duration, Lab authorization, Dependencies, Primary provider); found {len(meta)}")
    for k, (prefix, placeholder) in enumerate(expected_meta, start=1):
        got = meta[k - 1].text if k <= len(meta) else ""
        if placeholder is None:
            if got != prefix:
                problems.add(where, f"metadata line {k} must be '{prefix}'")
            values.append("")
        else:
            value = got[len(prefix) :].strip() if got.startswith(prefix) else ""
            if not value:
                problems.add(where, f"metadata line {k} must be '{prefix}{placeholder}'")
            values.append(value)
    provider = re.fullmatch(r"(.+?) \| Secondary: (.+)", values[6]) if values[6] else None
    if values[6] and not provider:
        problems.add(where, "metadata line 7 must be '- Primary provider: <one provider> | Secondary: <one provider or none>'")
    limit(problems, where, "the 'Lab authorization' line", values[4], MAX_SHORT_CHARS)
    limit(problems, where, "the 'Dependencies' line", values[5], MAX_SHORT_CHARS)
    limit(problems, where, "the 'Primary provider' line", values[6], MAX_PROVIDER_CHARS)
    primary, secondary = (provider.group(1).strip(), provider.group(2).strip()) if provider else ("", "")
    day["header"] = {
        "status": "PROPOSED",
        "date": date.isoformat(),
        "weekday": weekday,
        "lab_authorization": values[4],
        "dependencies": values[5],
        "provider": primary if secondary.lower() in ("", "none") else f"{primary}; secondary: {secondary}",
    }

    # sections: known, once, in order
    keys: list[str | None] = []
    show = {letter: (f"{letter} — {RECALL_TITLE} ({BLOCK_MINUTES[letter]} min)" if letter == "R" else f"{letter} — <title> ({BLOCK_MINUTES[letter]} min)") for letter in BLOCK_LETTERS}
    for section in sections:
        prefix = BLOCK_PREFIX_RE.match(section.title)
        if section.title in DAY_FIXED_BEFORE or section.title in DAY_FIXED_AFTER:
            keys.append(section.title)
        elif prefix:
            letter = prefix.group(1)
            keys.append(letter)
            m = BLOCK_HEAD_RE.match(section.title)
            if not m or int(m.group(3)) != BLOCK_MINUTES[letter] or (letter == "R" and m.group(2) != RECALL_TITLE):
                problems.add(f"{where}:{section.no}", f"block {letter} must be '## {show[letter]}'; found '## {section.title}'")
            elif len(m.group(2)) > MAX_BLOCK_TITLE_CHARS:
                problems.add(f"{where}:{section.no}", f"block {letter} title is {len(m.group(2))} characters (maximum {MAX_BLOCK_TITLE_CHARS})")
        else:
            keys.append(None)
    found = check_order(sections, keys, DAY_ORDER, where, problems, show)

    # objectives
    objective_lines = nonblank(found["Objectives"].body) if "Objectives" in found else []
    if "Objectives" in found and len(objective_lines) != 3:
        problems.add(where, f"{len(objective_lines)} objective lines (exactly 3 are required)")
    for k, ln in enumerate(objective_lines[:3], start=1):
        column = "ABC"[k - 1]
        m = OBJECTIVE_RE.match(ln.text)
        if not m:
            problems.add(f"{where}:{ln.no}", f"objective line {k} is not '{k}. <seed text of column {column}> — <criterion> [competency, ...]' ('[]' when no competency fits): {ln.text[:80]}")
            continue
        if int(m.group(1)) != k:
            problems.add(f"{where}:{ln.no}", f"objective line {k} is numbered {m.group(1)}")
        objective = m.group(2).strip()
        seed_text = plan_day["texts"][k - 1]
        if not objective.startswith(seed_text + " — "):
            problems.add(f"{where}:{ln.no}", f"objective {k} must start with the seed text of column {column} '{seed_text} — ' and continue with its success criterion (python3 -I {SELF} --seed-text {day_id(n)})")
        if len(objective) > MAX_OBJECTIVE_CHARS:
            problems.add(f"{where}:{ln.no}", f"objective {k} is {len(objective)} characters (maximum {MAX_OBJECTIVE_CHARS})")
        ids = [c.strip() for c in m.group(3).split(",")] if m.group(3).strip() else []
        if len(ids) > 3 or len(set(ids)) != len(ids):
            problems.add(f"{where}:{ln.no}", f"objective {k}: zero to three distinct competencies, got {ids}")
        for c in ids:
            if c not in info:
                problems.add(f"{where}:{ln.no}", f"objective {k}: unknown competency '{c}' (the ids are those of data/competency-packs/*/pack.json)")
        day["objectives"].append({"text": objective, "competency_ids": ids})

    # hand-in
    if "Hand-in" in found:
        parts = paragraphs(found["Hand-in"].body)
        seed_text = plan_day["texts"][3]
        if len(parts) != 1:
            problems.add(where, f"'## Hand-in' is one paragraph '<seed text of column D> — <acceptance criteria>'; found {len(parts)} paragraphs")
        else:
            hand_in = parts[0]
            if not hand_in.startswith(seed_text + " — "):
                problems.add(where, f"hand-in must start with the seed text of column D '{seed_text} — ' and continue with its acceptance criteria (python3 -I {SELF} --seed-text {day_id(n)})")
            limit(problems, where, "hand-in", hand_in, MAX_HAND_IN_CHARS)
            day["hand_in"] = hand_in

    # resources
    if "Resources" in found:
        rows = parse_table(found["Resources"].body, RESOURCES_HEADER, where, "resources", problems)
        if rows is not None:
            if not 1 <= len(rows) <= MAX_RESOURCE_ROWS:
                problems.add(where, f"resources: {len(rows)} rows (1 to {MAX_RESOURCE_ROWS} are required)")
            seen_rows: set[tuple[str, str]] = set()
            for k, (ident, use, block) in enumerate(rows, start=1):
                ident = ident.strip("`")
                if ident and ident not in registry_index:
                    problems.add(where, f"resources row {k}: '{ident}' is not an entry of a RESOURCES.json (an id is written wNN-...; add the entry to the week's registry first)")
                if block and block not in BLOCK_LETTERS:
                    problems.add(where, f"resources row {k}: block '{block}' is not one of R, A, B, C, D")
                if (ident, block) in seen_rows:
                    problems.add(where, f"resources row {k} repeats '{ident}' for block {block}")
                seen_rows.add((ident, block))
                day["resource_rows"].append((ident, use, block))
            hands_on = any(block in ("B", "C") and registry_index.get(ident, {}).get("kind") in HANDS_ON_KINDS for ident, _use, block in day["resource_rows"])
            if rows and not hands_on and not LOCAL_LAB_ONLY_RE.search(values[4]):
                problems.add(where, "resources: no hands-on exercise: at least one row of block B or C names an entry of kind MODULE, ROOM, LAB, WARGAME, MACHINE or CHALLENGE, unless the 'Lab authorization' line says the day is 'local-lab only'")

    # blocks
    for letter in BLOCK_LETTERS:
        if letter not in found:
            continue
        section = found[letter]
        m = BLOCK_HEAD_RE.match(section.title)
        count = words_of(section.body)
        minimum = MIN_WORDS_FRIDAY_BLOCK if friday else (MIN_WORDS_RECALL if letter == "R" else MIN_WORDS_BLOCK)
        if count < minimum:
            problems.add(where, f"block {letter} has {count} words (minimum {minimum}{' on a Friday' if friday else ''})")
        markdown = markdown_of(section.body)
        limit(problems, where, f"block {letter}", markdown, MAX_BLOCK_CHARS)
        day["blocks"][letter] = {"title": m.group(2) if m else letter, "markdown": markdown}
    if friday:
        exam_block = "D" if n == 55 else "A"  # day 55: block A is a lesson and the exam sits in block D (plan, week 11)
        wanted = {exam_block: ["Part 1"], "C": ["Part 2", "Part 3"], "D": ["Part 4"] + (["Part 1"] if n == 55 else [])}
        for letter in ("A", "C", "D"):
            body_text = day["blocks"].get(letter, {}).get("markdown")
            if body_text is None or letter not in wanted:
                continue
            named = parts_named(body_text)
            missing = [x for x in ["ASSESSMENT.md", *wanted[letter]] if not (x in body_text if x == "ASSESSMENT.md" else int(x[-1]) in named)]
            if missing:
                problems.add(where, f"block {letter} of a Friday refers to 'ASSESSMENT.md' and '{wanted[letter][0]}' (block {exam_block}: the knowledge exam, Part 1; C: the scored practical, Parts 2 and 3; D: scoring and the report, Part 4); missing: {missing}")

    # commands
    if "Commands proposed for review" in found:
        rows = parse_table(found["Commands proposed for review"].body, COMMANDS_HEADER, where, "commands", problems)
        if rows is not None:
            if not friday and len(rows) < MIN_COMMAND_ROWS:
                problems.add(where, f"commands: {len(rows)} rows (at least {MIN_COMMAND_ROWS} are required on a technical day; only a Friday may have none)")
            for k, row in enumerate(rows, start=1):
                if row[0] != str(k):
                    problems.add(where, f"commands row {k}: the '#' cell must be {k}; found '{row[0]}'")
            if rows:
                day["commands"] = markdown_of(found["Commands proposed for review"].body)
                limit(problems, where, "the commands table", day["commands"], MAX_TABLE_CHARS)

    # offense, detection, remediation, retest
    if "Offense, detection, remediation, retest" in found:
        rows = parse_table(found["Offense, detection, remediation, retest"].body, MAPPING_HEADER, where, "mapping", problems)
        if rows is not None:
            if not rows:
                problems.add(where, "mapping: at least one row is required (a day without an offensive mechanism writes 'not applicable: <reason>' in the first cell)")
            for k, row in enumerate(rows, start=1):
                if row[0].lower().startswith("not applicable") and not re.match(r"^not applicable: \S", row[0]):
                    problems.add(where, f"mapping row {k}: a day without an offensive mechanism writes 'not applicable: <reason>' in the first cell")
            day["mapping"] = markdown_of(found["Offense, detection, remediation, retest"].body)
            limit(problems, where, "the mapping table", day["mapping"], MAX_LIST_CHARS)

    # quiz
    if "Quiz" in found:
        lead, parts3 = split_sections(found["Quiz"].body, 3)
        if nonblank(lead):
            problems.add(f"{where}:{nonblank(lead)[0].no}", f"quiz: text outside the subsections '### {QUIZ_RECALL}' and '### {QUIZ_APPLIED}'")
        titles = [p.title for p in parts3]
        if titles != [QUIZ_RECALL, QUIZ_APPLIED]:
            problems.add(where, f"quiz: the subsections are '### {QUIZ_RECALL}' then '### {QUIZ_APPLIED}'; found {titles}")
        for p in parts3:
            if p.title == QUIZ_RECALL and not day["quiz"]["recall"]:
                day["quiz"]["recall"] = parse_quiz_part(p.body, recall_ids, "recall", where, problems)
            elif p.title == QUIZ_APPLIED and not day["quiz"]["applied"]:
                day["quiz"]["applied"] = parse_quiz_part(p.body, applied_ids, "applied", where, problems)

    # hint ladder
    if "Hint ladder" in found:
        hint_lines = nonblank(found["Hint ladder"].body)
        if len(hint_lines) != 4:
            problems.add(where, f"hint ladder: four numbered lines are required ('1. No hint: ', '2. Conceptual hint: ', '3. Scoped technical hint: ', '4. Instructor explanation: '); found {len(hint_lines)}")
        for k, ln in enumerate(hint_lines[:4], start=1):
            prefix = f"{k}. {HINT_LABELS[k - 1]}: "
            if k == 4:
                if ln.text != HINT_LINE_4:
                    problems.add(f"{where}:{ln.no}", f"hint ladder line 4 must be '{HINT_LINE_4}'")
            elif not ln.text.startswith(prefix) or not ln.text[len(prefix) :].strip():
                problems.add(f"{where}:{ln.no}", f"hint ladder line {k} must be '{prefix}<text>'")
        day["hint_ladder"] = "\n".join(ln.text for ln in hint_lines[:3])
        limit(problems, where, "the hint ladder", day["hint_ladder"], MAX_PARAGRAPH_CHARS)

    # troubleshooting and rollback
    if "Troubleshooting and rollback" in found:
        trouble = nonblank(found["Troubleshooting and rollback"].body)
        if len(trouble) != len(TROUBLESHOOTING_LABELS):
            problems.add(where, f"troubleshooting: the seven lines {', '.join(repr(f'- {x}: ') for x in TROUBLESHOOTING_LABELS)} are required, in this order; found {len(trouble)} lines")
        for ln, label in zip(trouble, TROUBLESHOOTING_LABELS):
            prefix = f"- {label}: "
            if not ln.text.startswith(prefix) or not ln.text[len(prefix) :].strip():
                problems.add(f"{where}:{ln.no}", f"troubleshooting: this line must be '{prefix}<text>' (the seven labelled lines, in the contract's order)")
        day["troubleshooting"] = markdown_of(found["Troubleshooting and rollback"].body)
        limit(problems, where, "troubleshooting", day["troubleshooting"], MAX_LIST_CHARS)

    # evidence and acceptance
    if "Evidence and acceptance" in found:
        evidence = nonblank(found["Evidence and acceptance"].body)
        present: set[str] = set()
        for ln in evidence:
            label = next((lb for lb in EVIDENCE_LABELS if ln.text.startswith(f"- {lb}: ") and ln.text[len(lb) + 4 :].strip()), None)
            if label is not None:
                present.add(label)
        for label in EVIDENCE_LABELS:
            if label not in present:
                problems.add(where, f"evidence: no '- {label}: ' line (at least one line of each of the three labels)")
        day["evidence"] = markdown_of(found["Evidence and acceptance"].body)
        limit(problems, where, "evidence and acceptance", day["evidence"], MAX_LIST_CHARS)

    # expert extension
    if "Expert extension (optional)" in found:
        day["extend"] = markdown_of(found["Expert extension (optional)"].body)
        if not day["extend"].strip():
            problems.add(where, "'## Expert extension (optional)' is empty: name the work that can be dropped without breaking the 480 hours")
        limit(problems, where, "the expert extension", day["extend"], MAX_PARAGRAPH_CHARS)

    # spaced recall
    if "Spaced recall" in found:
        spaced = nonblank(found["Spaced recall"].body)
        forms = [f"- {label}: " for label, _offset, _count in SPACED_LINES]
        if len(spaced) != 4 or any(not ln.text.startswith(form) for ln, form in zip(spaced, forms)):
            problems.add(where, f"spaced recall: the four lines {', '.join(repr(f) for f in forms)} are required, in this order")
        else:
            counts: dict[str, int] = {}
            for ln, (label, offset, count) in zip(spaced, SPACED_LINES):
                ids = [x.strip() for x in ln.text[len(label) + 4 :].split(",") if x.strip()]
                if len(ids) != count:
                    problems.add(f"{where}:{ln.no}", f"spaced recall: '- {label}: ' names {count} recall identifier{'s' if count > 1 else ''}; found {len(ids)}")
                for ident in ids:
                    if ident not in recall_ids:
                        problems.add(f"{where}:{ln.no}", f"spaced recall: '{ident}' is not a recall identifier of the day ({recall_ids[0]} to {recall_ids[-1]})")
                    counts[ident] = counts.get(ident, 0) + 1
                    day["spaced"][ident] = offset
            for ident in recall_ids:
                if counts.get(ident, 0) != 1:
                    problems.add(where, f"spaced recall: {ident} appears {counts.get(ident, 0)} times (each of the five recall identifiers exactly once)")

    # reflection
    if "Reflection" in found:
        if [ln.text for ln in nonblank(found["Reflection"].body)] != REFLECTION_LINES:
            problems.add(where, f"reflection: the section holds the three lines {', '.join(repr(x) for x in REFLECTION_LINES)}")
        day["reflection"] = markdown_of(found["Reflection"].body)

    # words
    if not MIN_WORDS_FILE <= day["words"] <= MAX_WORDS_FILE:
        problems.add(where, f"{day['words']} words ({MIN_WORDS_FILE:,} to {MAX_WORDS_FILE:,} are required in the whole file)")
    return day


def check_sources_named(text: str, where: str, registry_index: dict[str, dict], problems: Problems) -> None:
    for ident in REGISTRY_ID_IN_TEXT_RE.findall(text):
        if ident not in registry_index:
            problems.add(where, f"source names '{ident}', which is not an entry of a RESOURCES.json")


def check_day_key(n: int, registry_index: dict[str, dict], problems: Problems) -> dict | None:
    w = week_of(n)
    wname = week_name(w)
    where = f"{wname}/ANSWER_KEYS/DAY_{n:02d}_KEY.md"
    text = read_source(WEEKS_DIR / wname / "ANSWER_KEYS" / f"DAY_{n:02d}_KEY.md", where, problems)
    if text is None:
        return None
    lines, unclosed = scan(text)
    check_structure(lines, unclosed, where, problems)
    title = f"# Instructor key — G9 {wname} / DAY_{n:02d}"
    if not lines or lines[0].text != title:
        problems.add(f"{where}:1", f"line 1 must be '{title}'")
    preamble, sections = split_sections(lines[1:], 2)
    if [ln.text for ln in nonblank(preamble)] != [KEY_NOTICE]:
        problems.add(where, f"the line '{KEY_NOTICE}' stands alone between the title and '## Quiz key'")
    found = check_order(sections, [s.title if s.title in DAY_KEY_SECTIONS else None for s in sections], DAY_KEY_SECTIONS, where, problems)
    key: dict = {"where": where, "quiz": {}, "challenge": "", "hint4": "", "overclaims": ""}
    recall_ids, applied_ids = quiz_ids(n)
    expected = recall_ids + applied_ids
    if "Quiz key" in found:
        lead, entries = split_sections(found["Quiz key"].body, 3)
        if nonblank(lead):
            problems.add(f"{where}:{nonblank(lead)[0].no}", "quiz key: text before the first '### [id]' heading")
        got = [(KEY_HEAD_RE.match(e.title).group(1) if KEY_HEAD_RE.match(e.title) else e.title) for e in entries]
        if got != expected:
            problems.add(where, f"quiz key: the eight '### [id]' headings are {expected[0]} to {expected[4]} and {expected[5]} to {expected[7]}, in this order; found {got}")
        for entry, ident in zip(entries, got):
            if ident not in expected or ident in key["quiz"]:
                continue
            fields, stray = labelled_fields(entry.body, DAY_KEY_LABELS)
            if [label for label, _ in fields] != DAY_KEY_LABELS or stray or not all(value for _, value in fields):
                problems.add(f"{where}:{entry.no}", f"quiz key [{ident}]: the three lines '- Answer: ', '- Marking: ' and '- Source: ' are required, in this order, each with its text")
                continue
            values = dict(fields)
            limit(problems, where, f"quiz key [{ident}]: the answer", values["Answer"], MAX_KEY_TEXT_CHARS)
            limit(problems, where, f"quiz key [{ident}]: the marking", values["Marking"], MAX_SHORT_CHARS)
            limit(problems, where, f"quiz key [{ident}]: the source", values["Source"], MAX_SOURCE_CHARS)
            check_sources_named(values["Source"], f"{where}:{entry.no} [{ident}]", registry_index, problems)
            key["quiz"][ident] = {"answer": values["Answer"], "marking": values["Marking"], "source": values["Source"]}
    for title_, field in zip(DAY_KEY_SECTIONS[1:], ("challenge", "hint4", "overclaims")):
        if title_ in found:
            key[field] = markdown_of(found[title_].body)
            if not key[field].strip():
                problems.add(where, f"'## {title_}' is empty")
            limit(problems, where, f"'## {title_}'", key[field], MAX_KEY_SECTION_CHARS)
    return key


# ---------------------------------------------------------------------------------------------
# the Friday assessment and its key
# ---------------------------------------------------------------------------------------------


def exam_ids(w: int) -> list[str]:
    return [f"W{w:02d}X-Q{k:02d}" for k in range(1, 21)]


def check_assessment(w: int, info: dict[str, dict], problems: Problems) -> dict | None:
    wname = week_name(w)
    where = f"{wname}/ASSESSMENT.md"
    text = read_source(WEEKS_DIR / wname / "ASSESSMENT.md", where, problems)
    if text is None:
        return None
    lines, unclosed = scan(text)
    check_structure(lines, unclosed, where, problems)
    check_plain_markdown(lines, where, problems)
    code = exam_code(w)
    kind = "CAPSTONE" if w == 12 else "BLIND_PRACTICAL"
    exam: dict = {"where": where, "week": w, "text": text, "code": code, "kind": kind, "competency_id": "", "difficulty": 0, "title": "", "task_statement": "", "questions": []}
    title = f"# G9 {wname} — Friday assessment (day {5 * w:02d})"
    if not lines or lines[0].text != title:
        problems.add(f"{where}:1", f"line 1 must be '{title}'")
    preamble, sections = split_sections(lines[1:], 2)

    # metadata
    meta = [ln.text for ln in nonblank(preamble)]
    forms = [f"- Code: {code}", f"- Kind: {kind}", "- Competency: ", "- Difficulty: ", "- Title: "]
    if len(meta) != len(forms):
        problems.add(where, f"metadata: exactly five lines stand between the title and '## Rules' (Code, Kind, Competency, Difficulty, Title); found {len(meta)}")
    meta += [""] * (len(forms) - len(meta))
    for k in (0, 1):
        if meta[k] != forms[k]:
            problems.add(where, f"metadata line {k + 1} must be '{forms[k]}'")
    competency = meta[2][len(forms[2]) :].strip() if meta[2].startswith(forms[2]) else ""
    if not competency:
        problems.add(where, "metadata line 3 must be '- Competency: <one competency id of the eight packs>'")
    elif competency not in info:
        problems.add(where, f"unknown competency '{competency}' (the ids are those of data/competency-packs/*/pack.json)")
    exam["competency_id"] = competency
    raw = meta[3][len(forms[3]) :].strip() if meta[3].startswith(forms[3]) else ""
    if not re.fullmatch(r"[1-5]", raw):
        problems.add(where, f"metadata line 4 must be '- Difficulty: <1 to 5>'; difficulty '{raw}' must be an integer 1..5")
    else:
        exam["difficulty"] = int(raw)
        band = info.get(competency, {}).get("band")
        if band and not band[0] <= exam["difficulty"] <= band[1]:
            problems.add(where, f"difficulty {raw} is outside the difficulty band {band[0]}..{band[1]} of '{competency}'")
    exam["title"] = meta[4][len(forms[4]) :].strip() if meta[4].startswith(forms[4]) else ""
    if not exam["title"]:
        problems.add(where, "metadata line 5 must be '- Title: <at most 200 characters>'")
    limit(problems, where, "title", exam["title"], MAX_TITLE_CHARS)

    found = check_order(sections, [s.title if s.title in EXAM_SECTIONS else None for s in sections], EXAM_SECTIONS, where, problems)
    for name in EXAM_SECTIONS:
        if name in found and words_of(found[name].body) < MIN_WORDS_EXAM_SECTION:
            problems.add(where, f"section '## {name}' has {words_of(found[name].body)} words (minimum {MIN_WORDS_EXAM_SECTION})")

    # Part 1: the twenty questions
    part1 = EXAM_SECTIONS[1]
    expected = exam_ids(w)
    if part1 in found:
        items: list[dict] = []
        for ln in found[part1].body:
            if not ln.code and EXAM_ITEM_START_RE.match(ln.text):
                m = EXAM_ITEM_RE.match(ln.text)
                k = len(items) + 1
                want = expected[k - 1] if k <= 20 else "id"
                if not m or m.group(3) not in QUESTION_MIX:
                    problems.add(f"{where}:{ln.no}", f"Part 1: question {k} must be '{k}. [{want}] (CONCEPT|INTERPRET|CONTROL|SYNTHESIS) <question>': {ln.text[:60]}")
                    items.append({"id": f"?{k}", "kind": "", "lines": [], "nos": [], "codes": [], "options": [], "no": ln.no})
                    continue
                if int(m.group(1)) != k or m.group(2) != want:
                    problems.add(f"{where}:{ln.no}", f"Part 1: question {k} must be '{k}. [{want}] ...'; found '{m.group(1)}. [{m.group(2)}]'")
                items.append({"id": m.group(2), "kind": m.group(3), "lines": [m.group(4).strip()], "nos": [ln.no], "codes": [False], "options": [], "no": ln.no})
            elif items:
                items[-1]["lines"].append(ln.text)
                items[-1]["nos"].append(ln.no)
                items[-1]["codes"].append(ln.code)
                o = OPTION_RE.match(ln.text) if not ln.code else None
                if o:
                    items[-1]["options"].append(o.group(1))
        if len(items) != 20:
            problems.add(where, f"Part 1: {len(items)} questions (exactly 20 are required, {expected[0]} to {expected[-1]})")
        for item in items:
            body_text = "\n".join(item["lines"]).strip("\n")
            mcq = bool(item["options"])
            if mcq and item["options"] != ["A", "B", "C", "D"]:
                problems.add(f"{where}:{item['no']}", f"Part 1: {item['id']}: a multiple-choice question lists the options 'A.' to 'D.' in order, one a line; found {item['options']}")
            if item["kind"] == "INTERPRET" and SAMPLE_MARK not in body_text:
                problems.add(f"{where}:{item['no']}", f"Part 1: {item['id']}: an INTERPRET question gives what is to be interpreted in the question, marked '{SAMPLE_MARK}'")
            lowered = body_text.lower()
            leaks = [(no, leak) for line_text, no, code in zip(item["lines"], item["nos"], item["codes"]) if (leak := question_leak(line_text, code))]
            if leaks:
                problems.add(f"{where}:{leaks[0][0]}", f"Part 1: {item['id']}: a line of the question states or marks the answer ('{leaks[0][1]}'): keys live only under ANSWER_KEYS/")
            elif "hint:" in lowered or re.search(r"(?<![a-z])answer:", lowered):
                problems.add(f"{where}:{item['no']}", f"Part 1: {item['id']}: a question holds no answer and no 'hint:'")
            limit(problems, f"{where}:{item['no']}", f"Part 1: question {item['id']}", body_text, MAX_EXAM_QUESTION_CHARS)
            exam["questions"].append({"id": item["id"], "kind": item["kind"], "text": body_text, "mcq": mcq})
        for name, minimum in QUESTION_MIX.items():
            count = sum(1 for q in exam["questions"] if q["kind"] == name)
            if count < minimum:
                problems.add(where, f"Part 1: question mix: {count} {name} questions (at least {minimum} are required; the mix is 6 CONCEPT, 6 INTERPRET, 4 CONTROL, 4 SYNTHESIS)")
        multiple = sum(1 for q in exam["questions"] if q["mcq"])
        if multiple < MIN_MULTIPLE_CHOICE:
            problems.add(where, f"Part 1: {multiple} multiple-choice questions (at least {MIN_MULTIPLE_CHOICE} are required)")

    # the task statement: '## Rules' to the end of Part 4
    if EXAM_SECTIONS[0] in found and EXAM_SECTIONS[4] in found:
        first_no = found[EXAM_SECTIONS[0]].no
        last = found[EXAM_SECTIONS[4]]
        last_no = last.body[-1].no if last.body else last.no
        exam["task_statement"] = "\n".join(ln.text for ln in lines if first_no <= ln.no <= last_no).strip()
        if len(exam["task_statement"]) > MAX_TASK_STATEMENT_CHARS:
            problems.add(where, f"task statement ('## Rules' to the end of Part 4) is {len(exam['task_statement'])} characters (maximum {MAX_TASK_STATEMENT_CHARS})")
    return exam


def check_assessment_key(w: int, exam: dict | None, problems: Problems) -> dict | None:
    wname = week_name(w)
    where = f"{wname}/ANSWER_KEYS/ASSESSMENT_KEY.md"
    text = read_source(WEEKS_DIR / wname / "ANSWER_KEYS" / "ASSESSMENT_KEY.md", where, problems)
    if text is None:
        return None
    lines, unclosed = scan(text)
    check_structure(lines, unclosed, where, problems)
    if not lines or lines[0].level != 1:
        problems.add(f"{where}:1", "line 1 must be a '# ' title")
    key: dict = {"where": where, "questions": {}, "practical": "", "gate": ""}
    expected = exam_ids(w)
    multiple = {q["id"] for q in (exam or {}).get("questions", []) if q["mcq"]}
    # the '### [id]' entries: each runs to the next heading of level 1 to 3
    entries: list[Section] = []
    current: Section | None = None
    for ln in lines:
        if 1 <= ln.level <= 3:
            current = None
            if ln.level == 3:
                current = Section(heading_text(ln), ln.no, [])
                entries.append(current)
        elif current is not None:
            current.body.append(ln)
    got = [(KEY_HEAD_RE.match(e.title).group(1) if KEY_HEAD_RE.match(e.title) else e.title) for e in entries]
    if got != expected:
        problems.add(where, f"the twenty '### [id]' headings are {expected[0]} to {expected[-1]}, in this order; found {got if len(got) <= 6 else got[:6] + ['...']} ({len(got)} headings)")
    for entry, ident in zip(entries, got):
        if ident not in expected or ident in key["questions"]:
            continue
        fields, stray = labelled_fields(entry.body, ["Answer", "Why", "Distractors"])
        labels = [label for label, _ in fields]
        values = dict(fields)
        if labels[:2] != ["Answer", "Why"] or labels not in (["Answer", "Why"], ["Answer", "Why", "Distractors"]) or stray or not all(values.values()):
            problems.add(f"{where}:{entry.no}", f"[{ident}]: '- Answer: ' and '- Why: ' are required, in this order, each with its text (then '- Distractors: ' for a multiple-choice question)")
            continue
        if ident in multiple:
            if "Distractors" not in values:
                problems.add(f"{where}:{entry.no}", f"[{ident}]: the key of a multiple-choice question needs '- Distractors: ' (why each other option is wrong)")
            letter = re.match(r"^\W{0,2}([A-D])(?![A-Za-z0-9])", values["Answer"])
            if not letter:
                problems.add(f"{where}:{entry.no}", f"[{ident}]: the answer of a multiple-choice question starts with its option letter (A to D)")
            elif "Distractors" in values:
                missing = [x for x in "ABCD" if x != letter.group(1) and not option_named(x, values["Distractors"])]
                if missing:
                    problems.add(f"{where}:{entry.no}", f"[{ident}]: '- Distractors: ' says why each other option is wrong; not named: {', '.join(missing)}")
        for label in labels:
            limit(problems, where, f"[{ident}]: '- {label}: '", values[label], MAX_KEY_TEXT_CHARS)
        key["questions"][ident] = {"answer": values["Answer"], "why": values["Why"], "distractors": values.get("Distractors")}
    _preamble, sections = split_sections(lines[1:], 2)
    names = [s.title for s in sections]
    for name in (EXAM_KEY_PRACTICAL, EXAM_KEY_GATE):
        if names.count(name) != 1:
            problems.add(where, f"missing section '## {name}'" if name not in names else f"section '## {name}' appears twice")
    by_name = {s.title: s for s in sections}
    if names.count(EXAM_KEY_PRACTICAL) == 1 and names.count(EXAM_KEY_GATE) == 1:
        if names.index(EXAM_KEY_PRACTICAL) > names.index(EXAM_KEY_GATE):
            problems.add(where, f"sections are out of order: '## {EXAM_KEY_GATE}' stands before '## {EXAM_KEY_PRACTICAL}'")
    if EXAM_KEY_PRACTICAL in by_name:
        key["practical"] = markdown_of(by_name[EXAM_KEY_PRACTICAL].body)
        numbers = set(re.findall(r"(?<![0-9.])\d+(?![0-9])", key["practical"]))
        missing = [x for x in SCORE_PARTS if x not in numbers]
        if missing:
            problems.add(where, f"'## {EXAM_KEY_PRACTICAL}' gives the four score parts (20, 40, 25 and 15 points) and their bands; not named: {', '.join(missing)}")
        limit(problems, where, f"'## {EXAM_KEY_PRACTICAL}'", key["practical"], MAX_PRACTICAL_KEY_CHARS)
    if EXAM_KEY_GATE in by_name:
        key["gate"] = markdown_of(by_name[EXAM_KEY_GATE].body)
        if not key["gate"].strip():
            problems.add(where, f"'## {EXAM_KEY_GATE}' is empty")
        limit(problems, where, f"'## {EXAM_KEY_GATE}'", key["gate"], MAX_LIST_CHARS)
    return key


# ---------------------------------------------------------------------------------------------
# the week guide, the labs, the fixtures
# ---------------------------------------------------------------------------------------------


def check_week_guide(w: int, plan_week: dict, problems: Problems) -> dict:
    wname = week_name(w)
    where = f"{wname}/WEEK_GUIDE.md"
    out = {key: "" for _label, key in WEEK_PARAGRAPHS}
    text = read_source(WEEKS_DIR / wname / "WEEK_GUIDE.md", where, problems)
    if text is None:
        return out
    lines, unclosed = scan(text)
    check_structure(lines, unclosed, where, problems)
    title = f"# G9 {wname} — {plan_week['title']}"
    if not lines or lines[0].text != title:
        problems.add(f"{where}:1", f"line 1 must be '{title}' (the week title of the plan)")
    preamble, sections = split_sections(lines[1:], 2)
    order: list[str] = []
    for paragraph in paragraphs(preamble):
        m = re.match(r"^\*\*(Purpose|Lab stack|Competencies|Accelerator)\.\*\* (.+)$", paragraph)
        if not m:
            continue
        key = dict(WEEK_PARAGRAPHS)[m.group(1)]
        if m.group(1) in order:
            problems.add(where, f"the '**{m.group(1)}.**' paragraph appears twice")
            continue
        order.append(m.group(1))
        out[key] = m.group(2).strip()
        limit(problems, where, f"the '**{m.group(1)}.**' paragraph", out[key], MAX_PARAGRAPH_CHARS)
    for label, key in WEEK_PARAGRAPHS:
        if not out[key]:
            problems.add(where, f"missing the '**{label}.**' paragraph before '## Goals'")
    if order != [label for label, _key in WEEK_PARAGRAPHS if label in order]:
        problems.add(where, f"the paragraphs stand in the order Purpose, Lab stack, Competencies, Accelerator; found {order}")
    preamble_lines = [Line(ln.no, ln.text, 0, ln.code) for ln in preamble]
    check_plain_markdown(preamble_lines, where, problems)
    found = check_order(sections, [s.title if s.title in WEEK_SECTIONS else None for s in sections], WEEK_SECTIONS, where, problems)
    for name, section in found.items():
        if not nonblank(section.body):
            problems.add(f"{where}:{section.no}", f"section '## {name}' is empty")
    return out


def check_labs(w: int, problems: Problems) -> None:
    wname = week_name(w)
    labs_key_where = f"{wname}/ANSWER_KEYS/LABS_KEY.md"
    labs_key = read_source(WEEKS_DIR / wname / "ANSWER_KEYS" / "LABS_KEY.md", labs_key_where, problems) if (WEEKS_DIR / wname / "ANSWER_KEYS" / "LABS_KEY.md").is_file() else None
    key_headings: list[tuple[str, int]] = []  # (heading in lower case, words under it up to the next heading of its level or above)
    if labs_key is not None:
        key_lines, unclosed = scan(labs_key)
        check_structure(key_lines, unclosed, labs_key_where, problems)
        if not key_lines or key_lines[0].level != 1:
            problems.add(f"{labs_key_where}:1", "line 1 must be a '# ' title")
        for i, ln in enumerate(key_lines):
            if ln.level >= 2:
                end = next((j for j in range(i + 1, len(key_lines)) if 1 <= key_lines[j].level <= ln.level), len(key_lines))
                key_headings.append((heading_text(ln).lower(), words_of(key_lines[i + 1 : end])))
    for name, phrase in LAB_FILES:
        where = f"{wname}/LABS/{name}"
        path = WEEKS_DIR / wname / "LABS" / name
        if not path.is_file():
            continue
        text = read_source(path, where, problems)
        if text is None:
            continue
        lines, unclosed = scan(text)
        check_structure(lines, unclosed, where, problems)
        if not lines or lines[0].level != 1 or not heading_text(lines[0]):
            problems.add(f"{where}:1", "line 1 must be a '# ' title")
        _preamble, sections = split_sections(lines[1:], 2)
        guided = name == "GUIDED_LAB.md"
        required = [s for s in LAB_SECTIONS if guided or s != "Steps"]
        titles = [s.title for s in sections]
        for section in sections:
            if section.title == "Steps" and not guided:
                problems.add(f"{where}:{section.no}", "'## Steps' belongs to the guided lab only: the independent lab and the Friday practical give the task, not the steps")
            if titles.count(section.title) > 1 and section.title in LAB_SECTIONS and titles.index(section.title) != sections.index(section):
                problems.add(f"{where}:{section.no}", f"section '## {section.title}' appears twice")
            if section.title in required and not nonblank(section.body):
                problems.add(f"{where}:{section.no}", f"section '## {section.title}' is empty")
        for title in required:
            if title not in titles:
                problems.add(where, f"missing section '## {title}'")
        sequence = [t for t in dict.fromkeys(titles) if t in required]
        if sequence != [t for t in required if t in sequence]:
            problems.add(where, f"sections are out of order: {sequence}")
        if labs_key is not None:
            stem = name[: -len(".md")].lower()
            lab_title = heading_text(lines[0]).lower() if lines else ""
            named = [words for h, words in key_headings if stem in h or phrase in h or (lab_title and lab_title in h)]
            if not named:
                problems.add(labs_key_where, f"no heading names the lab '{name}': the reference observations and scoring of each lab stand under a heading with the lab's name ('{name[: -len('.md')]}', '{phrase}' or the title of the lab file)")
            elif max(named) < MIN_WORDS_LABS_KEY:
                problems.add(labs_key_where, f"the heading of the lab '{name}' has {max(named)} words under it (minimum {MIN_WORDS_LABS_KEY}): the reference observations and scoring of the lab stand under it")


def fixture_hashes_by_name(text: str, names: list[str]) -> dict[str, set[str]]:
    """The SHA-256 values the manifest attributes to each fixture. A value belongs to the last fixture named before it on its line (a table
    row: the name, then the hash); with none before it, to the fixture named by the lines above it up to a blank line (a heading, then its
    hash), else to the first one named after it."""
    out: dict[str, set[str]] = {name: set() for name in names}
    current: str | None = None
    for line in text.lower().split("\n"):
        if not line.strip():
            current = None
            continue
        spots = sorted((m.start(), n) for n in names for m in re.finditer(rf"(?<![\w./-]){re.escape(n.lower())}(?![\w-]|\.\w)", line))
        for h in SHA256_RE.finditer(line):
            before = [n for pos, n in spots if pos < h.start()]
            owner = before[-1] if before else (current or (spots[0][1] if spots else None))
            if owner is not None:
                out[owner].add(h.group(0))
        if spots:
            current = spots[0][1]
    return out


def check_fixtures(w: int, problems: Problems) -> None:
    wname = week_name(w)
    fdir = WEEKS_DIR / wname / "LABS" / "fixtures"
    where = f"{wname}/LABS/fixtures/MANIFEST.md"
    manifest_path = fdir / "MANIFEST.md"
    if not manifest_path.is_file():
        return
    text = read_source(manifest_path, where, problems)
    if text is None:
        return
    hashes: dict[str, str] = {}
    for path in sorted(p for p in fdir.rglob("*") if p.is_file() and p != manifest_path):
        hashes[path.relative_to(fdir).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    lowered = text.lower()
    for value in sorted(set(SHA256_RE.findall(lowered)) - set(hashes.values())):
        problems.add(where, f"'{value[:16]}...' is not the SHA-256 of a file under LABS/fixtures/ (a stale hash or a missing fixture)")
    attributed = fixture_hashes_by_name(text, list(hashes))
    recipe = re.search(r"\brecipe\b", lowered) is not None
    for name, value in hashes.items():
        if name not in text:
            problems.add(where, f"does not list the fixture '{name}': each fixture is listed with its SHA-256 or its generation recipe")
            continue
        for other in sorted(h for h in attributed[name] if h in hashes.values() and h != value):
            owners = sorted(n for n, v in hashes.items() if v == other)
            problems.add(where, f"the line of '{name}' holds the SHA-256 of another fixture ({', '.join(owners)}): each fixture has its own hash")
        if value not in attributed[name] and not recipe:
            problems.add(where, f"'{name}' has neither its SHA-256 ({value}) nor a generation recipe (the word 'recipe')")


def check_support_file(w: int, name: str, problems: Problems) -> None:
    where = f"{week_name(w)}/{name}"
    path = WEEKS_DIR / week_name(w) / name
    if not path.is_file():
        return
    text = read_source(path, where, problems)
    if text is None:
        return
    lines, unclosed = scan(text)
    check_structure(lines, unclosed, where, problems)
    if not lines or lines[0].level != 1:
        problems.add(f"{where}:1", "line 1 must be a '# ' title")
    if len(text.split()) < MIN_WORDS_SUPPORT_FILE:
        problems.add(where, f"{len(text.split())} words (minimum {MIN_WORDS_SUPPORT_FILE}): expected artifacts, allowed redactions, pass or fail")


# ---------------------------------------------------------------------------------------------
# one week
# ---------------------------------------------------------------------------------------------


def week_days(w: int) -> list[int]:
    return list(range(5 * w - 4, 5 * w + 1))


def check_weeks_folder(problems: Problems) -> None:
    """WEEKS/ holds W01 to W12, real folders, and nothing else (a week in a folder called W1 would be invisible to --available)."""
    if not WEEKS_DIR.is_dir():
        return
    for path in sorted(WEEKS_DIR.iterdir()):
        real = not path.is_symlink()
        if real and path.is_dir() and re.fullmatch(r"W(?:0[1-9]|1[0-2])", path.name):
            continue
        if real and path.is_file() and path.name.startswith("."):
            continue
        problems.add(f"WEEKS/{path.name}", "unexpected entry: a week folder is named W01 to W12 (a real folder, not a link) and nothing else stands in WEEKS/")


def week_has_day_file(w: int) -> bool:
    wdir = WEEKS_DIR / week_name(w)
    return wdir.is_dir() and any(re.fullmatch(r"DAY_\d{2}\.md", p.name) for p in wdir.iterdir() if p.is_file())


def week_has_every_day_file(w: int) -> bool:
    """All five day guides of the week exist: the week is submitted for checking (--available judges only these)."""
    wdir = WEEKS_DIR / week_name(w)
    return wdir.is_dir() and all((wdir / f"DAY_{n:02d}.md").is_file() for n in week_days(w))


def expected_files(w: int) -> tuple[list[str], list[str]]:
    """(the files an author writes, the files this script generates), relative to the week folder."""
    days = week_days(w)
    authored = ["RESOURCES.json", "WEEK_GUIDE.md", "ASSESSMENT.md", "EVIDENCE_CHECKLIST.md"]
    authored += [f"DAY_{n:02d}.md" for n in days]
    authored += [f"LABS/{name}" for name, _phrase in LAB_FILES] + ["LABS/fixtures/MANIFEST.md"]
    authored += [f"ANSWER_KEYS/DAY_{n:02d}_KEY.md" for n in days] + ["ANSWER_KEYS/ASSESSMENT_KEY.md", "ANSWER_KEYS/LABS_KEY.md"]
    return authored, [f"QUIZZES/DAY_{n:02d}_QUIZ.md" for n in days]


def check_week(w: int, plan_week: dict, info: dict[str, dict], registries: dict[int, dict], registry_index: dict[str, dict], problems: Problems) -> dict | None:
    wname = week_name(w)
    wdir = WEEKS_DIR / wname
    if not week_has_day_file(w):
        problems.add(wname, f"the week is not written: no day guide (DAY_{5 * w - 4:02d}.md to DAY_{5 * w:02d}.md) in {V2_REL}/WEEKS/{wname}/")
        return None
    authored, generated = expected_files(w)
    for name in authored:
        if not (wdir / name).is_file():
            problems.add(wname, f"missing file '{name}'")
    for p in sorted(wdir.rglob("*")):
        if p.is_symlink():
            problems.add(f"{wname}/{p.relative_to(wdir).as_posix()}", "is a symbolic link: a source file is a regular file of this tree")
    present = sorted(p.relative_to(wdir).as_posix() for p in wdir.rglob("*") if p.is_file())
    for name in present:
        if name not in authored and name not in generated and not name.startswith("LABS/fixtures/"):
            problems.add(f"{wname}/{name}", "unexpected file: AUTHORING.md names every file of a week (fixtures belong under LABS/fixtures/)")

    week: dict = {"number": w, "title": plan_week["title"], "days": [], "keys": {}, "exam": None, "exam_key": None}
    week["paragraphs"] = check_week_guide(w, plan_week, problems) if (wdir / "WEEK_GUIDE.md").is_file() else {key: "" for _l, key in WEEK_PARAGRAPHS}
    for plan_day in plan_week["days"]:
        n = plan_day["n"]
        if (wdir / f"DAY_{n:02d}.md").is_file():
            day = check_day(n, plan_day, info, registry_index, problems)
            if day is not None:
                week["days"].append(day)
        if (wdir / "ANSWER_KEYS" / f"DAY_{n:02d}_KEY.md").is_file():
            key = check_day_key(n, registry_index, problems)
            if key is not None:
                week["keys"][n] = key
    if (wdir / "ASSESSMENT.md").is_file():
        week["exam"] = check_assessment(w, info, problems)
    if (wdir / "ANSWER_KEYS" / "ASSESSMENT_KEY.md").is_file():
        week["exam_key"] = check_assessment_key(w, week["exam"], problems)
    check_labs(w, problems)
    check_fixtures(w, problems)
    check_support_file(w, "EVIDENCE_CHECKLIST.md", problems)

    # every other file of the week: no hidden character, no credential-like text; the learner files are kept for the leak rules
    parsed = {"RESOURCES.json", "WEEK_GUIDE.md", "ASSESSMENT.md", "EVIDENCE_CHECKLIST.md", "LABS/fixtures/MANIFEST.md", "ANSWER_KEYS/ASSESSMENT_KEY.md", "ANSWER_KEYS/LABS_KEY.md"}
    parsed |= {f"DAY_{n:02d}.md" for n in week_days(w)} | {f"ANSWER_KEYS/DAY_{n:02d}_KEY.md" for n in week_days(w)} | {f"LABS/{name}" for name, _p in LAB_FILES}
    learner: list[tuple[str, str]] = []  # (file, text); a registry contributes each of its strings
    for name in present:
        if name.startswith("QUIZZES/"):
            continue  # generated from the day guides and compared byte for byte (full run); a source rule would block --write from repairing them
        quiet = Problems() if name in parsed else problems  # a parsed file already reported its hidden character
        text = read_source(wdir / name, f"{wname}/{name}", quiet)
        if text is None:
            if name in parsed:
                continue
            if name.startswith("LABS/fixtures/"):
                problems.items[-1] = f"{wname}/{name}: not UTF-8 text: a fixture is a synthetic text file (log, toy source, sample output); give a binary by its generation recipe in MANIFEST.md"
            continue
        for no, what in sensitive_hits(text, ipv4=name != "RESOURCES.json"):
            problems.add(f"{wname}/{name}:{no}", what)
        if name == "RESOURCES.json":
            try:
                learner += [(name, s) for s in strings_of(json.loads(text))]
            except ValueError:
                pass  # reported by load_registry
        elif not name.startswith("ANSWER_KEYS/"):
            learner.append((name, text))
    for name, text in learner:
        if name == "RESOURCES.json":
            continue
        for no, line in enumerate(text.split("\n"), start=1):
            if answer_label(line):
                problems.add(f"{wname}/{name}:{no}", ANSWER_LABEL_MESSAGE)
            elif not name.startswith("LABS/fixtures/") and key_line(line):
                problems.add(f"{wname}/{name}:{no}", KEY_LINE_MESSAGE)
    week["learner"] = learner
    week["labs_key"] = labs_key_fields(w)

    # each question has its key
    for day in week["days"]:
        key = week["keys"].get(day["n"])
        if key is None:
            continue
        for q in day["quiz"]["recall"] + day["quiz"]["applied"]:
            if q["id"] not in key["quiz"] and not any(q["id"] in p for p in problems.items):
                problems.add(key["where"], f"quiz key: no key for {q['id']}")
    week["registry"] = registries.get(w)
    return week


# ---------------------------------------------------------------------------------------------
# key text in learner files (ADR-0023 decision 6)
# ---------------------------------------------------------------------------------------------


def labs_key_fields(w: int) -> list[tuple[str, str]]:
    """(heading, body) of each level-2 section of LABS_KEY.md: the reference observations and scoring of the labs."""
    path = WEEKS_DIR / week_name(w) / "ANSWER_KEYS" / "LABS_KEY.md"
    text = read_source(path, "", Problems()) if path.is_file() else None
    if text is None:
        return []
    _preamble, sections = split_sections(scan(text)[0], 2)
    return [(s.title, markdown_of(s.body)) for s in sections if nonblank(s.body)]


def key_fields(week: dict) -> list[KeyField]:
    """Every field of the keys of a week that no learner file may hold: answers (of a multiple-choice question: the text after the option
    letter), marking, rationale, distractors, reference observations, the fourth hint, overclaims, the practical, the gate, the lab keys."""
    w = week["number"]
    wname = week_name(w)
    out: list[KeyField] = []
    for n in sorted(week["keys"]):
        key = week["keys"][n]
        for ident, entry in key["quiz"].items():
            out += [KeyField("answer", ident, key["where"], entry["answer"]), KeyField("marking", ident, key["where"], entry["marking"])]
        for field, label in (("challenge", "challenge"), ("hint4", "hint 4"), ("overclaims", "overclaims")):
            out.append(KeyField(label, f"{wname}D{n:02d}", key["where"], key[field]))
    exam_key = week.get("exam_key")
    if exam_key:
        exam = week.get("exam") or {}
        multiple = {q["id"] for q in exam.get("questions", []) if q["mcq"]}
        where = exam_key["where"]
        for ident, entry in exam_key["questions"].items():
            answer = MCQ_LETTER_RE.sub("", entry["answer"], count=1) if ident in multiple else entry["answer"]
            out += [KeyField("answer", ident, where, answer), KeyField("why", ident, where, entry["why"])]
            if entry["distractors"]:
                out.append(KeyField("distractors", ident, where, entry["distractors"]))
        code = exam.get("code") or exam_code(w)
        out += [KeyField("practical", code, where, exam_key["practical"]), KeyField("gate", code, where, exam_key["gate"])]
    out += [KeyField("lab key", heading, f"{wname}/ANSWER_KEYS/LABS_KEY.md", body) for heading, body in week.get("labs_key", [])]
    return out


def build_leak_index(fields: list[KeyField]) -> LeakIndex:
    windows: dict[str, KeyField] = {}
    wholes: dict[str, KeyField] = {}
    flags: dict[str, KeyField] = {}
    for f in fields:
        words = normalize(f.text).split()
        if len(words) >= LEAK_SHINGLE_WORDS:
            for i in range(len(words) - LEAK_SHINGLE_WORDS + 1):
                windows.setdefault(" ".join(words[i : i + LEAK_SHINGLE_WORDS]), f)
        elif len(" ".join(words)) >= MIN_LEAK_CHARS:
            wholes.setdefault(" ".join(words), f)
        for m in FLAG_TOKEN_RE.finditer(f.text):
            if not FLAG_PLACEHOLDER_RE.match(m.group(1)):
                flags.setdefault(m.group(0), f)
    return LeakIndex(windows, list(wholes.items()), flags)


def option_owners(text: str) -> dict[int, str]:
    """line -> question id, for the option lines 'A.' to 'D.' of Part 1 of an assessment. They list the answers by design: the answer or
    the distractors of a multiple-choice question may equal its own options, and nothing else may be hidden there."""
    out: dict[int, str] = {}
    _preamble, sections = split_sections(scan(text)[0], 2)
    for section in sections:
        if section.title != EXAM_SECTIONS[1]:
            continue
        current: str | None = None
        for ln in section.body:
            if ln.code:
                continue
            if EXAM_ITEM_START_RE.match(ln.text):
                m = EXAM_ITEM_RE.match(ln.text)
                current = m.group(2) if m else None
            elif current is not None and OPTION_RE.match(ln.text):
                out[ln.no] = current
    return out


def leak_matches(index: LeakIndex, name: str, text: str) -> dict[tuple[KeyField, str], int]:
    """(key field, 'text' or 'flag') -> first line, for each key field whose text (a run of words) or flag the learner text holds."""
    options = option_owners(text) if name == "ASSESSMENT.md" else {}
    words: list[str] = []
    nos: list[int] = []
    for no, line in enumerate(text.split("\n"), start=1):
        found_words = normalize(line).split()
        words += found_words
        nos += [no] * len(found_words)

    def own_option(first: int, last: int, owner: KeyField) -> bool:
        """The words first..last are all in option lines of the question the key field is the answer or distractors of."""
        question = options.get(nos[first])
        return question is not None and owner.ident == question and owner.field in ("answer", "distractors") and all(options.get(nos[i]) == question for i in range(first, last + 1))

    found: dict[tuple[KeyField, str], int] = {}
    k = LEAK_SHINGLE_WORDS
    for i in range(len(words) - k + 1):
        owner = index.windows.get(" ".join(words[i : i + k]))
        if owner is not None and not own_option(i, i + k - 1, owner):
            found.setdefault((owner, "text"), nos[i])
    if index.wholes:
        padded = " " + " ".join(words) + " "
        for whole, owner in index.wholes:
            pos = padded.find(f" {whole} ")
            if pos >= 0:
                first = padded[: pos + 1].count(" ") - 1
                if not own_option(first, first + len(whole.split()) - 1, owner):
                    found.setdefault((owner, "text"), nos[first])
    for flag, owner in index.flags.items():
        pos = text.find(flag)
        if pos >= 0:
            found.setdefault((owner, "flag"), text.count("\n", 0, pos) + 1)
    return found


def quiet_week_keys(w: int, info: dict[str, dict], registry_index: dict[str, dict]) -> dict | None:
    """The keys of a week that is not being checked, read without judging them: they are forbidden text in the learner files of the others."""
    wdir = WEEKS_DIR / week_name(w)
    if not wdir.is_dir():
        return None
    quiet = Problems()
    week: dict = {"number": w, "keys": {}, "exam": None, "exam_key": None, "labs_key": labs_key_fields(w)}
    for n in week_days(w):
        if (wdir / "ANSWER_KEYS" / f"DAY_{n:02d}_KEY.md").is_file():
            key = check_day_key(n, registry_index, quiet)
            if key is not None:
                week["keys"][n] = key
    if (wdir / "ASSESSMENT.md").is_file():
        week["exam"] = check_assessment(w, info, quiet)
    if (wdir / "ANSWER_KEYS" / "ASSESSMENT_KEY.md").is_file():
        week["exam_key"] = check_assessment_key(w, week["exam"], quiet)
    return week


def leak_checks(weeks: list[dict], info: dict[str, dict], registry_index: dict[str, dict], problems: Problems) -> None:
    """No learner file of the weeks checked holds key text of any week (those not checked are read for their keys only)."""
    if not weeks:
        return
    by_number = {week["number"]: week for week in weeks}
    fields: list[KeyField] = []
    for w in range(1, 13):
        week = by_number.get(w) or quiet_week_keys(w, info, registry_index)
        if week is not None:
            fields += key_fields(week)
    index = build_leak_index(fields)
    for week in weeks:
        wname = week_name(week["number"])
        per_file: dict[str, dict[tuple[KeyField, str], int]] = {}
        for name, text in week.get("learner", []):
            for hit, no in leak_matches(index, name, text).items():
                per_file.setdefault(name, {}).setdefault(hit, no)
        for name, hits in per_file.items():
            ordered = sorted(hits.items(), key=lambda kv: (kv[1], kv[0][0].ident, kv[0][0].field, kv[0][1]))
            for (f, kind), no in ordered[:MAX_LEAK_REPORTS]:
                what = f"holds the flag of {f.ident}" if kind == "flag" else f"holds the {f.field} text of {f.ident}"
                at = "" if name == "RESOURCES.json" else f"; first match at line {no}"
                problems.add(f"{wname}/{name}", f"{what} ({f.where}){at}: {LEAK_TAIL}")
            if len(ordered) > MAX_LEAK_REPORTS:
                problems.add(f"{wname}/{name}", f"and {len(ordered) - MAX_LEAK_REPORTS} more key fields held ({LEAK_TAIL})")


# ---------------------------------------------------------------------------------------------
# generated files
# ---------------------------------------------------------------------------------------------


def generated_line(sources: str) -> str:
    return f"Generated by `{SELF}` from {sources}. Do not edit by hand; run `{WRITE_HINT}`."


def recall_queues(weeks: list[dict]) -> tuple[dict[int, list[dict]], list[dict]]:
    """day number -> its recall queue; and the questions that fall after day 60 (post-course review)."""
    queues: dict[int, list[dict]] = {n: [] for n in range(1, 61)}
    after: list[dict] = []
    for week in weeks:
        for day in week["days"]:
            texts = {q["id"]: q["text"] for q in day["quiz"]["recall"]}
            for ident in sorted(day["spaced"]):
                offset = day["spaced"][ident]
                item = {"id": ident, "from_day": day_id(day["n"]), "offset_days": offset, "text": texts.get(ident, "")}
                target = queue_day(day["n"], offset)
                due = day_date(day["n"]) + datetime.timedelta(days=offset)
                if target is None:
                    after.append(dict(item, due=due.isoformat()))
                else:
                    queues[target].append(item)
    for items in queues.values():
        items.sort(key=lambda x: (x["from_day"], x["offset_days"], x["id"]))
    after.sort(key=lambda x: (x["due"], x["id"]))
    return queues, after


def day_resources(day: dict, registry_index: dict[str, dict]) -> list[dict]:
    """The distinct registry entries a day cites, in the order of its table."""
    out: list[dict] = []
    for ident, _use, _block in day["resource_rows"]:
        entry = registry_index.get(ident)
        if entry is not None and all(e["id"] != ident for e in out):
            out.append(entry)
    return out


def manifest_resources(day: dict, registry_index: dict[str, dict]) -> list[dict]:
    out: list[dict] = []
    for entry in day_resources(day, registry_index):
        if any(r["ref"] == entry["url"] for r in out):
            continue
        suffix = f"; checked {entry['_checked_on']}"
        note = entry["access"] + (f": {entry['access_note'].strip()}" if entry["access_note"].strip() else "")
        out.append(
            {
                "ref": entry["url"],
                "kind": "EXTERNAL",
                "version": cut(" ".join(entry["version"].split()), MAX_RESOURCE_VERSION_CHARS - len(suffix)) + suffix,
                "license_note": cut(" ".join(note.split()), MAX_LICENSE_NOTE_CHARS),
            }
        )
    out.append({"ref": f"{V2_REL}/WEEKS/{week_name(day['week'])}/DAY_{day['n']:02d}.md", "kind": "INTERNAL"})
    return out


def sheet_resources(day: dict, registry_index: dict[str, dict]) -> str:
    """The resources of a day as a Markdown list: address as inline code, title, provider, access tier, verification label, then the use."""
    out = []
    for entry in day_resources(day, registry_index):
        uses = "; ".join(f"block {block}: {use}" for ident, use, block in day["resource_rows"] if ident == entry["id"])
        out.append(f"- `{entry['url']}`: {' '.join(entry['title'].split())}. Provider: {entry['provider'].strip()}. Access: {entry['access']}. Verification: {entry['verification']} ({entry['_checked_on']}). Use: {uses}")
    return "\n".join(out)


def build_draft(weeks: list[dict], registry_index: dict[str, dict]) -> dict:
    out = []
    for week in weeks:
        days = []
        for day in week["days"]:
            objectives = []
            for k, o in enumerate(day["objectives"], start=1):
                objective: dict = {"seq": k, "objective_id": f"{day_id(day['n'])}-O{k}", "text": o["text"]}
                if o["competency_ids"]:
                    objective["competency_ids"] = o["competency_ids"]
                objectives.append(objective)
            days.append(manifest_day(day["n"], objectives, day["hand_in"], manifest_resources(day, registry_index)))
        out.append({"week_number": week["number"], "title": week["title"], "days": days})
    return manifest(DRAFT_VERSION, DRAFT_NOTE, out)


def build_sheets(weeks: list[dict], registry_index: dict[str, dict], queues: dict[int, list[dict]]) -> dict:
    days: dict[str, dict] = {}
    for week in weeks:
        for day in week["days"]:
            codes = FRIDAY_CODES if is_friday(day["n"]) else WEEKDAY_CODES
            sheet: dict = {
                "week": week["number"],
                "title": day["topic"],
                "objectives": [o["text"] for o in day["objectives"]],
                "sections": [
                    {"key": codes[letter], "name": day["blocks"][letter]["title"], "minutes": BLOCK_MINUTES[letter], "markdown": day["blocks"][letter]["markdown"]}
                    for letter in BLOCK_LETTERS
                ],
                "watch_for": None,
                "extend": day["extend"],
                "resources": sheet_resources(day, registry_index),
                "header": day["header"],
                "hand_in": day["hand_in"],
            }
            if day["commands"]:
                sheet["commands"] = day["commands"]
            sheet.update(
                {
                    "mapping": day["mapping"],
                    "quiz": day["quiz"],
                    "recall_queue": queues[day["n"]],
                    "hint_ladder": day["hint_ladder"],
                    "troubleshooting": day["troubleshooting"],
                    "evidence": day["evidence"],
                    "reflection": day["reflection"],
                }
            )
            days[day_id(day["n"])] = sheet
    return {
        "sheets_schema_version": SHEETS_SCHEMA_VERSION,
        "curriculum_id": CURRICULUM_ID,
        "built_for_version": DRAFT_VERSION,
        "note": SHEETS_NOTE,
        "weeks": {str(week["number"]): dict({"title": week["title"]}, **week["paragraphs"]) for week in weeks},
        "days": days,
    }


def build_keys(weeks: list[dict]) -> dict:
    days: dict[str, dict] = {}
    exams: dict[str, dict] = {}
    for week in weeks:
        for day in week["days"]:
            key = week["keys"][day["n"]]
            days[day_id(day["n"])] = {"quiz": key["quiz"], "challenge": key["challenge"], "hint4": key["hint4"], "overclaims": key["overclaims"]}
        exams[week["exam"]["code"]] = {"questions": week["exam_key"]["questions"], "practical": week["exam_key"]["practical"], "gate": week["exam_key"]["gate"]}
    return {
        "keys_schema_version": KEYS_SCHEMA_VERSION,
        "curriculum_id": CURRICULUM_ID,
        "built_for_version": DRAFT_VERSION,
        "note": KEYS_NOTE,
        "days": days,
        "exams": exams,
    }


def assessment_payload(exam: dict) -> dict:
    return {
        "code": exam["code"],
        "title": exam["title"],
        "kind": exam["kind"],
        "competency_id": exam["competency_id"],
        "task_statement": exam["task_statement"],
        "difficulty": exam["difficulty"],
    }


def quiz_markdown(day: dict) -> str:
    n = day["n"]
    wname = week_name(day["week"])
    lines = [
        generated_line(f"`{V2_REL}/WEEKS/{wname}/DAY_{n:02d}.md`"),
        "",
        f"# G9 {wname} / DAY_{n:02d} — Quiz: {day['topic']}",
        "",
        f"Day {day_id(n)}, {day_date(n).isoformat()} ({WEEKDAYS[(n - 1) % 5]}). Five recall questions, closed book, and three applied questions. Write every answer before a key is opened; the keys are not part of this file.",
        "",
        f"## {QUIZ_RECALL}",
        "",
    ]
    lines += [f"{k}. [{q['id']}] {q['text']}" for k, q in enumerate(day["quiz"]["recall"], start=1)]
    lines += ["", f"## {QUIZ_APPLIED}", ""]
    lines += [f"{k}. [{q['id']}] {q['text']}" for k, q in enumerate(day["quiz"]["applied"], start=1)]
    return "\n".join(lines) + "\n"


def schedule_markdown(plan: list[dict], weeks: list[dict]) -> str:
    topics = {day["n"]: day["topic"] for week in weeks for day in week["days"]}
    lines = [
        generated_line(f"section 5 of `{PLAN_REL}` and the day guides under `{V2_REL}/WEEKS/`"),
        "",
        "# G9 Cybersecurity Mastery v2.0: schedule of the 60 days",
        "",
        "The dates are the plan's original planning dates (week 1 starts on Monday 2026-11-16, five days a week); they are not evidence of completion. "
        "Every day is 480 minutes: R active recall 30, A 120, B 120, C 150, D 60. Friday is the assessment day. "
        "Columns A to D are the seed texts of the plan (A, B and C are the three objectives, D is the evidence artifact).",
        "",
        "| Day | Date | Weekday | Week | Topic | A | B | C | D | Assessment |",
        "|---|---|---|---|---|---|---|---|---|---|",
    ]
    for w in plan:
        for d in w["days"]:
            n = d["n"]
            flag = f"`{exam_code(w['number'])}`" if is_friday(n) else "no"
            lines.append(f"| {day_id(n)} | {day_date(n).isoformat()} | {WEEKDAYS[(n - 1) % 5]} | {w['number']} | {cell(topics.get(n, ''))} | " + " | ".join(cell(t) for t in d["texts"]) + f" | {flag} |")
    return "\n".join(lines) + "\n"


def map_markdown(plan: list[dict], weeks: list[dict], info: dict[str, dict]) -> str:
    targets: dict[str, list[str]] = {c: [] for c in info}
    for week in weeks:
        for day in week["days"]:
            for k, o in enumerate(day["objectives"], start=1):
                for c in o["competency_ids"]:
                    targets[c].append(f"{day_id(day['n'])}-O{k}")
    assessed: dict[str, list[str]] = {c: [] for c in info}
    for week in weeks:
        assessed[week["exam"]["competency_id"]].append(week["exam"]["code"])
    lines = [
        generated_line(f"the day guides and assessments under `{V2_REL}/WEEKS/` and the competency packs under `data/competency-packs/`"),
        "",
        "# G9 Cybersecurity Mastery v2.0: curriculum map",
        "",
        f"Manifest `{CURRICULUM_ID}` version `{DRAFT_VERSION}`, provenance `PROPOSED_SEED`: nothing here is approved.",
        "",
        "## Competency to days",
        "",
        "Each objective names zero to three competencies of the eight packs. A competency with no objective is listed with a count of 0: the packs were written for another syllabus (ADR-0023).",
        "",
        "| Pack | Competency | Objectives | Count | Assessed by |",
        "|---|---|---|---|---|",
    ]
    for c in sorted(info, key=lambda k: (info[k]["pack"], k)):
        lines.append(f"| {info[c]['pack']} | `{c}` | {', '.join(targets[c])} | {len(targets[c])} | {', '.join(f'`{x}`' for x in assessed[c])} |")
    unlinked = sum(1 for week in weeks for day in week["days"] for o in day["objectives"] if not o["competency_ids"])
    lines += ["", f"Objectives without a competency: {unlinked} of {sum(len(day['objectives']) for week in weeks for day in week['days'])}.", ""]
    lines += ["## Day to resources, evidence and assessment", "", "| Day | Topic | Resources (registry ids) | Evidence | Assessment |", "|---|---|---|---|---|"]
    seeds = {d["n"]: d["texts"][3] for w in plan for d in w["days"]}
    for week in weeks:
        for day in week["days"]:
            ids = list(dict.fromkeys(ident for ident, _use, _block in day["resource_rows"]))
            flag = f"`{week['exam']['code']}`" if is_friday(day["n"]) else ""
            lines.append(f"| {day_id(day['n'])} | {cell(day['topic'])} | {', '.join(f'`{i}`' for i in ids)} | {cell(seeds[day['n']])} | {flag} |")
    lines += ["", "## Friday assessments", "", "| Week | Day | Code | Kind | Competency | Difficulty | Title |", "|---|---|---|---|---|---|---|"]
    for week in weeks:
        exam = week["exam"]
        lines.append(f"| {week['number']} | {day_id(5 * week['number'])} | `{exam['code']}` | {exam['kind']} | `{exam['competency_id']}` | {exam['difficulty']} | {cell(exam['title'])} |")
    return "\n".join(lines) + "\n"


def registry_markdown(weeks: list[dict]) -> str:
    cited: dict[str, list[str]] = {}
    for week in weeks:
        for day in week["days"]:
            for ident, _use, _block in day["resource_rows"]:
                if day_id(day["n"]) not in cited.setdefault(ident, []):
                    cited[ident].append(day_id(day["n"]))
    lines = [
        generated_line(f"the weekly registries `{V2_REL}/WEEKS/Wnn/RESOURCES.json`"),
        "",
        "# G9 Cybersecurity Mastery v2.0: resource registry",
        "",
        "Every lab and reference a day guide may cite. The verification label says how an entry was checked: SEARCH_CHECKED means a search result on the provider's own domain on the date shown, "
        "not a page that was opened and read; ASSUMED and UNVERIFIED say so. Access tiers and prices change: check the provider's page before relying on one.",
        "",
    ]
    for week in weeks:
        registry = week["registry"]
        lines += [f"## Week {week['number']:02d} — {week['title']}", "", f"Checked on {registry['checked_on']}. Method: {' '.join(registry['method'].split())}", ""]
        lines += ["| Id | Title | Address | Provider | Kind | Access | Verification | Checked | Fallback | Cited by |", "|---|---|---|---|---|---|---|---|---|---|"]
        for e in sorted(registry["entries"], key=lambda e: e["id"]):
            fallback = "" if e["fallback"] is None else (f"`{e['fallback']}`" if REGISTRY_ID_RE.match(e["fallback"]) else cell(e["fallback"]))
            lines.append(
                f"| `{e['id']}` | {cell(e['title'])} | `{e['url']}` | {cell(e['provider'])} | {e['kind']} | {e['access']} | {e['verification']} | {registry['checked_on']} | {fallback} | {', '.join(cited.get(e['id'], []))} |"
            )
        lines += ["", "### Plan fact checks", ""]
        if registry["plan_fact_checks"]:
            lines += ["| Claim | Result | Evidence |", "|---|---|---|"]
            lines += [f"| {cell(c['claim'])} | {c['result']} | {cell(c['evidence'])} |" for c in registry["plan_fact_checks"]]
        else:
            lines.append("None recorded.")
        lines += ["", "### Gaps", ""]
        lines += [f"- {' '.join(g.split())}" for g in registry["gaps"]] or ["None recorded."]
        lines.append("")
    return "\n".join(lines).rstrip("\n") + "\n"


def bank_markdown(weeks: list[dict]) -> str:
    counts = {"Daily recall": 0, "Daily applied": 0}
    counts.update({f"Exam {name}": 0 for name in QUESTION_MIX})
    multiple = 0
    for week in weeks:
        for day in week["days"]:
            counts["Daily recall"] += len(day["quiz"]["recall"])
            counts["Daily applied"] += len(day["quiz"]["applied"])
        for q in week["exam"]["questions"]:
            counts[f"Exam {q['kind']}"] += 1
            multiple += 1 if q["mcq"] else 0
    lines = [
        generated_line(f"the day guides and `ASSESSMENT.md` files under `{V2_REL}/WEEKS/`"),
        "",
        "# G9 Cybersecurity Mastery v2.0: question bank",
        "",
        "Every question of the course, by week and day, without keys. The keys are instructor material and live in a separate file.",
        "",
        "| Kind | Count |",
        "|---|---|",
    ]
    lines += [f"| {name} | {count} |" for name, count in counts.items()]
    lines += [f"| Total | {sum(counts.values())} |", f"| Of the exam questions, multiple choice | {multiple} |", ""]
    for week in weeks:
        lines += [f"## Week {week['number']:02d} — {week['title']}", ""]
        for day in week["days"]:
            lines += [f"### {day_id(day['n'])} — {day['topic']}", ""]
            lines += [f"- [{q['id']}] (recall) {q['text']}" for q in day["quiz"]["recall"]]
            lines += [f"- [{q['id']}] (applied) {q['text']}" for q in day["quiz"]["applied"]]
            lines.append("")
        lines += [f"### Friday exam {week['exam']['code']}", ""]
        for q in week["exam"]["questions"]:
            lines += [f"[{q['id']}] ({q['kind']}{', multiple choice' if q['mcq'] else ''}) {q['text']}", ""]
    return "\n".join(lines).rstrip("\n") + "\n"


def answers_markdown(weeks: list[dict]) -> str:
    lines = [
        generated_line(f"the key files `{V2_REL}/WEEKS/Wnn/ANSWER_KEYS/`"),
        "",
        "# G9 Cybersecurity Mastery v2.0: answer key (instructor)",
        "",
        KEY_NOTICE + " Every key of the daily quizzes and the Friday knowledge exams. The reference observations of the labs, the fourth hints, the practical fixture maps and the scoring are in the key files of each week.",
        "",
    ]
    for week in weeks:
        lines += [f"## Week {week['number']:02d} — {week['title']}", ""]
        for day in week["days"]:
            lines += [f"### {day_id(day['n'])} — {day['topic']}", ""]
            for ident, entry in week["keys"][day["n"]]["quiz"].items():
                lines += [f"**[{ident}]**", "", f"- Answer: {entry['answer']}", f"- Marking: {entry['marking']}", f"- Source: {entry['source']}", ""]
        lines += [f"### Friday exam {week['exam']['code']}", ""]
        for ident, entry in week["exam_key"]["questions"].items():
            lines += [f"**[{ident}]**", "", f"- Answer: {entry['answer']}", f"- Why: {entry['why']}"]
            if entry["distractors"] is not None:
                lines.append(f"- Distractors: {entry['distractors']}")
            lines.append("")
    return "\n".join(lines).rstrip("\n") + "\n"


def recall_markdown(weeks: list[dict], queues: dict[int, list[dict]], after: list[dict]) -> str:
    topics = {day["n"]: day["topic"] for week in weeks for day in week["days"]}
    lines = [
        generated_line(f"the '## Spaced recall' sections of the day guides under `{V2_REL}/WEEKS/`"),
        "",
        "# G9 Cybersecurity Mastery v2.0: spaced recall",
        "",
        "Each day tags its five recall questions with the offsets 1, 1, 3, 7 and 14 days. A question is placed in the recall block of the first training day on or after its day's date plus the offset "
        "(calendar days; the calendar is the plan's, week 1 starting on Monday 2026-11-16). A question that falls after day 60 is listed as post-course review. "
        "When a queue is short, block R of the day guide gives the warm-up tasks.",
        "",
    ]
    for n in range(1, 61):
        lines += [f"## {day_id(n)} — {day_date(n).isoformat()} ({WEEKDAYS[(n - 1) % 5]}) — {topics.get(n, '')}", ""]
        if queues[n]:
            lines += ["| Id | From | Offset (days) | Question |", "|---|---|---|---|"]
            lines += [f"| {q['id']} | {q['from_day']} | {q['offset_days']} | {cell(q['text'])} |" for q in queues[n]]
        else:
            lines.append("No question is due: use the warm-up tasks of block R.")
        lines.append("")
    lines += ["## Post-course review", "", "Questions whose date falls after day 60 (2027-02-05), in the order they fall due.", ""]
    if after:
        lines += ["| Due | Id | From | Offset (days) | Question |", "|---|---|---|---|---|"]
        lines += [f"| {q['due']} | {q['id']} | {q['from_day']} | {q['offset_days']} | {cell(q['text'])} |" for q in after]
    else:
        lines.append("None.")
    return "\n".join(lines) + "\n"


def build_outputs(plan: list[dict], weeks: list[dict], info: dict[str, dict], registry_index: dict[str, dict]) -> dict[Path, str]:
    queues, after = recall_queues(weeks)
    outputs: dict[Path, str] = {
        OUT_SEED: dump(build_seed(plan)),
        OUT_DRAFT: dump(build_draft(weeks, registry_index)),
        OUT_SHEETS: dump(build_sheets(weeks, registry_index, queues)),
        OUT_KEYS: dump(build_keys(weeks)),
    }
    for week in weeks:
        outputs[OUT_ASSESSMENTS / f"{week['exam']['code']}.json"] = dump(assessment_payload(week["exam"]))
        for day in week["days"]:
            outputs[WEEKS_DIR / week_name(week["number"]) / "QUIZZES" / f"DAY_{day['n']:02d}_QUIZ.md"] = quiz_markdown(day)
    outputs[OUT_SCHEDULE] = schedule_markdown(plan, weeks)
    outputs[OUT_MAP] = map_markdown(plan, weeks, info)
    outputs[OUT_REGISTRY] = registry_markdown(weeks)
    outputs[OUT_BANK] = bank_markdown(weeks)
    outputs[OUT_ANSWERS] = answers_markdown(weeks)
    outputs[OUT_RECALL] = recall_markdown(weeks, queues, after)
    return outputs


GENERATED_MARK = f"Generated by `{SELF}`".encode()


def generated_orphans(outputs: dict[Path, str]) -> list[Path]:
    """Generated files that no source produces any more. The payload folder holds generated files only, so everything in it that is not
    an output is one; elsewhere (docs/curriculum/v2/ and its ASSESSMENTS/ also hold the owner's hand-written documents, plan section 0.1)
    only a Markdown file that says it was generated by this script is."""
    found: list[Path] = []
    if OUT_ASSESSMENTS.is_dir():
        found += [p for p in sorted(OUT_ASSESSMENTS.iterdir()) if p not in outputs]
    for folder in (V2_DIR, OUT_BANK.parent):
        if not folder.is_dir():
            continue
        for p in sorted(folder.glob("*.md")):
            if p not in outputs and p.is_file() and not p.is_symlink() and p.read_bytes()[: len(GENERATED_MARK)] == GENERATED_MARK:
                found.append(p)
    return found


def unsafe_outputs(paths: list[Path]) -> list[str]:
    """Why a generated path must not be written or removed: a symbolic link at the path or in a folder above it (below the repository),
    a path that resolves outside the repository, a folder or other non-file where a file is written, or a folder that cannot be removed
    by this script. Nothing is written while one such path stands."""
    out: dict[str, None] = {}
    root = REPO.resolve()
    for path in paths:
        links = []
        here = path
        while here != REPO and here != here.parent:
            if here.is_symlink():
                links.append(f"{rel(here)}: is a symbolic link: a generated file is written in place, never through a link")
            here = here.parent
        if links:
            out.update(dict.fromkeys(links))
            continue
        try:
            path.resolve().relative_to(root)
        except ValueError:
            out[f"{rel(path)}: resolves outside the repository"] = None
            continue
        if path.is_dir():
            out[f"{rel(path)}: is a folder: remove it by hand" if path.parent == OUT_ASSESSMENTS else f"{rel(path)}: is not a regular file"] = None
        elif path.exists() and not path.is_file():
            out[f"{rel(path)}: is not a regular file"] = None
    return list(out)


# ---------------------------------------------------------------------------------------------
# main
# ---------------------------------------------------------------------------------------------


def corpus_checks(weeks: list[dict], registries: dict[int, dict], registry_index: dict[str, dict], selected: list[int], problems: Problems) -> None:
    """Rules over all the weeks that were checked: unique identifiers, no question twice, resolvable fallbacks, sheet sizes."""
    seen: dict[str, str] = {}
    texts: dict[str, str] = {}
    for week in weeks:
        questions = [(q, day["where"]) for day in week["days"] for q in day["quiz"]["recall"] + day["quiz"]["applied"]]
        if week["exam"]:
            questions += [(q, week["exam"]["where"]) for q in week["exam"]["questions"]]
        for q, where in questions:
            if q["id"] in seen:
                problems.add("identifiers", f"'{q['id']}' is used twice ({seen[q['id']]} and {where}): an identifier is unique across the corpus")
            else:
                seen[q["id"]] = where
            text = normalize(q["text"])
            if text and text in texts and texts[text] != q["id"]:
                problems.add("questions", f"{texts[text]} and {q['id']} have the same text: no two questions of the corpus are the same")
            elif text:
                texts[text] = q["id"]
        for day in week["days"]:
            limit(problems, day["where"], "the resources of the day, as the lesson sheet lists them,", sheet_resources(day, registry_index), MAX_SHEET_RESOURCES_CHARS)
    for w in selected:
        registry = registries.get(w)
        for entry in (registry or {}).get("entries", []):
            fallback = entry.get("fallback")
            if isinstance(fallback, str) and REGISTRY_ID_RE.match(fallback) and fallback not in registry_index:
                problems.add(f"{week_name(w)}/RESOURCES.json {entry['id']}", f"'fallback' names '{fallback}', which is not an entry of a RESOURCES.json")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--write", action="store_true", help="write the generated files (all 12 weeks are required)")
    ap.add_argument("--week", type=int, action="append", metavar="N", help="check only week N (repeatable); the generated files are not compared")
    ap.add_argument("--available", action="store_true", help="check every week whose folder holds all five day guides (a week with fewer is reported as in progress, not judged); write nothing")
    ap.add_argument("--seed-text", metavar="DNN", help="print the four seed texts of a day")
    ap.add_argument("--print-seed", action="store_true", help="print the seed manifest (generated from the plan alone) on standard output")
    args = ap.parse_args()
    if args.week and (args.available or args.write):
        ap.error("--week cannot be combined with --available or --write (--write needs all 12 weeks)")
    if (args.seed_text or args.print_seed) and (args.week or args.available or args.write or (args.seed_text and args.print_seed)):
        ap.error("--seed-text and --print-seed stand alone")
    if args.week and any(not 1 <= w <= 12 for w in args.week):
        ap.error("--week takes a week number from 1 to 12")
    if args.seed_text and not DAY_ID_RE.match(args.seed_text):
        ap.error("--seed-text takes a day id from D01 to D60")
    if not PLAN.is_file() or not PACKS.is_dir():
        sys.stderr.write(f"error: master plan ({PLAN_REL}) or competency packs (data/competency-packs) not found\n")
        return 2

    problems = Problems()
    plan = parse_plan(PLAN.read_text(encoding="utf-8"), problems)
    if problems.items:
        for p in problems.items:
            print(f"FAIL  {p}")
        print(f"\n{len(problems.items)} problem(s) in the day tables of the plan; nothing else was checked.")
        return 1
    if args.seed_text:
        print("\n".join(seed_text_lines(plan, int(args.seed_text[1:]))))
        return 0
    if args.print_seed:
        sys.stdout.write(dump(build_seed(plan)))
        return 0

    try:
        info = load_competencies()
    except PackError as exc:
        sys.stderr.write(f"error: {exc}\n")
        return 2
    in_progress: list[int] = []
    if args.week:
        selected = sorted(set(args.week))
    elif args.available:
        selected = [w for w in range(1, 13) if week_has_every_day_file(w)]
        in_progress = [w for w in range(1, 13) if week_has_day_file(w) and not week_has_every_day_file(w)]
    else:
        selected = list(range(1, 13))
    full = selected == list(range(1, 13)) and not args.week

    check_weeks_folder(problems)

    # every registry that exists is read (a day may cite another week's entry); only the selected weeks' are judged
    registries: dict[int, dict] = {}
    registry_index: dict[str, dict] = {}
    for w in range(1, 13):
        registry = load_registry(w, problems if w in selected else Problems())
        if registry is None:
            continue
        registries[w] = registry
        for entry in registry["entries"]:
            registry_index.setdefault(entry["id"], dict(entry, _checked_on=registry["checked_on"], _week=w))

    weeks = [week for w in selected if (week := check_week(w, plan[w - 1], info, registries, registry_index, problems)) is not None]
    corpus_checks(weeks, registries, registry_index, selected, problems)
    leak_checks(weeks, info, registry_index, problems)

    days = sum(len(week["days"]) for week in weeks)
    questions = sum(len(day["quiz"]["recall"]) + len(day["quiz"]["applied"]) for week in weeks for day in week["days"])
    exam_questions = sum(len(week["exam"]["questions"]) for week in weeks if week["exam"])
    entries = sum(len(registries[w]["entries"]) for w in selected if w in registries)
    cited = len({ident for week in weeks for day in week["days"] for ident, _u, _b in day["resource_rows"]})
    words = sum(day["words"] for week in weeks for day in week["days"])
    counts = f"{len(weeks)} week(s) {[week['number'] for week in weeks]}; {days} days; {questions} questions; {exam_questions} exam questions; {entries} resources ({cited} cited); {words} words in the day guides"
    if problems.items:
        for p in problems.items:
            print(f"FAIL  {p}")
        print(f"\n{len(problems.items)} problem(s); checked: {counts}.")
        return 1
    print(f"ok    v2 courseware valid: {counts}")
    if not full:
        print(f"      partial run (weeks {selected}): the generated files were not compared and nothing was written")
        if in_progress:
            print(f"      weeks in progress (some day guides, not all five), not judged by --available: {in_progress}; check one with --week N")
        return 0

    outputs = build_outputs(plan, weeks, info, registry_index)
    write = args.write and not args.available
    orphans = generated_orphans(outputs)
    refused = unsafe_outputs([*outputs, *orphans])
    if refused:
        for message in refused:
            print(f"FAIL  {message}")
        print(f"\n{len(refused)} generated path(s) refused; nothing was written.")
        return 1
    stale: list[Path] = []
    for path, content in outputs.items():
        data = content.encode("utf-8")
        if not path.is_file() or path.read_bytes() != data:
            stale.append(path)
            if write:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
    # a generated file that no longer has a source is stale too (a payload for a code that no longer exists, an index of an older script)
    for path in orphans:
        stale.append(path)
        if write:
            path.unlink()
    if write:
        print(f"wrote {len(stale)} file(s)" if stale else "nothing to write: generated files are current")
        return 0
    if args.write:
        print(f"--available: nothing was written; {len(stale)} generated file(s) would be written or removed")
        for p in stale:
            print(f"      {rel(p)}")
        return 0
    if stale:
        for p in stale:
            print(f"FAIL  {rel(p)} is out of date; run: {WRITE_HINT}")
        return 1
    print(f"ok    generated files are current ({len(outputs)} files)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
