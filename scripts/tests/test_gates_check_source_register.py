"""Black-box tests for scripts/check_source_register.py (step CLI-2).

Rules enumerated from the script (validate() and main()):
  schema         register validates against schemas/sources/reference-source-register.schema.json
  duplicate id   entry ids are unique
  host           `host` equals the host of `url`
  license_url    `license_url` is https
  entity         no HTML entity in the text fields
  approval       every entry is NOT_APPROVED
  verified-date  VERIFIED needs a clean ISO date in `checked_on`
  verified-url   VERIFIED needs evidence that names a fetched URL
  clean-date     PARTIAL/UNVERIFIED must not carry a clean fetch date
  status         register status stays PROPOSED_UNVERIFIED
  stale          docs/research/reference-source-register.md equals the rendering of the register
  usage          any argument other than --write-markdown is a usage error (exit 2)

The script resolves its root from its own location, so each test runs a byte-for-byte copy of it at
<tmp>/scripts/ next to copies of the register, its schema and the generated Markdown.
"""

from __future__ import annotations

import unittest

from _gate_helpers import (
    NEEDS_JSONSCHEMA,
    REPO,
    SCRIPTS,
    GateTestCase,
    InputGuard,
    copy_into,
    edit_json,
    output_of,
    read_text,
    run_gate,
    stage_script,
    write_text,
)

SCRIPT = "check_source_register.py"
REGISTER = "data/sources/reference-source-register.v0.json"
SCHEMA = "schemas/sources/reference-source-register.schema.json"
MARKDOWN = "docs/research/reference-source-register.md"
INPUTS = [REGISTER, SCHEMA, MARKDOWN]

_GUARD = InputGuard(INPUTS)


def setUpModule() -> None:
    _GUARD.snapshot()


def tearDownModule() -> None:
    _GUARD.verify()


@NEEDS_JSONSCHEMA
class RealTree(GateTestCase):
    def test_positive_control_real_tree(self) -> None:
        proc = run_gate(SCRIPTS / SCRIPT, cwd=REPO)
        self.assertExit(proc, 0)
        self.assertIn("source register: OK", output_of(proc))


@NEEDS_JSONSCHEMA
class Mutations(GateTestCase):
    def setUp(self) -> None:
        super().setUp()
        copy_into(self.tmp, INPUTS)
        self.script = stage_script(self.tmp, SCRIPT)
        self.register = self.tmp / REGISTER

    def run_copy(self, *args: str):
        return run_gate(self.script, *args, cwd=self.tmp)

    def edit_entry(self, index: int, **changes: object) -> None:
        def fn(reg: dict) -> None:
            reg["entries"][index].update(changes)

        edit_json(self.register, fn)

    # -- control on the unmutated copy ------------------------------------------------
    def test_copy_control_passes(self) -> None:
        proc = self.run_copy()
        self.assertExit(proc, 0)
        self.assertIn("source register: OK", output_of(proc))

    # -- one mutation per rule ------------------------------------------------------
    def test_schema_rejects_unknown_entry_field(self) -> None:
        self.edit_entry(0, unexpected_field="x")
        self.assertRule(self.run_copy(), "schema: entries/0: Additional properties are not allowed")

    def test_duplicate_id(self) -> None:
        def fn(reg: dict) -> None:
            reg["entries"][1]["id"] = reg["entries"][0]["id"]

        edit_json(self.register, fn)
        self.assertRule(self.run_copy(), "duplicate id ")

    def test_host_must_equal_url_host(self) -> None:
        self.edit_entry(0, host="example.org")
        self.assertRule(self.run_copy(), "is not the host of url")

    def test_license_url_must_be_https(self) -> None:
        def fn(reg: dict) -> None:
            e = reg["entries"][0]
            e["license_url"] = "http://" + e["license_url"].split("://", 1)[1]

        edit_json(self.register, fn)
        self.assertRule(self.run_copy(), "license_url is not https")

    def test_html_entity_in_text_field(self) -> None:
        def fn(reg: dict) -> None:
            reg["entries"][0]["caveats"] += " Terms &amp; conditions."

        edit_json(self.register, fn)
        self.assertRule(self.run_copy(), "caveats contains an HTML entity")

    def test_approval_state_must_be_not_approved(self) -> None:
        self.edit_entry(0, approval_state="APPROVED")
        self.assertRule(self.run_copy(), "approval_state must be NOT_APPROVED")

    def test_verified_needs_clean_checked_on(self) -> None:
        self.edit_entry(0, verification="VERIFIED", checked_on="2026-11-08 (fetch failed)", evidence="Read https://example.org/terms on the day.")
        self.assertRule(self.run_copy(), "VERIFIED needs checked_on to be the ISO date a page was read")

    def test_verified_needs_evidence_with_url(self) -> None:
        self.edit_entry(0, verification="VERIFIED", checked_on="2026-11-08", evidence="Read the licence page on the day.")
        self.assertRule(self.run_copy(), "VERIFIED needs evidence that names the fetched URL")

    def test_unverified_must_not_carry_clean_date(self) -> None:
        self.edit_entry(0, verification="UNVERIFIED", checked_on="2026-11-08")
        self.assertRule(self.run_copy(), "UNVERIFIED entry carries a clean fetch date")

    def test_partial_must_not_carry_clean_date(self) -> None:
        self.edit_entry(0, verification="PARTIAL", checked_on="2026-11-08")
        self.assertRule(self.run_copy(), "PARTIAL entry carries a clean fetch date")

    def test_register_status_must_stay_proposed_unverified(self) -> None:
        def fn(reg: dict) -> None:
            reg["status"] = "VERIFIED"

        edit_json(self.register, fn)
        self.assertRule(self.run_copy(), "register status must stay PROPOSED_UNVERIFIED")

    def test_stale_markdown(self) -> None:
        md = self.tmp / MARKDOWN
        write_text(md, read_text(md) + "\nhand edit\n")
        self.assertRule(self.run_copy(), "reference-source-register.md is stale")

    def test_write_markdown_repairs_stale_page_in_copy(self) -> None:
        # Writing mode, exercised on the temporary copy only (run_gate refuses it for the repository script).
        md = self.tmp / MARKDOWN
        write_text(md, read_text(md) + "\nhand edit\n")
        self.assertExit(self.run_copy("--write-markdown"), 0)
        self.assertEqual(read_text(md), read_text(REPO / MARKDOWN))
        self.assertExit(self.run_copy(), 0)

    def test_usage_error_on_unknown_argument(self) -> None:
        self.assertRule(self.run_copy("--frobnicate"), "Usage", code=2)

    # -- gaps: mutations that should be rejected and are accepted today ---------------
    # The VERIFIED probes run the temporary copy with --write-markdown, which writes only when
    # validate() found no problem: exit 0 then means "the register rule accepted the entry", not
    # "the stale-page check happened to pass".
    @unittest.expectedFailure
    def test_gap_verified_accepts_impossible_calendar_date(self) -> None:
        # GAP: CLEAN_DATE is the shape ^\d{4}-\d{2}-\d{2}$, not a calendar date, so "2026-13-45" passes
        # as "the ISO date a page was read" (docstring: "VERIFIED requires a clean ISO date").
        self.assertKnownGap(
            lambda: self.edit_entry(0, verification="VERIFIED", checked_on="2026-13-45", evidence="Read https://example.org/terms on the day."),
            lambda: self.run_copy("--write-markdown"),
            "VERIFIED needs checked_on to be the ISO date a page was read",
        )

    @unittest.expectedFailure
    def test_gap_verified_accepts_evidence_without_url(self) -> None:
        # GAP: the evidence rule is the substring test `"http" in evidence`, so evidence that names no
        # URL at all ("the http fetch was never made") satisfies "evidence that names the fetched URL".
        self.assertKnownGap(
            lambda: self.edit_entry(0, verification="VERIFIED", checked_on="2026-11-08", evidence="No page was read; the http fetch was never made."),
            lambda: self.run_copy("--write-markdown"),
            "VERIFIED needs evidence that names the fetched URL",
        )

    @unittest.expectedFailure
    def test_gap_missing_generated_markdown_is_accepted(self) -> None:
        # GAP (INFERRED requirement): the stale check runs only `if MARKDOWN.exists()`, so deleting the
        # generated register page passes; build_curriculum.py treats a missing generated file as out of date.
        self.assertKnownGap(lambda: (self.tmp / MARKDOWN).unlink(), self.run_copy, "reference-source-register.md is stale")

if __name__ == "__main__":
    unittest.main()
