#!/usr/bin/env python3
"""Strict completed-suite classification and four-file policy receipt gate.

Only numeric test totals are adapted from the authenticated prior parser.
All five pending failure identities/messages remain exact, not waived.
"""
import sys
if not __debug__ or sys.flags.optimize:
    raise SystemExit('Optimized Python forbidden; no workflow started.')
import hashlib
import json
from pathlib import Path
import types

P=Path(__file__).resolve().parent
ROOT=P.parents[3]
PARSER='tests/conformance/proposals/jpeg188-requalification-v1/suite-verification.py'
ENVELOPE='tests/conformance/proposals/jpeg188-requalification-v1/approval-subject.json'
PRIOR='6dbac083ea0d744880bebe0aab6a259f9530bad0635540faeba156edad23c1fa'
def require(ok,message):
    if not ok:raise ValueError(message)
def sha(data):return hashlib.sha256(data).hexdigest()
def canonical(v):return json.dumps(v,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()
def read(path):return json.loads(path.read_bytes())
def check(d):
    data=(ROOT/d['path']).read_bytes()
    require(len(data)==d['byte_length'] and sha(data)==d['sha256'],'stale descriptor '+d['path'])
    return data

def parser():
    bundle=read(ROOT/ENVELOPE)
    require(sha(canonical(bundle))==PRIOR,'historical parser subject changed')
    source=check(next(d for d in bundle['files'] if d['path']==PARSER)).decode()
    replacements={'[1226,5,19,0,0]':'[1244,5,19,0,0]','==1226':'==1244',"'passed':1226":"'passed':1244",
                  "'executed_pass_or_fail':1231":"'executed_pass_or_fail':1249","'including_ignored':1250":"'including_ignored':1268",
                  "('Running tests/fast/main.rs',595)":"('Running tests/fast/main.rs',613)","'renderer_fast_passed':595":"'renderer_fast_passed':613"}
    for before,after in replacements.items():
        require(source.count(before)==1,'parser adaptation seam changed');source=source.replace(before,after)
    m=types.ModuleType('authenticated_mpf_suite_parser');m.__file__=str(ROOT/PARSER)
    exec(compile(source,m.__file__,'exec',optimize=0),m.__dict__)
    return m

def frozen_workflow():
    path=P/'workflow.py';data=path.read_bytes()
    require(sha(data)==read(P/'final-freeze.json')['producer_files'][str(path.relative_to(ROOT))],'frozen workflow changed')
    q=types.ModuleType('authenticated_frozen_mpf_workflow');q.__file__=str(path)
    exec(compile(data,str(path),'exec',optimize=0),q.__dict__)
    return q

def verify_suite(applied=False):
    a=read(P/'checks/full-suite-record.json');s=parser();q=frozen_workflow()
    require(a['approval'] is None and a['command']==['cargo','test','--locked','--workspace','--no-fail-fast'],'command/approval mismatch')
    require(a['exit_code']==1 and a['completed_normally'] is True and a['timeout_ms']==1800000 and a['outcome']=='failed-only-at-exact-pending-binding-tests','failures cannot be relabeled or waived')
    require(a['environment']=={'toolchain':'1.97.1-x86_64-unknown-linux-gnu','CARGO_INCREMENTAL':'0','CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0','CARGO_PROFILE_RELEASE_DEBUG':'0'},'environment mismatch')
    require(s.classify(check(a['raw_log']))==a['classification'],'raw suite classification differs')
    check(a['parent_message']);check(a['freeze']);check(a['test_source'])
    require(a['test_source']['path']==q.SOURCE,'wrong test source')
    require(a['producer_sources']==read(ROOT/q.V12/'native-capture.json')['producer_sources'],'producer set mismatch')
    for d in a['producer_sources']:check(d)
    q.check_freeze(applied)
    r=a['unknown-owner-cli-reproduction']
    require(r['command']==['cargo','run','--release','--locked','-p','xtask','--','conformance','record-test-owner','surface_metadata_contract','editor_state_is_silent_preserved_and_independent_of_scene_and_frame_exports','Explicit reviewed editor-only surface contract','surface:unknown:/unknown:svg'],'wrong assertion-only reproduction')
    require(r['exit_code']!=0 and check(r['stdout'])==b'','diagnostic must fail closed')
    require([line for line in check(r['stderr']).decode().splitlines() if line.startswith('error:')]==['error: stale ownership evidence'],'unexpected diagnostic cause')
    require(r['ledger_before_sha256']==r['ledger_after_sha256'],'diagnostic mutated ledger')
    require(a['post_approval_required']==['cargo test --locked -p xtask --bin xtask --test coverage --test representative_candidate --no-fail-fast'],'post-approval receipt requirement differs')
    # Policy transition belongs to this exact approval-bearing packet, not a
    # mere source digest refresh. It is checked in both pending/applied phases.
    q.verify_policy(applied)
    return a['classification']

def gate():
    q=frozen_workflow();g=q.receipt_gate();original=g.evidence
    def with_suite(applied):
        verify_suite(applied)
        original(applied)
    g.evidence=with_suite
    return g

if __name__=='__main__':
    args=sys.argv[1:]
    if args==['seal']:
        counts=verify_suite();frozen_workflow().seal()
        print(f"Completed suite: {counts['passed']} passed / 5 exact pending failures / 19 ignored; no waiver.")
    elif args==['verify']:gate().verify(False)
    elif args==['verify-applied']:gate().verify(True)
    elif len(args)==2 and args[0]=='apply':gate().apply(args[1])
    else:raise SystemExit('seal|verify|verify-applied|apply ACTUAL_APPROVAL_JSON')
