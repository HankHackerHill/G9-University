#!/usr/bin/env python3
"""check_skill_packages.py — static validator for G9 skill packages.

Validates every package under a skills root against
docs/skills/skill-package-contract.md (layout, frontmatter, the 12 H2 sections,
schemas, evaluation fixtures) plus the registry/content invariants listed in the
contract's "Validator" section.

Usage:
    python3 -I scripts/check_skill_packages.py .agents/skills
    python3 -I scripts/check_skill_packages.py .agents/skills --expect-count 1
    python3 -I scripts/check_skill_packages.py .agents/skills --json

Exit codes: 0 all packages valid and count matches; 1 any failure; 2 usage or
environment error (missing root, missing jsonschema package).

The script is deliberately dependency-light: it needs only the standard library
and the `jsonschema` package (Draft 2020-12 metaschema validation and eval-input
validation). PyYAML is used only as a cross-check of the frontmatter when it is
importable; the contract's strict frontmatter subset is parsed by this file.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import stat
import sys
from pathlib import Path
from typing import Any

try:
    import jsonschema
    from jsonschema import Draft202012Validator
except ImportError:  # pragma: no cover - environment error path
    sys.stderr.write("error: the `jsonschema` package (>= 4.18) is required\n")
    sys.exit(2)

try:  # optional cross-check only
    import yaml  # type: ignore
except ImportError:  # pragma: no cover
    yaml = None

# --------------------------------------------------------------------------
# Contract constants (keep in sync with crates/g9-domain and the envelopes)
# --------------------------------------------------------------------------

DRAFT_2020_12 = "https://json-schema.org/draft/2020-12/schema"

MODES = ["STUDY", "LAB", "EXAM", "REVIEW", "EXECUTIVE", "RESEARCH", "MAINTENANCE"]
DATA_DOMAINS = ["LEARNING", "BUSINESS", "PERSONAL", "SECURITY_SENSITIVE", "SYSTEM"]
CAPABILITIES = [
    "PROTECTED_RECORD_WRITE", "GRADE_FINALIZE", "SCHEDULE_APPLY", "CURRICULUM_IMPORT",
    "CURRICULUM_PROMOTE", "SKILL_INVOKE", "MODEL_INFERENCE", "FILESYSTEM_READ",
    "FILESYSTEM_WRITE", "NETWORK_EGRESS", "SHELL_EXEC", "DATA_EXPORT", "DATA_DELETE",
    "POLICY_MODIFY", "MODEL_LOAD", "LAB_ORCHESTRATE", "CROSS_DOMAIN_READ",
]
OPERATOR_ONLY = {
    "SHELL_EXEC", "NETWORK_EGRESS", "FILESYSTEM_WRITE", "POLICY_MODIFY", "GRADE_FINALIZE",
    "CURRICULUM_PROMOTE", "DATA_DELETE", "MODEL_LOAD", "LAB_ORCHESTRATE",
}
ACTIONS = [
    "READ", "CREATE", "UPDATE", "DELETE", "EXPORT", "IMPORT", "PROMOTE_CANONICAL", "PROPOSE",
    "APPROVE", "APPLY", "FINALIZE", "INVOKE", "EXECUTE", "CONNECT", "LOAD", "CORRECT",
]
# Actions a skill may put in recommended_actions. The rest are operator decisions
# or operator-only operations (handoff §11, ARCHITECTURE §11).
SKILL_ALLOWED_ACTIONS = {"READ", "CREATE", "UPDATE", "EXPORT", "IMPORT", "PROPOSE", "INVOKE", "CORRECT"}
ACTOR_KINDS = ["OPERATOR", "SYSTEM", "SKILL", "MODEL"]
STATUSES = ["COMPLETED", "ABSTAINED", "ERROR"]
TRUST_CLASSES = [
    "Operator", "SystemDeterministic", "ApprovedReference", "ImportedDocument",
    "SkillPackage", "ModelOutput", "Unknown",
]
CLASSIFICATIONS = ["PUBLIC", "INTERNAL", "PRIVATE", "SECRET"]
FAMILIES = ["engineering", "teaching", "learning", "performance", "knowledge"]
FAMILY_BY_PREFIX = {"E": "engineering", "T": "teaching", "L": "learning", "P": "performance", "K": "knowledge"}
G9_STATUS = ["PROPOSED", "IMPLEMENTED", "EVALUATED"]

# Handoff §11 — the 37-skill registry. Code and name are bound.
REGISTRY: dict[str, str] = {
    "E01": "g9-spec-assurance", "E02": "g9-skill-engineer", "E03": "g9-curriculum-compiler",
    "E04": "g9-domain-architect", "E05": "g9-ui-accessibility", "E06": "g9-security-boundaries",
    "E07": "g9-data-integrity", "E08": "g9-scheduling-engine", "E09": "g9-learning-data-model",
    "E10": "g9-model-runtime-engineer", "E11": "g9-knowledge-engineer", "E12": "g9-mentor-evaluation",
    "E13": "g9-adversarial-assurance", "E14": "g9-release-recovery",
    "T01": "g9-session-director", "T02": "g9-prerequisite-diagnosis", "T03": "g9-adaptive-instruction",
    "T04": "g9-socratic-coaching", "T05": "g9-laboratory-coach", "T06": "g9-progressive-hints",
    "T07": "g9-teachback-review", "T08": "g9-independent-examiner",
    "L01": "g9-misconception-analysis", "L02": "g9-competency-evidence", "L03": "g9-retrieval-and-transfer",
    "L04": "g9-intervention-designer", "L05": "g9-learning-experiment", "L06": "g9-confidence-calibration",
    "P01": "g9-adaptive-agenda", "P02": "g9-performance-review", "P03": "g9-goal-decomposition",
    "P04": "g9-execution-diagnosis", "P05": "g9-decision-review", "P06": "g9-reflection-coaching",
    "K01": "g9-source-curation", "K02": "g9-memory-reconciliation", "K03": "g9-ct9-knowledge-export",
}
NAME_TO_CODE = {v: k for k, v in REGISTRY.items()}

CODE_RE = re.compile(r"^(E(0[1-9]|1[0-4])|T0[1-8]|L0[1-6]|P0[1-6]|K0[1-3])$")
NAME_RE = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")
SEMVER_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+$")
MODULE_RE = re.compile(r"^M(0[1-9]|1[0-7])$")
UINT_RE = re.compile(r"^(0|[1-9][0-9]*)$")
TAG_RE = re.compile(r"^[a-z][a-z0-9_]*$")
FRONTMATTER_KEY_RE = re.compile(r"^[a-z][a-z0-9-]*$")

TOP_LEVEL_REQUIRED = ["name", "description", "license", "metadata"]
TOP_LEVEL_OPTIONAL = ["compatibility", "allowed-tools"]
METADATA_KEYS = [
    "g9-code", "g9-family", "g9-version", "g9-owner-module", "g9-modes", "g9-data-domains",
    "g9-capabilities", "g9-budget-tokens", "g9-budget-seconds", "g9-budget-tool-calls",
    "g9-recursion-depth", "g9-depends-on", "g9-status",
]
BUDGET_BOUNDS = {  # mirrors resource_budget in the invocation envelope
    "g9-budget-tokens": (1, 32768),
    "g9-budget-seconds": (1, 600),
    "g9-budget-tool-calls": (0, 16),
}

SECTIONS = [
    "Mission",
    "Activation",
    "Inputs and provenance requirements",
    "Procedure",
    "Non-goals",
    "Tool and data scopes",
    "Output contract",
    "Confidence and abstention",
    "Escalation and stop conditions",
    "Failure recovery",
    "Acceptance tests",
    "References",
]

PACKAGE_ENTRIES = {"SKILL.md", "references", "schemas", "evals", "assets"}
EVAL_FILES = [
    ("positive.json", "positive", "POS", 3),
    ("negative.json", "negative", "NEG", 3),
    ("adversarial.json", "adversarial", "ADV", 3),
    ("acceptance.json", "acceptance", "ACC", 2),
]
REQUIRED_ADVERSARIAL_TAGS = ["embedded_instruction", "forged_reference", "scope_escalation"]
CASE_REQUIRED = ["id", "kind", "title", "mode", "actor_kind", "intent", "input", "expect", "notes"]
CASE_OPTIONAL = ["context_fixture", "context_segments", "tags"]
EXPECT_KEYS = {
    "activate", "status", "must_include", "must_not", "approval_requirements", "recommended_action_kinds",
}

FORBIDDEN_FRAMING_RE = re.compile(
    r"act\s+as\s+(the\s+|a\s+|an\s+)?(world'?s\s+|worlds\s+)?(best|leading|top|greatest|foremost|ultimate)"
    r"|world[-'’ ]?s?\s*(best|class)\s+expert",
    re.IGNORECASE,
)
USE_CUE_RE = re.compile(r"\buse\b", re.IGNORECASE)
# A `use` that belongs to a negative cue ("do not use", "never use", "when not to use", ...). These
# phrases are removed before USE_CUE_RE looks for the positive cue, so "Do not use for X" alone does
# not count as stating when to use the skill.
NEGATED_USE_RE = re.compile(
    r"\b(?:not|never|don['\u2019]?t|doesn['\u2019]?t|can['\u2019]?t|cannot)(?:\s+to)?\s+use\b"
    # "Not for use in X", "never for the use of X": `for` (and `the`) between the negation and `use`.
    r"|\b(?:not|never)\s+for\s+(?:the\s+)?use\b",
    re.IGNORECASE,
)
NEG_CUE_RE = re.compile(
    r"\b(do not use|don'?t use|never use|not for|not when|not during|not in|must not|except)\b",
    re.IGNORECASE,
)
BULLET_RE = re.compile(r"^\s*[-*+]\s+\S")
NUMBERED_RE = re.compile(r"^\s*\d+\.\s+\S")
H2_RE = re.compile(r"^##\s+(.*?)\s*$")
H3_RE = re.compile(r"^###\s+(.*?)\s*$")
H1_RE = re.compile(r"^#\s+")
FENCE_RE = re.compile(r"^\s*(```|~~~)")
CASE_ID_RE = re.compile(r"\b([ETLPK][0-9]{2})-(POS|NEG|ADV|ACC)-([0-9]{3})\b")

# Content invariants for specific registry entries (task content standards).
# body_must: regexes that must match the SKILL.md body; body_must_not: must not.
CONTENT_RULES: dict[str, dict[str, list[str]]] = {
    "g9-independent-examiner": {
        "body_must": [r"MODEL_ADVISORY"],
        "body_must_not": [r"\bfinali[sz]es?\s+(the\s+)?grade"],
        "forbid_caps": ["GRADE_FINALIZE", "PROTECTED_RECORD_WRITE"],
    },
    "g9-retrieval-and-transfer": {"body_must": [r"\bdeclarative\b"]},
    "g9-learning-experiment": {
        "body_must": [r"\binconclusive\b", r"\bpredeclared\b|\bpre-declared\b"],
        "body_must_not": [r"p\s*<\s*0?\.05", r"\bn\s*=\s*20\b"],
    },
    "g9-confidence-calibration": {"body_must": [r"\bprobabilistic\b", r"\bmapping\b"]},
    "g9-adaptive-agenda": {"forbid_caps": ["SCHEDULE_APPLY"], "actions_only": ["PROPOSE", "READ"]},
    "g9-scheduling-engine": {"forbid_caps": ["SCHEDULE_APPLY"], "actions_only": ["PROPOSE", "READ"]},
    "g9-ct9-knowledge-export": {"body_must": [r"\bapproved\b", r"\blicen[cs]e", r"\bclassif"]},
}


# --------------------------------------------------------------------------
# Result container
# --------------------------------------------------------------------------

class PackageReport:
    def __init__(self, name: str, path: Path) -> None:
        self.name = name
        self.path = path
        self.code: str = "-"
        self.family: str = "-"
        self.g9_status: str = "-"
        self.failures: list[str] = []
        self.warnings: list[str] = []
        self.checks = 0

    def fail(self, msg: str) -> None:
        self.checks += 1
        self.failures.append(msg)

    def ok(self) -> None:
        self.checks += 1

    def warn(self, msg: str) -> None:
        self.warnings.append(msg)

    def check(self, cond: bool, msg: str) -> bool:
        if cond:
            self.ok()
        else:
            self.fail(msg)
        return cond

    @property
    def passed(self) -> bool:
        return not self.failures

    def to_json(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "path": str(self.path),
            "code": self.code,
            "family": self.family,
            "g9_status": self.g9_status,
            "checks": self.checks,
            "pass": self.passed,
            "failures": list(self.failures),
            "warnings": list(self.warnings),
        }


# --------------------------------------------------------------------------
# Frontmatter: strict subset parser (contract: exact keys, string values)
# --------------------------------------------------------------------------

class FrontmatterError(ValueError):
    pass


def _strip_comment_plain(raw: str) -> str:
    """Strip a YAML trailing comment (` #...`) from a plain scalar."""
    idx = raw.find(" #")
    if raw.startswith("#"):
        return ""
    if idx >= 0:
        raw = raw[:idx]
    return raw.strip()


def _parse_scalar(raw: str, where: str) -> tuple[str, bool]:
    """Return (value, quoted). Supports double-quoted, single-quoted and plain scalars."""
    raw = raw.strip()
    if raw.startswith('"'):
        out = []
        i = 1
        closed = False
        while i < len(raw):
            c = raw[i]
            if c == "\\":
                if i + 1 >= len(raw):
                    raise FrontmatterError(f"{where}: dangling escape in double-quoted scalar")
                nxt = raw[i + 1]
                if nxt in ('"', "\\", "/"):
                    out.append(nxt)
                elif nxt == "n":
                    out.append("\n")
                elif nxt == "t":
                    out.append("\t")
                else:
                    raise FrontmatterError(f"{where}: unsupported escape \\{nxt}")
                i += 2
                continue
            if c == '"':
                closed = True
                rest = raw[i + 1:].strip()
                if rest and not rest.startswith("#"):
                    raise FrontmatterError(f"{where}: trailing content after closing quote: {rest!r}")
                break
            out.append(c)
            i += 1
        if not closed:
            raise FrontmatterError(f"{where}: unterminated double-quoted scalar")
        return "".join(out), True
    if raw.startswith("'"):
        i = 1
        out = []
        closed = False
        while i < len(raw):
            c = raw[i]
            if c == "'":
                if i + 1 < len(raw) and raw[i + 1] == "'":
                    out.append("'")
                    i += 2
                    continue
                closed = True
                rest = raw[i + 1:].strip()
                if rest and not rest.startswith("#"):
                    raise FrontmatterError(f"{where}: trailing content after closing quote: {rest!r}")
                break
            out.append(c)
            i += 1
        if not closed:
            raise FrontmatterError(f"{where}: unterminated single-quoted scalar")
        return "".join(out), True
    value = _strip_comment_plain(raw)
    if value and value[0] in "[{&*!|>%@`":
        raise FrontmatterError(f"{where}: unsupported YAML construct starting with {value[0]!r}; use a plain or quoted string")
    return value, False


def parse_frontmatter(text: str) -> tuple[dict[str, Any], dict[str, bool], str, str]:
    """Parse the strict frontmatter subset.

    Returns (data, quoted_flags, raw_frontmatter, body). `data["metadata"]` is a dict of
    strings; `quoted_flags` records whether each metadata value was quoted.
    """
    lines = text.split("\n")
    if not lines or lines[0].rstrip("\r") != "---":
        raise FrontmatterError("SKILL.md must start with a `---` line")
    end = None
    for i in range(1, len(lines)):
        if lines[i].rstrip("\r") == "---":
            end = i
            break
    if end is None:
        raise FrontmatterError("frontmatter closing `---` not found")
    fm_lines = lines[1:end]
    body = "\n".join(lines[end + 1:])
    data: dict[str, Any] = {}
    quoted: dict[str, bool] = {}
    in_metadata = False
    for n, line in enumerate(fm_lines, start=2):
        where = f"frontmatter line {n}"
        if not line.strip() or line.strip().startswith("#"):
            continue
        if line.startswith("\t"):
            raise FrontmatterError(f"{where}: tabs are not allowed for indentation")
        if line.startswith(" "):
            if not in_metadata:
                raise FrontmatterError(f"{where}: unexpected indented line outside `metadata:`")
            if not line.startswith("  ") or line.startswith("   "):
                raise FrontmatterError(f"{where}: metadata entries must be indented by exactly two spaces")
            stripped = line[2:]
            if ":" not in stripped:
                raise FrontmatterError(f"{where}: expected `key: value`")
            key, _, rawval = stripped.partition(":")
            key = key.strip()
            if not FRONTMATTER_KEY_RE.match(key):
                raise FrontmatterError(f"{where}: invalid metadata key {key!r}")
            if key in data["metadata"]:
                raise FrontmatterError(f"{where}: duplicate metadata key {key!r}")
            if rawval and not rawval.startswith(" "):
                raise FrontmatterError(f"{where}: expected a space after `:`")
            value, was_quoted = _parse_scalar(rawval, where)
            data["metadata"][key] = value
            quoted[key] = was_quoted
            continue
        in_metadata = False
        if ":" not in line:
            raise FrontmatterError(f"{where}: expected `key: value`")
        key, _, rawval = line.partition(":")
        key = key.strip()
        if not FRONTMATTER_KEY_RE.match(key):
            raise FrontmatterError(f"{where}: invalid key {key!r}")
        if key in data:
            raise FrontmatterError(f"{where}: duplicate key {key!r}")
        if key == "metadata":
            if _strip_comment_plain(rawval.strip()) != "":
                raise FrontmatterError(f"{where}: `metadata:` must introduce an indented block")
            data["metadata"] = {}
            in_metadata = True
            continue
        if rawval and not rawval.startswith(" "):
            raise FrontmatterError(f"{where}: expected a space after `:`")
        value, _ = _parse_scalar(rawval, where)
        data[key] = value
    return data, quoted, "\n".join(fm_lines), body


# --------------------------------------------------------------------------
# Body: section splitter that ignores fenced code blocks
# --------------------------------------------------------------------------

def split_sections(body: str) -> tuple[list[tuple[str, list[str]]], list[str], list[str]]:
    """Return ([(h2_title, lines)], preamble_lines, structural_errors)."""
    sections: list[tuple[str, list[str]]] = []
    preamble: list[str] = []
    errors: list[str] = []
    fence: str | None = None
    current: list[str] | None = None
    for line in body.split("\n"):
        m = FENCE_RE.match(line)
        if m:
            marker = m.group(1)
            if fence is None:
                fence = marker
            elif fence == marker:
                fence = None
            (current if current is not None else preamble).append(line)
            continue
        if fence is not None:
            (current if current is not None else preamble).append(line)
            continue
        h2 = H2_RE.match(line)
        if h2:
            current = []
            sections.append((h2.group(1), current))
            continue
        if H1_RE.match(line) and current is not None:
            errors.append(f"H1 heading inside section is not allowed: {line.strip()!r}")
        (current if current is not None else preamble).append(line)
    if fence is not None:
        errors.append("unterminated fenced code block in SKILL.md body")
    return sections, preamble, errors


def paragraphs(lines: list[str]) -> list[str]:
    paras: list[str] = []
    buf: list[str] = []
    for line in lines:
        if line.strip():
            buf.append(line.strip())
        elif buf:
            paras.append(" ".join(buf))
            buf = []
    if buf:
        paras.append(" ".join(buf))
    return paras


def bullets(lines: list[str]) -> list[str]:
    return [ln for ln in lines if BULLET_RE.match(ln)]


def h3_blocks(lines: list[str]) -> list[tuple[str, list[str]]]:
    blocks: list[tuple[str, list[str]]] = []
    cur: list[str] | None = None
    for ln in lines:
        m = H3_RE.match(ln)
        if m:
            cur = []
            blocks.append((m.group(1), cur))
        elif cur is not None:
            cur.append(ln)
    return blocks


def table_rows(lines: list[str]) -> tuple[list[str] | None, list[list[str]]]:
    """Return (header_cells, data_rows) of the first markdown table in lines."""
    header: list[str] | None = None
    rows: list[list[str]] = []
    seen_sep = False
    for ln in lines:
        s = ln.strip()
        if not s.startswith("|"):
            if header is not None and seen_sep:
                break
            continue
        cells = [c.strip() for c in s.strip("|").split("|")]
        if header is None:
            header = cells
            continue
        if not seen_sep:
            if all(set(c) <= set(":- ") and c for c in cells):
                seen_sep = True
                continue
            header = None
            continue
        rows.append(cells)
    return (header if seen_sep else None), rows


# --------------------------------------------------------------------------
# JSON helpers
# --------------------------------------------------------------------------

def load_json(path: Path) -> tuple[Any, str | None]:
    try:
        with path.open("r", encoding="utf-8") as fh:
            return json.load(fh), None
    except FileNotFoundError:
        return None, "missing"
    except json.JSONDecodeError as exc:
        return None, f"invalid JSON ({exc.msg} at line {exc.lineno})"
    except UnicodeDecodeError:
        return None, "not UTF-8"


def find_key(node: Any, key: str, path: str = "$") -> list[str]:
    hits: list[str] = []
    if isinstance(node, dict):
        for k, v in node.items():
            if k == key:
                hits.append(f"{path}.{k}")
            hits.extend(find_key(v, key, f"{path}.{k}"))
    elif isinstance(node, list):
        for i, v in enumerate(node):
            hits.extend(find_key(v, key, f"{path}[{i}]"))
    return hits


def nested_objects_without_closed_props(node: Any, path: str = "$") -> list[str]:
    hits: list[str] = []
    if isinstance(node, dict):
        if path != "$" and node.get("type") == "object" and "properties" in node:
            if node.get("additionalProperties") is not False:
                hits.append(path)
        for k, v in node.items():
            hits.extend(nested_objects_without_closed_props(v, f"{path}.{k}"))
    elif isinstance(node, list):
        for i, v in enumerate(node):
            hits.extend(nested_objects_without_closed_props(v, f"{path}[{i}]"))
    return hits


def pascal_to_screaming(s: str) -> str:
    if s.isupper():
        return s
    return re.sub(r"(?<!^)(?=[A-Z])", "_", s).upper()


# --------------------------------------------------------------------------
# Package validation
# --------------------------------------------------------------------------

def check_layout(rep: PackageReport, pkg: Path) -> None:
    entries = sorted(p.name for p in pkg.iterdir())
    extra = [e for e in entries if e not in PACKAGE_ENTRIES]
    rep.check(not extra, f"layout: unexpected entries in package root: {extra}")
    rep.check((pkg / "SKILL.md").is_file(), "layout: SKILL.md missing")
    refs = pkg / "references"
    if rep.check(refs.is_dir(), "layout: references/ directory missing"):
        md = sorted(p.name for p in refs.iterdir() if p.is_file() and p.suffix == ".md")
        rep.check(len(md) >= 1, "layout: references/ must contain at least one .md file")
        others = sorted(p.name for p in refs.iterdir() if not (p.is_file() and p.suffix == ".md"))
        rep.check(not others, f"layout: references/ may contain only .md files, found {others}")
    schemas = pkg / "schemas"
    if rep.check(schemas.is_dir(), "layout: schemas/ directory missing"):
        names = sorted(p.name for p in schemas.iterdir())
        rep.check(names == ["input.schema.json", "output.schema.json"],
                  f"layout: schemas/ must contain exactly input.schema.json and output.schema.json, found {names}")
    evals = pkg / "evals"
    if rep.check(evals.is_dir(), "layout: evals/ directory missing"):
        names = sorted(p.name for p in evals.iterdir())
        rep.check(names == sorted(f for f, _, _, _ in EVAL_FILES),
                  f"layout: evals/ must contain exactly the four eval files, found {names}")
    assets = pkg / "assets"
    if assets.exists():
        if rep.check(assets.is_dir(), "layout: assets must be a directory"):
            for p in assets.rglob("*"):
                if p.is_file():
                    mode = p.stat().st_mode
                    rep.check(not (mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)),
                              f"layout: assets/{p.relative_to(assets)} has an executable bit set; assets are never executed")
                    rep.check(p.suffix.lower() not in {".sh", ".py", ".exe", ".bat", ".ps1", ".js", ".rb", ".pl"},
                              f"layout: assets/{p.relative_to(assets)} looks like a script; assets are templates only")


def check_frontmatter(rep: PackageReport, pkg: Path, text: str) -> tuple[dict[str, Any] | None, str]:
    try:
        data, quoted, raw_fm, body = parse_frontmatter(text)
    except FrontmatterError as exc:
        rep.fail(f"frontmatter: {exc}")
        return None, ""
    rep.ok()

    # Optional cross-check: the strict subset must agree with a real YAML parser.
    if yaml is not None:
        try:
            loaded = yaml.safe_load(raw_fm)
            if loaded != data:
                rep.fail("frontmatter: strict parse differs from PyYAML parse (check quoting/escapes)")
            else:
                rep.ok()
        except Exception as exc:  # noqa: BLE001 - report any YAML error
            rep.fail(f"frontmatter: PyYAML could not parse the frontmatter: {exc}")

    keys = list(data.keys())
    for k in TOP_LEVEL_REQUIRED:
        rep.check(k in data, f"frontmatter: required key `{k}` missing")
    unknown = [k for k in keys if k not in TOP_LEVEL_REQUIRED + TOP_LEVEL_OPTIONAL]
    rep.check(not unknown, f"frontmatter: unknown top-level keys {unknown}")
    for k in ("name", "description", "license", "compatibility"):
        if k in data:
            rep.check(isinstance(data[k], str) and data[k].strip() != "", f"frontmatter: `{k}` must be a non-empty string")

    name = data.get("name", "")
    if isinstance(name, str):
        rep.check(1 <= len(name) <= 64, "frontmatter: name must be 1-64 characters")
        rep.check(bool(NAME_RE.match(name)), "frontmatter: name must match ^[a-z0-9]+(-[a-z0-9]+)*$ (no leading/trailing/double hyphen)")
        rep.check(name.startswith("g9-"), "frontmatter: name must start with `g9-`")
        rep.check(name == pkg.name, f"frontmatter: name {name!r} does not match directory {pkg.name!r}")

    desc = data.get("description", "")
    if isinstance(desc, str):
        rep.check(1 <= len(desc) <= 1024, f"frontmatter: description must be 1-1024 characters (got {len(desc)})")
        rep.check(bool(USE_CUE_RE.search(NEGATED_USE_RE.sub(" ", desc))),
                  "frontmatter: description must state when to use the skill (`use` outside a negative cue such as `do not use`)")
        rep.check(bool(NEG_CUE_RE.search(desc)),
                  "frontmatter: description must state when NOT to use the skill (e.g. `do not use`, `not for`, `never`)")
        rep.check(not FORBIDDEN_FRAMING_RE.search(desc), "frontmatter: description uses forbidden `act as the world's best expert` framing")

    md = data.get("metadata")
    if not isinstance(md, dict):
        rep.fail("frontmatter: `metadata` block missing")
        return data, body
    mkeys = list(md.keys())
    missing = [k for k in METADATA_KEYS if k not in md]
    extra = [k for k in mkeys if k not in METADATA_KEYS]
    rep.check(not missing, f"metadata: missing keys {missing}")
    rep.check(not extra, f"metadata: unknown keys {extra}")
    for k in mkeys:
        rep.check(quoted.get(k, False), f"metadata: `{k}` must be a quoted string (all metadata values are strings)")

    code = md.get("g9-code", "")
    fam = md.get("g9-family", "")
    rep.code = code or "-"
    rep.family = fam or "-"
    rep.g9_status = md.get("g9-status", "-") or "-"
    if rep.check(bool(CODE_RE.match(code)), f"metadata: g9-code {code!r} is not one of E01-E14/T01-T08/L01-L06/P01-P06/K01-K03"):
        rep.check(fam == FAMILY_BY_PREFIX[code[0]],
                  f"metadata: g9-family {fam!r} inconsistent with code {code} (expected {FAMILY_BY_PREFIX[code[0]]})")
        expected_name = REGISTRY[code]
        rep.check(name == expected_name, f"metadata: registry binds {code} to {expected_name!r}, package is {name!r}")
    rep.check(fam in FAMILIES, f"metadata: g9-family {fam!r} not in {FAMILIES}")
    rep.check(bool(SEMVER_RE.match(md.get("g9-version", ""))), "metadata: g9-version must be MAJOR.MINOR.PATCH")
    rep.check(bool(MODULE_RE.match(md.get("g9-owner-module", ""))), "metadata: g9-owner-module must be M01-M17")

    def csv_set(key: str, allowed: list[str], allow_empty: bool) -> list[str]:
        raw = md.get(key, "")
        if raw == "":
            rep.check(allow_empty, f"metadata: `{key}` must not be empty")
            return []
        parts = [p.strip() for p in raw.split(",")]
        rep.check(all(parts), f"metadata: `{key}` has an empty element")
        bad = [p for p in parts if p and p not in allowed]
        rep.check(not bad, f"metadata: `{key}` has values outside the enum: {bad}")
        rep.check(len(set(parts)) == len(parts), f"metadata: `{key}` has duplicates")
        rep.check(raw == ",".join(parts), f"metadata: `{key}` must be comma-separated without spaces")
        return [p for p in parts if p in allowed]

    modes = csv_set("g9-modes", MODES, allow_empty=False)
    csv_set("g9-data-domains", DATA_DOMAINS, allow_empty=False)
    caps = csv_set("g9-capabilities", CAPABILITIES, allow_empty=True)
    op_only = [c for c in caps if c in OPERATOR_ONLY]
    rep.check(not op_only, f"metadata: g9-capabilities requests operator-only capabilities {op_only}")
    if fam == "engineering":
        non_fs = [c for c in caps if c != "FILESYSTEM_READ"]
        rep.check(not non_fs, f"metadata: engineering skills may list FILESYSTEM_READ only, found {non_fs}")
    rule = CONTENT_RULES.get(name, {})
    for c in rule.get("forbid_caps", []):
        rep.check(c not in caps, f"metadata: {name} must not request {c}")

    for key, (lo, hi) in BUDGET_BOUNDS.items():
        raw = md.get(key, "")
        if rep.check(bool(UINT_RE.match(raw)), f"metadata: `{key}` must be a non-negative integer string"):
            rep.check(lo <= int(raw) <= hi, f"metadata: `{key}`={raw} outside envelope bounds [{lo},{hi}]")
    rep.check(md.get("g9-recursion-depth") == "0", 'metadata: g9-recursion-depth must be "0"')

    deps_raw = md.get("g9-depends-on", "")
    if deps_raw != "":
        deps = [d.strip() for d in deps_raw.split(",")]
        rep.check(all(deps), "metadata: g9-depends-on has an empty element")
        rep.check(deps_raw == ",".join(deps), "metadata: g9-depends-on must be comma-separated without spaces")
        for d in deps:
            if not d:
                continue
            rep.check(d in NAME_TO_CODE, f"metadata: g9-depends-on {d!r} is not a registry skill name")
            rep.check(d != name, "metadata: g9-depends-on must not include the skill itself")
        rep.check(len(set(deps)) == len(deps), "metadata: g9-depends-on has duplicates")
    else:
        rep.ok()
    rep.check(md.get("g9-status") in G9_STATUS, f"metadata: g9-status must be one of {G9_STATUS}")
    data["_modes"] = modes
    data["_caps"] = caps
    return data, body


def check_body(rep: PackageReport, pkg: Path, body: str, name: str, eval_ids: dict[str, set[str]],
               output_schema: dict[str, Any] | None) -> None:
    sections, preamble, errs = split_sections(body)
    for e in errs:
        rep.fail(f"body: {e}")
    titles = [t for t, _ in sections]
    if not rep.check(titles == SECTIONS, f"body: H2 sections must be exactly {SECTIONS} in order; found {titles}"):
        return
    rep.check(not FORBIDDEN_FRAMING_RE.search(body), "body: forbidden `act as the world's best expert` framing")
    sec = {t: lines for t, lines in sections}

    # 1 Mission
    paras = paragraphs(sec["Mission"])
    rep.check(len(paras) == 1 and len(paras[0]) >= 40, "Mission: must be exactly one substantive paragraph")

    # 2 Activation
    blocks = h3_blocks(sec["Activation"])
    btitles = [t for t, _ in blocks]
    if rep.check(btitles == ["Positive triggers", "Negative triggers"],
                 f"Activation: must contain H3 `Positive triggers` then `Negative triggers`; found {btitles}"):
        pos_b = bullets(blocks[0][1])
        neg_b = bullets(blocks[1][1])
        rep.check(len(pos_b) >= 1, "Activation: Positive triggers needs at least one bullet")
        rep.check(len(neg_b) >= 2, "Activation: Negative triggers needs at least two concrete bullets")

    # 3 Inputs and provenance requirements
    inp = "\n".join(sec["Inputs and provenance requirements"])
    rep.check(any(tc in inp for tc in TRUST_CLASSES), "Inputs: must name at least one TrustClass (Operator, SystemDeterministic, ...)")
    rep.check(bool(re.search(r"\brefus|\breject", inp, re.IGNORECASE)), "Inputs: must state what is refused")

    # 4 Procedure
    proc_lines = sec["Procedure"]
    steps = [ln for ln in proc_lines if NUMBERED_RE.match(ln)]
    rep.check(len(steps) >= 3, "Procedure: needs at least three numbered steps")
    rep.check(bool(re.search(r"\bevidence\b", "\n".join(proc_lines), re.IGNORECASE)),
              "Procedure: steps that produce claims must say which evidence they cite")
    for m in re.finditer(r"references/([A-Za-z0-9._-]+\.md)", "\n".join(proc_lines)):
        rep.check((pkg / "references" / m.group(1)).is_file(), f"Procedure: references/{m.group(1)} does not exist")

    # 5 Non-goals
    rep.check(len(bullets(sec["Non-goals"])) >= 1, "Non-goals: needs at least one bullet")

    # 6 Tool and data scopes
    header, rows = table_rows(sec["Tool and data scopes"])
    if rep.check(header is not None and len(header) >= 3, "Tool and data scopes: needs a markdown table"):
        norm = [h.strip().lower() for h in header]  # type: ignore[union-attr]
        rep.check(norm[:3] == ["resource kind", "action", "why"],
                  f"Tool and data scopes: table header must be `resource kind | action | why`, found {header}")
        rep.check(len(rows) >= 1, "Tool and data scopes: table needs at least one row")
        bad = [r[1] for r in rows if len(r) < 3 or r[1] not in ("Read", "Propose")]
        rep.check(not bad, f"Tool and data scopes: action must be `Read` or `Propose`, found {bad}")

    # 7 Output contract
    out_text = "\n".join(sec["Output contract"])
    rep.check(len(out_text.strip()) > 0, "Output contract: empty")
    if output_schema is not None:
        props = output_schema.get("properties", {})
        if isinstance(props, dict):
            missing = [p for p in props if p not in out_text]
            rep.check(not missing, f"Output contract: does not mention output schema fields {missing}")

    # 8 Confidence and abstention
    rep.check("ABSTAINED" in "\n".join(sec["Confidence and abstention"]), "Confidence and abstention: must say when to return ABSTAINED")

    # 9 Escalation and stop conditions
    esc = "\n".join(sec["Escalation and stop conditions"])
    rep.check(bool(re.search(r"\bstop\b", esc, re.IGNORECASE)), "Escalation: must define hard stop conditions")
    rep.check(bool(re.search(r"\boperator\b", esc, re.IGNORECASE)), "Escalation: must say when to request an operator decision")

    # 10 Failure recovery
    fr = "\n".join(sec["Failure recovery"]).lower()
    for word in ("timeout", "invalid", "missing"):
        rep.check(word in fr, f"Failure recovery: must cover `{word}` (timeout, invalid output, missing evidence)")

    # 11 Acceptance tests
    acc_text = "\n".join(sec["Acceptance tests"])
    acc_bullets = bullets(sec["Acceptance tests"])
    rep.check(len(acc_bullets) >= 1, "Acceptance tests: needs a bullet list of eval case ids")
    referenced = {m.group(0) for m in CASE_ID_RE.finditer(acc_text)}
    all_ids = set().union(*eval_ids.values()) if eval_ids else set()
    unknown_ids = sorted(i for i in referenced if i not in all_ids)
    rep.check(not unknown_ids, f"Acceptance tests: references unknown eval case ids {unknown_ids}")
    acc_ids = eval_ids.get("acceptance", set())
    unlisted = sorted(i for i in acc_ids if i not in referenced)
    rep.check(not unlisted, f"Acceptance tests: acceptance case ids not listed {unlisted}")

    # 12 References
    ref_text = "\n".join(sec["References"])
    listed = {m.group(1) for m in re.finditer(r"references/([A-Za-z0-9._-]+\.md)", ref_text)}
    present = {p.name for p in (pkg / "references").glob("*.md")} if (pkg / "references").is_dir() else set()
    rep.check(len(bullets(sec["References"])) >= 1, "References: needs a bullet list")
    rep.check(not (present - listed), f"References: files not listed {sorted(present - listed)}")
    rep.check(not (listed - present), f"References: listed files missing {sorted(listed - present)}")

    rule = CONTENT_RULES.get(name, {})
    for pat in rule.get("body_must", []):
        rep.check(bool(re.search(pat, body, re.IGNORECASE)), f"content rule: {name} body must match /{pat}/")
    for pat in rule.get("body_must_not", []):
        rep.check(not re.search(pat, body, re.IGNORECASE), f"content rule: {name} body must not match /{pat}/")


def check_schema_file(rep: PackageReport, pkg: Path, which: str) -> dict[str, Any] | None:
    path = pkg / "schemas" / f"{which}.schema.json"
    doc, err = load_json(path)
    if err is not None:
        rep.fail(f"schemas/{which}: {err}")
        return None
    rep.ok()
    if not rep.check(isinstance(doc, dict), f"schemas/{which}: root must be an object"):
        return None
    rep.check(doc.get("$schema") == DRAFT_2020_12, f"schemas/{which}: $schema must be {DRAFT_2020_12}")
    expected_id = f"https://g9.local/skills/{pkg.name}/{which}"
    rep.check(doc.get("$id") == expected_id, f"schemas/{which}: $id must be {expected_id}")
    rep.check(isinstance(doc.get("title"), str) and doc["title"].strip() != "", f"schemas/{which}: title required")
    rep.check(doc.get("type") == "object", f"schemas/{which}: type must be \"object\"")
    rep.check(doc.get("additionalProperties") is False, f"schemas/{which}: additionalProperties must be false")
    props = doc.get("properties")
    if rep.check(isinstance(props, dict) and len(props) >= 1, f"schemas/{which}: properties must be a non-empty object"):
        req = doc.get("required")
        if rep.check(isinstance(req, list) and all(isinstance(r, str) for r in req), f"schemas/{which}: required must be an array of strings"):
            unknown = [r for r in req if r not in props]
            rep.check(not unknown, f"schemas/{which}: required names not in properties {unknown}")
            rep.check(len(set(req)) == len(req), f"schemas/{which}: required has duplicates")
    refs = find_key(doc, "$ref")
    rep.check(not refs, f"schemas/{which}: `$ref` is not allowed anywhere (found at {refs})")
    try:
        Draft202012Validator.check_schema(doc)
        rep.ok()
    except jsonschema.SchemaError as exc:
        rep.fail(f"schemas/{which}: not a valid Draft 2020-12 schema: {exc.message}")
        return None
    open_nested = nested_objects_without_closed_props(doc)
    if open_nested:
        rep.warn(f"schemas/{which}: nested object schemas without additionalProperties:false at {open_nested}")
    return doc


def check_case(rep: PackageReport, where: str, case: Any, code: str, kind: str, prefix: str,
               modes: list[str], input_validator: Draft202012Validator | None, name: str) -> str | None:
    if not isinstance(case, dict):
        rep.fail(f"{where}: case must be an object")
        return None
    missing = [k for k in CASE_REQUIRED if k not in case]
    extra = [k for k in case if k not in CASE_REQUIRED + CASE_OPTIONAL]
    rep.check(not missing, f"{where}: missing keys {missing}")
    rep.check(not extra, f"{where}: unknown keys {extra}")
    cid = case.get("id")
    id_re = re.compile(rf"^{code}-{prefix}-[0-9]{{3}}$")
    rep.check(isinstance(cid, str) and bool(id_re.match(cid)), f"{where}: id must match {code}-{prefix}-NNN")
    rep.check(case.get("kind") == kind, f"{where}: kind must be {kind!r}")
    for k in ("title", "intent", "notes"):
        rep.check(isinstance(case.get(k), str) and case.get(k, "").strip() != "", f"{where}: `{k}` must be a non-empty string")
    mode = case.get("mode")
    rep.check(mode in MODES, f"{where}: mode must be one of {MODES}")
    rep.check(case.get("actor_kind") in ACTOR_KINDS, f"{where}: actor_kind must be one of {ACTOR_KINDS}")
    rep.check(isinstance(case.get("input"), dict), f"{where}: input must be an object")
    if "context_fixture" in case:
        rep.check(isinstance(case["context_fixture"], str) and case["context_fixture"].strip() != "",
                  f"{where}: context_fixture must be a non-empty path string")
    tags: list[str] = []
    if "tags" in case:
        t = case["tags"]
        if rep.check(isinstance(t, list) and all(isinstance(x, str) and TAG_RE.match(x) for x in t),
                     f"{where}: tags must be an array of snake_case strings"):
            rep.check(len(set(t)) == len(t), f"{where}: tags has duplicates")
            tags = list(t)
    if "context_segments" in case:
        segs = case["context_segments"]
        if rep.check(isinstance(segs, list) and len(segs) >= 1, f"{where}: context_segments must be a non-empty array"):
            for i, s in enumerate(segs):
                sw = f"{where}.context_segments[{i}]"
                if not rep.check(isinstance(s, dict), f"{sw}: must be an object"):
                    continue
                rep.check(set(s.keys()) == {"trust_class", "classification", "content"},
                          f"{sw}: keys must be exactly trust_class, classification, content")
                rep.check(s.get("trust_class") in TRUST_CLASSES, f"{sw}: trust_class must be one of {TRUST_CLASSES}")
                rep.check(s.get("classification") in CLASSIFICATIONS, f"{sw}: classification must be one of {CLASSIFICATIONS}")
                rep.check(isinstance(s.get("content"), str) and s.get("content", "").strip() != "", f"{sw}: content must be a non-empty string")
                rep.check(s.get("classification") != "SECRET" or kind in ("negative", "adversarial"),
                          f"{sw}: SECRET content may appear only in negative/adversarial fixtures")
    exp = case.get("expect")
    if not rep.check(isinstance(exp, dict), f"{where}: expect must be an object"):
        return cid if isinstance(cid, str) else None
    rep.check(set(exp.keys()) <= EXPECT_KEYS, f"{where}: expect has unknown keys {sorted(set(exp.keys()) - EXPECT_KEYS)}")
    activate = exp.get("activate")
    status = exp.get("status")
    rep.check(isinstance(activate, bool), f"{where}: expect.activate must be a boolean")
    rep.check(status in STATUSES, f"{where}: expect.status must be one of {STATUSES}")
    for k in ("must_include", "must_not"):
        if k in exp:
            rep.check(isinstance(exp[k], list) and all(isinstance(x, str) and x.strip() for x in exp[k]),
                      f"{where}: expect.{k} must be an array of non-empty strings")
    approvals = exp.get("approval_requirements", [])
    if rep.check(isinstance(approvals, list) and all(isinstance(a, str) for a in approvals),
                 f"{where}: expect.approval_requirements must be an array of strings"):
        bad = [a for a in approvals if a not in CAPABILITIES]
        rep.check(not bad, f"{where}: expect.approval_requirements outside the capability enum {bad}")
        op = [a for a in approvals if a in OPERATOR_ONLY]
        rep.check(not op, f"{where}: a skill can never expect operator-only approval requirements {op}")
    kinds = exp.get("recommended_action_kinds", [])
    if rep.check(isinstance(kinds, list) and all(isinstance(a, str) for a in kinds),
                 f"{where}: expect.recommended_action_kinds must be an array of strings"):
        normalized = [pascal_to_screaming(a) for a in kinds]
        bad = [a for a in normalized if a not in ACTIONS]
        rep.check(not bad, f"{where}: expect.recommended_action_kinds outside the Action enum {bad}")
        forbidden = [a for a in normalized if a in ACTIONS and a not in SKILL_ALLOWED_ACTIONS]
        rep.check(not forbidden, f"{where}: skills may not recommend {forbidden} (operator decisions/operator-only operations)")
        only = CONTENT_RULES.get(name, {}).get("actions_only")
        if only is not None:
            off = [a for a in normalized if a not in only]
            rep.check(not off, f"{where}: {name} may only recommend {only}, found {off}")

    # Kind-specific semantics
    if kind == "positive":
        rep.check(activate is True and status == "COMPLETED", f"{where}: positive cases must activate and COMPLETE")
        rep.check(mode in modes, f"{where}: positive case mode {mode!r} not in the skill's g9-modes {modes}")
    elif kind == "negative":
        rep.check(activate is False or status == "ABSTAINED", f"{where}: negative cases must not activate or must ABSTAIN")
        if mode not in modes:
            rep.check(activate is False, f"{where}: mode {mode!r} is outside g9-modes, so activate must be false")
    elif kind == "adversarial":
        rep.check(len(tags) >= 1, f"{where}: adversarial cases must carry at least one tag")
        rep.check(isinstance(exp.get("must_not"), list) and len(exp.get("must_not", [])) >= 1,
                  f"{where}: adversarial cases must state expect.must_not (what must not appear/happen)")
    elif kind == "acceptance":
        rep.check(activate is True, f"{where}: acceptance cases must activate")
        rep.check(mode in modes, f"{where}: acceptance case mode {mode!r} not in the skill's g9-modes {modes}")
        rep.check(bool(exp.get("must_include")) or bool(exp.get("must_not")),
                  f"{where}: acceptance cases need a measurable expectation (must_include or must_not)")
    if kind in ("positive", "acceptance") and input_validator is not None and isinstance(case.get("input"), dict):
        errors = sorted(input_validator.iter_errors(case["input"]), key=lambda e: list(e.path))
        rep.check(not errors, f"{where}: input does not validate against input.schema.json: "
                  + "; ".join(f"{'/'.join(str(p) for p in e.path) or '$'}: {e.message}" for e in errors[:3]))
    return cid if isinstance(cid, str) else None


def check_evals(rep: PackageReport, pkg: Path, code: str, modes: list[str],
                input_schema: dict[str, Any] | None, name: str) -> dict[str, set[str]]:
    ids_by_kind: dict[str, set[str]] = {}
    seen: dict[str, str] = {}
    validator = Draft202012Validator(input_schema) if input_schema is not None else None
    for fname, kind, prefix, minimum in EVAL_FILES:
        path = pkg / "evals" / fname
        doc, err = load_json(path)
        where = f"evals/{fname}"
        if err is not None:
            rep.fail(f"{where}: {err}")
            continue
        if not rep.check(isinstance(doc, list), f"{where}: must be a JSON array"):
            continue
        rep.check(len(doc) >= minimum, f"{where}: needs at least {minimum} cases, found {len(doc)}")
        ids: set[str] = set()
        tag_union: set[str] = set()
        for i, case in enumerate(doc):
            cid = check_case(rep, f"{where}[{i}]", case, code, kind, prefix, modes, validator, name)
            if cid:
                rep.check(cid not in seen, f"{where}[{i}]: duplicate case id {cid} (also in {seen.get(cid)})")
                seen[cid] = where
                ids.add(cid)
            if isinstance(case, dict) and isinstance(case.get("tags"), list):
                tag_union.update(t for t in case["tags"] if isinstance(t, str))
        if kind == "adversarial":
            for t in REQUIRED_ADVERSARIAL_TAGS:
                rep.check(t in tag_union, f"{where}: must include at least one case tagged `{t}`")
        ids_by_kind[kind] = ids
    return ids_by_kind


def check_package(pkg: Path, repo_root: Path | None) -> PackageReport:
    rep = PackageReport(pkg.name, pkg)
    check_layout(rep, pkg)
    skill_md = pkg / "SKILL.md"
    if not skill_md.is_file():
        return rep
    try:
        text = skill_md.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        rep.fail("SKILL.md is not UTF-8")
        return rep
    data, body = check_frontmatter(rep, pkg, text)
    if data is None:
        return rep
    name = data.get("name", pkg.name) if isinstance(data.get("name"), str) else pkg.name
    code = rep.code if CODE_RE.match(rep.code) else None
    modes = data.get("_modes", [])
    input_schema = check_schema_file(rep, pkg, "input") if (pkg / "schemas").is_dir() else None
    output_schema = check_schema_file(rep, pkg, "output") if (pkg / "schemas").is_dir() else None
    eval_ids: dict[str, set[str]] = {}
    if code is not None and (pkg / "evals").is_dir():
        eval_ids = check_evals(rep, pkg, code, modes, input_schema, name)
    check_body(rep, pkg, body, name, eval_ids, output_schema)
    if rep.g9_status == "EVALUATED" and repo_root is not None:
        doc = repo_root / "docs" / "evaluations" / "skill-evals" / f"{name}.md"
        rep.check(doc.is_file(), f"status: EVALUATED requires {doc.relative_to(repo_root)}")
    return rep


# --------------------------------------------------------------------------
# CLI
# --------------------------------------------------------------------------

def render_table(reports: list[PackageReport]) -> str:
    cols = [("Package", 30), ("Code", 4), ("Family", 11), ("g9-status", 11), ("Checks", 6), ("Fail", 4), ("Warn", 4), ("Result", 6)]
    line = "  ".join(f"{t:<{w}}" for t, w in cols)
    out = [line, "-" * len(line)]
    for r in reports:
        cells = [r.name, r.code, r.family, r.g9_status, str(r.checks), str(len(r.failures)), str(len(r.warnings)),
                 "PASS" if r.passed else "FAIL"]
        out.append("  ".join(f"{c[:w]:<{w}}" for c, (_, w) in zip(cells, cols)))
    return "\n".join(out)


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Validate G9 skill packages against docs/skills/skill-package-contract.md")
    ap.add_argument("root", help="skills root directory, e.g. .agents/skills")
    ap.add_argument("--expect-count", type=int, default=37, help="required number of VALID packages (default 37)")
    ap.add_argument("--json", action="store_true", help="machine-readable JSON output")
    ap.add_argument("--repo-root", default=None, help="repository root (default: derived from root)")
    args = ap.parse_args(argv)

    root = Path(args.root)
    if not root.is_dir():
        sys.stderr.write(f"error: {root} is not a directory\n")
        return 2
    root = root.resolve()
    if args.repo_root:
        repo_root: Path | None = Path(args.repo_root).resolve()
    elif root.parent.name == ".agents":
        repo_root = root.parent.parent
    else:
        repo_root = None

    packages = sorted(p for p in root.iterdir() if p.is_dir() and not p.name.startswith("."))
    reports = [check_package(p, repo_root) for p in packages]

    # Cross-package invariants
    global_failures: list[str] = []
    codes: dict[str, list[str]] = {}
    for r in reports:
        if CODE_RE.match(r.code):
            codes.setdefault(r.code, []).append(r.name)
    for c, names in sorted(codes.items()):
        if len(names) > 1:
            global_failures.append(f"registry code {c} used by more than one package: {names}")
    stray = sorted(p.name for p in root.iterdir() if not p.is_dir())
    if stray:
        global_failures.append(f"non-directory entries in skills root are not allowed: {stray}")

    valid = sum(1 for r in reports if r.passed)
    invalid = len(reports) - valid
    count_ok = valid == args.expect_count
    ok = invalid == 0 and count_ok and not global_failures

    if args.json:
        payload = {
            "root": str(root),
            "expected_count": args.expect_count,
            "packages": [r.to_json() for r in reports],
            "global_failures": global_failures,
            "summary": {
                "packages": len(reports),
                "valid": valid,
                "invalid": invalid,
                "warnings": sum(len(r.warnings) for r in reports),
                "expected_count_ok": count_ok,
            },
            "ok": ok,
        }
        print(json.dumps(payload, indent=2, sort_keys=False))
        return 0 if ok else 1

    print(f"Skill package contract check — root: {root}")
    print(render_table(reports))
    for r in reports:
        if r.failures or r.warnings:
            print(f"\n{r.name}:")
            for f in r.failures:
                print(f"  FAIL  {f}")
            for w in r.warnings:
                print(f"  WARN  {w}")
    for g in global_failures:
        print(f"\nFAIL  {g}")
    print(f"\nSummary: {len(reports)} package(s), {valid} valid, {invalid} invalid, "
          f"{sum(len(r.warnings) for r in reports)} warning(s); expected {args.expect_count} valid -> "
          f"{'OK' if count_ok else 'MISMATCH'}")
    print("RESULT: " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
