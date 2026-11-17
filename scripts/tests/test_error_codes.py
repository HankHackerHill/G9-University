"""Tests of scripts/gen_error_codes.py (OBS-5): the error-code catalogue and its gate.

Black box: the script runs as ``[sys.executable, '-I', script, ...]`` in a subprocess. Every
mutation is made on a temporary copy of the inputs (``--root``), never on the repository. The copy
is regenerated with ``--write`` first, so the negative cases do not depend on whether the real
catalogue is current while other work changes the Rust sources.

Run from the repository root:
    python3 -I -m unittest discover -s scripts/tests -p 'test_error_codes.py'
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
SCRIPT = os.path.join(REPO, 'scripts', 'gen_error_codes.py')
CATALOGUE = 'docs/runbooks/error-codes.md'
ACTIONS = 'docs/runbooks/error-codes.actions.json'
DOCS = ('docs/ui/ux-copy.md', 'docs/runbooks/first-run.md', 'docs/runbooks/backup-restore.md',
        'docs/curriculum/promotion.md')
TIMEOUT_S = 300


def run(*args, root=None):
    cmd = [sys.executable, '-I', SCRIPT] + list(args)
    if root is not None:
        cmd += ['--root', root]
    proc = subprocess.run(cmd, cwd=REPO, capture_output=True, text=True, timeout=TIMEOUT_S)
    return proc.returncode, proc.stdout + proc.stderr


def copy_inputs(dest):
    """Copy what the script reads: production source roots, the frontend, the docs it checks."""
    crates = os.path.join(REPO, 'crates')
    for name in sorted(os.listdir(crates)):
        src = os.path.join(crates, name, 'src')
        if os.path.isdir(src):
            shutil.copytree(src, os.path.join(dest, 'crates', name, 'src'))
    for rel in ('apps/cli/src', 'apps/desktop/src-tauri/src', 'apps/desktop/src'):
        shutil.copytree(os.path.join(REPO, rel), os.path.join(dest, rel),
                        ignore=shutil.ignore_patterns('node_modules', 'dist'))
    for rel in DOCS + (CATALOGUE, ACTIONS):
        os.makedirs(os.path.dirname(os.path.join(dest, rel)), exist_ok=True)
        shutil.copy2(os.path.join(REPO, rel), os.path.join(dest, rel))


class TreeCase(unittest.TestCase):
    """One fresh, regenerated copy of the inputs per test."""

    @classmethod
    def setUpClass(cls):
        cls._base = tempfile.mkdtemp(prefix='g9-obs5-base-')
        copy_inputs(cls._base)
        code, out = run('--write', root=cls._base)
        if code != 0:
            raise AssertionError('--write on the pristine copy failed: ' + out)

    @classmethod
    def tearDownClass(cls):
        shutil.rmtree(cls._base, ignore_errors=True)

    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix='g9-obs5-')
        self.root = os.path.join(self.tmp, 'tree')
        shutil.copytree(self._base, self.root)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def path(self, rel):
        return os.path.join(self.root, rel)

    def write(self, rel, text):
        os.makedirs(os.path.dirname(self.path(rel)), exist_ok=True)
        with open(self.path(rel), 'w', encoding='utf-8') as handle:
            handle.write(text)

    def append(self, rel, text):
        with open(self.path(rel), 'a', encoding='utf-8') as handle:
            handle.write(text)

    def read(self, rel):
        with open(self.path(rel), encoding='utf-8') as handle:
            return handle.read()

    def actions(self):
        return json.loads(self.read(ACTIONS))

    def save_actions(self, data):
        self.write(ACTIONS, json.dumps(data, indent=2))

    def check(self):
        return run(root=self.root)

    def assert_fails(self, *needles):
        code, out = self.check()
        self.assertEqual(code, 1, out)
        for needle in needles:
            self.assertIn(needle, out)
        return out

    def rows_of(self, code):
        """``{category heading: row line}`` of a code in the copy's catalogue."""
        found, heading = {}, None
        for line in self.read(CATALOGUE).splitlines():
            if line.startswith('## '):
                heading = line[3:].strip('`')
            elif line.startswith('| `%s` |' % code):
                found[heading] = line
        return found


class RealTree(unittest.TestCase):
    def test_positive_control_on_the_repository(self):
        code, out = run()
        self.assertEqual(code, 0, out)
        self.assertIn('PASS', out)

    def test_catalogue_lists_every_category_and_the_dynamic_pattern(self):
        with open(os.path.join(REPO, CATALOGUE), encoding='utf-8') as handle:
            text = handle.read()
        for cat in ('INVALID_INPUT', 'UNAUTHORIZED', 'APPROVAL_REQUIRED', 'STALE_REVISION', 'PRECONDITION_FAILED',
                    'NOT_FOUND', 'CONFLICT', 'RESOURCE_UNAVAILABLE', 'TIMEOUT', 'CANCELLED', 'RETRYABLE_STORAGE',
                    'DATA_CORRUPTION', 'INTERNAL'):
            self.assertIn('## `%s`' % cat, text)
        self.assertIn('| `{area}.not_implemented` |', text)
        self.assertIn('| `ipc.tauri_runtime_missing` |', text)
        self.assertNotRegex(text, r'\b20\d\d-\d\d-\d\d\b')    # reproducible: no dates


class Generation(TreeCase):
    def test_pristine_copy_passes_and_write_is_deterministic(self):
        code, out = self.check()
        self.assertEqual(code, 0, out)
        first = self.read(CATALOGUE)
        code, out = run('--write', root=self.root)
        self.assertEqual(code, 0, out)
        self.assertEqual(first, self.read(CATALOGUE))

    def test_enum_codes_take_the_category_of_their_match_arm(self):
        # From<ApprovalError>: `let code = e.code(); match e { .. => G9Error::unauthorized(code, ..) }`
        self.assertEqual(list(self.rows_of('approval.already_consumed')), ['UNAUTHORIZED'])
        self.assertEqual(list(self.rows_of('storage.busy')), ['RETRYABLE_STORAGE'])
        self.assertEqual(list(self.rows_of('restore.swap_failed')), ['RESOURCE_UNAVAILABLE'])

    def test_operation_names_are_not_codes(self):
        self.assertEqual(self.rows_of('notes.create'), {})
        self.assertEqual(self.rows_of('backup.verify'), {})

    def test_spot_check_is_seeded_and_deterministic(self):
        code1, out1 = run('--spot-check', '--seed', '7', root=self.root)
        code2, out2 = run('--spot-check', '--seed', '7', root=self.root)
        self.assertEqual((code1, code2), (0, 0), out1 + out2)
        self.assertEqual(out1, out2)
        report = json.loads(out1)
        self.assertEqual(len(report['sample']), 25)
        self.assertTrue(all(item['verdict'] for item in report['sample']))


class NewCodes(TreeCase):
    PROBE = 'crates/g9-app/src/obs5_probe.rs'

    def test_new_code_in_a_rust_file_without_regenerating_fails_and_names_it(self):
        self.write(self.PROBE, 'use g9_domain::G9Error;\n\npub fn obs5_probe() -> G9Error {\n'
                               '    G9Error::invalid_input("obs5.probe_new_code", "probe")\n}\n')
        self.assert_fails('stale', 'obs5.probe_new_code', 'has no row')
        code, out = run('--write', root=self.root)
        self.assertEqual(code, 0, out)
        self.assertIn('INVALID_INPUT', self.rows_of('obs5.probe_new_code'))
        code, out = self.check()
        self.assertEqual(code, 0, out)

    def test_new_code_in_an_existing_file_without_regenerating_fails(self):
        self.append('crates/g9-app/src/commands/notes.rs',
                    '\nfn obs5_existing() -> G9Error {\n    G9Error::conflict("note.obs5_probe", "probe")\n}\n')
        self.assert_fails('stale', 'note.obs5_probe')

    def test_changed_enum_arm_names_the_new_and_the_removed_code(self):
        rel = 'crates/g9-storage/src/error.rs'
        text = self.read(rel)
        self.assertIn('"restore.swap_failed"', text)
        self.write(rel, text.replace('"restore.swap_failed"', '"restore.swap_failed_obs5"', 1))
        self.assert_fails('code restore.swap_failed_obs5 has no row',
                          'has a row for restore.swap_failed, which the source no longer has')

    def test_code_through_a_helper_is_found(self):
        self.write(self.PROBE, 'use g9_domain::G9Error;\n\n'
                               'fn obs5_refuse(code: &str) -> G9Error {\n    G9Error::conflict(code, "probe")\n}\n\n'
                               'pub fn obs5_caller() -> G9Error {\n    obs5_refuse("obs5.via_helper")\n}\n')
        self.assert_fails('obs5.via_helper')
        run('--write', root=self.root)
        row = self.rows_of('obs5.via_helper')
        self.assertEqual(list(row), ['CONFLICT'])

    def test_new_frontend_code_fails_and_mock_codes_are_not_catalogued(self):
        self.append('apps/desktop/src/ipc/transports/mock.ts',
                    "\nexport const obs5Mock = () => fail('INTERNAL', 'mock.obs5_only', 'probe');\n")
        code, out = self.check()
        self.assertEqual(code, 0, out)
        self.append('apps/desktop/src/ipc/client.ts',
                    "\nexport function obs5Probe(): G9Error {\n"
                    "  return { category: 'CONFLICT', code: 'ipc.obs5_probe', message: `probe ${'x'}` };\n}\n")
        self.assert_fails('stale', 'ipc.obs5_probe')

    def test_test_only_code_is_ignored(self):
        self.write(self.PROBE, 'use g9_domain::G9Error;\n\n#[cfg(all(test, unix))]\nmod tests {\n'
                               '    fn f() -> super::G9Error { super::G9Error::invalid_input("obs5.test_only", "x") }\n}\n\n'
                               '#[test]\nfn t() { let _ = G9Error::internal_redacted("obs5.test_fn"); }\n')
        self.write('crates/g9-app/src/tests/obs5.rs',
                   'fn f() -> G9Error { G9Error::invalid_input("obs5.tests_dir", "x") }\n')
        code, out = self.check()
        self.assertEqual(code, 0, out)

    def test_code_passed_through_a_closure_parameter_is_unresolved(self):
        self.write(self.PROBE, 'use g9_domain::G9Error;\n\npub fn obs5_closure() -> G9Error {\n'
                               '    let refuse = |c: &str| G9Error::invalid_input(c, "probe");\n'
                               '    refuse("obs5.closure")\n}\n')
        self.assert_fails('unresolved', 'crates/g9-app/src/obs5_probe.rs')


class ReviewFindings(TreeCase):
    """Blind spots the independent review of OBS-5 found, each now a failure of the check."""

    PROBE = 'crates/g9-app/src/obs5_probe.rs'

    def test_struct_literal_that_sets_a_code_fails(self):
        self.write(self.PROBE, 'use g9_domain::{ErrorCategory, G9Error};\n\npub fn obs5() -> G9Error {\n'
                               '    G9Error { category: ErrorCategory::Conflict, code: "obs5.literal".into(), '
                               'message: String::new(), details: None, approval_id: None }\n}\n')
        self.assert_fails('unsupported', 'obs5_probe.rs', 'struct literal')

    def test_code_assignment_fails(self):
        self.write(self.PROBE, 'use g9_domain::G9Error;\n\npub fn obs5() -> G9Error {\n'
                               '    let mut e = G9Error::conflict("obs5.base", "probe");\n'
                               '    e.code = "obs5.mutated".to_string();\n    e\n}\n')
        self.assert_fails('unsupported', 'obs5_probe.rs', 'assigns .code')

    def test_wrapping_struct_literal_is_not_a_failure(self):
        self.write(self.PROBE, 'use g9_domain::G9Error;\n\npub fn obs5(e: G9Error) -> G9Error {\n'
                               '    G9Error { details: None, ..e }\n}\n')
        code, out = self.check()
        self.assertNotIn('unsupported', out)

    def test_not_implemented_pattern_matches_only_real_areas(self):
        self.append('docs/runbooks/first-run.md', '\nA refusal `ghost.not_implemented` means nothing ran.\n')
        self.assert_fails('ghost.not_implemented')
        code, out = self.check()
        self.assertNotIn('teaching.not_implemented', out.replace('ghost.not_implemented', ''))

    def test_action_quoted_from_another_row_fails(self):
        data = self.actions()
        data['codes']['reconcile.target_exists']['action'] = 'use a new `--candidate-version`'
        self.save_actions(data)
        self.assert_fails('not from the row or paragraph that names reconcile.target_exists')

    def test_summary_rows_are_named_as_counts(self):
        text = self.read(CATALOGUE)
        lines = text.splitlines()
        i = next(n for n, l in enumerate(lines) if l.startswith('| `CONFLICT` |'))
        lines[i] = lines[i].replace('| `CONFLICT` |', '| `CONFLICT` | 999 |', 1)
        self.write(CATALOGUE, '\n'.join(lines) + '\n')
        out = self.assert_fails('summary of CONFLICT count differs')
        self.assertNotIn('the row of CONFLICT', out)


class DeadReferences(TreeCase):
    def test_dead_reference_fails_and_names_the_document_line(self):
        self.append('docs/runbooks/first-run.md', '\n| You see | `curriculum.obs5_ghost` | Do |\n')
        line = len(self.read('docs/runbooks/first-run.md').splitlines())
        self.assert_fails('dead-reference', 'docs/runbooks/first-run.md:%d' % line, 'curriculum.obs5_ghost')

    def test_dead_reference_in_each_checked_document(self):
        for rel in DOCS:
            with self.subTest(doc=rel):
                original = self.read(rel)
                self.append(rel, '\nThe code `storage.obs5_ghost` is refused.\n')
                self.assert_fails('dead-reference', rel, 'storage.obs5_ghost')
                self.write(rel, original)

    def test_allow_list_with_a_reason_resolves_it(self):
        self.append('docs/ui/ux-copy.md', '\nPlanned: `goals.obs5_planned`.\n')
        self.assert_fails('dead-reference', 'goals.obs5_planned')
        data = self.actions()
        data['doc_reference_allow']['goals.obs5_planned'] = 'a planned operation name, not a code'
        self.save_actions(data)
        code, out = self.check()
        self.assertEqual(code, 0, out)

    def test_allow_list_entry_needs_a_reason_and_a_reference(self):
        data = self.actions()
        data['doc_reference_allow']['goals.list'] = ''
        data['doc_reference_allow']['nowhere.obs5'] = 'not named anywhere'
        self.save_actions(data)
        self.assert_fails('doc_reference_allow entry goals.list has no reason',
                          'doc_reference_allow entry nowhere.obs5 is not named in any checked document')

    def test_allow_list_entry_for_a_real_code_is_stale(self):
        data = self.actions()
        data['doc_reference_allow']['restore.database_in_use'] = 'stale'
        self.save_actions(data)
        self.assert_fails('doc_reference_allow entry restore.database_in_use is now resolved by the source')

    def test_category_contradiction_fails(self):
        self.append('docs/runbooks/backup-restore.md', '\nAn unknown id answers `INTERNAL backup.not_found`.\n')
        self.assert_fails('category-contradiction', 'backup.not_found', 'NOT_FOUND')

    def test_paths_files_and_operations_are_not_references(self):
        self.append('docs/runbooks/first-run.md',
                    '\nRun `notes.create`, read `crates/g9-app/src/obs5.rs`, open `obs5.sqlite`, e.g. later.\n')
        code, out = self.check()
        self.assertEqual(code, 0, out)


class Actions(TreeCase):
    def test_action_must_be_a_quotation_of_its_source(self):
        data = self.actions()
        data['codes']['reconcile.target_exists']['action'] = 'delete the existing file and run again'
        self.save_actions(data)
        self.assert_fails('actions', 'reconcile.target_exists', 'is not a quotation of docs/curriculum/promotion.md')

    def test_action_source_must_name_the_code(self):
        data = self.actions()
        data['codes']['note.not_found'] = {'action': 'Check the name or the code.', 'source': 'docs/ui/ux-copy.md'}
        self.save_actions(data)
        self.assert_fails('the action of note.not_found quotes docs/ui/ux-copy.md, which does not mention note.not_found')

    def test_entry_for_a_code_the_source_lacks_fails(self):
        data = self.actions()
        data['codes']['obs5.unknown'] = {'meaning': 'x', 'basis': 'crates/g9-app/src/lib.rs'}
        self.save_actions(data)
        self.assert_fails('names obs5.unknown, which the source does not have')

    def test_missing_category_default_fails(self):
        data = self.actions()
        del data['category_defaults']['INTERNAL']
        self.save_actions(data)
        self.assert_fails('no category default for INTERNAL', 'has no operator action')

    def test_written_meaning_needs_a_basis(self):
        data = self.actions()
        data['codes']['skills.cancelled']['basis'] = 'crates/obs5/missing.rs'
        self.save_actions(data)
        self.assert_fails('the written meaning of skills.cancelled names no readable "basis" file')


class ToolErrors(TreeCase):
    def test_missing_catalogue_fails(self):
        os.remove(self.path(CATALOGUE))
        self.assert_fails('stale', 'does not exist')

    def test_missing_actions_file_is_a_tool_error(self):
        os.remove(self.path(ACTIONS))
        code, out = self.check()
        self.assertEqual(code, 2, out)
        self.assertIn('missing ' + ACTIONS, out)

    def test_unreadable_actions_json_is_a_tool_error(self):
        self.write(ACTIONS, '{not json')
        code, out = self.check()
        self.assertEqual(code, 2, out)
        self.assertIn('cannot read', out)

    def test_hand_edit_of_a_row_is_stale(self):
        text = self.read(CATALOGUE)
        edited = re.sub(r'(\| `note\.not_found` \| )"[^"]*"', r'\1"edited by hand"', text, count=1)
        self.assertNotEqual(text, edited)
        self.write(CATALOGUE, edited)
        self.assert_fails('stale: the row of note.not_found differs from the source')


if __name__ == '__main__':
    unittest.main()
