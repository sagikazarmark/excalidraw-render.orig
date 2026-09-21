#!/usr/bin/env python3
"""Exact raw-result classification tests; no production tests or live mutations."""
import copy
from pathlib import Path
import subprocess
import sys
import types
import unittest

P=Path(__file__).resolve().parent
def load(name):
    m=types.ModuleType('fixture_'+name);m.__file__=str(P/name)
    exec(compile((P/name).read_bytes(),m.__file__,'exec',optimize=0),m.__dict__)
    return m
s=load('suite-verification.py')

class RawClassification(unittest.TestCase):
    def setUp(self):self.raw=(P/'checks/full-workspace.raw.log').read_bytes()
    def test_actual_completed_run_totals_and_exact_five_failures(self):
        r=s.classify(self.raw);self.assertEqual((r['passed'],r['failed'],r['ignored']),(1226,5,19));self.assertFalse(r['all_tests_green'])
        self.assertEqual({f['test'] for f in r['failures']},set(s.EXPECTED))
    def test_unknown_failure_identity_rejected_even_with_same_count(self):
        old=next(iter(s.EXPECTED));bad=self.raw.replace(old.encode(),b'unknown::regression')
        with self.assertRaisesRegex(ValueError,'failed test identity'):s.classify(bad)
    def test_different_error_message_rejected(self):
        for old,new in [(b'"stale ownership evidence"',b'"actual renderer regression"'),
                        (b'error: stale adapter production/source bindings',b'error: malformed native output'),
                        (b'assertion failed: String::from_utf8_lossy(&output.stderr).contains("unknown ownership identity")',b'assertion failed: false')]:
            with self.subTest(old=old):
                with self.assertRaisesRegex(ValueError,'failure message/body'):s.classify(self.raw.replace(old,new,1))
    def test_extra_error_and_hidden_message_rejected(self):
        for bad in [self.raw+b'\nerror: unexpected compilation failure\n',self.raw.replace(b'error: stale ownership evidence\n',b'error: stale ownership evidence\nadditional unexpected panic details\n',1)]:
            with self.assertRaises(ValueError):s.classify(bad)
    def test_truncated_command_missing_summary_rejected(self):
        with self.assertRaisesRegex(ValueError,'error outside|did not finish'):s.classify(self.raw.split(b'error: 3 targets failed:')[0])
    def test_wrong_counts_or_relabelled_target_rejected(self):
        for bad in [self.raw.replace(b'595 passed;',b'594 passed;',1),self.raw.replace(b'test result: FAILED. 101 passed;',b'test result: ok. 101 passed;',1)]:
            with self.assertRaises(ValueError):s.classify(bad)
    def test_duplicate_or_missing_pass_listing_rejected(self):
        line=next(x for x in self.raw.splitlines(True) if x.startswith(b'test ') and x.endswith(b' ... ok\n'))
        for bad in [self.raw+line,self.raw.replace(line,b'',1)]:
            with self.assertRaisesRegex(ValueError,'pass listings'):s.classify(bad)
    def test_optimized_checker_exits_before_sealing(self):
        r=subprocess.run([sys.executable,'-O',str(P/'suite-verification.py'),'seal'],capture_output=True)
        self.assertNotEqual(r.returncode,0);self.assertIn(b'Optimized Python forbidden',r.stderr)

class ApprovalScope(unittest.TestCase):
    def test_frozen_authorization_scope_rejects_other_issue_and_nonaffirmative_decisions(self):
        q=load('workflow.py');prod='sha256:'+'a'*64;rep='sha256:'+'b'*64
        import json
        body=lambda d:'```json\n'+json.dumps(d)+'\n```'
        good={'decision':'approve','scope':'jpeg188-icc','production_subject_sha256':prod,'representative_subject_sha256':rep}
        q.affirmative(body(good),prod,rep)
        for bad in [dict(good,scope='jpeg187-color-only'),dict(good,decision='reject'),dict(good,extra=True)]:
            with self.assertRaises(ValueError):q.affirmative(body(bad),prod,rep)
        with self.assertRaises(ValueError):q.affirmative('Reject '+prod+' '+rep,prod,rep)

if __name__=='__main__':unittest.main(verbosity=2)
