#!/usr/bin/env python3
"""Strict result classification and scope tests; no full-workspace rerun."""
from pathlib import Path
import json
import subprocess
import sys
import types
import unittest
P=Path(__file__).resolve().parent
s=types.ModuleType('suite_tests');s.__file__=str(P/'suite_gate.py')
exec(compile((P/'suite_gate.py').read_bytes(),s.__file__,'exec'),s.__dict__)

class SuiteTests(unittest.TestCase):
    def setUp(self):self.raw=(P/'checks/full-workspace.raw.log').read_bytes();self.p=s.parser()
    def test_actual_totals_and_exact_five_failures(self):
        r=self.p.classify(self.raw);self.assertEqual((r['passed'],r['failed'],r['ignored']),(1257,5,19));self.assertEqual(r['renderer_fast_passed'],626);self.assertFalse(r['all_tests_green'])
    def test_unknown_same_count_failure_rejected(self):
        identity=next(iter(self.p.EXPECTED)).encode()
        with self.assertRaisesRegex(ValueError,'identity'):self.p.classify(self.raw.replace(identity,b'unknown::regression'))
    def test_changed_failure_message_rejected(self):
        for old,new in [(b'"stale ownership evidence"',b'"renderer error"'),(b'error: stale adapter production/source bindings',b'error: unexpected artifact')]:
            with self.assertRaisesRegex(ValueError,'message/body'):self.p.classify(self.raw.replace(old,new,1))
    def test_extra_error_or_incomplete_trailer_rejected(self):
        for raw in [self.raw+b'\nerror: hidden failure\n',self.raw.split(b'error: 3 targets failed:')[0]]:
            with self.assertRaises(ValueError):self.p.classify(raw)
    def test_count_pass_list_or_green_relabel_rejected(self):
        line=next(x for x in self.raw.splitlines(True) if x.startswith(b'test ') and x.endswith(b' ... ok\n'))
        for raw in [self.raw.replace(line,b'',1),self.raw+line,self.raw.replace(b'626 passed;',b'625 passed;',1),self.raw.replace(b'test result: FAILED. 101 passed;',b'test result: ok. 101 passed;',1)]:
            with self.assertRaises(ValueError):self.p.classify(raw)
    def test_only_standard_backtrace_note_is_nonsemantic(self):
        note=self.p.NOTE.encode()
        self.assertEqual(self.p.classify(self.raw.replace(note+b'\n',b'')),self.p.classify(self.raw))
        with self.assertRaises(ValueError):self.p.classify(self.raw.replace(note,b'note: actual failure omitted',1))
    def test_composition_authorization_does_not_cover_partial_alpha(self):
        q=s.frozen_workflow();prod='sha256:'+'a'*64;rep='sha256:'+'b'*64
        good={'decision':'approve','scope':'webp191-composition','production_subject_sha256':prod,'representative_subject_sha256':rep}
        body=lambda v:'```json\n'+json.dumps(v)+'\n```'
        q.affirmative(body(good),prod,rep)
        for bad in [dict(good,scope='webp191-alpha'),dict(good,decision='reject'),dict(good,extra=True)]:
            with self.assertRaises(ValueError):q.affirmative(body(bad),prod,rep)
    def test_receipt_gate_still_requires_109_names_and_six_commands(self):
        g=s.gate();self.assertEqual([r['count'] for r in g.test_inventory()],[103,5,1]);self.assertEqual(len(g.COMMANDS),6)
        self.assertEqual(g.CASE,'representative.stage4-frames-text-assets.v13')
        self.assertEqual(g.TEST_COMMAND,['cargo','test','--locked','-p','xtask','--bin','xtask','--test','coverage','--test','representative_candidate','--no-fail-fast'])
    def test_optimized_entry_refuses_before_seal(self):
        r=subprocess.run([sys.executable,'-O',str(P/'suite_gate.py'),'seal'],capture_output=True)
        self.assertNotEqual(r.returncode,0);self.assertIn(b'Optimized Python forbidden',r.stderr)

if __name__=='__main__':unittest.main(verbosity=2)
