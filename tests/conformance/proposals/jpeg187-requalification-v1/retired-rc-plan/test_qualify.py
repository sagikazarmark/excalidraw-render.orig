#!/usr/bin/env python3
"""Isolated proposal/authorization checks. Never creates live fake approval."""
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

HERE=Path(__file__).resolve().parent
def checker():
    m=types.ModuleType('isolated_jpeg187_checker'); m.__file__=str(HERE/'qualify.py')
    exec(compile((HERE/'qualify.py').read_bytes(),m.__file__,'exec',optimize=0),m.__dict__)
    return m

class MigrationTests(unittest.TestCase):
    def setUp(self):
        self.q=checker(); self.base=json.loads((HERE/'before/tests/conformance/coverage/ownership.json').read_bytes())
        self.registry=json.loads((HERE/'before/tests/conformance/candidates/registry.json').read_bytes())
        self.source='a'*64; self.subject='sha256:'+'b'*64; self.manifest_hash='c'*64
        scopes=copy.deepcopy(self.registry['cases'][0]['exceptions'])
        for s in scopes:s['case_id']='representative.stage4-frames-text-assets.v9'
        self.v9={'case_id':'representative.stage4-frames-text-assets.v9','approval':None,'exception_proposal':{'exceptions':scopes}}
    def derive(self,before=None,v9=None):
        return self.q.expected_transaction(before or self.base,self.registry,self.source,v9 or self.v9,self.subject,self.manifest_hash)
    def test_exact_21_additions_17_refresh_and_one_representative_preserve_rest(self):
        original=copy.deepcopy(self.base); registry_before=copy.deepcopy(self.registry)
        ledger,registry=self.derive()
        changed=[]; added=[]; refreshed=[]
        for old,new in zip(self.base['assignments'],ledger['assignments']):
            self.assertEqual(old['id'],new['id'])
            if old!=new:
                changed.append(old['id'])
                if old['id'].startswith(self.q.PREFIX):
                    if old['disposition']['status']=='gap':added.append(old['id'])
                    else:
                        refreshed.append(old['id']); normalized=copy.deepcopy(new)
                        normalized['disposition']['owner']['source']=old['disposition']['owner']['source']
                        self.assertEqual(normalized,old)
        self.assertEqual((len(changed),len(added),len(refreshed)),(39,21,17))
        self.assertEqual({s:sum(r['disposition']['status']==s for r in ledger['assignments']) for s in ['owned','excluded','gap']},{'owned':1838,'excluded':3,'gap':71})
        self.assertEqual(ledger['catalog_sha256'],self.base['catalog_sha256']);self.assertIsNone(ledger['approval'])
        self.assertEqual([r for r in ledger['assignments'] if r['disposition']['status']=='excluded'],[r for r in self.base['assignments'] if r['disposition']['status']=='excluded'])
        self.assertEqual(self.base,original);self.assertEqual(self.registry,registry_before)
        self.assertEqual(len(registry['cases'][0]['execution_keys']),3)
    def test_wrong_gap_issue_rejected(self):
        before=copy.deepcopy(self.base)
        row=next(r for r in before['assignments'] if r['id'].startswith(self.q.PREFIX) and r['disposition']['status']=='gap')
        row['disposition']['issue']=999
        with self.assertRaisesRegex(ValueError,'unowned #187'):self.derive(before)
    def test_wrong_old_owner_selector_rejected(self):
        before=copy.deepcopy(self.base)
        row=next(r for r in before['assignments'] if r['id'].startswith(self.q.PREFIX) and r['disposition']['status']=='owned')
        row['disposition']['owner']['function']='unreviewed'
        with self.assertRaisesRegex(ValueError,'selector/command'):self.derive(before)
    def test_omitted_or_substituted_cell_rejected(self):
        before=copy.deepcopy(self.base)
        next(r for r in before['assignments'] if r['id'].startswith(self.q.PREFIX))['id']=self.q.PREFIX+'unknown'
        with self.assertRaisesRegex(ValueError,'exact 38'):self.derive(before)
    def test_loosened_representative_exception_rejected(self):
        v9=copy.deepcopy(self.v9);v9['exception_proposal']['exceptions'][0]['max_changed_pixels']+=1
        with self.assertRaisesRegex(ValueError,'tolerances'):self.derive(v9=v9)
    def test_freeze_required_before_native_operations(self):
        with tempfile.TemporaryDirectory(dir='/tmp/opencode') as d:
            self.q.PACKET=Path(d)
            with self.assertRaisesRegex(ValueError,'final-freeze'):
                self.q.representative()
            self.assertEqual(list(Path(d).iterdir()),[])
    def test_optimized_startup_refuses_before_mutation(self):
        for flag in ['-O','-OO']:
            r=subprocess.run([sys.executable,flag,str(HERE/'qualify.py'),'archive'],capture_output=True)
            self.assertNotEqual(r.returncode,0);self.assertIn(b'Optimized Python is forbidden',r.stderr)

class AuthorizationTests(unittest.TestCase):
    def setUp(self):
        self.q=checker();self.subject='sha256:'+'a'*64;self.rep='sha256:'+'b'*64
        self.a={'approved_by':'fixture-reviewer','approved_at':'2026-09-20','approval_url':'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-123456789','approval_subject_sha256':self.subject}
        self.comment={'id':123456789,'html_url':self.a['approval_url'],'created_at':'2026-09-20T12:00:00Z','user':{'login':'fixture-reviewer'},'body':'ISOLATED FIXTURE ONLY '+self.subject+' '+self.rep}
        self.remote=copy.deepcopy(self.comment);self.q.fetch_comment=lambda url:copy.deepcopy(self.remote)
        self.ra=dict(self.a,approval_subject_sha256=self.rep)
    def call(self,a=None,c=None,ra=None):self.q.authenticate(a if a is not None else self.a,c if c is not None else self.comment,self.subject,self.rep,ra if ra is not None else self.ra)
    def test_authenticated_shaped_fixture_passes_no_live_writes(self):self.call()
    def test_missing_and_wrong_production_approval_rejected(self):
        for a in [{},dict(self.a,approval_subject_sha256=self.rep)]:
            with self.assertRaises(ValueError):self.call(a=a)
    def test_both_subjects_required(self):
        for s in [self.subject,self.rep]:
            with self.assertRaisesRegex(ValueError,'both exact'):self.call(c=dict(self.comment,body=self.comment['body'].replace(s,'')))
    def test_identity_date_and_url_must_agree(self):
        for c in [dict(self.comment,user={'login':'other'}),dict(self.comment,created_at='2026-09-21T12:00:00Z'),dict(self.comment,html_url=self.comment['html_url']+'0')]:
            with self.assertRaisesRegex(ValueError,'identity/date/url'):self.call(c=c)
    def test_saved_decision_must_equal_remote(self):
        self.remote['body']+=' changed'
        with self.assertRaisesRegex(ValueError,'genuine GitHub'):self.call()
    def test_representative_alone_does_not_authorize_production(self):
        with self.assertRaisesRegex(ValueError,'same scoped decision'):self.call(ra=dict(self.ra,approved_by='other'))
    def test_applied_audit_requires_persisted_production_decision_not_just_v9(self):
        with tempfile.TemporaryDirectory(dir='/tmp/opencode',prefix='jpeg187-auth-') as d:
            self.q.ROOT=Path(d)
            app=self.q.ROOT/'tests/conformance/coverage/jpeg187-application-v1';app.mkdir(parents=True)
            candidate=self.q.ROOT/self.q.V9;candidate.mkdir(parents=True)
            (candidate/'manifest.json').write_text(json.dumps({'approval':self.ra}))
            (app/'status.json').write_text(json.dumps({'state':'applied','production_subject_sha256':self.subject,'representative_subject_sha256':self.rep}))
            with self.assertRaisesRegex(ValueError,'approval.json'):self.q.authenticate_application(self.subject,self.rep)
            (app/'approval.json').write_text(json.dumps(self.a))
            with self.assertRaisesRegex(ValueError,'comment.json'):self.q.authenticate_application(self.subject,self.rep)
            (app/'comment.json').write_text(json.dumps(self.comment))
            self.q.authenticate_application(self.subject,self.rep)
            (app/'approval.json').write_text(json.dumps(dict(self.a,approval_subject_sha256=self.rep)))
            with self.assertRaisesRegex(ValueError,'wrong production subject'):self.q.authenticate_application(self.subject,self.rep)
    def test_incomplete_applied_attempt_rejected(self):
        with tempfile.TemporaryDirectory(dir='/tmp/opencode') as d:
            self.q.ROOT=Path(d);app=self.q.ROOT/'tests/conformance/coverage/jpeg187-application-v1';app.mkdir(parents=True)
            (app/'status.json').write_text(json.dumps({'state':'applying'}))
            with self.assertRaisesRegex(ValueError,'not successfully applied'):self.q.authenticate_application(self.subject,self.rep)

if __name__=='__main__':unittest.main(verbosity=2)
