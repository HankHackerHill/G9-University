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
  cycle       no prerequisite cycle across the whole graph; every elementary cycle is reported
              (at most 100, then a line saying that more exist), without a traceback
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


def _cycle(*ids: str) -> str:
    """The FAIL text for a prerequisite cycle through competencies of PACK, in the order given."""
    return "prerequisite cycle: " + " -> ".join(f"{PACK}:{i}" for i in ids)


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
    # The self-reference and cycle cases below use linux.blind-troubleshooting (index 7), which no other
    # competency depends on; the cases after them close cycles that later nodes reach again.
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

    # Cycles that a later depth-first search reaches again. Before the fix, check_graph() cleared its
    # stack after the first cycle but left those nodes GREY; the next search that reached one called
    # stack.index() on a node no longer on the stack and died with ValueError (exit 1, a traceback and
    # no FAIL line at all, so every other failure of the run went unreported too).
    def test_cycle_reached_from_later_nodes_reports_every_cycle(self) -> None:
        # lab.authorization (index 0) gains linux.blind-troubleshooting. Two elementary cycles close
        # through that edge (via linux.hardening-baseline and via python.testing-fixtures), and most of
        # the pack depends on lab.authorization. A stray file adds a failure reported after the graph.
        self.edit(lambda d: d["competencies"][0].update({"prerequisites": ["linux.blind-troubleshooting"]}))
        write_text(self.root / "README.md", "stray\n")
        proc = self.run_copy()
        self.assertRule(
            proc,
            _cycle("lab.authorization", "linux.blind-troubleshooting", "linux.hardening-baseline", "linux.evidence-completeness",
                   "linux.processes-services", "linux.filesystem", "lab.authorization"),
        )
        out = output_of(proc)
        self.assertNotIn("Traceback", out)
        self.assertIn(
            _cycle("lab.authorization", "linux.blind-troubleshooting", "python.testing-fixtures", "python.secure-parsing",
                   "linux.processes-services", "linux.filesystem", "lab.authorization"),
            out,
        )
        self.assertEqual(out.count("FAIL  prerequisite cycle: "), 2)
        self.assertIn("non-directory entries in packs root are not allowed: ['README.md']", out)
        self.assertIn("RESULT: FAIL", out)

    def test_self_prerequisite_with_dependents_is_reported(self) -> None:
        # linux.filesystem (index 1) lists itself; linux.processes-services and everything after it
        # depend on it. Both the pack rule and the one-node cycle are reported.
        self.edit(lambda d: d["competencies"][1]["prerequisites"].append("linux.filesystem"))
        proc = self.run_copy()
        self.assertRule(proc, "linux.filesystem: lists itself as a prerequisite")
        out = output_of(proc)
        self.assertNotIn("Traceback", out)
        self.assertIn(_cycle("linux.filesystem", "linux.filesystem"), out)
        self.assertEqual(out.count("FAIL  prerequisite cycle: "), 1)

    def test_cycles_in_two_packs_are_both_reported(self) -> None:
        # One cycle per pack, the second in a pack that depends on the first (networking ->
        # linux-python-systems:lab.authorization): each is reported once.
        self.edit(lambda d: d["competencies"][0].update({"prerequisites": ["linux.filesystem"]}))
        edit_json(
            self.root / "networking-protocols-recon" / "pack.json",
            lambda d: d["competencies"][0].update({"prerequisites": ["net.tcp-states"]}),
        )
        proc = self.run_copy()
        self.assertRule(proc, _cycle("lab.authorization", "linux.filesystem", "lab.authorization"))
        out = output_of(proc)
        self.assertIn(
            "prerequisite cycle: networking-protocols-recon:net.layers-addressing -> networking-protocols-recon:net.tcp-states"
            " -> networking-protocols-recon:net.layers-addressing",
            out,
        )
        self.assertEqual(out.count("FAIL  prerequisite cycle: "), 2)
        self.assertNotIn("Traceback", out)

    def test_cycle_report_is_bounded(self) -> None:
        # Every competency of the pack requires every other one: 16064 elementary cycles. The gate
        # lists the first 100 and says that more exist, instead of enumerating all of them.
        def fn(d: dict) -> None:
            ids = [c["id"] for c in d["competencies"]]
            for c in d["competencies"]:
                c["prerequisites"] = [i for i in ids if i != c["id"]]

        self.edit(fn)
        proc = self.run_copy()
        self.assertRule(proc, "prerequisite cycles: more than 100; only the first 100 are listed")
        out = output_of(proc)
        self.assertEqual(out.count("FAIL  prerequisite cycle: "), 100)
        self.assertNotIn("Traceback", out)

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
