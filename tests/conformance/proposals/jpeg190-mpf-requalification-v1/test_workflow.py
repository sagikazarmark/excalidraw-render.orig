#!/usr/bin/env python3
"""Isolated exact policy/owner/HDR preservation and real-receipt negative checks."""
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
    q=types.ModuleType('mpf_fixture');q.__file__=str(P/'workflow.py')
    exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec',optimize=0),q.__dict__)
    return q

class PolicyTests(unittest.TestCase):
    def setUp(self):
        self.q=workflow();self.before=self.q.read(P/'before'/self.q.POLICY)
        self.old_catalog=self.q.read(P/'policy/before-catalog.json');self.new_catalog=self.q.read(P/'pending/catalog.json')
        self.plan=self.q.read(P/'policy/criterion-transition.json')
    def test_only_sixteen_explicit_recovery_criteria_change(self):
        after=self.q.proposed_policy(self.before);_,index,hdr=self.q.cell_sets();changes=[]
        for a,b in zip(self.before['rows'],after['rows']):
            if a!=b:
                changes.append(a['id']);self.assertEqual({**b,'criterion':a['criterion']},a)
                self.assertIn('image.decode-failed',b['criterion']);self.assertIn('without claiming canonical visual equivalence',b['criterion'])
            if a['id'] in hdr:self.assertEqual(a,b)
        self.assertEqual(set(changes),index);self.assertEqual(len(changes),16)
        self.assertEqual(self.q.verify_catalog_delta(self.old_catalog,self.new_catalog,self.plan['before_policy_sha256'],self.plan['proposed_policy_sha256']),sorted(index))
    def test_unrelated_criterion_or_authority_change_rejected(self):
        for field in ['criterion','authority','pointer','candidates']:
            after=copy.deepcopy(self.new_catalog);row=next(r for r in after['obligations'] if r['id'] not in self.q.cell_sets()[1])
            row[field]='unreviewed'
            with self.assertRaises(ValueError):self.q.verify_catalog_delta(self.old_catalog,after,self.plan['before_policy_sha256'],self.plan['proposed_policy_sha256'])
    def test_original_scope_drift_does_not_get_blindly_rewritten(self):
        before=copy.deepcopy(self.before);row=next(r for r in before['rows'] if r['id'] in self.q.cell_sets()[1]);row['criterion']='changed before semantics'
        with self.assertRaisesRegex(ValueError,'original canonical criterion'):self.q.proposed_policy(before)
    def test_hdr_scope_cannot_be_changed_by_policy_transition(self):
        after=copy.deepcopy(self.new_catalog);row=next(r for r in after['obligations'] if r['id'] in self.q.cell_sets()[2]);row['criterion']='HDR now qualified'
        with self.assertRaises(ValueError):self.q.verify_catalog_delta(self.old_catalog,after,self.plan['before_policy_sha256'],self.plan['proposed_policy_sha256'])

class MigrationTests(unittest.TestCase):
    def setUp(self):
        self.q=workflow();self.before=self.q.read(P/'before'/self.q.LEDGER);self.registry=self.q.read(P/'before'/self.q.REGISTRY)
        self.mapping={key:'corrected_mpf_index_ranges_have_exact_recovery' for key in self.q.cell_sets()[1]}
        scopes=copy.deepcopy(self.registry['cases'][0]['exceptions'])
        for s in scopes:s['case_id']='representative.stage4-frames-text-assets.v12'
        self.candidate={'case_id':'representative.stage4-frames-text-assets.v12','approval':None,'exception_proposal':{'exceptions':scopes}}
    def derive(self,before=None,mapping=None):return self.q.expected_transaction(before or self.before,self.registry,'a'*64,mapping or self.mapping,self.candidate,'sha256:'+'b'*64,'c'*64,'d'*64)
    def test_exact_sixteen_additions_28_refresh_one_rep_and_hdr_unowned(self):
        new,registry=self.derive();changed=[];added=[];refreshed=[];_,index,hdr=self.q.cell_sets()
        for a,b in zip(self.before['assignments'],new['assignments']):
            self.assertEqual(a['id'],b['id'])
            if a!=b:
                changed.append(a['id'])
                if a['id'].startswith(self.q.PREFIX):
                    if a['disposition']['status']=='gap':added.append(a['id'])
                    else:
                        refreshed.append(a['id']);restored=copy.deepcopy(b);restored['disposition']['owner']['source']=a['disposition']['owner']['source'];self.assertEqual(restored,a)
            if a['id'] in hdr or a['id'].startswith(('codec:jpeg:icc:','codec:jpeg:color-sampling:','codec:jpeg:exif-precedence:')):self.assertEqual(a,b)
        self.assertEqual((len(changed),len(added),len(refreshed)),(45,16,28));self.assertEqual(set(added),index)
        self.assertEqual(new['catalog_sha256'],'d'*64);self.assertIsNone(new['approval'])
        self.assertEqual({s:sum(r['disposition']['status']==s for r in new['assignments']) for s in ['owned','gap','excluded']},{'owned':2038,'gap':85,'excluded':3})
        self.assertEqual([r for r in new['assignments'] if r['disposition']['status']=='excluded'],[r for r in self.before['assignments'] if r['disposition']['status']=='excluded'])
    def test_hdr_cannot_be_promoted_in_new_selector_map(self):
        mapping=dict(self.mapping);mapping[next(iter(self.q.cell_sets()[2]))]='hdr_projection_passes'
        with self.assertRaises(ValueError):self.derive(mapping=mapping)
    def test_existing_command_and_gap_issue_must_match(self):
        before=copy.deepcopy(self.before);row=next(r for r in before['assignments'] if r['id'] in self.q.cell_sets()[1]);row['disposition']['issue']=189
        with self.assertRaisesRegex(ValueError,'only unowned #190'):self.derive(before=before)
        before=copy.deepcopy(self.before);row=next(r for r in before['assignments'] if r['id'].startswith(self.q.PREFIX) and r['disposition']['status']=='owned');row['disposition']['owner']['command'].append('--ignored')
        with self.assertRaisesRegex(ValueError,'old owner'):self.derive(before=before)
    def test_current_capture_requires_explicit_freeze(self):
        with tempfile.TemporaryDirectory(dir='/tmp/opencode') as d:
            self.q.P=Path(d)
            with self.assertRaisesRegex(ValueError,'final-freeze'):self.q.representative()
            self.assertEqual(list(Path(d).iterdir()),[])
    def test_four_file_transaction_includes_policy_and_derived_manifest_hash(self):
        ledger,registry=self.derive()
        with tempfile.TemporaryDirectory(dir='/tmp/opencode') as d:
            q=self.q;q.ROOT=Path(d);q.P=q.ROOT/'packet';q.P.mkdir()
            candidate=q.ROOT/q.V12;candidate.mkdir(parents=True);(candidate/'manifest.json').write_text(json.dumps(self.candidate))
            pending=q.P/'pending';pending.mkdir()
            policy={'fixture':'exact reviewed policy'}
            for name,value in [('ownership.template.json',ledger),('registry.json',registry),('ownership-policy.json',policy)]:
                (pending/name).write_bytes(q.pretty(value))
            (q.P/'migration-plan.json').write_text(json.dumps({'representative_subject_sha256':'sha256:'+'b'*64}))
            approval={'approved_by':'fixture-only','approved_at':'2026-09-21','approval_url':'fixture','approval_subject_sha256':'sha256:'+'e'*64}
            result=q.transaction(approval)
            self.assertEqual(set(result),{q.V12+'/manifest.json',q.REGISTRY,q.LEDGER,q.POLICY})
            self.assertEqual(result[q.POLICY],q.pretty(policy))
            actual=json.loads(result[q.LEDGER]);binding=next(r for r in actual['assignments'] if r['id']=='interaction:frames-text-assets')['disposition']['owner']['binding']
            self.assertEqual(binding['manifest']['sha256'],q.sha(result[q.V12+'/manifest.json']))

class AuthorizationTests(unittest.TestCase):
    def test_exact_mpf_scope_and_affirmative_only(self):
        q=workflow();prod='sha256:'+'a'*64;rep='sha256:'+'b'*64
        good={'decision':'approve','scope':'jpeg190-mpf','production_subject_sha256':prod,'representative_subject_sha256':rep};body=lambda d:'```json\n'+json.dumps(d)+'\n```'
        q.affirmative(body(good),prod,rep)
        for bad in [dict(good,scope='jpeg190-hdr'),dict(good,decision='reject'),dict(good,extra=True)]:
            with self.assertRaises(ValueError):q.affirmative(body(bad),prod,rep)
        for bad in [body(good)*2,'reject '+prod+' '+rep,body(good).replace('"approve"','"reject", "decision":"approve"')]:
            with self.assertRaises(ValueError):q.affirmative(bad,prod,rep)
    def test_wrong_maintainer_refused_by_authenticated_helper(self):
        q=workflow().authentication_helper();a={'approved_by':'other','approved_at':'2026-09-21','approval_url':'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-123','approval_subject_sha256':'sha256:'+'a'*64}
        with self.assertRaisesRegex(ValueError,'designated maintainer'):q.authenticate(a,{'user':{'login':'other'}},a['approval_subject_sha256'],'sha256:'+'b'*64,{})
    def test_optimized_startup_rejected(self):
        for name in ['workflow.py','receipt_gate.py']:
            r=subprocess.run([sys.executable,'-O',str(P/name),'archive'],capture_output=True)
            self.assertNotEqual(r.returncode,0);self.assertIn(b'Optimized Python forbidden',r.stderr)

def reused_receipt_tests():
    q=workflow();path=q.ROOT/'tests/conformance/proposals/jpeg188-requalification-v2/test_verify.py';data=path.read_bytes()
    require=q.require;require(data==q.run(['git','show',q.read(P/'basis.json')['base_commit']+':'+str(path.relative_to(q.ROOT))]),'old tests changed')
    text=data.decode().replace('verified 1992 ownership assignments: 1915 assigned/excluded, 77 explicit gaps; ledger approval pending','verified 2126 ownership assignments: 2041 assigned/excluded, 85 explicit gaps; ledger approval pending')
    m=types.ModuleType('original_receipt_tests');m.__file__=str(path);exec(compile(text,str(path),'exec',optimize=0),m.__dict__)
    def factory():
        w=workflow();gate=w.receipt_gate();gate.REP='sha256:'+'d'*64;gate.test_inventory=w.helper('receipts').test_inventory
        gate.load_original=lambda name:w.authentication_helper()
        return gate
    m.module=factory
    return unittest.TestSuite([unittest.defaultTestLoader.loadTestsFromTestCase(m.Receipts),unittest.defaultTestLoader.loadTestsFromTestCase(m.TransactionTests)])

if __name__=='__main__':
    suite=unittest.TestSuite([unittest.defaultTestLoader.loadTestsFromTestCase(c) for c in [PolicyTests,MigrationTests,AuthorizationTests]]+[reused_receipt_tests()])
    result=unittest.TextTestRunner(verbosity=2).run(suite);raise SystemExit(not result.wasSuccessful())
