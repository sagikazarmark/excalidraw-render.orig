#!/usr/bin/env python3
"""Authorization/rollback tests in disposable roots, never fake live approval.

The GitHub transport and native-consumer process are mocked. Fixtures use the
actual immutable v1 migration templates and its unchanged validate_migration;
the production repository, approved evidence and V8 manifest are read-only.
"""
import copy
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import types
import unittest

HERE = Path(__file__).resolve().parent
REAL_ROOT = HERE.parents[3]
REAL_BASE = REAL_ROOT / 'tests/conformance/proposals/stage4-format-requalification-v1'
SOURCE = (HERE / 'requalify.py').read_bytes()


def module():
    result = types.ModuleType('review_fix_fixture')
    result.__file__ = str(HERE / 'requalify.py')
    exec(compile(SOURCE, result.__file__, 'exec', optimize=0), result.__dict__)
    return result


class AuthorizationTests(unittest.TestCase):
    def setUp(self):
        self.output = contextlib.redirect_stdout(io.StringIO())
        self.output.__enter__()
        self.addCleanup(self.output.__exit__, None, None, None)
        self.temporary = tempfile.TemporaryDirectory(prefix='format-auth-fixture-', dir='/tmp/opencode')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.m = module()
        self.m.ROOT = self.root
        self.m.PACKET = self.root / 'tests/conformance/proposals/stage4-format-requalification-v2'
        self.m.PACKET.mkdir(parents=True)
        self.m.BASE = self.root / 'tests/conformance/proposals/stage4-format-requalification-v1'
        # Read-only evidence sharing; the checker loads authenticated bytes and
        # never writes or imports pycache into this preserved predecessor.
        self.m.BASE.symlink_to(REAL_BASE, target_is_directory=True)
        self.legacy = self.m.predecessor()
        self.legacy.ROOT = self.root
        self.checks = []
        self.calls = []
        self.legacy.xtask = self.native_consumer
        self.m.evidence_verification = self.exact_migration
        self.live_paths = [self.m.V8 + '/manifest.json', self.m.REGISTRY, self.m.LEDGER]
        for relative in self.live_paths + ['tests/conformance/coverage/inventory.json', 'tests/conformance/coverage/requirements.json']:
            destination = self.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(REAL_ROOT / relative, destination)
        for name in ['png_stage4_contract', 'stage4_interaction_contract']:
            relative = f'crates/excalidraw-render/tests/fast/{name}.rs'
            destination = self.root / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(REAL_ROOT / relative, destination)
        self.before = {p: (self.root / p).read_bytes() for p in self.live_paths}
        self.real_before = {p: (REAL_ROOT / p).read_bytes() for p in self.live_paths}
        (self.m.PACKET / 'requalify.py').write_bytes(SOURCE)
        files = [self.m.descriptor(self.m.PACKET / 'requalify.py')]
        manifest = {'schema': 'isolated-authentication-test-fixture', 'approval': None,
                    'predecessor_subject_sha256': self.m.BASE_SUBJECT, 'representative_subject_sha256': self.m.V8_SUBJECT,
                    'files': files, 'predecessor_files': [self.m.descriptor(self.m.BASE / 'approval-subject.json'),
                                                        self.m.descriptor(self.m.BASE / 'requalify.py')]}
        bundle = {'schema': 'excalidraw-render/conformance-approval-subject/v1',
                  'manifest': {k: v for k, v in manifest.items() if k != 'approval'}, 'files': files}
        self.write(self.m.PACKET / 'manifest.json', manifest)
        self.write(self.m.PACKET / 'approval-subject.json', bundle)
        self.subject = 'sha256:' + self.m.digest(self.m.canonical(bundle))
        self.approval = {'approved_by': 'fixture-reviewer', 'approved_at': '2026-09-20',
                         'approval_url': 'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-123456789',
                         'approval_subject_sha256': self.subject}
        self.comment = {'id': 123456789, 'user': {'login': 'fixture-reviewer'}, 'created_at': '2026-09-20T12:00:00Z',
                        'html_url': self.approval['approval_url'],
                        'body': f'ISOLATED TEST FIXTURE, NOT A REAL APPROVAL. Shaped decision names {self.subject} and {self.m.V8_SUBJECT}.'}
        self.remote = copy.deepcopy(self.comment)
        self.fetches = []
        self.m.fetch_comment = self.fetch
        self.approval_path = self.root / 'fixture-approval.json'
        self.write(self.approval_path, self.approval)

    def tearDown(self):
        for p, data in self.real_before.items():
            self.assertEqual((REAL_ROOT / p).read_bytes(), data, 'test modified real metadata')

    def write(self, path, value):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(self.m.pretty(value))

    def fetch(self, url):
        self.fetches.append(url)
        return copy.deepcopy(self.remote)

    def native_consumer(self, *args):
        self.calls.append(args)
        return b'{"isolated_test_process_stub":true}\n'

    def exact_migration(self, applied):
        self.checks.append(applied)
        self.legacy.validate_migration(applied)

    def applied_fixture(self):
        files = self.m.live_transaction(self.approval)
        for p, data in files.items():
            (self.root / p).write_bytes(data)
        app = self.m.application_path()
        self.write(app / 'status.json', {'state': 'applied', 'formatting_subject_sha256': self.subject,
                                        'representative_subject_sha256': self.m.V8_SUBJECT})
        self.write(app / 'approval.json', self.approval)
        self.write(app / 'comment.json', self.comment)

    def reject_applied(self, pattern):
        prior = len(self.checks)
        with self.assertRaisesRegex(ValueError, pattern):
            self.m.verify_applied()
        self.assertEqual(len(self.checks), prior, 'unauthorized audit reached migration validation')

    def test_optimized_startup_refuses_before_any_workflow_operation(self):
        for flags, optimize in [(['-O'], None), (['-OO'], None), ([], '1'), ([], '2')]:
            with self.subTest(flags=flags, optimize=optimize):
                directory = self.root / ('startup-' + str(len(list(self.root.glob('startup-*')))))
                directory.mkdir()
                script = directory / 'checker.py'
                script.write_bytes(SOURCE)
                marker = directory / 'live-canary'
                marker.write_bytes(b'unchanged')
                env = dict(os.environ, PYTHONDONTWRITEBYTECODE='1')
                env.pop('PYTHONOPTIMIZE', None)
                if optimize is not None: env['PYTHONOPTIMIZE'] = optimize
                result = subprocess.run([sys.executable, *flags, str(script), 'apply', str(directory / 'missing-approval.json')],
                                        cwd=directory, env=env, capture_output=True)
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout, b'')
                self.assertIn(b'Optimized Python execution is forbidden; no workflow started.', result.stderr)
                self.assertEqual(marker.read_bytes(), b'unchanged')
                self.assertEqual({p.name for p in directory.iterdir()}, {'checker.py', 'live-canary'})

    def test_applied_audit_requires_formatting_approval_even_with_v8_approval(self):
        self.applied_fixture()
        (self.m.application_path() / 'approval.json').unlink()
        self.reject_applied('missing or invalid approval.json')

    def test_applied_audit_requires_saved_genuine_decision(self):
        self.applied_fixture()
        (self.m.application_path() / 'comment.json').unlink()
        self.reject_applied('missing or invalid comment.json')

    def test_changed_formatting_approval_subject_and_fields_are_rejected(self):
        self.applied_fixture()
        for changed, needle in [(dict(self.approval, approval_subject_sha256=self.m.BASE_SUBJECT), 'subject mismatch'),
                                (dict(self.approval, extra='not allowed'), 'exactly four fields')]:
            with self.subTest(needle=needle):
                self.write(self.m.application_path() / 'approval.json', changed)
                self.reject_applied(needle)

    def test_both_subjects_are_required_in_the_actual_decision(self):
        self.applied_fixture()
        for omitted in [self.subject, self.m.V8_SUBJECT]:
            with self.subTest(omitted=omitted):
                changed = dict(self.comment, body=self.comment['body'].replace(omitted, 'omitted'))
                self.remote = changed
                self.write(self.m.application_path() / 'comment.json', changed)
                self.reject_applied('both exact subjects')

    def test_comment_author_date_url_and_id_must_agree(self):
        self.applied_fixture()
        mutations = [({'user': {'login': 'other-reviewer'}}, 'author mismatch'),
                     ({'created_at': '2026-09-21T12:00:00Z'}, 'date mismatch'),
                     ({'html_url': self.comment['html_url'] + '0'}, 'URL mismatch'),
                     ({'id': 987654321}, 'ID does not match')]
        for fields, needle in mutations:
            with self.subTest(needle=needle):
                changed = dict(self.comment, **fields)
                self.write(self.m.application_path() / 'comment.json', changed)
                self.remote = changed
                self.reject_applied(needle)

    def test_saved_comment_must_match_authenticated_github_response(self):
        self.applied_fixture()
        self.remote['body'] += ' remote-only change'
        self.reject_applied('does not match genuine GitHub comment')
        self.assertTrue(self.fetches)

    def test_v8_approval_must_be_the_same_decision_for_the_v8_subject(self):
        self.applied_fixture()
        path = self.root / self.m.V8 / 'manifest.json'
        original = self.m.read_json(path)
        for field, value in [('approved_by', 'different'), ('approved_at', '2026-09-21'),
                             ('approval_url', self.approval['approval_url'] + '0'),
                             ('approval_subject_sha256', self.subject)]:
            with self.subTest(field=field):
                changed = copy.deepcopy(original)
                changed['approval'][field] = value
                self.write(path, changed)
                self.reject_applied('V8 approval differs')

    def test_genuine_shaped_authenticated_fixture_passes_exact_transaction(self):
        self.applied_fixture()
        self.m.verify_applied()
        self.assertEqual(self.checks, [True])
        self.assertTrue(self.fetches)

    def test_authorization_does_not_relax_exact_transaction_validation(self):
        self.applied_fixture()
        path = self.root / self.m.LEDGER
        ledger = self.m.read_json(path)
        ledger['assignments'][0]['disposition']['rationale'] = 'unrelated mutation'
        self.write(path, ledger)
        with self.assertRaises(AssertionError):
            self.m.verify_applied()
        self.assertEqual(self.checks, [True])

    def test_decision_records_exist_before_final_applied_audit(self):
        original = self.m.applied_audit
        original_write = self.m.write_live
        observed = []
        def write(path, data):
            app = self.m.application_path()
            self.assertEqual(self.m.read_json(app / 'approval.json'), self.approval)
            self.assertEqual(self.m.read_json(app / 'comment.json'), self.comment)
            original_write(path, data)
        self.m.write_live = write
        def audit(subject, allow_validating=False):
            app = self.m.application_path()
            self.assertEqual(self.m.read_json(app / 'approval.json'), self.approval)
            self.assertEqual(self.m.read_json(app / 'comment.json'), self.comment)
            observed.append(self.m.read_json(app / 'status.json')['state'])
            return original(subject, allow_validating)
        self.m.applied_audit = audit
        self.m.apply(self.approval_path)
        self.assertEqual(observed, ['validating'])
        self.assertEqual(self.m.read_json(self.m.application_path() / 'status.json')['state'], 'applied')
        self.m.verify_applied()
        self.assertEqual(observed, ['validating', 'applied'])
        self.assertEqual(len(self.calls), 5)

    def test_failed_audit_rolls_back_exact_bytes_and_retry_archives_failure(self):
        def fail_at_final_auth(*args):
            result = self.native_consumer(*args)
            if args[0] == 'verify-ownership': self.remote['body'] += ' changed after initial authentication'
            return result
        self.legacy.xtask = fail_at_final_auth
        with self.assertRaisesRegex(ValueError, 'does not match genuine GitHub comment'):
            self.m.apply(self.approval_path)
        for p, data in self.before.items(): self.assertEqual((self.root / p).read_bytes(), data)
        failed = self.m.read_json(self.m.application_path() / 'status.json')
        self.assertEqual(failed['state'], 'failed_rolled_back')
        self.remote = copy.deepcopy(self.comment)
        self.legacy.xtask = self.native_consumer
        self.m.apply(self.approval_path)
        archive = self.m.application_path().with_name(self.m.application_path().name + '.failed-' + failed['attempt_id'])
        self.assertEqual(self.m.read_json(archive / 'status.json'), failed)
        self.assertEqual(self.m.read_json(archive / 'approval.json'), self.approval)
        for p, data in self.before.items(): self.assertEqual((archive / 'before' / p).read_bytes(), data)
        self.m.verify_applied()

    def test_rollback_failure_is_explicit_and_blocks_automatic_retry(self):
        original_write = self.m.write_live
        def write(path, data):
            if path == self.root / self.m.REGISTRY and data == self.before[self.m.REGISTRY]:
                raise OSError('isolated rollback failure')
            original_write(path, data)
        self.m.write_live = write
        self.legacy.xtask = lambda *args: (_ for _ in ()).throw(RuntimeError('isolated consumer failure'))
        with self.assertRaisesRegex(RuntimeError, 'rollback incomplete'):
            self.m.apply(self.approval_path)
        self.assertEqual(self.m.read_json(self.m.application_path() / 'status.json')['state'], 'rollback_failed')
        with self.assertRaisesRegex(ValueError, 'manual inspection/recovery required'):
            self.m.apply(self.approval_path)

    def test_incomplete_attempt_is_not_treated_as_retryable_or_applied(self):
        self.write(self.m.application_path() / 'status.json', {'state': 'applying'})
        with self.assertRaisesRegex(ValueError, 'manual inspection/recovery required'):
            self.m.apply(self.approval_path)
        self.reject_applied('not successfully applied')
        for p, data in self.before.items(): self.assertEqual((self.root / p).read_bytes(), data)


if __name__ == '__main__':
    unittest.main(verbosity=2)
