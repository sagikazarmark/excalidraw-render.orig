#!/usr/bin/env python3
"""Isolated receipt/rollback tests; mocked processes, never fake live approval."""
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

P=Path(__file__).resolve().parent
def module():
    v=types.ModuleType('receipt_fixture');v.__file__=str(P/'verify.py')
    exec(compile((P/'verify.py').read_bytes(),v.__file__,'exec',optimize=0),v.__dict__)
    return v

class Receipts(unittest.TestCase):
    def setUp(self):
        self.v=module();self.inventory=self.v.test_inventory()
        self.tmp=tempfile.TemporaryDirectory(dir='/tmp/opencode',prefix='jpeg188-receipts-');self.addCleanup(self.tmp.cleanup)
        self.v.ROOT=Path(self.tmp.name);self.app=self.v.ROOT/self.v.APP
        self.subject='sha256:'+'a'*64
        self.binding={'production_subject_sha256':self.subject,'representative_subject_sha256':self.v.REP,'source_sha256':'source','manifest_sha256':'approved-manifest','registry_sha256':'registry','ledger_sha256':'ledger'}
        self.v.current_binding=lambda subject:copy.deepcopy(self.binding)
        self.v.test_inventory=lambda:copy.deepcopy(self.inventory)
        self.v.expected_consumer=lambda:{'schema':'fixture-bound-receipts','approval_required':True,'approval_subject_sha256':self.v.REP,'executions':[{'execution_key':self.v.CASE+':default:'+f,'fresh_process_runs':2} for f in ['svg','png','pdf']]}
        self.outputs={}
        stdout=''
        for target in self.inventory:
            stdout+=f'\nrunning {target["count"]} tests\n'+''.join(f'test {name} ... ok\n' for name in target['identities'])+f'\ntest result: ok. {target["count"]} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'
        stderr='    Finished `test` profile [optimized] target(s) in 0.01s\n'
        for path,name in [('unittests src/main.rs','xtask'),('tests/coverage.rs','coverage'),('tests/representative_candidate.rs','representative_candidate')]:stderr+=f'     Running {path} (target/debug/deps/{name}-abcd)\n'
        self.outputs['affected-tests']=(stdout.encode(),stderr.encode())
        self.outputs['candidate']=(self.v.canonical(self.v.expected_consumer()),b'')
        self.outputs['registry']=(self.v.canonical({'schema':'excalidraw-render/representative-corpus-results/v1','cases':[self.v.expected_consumer()]}),b'')
        self.outputs['owner']=(f'verified approved representative owner {self.v.CASE}\n'.encode(),b'')
        self.outputs['inventory']=(b'verified draft inventory: 320 coverage rows, 168 executions, 8 unresolved milestones; release closure is pending\n',b'')
        self.outputs['ownership']=(b'verified 1992 ownership assignments: 1915 assigned/excluded, 77 explicit gaps; ledger approval pending\n',b'')
        self.commands=[]
        def process(args):
            self.commands.append(args)
            name=next(name for name,command in self.v.COMMANDS if command==args)
            out,err=self.outputs[name]
            return types.SimpleNamespace(returncode=0,stdout=out,stderr=err)
        self.v.process=process
        self.v.run_required(self.subject,self.app)
        self.receipts=self.v.read(self.app/'receipts.json')
    def validate(self,r=None):return self.v.validate_receipts(r or self.receipts,self.subject,self.app)
    def mutate_log(self,name,key,data):
        row=next(r for r in self.receipts['runs'] if r['name']==name)
        path=self.v.ROOT/row[key]['path'];path.write_bytes(data);row[key]=self.v.desc(path)
    def test_actual_identity_sets_and_all_six_command_receipts_required(self):
        self.assertEqual(self.validate()[0]['passed'],109)
        self.assertEqual(self.commands,[args for _,args in self.v.COMMANDS])
    def test_missing_receipt_or_log_rejected(self):
        r=copy.deepcopy(self.receipts);r['runs'].pop()
        with self.assertRaisesRegex(ValueError,'missing or extra'):self.validate(r)
        (self.app/'logs/affected-tests.stdout').unlink()
        with self.assertRaises(OSError):self.validate()
    def test_nonzero_exit_even_green_stdout_rejected(self):
        self.receipts['runs'][0]['exit_code']=1
        with self.assertRaisesRegex(ValueError,'exit successfully'):self.validate()
    def test_wrong_command_flags_or_target_rejected(self):
        self.receipts['runs'][0]['command'].append('--ignored')
        with self.assertRaisesRegex(ValueError,'wrong command'):self.validate()
    def test_wrong_source_subject_metadata_or_snapshot_rejected(self):
        for field in ['production_subject_sha256','representative_subject_sha256','binding','before','after']:
            r=copy.deepcopy(self.receipts)
            if field in ['before','after']:r['runs'][0][field]['ledger_sha256']='different'
            elif field=='binding':r[field]['manifest_sha256']='different'
            else:r[field]='sha256:'+'0'*64
            with self.subTest(field=field),self.assertRaises(ValueError):self.validate(r)
    def test_forged_summary_cannot_replace_full_success_identities(self):
        self.mutate_log('affected-tests','stdout',b'\nrunning 103 tests\ntest result: ok. 103 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n')
        with self.assertRaises(ValueError):self.validate()
    def test_same_count_substituted_test_identity_rejected(self):
        data=self.outputs['affected-tests'][0].replace(self.inventory[0]['identities'][0].encode(),b'forged::replacement',1)
        self.mutate_log('affected-tests','stdout',data)
        with self.assertRaisesRegex(ValueError,'identities'):self.validate()
    def test_forged_checked_summary_rejected(self):
        self.receipts['runs'][0]['checked_outcome']['passed']=999
        with self.assertRaisesRegex(ValueError,'forged command summary'):self.validate()
    def test_stderr_failure_or_wrong_binary_rejected(self):
        for data in [self.outputs['affected-tests'][1]+b'error: failed\n',self.outputs['affected-tests'][1].replace(b'coverage-abcd',b'wrong-abcd')]:
            self.mutate_log('affected-tests','stderr',data)
            with self.assertRaises(ValueError):self.validate()
    def test_wrong_candidate_subject_artifact_or_missing_registry_case_rejected(self):
        self.mutate_log('candidate','stdout',b'{"approval_required":true,"executions":[]}')
        with self.assertRaisesRegex(ValueError,'candidate receipts'):self.validate()
    def test_pending_or_wrong_ownership_totals_rejected(self):
        self.mutate_log('ownership','stdout',b'verified 1992 ownership assignments: 1899 assigned/excluded, 93 explicit gaps; ledger approval pending\n')
        with self.assertRaisesRegex(ValueError,'ownership verification'):self.validate()
    def test_saved_parseable_forgery_is_not_accepted_when_actual_rerun_fails(self):
        self.validate()
        self.v.packet=lambda applied:self.subject
        self.v.authorize=lambda subject:None
        self.v.evidence=lambda applied:None
        self.v.process=lambda args:types.SimpleNamespace(returncode=1,stdout=b'',stderr=b'actual failure')
        with self.assertRaisesRegex(ValueError,'command failed'):self.v.verify(True)
    def test_successful_applied_audit_reexecutes_all_required_commands(self):
        self.v.packet=lambda applied:self.subject;self.v.authorize=lambda subject:None;self.v.evidence=lambda applied:None
        self.commands=[]
        with contextlib.redirect_stdout(io.StringIO()):self.v.verify(True)
        self.assertEqual(self.commands,[args for _,args in self.v.COMMANDS])

class TransactionTests(unittest.TestCase):
    def setUp(self):
        self.v=module();self.tmp=tempfile.TemporaryDirectory(dir='/tmp/opencode',prefix='jpeg188-transaction-');self.addCleanup(self.tmp.cleanup)
        self.v.ROOT=Path(self.tmp.name);self.subject='sha256:'+'a'*64
        self.approval={'approved_by':'sagikazarmark','approved_at':'2026-09-21','approval_url':'fixture','approval_subject_sha256':self.subject}
        self.path=self.v.ROOT/'fixture-approval.json';self.path.write_text(json.dumps(self.approval))
        self.before={self.v.V10+'/manifest.json':b'old-manifest',self.v.REGISTRY:b'old-registry',self.v.LEDGER:b'old-ledger'}
        self.after={p:b'new-'+b for p,b in self.before.items()}
        for p,b in self.before.items():
            path=self.v.ROOT/p;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(b)
        self.v.verify=lambda applied=False:self.subject
        self.v.transaction=lambda approval:dict(self.after)
        self.v.evidence=lambda applied:None
        q=types.SimpleNamespace(fetch_comment=lambda url:{'fixture':True},authenticate=lambda *args:None,verify_migration=lambda applied:None)
        self.v.load_original=lambda name:q
    def test_failure_restores_exact_preimages_and_never_marks_applied(self):
        def fail(subject,directory):
            self.assertTrue((directory/'approval.json').is_file());self.assertTrue((directory/'comment.json').is_file())
            self.v.put(directory/'logs/partial.stdout',b'partial evidence')
            raise ValueError('isolated required-test failure')
        self.v.run_required=fail
        with self.assertRaisesRegex(ValueError,'required-test failure'):self.v.apply(self.path)
        for p,b in self.before.items():self.assertEqual((self.v.ROOT/p).read_bytes(),b)
        app=self.v.ROOT/self.v.APP
        self.assertEqual(self.v.read(app/'status.json')['state'],'failed_rolled_back')
        self.assertEqual((app/'logs/partial.stdout').read_bytes(),b'partial evidence')
        with self.assertRaisesRegex(ValueError,'existing attempt'):self.v.apply(self.path)
    def test_applied_status_only_after_recorded_checks_validate(self):
        events=[]
        def execute(subject,directory):
            self.assertEqual(self.v.read(directory/'status.json')['state'],'checking')
            self.v.put(directory/'receipts.json',b'{"fixture":true}');events.append('executed')
        def validate(*args):
            self.assertEqual(events,['executed']);events.append('validated')
        self.v.run_required=execute;self.v.validate_receipts=validate
        # Only fake metadata in disposable fixture, no authorization claim.
        self.v.evidence=lambda applied:None
        manifest=self.v.V10+'/manifest.json';self.after[manifest]=b'{"approval":{}}'
        with contextlib.redirect_stdout(io.StringIO()):self.v.apply(self.path)
        self.assertEqual(events,['executed','validated'])
        self.assertEqual(self.v.read(self.v.ROOT/self.v.APP/'status.json')['state'],'applied')

class SafetyTests(unittest.TestCase):
    def test_optimized_refuses_before_workflow(self):
        result=subprocess.run([sys.executable,'-O',str(P/'verify.py'),'apply','missing.json'],capture_output=True)
        self.assertNotEqual(result.returncode,0);self.assertIn(b'Optimized Python forbidden',result.stderr)
    def test_strict_existing_scope_and_maintainer_auth_preserved(self):
        v=module();q=v.load_original('workflow.py');prod='sha256:'+'a'*64;rep=v.REP
        good={'decision':'approve','scope':'jpeg188-icc','production_subject_sha256':prod,'representative_subject_sha256':rep}
        q.affirmative('```json\n'+json.dumps(good)+'\n```',prod,rep)
        for bad in [dict(good,decision='reject'),dict(good,scope='other')]:
            with self.assertRaises(ValueError):q.affirmative('```json\n'+json.dumps(bad)+'\n```',prod,rep)

if __name__=='__main__':unittest.main(verbosity=2)
