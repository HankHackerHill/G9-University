"""Black-box tests for scripts/check_schemas.py (step CLI-2).

Rules enumerated from the script (check_schema(), main()):
  json         parses as JSON; UTF-8; root is an object
  $schema      exactly https://json-schema.org/draft/2020-12/schema
  $id          present, an absolute URI, without a fragment; unique across all scanned files
  title        non-empty string
  type         "object"
  additional   additionalProperties: false at the root
  properties   an object
  required     an explicit array of strings, every name in properties, no duplicates
  $ref         a string; no external target; `#/pointer` resolves; `#name` names a known $anchor or
               $dynamicAnchor; the same rules apply to $dynamicRef
  metaschema   valid against the Draft 2020-12 metaschema
  empty        at least one *.schema.json under the roots
  usage        every root exists (exit 2)
  warnings     $id outside https://g9.local/ and open nested objects warn without failing

Every rule is per file except the duplicate-$id and empty-root rules, so negative cases run on a copy
of two schema directories (COPIED, 3 files, including the target) instead of all 189 files, which keeps
one run near 0.1 s. Each mutates one copied schema (schemas/sources/reference-source-register.schema.json,
which carries an internal $ref). The real-tree control runs the preflight command
`check_schemas.py schemas .agents/skills` read-only.
"""

from __future__ import annotations

import shutil
import unittest
from pathlib import Path

from _gate_helpers import (
    NEEDS_JSONSCHEMA,
    REPO,
    SCRIPTS,
    GateTestCase,
    InputGuard,
    copy_into,
    edit_json,
    load_json,
    output_of,
    run_gate,
    write_text,
)

SCRIPT = SCRIPTS / "check_schemas.py"
TARGET = "schemas/sources/reference-source-register.schema.json"
INPUTS = ["schemas", ".agents/skills"]
COPIED = ["schemas/sources", "schemas/curriculum"]

_GUARD = InputGuard(INPUTS)


def setUpModule() -> None:
    _GUARD.snapshot()


def tearDownModule() -> None:
    _GUARD.verify()


@NEEDS_JSONSCHEMA
class RealTree(GateTestCase):
    def test_positive_control_real_tree(self) -> None:
        proc = run_gate(SCRIPT, "schemas", ".agents/skills", cwd=REPO)
        self.assertExit(proc, 0)
        self.assertIn("RESULT: PASS", output_of(proc))


@NEEDS_JSONSCHEMA
class Mutations(GateTestCase):
    def setUp(self) -> None:
        super().setUp()
        copy_into(self.tmp, COPIED)
        self.target = self.tmp / TARGET

    def run_copy(self, *roots: str):
        return run_gate(SCRIPT, *(roots or ("schemas",)), cwd=self.tmp)

    def edit(self, fn) -> None:
        edit_json(self.target, fn)

    def test_copy_control_passes(self) -> None:
        proc = self.run_copy()
        self.assertExit(proc, 0)
        self.assertIn("RESULT: PASS", output_of(proc))

    # -- parse ---------------------------------------------------------------------
    def test_invalid_json(self) -> None:
        write_text(self.target, '{"$schema": ')
        self.assertRule(self.run_copy(), "invalid JSON")

    def test_not_utf8(self) -> None:
        self.target.write_bytes(b'{"title": "\xff\xfe"}')
        self.assertRule(self.run_copy(), "not UTF-8")

    def test_root_must_be_object(self) -> None:
        write_text(self.target, "[]\n")
        self.assertRule(self.run_copy(), "root must be an object")

    # -- header --------------------------------------------------------------------
    def test_schema_dialect(self) -> None:
        self.edit(lambda d: d.update({"$schema": "http://json-schema.org/draft-07/schema#"}))
        self.assertRule(self.run_copy(), "$schema must be https://json-schema.org/draft/2020-12/schema")

    def test_id_missing(self) -> None:
        self.edit(lambda d: d.pop("$id"))
        self.assertRule(self.run_copy(), "$id must be present and an absolute URI")

    def test_id_not_absolute(self) -> None:
        self.edit(lambda d: d.update({"$id": "schemas/sources/register"}))
        self.assertRule(self.run_copy(), "$id must be present and an absolute URI")

    def test_id_with_fragment(self) -> None:
        self.edit(lambda d: d.update({"$id": d["$id"] + "#root"}))
        self.assertRule(self.run_copy(), "$id must not contain a fragment")

    def test_duplicate_id_across_files(self) -> None:
        shutil.copy2(self.target, self.target.with_name("copy-of-register.schema.json"))
        self.assertRule(self.run_copy(), "duplicate $id https://g9.local/schemas/sources/reference-source-register")

    def test_title_empty(self) -> None:
        self.edit(lambda d: d.update({"title": "  "}))
        self.assertRule(self.run_copy(), "title must be a non-empty string")

    def test_type_object(self) -> None:
        self.edit(lambda d: d.update({"type": "array"}))
        self.assertRule(self.run_copy(), 'type must be "object"')

    def test_additional_properties_false(self) -> None:
        self.edit(lambda d: d.update({"additionalProperties": True}))
        self.assertRule(self.run_copy(), "additionalProperties must be false at the root")

    def test_properties_object(self) -> None:
        self.edit(lambda d: d.update({"properties": ["schema_version"]}))
        self.assertRule(self.run_copy(), "properties must be an object")

    def test_required_array_of_strings(self) -> None:
        self.edit(lambda d: d.update({"required": "schema_version"}))
        self.assertRule(self.run_copy(), "required must be an explicit array of strings")

    def test_required_missing(self) -> None:
        self.edit(lambda d: d.pop("required"))
        self.assertRule(self.run_copy(), "required must be an explicit array of strings")

    def test_required_name_not_in_properties(self) -> None:
        self.edit(lambda d: d["required"].append("ghost_field"))
        self.assertRule(self.run_copy(), "required names not in properties: ['ghost_field']")

    def test_required_duplicates(self) -> None:
        self.edit(lambda d: d["required"].append(d["required"][0]))
        self.assertRule(self.run_copy(), "required has duplicates")

    # -- $ref ------------------------------------------------------------------------
    def test_ref_must_be_string(self) -> None:
        self.edit(lambda d: d["properties"]["entries"]["items"].update({"$ref": 5}))
        self.assertRule(self.run_copy(), "$ref must be a string")

    def test_external_ref(self) -> None:
        self.edit(lambda d: d["properties"]["entries"]["items"].update({"$ref": "https://g9.local/schemas/other#/$defs/entry"}))
        self.assertRule(self.run_copy(), "external $ref 'https://g9.local/schemas/other#/$defs/entry' is not allowed")

    def test_relative_file_ref(self) -> None:
        self.edit(lambda d: d["properties"]["entries"]["items"].update({"$ref": "entry.schema.json"}))
        self.assertRule(self.run_copy(), "external $ref 'entry.schema.json' is not allowed")

    def test_internal_pointer_must_resolve(self) -> None:
        self.edit(lambda d: d["properties"]["entries"]["items"].update({"$ref": "#/$defs/no_such_entry"}))
        self.assertRule(self.run_copy(), "internal $ref '#/$defs/no_such_entry' does not resolve")

    def test_anchor_ref_must_name_known_anchor(self) -> None:
        self.edit(lambda d: d["properties"]["entries"]["items"].update({"$ref": "#entryAnchor"}))
        self.assertRule(self.run_copy(), "internal $ref '#entryAnchor' names an unknown $anchor")

    # -- metaschema ----------------------------------------------------------------------
    def test_metaschema_violation(self) -> None:
        self.edit(lambda d: d["properties"]["register_id"].update({"minLength": -1}))
        self.assertRule(self.run_copy(), "not a valid Draft 2020-12 schema")

    # -- run-level -----------------------------------------------------------------------
    def test_no_schema_files_found(self) -> None:
        (self.tmp / "empty").mkdir()
        self.assertRule(self.run_copy("empty"), "no *.schema.json files found under the given roots")

    def test_missing_root_is_usage_error(self) -> None:
        self.assertRule(self.run_copy("does-not-exist"), "roots do not exist", code=2)

    # -- warnings stay warnings ------------------------------------------------------------
    def test_foreign_id_namespace_only_warns(self) -> None:
        self.edit(lambda d: d.update({"$id": "https://example.org/schemas/register"}))
        proc = self.run_copy()
        self.assertExit(proc, 0)
        self.assertIn("WARN  $id is outside the house namespace https://g9.local/", output_of(proc))

    def test_open_nested_object_only_warns(self) -> None:
        def fn(d: dict) -> None:
            d["$defs"]["entry"]["additionalProperties"] = True

        self.edit(fn)
        proc = self.run_copy()
        self.assertExit(proc, 0)
        self.assertIn("WARN  nested object schemas without additionalProperties:false at ['#/$defs/entry']", output_of(proc))

    # -- $dynamicRef: the same rules as $ref ------------------------------------------------
    # Before the fix only the keyword `$ref` was inspected, so a `$dynamicRef` to another document (a
    # reference outside the file just the same: "no external $ref ... file resolution is disabled")
    # passed. The cases mirror the $ref cases above.
    def test_dynamic_ref_must_be_string(self) -> None:
        self.edit(lambda d: d["properties"]["entries"]["items"].update({"$dynamicRef": 5}))
        self.assertRule(self.run_copy(), "#/properties/entries/items/$dynamicRef: $dynamicRef must be a string")

    def test_external_dynamic_ref(self) -> None:
        self.edit(lambda d: d["properties"]["entries"]["items"].update({"$dynamicRef": "https://example.org/schemas/entry"}))
        self.assertRule(self.run_copy(), "external $dynamicRef 'https://example.org/schemas/entry' is not allowed")

    def test_relative_file_dynamic_ref(self) -> None:
        self.edit(lambda d: d["properties"]["entries"]["items"].update({"$dynamicRef": "entry.schema.json#meta"}))
        self.assertRule(self.run_copy(), "external $dynamicRef 'entry.schema.json#meta' is not allowed")

    def test_internal_dynamic_ref_pointer_must_resolve(self) -> None:
        self.edit(lambda d: d["properties"]["entries"]["items"].update({"$dynamicRef": "#/$defs/no_such_entry"}))
        self.assertRule(self.run_copy(), "internal $dynamicRef '#/$defs/no_such_entry' does not resolve")

    def test_dynamic_ref_must_name_known_anchor(self) -> None:
        self.edit(lambda d: d["properties"]["entries"]["items"].update({"$dynamicRef": "#entryAnchor"}))
        self.assertRule(self.run_copy(), "internal $dynamicRef '#entryAnchor' names an unknown $anchor")

    def test_dynamic_ref_to_dynamic_anchor_passes(self) -> None:
        # Control: `$dynamicAnchor` defines a plain-name fragment (as `$anchor` does), so both
        # `$dynamicRef` and `$ref` to "#entry" resolve inside the document.
        def fn(d: dict) -> None:
            d["$defs"]["entry"]["$dynamicAnchor"] = "entry"
            d["properties"]["entries"]["items"] = {"$dynamicRef": "#entry"}
            d["properties"]["entries"]["contains"] = {"$ref": "#entry"}

        self.edit(fn)
        proc = self.run_copy()
        self.assertExit(proc, 0)
        self.assertIn("RESULT: PASS", output_of(proc))

    def test_dynamic_ref_to_pointer_passes(self) -> None:
        self.edit(lambda d: d["properties"]["entries"]["items"].update({"$dynamicRef": "#/$defs/entry"}))
        proc = self.run_copy()
        self.assertExit(proc, 0)
        self.assertIn("RESULT: PASS", output_of(proc))


class Sanity(unittest.TestCase):
    def test_target_carries_an_internal_ref(self) -> None:
        # The $ref mutations above edit this exact location; fail loudly if the fixture changes shape.
        doc = load_json(Path(REPO / TARGET))
        self.assertEqual(doc["properties"]["entries"]["items"], {"$ref": "#/$defs/entry"})


if __name__ == "__main__":
    unittest.main()
