#!/usr/bin/env python3
"""Strict read-only classification; five pending failures remain failures."""
import sys
if not __debug__ or sys.flags.optimize:
    raise SystemExit('Optimized Python forbidden; no workflow started.')
import hashlib
import json
from pathlib import Path
import re
import types

P=Path(__file__).resolve().parent
ROOT=P.parents[3]
EXPECTED={
 'coverage::ownership::tests::inventory_extension_preserves_live_owners_and_approved_exclusions': ('xtask/src/coverage/ownership.rs:985:10', 'called `Result::unwrap()` on an `Err` value: "stale ownership evidence"'),
 'coverage::ownership::tests::test_owner_checks_compiled_module_exact_selector_and_source_digest': ('xtask/src/coverage/ownership.rs:1312:44', 'called `Result::unwrap()` on an `Err` value: "stale ownership evidence"'),
 'ownership_recording_rejects_unknown_obligations_without_changing_the_ledger': ('xtask/tests/coverage.rs:103:5', 'assertion failed: String::from_utf8_lossy(&output.stderr).contains("unknown ownership identity")'),
 'ownership_recording_validates_the_entire_batch_before_writing': ('xtask/tests/coverage.rs:168:9', 'error: stale ownership evidence'),
 'representative_registry_is_consumed_through_approval_aware_native_execution': ('xtask/tests/representative_candidate.rs:32:9', 'error: stale adapter production/source bindings'),
}
TARGETS=['-p xtask --bin xtask','-p xtask --test coverage','-p xtask --test representative_candidate']
NOTE='note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace'
def require(condition,message):
    if not condition:raise ValueError(message)
def sha(data):return hashlib.sha256(data).hexdigest()
def read(path):return json.loads(path.read_bytes())
def check(d):
    data=(ROOT/d['path']).read_bytes()
    require(len(data)==d['byte_length'] and sha(data)==d['sha256'],'stale descriptor '+d['path'])
    return data

def classify(raw):
    text=raw.decode()
    results=re.findall(r'^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out; finished in [0-9.]+s$',text,re.M)
    require(len(results)==39,'incomplete/unexpected target summary set')
    totals=[sum(int(r[i]) for r in results) for i in range(1,6)]
    require(totals==[1226,5,19,0,0],'unexpected aggregate test counts')
    require(len(re.findall(r'^test .+ \.\.\. ok$',text,re.M))==1226,'pass listings disagree with summaries')
    failed=re.findall(r'^test (.+) \.\.\. FAILED$',text,re.M)
    require(len(failed)==5 and set(failed)==set(EXPECTED),'unknown/duplicate/missing failed test identity')
    require([tuple(map(int,r[1:])) for r in results if r[0]=='FAILED']==[(101,2,0,0,0),(3,2,0,0,0),(0,1,0,0,0)],'wrong failed target counts')
    require(all((r[0]=='FAILED')==(int(r[2])>0) for r in results),'failure cannot be relabeled passing')
    chunks=re.findall(r"^---- ([^\n]+) stdout ----\n\nthread '([^'\n]+)' \(\d+\) panicked at ([^\n]+):\n(.*?)(?=^---- |^failures:)",text,re.M|re.S)
    require(len(chunks)==5 and len(re.findall(r"^thread '.+' .*panicked at",text,re.M))==5,'unexpected panic blocks')
    classified=[]
    for heading,identity,location,body in chunks:
        require(heading==identity and identity in EXPECTED,'panic identity mismatch')
        expected_location,expected_message=EXPECTED[identity]
        require(location==expected_location,'unexpected failure location')
        lines=[line for line in body.splitlines() if line and line!=NOTE]
        require(lines==[expected_message],'unknown failure message/body: '+identity)
        classified.append({'test':identity,'location':location,'message':expected_message,'outcome':'failed-pending-binding-not-waived'})
    lists=re.findall(r'^failures:\n((?:    [^\n]+\n)+)',text,re.M)
    require([line.strip() for block in lists for line in block.splitlines()]==[
        'coverage::ownership::tests::inventory_extension_preserves_live_owners_and_approved_exclusions',
        'coverage::ownership::tests::test_owner_checks_compiled_module_exact_selector_and_source_digest',
        'ownership_recording_rejects_unknown_obligations_without_changing_the_ledger',
        'ownership_recording_validates_the_entire_batch_before_writing',
        'representative_registry_is_consumed_through_approval_aware_native_execution'], 'failure summary identities differ')
    expected_errors=['error: stale ownership evidence','error: stale adapter production/source bindings',
        *['error: test failed, to rerun pass `'+t+'`' for t in TARGETS],'error: 3 targets failed:']
    require(sorted(re.findall(r'^error:.*$',text,re.M))==sorted(expected_errors),'unknown error outside classified failures')
    require(text.rstrip().endswith('error: 3 targets failed:\n'+'\n'.join('    `'+t+'`' for t in TARGETS)),'command did not finish with exact target summary')
    for marker,count in [('Running unittests src/lib.rs (target/debug/deps/excalidraw_render-',404),('Running tests/fast/main.rs',595)]:
        start=text.index(marker); end=text.index('test result:',start)
        section=text[start:end]
        require(len(re.findall(r'^test .+ \.\.\. ok$',section,re.M))==count,'renderer pass count differs')
    return {'passed':1226,'failed':5,'ignored':19,'executed_pass_or_fail':1231,'including_ignored':1250,
            'target_summaries':39,'renderer_library_passed':404,'renderer_fast_passed':595,
            'failures':sorted(classified,key=lambda r:r['test']),'all_tests_green':False}

def frozen():
    source=P/'workflow.py';data=source.read_bytes();freeze=read(P/'final-freeze.json')
    require(sha(data)==freeze['producer_files'][str(source.relative_to(ROOT))],'frozen checker changed')
    module=types.ModuleType('authenticated_icc_frozen_checker');module.__file__=str(source)
    exec(compile(data,module.__file__,'exec',optimize=0),module.__dict__)
    return module

def verify_suite(applied=False):
    a=read(P/'checks/full-suite-record.json')
    require(a['approval'] is None and a['command']==['cargo','test','--locked','--workspace','--no-fail-fast'],'command/approval drift')
    require(a['completed_normally'] is True and a['timeout_ms']==1800000 and a['outcome']=='failed-only-at-exact-pending-binding-tests','timeout/failures cannot be waived')
    require(a['environment']=={'toolchain':'1.97.1-x86_64-unknown-linux-gnu','CARGO_INCREMENTAL':'0','CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0','CARGO_PROFILE_RELEASE_DEBUG':'0'},'test environment differs')
    require(classify(check(a['raw_log']))==a['classification'],'raw result classification differs')
    check(a['parent_message']);check(a['freeze']);check(a['test_source'])
    require(a['test_source']['path']=='crates/excalidraw-render/tests/fast/jpeg_icc_contract.rs','wrong qualification test')
    require(a['producer_sources']==read(ROOT/'tests/conformance/candidates/stage4-frames-text-assets-v10/native-capture.json')['producer_sources'],'not bound to final producer set')
    for d in a['producer_sources']:check(d)
    for p,h in read(P/'final-freeze.json')['producer_files'].items():
        if applied and p in ['tests/conformance/coverage/ownership.json','tests/conformance/candidates/registry.json']:continue
        require(sha((ROOT/p).read_bytes())==h,'source changed '+p)
    # One failed test asserts on stderr but does not print its content. Bind an
    # independent reproduction of that exact CLI to support the causal attribution.
    r=a['unknown-owner-cli-reproduction']
    require(r['command']==['cargo','run','--release','--locked','-p','xtask','--','conformance','record-test-owner','surface_metadata_contract','editor_state_is_silent_preserved_and_independent_of_scene_and_frame_exports','Explicit reviewed editor-only surface contract','surface:unknown:/unknown:svg'],'wrong diagnostic invocation')
    require(r['exit_code']!=0 and check(r['stdout'])==b'','diagnostic did not fail closed')
    lines=check(r['stderr']).decode().splitlines()
    require([s for s in lines if s.startswith('error:')]==['error: stale ownership evidence'],'unknown-owner CLI failure cause differs')
    require(r['ledger_before_sha256']==r['ledger_after_sha256'],'diagnostic mutated ledger')
    require(a['post_approval_required']==['cargo test --locked -p xtask --bin xtask --test coverage --test representative_candidate --no-fail-fast'],'affected retest requirement changed')
    return a['classification']

if __name__=='__main__':
    require(sys.argv[1:] in [['seal'],['verify'],['verify-applied']],'usage: suite-verification.py seal|verify|verify-applied')
    applied=sys.argv[1]=='verify-applied'
    q=frozen()
    if applied:q.verify(True)  # strict real decision authentication precedes audit
    counts=verify_suite(applied)
    if sys.argv[1]=='seal':q.seal()
    elif not applied:q.verify(False)
    print(f"Full suite verified: {counts['passed']} passed / {counts['failed']} exact pending failures / {counts['ignored']} ignored; failures not waived.")
