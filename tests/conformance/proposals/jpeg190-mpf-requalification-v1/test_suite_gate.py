#!/usr/bin/env python3
"""Strict suite/policy/scope checks and isolated actual four-file rollback."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest

P=Path(__file__).resolve().parent
s=types.ModuleType('mpf_suite_tests');s.__file__=str(P/'suite_gate.py')
exec(compile((P/'suite_gate.py').read_bytes(),s.__file__,'exec'),s.__dict__)

class SuiteTests(unittest.TestCase):
    def setUp(self):self.raw=(P/'checks/full-workspace.raw.log').read_bytes();self.parser=s.parser()
    def test_actual_complete_result_exact_pending_failures(self):
        r=self.parser.classify(self.raw)
        self.assertEqual((r['passed'],r['failed'],r['ignored']),(1244,5,19));self.assertEqual(r['renderer_fast_passed'],613);self.assertFalse(r['all_tests_green'])
    def test_same_count_unknown_failure_rejected(self):
        old=next(iter(self.parser.EXPECTED)).encode()
        with self.assertRaisesRegex(ValueError,'identity'):self.parser.classify(self.raw.replace(old,b'unknown::regression'))
    def test_changed_body_or_extra_error_rejected(self):
        for raw in [self.raw.replace(b'"stale ownership evidence"',b'"unexpected production error"',1),self.raw+b'\nerror: hidden failure\n']:
            with self.assertRaises(ValueError):self.parser.classify(raw)
    def test_only_exact_nonsemantic_note_ignored(self):
        note=self.parser.NOTE.encode()
        self.assertEqual(self.parser.classify(self.raw.replace(note+b'\n',b'')),self.parser.classify(self.raw))
        with self.assertRaises(ValueError):self.parser.classify(self.raw.replace(note,b'note: actual failure details',1))
    def test_missing_completion_count_or_pass_listing_rejected(self):
        line=next(x for x in self.raw.splitlines(True) if x.startswith(b'test ') and x.endswith(b' ... ok\n'))
        for raw in [self.raw.split(b'error: 3 targets failed:')[0],self.raw.replace(b'613 passed;',b'612 passed;',1),self.raw.replace(line,b'',1),self.raw+line]:
            with self.assertRaises(ValueError):self.parser.classify(raw)
    def test_receipt_targets_and_four_file_template_exact(self):
        q=s.frozen_workflow();gate=q.receipt_gate()
        self.assertEqual([r['count'] for r in gate.test_inventory()],[103,5,1]);self.assertEqual(len(gate.COMMANDS),6)
        before=q.read(P/'before'/q.POLICY);after=q.proposed_policy(before)
        changed=[a['id'] for a,b in zip(before['rows'],after['rows']) if a!=b]
        self.assertEqual(set(changed),q.cell_sets()[1]);self.assertEqual(len(changed),16)
        with tempfile.TemporaryDirectory(dir='/tmp/opencode') as d:
            q.ROOT=Path(d);q.P=q.ROOT/'packet';q.P.mkdir()
            v=q.ROOT/q.V12;v.mkdir(parents=True)
            (v/'manifest.json').write_text(json.dumps({'approval':None}))
            pending=q.P/'pending';pending.mkdir()
            for name in ['registry.json','ownership.template.json','ownership-policy.json']:(pending/name).write_bytes((P/'pending'/name).read_bytes())
            (q.P/'migration-plan.json').write_bytes((P/'migration-plan.json').read_bytes())
            files=q.transaction({'approved_by':'fixture-only','approved_at':'2026-09-21','approval_url':'fixture','approval_subject_sha256':'sha256:'+'a'*64})
            self.assertEqual(set(files),{q.V12+'/manifest.json',q.REGISTRY,q.POLICY,q.LEDGER})
            self.assertEqual(files[q.POLICY],(P/'pending/ownership-policy.json').read_bytes())
    def test_unrelated_policy_or_hdr_criterion_rejected(self):
        q=s.frozen_workflow();old=q.read(P/'policy/before-catalog.json');after=q.read(P/'pending/catalog.json');plan=q.read(P/'policy/criterion-transition.json')
        target=next(r for r in after['obligations'] if r['id'] in q.cell_sets()[2]);target['criterion']='HDR now qualifies'
        with self.assertRaises(ValueError):q.verify_catalog_delta(old,after,plan['before_policy_sha256'],plan['proposed_policy_sha256'])
    def test_authentication_is_only_affirmative_mpf_scope(self):
        q=s.frozen_workflow();prod='sha256:'+'a'*64;rep='sha256:'+'b'*64
        good={'decision':'approve','scope':'jpeg190-mpf','production_subject_sha256':prod,'representative_subject_sha256':rep};body=lambda d:'```json\n'+json.dumps(d)+'\n```'
        q.affirmative(body(good),prod,rep)
        for bad in [dict(good,scope='jpeg190-hdr'),dict(good,decision='reject'),dict(good,extra=True)]:
            with self.assertRaises(ValueError):q.affirmative(body(bad),prod,rep)
    def test_failed_receipt_run_restores_all_four_files_including_policy(self):
        q=s.frozen_workflow();gate=q.receipt_gate()
        with tempfile.TemporaryDirectory(dir='/tmp/opencode',prefix='mpf-four-file-rollback-') as d:
            root=Path(d);gate.ROOT=root
            paths=[q.V12+'/manifest.json',q.REGISTRY,q.POLICY,q.LEDGER]
            before={p:('before '+p).encode() for p in paths};after={p:('after '+p).encode() for p in paths}
            for path,data in before.items():
                dest=root/path;dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data)
            approval=root/'fixture.json';approval.write_text(json.dumps({'approved_by':'fixture-only','approved_at':'2026-09-21','approval_url':'fixture','approval_subject_sha256':'sha256:'+'a'*64}))
            gate.verify=lambda applied=False:'sha256:'+'a'*64
            gate.transaction=lambda a:after
            gate.load_original=lambda name:types.SimpleNamespace(fetch_comment=lambda url:{'fixture':True},authenticate=lambda *args:None,verify_migration=lambda applied:None)
            gate.evidence=lambda applied:None
            def fail(subject,app):
                self.assertTrue((app/'approval.json').exists() and (app/'comment.json').exists())
                self.assertEqual((app/'before'/q.POLICY).read_bytes(),before[q.POLICY])
                self.assertEqual((root/q.POLICY).read_bytes(),after[q.POLICY])
                gate.put(app/'logs/partial.stdout',b'preserved failed attempt')
                raise ValueError('isolated receipt failure')
            gate.run_required=fail
            with self.assertRaisesRegex(ValueError,'isolated receipt failure'):gate.apply(str(approval))
            for path,data in before.items():self.assertEqual((root/path).read_bytes(),data)
            app=root/gate.APP;self.assertEqual(gate.read(app/'status.json')['state'],'failed_rolled_back')
            self.assertEqual(set(gate.read(app/'status.json')['before_sha256']),set(paths))
            self.assertEqual((app/'logs/partial.stdout').read_bytes(),b'preserved failed attempt')
            with self.assertRaisesRegex(ValueError,'existing attempt'):gate.apply(str(approval))
    def test_optimized_startup_refuses_before_seal(self):
        r=subprocess.run([sys.executable,'-O',str(P/'suite_gate.py'),'seal'],capture_output=True)
        self.assertNotEqual(r.returncode,0);self.assertIn(b'Optimized Python forbidden',r.stderr)

if __name__=='__main__':unittest.main(verbosity=2)
