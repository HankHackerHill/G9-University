#!/usr/bin/env python3
"""check_source_register.py - validate the reference source register and render its table.

Validates data/sources/reference-source-register.v0.json against
schemas/sources/reference-source-register.schema.json (Draft 2020-12) and enforces the
honesty invariants that a schema cannot express:

  * ids are unique; every `host` equals the host of `url`; `license_url` is https
  * no HTML entities left in text fields
  * every entry is NOT_APPROVED (nothing in this file approves ingestion)
  * VERIFIED requires a clean ISO date in `checked_on` that is a real calendar date (the day a page
    was actually read) and evidence that names a fetched http(s) URL with a host name;
    PARTIAL/UNVERIFIED must not claim a clean fetch date (any YYYY-MM-DD value, valid or not)
  * docs/research/reference-source-register.md exists and equals the rendering of the register
    (a missing generated page is out of date, as in build_curriculum.py)
  * the register status stays PROPOSED_UNVERIFIED

Usage (repository root; needs `jsonschema`; no network):
    python3 -I scripts/check_source_register.py
    python3 -I scripts/check_source_register.py --write-markdown   # regenerates docs/research/reference-source-register.md

Exit codes: 0 valid; 1 any failure; 2 usage/environment error.
"""
from __future__ import annotations

import collections
import datetime
import json
import re
import sys
from pathlib import Path
from urllib.parse import urlparse

try:
    from jsonschema import Draft202012Validator
except ImportError:  # pragma: no cover
    sys.stderr.write("error: the `jsonschema` package (>= 4.18) is required\n")
    sys.exit(2)

ROOT = Path(__file__).resolve().parent.parent
REGISTER = ROOT / "data" / "sources" / "reference-source-register.v0.json"
SCHEMA = ROOT / "schemas" / "sources" / "reference-source-register.schema.json"
MARKDOWN = ROOT / "docs" / "research" / "reference-source-register.md"
CLEAN_DATE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
URL_IN_TEXT = re.compile(r"https?://\S+", re.IGNORECASE)
# Dotted host name; each label starts and ends with a letter or digit (no all-hyphen label).
HOST_LABEL = r"[a-z0-9](?:[a-z0-9-]*[a-z0-9])?"
HOST_NAME = re.compile(rf"^{HOST_LABEL}(\.{HOST_LABEL})+$")
ENTITY = re.compile(r"&(amp|lt|gt|quot|#\d+);")
TEXT_FIELDS = ("name", "publisher", "provides", "access", "license_or_terms", "update_cadence", "caveats", "evidence")


def is_calendar_date(value: str) -> bool:
    """True if `value` is YYYY-MM-DD and names a day that exists (no month 13, no 30 February)."""
    if not CLEAN_DATE.match(value):
        return False
    try:
        datetime.date.fromisoformat(value)
    except ValueError:
        return False
    return True


def names_http_url(text: str) -> bool:
    """True if `text` contains an absolute http(s) URL with a dotted host name, not just the word http."""
    for candidate in URL_IN_TEXT.findall(text):
        try:
            parsed = urlparse(candidate.rstrip(".,;:!?)]}>'\""))
            host = parsed.hostname or ""
            parsed.port  # raises ValueError for a port outside 0-65535 or one that is not a number
        except ValueError:
            continue
        if HOST_NAME.match(host):
            return True
    return False


def validate(reg: dict) -> list[str]:
    problems = []
    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    for err in sorted(Draft202012Validator(schema).iter_errors(reg), key=lambda e: list(e.absolute_path)):
        problems.append("schema: " + "/".join(str(p) for p in err.absolute_path) + ": " + err.message[:160])
    entries = reg.get("entries", [])
    ids = [e.get("id") for e in entries]
    for dup in sorted({i for i in ids if ids.count(i) > 1}):
        problems.append(f"duplicate id {dup}")
    for e in entries:
        i = e.get("id", "?")
        if urlparse(e.get("url", "")).hostname != e.get("host"):
            problems.append(f"{i}: host {e.get('host')!r} is not the host of url")
        if urlparse(e.get("license_url", "")).scheme != "https":
            problems.append(f"{i}: license_url is not https")
        for f in TEXT_FIELDS:
            if ENTITY.search(str(e.get(f, ""))):
                problems.append(f"{i}: {f} contains an HTML entity")
        if e.get("approval_state") != "NOT_APPROVED":
            problems.append(f"{i}: approval_state must be NOT_APPROVED")
        v = e.get("verification")
        checked_on = str(e.get("checked_on", ""))
        if v == "VERIFIED":
            if not is_calendar_date(checked_on):
                problems.append(f"{i}: VERIFIED needs checked_on to be the ISO date a page was read")
            if not names_http_url(str(e.get("evidence", ""))):
                problems.append(f"{i}: VERIFIED needs evidence that names the fetched URL")
        elif CLEAN_DATE.match(checked_on):
            problems.append(f"{i}: {v} entry carries a clean fetch date; either verify it or record why it is not")
    if reg.get("status") != "PROPOSED_UNVERIFIED":
        problems.append("register status must stay PROPOSED_UNVERIFIED")
    return problems


def markdown(reg: dict) -> str:
    es = reg["entries"]
    count = collections.Counter(e["verification"] for e in es)
    out = [
        "# Reference source register (cybersecurity and AI), v0",
        "",
        "Generated from `data/sources/reference-source-register.v0.json` by `python3 -I scripts/check_source_register.py --write-markdown`; do not edit by hand. Schema: `schemas/sources/reference-source-register.schema.json`.",
        "",
        f"**Status: {reg['status']}.** {reg['verification_note']}",
        "",
        f"Entries: {len(es)}. Verification: VERIFIED {count.get('VERIFIED', 0)}, PARTIAL {count.get('PARTIAL', 0)}, UNVERIFIED {count.get('UNVERIFIED', 0)}. Approved for ingestion: 0.",
        "",
        "Reading the columns: *Verification* says how much of the row was confirmed from a publisher page (VERIFIED = page read and dated; PARTIAL = search snippets; UNVERIFIED = prior knowledge only). *Terms* is the expected license or terms text and is not confirmed unless the row is VERIFIED. Rows whose caveats say link-only or do not ingest must stay that way whatever the license says.",
        "",
        "| ID | Source | Publisher | Tier | Kind | Verification | Terms (expected) | Caveats |",
        "|---|---|---|---|---|---|---|---|",
    ]
    def cell(s: str, n: int) -> str:
        s = " ".join(str(s).split()).replace("|", "/")
        return s if len(s) <= n else s[: n - 1] + "…"
    for e in es:
        out.append(
            f"| {e['id']} | [{cell(e['name'], 70)}]({e['url']}) | {cell(e['publisher'], 50)} | {e['trust_tier']} | {e['kind']} | {e['verification']} | {cell(e['license_or_terms'], 140)} | {cell(e['caveats'], 220)} |"
        )
    out += [
        "",
        "## Verification procedure (run on a host with network access)",
        "",
        "1. `python3 -I scripts/verify_source_register.py` lists the exact hosts and request count and does nothing else (dry run).",
        "2. After you have read that list, `python3 -I scripts/verify_source_register.py --approve-network --out verification-report.json` performs one HTTPS GET per distinct URL, follows at most five redirects, reads at most 256 KiB, and records status, final host, page title and licence keywords. It never modifies the register and never downloads datasets.",
        "3. A person reads each licence/terms page, then edits the register entry: set `verification` to VERIFIED only with `checked_on` = the ISO date and `evidence` naming the fetched URL and the decisive licence sentence. `python3 -I scripts/check_source_register.py` rejects anything weaker.",
        "4. Approval for ingestion is a separate operator decision recorded in the application (Stage 12, `SourceState`); this register never grants it.",
        "",
    ]
    return "\n".join(out)


def main(argv: list[str]) -> int:
    if argv[1:] not in ([], ["--write-markdown"]):
        print(__doc__)
        return 2
    reg = json.loads(REGISTER.read_text(encoding="utf-8"))
    problems = validate(reg)
    if problems:
        print(f"source register: FAIL ({len(problems)} problem(s))")
        for p in problems:
            print("  -", p)
        return 1
    count = collections.Counter(e["verification"] for e in reg["entries"])
    print(f"source register: OK ({len(reg['entries'])} entries; {dict(count)}; 0 approved)")
    if argv[1:] == ["--write-markdown"]:
        MARKDOWN.write_text(markdown(reg), encoding="utf-8")
        print("wrote", MARKDOWN.relative_to(ROOT))
    elif not MARKDOWN.is_file():
        print("source register: FAIL docs/research/reference-source-register.md is missing; run with --write-markdown")
        return 1
    elif MARKDOWN.read_text(encoding="utf-8") != markdown(reg):
        print("source register: FAIL docs/research/reference-source-register.md is stale; run with --write-markdown")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
