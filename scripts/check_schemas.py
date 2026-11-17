#!/usr/bin/env python3
"""check_schemas.py — validate every *.schema.json under the given roots.

House rules (CLAUDE.md "Documents and schemas", docs/skills/skill-package-contract.md):
  * parses as JSON and the root is an object
  * `$schema` is exactly https://json-schema.org/draft/2020-12/schema
  * `$id` present, an absolute URI, unique across all scanned schemas
    (non-`https://g9.local/` ids produce a warning)
  * `title` non-empty, `type: "object"`, `additionalProperties: false`,
    explicit `required` array whose names all exist in `properties`
  * no external `$ref` or `$dynamicRef` (only `#...` fragments that resolve inside the document:
    a `#/pointer` that exists, or a `#name` defined by `$anchor` or `$dynamicAnchor`)
  * validates against the Draft 2020-12 metaschema with the `jsonschema` package
  * warning: nested object schemas without `additionalProperties: false`

Usage:
    python3 -I scripts/check_schemas.py schemas .agents/skills
    python3 -I scripts/check_schemas.py schemas .agents/skills --json

Exit codes: 0 all valid; 1 any failure; 2 usage/environment error.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

try:
    import jsonschema
    from jsonschema import Draft202012Validator
except ImportError:  # pragma: no cover
    sys.stderr.write("error: the `jsonschema` package (>= 4.18) is required\n")
    sys.exit(2)

DRAFT_2020_12 = "https://json-schema.org/draft/2020-12/schema"
HOUSE_ID_PREFIX = "https://g9.local/"
ABS_URI_RE = re.compile(r"^[a-z][a-z0-9+.-]*://[^\s]+$")
SKIP_DIRS = {".git", "node_modules", "target", "dist", "build", "__pycache__"}


class SchemaReport:
    def __init__(self, path: Path) -> None:
        self.path = path
        self.schema_id: str = "-"
        self.failures: list[str] = []
        self.warnings: list[str] = []
        self.checks = 0

    def check(self, cond: bool, msg: str) -> bool:
        self.checks += 1
        if not cond:
            self.failures.append(msg)
        return cond

    @property
    def passed(self) -> bool:
        return not self.failures

    def to_json(self, base: Path) -> dict[str, Any]:
        try:
            rel = str(self.path.relative_to(base))
        except ValueError:
            rel = str(self.path)
        return {
            "path": rel,
            "id": self.schema_id,
            "checks": self.checks,
            "pass": self.passed,
            "failures": list(self.failures),
            "warnings": list(self.warnings),
        }


# Keywords whose value is a URI reference to another schema; file resolution is disabled, so each
# must be an internal fragment.
REF_KEYWORDS = ("$ref", "$dynamicRef")


def iter_refs(node: Any, path: str = "#") -> list[tuple[str, str, Any]]:
    """(location, keyword, target) for every `$ref` and `$dynamicRef` in the document."""
    out: list[tuple[str, str, Any]] = []
    if isinstance(node, dict):
        for k, v in node.items():
            if k in REF_KEYWORDS:
                out.append((f"{path}/{k}", k, v))
            out.extend(iter_refs(v, f"{path}/{k}"))
    elif isinstance(node, list):
        for i, v in enumerate(node):
            out.extend(iter_refs(v, f"{path}/{i}"))
    return out


def iter_anchors(node: Any) -> set[str]:
    """Plain-name fragments defined in the document: `$anchor` and `$dynamicAnchor` both define one."""
    anchors: set[str] = set()
    if isinstance(node, dict):
        for keyword in ("$anchor", "$dynamicAnchor"):
            a = node.get(keyword)
            if isinstance(a, str):
                anchors.add(a)
        for v in node.values():
            anchors |= iter_anchors(v)
    elif isinstance(node, list):
        for v in node:
            anchors |= iter_anchors(v)
    return anchors


def resolve_pointer(doc: Any, pointer: str) -> bool:
    """Return True if the JSON pointer (without leading '#') resolves in doc."""
    if pointer in ("", "/"):
        return True
    if not pointer.startswith("/"):
        return False
    node = doc
    for raw in pointer[1:].split("/"):
        token = raw.replace("~1", "/").replace("~0", "~")
        if isinstance(node, dict):
            if token not in node:
                return False
            node = node[token]
        elif isinstance(node, list):
            if not token.isdigit() or int(token) >= len(node):
                return False
            node = node[int(token)]
        else:
            return False
    return True


def open_nested_objects(node: Any, path: str = "#") -> list[str]:
    hits: list[str] = []
    if isinstance(node, dict):
        if path != "#" and node.get("type") == "object" and "properties" in node and node.get("additionalProperties") is not False:
            hits.append(path)
        for k, v in node.items():
            hits.extend(open_nested_objects(v, f"{path}/{k}"))
    elif isinstance(node, list):
        for i, v in enumerate(node):
            hits.extend(open_nested_objects(v, f"{path}/{i}"))
    return hits


def check_schema(path: Path) -> SchemaReport:
    rep = SchemaReport(path)
    try:
        doc = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        rep.check(False, f"invalid JSON: {exc.msg} at line {exc.lineno}")
        return rep
    except UnicodeDecodeError:
        rep.check(False, "not UTF-8")
        return rep
    rep.checks += 1
    if not rep.check(isinstance(doc, dict), "root must be an object"):
        return rep
    rep.check(doc.get("$schema") == DRAFT_2020_12, f"$schema must be {DRAFT_2020_12}")
    sid = doc.get("$id")
    if rep.check(isinstance(sid, str) and bool(ABS_URI_RE.match(sid)), "$id must be present and an absolute URI"):
        rep.schema_id = sid
        if not sid.startswith(HOUSE_ID_PREFIX):
            rep.warnings.append(f"$id is outside the house namespace {HOUSE_ID_PREFIX}")
        rep.check("#" not in sid, "$id must not contain a fragment")
    rep.check(isinstance(doc.get("title"), str) and doc["title"].strip() != "", "title must be a non-empty string")
    rep.check(doc.get("type") == "object", 'type must be "object"')
    rep.check(doc.get("additionalProperties") is False, "additionalProperties must be false at the root")
    props = doc.get("properties")
    rep.check(isinstance(props, dict), "properties must be an object")
    req = doc.get("required")
    if rep.check(isinstance(req, list) and all(isinstance(r, str) for r in req), "required must be an explicit array of strings"):
        if isinstance(props, dict):
            unknown = [r for r in req if r not in props]
            rep.check(not unknown, f"required names not in properties: {unknown}")
        rep.check(len(set(req)) == len(req), "required has duplicates")

    anchors = iter_anchors(doc)
    for where, keyword, target in iter_refs(doc):
        if not rep.check(isinstance(target, str), f"{where}: {keyword} must be a string"):
            continue
        if not rep.check(target.startswith("#"), f"{where}: external {keyword} {target!r} is not allowed"):
            continue
        frag = target[1:]
        if frag.startswith("/") or frag == "":
            rep.check(resolve_pointer(doc, frag), f"{where}: internal {keyword} {target!r} does not resolve")
        else:
            rep.check(frag in anchors, f"{where}: internal {keyword} {target!r} names an unknown $anchor or $dynamicAnchor")

    try:
        Draft202012Validator.check_schema(doc)
        rep.checks += 1
    except jsonschema.SchemaError as exc:
        rep.check(False, f"not a valid Draft 2020-12 schema: {exc.message}")

    nested = open_nested_objects(doc)
    if nested:
        rep.warnings.append(f"nested object schemas without additionalProperties:false at {nested[:6]}"
                            + (" ..." if len(nested) > 6 else ""))
    return rep


def collect(roots: list[Path]) -> list[Path]:
    files: list[Path] = []
    for root in roots:
        if root.is_file():
            if root.name.endswith(".schema.json"):
                files.append(root)
            continue
        for p in sorted(root.rglob("*.schema.json")):
            if any(part in SKIP_DIRS for part in p.parts):
                continue
            if p.is_file():
                files.append(p)
    return sorted(set(files))


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Validate *.schema.json files against Draft 2020-12 and the G9 house rules")
    ap.add_argument("roots", nargs="+", help="directories (or files) to scan")
    ap.add_argument("--json", action="store_true", help="machine-readable JSON output")
    args = ap.parse_args(argv)

    roots = [Path(r) for r in args.roots]
    missing = [str(r) for r in roots if not r.exists()]
    if missing:
        sys.stderr.write(f"error: roots do not exist: {missing}\n")
        return 2
    base = Path.cwd().resolve()
    files = collect(roots)
    reports = [check_schema(p) for p in files]

    ids: dict[str, list[str]] = {}
    for r in reports:
        if r.schema_id != "-":
            ids.setdefault(r.schema_id, []).append(str(r.path))
    global_failures = [f"duplicate $id {sid} in {paths}" for sid, paths in sorted(ids.items()) if len(paths) > 1]
    if not files:
        global_failures.append("no *.schema.json files found under the given roots")

    valid = sum(1 for r in reports if r.passed)
    ok = valid == len(reports) and not global_failures

    if args.json:
        print(json.dumps({
            "roots": [str(r) for r in roots],
            "schemas": [r.to_json(base) for r in reports],
            "global_failures": global_failures,
            "summary": {"schemas": len(reports), "valid": valid, "invalid": len(reports) - valid,
                        "warnings": sum(len(r.warnings) for r in reports)},
            "ok": ok,
        }, indent=2))
        return 0 if ok else 1

    width = max([len(str(r.to_json(base)["path"])) for r in reports] + [6])
    print(f"{'Schema':<{width}}  {'Checks':>6}  {'Fail':>4}  {'Warn':>4}  Result")
    print("-" * (width + 30))
    for r in reports:
        rel = r.to_json(base)["path"]
        print(f"{rel:<{width}}  {r.checks:>6}  {len(r.failures):>4}  {len(r.warnings):>4}  {'PASS' if r.passed else 'FAIL'}")
    for r in reports:
        if r.failures or r.warnings:
            print(f"\n{r.to_json(base)['path']}:")
            for f in r.failures:
                print(f"  FAIL  {f}")
            for w in r.warnings:
                print(f"  WARN  {w}")
    for g in global_failures:
        print(f"\nFAIL  {g}")
    print(f"\nSummary: {len(reports)} schema(s), {valid} valid, {len(reports) - valid} invalid, "
          f"{sum(len(r.warnings) for r in reports)} warning(s)")
    print("RESULT: " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
