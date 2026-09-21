#!/usr/bin/env python3
"""Strict log classification tests, no production test execution or live edits."""
from pathlib import Path
import subprocess
import sys
import types
import unittest
P=Path(__file__).resolve().parent
s=types.ModuleType('suite_fixture');s.__file__=str(P/'suite_gate.py')
exec(compile((P/'suite_gate.py').read_bytes(),s.__file__,'exec'),s.__dict__)

class SuiteChecks(unittest.TestCase):
    def setUp(self):self.raw=(P/'checks/full-workspace.raw.log').read_bytes();self.parser=s.parser()
    def test_actual_counts_and_exact_failures(self):
        result=self.parser.classify(self.raw)
        self.assertEqual((result['passed'],result['failed'],result['ignored']),(1232,5,19))
        self.assertEqual((result['renderer_fast_passed'],result['renderer_library_passed']),(601,404))
        self.assertFalse(result['all_tests_green'])
    def test_unknown_identity_even_same_count_rejected(self):
        name=next(iter(self.parser.EXPECTED)).encode()
        with self.assertRaisesRegex(ValueError,'identity'):self.parser.classify(self.raw.replace(name,b'unknown::regression'))
    def test_changed_error_body_rejected(self):
        for old,new in [(b'"stale ownership evidence"',b'"actual regression"'),(b'error: stale adapter production/source bindings',b'error: invalid output')]:
            with self.assertRaisesRegex(ValueError,'message/body'):self.parser.classify(self.raw.replace(old,new,1))
    def test_only_exact_nonsemantic_backtrace_note_can_move(self):
        note=self.parser.NOTE.encode()
        without=self.raw.replace(note+b'\n',b'')
        self.assertEqual(self.parser.classify(without),self.parser.classify(self.raw))
        with self.assertRaises(ValueError):self.parser.classify(self.raw.replace(note,b'note: hidden renderer failure',1))
    def test_extra_error_or_truncation_rejected(self):
        for raw in [self.raw+b'\nerror: extra failure\n',self.raw.split(b'error: 3 targets failed:')[0]]:
            with self.assertRaises(ValueError):self.parser.classify(raw)
    def test_counts_or_pass_list_cannot_be_forged(self):
        line=next(x for x in self.raw.splitlines(True) if x.startswith(b'test ') and x.endswith(b' ... ok\n'))
        for raw in [self.raw.replace(line,b'',1),self.raw+line,self.raw.replace(b'601 passed;',b'600 passed;',1),self.raw.replace(b'test result: FAILED. 101 passed;',b'test result: ok. 101 passed;',1)]:
            with self.assertRaises(ValueError):self.parser.classify(raw)
    def test_receipt_gate_retains_exact_109_ids_and_six_commands(self):
        gate=s.gate()
        self.assertEqual([r['count'] for r in gate.test_inventory()],[103,5,1]);self.assertEqual(len(gate.COMMANDS),6)
        self.assertEqual(gate.CASE,'representative.stage4-frames-text-assets.v11')
        self.assertEqual(gate.TEST_COMMAND,['cargo','test','--locked','-p','xtask','--bin','xtask','--test','coverage','--test','representative_candidate','--no-fail-fast'])
    def test_auth_scope_stays_exif_and_requires_affirmative(self):
        import json
        q=s.frozen_workflow();prod='sha256:'+'a'*64;rep='sha256:'+'b'*64
        good={'decision':'approve','scope':'jpeg189-exif','production_subject_sha256':prod,'representative_subject_sha256':rep}
        body=lambda x:'```json\n'+json.dumps(x)+'\n```'
        q.affirmative(body(good),prod,rep)
        for bad in [dict(good,decision='reject'),dict(good,scope='jpeg188-icc'),dict(good,extra=True)]:
            with self.assertRaises(ValueError):q.affirmative(body(bad),prod,rep)
    def test_optimized_refusal_precedes_seal(self):
        r=subprocess.run([sys.executable,'-O',str(P/'suite_gate.py'),'seal'],capture_output=True)
        self.assertNotEqual(r.returncode,0);self.assertIn(b'Optimized Python forbidden',r.stderr)

if __name__=='__main__':unittest.main(verbosity=2)
