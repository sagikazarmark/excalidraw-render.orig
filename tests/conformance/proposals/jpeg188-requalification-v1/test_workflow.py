#!/usr/bin/env python3
"""Isolated ICC migration/authorization checks; never writes fake live approval."""
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
def checker():
    q=types.ModuleType('icc_fixture');q.__file__=str(P/'workflow.py')
    exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec',optimize=0),q.__dict__)
    return q

class MigrationTests(unittest.TestCase):
    def setUp(self):
        self.q=checker();self.before=self.q.read(P/'before'/self.q.LEDGER);self.registry=self.q.read(P/'before'/self.q.REGISTRY)
        self.selectors={key:'qualified_icc_precision_and_model_cells' for key in self.q.GAPS}
        scopes=copy.deepcopy(self.registry['cases'][0]['exceptions'])
        for e in scopes:e['case_id']='representative.stage4-frames-text-assets.v10'
        self.candidate={'case_id':'representative.stage4-frames-text-assets.v10','approval':None,'exception_proposal':{'exceptions':scopes}}
    def transaction(self,before=None,selectors=None,candidate=None):
        return self.q.expected_transaction(before or self.before,self.registry,'a'*64,selectors or self.selectors,candidate or self.candidate,'sha256:'+'b'*64,'c'*64)
    def test_exact_16_new_64_refresh_one_representative_and_preserved_sampling(self):
        ledger,registry=self.transaction();changes=[];new=[];refresh=[]
        for a,b in zip(self.before['assignments'],ledger['assignments']):
            self.assertEqual(a['id'],b['id'])
            if a!=b:
                changes.append(a['id'])
                if a['id'].startswith(self.q.PREFIX):
                    if a['disposition']['status']=='gap':new.append(a['id'])
                    else:
                        refresh.append(a['id']);restored=copy.deepcopy(b)
                        restored['disposition']['owner']['source']=a['disposition']['owner']['source']
                        self.assertEqual(restored,a)
            if a['id'].startswith('codec:jpeg:color-sampling:'):self.assertEqual(a,b)
        self.assertEqual((len(changes),len(new),len(refresh)),(81,16,64))
        self.assertEqual(set(new),self.q.GAPS)
        self.assertEqual({s:sum(r['disposition']['status']==s for r in ledger['assignments']) for s in ['owned','gap','excluded']},{'owned':1912,'gap':77,'excluded':3})
        self.assertEqual(ledger['catalog_sha256'],self.before['catalog_sha256']);self.assertIsNone(ledger['approval'])
        self.assertEqual([r for r in ledger['assignments'] if r['disposition']['status']=='excluded'],[r for r in self.before['assignments'] if r['disposition']['status']=='excluded'])
        self.assertEqual(len(registry['cases'][0]['execution_keys']),3)
    def test_missing_or_substituted_icc_cell_rejected(self):
        before=copy.deepcopy(self.before);next(r for r in before['assignments'] if r['id'].startswith(self.q.PREFIX))['id']=self.q.PREFIX+'unknown'
        with self.assertRaisesRegex(ValueError,'80 exact'):self.transaction(before=before)
    def test_wrong_gap_issue_rejected(self):
        before=copy.deepcopy(self.before);next(r for r in before['assignments'] if r['id'] in self.q.GAPS)['disposition']['issue']=187
        with self.assertRaisesRegex(ValueError,'#188 gaps'):self.transaction(before=before)
    def test_old_owner_command_cannot_change(self):
        before=copy.deepcopy(self.before);row=next(r for r in before['assignments'] if r['id'].startswith(self.q.PREFIX) and r['disposition']['status']=='owned')
        row['disposition']['owner']['command'].append('ignored')
        with self.assertRaisesRegex(ValueError,'command drift'):self.transaction(before=before)
    def test_new_selector_mapping_must_be_qualified_exact_and_executable(self):
        self.q.validate_selectors(self.selectors,'#[test]\nfn qualified_icc_precision_and_model_cells() {}')
        for source in ['fn qualified_icc_precision_and_model_cells() {}','#[test]\n#[ignore]\nfn qualified_icc_precision_and_model_cells() {}']:
            with self.assertRaisesRegex(ValueError,'unconditional test'):self.q.validate_selectors(self.selectors,source)
        mapping=dict(self.selectors);mapping[next(iter(mapping))]='issue_188_remains_unqualified'
        with self.assertRaisesRegex(ValueError,'not qualified'):self.transaction(selectors=mapping)
        mapping=dict(self.selectors);mapping.pop(next(iter(mapping)))
        with self.assertRaisesRegex(ValueError,'exact #188'):self.transaction(selectors=mapping)
    def test_exception_budget_cannot_be_loosened(self):
        candidate=copy.deepcopy(self.candidate);candidate['exception_proposal']['exceptions'][0]['max_changed_pixels']+=1
        with self.assertRaisesRegex(ValueError,'scopes changed'):self.transaction(candidate=candidate)
    def test_no_current_collection_without_freeze(self):
        with tempfile.TemporaryDirectory(dir='/tmp/opencode') as directory:
            self.q.P=Path(directory)
            with self.assertRaisesRegex(ValueError,'final-freeze'):self.q.representative()
            self.assertEqual(list(Path(directory).iterdir()),[])
    def test_authenticated_helper_refuses_changed_archived_bytes(self):
        with tempfile.TemporaryDirectory(dir='/tmp/opencode') as directory:
            q=self.q;q.P=Path(directory);basis=q.read(P/'basis.json')
            (q.P/'basis.json').write_text(json.dumps(basis));path=q.P/'before'/q.REUSED[0];path.parent.mkdir(parents=True);path.write_bytes(b'changed')
            q.desc=lambda p:{'path':basis['reused_helpers'][0]['path'],'sha256':q.sha(p.read_bytes()),'byte_length':len(p.read_bytes())}
            with self.assertRaisesRegex(ValueError,'helper changed'):q.collector()

class ApprovalTests(unittest.TestCase):
    def setUp(self):
        self.q=checker();self.subject='sha256:'+'a'*64;self.rep='sha256:'+'b'*64
        self.a={'approved_by':'sagikazarmark','approved_at':'2026-09-21','approval_url':'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-123456789','approval_subject_sha256':self.subject}
        self.decision={'decision':'approve','scope':'jpeg188-icc','production_subject_sha256':self.subject,'representative_subject_sha256':self.rep}
        self.comment={'id':123456789,'html_url':self.a['approval_url'],'created_at':'2026-09-21T00:00:00Z','user':{'login':'sagikazarmark'},'body':self.body(self.decision)}
        self.remote=copy.deepcopy(self.comment);self.q.fetch_comment=lambda url:copy.deepcopy(self.remote)
        self.ra=dict(self.a,approval_subject_sha256=self.rep)
    def body(self,value):return '```json\n'+json.dumps(value)+'\n```'
    def call(self):self.q.authenticate(self.a,self.comment,self.subject,self.rep,self.ra)
    def test_positive_in_memory_api_shape_passes(self):self.call()
    def test_other_login_rejected_even_if_consistent(self):
        self.a['approved_by']='other';self.ra['approved_by']='other';self.comment['user']['login']='other';self.remote=copy.deepcopy(self.comment)
        with self.assertRaisesRegex(ValueError,'designated maintainer'):self.call()
    def test_hash_mentions_and_rejection_never_approve(self):
        for body in ['Reject '+self.subject+' '+self.rep,self.body(dict(self.decision,decision='reject')),'No approval.\n'+self.body(self.decision)]:
            self.comment['body']=body;self.remote=copy.deepcopy(self.comment)
            with self.assertRaises(ValueError):self.call()
    def test_duplicate_unknown_malformed_and_wrong_scope_rejected(self):
        for body in [self.body(self.decision)*2,self.body(dict(self.decision,extra=True)),self.body(self.decision).replace('"approve"','"reject", "decision":"approve"'),'```json\n{bad}\n```',self.body(dict(self.decision,scope='jpeg187-color-only'))]:
            self.comment['body']=body;self.remote=copy.deepcopy(self.comment)
            with self.assertRaises(ValueError):self.call()
    def test_remote_author_or_text_drift_rejected(self):
        self.remote['user']['login']='other'
        with self.assertRaisesRegex(ValueError,'genuine comment author'):self.call()
        self.remote=copy.deepcopy(self.comment);self.remote['created_at']='2026-09-21T01:00:00Z'
        with self.assertRaisesRegex(ValueError,'saved/genuine'):self.call()
    def test_wrong_date_subject_or_representative_rejected(self):
        original=copy.deepcopy(self.a)
        for field,value in [('approved_at','2026-09-20'),('approval_subject_sha256',self.rep)]:
            self.a=dict(original,**{field:value})
            with self.assertRaises(ValueError):self.call()
        self.a=original;self.ra=dict(self.ra,approval_subject_sha256=self.subject)
        with self.assertRaisesRegex(ValueError,'representative decision'):self.call()
    def test_applied_audit_requires_saved_production_decision(self):
        with tempfile.TemporaryDirectory(dir='/tmp/opencode') as directory:
            self.q.ROOT=Path(directory);app=self.q.ROOT/'tests/conformance/coverage/jpeg188-application-v1';app.mkdir(parents=True)
            target=self.q.ROOT/self.q.V10;target.mkdir(parents=True);(target/'manifest.json').write_text(json.dumps({'approval':self.ra}))
            (app/'status.json').write_text(json.dumps({'state':'applied','production_subject_sha256':self.subject,'representative_subject_sha256':self.rep}))
            with self.assertRaisesRegex(ValueError,'approval.json'):self.q.authenticate_application(self.subject,self.rep)
            (app/'approval.json').write_text(json.dumps(self.a))
            with self.assertRaisesRegex(ValueError,'comment.json'):self.q.authenticate_application(self.subject,self.rep)
            (app/'comment.json').write_text(json.dumps(self.comment));self.q.authenticate_application(self.subject,self.rep)
    def test_optimized_execution_refused_before_mutation(self):
        for flags,environment in [(['-O'],None),([], '1')]:
            env=dict(os.environ,PYTHONDONTWRITEBYTECODE='1');env.pop('PYTHONOPTIMIZE',None)
            if environment:env['PYTHONOPTIMIZE']=environment
            result=subprocess.run([sys.executable,*flags,str(P/'workflow.py'),'archive'],env=env,capture_output=True)
            self.assertNotEqual(result.returncode,0);self.assertIn(b'Optimized Python forbidden',result.stderr)

if __name__=='__main__':unittest.main(verbosity=2)
