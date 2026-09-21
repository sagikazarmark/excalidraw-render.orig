#!/usr/bin/env python3
"""Isolated decision tests; mocked GitHub only, no fake live approval or writes."""
import copy
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest

P = Path(__file__).resolve().parent


def checker():
    m = types.ModuleType('jpeg187_strict_decision_fixture')
    m.__file__ = str(P / 'verify.py')
    exec(compile((P / 'verify.py').read_bytes(), m.__file__, 'exec', optimize=0), m.__dict__)
    return m


class Decisions(unittest.TestCase):
    def setUp(self):
        self.v = checker()
        self.subject = 'sha256:' + 'a' * 64
        self.decision = {'decision': 'approve', 'scope': 'jpeg187-color-only',
                         'production_subject_sha256': self.subject, 'representative_subject_sha256': self.v.V9_SUBJECT}
        self.a = {'approved_by': 'sagikazarmark', 'approved_at': '2026-09-21',
                  'approval_url': 'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-123456789',
                  'approval_subject_sha256': self.subject}
        self.c = {'id': 123456789, 'html_url': self.a['approval_url'], 'user': {'login': 'sagikazarmark'},
                  'created_at': '2026-09-21T00:00:00Z', 'body': self.body(self.decision)}
        self.remote = copy.deepcopy(self.c)
        self.calls = []
        def fetch(url):
            self.calls.append(url)
            return copy.deepcopy(self.remote)
        self.v.fetch_comment = fetch
        self.rep = dict(self.a, approval_subject_sha256=self.v.V9_SUBJECT)

    def body(self, obj):
        return '```json\n' + json.dumps(obj, indent=2) + '\n```'

    def authenticate(self):
        self.v.authenticate(self.a, self.c, self.subject, self.rep)

    def test_exact_affirmative_designated_maintainer_fixture_passes(self):
        self.authenticate()
        self.assertEqual(self.calls, [self.a['approval_url']])

    def test_old_hash_mention_positive_fixture_is_now_rejected(self):
        self.c['body'] = 'Fixture review discusses ' + self.subject + ' and ' + self.v.V9_SUBJECT
        self.remote = copy.deepcopy(self.c)
        with self.assertRaisesRegex(ValueError, 'standalone fenced'): self.authenticate()

    def test_actual_old_verifier_accepts_rejection_but_successor_rejects_it(self):
        self.c['body'] = 'I reject ' + self.subject + ' and ' + self.v.V9_SUBJECT + '. Do not apply.'
        self.remote = copy.deepcopy(self.c)
        old = self.v.load_frozen('qualify.py')
        old.fetch_comment = lambda url: copy.deepcopy(self.remote)
        old.authenticate(self.a, self.c, self.subject, self.v.V9_SUBJECT, self.rep)
        with self.assertRaisesRegex(ValueError, 'standalone fenced'): self.authenticate()

    def test_other_author_cannot_grant_approval_even_when_supplied_identity_matches(self):
        self.a['approved_by'] = 'another-reviewer'
        self.rep['approved_by'] = 'another-reviewer'
        self.c['user']['login'] = 'another-reviewer'
        self.remote = copy.deepcopy(self.c)
        with self.assertRaisesRegex(ValueError, 'designated maintainer'): self.authenticate()

    def test_designated_supplied_login_does_not_override_actual_comment_author(self):
        self.c['user']['login'] = 'another-reviewer'
        self.remote = copy.deepcopy(self.c)
        with self.assertRaisesRegex(ValueError, 'saved comment is not'): self.authenticate()

    def test_remote_comment_author_must_be_designated_maintainer(self):
        self.remote['user']['login'] = 'another-reviewer'
        with self.assertRaisesRegex(ValueError, 'genuine comment is not'): self.authenticate()

    def test_reject_pending_missing_or_wrong_scope_and_subject(self):
        mutations = [('decision', 'reject'), ('decision', 'pending'), ('decision', 'approved'),
                     ('scope', 'all-jpeg'), ('production_subject_sha256', self.v.BASE_SUBJECT),
                     ('representative_subject_sha256', self.subject)]
        for field, value in mutations:
            with self.subTest(field=field, value=value):
                bad = dict(self.decision, **{field: value})
                self.c['body'] = self.body(bad); self.remote = copy.deepcopy(self.c)
                with self.assertRaisesRegex(ValueError, 'affirmative exact'): self.authenticate()
        missing = dict(self.decision); del missing['decision']
        self.c['body'] = self.body(missing)
        with self.assertRaisesRegex(ValueError, 'four decision fields'): self.authenticate()

    def test_duplicate_blocks_keys_and_unknown_fields_rejected(self):
        duplicate = self.body(self.decision).replace('"decision": "approve",', '"decision": "reject", "decision": "approve",')
        bodies = [self.body(self.decision) + '\n' + self.body(self.decision), duplicate,
                  self.body(dict(self.decision, extra='unreviewed')), '```json\n{broken}\n```']
        for body in bodies:
            with self.subTest(body=body):
                self.c['body'] = body; self.remote = copy.deepcopy(self.c)
                with self.assertRaises(ValueError): self.authenticate()

    def test_contradictory_or_quoted_approval_block_rejected(self):
        for body in ['I reject this.\n' + self.body(self.decision), self.body(self.decision) + '\nDo not apply.',
                     '\n'.join('> ' + line for line in self.body(self.decision).splitlines())]:
            with self.subTest(body=body):
                self.c['body'] = body; self.remote = copy.deepcopy(self.c)
                with self.assertRaisesRegex(ValueError, 'standalone fenced'): self.authenticate()

    def test_saved_vs_remote_decision_mismatch_rejected(self):
        self.remote['created_at'] = '2026-09-21T01:00:00Z'
        with self.assertRaisesRegex(ValueError, 'differs from genuine'): self.authenticate()

    def test_actual_remote_rejection_cannot_use_saved_affirmative_snapshot(self):
        self.remote['body'] = self.body(dict(self.decision, decision='reject'))
        with self.assertRaisesRegex(ValueError, 'affirmative exact'): self.authenticate()

    def test_approval_fields_and_v9_decision_must_match(self):
        for mutation in ['subject', 'date', 'url', 'extra', 'v9']:
            with self.subTest(mutation=mutation):
                a = copy.deepcopy(self.a); rep = copy.deepcopy(self.rep)
                if mutation == 'subject': a['approval_subject_sha256'] = self.v.BASE_SUBJECT
                elif mutation == 'date': a['approved_at'] = '2026-09-20'
                elif mutation == 'url': a['approval_url'] = a['approval_url'].replace('/85#', '/187#')
                elif mutation == 'extra': a['extra'] = True
                else: rep['approval_subject_sha256'] = self.subject
                with self.assertRaises(ValueError): self.v.authenticate(a, self.c, self.subject, rep)

    def test_applied_verification_authenticates_before_migration(self):
        with tempfile.TemporaryDirectory(dir='/tmp/opencode', prefix='jpeg187-v2-auth-') as temp:
            self.v.ROOT = Path(temp)
            app = self.v.ROOT / self.v.APPLICATION; app.mkdir(parents=True)
            candidate = self.v.ROOT / self.v.V9; candidate.mkdir(parents=True)
            write = lambda path, value: path.write_text(json.dumps(value))
            write(app / 'status.json', {'state': 'applied', 'production_subject_sha256': self.subject,
                                      'representative_subject_sha256': self.v.V9_SUBJECT})
            write(app / 'approval.json', self.a)
            write(candidate / 'manifest.json', {'approval': self.rep})
            self.v.packet = lambda applied=False: self.subject
            audited = []
            self.v.evidence = lambda applied: audited.append(applied)
            with self.assertRaisesRegex(ValueError, 'comment.json'): self.v.verify(True)
            self.assertEqual(audited, [])
            write(app / 'comment.json', dict(self.c, body='Reject ' + self.subject + ' ' + self.v.V9_SUBJECT))
            with self.assertRaisesRegex(ValueError, 'standalone fenced'): self.v.verify(True)
            self.assertEqual(audited, [])
            write(app / 'comment.json', self.c)
            with contextlib.redirect_stdout(io.StringIO()): self.v.verify(True)
            self.assertEqual(audited, [True])

    def test_optimized_execution_refuses_before_any_workflow(self):
        for flags, environment in [(['-O'], None), (['-OO'], None), ([], '1')]:
            with tempfile.TemporaryDirectory(dir='/tmp/opencode') as temp:
                script = Path(temp) / 'verify.py'; script.write_bytes((P / 'verify.py').read_bytes())
                env = dict(os.environ, PYTHONDONTWRITEBYTECODE='1'); env.pop('PYTHONOPTIMIZE', None)
                if environment: env['PYTHONOPTIMIZE'] = environment
                result = subprocess.run([sys.executable, *flags, str(script), 'verify-applied'], cwd=temp, env=env, capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(b'Optimized Python forbidden; no workflow started.', result.stderr)
                self.assertEqual([p.name for p in Path(temp).iterdir()], ['verify.py'])


if __name__ == '__main__': unittest.main(verbosity=2)
