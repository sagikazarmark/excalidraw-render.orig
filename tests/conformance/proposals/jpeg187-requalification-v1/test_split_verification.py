#!/usr/bin/env python3
"""Verify strict split-result interpretation without rerunning production tests."""
import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import types
import unittest

P=Path(__file__).resolve().parent
s=types.ModuleType('split_fixture');s.__file__=str(P/'split-verification.py')
exec(compile((P/'split-verification.py').read_bytes(),s.__file__,'exec',optimize=0),s.__dict__)

class SplitChecks(unittest.TestCase):
    def setUp(self):
        self.raw=(P/'final-checks/workspace-timeout.raw.log').read_bytes()
        self.a=json.loads((P/'final-checks/parent-split-verification.json').read_bytes())
    def test_actual_combined_record_passes(self):
        s.validate_attestation(self.a,s.parse_workspace(self.raw))
    def test_removed_or_duplicated_fast_success_rejected(self):
        text=self.raw.decode(); line=next(v for v in text.splitlines(True) if v.startswith('test ') and 'whole_scene_controls::' in v and ' ... ok' in v)
        for value in [text.replace(line,'',1),text+line]:
            with self.assertRaisesRegex(ValueError,'584 unique'):s.parse_workspace(value.encode())
    def test_timeout_cannot_be_relabelled_pass(self):
        self.a['initial']['outcome']='passed'
        with self.assertRaisesRegex(ValueError,'timeout'):s.validate_attestation(self.a,s.parse_workspace(self.raw))
    def test_pending_failure_cannot_be_waived_or_expanded(self):
        for mutation in ['passed','six','reason','all-green']:
            a=copy.deepcopy(self.a)
            if mutation=='passed':a['remaining_workspace']['outcome']='passed'
            elif mutation=='six':a['remaining_workspace']['failure_count']=6
            elif mutation=='reason':a['remaining_workspace']['expected_failures'][0]['reason']='arbitrary production regression'
            else:a['all_tests_green_before_approval']=True
            with self.assertRaises(ValueError):s.validate_attestation(a,s.parse_workspace(self.raw))
    def test_long_test_continuation_must_be_exact(self):
        for mutation in ['missing','different']:
            a=copy.deepcopy(self.a)
            if mutation=='missing':a['long_test_continuation']['outcome']='not-run'
            else:a['long_test_continuation']['command'][-3]='other::test'
            with self.assertRaises(ValueError):s.validate_attestation(a,s.parse_workspace(self.raw))
    def test_no_invented_continuation_raw_log(self):
        self.a['long_test_continuation']['evidence_kind']='raw-log'
        with self.assertRaisesRegex(ValueError,'invent'):s.validate_attestation(self.a,s.parse_workspace(self.raw))
    def test_optimized_execution_stops_before_seal(self):
        result=subprocess.run([sys.executable,'-O',str(P/'split-verification.py'),'seal'],capture_output=True,env=dict(os.environ,PYTHONDONTWRITEBYTECODE='1'))
        self.assertNotEqual(result.returncode,0);self.assertIn(b'Optimized Python forbidden',result.stderr)

if __name__=='__main__':unittest.main(verbosity=2)
