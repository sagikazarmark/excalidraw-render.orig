#!/usr/bin/env python3
"""Isolated exact composition/migration/auth and reused strict receipt tests."""
import copy
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest

P=Path(__file__).resolve().parent
def workflow():
    q=types.ModuleType('webp_fixture');q.__file__=str(P/'workflow.py')
    exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec',optimize=0),q.__dict__);return q

class MeasurementTests(unittest.TestCase):
    def setUp(self):
        self.q=workflow();self.old=self.q.read(self.q.ROOT/self.q.NATIVE_OLD);self.new=self.q.read(self.q.ROOT/self.q.NATIVE_NEW);self.ref=self.q.read(self.q.ROOT/self.q.REFERENCE)
    def test_exact_30_vector_60_record_delta_and_partial_residuals(self):
        result=self.q.measurements(self.old,self.new,self.ref)
        self.assertEqual((len(result['changed_vectors']),len(result['changed_raster_records']),len(result['improved_but_failing_partial_vectors'])),(30,60,8))
        self.assertEqual(result['full_corpus_passes'],{'svg':72,'png':48,'pdf':48});self.assertEqual(result['exact_failed_views'],72)
    def test_partial_success_cannot_be_promoted(self):
        new=copy.deepcopy(self.new);row=next(r for r in new['cases'] if r['cell'] in self.q.RESIDUAL and 'blend' in r['cell']);row['formats'][1]['requirement_passed']=True
        with self.assertRaisesRegex(ValueError,'residual'):self.q.measurements(self.old,new,self.ref)
    def test_static_and_svg_records_cannot_move(self):
        for static in [False,True]:
            new=copy.deepcopy(self.new);row=next(r for r in new['cases'] if ('static-full' in r['cell'])==static)
            if static:row['formats'][1]['actual_rgba_sha256']='changed'
            else:row['formats'][0]['diagnostic_sha256']='changed'
            with self.assertRaises(ValueError):self.q.measurements(self.old,new,self.ref)
    def test_reference_or_alpha_invariant_cannot_change(self):
        for key in ['expected_rgba_sha256','alpha']:
            new=copy.deepcopy(self.new);fmt=new['cases'][0]['formats'][1]
            if key=='alpha':fmt['alpha']['expected_sha256']='changed'
            else:fmt['views'][0][key]='changed'
            with self.assertRaises(ValueError):self.q.measurements(self.old,new,self.ref)
    def test_residual_failure_cannot_be_replaced_by_tolerance(self):
        new=copy.deepcopy(self.new);row=next(r for r in new['cases'] if r['cell'] in self.q.RESIDUAL and 'blend' in r['cell']);view=next(v for v in row['formats'][1]['views'] if not v['requirement_passed'])
        view['canonical']['ssim']={'kind':'score','value':0.9}
        with self.assertRaisesRegex(ValueError,'exact small-artifact'):self.q.measurements(self.old,new,self.ref)

class MigrationTests(unittest.TestCase):
    def setUp(self):
        self.q=workflow();self.before=self.q.read(P/'before'/self.q.LEDGER);self.registry=self.q.read(P/'before'/self.q.REGISTRY)
        self.mapping={key:self.q.NEW_SELECTOR for key in self.q.GAPS}
        scopes=copy.deepcopy(self.registry['cases'][0]['exceptions'])
        for e in scopes:e['case_id']='representative.stage4-frames-text-assets.v13'
        self.candidate={'case_id':'representative.stage4-frames-text-assets.v13','approval':None,'exception_proposal':{'exceptions':scopes}}
    def derive(self,before=None,mapping=None,candidate=None):return self.q.expected_transaction(before or self.before,self.registry,'a'*64,mapping or self.mapping,candidate or self.candidate,'sha256:'+'b'*64,'c'*64)
    def test_exact_11_new_25_refresh_one_rep_other_criteria_and_residuals_unchanged(self):
        new,out=self.derive();added=[];refresh=[];changed=[]
        for a,b in zip(self.before['assignments'],new['assignments']):
            self.assertEqual(a['id'],b['id'])
            if a!=b:
                changed.append(a['id'])
                if a['id'].startswith(self.q.PREFIX):
                    if a['disposition']['status']=='gap':added.append(a['id'])
                    else:
                        refresh.append(a['id']);restored=copy.deepcopy(b);restored['disposition']['owner']['source']=a['disposition']['owner']['source'];self.assertEqual(restored,a)
            if a['id'] in {self.q.PREFIX+c for c in self.q.RESIDUAL}:self.assertEqual(a,b)
        self.assertEqual((len(changed),len(added),len(refresh)),(37,11,25));self.assertEqual(set(added),self.q.GAPS)
        self.assertEqual(new['catalog_sha256'],self.before['catalog_sha256']);self.assertIsNone(new['approval'])
        self.assertEqual({s:sum(r['disposition']['status']==s for r in new['assignments']) for s in ['owned','gap','excluded']},{'owned':2318,'gap':97,'excluded':3})
        self.assertEqual([r for r in new['assignments'] if r['disposition']['status']=='excluded'],[r for r in self.before['assignments'] if r['disposition']['status']=='excluded'])
    def test_partial_cell_cannot_enter_owner_map(self):
        mapping=dict(self.mapping);mapping[self.q.PREFIX+next(iter(self.q.RESIDUAL))]=self.q.NEW_SELECTOR
        with self.assertRaises(ValueError):self.derive(mapping=mapping)
    def test_old_selector_and_gap_issue_preserved(self):
        before=copy.deepcopy(self.before);row=next(r for r in before['assignments'] if r['id'].startswith(self.q.PREFIX) and r['disposition']['status']=='owned');row['disposition']['owner']['command'].append('--ignored')
        with self.assertRaises(ValueError):self.derive(before=before)
        before=copy.deepcopy(self.before);next(r for r in before['assignments'] if r['id'] in self.q.GAPS)['disposition']['issue']=190
        with self.assertRaises(ValueError):self.derive(before=before)
    def test_no_current_capture_without_explicit_freeze(self):
        with tempfile.TemporaryDirectory(dir='/tmp/opencode') as d:
            self.q.P=Path(d)
            with self.assertRaisesRegex(ValueError,'final-freeze'):self.q.representative()
            self.assertEqual(list(Path(d).iterdir()),[])
    def test_representative_scope_cannot_be_loosened(self):
        candidate=copy.deepcopy(self.candidate);candidate['exception_proposal']['exceptions'][0]['max_changed_pixels']+=1
        with self.assertRaisesRegex(ValueError,'scope changed'):self.derive(candidate=candidate)

class AuthTests(unittest.TestCase):
    def test_only_affirmative_composition_scope(self):
        q=workflow();p='sha256:'+'a'*64;r='sha256:'+'b'*64;good={'decision':'approve','scope':'webp191-composition','production_subject_sha256':p,'representative_subject_sha256':r};body=lambda d:'```json\n'+json.dumps(d)+'\n```'
        q.affirmative(body(good),p,r)
        for bad in [dict(good,decision='reject'),dict(good,scope='webp191-alpha'),dict(good,extra=True)]:
            with self.assertRaises(ValueError):q.affirmative(body(bad),p,r)
        for text in [body(good)*2,'reject '+p+' '+r,body(good).replace('"approve"','"reject", "decision":"approve"')]:
            with self.assertRaises(ValueError):q.affirmative(text,p,r)
    def test_designated_maintainer_required(self):
        q=workflow().authentication_helper();a={'approved_by':'other','approved_at':'2026-09-21','approval_url':'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-123','approval_subject_sha256':'sha256:'+'a'*64}
        with self.assertRaisesRegex(ValueError,'designated maintainer'):q.authenticate(a,{'user':{'login':'other'}},a['approval_subject_sha256'],'sha256:'+'b'*64,{})
    def test_optimized_startup_refused(self):
        for name in ['workflow.py','receipt_gate.py']:
            r=subprocess.run([sys.executable,'-O',str(P/name),'archive'],capture_output=True)
            self.assertNotEqual(r.returncode,0);self.assertIn(b'Optimized Python forbidden',r.stderr)

def receipt_tests():
    q=workflow();path=q.ROOT/'tests/conformance/proposals/jpeg188-requalification-v2/test_verify.py';data=path.read_bytes()
    q.require(data==q.run(['git','show',q.read(P/'basis.json')['base_commit']+':'+str(path.relative_to(q.ROOT))]),'original receipt tests changed')
    text=data.decode().replace('verified 1992 ownership assignments: 1915 assigned/excluded, 77 explicit gaps; ledger approval pending','verified 2418 ownership assignments: 2321 assigned/excluded, 97 explicit gaps; ledger approval pending')
    m=types.ModuleType('authenticated_receipt_tests');m.__file__=str(path);exec(compile(text,str(path),'exec',optimize=0),m.__dict__)
    def factory():
        w=workflow();g=w.receipt_gate();g.REP='sha256:'+'d'*64;g.test_inventory=w.helper('receipts').test_inventory;g.load_original=lambda name:w.authentication_helper();return g
    m.module=factory
    return unittest.TestSuite([unittest.defaultTestLoader.loadTestsFromTestCase(m.Receipts),unittest.defaultTestLoader.loadTestsFromTestCase(m.TransactionTests)])

if __name__=='__main__':
    suite=unittest.TestSuite([unittest.defaultTestLoader.loadTestsFromTestCase(c) for c in [MeasurementTests,MigrationTests,AuthTests]]+[receipt_tests()])
    result=unittest.TextTestRunner(verbosity=2).run(suite);raise SystemExit(not result.wasSuccessful())
