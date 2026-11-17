"""Black-box tests for scripts/check_curriculum_seed.py (gate A01, step CLI-2).

The script reports schema errors, error codes (identical to g9_curriculum::validate::codes),
arithmetic violations (g9_domain::CurriculumCounts) and warnings. Every error code it can emit, every
violation code and every warning code has one case below (CASES); each case copies one manifest
(the seed, or the draft for the optional extensions competency_ids/resources) and the manifest schema
into a temporary directory, applies one mutation, runs the gate on the copy and asserts the exit code
and the code string. Mutations that cannot violate a single rule alone (for example a missing day
also breaks the arithmetic counts) assert the rule they target; the extra findings are expected.

Positive controls: the gate passes on data/curriculum/g9-ct9-480.seed.v0.json (the preflight input)
and on data/curriculum/g9-ct9-480.draft.v1.json.
"""

from __future__ import annotations

import json
import unittest
from typing import Any, Callable

from _gate_helpers import (
    NEEDS_JSONSCHEMA,
    REPO,
    SCRIPTS,
    GateTestCase,
    InputGuard,
    copy_into,
    dump_json,
    load_json,
    output_of,
    run_gate,
    write_text,
)

SCRIPT = SCRIPTS / "check_curriculum_seed.py"
SEED = "data/curriculum/g9-ct9-480.seed.v0.json"
DRAFT = "data/curriculum/g9-ct9-480.draft.v1.json"
SCHEMA = "schemas/curriculum/manifest.schema.json"
INPUTS = [SEED, DRAFT, SCHEMA]
HEX = "0123456789abcdef" * 4
GOOD_RECONCILIATION = {"original_checksum": HEX, "diff_checksum": HEX[::-1], "approved_by": "operator-1", "approved_at": 1763251200000}

_GUARD = InputGuard(INPUTS)


def setUpModule() -> None:
    _GUARD.snapshot()


def tearDownModule() -> None:
    _GUARD.verify()


def day(m: dict, w: int = 0, d: int = 0) -> dict:
    return m["weeks"][w]["days"][d]


def obj(m: dict, k: int = 0, w: int = 0, d: int = 0) -> dict:
    return day(m, w, d)["objectives"][k]


def reconciled(rec: Any) -> Callable[[dict], None]:
    def fn(m: dict) -> None:
        m["provenance"] = "RECONCILED_CANDIDATE"
        m["reconciliation"] = rec

    return fn


def swap(lst: list, i: int, j: int) -> None:
    lst[i], lst[j] = lst[j], lst[i]


def swap_block_names(m: dict) -> None:
    b = day(m)["blocks"]
    b[2]["block"], b[3]["block"] = b[3]["block"], b[2]["block"]  # guided_lab <-> independent_lab, both 120 min


# (test name, base manifest, mutation, expected substrings, expected exit code)
CASES: list[tuple[str, str, Callable[[Any], Any], list[str], int]] = [
    # -- schema and file -----------------------------------------------------------------
    ("schema_unknown_top_level_key", SEED, lambda m: m.update({"unexpected": 1}), ["SCHEMA  /: Additional properties are not allowed ('unexpected' was unexpected)"], 1),
    # -- header ----------------------------------------------------------------------------
    ("schema_version_unsupported", SEED, lambda m: m.update({"manifest_schema_version": "1.1.0"}), ["manifest.schema_version.unsupported /manifest_schema_version"], 1),
    ("curriculum_id_invalid", SEED, lambda m: m.update({"curriculum_id": "g9 ct9 480"}), ["manifest.curriculum_id.invalid /curriculum_id"], 1),
    ("name_empty", SEED, lambda m: m.update({"name": "   "}), ["manifest.name.empty /name"], 1),
    ("version_empty", SEED, lambda m: m.update({"version": ""}), ["manifest.version.empty /version"], 1),
    # -- structure and canonical block lists ------------------------------------------------
    ("structure_mismatch", SEED, lambda m: m["structure"].update({"weeks": 11}), ["structure.mismatch /structure/weeks: expected 12, declared 11"], 1),
    ("structure_sunday", SEED, lambda m: m["structure"].update({"sunday": "scheduled"}), ["structure.sunday.not_unscheduled /structure/sunday"], 1),
    ("weekday_blocks_canonical", SEED, lambda m: swap(m["weekday_blocks"], 0, 1), ["blocks.weekday.canonical_mismatch /weekday_blocks"], 1),
    ("weekday_blocks_not_a_list", SEED, lambda m: m.update({"weekday_blocks": {}}), ["/weekday_blocks: expected a list"], 1),
    ("saturday_blocks_canonical", SEED, lambda m: swap(m["saturday_blocks"], 0, 1), ["blocks.saturday.canonical_mismatch /saturday_blocks"], 1),
    ("canonical_block_non_positive", SEED, lambda m: m["weekday_blocks"][0].update({"minutes": 0}), ["duration.non_positive /weekday_blocks/0/minutes"], 1),
    # -- weeks -------------------------------------------------------------------------------
    ("week_count", SEED, lambda m: m["weeks"].pop(), ["week.count /weeks: expected 12 weeks, found 11", "VIOLATION curriculum.weeks: expected 12, actual 11"], 1),
    ("weeks_not_a_list", SEED, lambda m: m.update({"weeks": {}}), ["week.count /weeks: weeks must be a list"], 1),
    ("week_not_object", SEED, lambda m: m["weeks"].__setitem__(0, "week one"), ["week.days.count /weeks/0: week must be an object"], 1),
    ("week_number_sequence", SEED, lambda m: m["weeks"][0].update({"week_number": 3}), ["week.number.out_of_sequence /weeks/0/week_number"], 1),
    ("week_title_empty", SEED, lambda m: m["weeks"][0].update({"title": " "}), ["week.title.empty /weeks/0/title"], 1),
    ("week_days_count", SEED, lambda m: m["weeks"][0]["days"].pop(), ["week.days.count /weeks/0/days: expected 6 days, found 5", "VIOLATION curriculum.days: expected 72, actual 71", "VIOLATION curriculum.saturday_days"], 1),
    # -- days --------------------------------------------------------------------------------
    ("day_not_object", SEED, lambda m: m["weeks"][0]["days"].__setitem__(0, "monday"), ["day.id.mismatch /weeks/0/days/0: day must be an object"], 1),
    ("day_sunday_present", SEED, lambda m: day(m, 0, 5).update({"day_of_week": 7}), ["day.sunday_present /weeks/0/days/5/day_of_week"], 1),
    ("day_of_week_out_of_range", SEED, lambda m: day(m).update({"day_of_week": 0}), ["day.day_of_week.out_of_range /weeks/0/days/0/day_of_week"], 1),
    ("day_of_week_not_increasing", SEED, lambda m: day(m, 0, 1).update({"day_of_week": 1}), ["day.day_of_week.not_increasing /weeks/0/days/1/day_of_week"], 1),
    ("day_id_mismatch", SEED, lambda m: day(m, 0, 1).update({"day_id": "D09"}), ["day.id.mismatch /weeks/0/days/1/day_id: expected `D02`, found `D09`"], 1),
    ("day_id_duplicate", SEED, lambda m: day(m, 0, 1).update({"day_id": "D01"}), ["day.id.duplicate /weeks/0/days/1/day_id"], 1),
    ("planned_minutes_not_integer", SEED, lambda m: day(m).update({"planned_minutes": "420"}), ["duration.non_positive /weeks/0/days/0/planned_minutes: planned_minutes must be an integer"], 1),
    ("planned_minutes_not_positive", SEED, lambda m: day(m).update({"planned_minutes": 0}), ["duration.non_positive /weeks/0/days/0/planned_minutes: planned_minutes 0 is not positive"], 1),
    ("planned_minutes_mismatch", SEED, lambda m: day(m).update({"planned_minutes": 300}), ["day.planned_minutes.mismatch /weeks/0/days/0/planned_minutes", "VIOLATION curriculum.total_minutes"], 1),
    ("assessment_flag", SEED, lambda m: day(m).update({"is_assessment_day": True}), ["day.assessment_flag.mismatch /weeks/0/days/0/is_assessment_day"], 1),
    ("weekday_counted_as_saturday", SEED, lambda m: day(m, 0, 4).update({"day_of_week": 6}), ["VIOLATION curriculum.weekday_days: expected 60, actual 59", "VIOLATION curriculum.saturday_days: expected 12, actual 13"], 1),
    ("evidence_artifact_empty", SEED, lambda m: day(m).update({"evidence_artifact": " "}), ["day.evidence_artifact.empty /weeks/0/days/0/evidence_artifact"], 1),
    # -- objectives --------------------------------------------------------------------------
    ("objectives_count", SEED, lambda m: day(m)["objectives"].pop(), ["day.objectives.count /weeks/0/days/0/objectives", "VIOLATION curriculum.objectives: expected 216, actual 215"], 1),
    ("objective_not_object", SEED, lambda m: day(m)["objectives"].__setitem__(0, "objective"), ["objective.id.mismatch /weeks/0/days/0/objectives/0: objective must be an object"], 1),
    ("objective_seq", SEED, lambda m: obj(m).update({"seq": 2}), ["objective.seq.mismatch /weeks/0/days/0/objectives/0/seq"], 1),
    ("objective_id_mismatch", SEED, lambda m: obj(m).update({"objective_id": "D01-O3"}), ["objective.id.mismatch /weeks/0/days/0/objectives/0/objective_id"], 1),
    ("objective_id_duplicate", SEED, lambda m: obj(m, 1).update({"objective_id": "D01-O1"}), ["objective.id.duplicate /weeks/0/days/0/objectives/1/objective_id"], 1),
    ("objective_text_empty", SEED, lambda m: obj(m).update({"text": "  "}), ["objective.text.empty /weeks/0/days/0/objectives/0/text"], 1),
    ("competency_id_invalid", DRAFT, lambda m: obj(m).update({"competency_ids": ["lab authorization"]}), ["objective.competency_id.invalid /weeks/0/days/0/objectives/0/competency_ids/0"], 1),
    # -- blocks ------------------------------------------------------------------------------
    ("block_not_object", SEED, lambda m: day(m)["blocks"].__setitem__(0, "recall"), ["block.seq.mismatch /weeks/0/days/0/blocks/0: block must be an object"], 1),
    ("block_seq", SEED, lambda m: day(m)["blocks"][0].update({"seq": 6}), ["block.seq.mismatch /weeks/0/days/0/blocks/0/seq"], 1),
    ("block_minutes_non_positive", SEED, lambda m: day(m)["blocks"][0].update({"minutes": 0}), ["duration.non_positive /weeks/0/days/0/blocks/0/minutes"], 1),
    ("blocks_sum_mismatch", SEED, lambda m: day(m)["blocks"][0].update({"minutes": 16}), ["day.blocks.sum_mismatch /weeks/0/days/0/blocks: blocks sum to 421, planned_minutes is 420"], 1),
    ("blocks_canonical", SEED, swap_block_names, ["day.blocks.canonical_mismatch /weeks/0/days/0/blocks"], 1),
    # -- resources (draft only) ------------------------------------------------------------------
    ("resource_not_object", DRAFT, lambda m: day(m)["resources"].__setitem__(0, "https://example.org"), ["resource.ref.empty /weeks/0/days/0/resources/0: resource must be an object"], 1),
    ("resource_ref_empty", DRAFT, lambda m: day(m)["resources"][0].update({"ref": " "}), ["resource.ref.empty /weeks/0/days/0/resources/0/ref"], 1),
    ("resource_version_required", DRAFT, lambda m: day(m)["resources"][0].pop("version"), ["resource.version.required_for_external /weeks/0/days/0/resources/0/version"], 1),
    # -- provenance and reconciliation -----------------------------------------------------------------
    ("provenance_requires_reconciliation", SEED, lambda m: m.update({"provenance": "RECONCILED_CANDIDATE"}), ["provenance.reconciliation_required /provenance"], 1),
    ("reconciliation_not_object", SEED, reconciled("approved"), ["reconciliation.incomplete /reconciliation: reconciliation must be an object"], 1),
    ("reconciliation_checksum", SEED, reconciled(dict(GOOD_RECONCILIATION, original_checksum="ABC")), ["reconciliation.incomplete /reconciliation/original_checksum"], 1),
    ("reconciliation_diff_checksum", SEED, reconciled(dict(GOOD_RECONCILIATION, diff_checksum=HEX.upper())), ["reconciliation.incomplete /reconciliation/diff_checksum"], 1),
    ("reconciliation_approved_by", SEED, reconciled(dict(GOOD_RECONCILIATION, approved_by="the operator")), ["reconciliation.incomplete /reconciliation/approved_by"], 1),
    ("reconciliation_approved_at", SEED, reconciled(dict(GOOD_RECONCILIATION, approved_at=0)), ["reconciliation.incomplete /reconciliation/approved_at"], 1),
    ("proposed_seed_checks_carried_reconciliation", SEED, lambda m: m.update({"reconciliation": dict(GOOD_RECONCILIATION, approved_at=-5)}), ["WARNING provenance.reconciliation_ignored", "reconciliation.incomplete /reconciliation/approved_at"], 1),
    # -- warnings ------------------------------------------------------------------------------
    ("warning_competency_duplicate_is_schema_error", DRAFT, lambda m: obj(m).update({"competency_ids": ["lab.authorization", "lab.authorization"]}), ["WARNING objective.competency_id.duplicate", "has non-unique elements"], 1),
    ("warning_reconciliation_ignored_passes", SEED, lambda m: m.update({"reconciliation": dict(GOOD_RECONCILIATION)}), ["WARNING provenance.reconciliation_ignored /reconciliation", "result: PASS"], 0),
    ("warning_duplicate_objective_text_passes", SEED, lambda m: obj(m, 1).update({"text": obj(m, 0)["text"]}), ["WARNING objective.text.duplicate /weeks/0/days/0/objectives/1/text", "result: PASS"], 0),
    # -- a complete reconciliation is accepted (control for the cases above) ------------------------
    ("complete_reconciliation_passes", SEED, reconciled(dict(GOOD_RECONCILIATION)), ["result: PASS (schema_errors=0 errors=0 violations=0"], 0),
]


@NEEDS_JSONSCHEMA
class RealTree(GateTestCase):
    def test_positive_control_seed(self) -> None:
        proc = run_gate(SCRIPT, SEED, cwd=REPO)
        self.assertExit(proc, 0)
        self.assertIn("== 1 manifest(s) checked, 0 failed", output_of(proc))

    def test_positive_control_draft(self) -> None:
        proc = run_gate(SCRIPT, DRAFT, cwd=REPO)
        self.assertExit(proc, 0)
        self.assertIn("== 1 manifest(s) checked, 0 failed", output_of(proc))


@NEEDS_JSONSCHEMA
class Mutations(GateTestCase):
    def stage(self, base: str) -> tuple:
        copy_into(self.tmp, [base, SCHEMA])
        return self.tmp / base, self.tmp / SCHEMA

    def run_case(self, base: str, mutate: Callable[[Any], Any], needles: list[str], code: int) -> None:
        manifest, schema = self.stage(base)
        doc = load_json(manifest)
        mutate(doc)  # in place; the return value is ignored
        dump_json(manifest, doc)
        proc = run_gate(SCRIPT, manifest, "--schema", schema, cwd=self.tmp)
        self.assertExit(proc, code)
        for needle in needles:
            self.assertTrue(needle in output_of(proc), f"{needle!r} missing; output tail:\n{output_of(proc)[-3000:]}")

    def test_copy_control_seed(self) -> None:
        manifest, schema = self.stage(SEED)
        self.assertExit(run_gate(SCRIPT, manifest, "--schema", schema, cwd=self.tmp), 0)

    def test_copy_control_draft(self) -> None:
        manifest, schema = self.stage(DRAFT)
        self.assertExit(run_gate(SCRIPT, manifest, "--schema", schema, cwd=self.tmp), 0)

    def test_unparsable_json(self) -> None:
        manifest, schema = self.stage(SEED)
        write_text(manifest, '{"weeks": [')
        proc = run_gate(SCRIPT, manifest, "--schema", schema, cwd=self.tmp)
        self.assertRule(proc, "cannot read/parse JSON: JSONDecodeError")

    def test_root_not_object(self) -> None:
        manifest, schema = self.stage(SEED)
        write_text(manifest, "[]\n")
        proc = run_gate(SCRIPT, manifest, "--schema", schema, cwd=self.tmp)
        self.assertRule(proc, "manifest.not_object")

    def test_one_failing_manifest_fails_the_run(self) -> None:
        copy_into(self.tmp, [DRAFT])
        manifest, schema = self.stage(SEED)
        doc = load_json(manifest)
        doc["name"] = ""
        dump_json(manifest, doc)
        proc = run_gate(SCRIPT, self.tmp / DRAFT, manifest, "--schema", schema, cwd=self.tmp)
        self.assertRule(proc, "== 2 manifest(s) checked, 1 failed")

    def test_json_report_marks_failure(self) -> None:
        manifest, schema = self.stage(SEED)
        doc = load_json(manifest)
        doc["structure"]["sunday"] = "scheduled"
        dump_json(manifest, doc)
        proc = run_gate(SCRIPT, manifest, "--schema", schema, "--json", cwd=self.tmp)
        self.assertExit(proc, 1)
        report = json.loads(proc.stdout)
        self.assertFalse(report[0]["ok"])
        self.assertIn("structure.sunday.not_unscheduled", [e["code"] for e in report[0]["errors"]])

    def test_missing_schema_is_usage_error(self) -> None:
        manifest, _ = self.stage(SEED)
        proc = run_gate(SCRIPT, manifest, "--schema", self.tmp / "missing.schema.json", cwd=self.tmp)
        self.assertRule(proc, "error: cannot load schema", code=2)

    def test_invalid_schema_is_usage_error(self) -> None:
        manifest, schema = self.stage(SEED)
        write_text(schema, '{"type": 5}\n')
        proc = run_gate(SCRIPT, manifest, "--schema", schema, cwd=self.tmp)
        self.assertRule(proc, "error: schema is not a valid draft 2020-12 schema", code=2)


def _make(base: str, mutate: Callable[[Any], Any], needles: list[str], code: int) -> Callable[[Mutations], None]:
    def test(self: Mutations) -> None:
        self.run_case(base, mutate, needles, code)

    return test


for _name, _base, _mutate, _needles, _code in CASES:
    _test = _make(_base, _mutate, _needles, _code)
    _test.__name__ = f"test_{_name}"
    _test.__doc__ = f"{_base.rsplit('/', 1)[-1]}: expects {_needles[0]!r} (exit {_code})"
    setattr(Mutations, _test.__name__, _test)


class Sanity(unittest.TestCase):
    def test_case_names_unique(self) -> None:
        names = [c[0] for c in CASES]
        self.assertEqual(len(names), len(set(names)))

    def test_every_script_code_has_a_case(self) -> None:
        # Every error and warning code string defined in the script appears in at least one expectation.
        text = (SCRIPTS / "check_curriculum_seed.py").read_text(encoding="utf-8")
        start = text.index("C = {")
        block = text[start : text.index("}", start)]
        codes = [line.split(":", 1)[1].strip().strip('",') for line in block.splitlines()[1:] if ":" in line]
        # A change in how the table is written must not turn this into a check of nothing.
        self.assertGreaterEqual(len(codes), 37, f"the code table of check_curriculum_seed.py was not read: {codes}")
        expected = " ".join(n for c in CASES for n in c[3]) + " manifest.not_object"  # test_root_not_object
        missing = [c for c in codes if c not in expected]
        self.assertEqual(missing, [], "error codes of check_curriculum_seed.py without a negative case")


if __name__ == "__main__":
    unittest.main()
