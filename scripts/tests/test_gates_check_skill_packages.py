"""Black-box tests for scripts/check_skill_packages.py (step CLI-2).

Each case copies one real package into <tmp>/.agents/skills/, applies one mutation and runs
`check_skill_packages.py <tmp>/.agents/skills --expect-count 1`; it asserts exit 1 and the failure text
that names the rule. The checks are enumerated from the script, function by function:

  check_layout       root entries; SKILL.md; references/ (exists, >= 1 .md, only .md); schemas/ (exactly
                     two files); evals/ (exactly four files); assets (a directory, no executable bit, no
                     script suffix); SKILL.md is UTF-8
  parse_frontmatter  opening/closing `---`; tabs; indentation inside/outside metadata; `key: value`;
                     key syntax; duplicate keys; space after `:`; `metadata:` block; double/single quoted
                     scalars (dangling/unsupported escape, trailing content, unterminated); flow/anchor
                     constructs; PyYAML cross-check (only when PyYAML is importable)
  check_frontmatter  required/unknown top-level keys; non-empty strings; name (length, pattern, g9-
                     prefix, directory); description (length, use cue, not-use cue, forbidden framing);
                     metadata (present, missing/unknown keys, quoted values, code, family, registry
                     binding, version, owner module, list syntax and enums for modes/domains/capabilities,
                     operator-only capabilities, engineering FILESYSTEM_READ only, per-skill forbidden
                     capabilities, budgets, recursion depth, depends-on, status)
  check_body         H1 inside a section; unterminated fence; the 12 H2 sections in order; forbidden
                     framing; one Mission paragraph; Activation H3s and bullets; TrustClass and refusal in
                     Inputs; Procedure steps, evidence, reference files; Non-goals bullet; scopes table
                     (present, header, rows, Read/Propose); Output contract (non-empty, every output
                     field); ABSTAINED; stop and operator in Escalation; timeout/invalid/missing in
                     Failure recovery; Acceptance tests (bullets, unknown ids, unlisted acceptance ids);
                     References (bullets, unlisted files, missing files); per-skill body rules
  check_schema_file  file present, JSON, UTF-8, object; $schema; $id; title; type; additionalProperties;
                     non-empty properties; required (array of strings, known names, no duplicates);
                     no $ref anywhere; Draft 2020-12 metaschema; nested open objects warn only
  check_case/evals   file present/JSON/UTF-8/array; minimum case count; duplicate ids; case object; keys;
                     id pattern; kind; non-empty title/intent/notes; mode; actor_kind; input object;
                     context_fixture; tags; context_segments (array, object, keys, trust class,
                     classification, content, SECRET only in negative/adversarial); expect (object,
                     keys, activate, status, must_include/must_not, approval requirements, recommended
                     action kinds, per-skill action allow-list); kind semantics; input against
                     input.schema.json; required adversarial tags
  check_package      EVALUATED needs docs/evaluations/skill-evals/<name>.md
  main               duplicate registry code; stray files in the root; --expect-count; root missing (exit 2)
"""

from __future__ import annotations

import json
import os
import shutil
import unittest
from pathlib import Path
from typing import Callable

from _gate_helpers import (
    HAVE_YAML,
    NEEDS_JSONSCHEMA,
    REPO,
    SCRIPTS,
    GateTestCase,
    InputGuard,
    copy_into,
    edit_json,
    output_of,
    read_text,
    replace_once,
    run_gate,
    write_text,
)

SCRIPT = SCRIPTS / "check_skill_packages.py"
SKILLS = ".agents/skills"
BASE = "g9-session-director"  # T01, teaching, modes STUDY,LAB
INPUTS = [SKILLS]

_GUARD = InputGuard(INPUTS)


def setUpModule() -> None:
    _GUARD.snapshot()


def tearDownModule() -> None:
    _GUARD.verify()


class Pkg:
    """Mutations of one copied package; every edit raises if its target is absent."""

    def __init__(self, root: Path, name: str) -> None:
        self.dir = root / name
        self.md = self.dir / "SKILL.md"

    def sub(self, old: str, new: str, after: str | None = None) -> None:
        replace_once(self.md, old, new, after=after)

    def sub_all(self, old: str, new: str) -> None:
        text = read_text(self.md)
        if old not in text:
            raise AssertionError(f"{old!r} not in SKILL.md")
        write_text(self.md, text.replace(old, new))

    def fm(self, key: str, raw: str) -> None:
        """Replace the first frontmatter line for `key` (top level or metadata) with `key: raw`."""
        lines = read_text(self.md).split("\n")
        for i, line in enumerate(lines):
            stripped = line.lstrip(" ")
            if stripped.startswith(f"{key}:"):
                lines[i] = line[: len(line) - len(stripped)] + f"{key}: {raw}"
                write_text(self.md, "\n".join(lines))
                return
        raise AssertionError(f"frontmatter key {key!r} not found")

    def drop_line(self, prefix: str) -> None:
        lines = read_text(self.md).split("\n")
        idx = next(i for i, line in enumerate(lines) if line.startswith(prefix))
        del lines[idx]
        write_text(self.md, "\n".join(lines))

    def section(self, title: str, fn: Callable[[list[str]], list[str]]) -> None:
        """Rewrite the body lines of the H2 section `title` (up to the next H2)."""
        lines = read_text(self.md).split("\n")
        start = lines.index(f"## {title}") + 1
        end = next((i for i in range(start, len(lines)) if lines[i].startswith("## ")), len(lines))
        write_text(self.md, "\n".join(lines[:start] + fn(lines[start:end]) + lines[end:]))

    def unbullet(self, title: str) -> None:
        self.section(title, lambda ls: [line[2:] if line.startswith("- ") else line for line in ls])

    def ev(self, fname: str, fn: Callable) -> None:
        edit_json(self.dir / "evals" / fname, fn)

    def schema(self, which: str, fn: Callable) -> None:
        edit_json(self.dir / "schemas" / f"{which}.schema.json", fn)

    def write(self, rel: str, text: str) -> None:
        write_text(self.dir / rel, text)

    def remove(self, rel: str) -> None:
        path = self.dir / rel
        if path.is_dir():
            shutil.rmtree(path)
        else:
            path.unlink()


def first(fname: str, fn: Callable[[dict], None], index: int = 0) -> Callable[[Pkg], None]:
    """Mutation of case `index` of an eval file."""
    return lambda p: p.ev(fname, lambda cases: fn(cases[index]))


def expect(fname: str, fn: Callable[[dict], None], index: int = 0) -> Callable[[Pkg], None]:
    return first(fname, lambda case: fn(case["expect"]), index)


def segment(**fields) -> Callable[[Pkg], None]:
    seg = {"trust_class": "Operator", "classification": "INTERNAL", "content": "operator note"}
    seg.update(fields)
    return first("positive.json", lambda c: c.update({"context_segments": [seg]}))


def assets_file(name: str, mode: int) -> Callable[[Pkg], None]:
    def fn(p: Pkg) -> None:
        p.write(f"assets/{name}", "template\n")
        os.chmod(p.dir / "assets" / name, mode)

    return fn


def drop_metadata_block(p: Pkg) -> None:
    text = read_text(p.md)
    start = text.index("metadata:\n")
    end = text.index("\n---\n", start)
    write_text(p.md, text[:start] + text[end + 1:])


def swap_trigger_headings(p: Pkg) -> None:
    p.sub("### Positive triggers", "### PLACEHOLDER")
    p.sub("### Negative triggers", "### Positive triggers")
    p.sub("### PLACEHOLDER", "### Negative triggers")


POS = "positive.json"
NEG = "negative.json"
ADV = "adversarial.json"
ACC = "acceptance.json"

# (name, package, mutation, expected substrings, expected exit)
CASES: list[tuple[str, str, Callable[[Pkg], None], list[str], int]] = [
    # -- layout ------------------------------------------------------------------------------------
    ("layout_unexpected_root_entry", BASE, lambda p: p.write("NOTES.md", "x\n"), ["layout: unexpected entries in package root: ['NOTES.md']"], 1),
    ("layout_skill_md_missing", BASE, lambda p: p.remove("SKILL.md"), ["layout: SKILL.md missing"], 1),
    ("layout_references_missing", BASE, lambda p: p.remove("references"), ["layout: references/ directory missing"], 1),
    ("layout_references_without_md", BASE, lambda p: [p.remove(f"references/{f.name}") for f in sorted((p.dir / "references").iterdir())], ["layout: references/ must contain at least one .md file"], 1),
    ("layout_references_non_md", BASE, lambda p: p.write("references/notes.txt", "x\n"), ["layout: references/ may contain only .md files, found ['notes.txt']"], 1),
    ("layout_schemas_missing", BASE, lambda p: p.remove("schemas"), ["layout: schemas/ directory missing"], 1),
    ("layout_schemas_extra_file", BASE, lambda p: p.write("schemas/extra.schema.json", "{}\n"), ["layout: schemas/ must contain exactly input.schema.json and output.schema.json"], 1),
    ("layout_evals_missing", BASE, lambda p: p.remove("evals"), ["layout: evals/ directory missing"], 1),
    ("layout_evals_extra_file", BASE, lambda p: p.write("evals/extra.json", "[]\n"), ["layout: evals/ must contain exactly the four eval files"], 1),
    ("layout_assets_not_directory", BASE, lambda p: p.write("assets", "x\n"), ["layout: assets must be a directory"], 1),
    ("layout_assets_executable", BASE, assets_file("template.md", 0o755), ["layout: assets/template.md has an executable bit set"], 1),
    ("layout_assets_script_suffix", BASE, assets_file("run.sh", 0o644), ["layout: assets/run.sh looks like a script"], 1),
    ("skill_md_not_utf8", BASE, lambda p: p.md.write_bytes(b"---\nname: g9-\xff\n---\n"), ["SKILL.md is not UTF-8"], 1),
    # -- frontmatter syntax -------------------------------------------------------------------------
    ("fm_must_start_with_dashes", BASE, lambda p: p.sub("---\nname:", "# Title\n---\nname:"), ["frontmatter: SKILL.md must start with a `---` line"], 1),
    ("fm_closing_missing", BASE, lambda p: p.sub('  g9-status: "PROPOSED"\n---\n', '  g9-status: "PROPOSED"\n'), ["frontmatter: frontmatter closing `---` not found"], 1),
    ("fm_tab_indentation", BASE, lambda p: p.sub('  g9-code: "T01"', '\tg9-code: "T01"'), ["tabs are not allowed for indentation"], 1),
    ("fm_indented_outside_metadata", BASE, lambda p: p.sub("license: Apache-2.0", " license: Apache-2.0"), ["unexpected indented line outside `metadata:`"], 1),
    ("fm_metadata_indent", BASE, lambda p: p.sub('  g9-code: "T01"', '   g9-code: "T01"'), ["metadata entries must be indented by exactly two spaces"], 1),
    ("fm_metadata_line_without_colon", BASE, lambda p: p.sub('  g9-status: "PROPOSED"', '  g9-status "PROPOSED"'), ["frontmatter line 18: expected `key: value`"], 1),
    ("fm_top_line_without_colon", BASE, lambda p: p.sub("license: Apache-2.0", "license Apache-2.0"), ["frontmatter line 4: expected `key: value`"], 1),
    ("fm_invalid_metadata_key", BASE, lambda p: p.sub('  g9-code: "T01"', '  G9-Code: "T01"'), ["invalid metadata key 'G9-Code'"], 1),
    ("fm_duplicate_metadata_key", BASE, lambda p: p.sub('  g9-family: "teaching"', '  g9-code: "T01"\n  g9-family: "teaching"'), ["duplicate metadata key 'g9-code'"], 1),
    ("fm_metadata_no_space_after_colon", BASE, lambda p: p.sub('  g9-code: "T01"', '  g9-code:"T01"'), ["frontmatter line 6: expected a space after `:`"], 1),
    ("fm_top_no_space_after_colon", BASE, lambda p: p.sub("license: Apache-2.0", "license:Apache-2.0"), ["frontmatter line 4: expected a space after `:`"], 1),
    ("fm_invalid_top_key", BASE, lambda p: p.sub("license: Apache-2.0", "License: Apache-2.0"), ["invalid key 'License'"], 1),
    ("fm_duplicate_top_key", BASE, lambda p: p.sub("license: Apache-2.0", "license: Apache-2.0\nlicense: MIT"), ["duplicate key 'license'"], 1),
    ("fm_metadata_inline_value", BASE, lambda p: p.sub("metadata:\n", "metadata: inline\n"), ["`metadata:` must introduce an indented block"], 1),
    ("fm_dangling_escape", BASE, lambda p: p.sub('  g9-code: "T01"', '  g9-code: "T01\\'), ["dangling escape in double-quoted scalar"], 1),
    ("fm_unsupported_escape", BASE, lambda p: p.sub('  g9-code: "T01"', '  g9-code: "T\\x01"'), ["unsupported escape \\x"], 1),
    ("fm_double_quote_trailing_content", BASE, lambda p: p.sub('  g9-code: "T01"', '  g9-code: "T01" extra'), ["trailing content after closing quote: 'extra'"], 1),
    ("fm_double_quote_unterminated", BASE, lambda p: p.sub('  g9-code: "T01"', '  g9-code: "T01'), ["unterminated double-quoted scalar"], 1),
    ("fm_single_quote_trailing_content", BASE, lambda p: p.sub('  g9-code: "T01"', "  g9-code: 'T01' extra"), ["trailing content after closing quote: 'extra'"], 1),
    ("fm_single_quote_unterminated", BASE, lambda p: p.sub('  g9-code: "T01"', "  g9-code: 'T01"), ["unterminated single-quoted scalar"], 1),
    ("fm_flow_construct", BASE, lambda p: p.sub("license: Apache-2.0", "license: [Apache-2.0]"), ["unsupported YAML construct starting with '['"], 1),
    # -- frontmatter content -----------------------------------------------------------------------
    ("fm_required_key_missing", BASE, lambda p: p.drop_line("license: "), ["frontmatter: required key `license` missing"], 1),
    ("fm_unknown_top_key", BASE, lambda p: p.sub("license: Apache-2.0", "license: Apache-2.0\nhomepage: local"), ["frontmatter: unknown top-level keys ['homepage']"], 1),
    ("fm_empty_string", BASE, lambda p: p.fm("license", '""'), ["frontmatter: `license` must be a non-empty string"], 1),
    ("name_too_long", BASE, lambda p: p.fm("name", "g9-" + "a" * 62), ["frontmatter: name must be 1-64 characters"], 1),
    ("name_pattern", BASE, lambda p: p.fm("name", "g9--session-director"), ["frontmatter: name must match ^[a-z0-9]+(-[a-z0-9]+)*$"], 1),
    ("name_prefix", BASE, lambda p: p.fm("name", "x9-session-director"), ["frontmatter: name must start with `g9-`"], 1),
    ("name_directory", BASE, lambda p: p.fm("name", "g9-session-directors"), ["frontmatter: name 'g9-session-directors' does not match directory 'g9-session-director'"], 1),
    ("description_too_long", BASE, lambda p: p.sub("or any EXAM session.", "or any EXAM session." + " pad" * 300), ["frontmatter: description must be 1-1024 characters (got"], 1),
    ("description_without_use_cue", BASE, lambda p: p.fm("description", "Prepares the daily session brief from recorded evidence. Not for grading or EXAM sessions."), ["frontmatter: description must state when to use the skill (contains `use`)"], 1),
    ("description_without_not_use_cue", BASE, lambda p: p.fm("description", "Prepares the daily session brief. Use at day start in STUDY or LAB mode."), ["frontmatter: description must state when NOT to use the skill"], 1),
    ("description_forbidden_framing", BASE, lambda p: p.sub("or any EXAM session.", "or any EXAM session. Act as the world's best expert."), ["frontmatter: description uses forbidden `act as the world's best expert` framing"], 1),
    ("metadata_block_missing", BASE, drop_metadata_block, ["frontmatter: `metadata` block missing"], 1),
    ("metadata_key_missing", BASE, lambda p: p.drop_line('  g9-status: "PROPOSED"'), ["metadata: missing keys ['g9-status']"], 1),
    ("metadata_key_unknown", BASE, lambda p: p.sub('  g9-status: "PROPOSED"', '  g9-status: "PROPOSED"\n  g9-extra: "x"'), ["metadata: unknown keys ['g9-extra']"], 1),
    ("metadata_value_unquoted", BASE, lambda p: p.fm("g9-version", "0.1.0"), ["metadata: `g9-version` must be a quoted string"], 1),
    ("metadata_code_invalid", BASE, lambda p: p.fm("g9-code", '"T99"'), ["metadata: g9-code 'T99' is not one of E01-E14/T01-T08/L01-L06/P01-P06/K01-K03"], 1),
    ("metadata_family_inconsistent", BASE, lambda p: p.fm("g9-family", '"learning"'), ["metadata: g9-family 'learning' inconsistent with code T01 (expected teaching)"], 1),
    ("metadata_registry_binding", BASE, lambda p: p.fm("g9-code", '"T02"'), ["metadata: registry binds T02 to 'g9-prerequisite-diagnosis', package is 'g9-session-director'"], 1),
    ("metadata_family_enum", BASE, lambda p: p.fm("g9-family", '"teach"'), ["metadata: g9-family 'teach' not in"], 1),
    ("metadata_version_semver", BASE, lambda p: p.fm("g9-version", '"0.1"'), ["metadata: g9-version must be MAJOR.MINOR.PATCH"], 1),
    ("metadata_owner_module", BASE, lambda p: p.fm("g9-owner-module", '"M18"'), ["metadata: g9-owner-module must be M01-M17"], 1),
    ("metadata_modes_empty", BASE, lambda p: p.fm("g9-modes", '""'), ["metadata: `g9-modes` must not be empty"], 1),
    ("metadata_modes_empty_element", BASE, lambda p: p.fm("g9-modes", '"STUDY,,LAB"'), ["metadata: `g9-modes` has an empty element"], 1),
    ("metadata_modes_enum", BASE, lambda p: p.fm("g9-modes", '"STUDY,PLAY"'), ["metadata: `g9-modes` has values outside the enum: ['PLAY']"], 1),
    ("metadata_modes_duplicates", BASE, lambda p: p.fm("g9-modes", '"STUDY,LAB,STUDY"'), ["metadata: `g9-modes` has duplicates"], 1),
    ("metadata_modes_spaces", BASE, lambda p: p.fm("g9-modes", '"STUDY, LAB"'), ["metadata: `g9-modes` must be comma-separated without spaces"], 1),
    ("metadata_domains_enum", BASE, lambda p: p.fm("g9-data-domains", '"LEARNING,GAMES"'), ["metadata: `g9-data-domains` has values outside the enum: ['GAMES']"], 1),
    ("metadata_capabilities_enum", BASE, lambda p: p.fm("g9-capabilities", '"SKILL_INVOKE,TELEPORT"'), ["metadata: `g9-capabilities` has values outside the enum: ['TELEPORT']"], 1),
    ("metadata_operator_only_capability", BASE, lambda p: p.fm("g9-capabilities", '"SKILL_INVOKE,MODEL_INFERENCE,SHELL_EXEC"'), ["metadata: g9-capabilities requests operator-only capabilities ['SHELL_EXEC']"], 1),
    ("metadata_engineering_filesystem_read_only", "g9-spec-assurance", lambda p: p.fm("g9-capabilities", '"FILESYSTEM_READ,MODEL_INFERENCE"'), ["metadata: engineering skills may list FILESYSTEM_READ only, found ['MODEL_INFERENCE']"], 1),
    ("metadata_examiner_forbidden_capability", "g9-independent-examiner", lambda p: p.fm("g9-capabilities", '"SKILL_INVOKE,MODEL_INFERENCE,PROTECTED_RECORD_WRITE"'), ["metadata: g9-independent-examiner must not request PROTECTED_RECORD_WRITE"], 1),
    ("metadata_agenda_forbidden_capability", "g9-adaptive-agenda", lambda p: p.fm("g9-capabilities", '"SKILL_INVOKE,MODEL_INFERENCE,SCHEDULE_APPLY"'), ["metadata: g9-adaptive-agenda must not request SCHEDULE_APPLY"], 1),
    ("metadata_budget_not_integer", BASE, lambda p: p.fm("g9-budget-tokens", '"4k"'), ["metadata: `g9-budget-tokens` must be a non-negative integer string"], 1),
    ("metadata_budget_tokens_bounds", BASE, lambda p: p.fm("g9-budget-tokens", '"40000"'), ["metadata: `g9-budget-tokens`=40000 outside envelope bounds [1,32768]"], 1),
    ("metadata_budget_seconds_bounds", BASE, lambda p: p.fm("g9-budget-seconds", '"0"'), ["metadata: `g9-budget-seconds`=0 outside envelope bounds [1,600]"], 1),
    ("metadata_budget_tool_calls_bounds", BASE, lambda p: p.fm("g9-budget-tool-calls", '"17"'), ["metadata: `g9-budget-tool-calls`=17 outside envelope bounds [0,16]"], 1),
    ("metadata_recursion_depth", BASE, lambda p: p.fm("g9-recursion-depth", '"1"'), ['metadata: g9-recursion-depth must be "0"'], 1),
    ("metadata_depends_empty_element", BASE, lambda p: p.fm("g9-depends-on", '"g9-adaptive-agenda,"'), ["metadata: g9-depends-on has an empty element"], 1),
    ("metadata_depends_spaces", BASE, lambda p: p.fm("g9-depends-on", '"g9-adaptive-agenda, g9-goal-decomposition"'), ["metadata: g9-depends-on must be comma-separated without spaces"], 1),
    ("metadata_depends_unknown", BASE, lambda p: p.fm("g9-depends-on", '"g9-unknown-skill"'), ["metadata: g9-depends-on 'g9-unknown-skill' is not a registry skill name"], 1),
    ("metadata_depends_self", BASE, lambda p: p.fm("g9-depends-on", '"g9-session-director"'), ["metadata: g9-depends-on must not include the skill itself"], 1),
    ("metadata_depends_duplicates", BASE, lambda p: p.fm("g9-depends-on", '"g9-adaptive-agenda,g9-adaptive-agenda"'), ["metadata: g9-depends-on has duplicates"], 1),
    ("metadata_status_enum", BASE, lambda p: p.fm("g9-status", '"DONE"'), ["metadata: g9-status must be one of"], 1),
    ("status_evaluated_needs_report", BASE, lambda p: p.fm("g9-status", '"EVALUATED"'), ["status: EVALUATED requires docs/evaluations/skill-evals/g9-session-director.md"], 1),
    # -- body ---------------------------------------------------------------------------------------
    ("body_h1_inside_section", BASE, lambda p: p.sub("## Non-goals\n", "## Non-goals\n\n# Stray heading\n"), ["body: H1 heading inside section is not allowed: '# Stray heading'"], 1),
    ("body_unterminated_fence", BASE, lambda p: p.sub("## Non-goals\n", "## Non-goals\n\n```text\n"), ["body: unterminated fenced code block in SKILL.md body"], 1),
    ("body_sections_exact", BASE, lambda p: p.sub("## Non-goals", "## Nongoals"), ["body: H2 sections must be exactly"], 1),
    ("body_forbidden_framing", BASE, lambda p: p.sub("Give the operator an exact", "Act as the world's best expert. Give the operator an exact"), ["body: forbidden `act as the world's best expert` framing"], 1),
    ("mission_one_paragraph", BASE, lambda p: p.section("Mission", lambda ls: ls + ["A second paragraph that is long enough to count as substance.", ""]), ["Mission: must be exactly one substantive paragraph"], 1),
    ("activation_h3_order", BASE, swap_trigger_headings, ["Activation: must contain H3 `Positive triggers` then `Negative triggers`"], 1),
    ("activation_positive_bullet", BASE, lambda p: p.sub("- Operator starts the day and asks what to work on\n- A LAB session begins for a training day\n- Operator asks for today's brief after a missed day\n", "Operator starts the day and asks what to work on.\n"), ["Activation: Positive triggers needs at least one bullet"], 1),
    ("activation_negative_bullets", BASE, lambda p: p.sub("- Requests to grade or finalize an attempt\n- Weekly executive planning or business goals\n", ""), ["Activation: Negative triggers needs at least two concrete bullets"], 1),
    ("inputs_trust_class", BASE, lambda p: p.section("Inputs and provenance requirements", lambda ls: ["", "- training day objectives (curriculum version)", "", "Refused: secret content.", ""]), ["Inputs: must name at least one TrustClass"], 1),
    ("inputs_refusal", BASE, lambda p: p.section("Inputs and provenance requirements", lambda ls: ["", "- training day objectives — TrustClass `SystemDeterministic`", "", "Secret content never enters the context.", ""]), ["Inputs: must state what is refused"], 1),
    ("procedure_three_steps", BASE, lambda p: p.section("Procedure", lambda ls: ["", "1. Load the day's objectives (evidence: observation ids).", "2. Return the brief.", ""]), ["Procedure: needs at least three numbered steps"], 1),
    ("procedure_evidence", BASE, lambda p: p.sub("(evidence: observation ids)", "(observation ids)"), ["Procedure: steps that produce claims must say which evidence they cite"], 1),
    ("procedure_reference_exists", BASE, lambda p: p.sub("follow references/methodology.md", "follow references/method.md"), ["Procedure: references/method.md does not exist"], 1),
    ("non_goals_bullet", BASE, lambda p: p.unbullet("Non-goals"), ["Non-goals: needs at least one bullet"], 1),
    ("scopes_table_missing", BASE, lambda p: p.section("Tool and data scopes", lambda ls: ["", "Scopes are requests evaluated by `g9-policy`.", ""]), ["Tool and data scopes: needs a markdown table"], 1),
    ("scopes_table_header", BASE, lambda p: p.sub("| Resource kind | Action | Why |", "| Resource | Action | Why |"), ["Tool and data scopes: table header must be `resource kind | action | why`"], 1),
    ("scopes_table_rows", BASE, lambda p: p.section("Tool and data scopes", lambda ls: ["", "| Resource kind | Action | Why |", "|---|---|---|", "", "Scopes are requests.", ""]), ["Tool and data scopes: table needs at least one row"], 1),
    ("scopes_table_action", BASE, lambda p: p.sub("| Curriculum | Read |", "| Curriculum | Write |"), ["Tool and data scopes: action must be `Read` or `Propose`, found ['Write']"], 1),
    ("output_contract_empty", BASE, lambda p: p.section("Output contract", lambda ls: [""]), ["Output contract: empty"], 1),
    ("output_contract_field", BASE, lambda p: p.sub("- `next_step` — Proposed first action (string)\n", ""), ["Output contract: does not mention output schema fields ['next_step']"], 1),
    ("confidence_abstained", BASE, lambda p: p.sub("ABSTAINED when the day id", "Abstain when the day id"), ["Confidence and abstention: must say when to return ABSTAINED"], 1),
    ("escalation_stop", BASE, lambda p: p.sub("Stop if a note asks", "Halt if a note asks"), ["Escalation: must define hard stop conditions"], 1),
    ("escalation_operator", BASE, lambda p: p.sub("Operator decision when prerequisites", "A decision when prerequisites"), ["Escalation: must say when to request an operator decision"], 1),
    ("failure_recovery_timeout", BASE, lambda p: p.sub("Timeout: brief", "Time limit: brief"), ["Failure recovery: must cover `timeout`"], 1),
    ("failure_recovery_invalid", BASE, lambda p: p.sub("invalid output: rejected", "malformed output: rejected"), ["Failure recovery: must cover `invalid`"], 1),
    ("failure_recovery_missing", BASE, lambda p: p.sub("missing evidence: list missing record ids", "absent evidence: list absent record ids"), ["Failure recovery: must cover `missing`"], 1),
    ("acceptance_bullets", BASE, lambda p: p.unbullet("Acceptance tests"), ["Acceptance tests: needs a bullet list of eval case ids"], 1),
    ("acceptance_unknown_id", BASE, lambda p: p.sub("- `T01-ACC-002`", "- `T01-POS-009` — not a case\n- `T01-ACC-002`"), ["Acceptance tests: references unknown eval case ids ['T01-POS-009']"], 1),
    ("acceptance_unlisted_id", BASE, lambda p: p.drop_line("- `T01-ACC-002`"), ["Acceptance tests: acceptance case ids not listed ['T01-ACC-002']"], 1),
    ("references_bullets", BASE, lambda p: p.unbullet("References"), ["References: needs a bullet list"], 1),
    ("references_file_not_listed", BASE, lambda p: p.write("references/extra.md", "# Extra\n"), ["References: files not listed ['extra.md']"], 1),
    ("references_listed_file_missing", BASE, lambda p: p.section("References", lambda ls: ls[:-1] + ["- `references/ghost.md` — missing", ""]), ["References: listed files missing ['ghost.md']"], 1),
    ("content_rule_body_must", "g9-independent-examiner", lambda p: p.sub_all("MODEL_ADVISORY", "MODEL_HINT"), ["content rule: g9-independent-examiner body must match /MODEL_ADVISORY/"], 1),
    ("content_rule_body_must_not_finalize", "g9-independent-examiner", lambda p: p.sub("the operator reviewer makes the final decision.", "the skill finalizes the grade."), ["content rule: g9-independent-examiner body must not match"], 1),
    ("content_rule_body_must_not_threshold", "g9-learning-experiment", lambda p: p.sub("whenever the data do not discriminate.", "whenever the data do not discriminate, and a win at p < 0.05."), ["content rule: g9-learning-experiment body must not match"], 1),
    # -- schemas ------------------------------------------------------------------------------------
    ("schema_file_missing", BASE, lambda p: p.remove("schemas/input.schema.json"), ["schemas/input: missing"], 1),
    ("schema_invalid_json", BASE, lambda p: p.write("schemas/input.schema.json", '{"type": '), ["schemas/input: invalid JSON"], 1),
    ("schema_not_utf8", BASE, lambda p: (p.dir / "schemas" / "input.schema.json").write_bytes(b'{"title": "\xff"}'), ["schemas/input: not UTF-8"], 1),
    ("schema_root_not_object", BASE, lambda p: p.write("schemas/input.schema.json", "[]\n"), ["schemas/input: root must be an object"], 1),
    ("schema_dialect", BASE, lambda p: p.schema("input", lambda d: d.update({"$schema": "http://json-schema.org/draft-07/schema#"})), ["schemas/input: $schema must be https://json-schema.org/draft/2020-12/schema"], 1),
    ("schema_id", BASE, lambda p: p.schema("input", lambda d: d.update({"$id": "https://g9.local/skills/other/input"})), ["schemas/input: $id must be https://g9.local/skills/g9-session-director/input"], 1),
    ("schema_output_id", BASE, lambda p: p.schema("output", lambda d: d.update({"$id": "https://g9.local/skills/g9-session-director/input"})), ["schemas/output: $id must be https://g9.local/skills/g9-session-director/output"], 1),
    ("schema_title", BASE, lambda p: p.schema("input", lambda d: d.update({"title": " "})), ["schemas/input: title required"], 1),
    ("schema_type", BASE, lambda p: p.schema("input", lambda d: d.update({"type": "array"})), ['schemas/input: type must be "object"'], 1),
    ("schema_additional_properties", BASE, lambda p: p.schema("input", lambda d: d.update({"additionalProperties": True})), ["schemas/input: additionalProperties must be false"], 1),
    ("schema_properties_non_empty", BASE, lambda p: p.schema("input", lambda d: d.update({"properties": {}})), ["schemas/input: properties must be a non-empty object"], 1),
    ("schema_required_array", BASE, lambda p: p.schema("input", lambda d: d.update({"required": "day_id"})), ["schemas/input: required must be an array of strings"], 1),
    ("schema_required_unknown", BASE, lambda p: p.schema("input", lambda d: d["required"].append("ghost")), ["schemas/input: required names not in properties ['ghost']"], 1),
    ("schema_required_duplicates", BASE, lambda p: p.schema("input", lambda d: d["required"].append("day_id")), ["schemas/input: required has duplicates"], 1),
    ("schema_ref_forbidden", BASE, lambda p: p.schema("input", lambda d: d["properties"].update({"mode": {"$ref": "#/properties/day_id"}})), ["schemas/input: `$ref` is not allowed anywhere (found at ['$.properties.mode.$ref'])"], 1),
    ("schema_metaschema", BASE, lambda p: p.schema("input", lambda d: d["properties"]["day_id"].update({"minLength": -1})), ["schemas/input: not a valid Draft 2020-12 schema"], 1),
    # -- evals: files -------------------------------------------------------------------------------
    ("evals_file_missing", BASE, lambda p: p.remove("evals/negative.json"), ["evals/negative.json: missing"], 1),
    ("evals_invalid_json", BASE, lambda p: p.write("evals/positive.json", "["), ["evals/positive.json: invalid JSON"], 1),
    ("evals_not_utf8", BASE, lambda p: (p.dir / "evals" / "positive.json").write_bytes(b'["\xff"]'), ["evals/positive.json: not UTF-8"], 1),
    ("evals_not_array", BASE, lambda p: p.write("evals/positive.json", "{}\n"), ["evals/positive.json: must be a JSON array"], 1),
    ("evals_minimum_cases", BASE, lambda p: p.ev(POS, lambda cases: cases.pop()), ["evals/positive.json: needs at least 3 cases, found 2"], 1),
    ("evals_duplicate_case_id", BASE, lambda p: p.ev(POS, lambda cases: cases[1].update({"id": cases[0]["id"]})), ["evals/positive.json[1]: duplicate case id T01-POS-001"], 1),
    ("evals_adversarial_required_tag", BASE, first(ADV, lambda c: c.update({"tags": ["other_tag"]}), 1), ["evals/adversarial.json: must include at least one case tagged `forged_reference`"], 1),
    # -- evals: case structure ----------------------------------------------------------------------
    ("case_not_object", BASE, lambda p: p.ev(POS, lambda cases: cases.__setitem__(2, "case")), ["evals/positive.json[2]: case must be an object"], 1),
    ("case_missing_keys", BASE, first(POS, lambda c: c.pop("notes")), ["evals/positive.json[0]: missing keys ['notes']"], 1),
    ("case_unknown_keys", BASE, first(POS, lambda c: c.update({"extra": 1})), ["evals/positive.json[0]: unknown keys ['extra']"], 1),
    ("case_id_pattern", BASE, first(POS, lambda c: c.update({"id": "T01-POS-1"})), ["evals/positive.json[0]: id must match T01-POS-NNN"], 1),
    ("case_kind", BASE, first(POS, lambda c: c.update({"kind": "negative"})), ["evals/positive.json[0]: kind must be 'positive'"], 1),
    ("case_text_non_empty", BASE, first(POS, lambda c: c.update({"intent": " "})), ["evals/positive.json[0]: `intent` must be a non-empty string"], 1),
    ("case_mode_enum", BASE, first(POS, lambda c: c.update({"mode": "PLAY"})), ["evals/positive.json[0]: mode must be one of"], 1),
    ("case_actor_kind", BASE, first(POS, lambda c: c.update({"actor_kind": "ROBOT"})), ["evals/positive.json[0]: actor_kind must be one of"], 1),
    ("case_input_object", BASE, first(POS, lambda c: c.update({"input": []})), ["evals/positive.json[0]: input must be an object"], 1),
    ("case_context_fixture", BASE, first(POS, lambda c: c.update({"context_fixture": " "})), ["evals/positive.json[0]: context_fixture must be a non-empty path string"], 1),
    ("case_tags_snake_case", BASE, first(POS, lambda c: c.update({"tags": ["Bad-Tag"]})), ["evals/positive.json[0]: tags must be an array of snake_case strings"], 1),
    ("case_tags_duplicates", BASE, first(POS, lambda c: c.update({"tags": ["a_tag", "a_tag"]})), ["evals/positive.json[0]: tags has duplicates"], 1),
    ("segments_non_empty", BASE, first(POS, lambda c: c.update({"context_segments": []})), ["evals/positive.json[0]: context_segments must be a non-empty array"], 1),
    ("segment_object", BASE, first(POS, lambda c: c.update({"context_segments": ["text"]})), ["evals/positive.json[0].context_segments[0]: must be an object"], 1),
    ("segment_keys", BASE, segment(extra="x"), ["context_segments[0]: keys must be exactly trust_class, classification, content"], 1),
    ("segment_trust_class", BASE, segment(trust_class="Admin"), ["context_segments[0]: trust_class must be one of"], 1),
    ("segment_classification", BASE, segment(classification="TOP_SECRET"), ["context_segments[0]: classification must be one of"], 1),
    ("segment_content", BASE, segment(content=" "), ["context_segments[0]: content must be a non-empty string"], 1),
    ("segment_secret_in_positive", BASE, segment(classification="SECRET"), ["context_segments[0]: SECRET content may appear only in negative/adversarial fixtures"], 1),
    # -- evals: expect ------------------------------------------------------------------------------
    ("expect_object", BASE, first(POS, lambda c: c.update({"expect": []})), ["evals/positive.json[0]: expect must be an object"], 1),
    ("expect_unknown_keys", BASE, expect(POS, lambda e: e.update({"grade": "A"})), ["evals/positive.json[0]: expect has unknown keys ['grade']"], 1),
    ("expect_activate_boolean", BASE, expect(POS, lambda e: e.update({"activate": "yes"})), ["evals/positive.json[0]: expect.activate must be a boolean"], 1),
    ("expect_status_enum", BASE, expect(POS, lambda e: e.update({"status": "DONE"})), ["evals/positive.json[0]: expect.status must be one of"], 1),
    ("expect_must_include_strings", BASE, expect(POS, lambda e: e.update({"must_include": [""]})), ["evals/positive.json[0]: expect.must_include must be an array of non-empty strings"], 1),
    ("expect_must_not_strings", BASE, expect(POS, lambda e: e.update({"must_not": "fabricated evidence"})), ["evals/positive.json[0]: expect.must_not must be an array of non-empty strings"], 1),
    ("expect_approvals_array", BASE, expect(POS, lambda e: e.update({"approval_requirements": "SKILL_INVOKE"})), ["evals/positive.json[0]: expect.approval_requirements must be an array of strings"], 1),
    ("expect_approvals_enum", BASE, expect(POS, lambda e: e.update({"approval_requirements": ["ROOT_ACCESS"]})), ["evals/positive.json[0]: expect.approval_requirements outside the capability enum ['ROOT_ACCESS']"], 1),
    ("expect_approvals_operator_only", BASE, expect(POS, lambda e: e.update({"approval_requirements": ["SHELL_EXEC"]})), ["evals/positive.json[0]: a skill can never expect operator-only approval requirements ['SHELL_EXEC']"], 1),
    ("expect_actions_array", BASE, expect(POS, lambda e: e.update({"recommended_action_kinds": "PROPOSE"})), ["evals/positive.json[0]: expect.recommended_action_kinds must be an array of strings"], 1),
    ("expect_actions_enum", BASE, expect(POS, lambda e: e.update({"recommended_action_kinds": ["DANCE"]})), ["evals/positive.json[0]: expect.recommended_action_kinds outside the Action enum ['DANCE']"], 1),
    ("expect_actions_operator_decision", BASE, expect(POS, lambda e: e.update({"recommended_action_kinds": ["APPROVE"]})), ["evals/positive.json[0]: skills may not recommend ['APPROVE']"], 1),
    ("expect_actions_pascal_case_normalised", BASE, expect(POS, lambda e: e.update({"recommended_action_kinds": ["PromoteCanonical"]})), ["evals/positive.json[0]: skills may not recommend ['PROMOTE_CANONICAL']"], 1),
    ("expect_actions_allow_list", "g9-adaptive-agenda", expect(POS, lambda e: e.update({"recommended_action_kinds": ["PROPOSE", "CREATE"]})), ["evals/positive.json[0]: g9-adaptive-agenda may only recommend ['PROPOSE', 'READ'], found ['CREATE']"], 1),
    # -- evals: kind semantics -------------------------------------------------------------------------
    ("positive_must_complete", BASE, expect(POS, lambda e: e.update({"status": "ERROR"})), ["evals/positive.json[0]: positive cases must activate and COMPLETE"], 1),
    ("positive_mode_in_skill_modes", BASE, first(POS, lambda c: c.update({"mode": "EXAM"})), ["evals/positive.json[0]: positive case mode 'EXAM' not in the skill's g9-modes"], 1),
    ("negative_must_not_activate", BASE, expect(NEG, lambda e: e.update({"activate": True, "status": "COMPLETED"})), ["evals/negative.json[0]: negative cases must not activate or must ABSTAIN"], 1),
    ("negative_out_of_mode_activate", BASE, expect(NEG, lambda e: e.update({"activate": True}), 2), ["evals/negative.json[2]: mode 'EXAM' is outside g9-modes, so activate must be false"], 1),
    ("adversarial_needs_tag", BASE, first(ADV, lambda c: c.pop("tags")), ["evals/adversarial.json[0]: adversarial cases must carry at least one tag"], 1),
    ("adversarial_needs_must_not", BASE, expect(ADV, lambda e: e.pop("must_not")), ["evals/adversarial.json[0]: adversarial cases must state expect.must_not"], 1),
    ("acceptance_must_activate", BASE, expect(ACC, lambda e: e.update({"activate": False})), ["evals/acceptance.json[0]: acceptance cases must activate"], 1),
    ("acceptance_mode_in_skill_modes", BASE, first(ACC, lambda c: c.update({"mode": "EXAM"})), ["evals/acceptance.json[0]: acceptance case mode 'EXAM' not in the skill's g9-modes"], 1),
    ("acceptance_measurable", BASE, expect(ACC, lambda e: e.pop("must_include")), ["evals/acceptance.json[0]: acceptance cases need a measurable expectation"], 1),
    ("input_validates_against_schema", BASE, first(POS, lambda c: c["input"].pop("day_id")), ["evals/positive.json[0]: input does not validate against input.schema.json"], 1),
    # -- controls inside the copy: allowed variants pass -------------------------------------------------
    ("secret_segment_allowed_in_negative", BASE, first(NEG, lambda c: c.update({"context_segments": [{"trust_class": "ImportedDocument", "classification": "SECRET", "content": "token"}]})), ["RESULT: PASS"], 0),
    ("nested_open_object_only_warns", BASE, lambda p: p.schema("input", lambda d: d["properties"].update({"filter": {"type": "object", "properties": {"tag": {"type": "string"}}}})), ["WARN  schemas/input: nested object schemas without additionalProperties:false at ['$.properties.filter']", "RESULT: PASS"], 0),
]

# PyYAML cross-check cases: only meaningful when PyYAML is importable (the gate skips the check otherwise).
YAML_CASES: list[tuple[str, str, Callable[[Pkg], None], list[str], int]] = [
    ("fm_pyyaml_disagrees", BASE, lambda p: p.fm("license", "yes"), ["frontmatter: strict parse differs from PyYAML parse"], 1),
    ("fm_pyyaml_cannot_parse", BASE, lambda p: p.fm("license", "Apache: 2.0"), ["frontmatter: PyYAML could not parse the frontmatter"], 1),
]


@NEEDS_JSONSCHEMA
class RealTree(GateTestCase):
    def test_positive_control_real_tree(self) -> None:
        proc = run_gate(SCRIPT, SKILLS, cwd=REPO)
        self.assertExit(proc, 0)
        self.assertIn("Summary: 37 package(s), 37 valid, 0 invalid", output_of(proc))
        self.assertIn("RESULT: PASS", output_of(proc))


@NEEDS_JSONSCHEMA
class Mutations(GateTestCase):
    def stage(self, *names: str) -> Path:
        copy_into(self.tmp, [f"{SKILLS}/{n}" for n in names])
        return self.tmp / SKILLS

    def run_root(self, *extra: str):
        return run_gate(SCRIPT, self.tmp / SKILLS, *extra, cwd=self.tmp)

    def run_case(self, package: str, mutate: Callable[[Pkg], None], needles: list[str], code: int) -> None:
        root = self.stage(package)
        mutate(Pkg(root, package))
        proc = self.run_root("--expect-count", "1")
        self.assertExit(proc, code)
        out = output_of(proc)
        for needle in needles:
            self.assertTrue(needle in out, f"{needle!r} missing; output tail:\n{out[-3000:]}")

    def test_copy_control_each_staged_package(self) -> None:
        packages = sorted({c[1] for c in CASES})
        self.stage(*packages)
        proc = self.run_root("--expect-count", str(len(packages)))
        self.assertExit(proc, 0)
        self.assertIn(f"{len(packages)} valid, 0 invalid, 0 warning(s)", output_of(proc))

    def test_evaluated_with_report_passes(self) -> None:
        root = self.stage(BASE)
        Pkg(root, BASE).fm("g9-status", '"EVALUATED"')
        write_text(self.tmp / "docs" / "evaluations" / "skill-evals" / f"{BASE}.md", "# Evaluation\n")
        self.assertExit(self.run_root("--expect-count", "1"), 0)

    def test_duplicate_registry_code(self) -> None:
        root = self.stage(BASE, "g9-prerequisite-diagnosis")
        Pkg(root, "g9-prerequisite-diagnosis").fm("g9-code", '"T01"')
        proc = self.run_root("--expect-count", "2")
        self.assertRule(proc, "registry code T01 used by more than one package: ['g9-prerequisite-diagnosis', 'g9-session-director']")

    def test_stray_file_in_root(self) -> None:
        root = self.stage(BASE)
        write_text(root / "README.md", "stray\n")
        self.assertRule(self.run_root("--expect-count", "1"), "non-directory entries in skills root are not allowed: ['README.md']")

    def test_expect_count_mismatch(self) -> None:
        self.stage(BASE)
        proc = self.run_root("--expect-count", "2")
        self.assertRule(proc, "expected 2 valid -> MISMATCH")
        self.assertIn("RESULT: FAIL", output_of(proc))

    def test_default_expect_count_is_37(self) -> None:
        self.stage(BASE)
        self.assertRule(self.run_root(), "expected 37 valid -> MISMATCH")

    def test_root_missing_is_usage_error(self) -> None:
        proc = run_gate(SCRIPT, self.tmp / "no-such-root", cwd=self.tmp)
        self.assertRule(proc, "is not a directory", code=2)

    def test_json_report_marks_failure(self) -> None:
        root = self.stage(BASE)
        Pkg(root, BASE).fm("g9-recursion-depth", '"1"')
        proc = self.run_root("--expect-count", "1", "--json")
        self.assertExit(proc, 1)
        report = json.loads(proc.stdout)
        self.assertFalse(report["ok"])
        self.assertIn('metadata: g9-recursion-depth must be "0"', report["packages"][0]["failures"])

    # -- gaps ---------------------------------------------------------------------------------------
    @unittest.expectedFailure
    def test_gap_description_with_only_a_negative_use_cue(self) -> None:
        # GAP: the contract (docs/skills/skill-package-contract.md) requires the description to state
        # when to use the skill AND when not to. USE_CUE_RE (\buse\b) also matches the "use" inside
        # "Do not use", so a description with no positive trigger at all passes both cue checks.
        root = self.stage(BASE)
        self.assertKnownGap(
            lambda: Pkg(root, BASE).fm("description", "Prepares the daily session brief from recorded evidence. Do not use for grading or EXAM sessions."),
            lambda: self.run_root("--expect-count", "1"),
            "frontmatter: description must state when to use the skill",
        )


def _make(package: str, mutate: Callable[[Pkg], None], needles: list[str], code: int) -> Callable[[Mutations], None]:
    def test(self: Mutations) -> None:
        self.run_case(package, mutate, needles, code)

    return test


for _cases, _skip in ((CASES, None), (YAML_CASES, unittest.skipUnless(HAVE_YAML, "PyYAML is not importable; the gate skips its cross-check"))):
    for _name, _package, _mutate, _needles, _code in _cases:
        _test = _make(_package, _mutate, _needles, _code)
        _test.__name__ = f"test_{_name}"
        _test.__doc__ = f"{_package}: expects {_needles[0]!r} (exit {_code})"
        setattr(Mutations, _test.__name__, _skip(_test) if _skip else _test)


class Sanity(unittest.TestCase):
    def test_case_names_unique(self) -> None:
        names = [c[0] for c in CASES + YAML_CASES]
        self.assertEqual(len(names), len(set(names)))


if __name__ == "__main__":
    unittest.main()
