"""Black-box tests for scripts/check_competency_packs.py (step CLI-2).

Rules enumerated from the script (load_pack, check_pack, check_graph, main):
  load        pack.json present, UTF-8, valid JSON, valid against schemas/competency-pack.schema.json
  pack_id     equals the directory name; unique across packs
  ids         competency, misconception (per competency), assessment and lab fixture ids unique in a pack
  band        difficulty_band.min <= max; every assessment difficulty inside the band
  rubric      BLIND_PRACTICAL and TRANSFER_CHECK assessments carry >= 1 rubric_focus
  pack        >= 1 BLIND_PRACTICAL assessment and >= 1 nice_refs entry per pack
  self        no competency lists itself (bare or pack-qualified) as a prerequisite
  approved    an APPROVED pack has only VERIFIED sources and VERIFIED nice_refs
  resolve     `pack:comp` names a known pack and a competency in it; a bare id exists in the pack
  cycle       no prerequisite cycle across the whole graph
  root        no non-directory entries in the packs root; --expect-count valid packs (default 8)
  usage       root is a directory and the schema loads (exit 2)

Negative cases copy data/competency-packs and the schema into a temporary directory and mutate the
`linux-python-systems` pack (PACK) of the copy.
"""

from __future__ import annotations

import copy
import shutil
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
    run_gate,
    write_text,
)

SCRIPT = SCRIPTS / "check_competency_packs.py"
PACKS = "data/competency-packs"
SCHEMA = "schemas/competency-pack.schema.json"
PACK = "linux-python-systems"
INPUTS = [PACKS, SCHEMA]

_GUARD = InputGuard(INPUTS)


def setUpModule() -> None:
    _GUARD.snapshot()


def tearDownModule() -> None:
    _GUARD.verify()


@NEEDS_JSONSCHEMA
class RealTree(GateTestCase):
    def test_positive_control_real_tree(self) -> None:
        proc = run_gate(SCRIPT, PACKS, cwd=REPO)
        self.assertExit(proc, 0)
        self.assertIn("RESULT: PASS", output_of(proc))
        self.assertIn("8 pack(s), 8 valid, 0 invalid; expected 8 valid -> OK", output_of(proc))


@NEEDS_JSONSCHEMA
class Mutations(GateTestCase):
    def setUp(self) -> None:
        super().setUp()
        copy_into(self.tmp, INPUTS)
        self.root = self.tmp / PACKS
        self.pack_json = self.root / PACK / "pack.json"

    def run_copy(self, *extra: str, root: str = PACKS):
        return run_gate(SCRIPT, root, "--schema", self.tmp / SCHEMA, *extra, cwd=self.tmp)

    def edit(self, fn) -> None:
        edit_json(self.pack_json, fn)

    def test_copy_control_passes(self) -> None:
        proc = self.run_copy()
        self.assertExit(proc, 0)
        self.assertIn("RESULT: PASS", output_of(proc))

    # -- load -------------------------------------------------------------------------
    def test_pack_json_missing(self) -> None:
        self.pack_json.unlink()
        self.assertRule(self.run_copy(), "pack.json missing")

    def test_pack_json_invalid(self) -> None:
        write_text(self.pack_json, '{"pack_id": ')
        self.assertRule(self.run_copy(), "pack.json invalid JSON")

    def test_pack_json_not_utf8(self) -> None:
        self.pack_json.write_bytes(b'{"pack_id": "\xff"}')
        self.assertRule(self.run_copy(), "pack.json is not UTF-8")

    def test_schema_violation(self) -> None:
        self.edit(lambda d: d.update({"unexpected": True}))
        self.assertRule(self.run_copy(), "schema: $: Additional properties are not allowed")

    # -- identity ---------------------------------------------------------------------
    def test_pack_id_must_match_directory(self) -> None:
        self.edit(lambda d: d.update({"pack_id": "linux-python-system"}))
        self.assertRule(self.run_copy(), "pack_id 'linux-python-system' does not match directory 'linux-python-systems'")

    def test_duplicate_pack_id(self) -> None:
        shutil.copytree(self.root / PACK, self.root / "zz-second-copy")
        self.assertRule(self.run_copy(), "duplicate pack_id 'linux-python-systems'")

    def test_duplicate_competency_id(self) -> None:
        def fn(d: dict) -> None:
            dup = copy.deepcopy(d["competencies"][0])
            for a in dup["assessments"]:
                a["id"] += "-copy"
            for f in dup["lab_fixtures"]:
                f["id"] += "-copy"
            d["competencies"].append(dup)

        self.edit(fn)
        self.assertRule(self.run_copy(), "duplicate competency ids ['lab.authorization']")

    def test_duplicate_misconception_id(self) -> None:
        def fn(d: dict) -> None:
            ms = d["competencies"][0]["misconceptions"]
            ms[1]["id"] = ms[0]["id"]

        self.edit(fn)
        self.assertRule(self.run_copy(), "lab.authorization: duplicate misconception ids")

    def test_duplicate_assessment_id(self) -> None:
        def fn(d: dict) -> None:
            d["competencies"][1]["assessments"][2]["id"] = d["competencies"][0]["assessments"][2]["id"]

        self.edit(fn)
        self.assertRule(self.run_copy(), "duplicate assessment ids ['lab.authorization.teachback']")

    def test_duplicate_lab_fixture_id(self) -> None:
        def fn(d: dict) -> None:
            d["competencies"][1]["lab_fixtures"][0]["id"] = d["competencies"][0]["lab_fixtures"][0]["id"]

        self.edit(fn)
        self.assertRule(self.run_copy(), "duplicate lab fixture ids ['lab.authorization.lab1']")

    # -- difficulty and rubric --------------------------------------------------------
    def test_band_min_greater_than_max(self) -> None:
        self.edit(lambda d: d["competencies"][0].update({"difficulty_band": {"min": 2, "max": 1}}))
        self.assertRule(self.run_copy(), "lab.authorization: difficulty_band.min > max")

    def test_assessment_difficulty_outside_band(self) -> None:
        self.edit(lambda d: d["competencies"][0]["assessments"][0].update({"difficulty": 3}))
        self.assertRule(self.run_copy(), "assessment lab.authorization.blind difficulty 3 outside band [1,2]")

    def test_blind_practical_needs_rubric_focus(self) -> None:
        self.edit(lambda d: d["competencies"][0]["assessments"][0].update({"rubric_focus": []}))
        self.assertRule(self.run_copy(), "assessment lab.authorization.blind (BLIND_PRACTICAL) needs >= 1 rubric_focus")

    def test_transfer_check_needs_rubric_focus(self) -> None:
        self.edit(lambda d: d["competencies"][0]["assessments"][1].update({"rubric_focus": []}))
        self.assertRule(self.run_copy(), "assessment lab.authorization.transfer (TRANSFER_CHECK) needs >= 1 rubric_focus")

    # -- pack-level requirements ------------------------------------------------------
    def test_pack_needs_blind_practical(self) -> None:
        def fn(d: dict) -> None:
            for c in d["competencies"]:
                for a in c["assessments"]:
                    if a["kind"] == "BLIND_PRACTICAL":
                        a["kind"] = "RECALL_ITEM"

        self.edit(fn)
        self.assertRule(self.run_copy(), "pack needs at least one BLIND_PRACTICAL assessment")

    def test_pack_needs_nice_refs(self) -> None:
        def fn(d: dict) -> None:
            for c in d["competencies"]:
                c["nice_refs"] = []

        self.edit(fn)
        self.assertRule(self.run_copy(), "pack needs at least one nice_refs entry")

    def test_approved_pack_with_assumed_sources(self) -> None:
        def fn(d: dict) -> None:
            d["status"] = "APPROVED"
            for c in d["competencies"]:
                for n in c["nice_refs"]:
                    n["confidence"] = "VERIFIED"

        self.edit(fn)
        self.assertRule(self.run_copy(), "APPROVED pack has ASSUMED sources")

    def test_approved_pack_with_assumed_nice_refs(self) -> None:
        def fn(d: dict) -> None:
            d["status"] = "APPROVED"
            for s in d["sources"]:
                s["verification"] = "VERIFIED"

        self.edit(fn)
        self.assertRule(self.run_copy(), "APPROVED pack has ASSUMED nice_refs")

    # -- prerequisites ----------------------------------------------------------------
    # The self-reference and cycle cases use linux.blind-troubleshooting (index 7), which no other
    # competency depends on: the cycle detector reports cleanly only when no later node reaches the
    # cycle (see the known-defect tests below).
    def test_self_prerequisite_bare(self) -> None:
        self.edit(lambda d: d["competencies"][7]["prerequisites"].append("linux.blind-troubleshooting"))
        self.assertRule(self.run_copy(), "linux.blind-troubleshooting: lists itself as a prerequisite")

    def test_self_prerequisite_qualified(self) -> None:
        self.edit(lambda d: d["competencies"][7]["prerequisites"].append("linux-python-systems:linux.blind-troubleshooting"))
        self.assertRule(self.run_copy(), "linux.blind-troubleshooting: lists itself (qualified) as a prerequisite")

    def test_prerequisite_unknown_pack(self) -> None:
        self.edit(lambda d: d["competencies"][0].update({"prerequisites": ["no-such-pack:lab.x"]}))
        self.assertRule(self.run_copy(), "prerequisite 'no-such-pack:lab.x' names unknown pack 'no-such-pack'")

    def test_prerequisite_missing_in_other_pack(self) -> None:
        self.edit(lambda d: d["competencies"][0].update({"prerequisites": ["web-api-appsec:web.no-such"]}))
        self.assertRule(self.run_copy(), "prerequisite 'web-api-appsec:web.no-such' not found in pack 'web-api-appsec'")

    def test_prerequisite_missing_in_this_pack(self) -> None:
        self.edit(lambda d: d["competencies"][0].update({"prerequisites": ["linux.no-such"]}))
        self.assertRule(self.run_copy(), "prerequisite 'linux.no-such' not found in this pack")

    def test_prerequisite_cycle(self) -> None:
        # python.testing-fixtures (index 4) <-> linux.blind-troubleshooting (index 7)
        self.edit(lambda d: d["competencies"][4]["prerequisites"].append("linux.blind-troubleshooting"))
        self.assertRule(
            self.run_copy(),
            "prerequisite cycle: linux-python-systems:linux.blind-troubleshooting -> linux-python-systems:python.testing-fixtures"
            " -> linux-python-systems:linux.blind-troubleshooting",
        )

    # -- known defects ------------------------------------------------------------------
    @unittest.expectedFailure
    def test_defect_cycle_reached_from_later_node_crashes(self) -> None:
        # DEFECT: check_graph() clears `stack` after the first cycle but leaves its nodes GREY; the next
        # DFS that reaches one of them calls stack.index() on a node no longer on the stack and dies
        # with ValueError (exit 1, traceback, no FAIL lines). Mutation: lab.authorization gains the
        # prerequisite linux.blind-troubleshooting, closing a cycle that many competencies depend on.
        self.assertKnownGap(
            lambda: self.edit(lambda d: d["competencies"][0].update({"prerequisites": ["linux.blind-troubleshooting"]})),
            self.run_copy,
            "prerequisite cycle: ",
            kind="crash",
        )

    @unittest.expectedFailure
    def test_defect_self_prerequisite_with_dependents_crashes(self) -> None:
        # DEFECT: same root cause. linux.filesystem lists itself; linux.processes-services depends on it,
        # so the report ("lists itself as a prerequisite") is never printed.
        self.assertKnownGap(
            lambda: self.edit(lambda d: d["competencies"][1]["prerequisites"].append("linux.filesystem")),
            self.run_copy,
            "linux.filesystem: lists itself as a prerequisite",
            kind="crash",
        )

    # -- root -------------------------------------------------------------------------
    def test_stray_file_in_root(self) -> None:
        write_text(self.root / "README.md", "stray\n")
        self.assertRule(self.run_copy(), "non-directory entries in packs root are not allowed: ['README.md']")

    def test_expect_count_mismatch(self) -> None:
        proc = self.run_copy("--expect-count", "9")
        self.assertRule(proc, "expected 9 valid -> MISMATCH")
        self.assertIn("RESULT: FAIL", output_of(proc))

    def test_root_not_a_directory(self) -> None:
        self.assertRule(self.run_copy(root="no-such-dir"), "is not a directory", code=2)

    def test_schema_not_loadable(self) -> None:
        proc = run_gate(SCRIPT, PACKS, "--schema", self.tmp / "missing.schema.json", cwd=self.tmp)
        self.assertRule(proc, "cannot load schema", code=2)


if __name__ == "__main__":
    unittest.main()
