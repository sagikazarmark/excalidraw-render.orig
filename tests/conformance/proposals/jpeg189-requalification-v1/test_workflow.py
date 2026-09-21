#!/usr/bin/env python3
"""Isolated EXIF migration, affirmative auth and receipt-adapter regression tests."""
import copy
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest

P=Path(__file__).resolve().parent
def workflow():
    q=types.ModuleType('fixture_workflow');q.__file__=str(P/'workflow.py')
    exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec',optimize=0),q.__dict__)
    return q

class MigrationTests(unittest.TestCase):
    def setUp(self):
        self.q=workflow();self.before=self.q.read(P/'before'/self.q.LEDGER);self.registry=self.q.read(P/'before'/self.q.REGISTRY)
        self.mapping={key:'corrected_exif_precedence_matches_reference' for key in self.q.gap_ids()}
        scopes=copy.deepcopy(self.registry['cases'][0]['exceptions'])
        for e in scopes:e['case_id']='representative.stage4-frames-text-assets.v11'
        self.candidate={'case_id':'representative.stage4-frames-text-assets.v11','approval':None,'exception_proposal':{'exceptions':scopes}}
    def derive(self,before=None,mapping=None,candidate=None):
        return self.q.expected_transaction(before or self.before,self.registry,'a'*64,mapping or self.mapping,candidate or self.candidate,'sha256:'+'b'*64,'c'*64)
    def test_exact_36_additions_46_refresh_one_representative(self):
        new,registry=self.derive();changed=[];added=[];refresh=[]
        for a,b in zip(self.before['assignments'],new['assignments']):
            self.assertEqual(a['id'],b['id'])
            if a!=b:
                changed.append(a['id'])
                if a['id'].startswith(self.q.PREFIX):
                    if a['disposition']['status']=='gap':added.append(a['id'])
                    else:
                        refresh.append(a['id']);normalized=copy.deepcopy(b)
                        normalized['disposition']['owner']['source']=a['disposition']['owner']['source']
                        self.assertEqual(normalized,a)
            if a['id'].startswith(('codec:jpeg:icc:','codec:jpeg:color-sampling:')):self.assertEqual(a,b)
        self.assertEqual((len(changed),len(added),len(refresh)),(83,36,46))
        self.assertEqual(set(added),self.q.gap_ids())
        self.assertEqual({s:sum(r['disposition']['status']==s for r in new['assignments']) for s in ['owned','gap','excluded']},{'owned':1994,'gap':77,'excluded':3})
        self.assertEqual(new['catalog_sha256'],self.before['catalog_sha256']);self.assertIsNone(new['approval'])
        self.assertEqual([r for r in new['assignments'] if r['disposition']['status']=='excluded'],[r for r in self.before['assignments'] if r['disposition']['status']=='excluded'])
        self.assertEqual(len(registry['cases'][0]['execution_keys']),3)
    def test_gap_scope_or_issue_substitution_rejected(self):
        before=copy.deepcopy(self.before);r=next(r for r in before['assignments'] if r['id'] in self.q.gap_ids());r['disposition']['issue']=188
        with self.assertRaisesRegex(ValueError,'only #189'):self.derive(before=before)
        before=copy.deepcopy(self.before);next(r for r in before['assignments'] if r['id'].startswith(self.q.PREFIX))['id']=self.q.PREFIX+'wrong'
        with self.assertRaisesRegex(ValueError,'82 exact'):self.derive(before=before)
    def test_old_owner_selector_or_command_cannot_change(self):
        before=copy.deepcopy(self.before);r=next(r for r in before['assignments'] if r['id'].startswith(self.q.PREFIX) and r['disposition']['status']=='owned')
        r['disposition']['owner']['command'].append('--ignored')
        with self.assertRaisesRegex(ValueError,'selector/command'):self.derive(before=before)
    def test_new_owner_requires_qualified_unconditional_selector(self):
        self.q.validate_selectors(self.mapping,'#[test]\nfn corrected_exif_precedence_matches_reference() {}')
        for source in ['fn corrected_exif_precedence_matches_reference() {}','#[test]\n#[ignore]\nfn corrected_exif_precedence_matches_reference() {}']:
            with self.assertRaises(ValueError):self.q.validate_selectors(self.mapping,source)
        mapping=dict(self.mapping);mapping[next(iter(mapping))]='known_failures'
        with self.assertRaisesRegex(ValueError,'not qualified'):self.derive(mapping=mapping)
    def test_representative_tolerance_change_rejected(self):
        c=copy.deepcopy(self.candidate);c['exception_proposal']['exceptions'][0]['max_changed_pixels']+=1
        with self.assertRaisesRegex(ValueError,'scopes changed'):self.derive(candidate=c)
    def test_current_capture_needs_explicit_freeze(self):
        with tempfile.TemporaryDirectory(dir='/tmp/opencode') as d:
            self.q.P=Path(d)
            with self.assertRaisesRegex(ValueError,'final-freeze'):self.q.representative()
            self.assertEqual(list(Path(d).iterdir()),[])
    def test_receipt_adapter_preserves_exact_bounded_commands_and_counts(self):
        gate=self.q.receipt_gate()
        self.assertEqual(gate.TEST_COMMAND,['cargo','test','--locked','-p','xtask','--bin','xtask','--test','coverage','--test','representative_candidate','--no-fail-fast'])
        self.assertEqual(len(gate.COMMANDS),6)
        self.assertEqual(gate.V10,self.q.V11)
        self.assertIn(self.q.V11,gate.COMMANDS[1][1])
        self.assertEqual(gate.outcome('ownership',b'verified 2074 ownership assignments: 1997 assigned/excluded, 77 explicit gaps; ledger approval pending\n',b''),{'verified_output':'verified 2074 ownership assignments: 1997 assigned/excluded, 77 explicit gaps; ledger approval pending\n'})
        with self.assertRaises(ValueError):gate.outcome('ownership',b'verified 1992 ownership assignments: 1915 assigned/excluded, 77 explicit gaps; ledger approval pending\n',b'')

class ApprovalTests(unittest.TestCase):
    def setUp(self):
        self.q=workflow();self.prod='sha256:'+'a'*64;self.rep='sha256:'+'b'*64
    def test_only_exact_affirmative_exif_scope(self):
        good={'decision':'approve','scope':'jpeg189-exif','production_subject_sha256':self.prod,'representative_subject_sha256':self.rep}
        body=lambda d:'```json\n'+json.dumps(d)+'\n```'
        self.q.affirmative(body(good),self.prod,self.rep)
        for bad in [dict(good,scope='jpeg188-icc'),dict(good,decision='reject'),dict(good,extra=True)]:
            with self.assertRaises(ValueError):self.q.affirmative(body(bad),self.prod,self.rep)
        for text in ['Reject '+self.prod+' '+self.rep,body(good)*2,body(good).replace('"approve"','"reject", "decision":"approve"')]:
            with self.assertRaises(ValueError):self.q.affirmative(text,self.prod,self.rep)
    def test_actual_maintainer_required_by_reused_auth(self):
        q=self.q.authentication_helper()
        a={'approved_by':'other','approved_at':'2026-09-21','approval_url':'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-123456789','approval_subject_sha256':self.prod}
        c={'user':{'login':'other'}}
        with self.assertRaisesRegex(ValueError,'designated maintainer'):q.authenticate(a,c,self.prod,self.rep,dict(a,approval_subject_sha256=self.rep))
    def test_optimized_entry_rejects_before_mutation(self):
        for name in ['workflow.py','receipt_gate.py']:
            r=subprocess.run([sys.executable,'-O',str(P/name),'archive'],capture_output=True)
            self.assertNotEqual(r.returncode,0);self.assertIn(b'Optimized Python forbidden',r.stderr)

def reused_receipt_suite():
    # Execute the previous reviewed 17 receipt/rollback tests with the new runtime
    # adapter and exact new ownership output; original approved source untouched.
    q=workflow();path=q.ROOT/'tests/conformance/proposals/jpeg188-requalification-v2/test_verify.py'
    raw=path.read_bytes();expected=q.run(['git','show',q.read(P/'basis.json')['base_commit']+':'+str(path.relative_to(q.ROOT))])
    q.require(raw==expected,'reviewed previous tests changed')
    text=raw.decode().replace('verified 1992 ownership assignments: 1915 assigned/excluded, 77 explicit gaps; ledger approval pending','verified 2074 ownership assignments: 1997 assigned/excluded, 77 explicit gaps; ledger approval pending')
    m=types.ModuleType('reused_receipt_tests');m.__file__=str(path)
    exec(compile(text,str(path),'exec',optimize=0),m.__dict__)
    def factory():
        w=workflow();gate=w.receipt_gate()
        gate.REP='sha256:'+'d'*64  # isolated receipt fixture identity, never live evidence
        original=w.helper('receipts')
        # The exact same 109 target identities remain authenticated by their old
        # source log until parent freezes/re-lists xtask; no tests are added there.
        gate.test_inventory=original.test_inventory
        gate.load_original=lambda name:w.authentication_helper()
        return gate
    m.module=factory
    # The previous scope-specific test exercises old approval wording, so retain
    # its original function factory only for that one scope test, separately
    # covered for jpeg189-exif above; receipt and rollback tests use the adapter.
    suite=unittest.TestSuite()
    for cls in [m.Receipts,m.TransactionTests]:suite.addTests(unittest.defaultTestLoader.loadTestsFromTestCase(cls))
    return suite

if __name__=='__main__':
    suite=unittest.TestSuite([unittest.defaultTestLoader.loadTestsFromTestCase(MigrationTests),unittest.defaultTestLoader.loadTestsFromTestCase(ApprovalTests),reused_receipt_suite()])
    result=unittest.TextTestRunner(verbosity=2).run(suite);raise SystemExit(not result.wasSuccessful())
