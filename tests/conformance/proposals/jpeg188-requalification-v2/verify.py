#!/usr/bin/env python3
"""Verify pending/applied qualification; apply only an affirmative reviewed decision.

Applied verification authenticates persisted successful rerun receipts and repeats
the same bounded checks independently. No status marker or summary grants a pass.
"""
import sys
if not __debug__ or sys.flags.optimize:
    raise SystemExit('Optimized Python forbidden; no workflow started.')

import contextlib
from datetime import datetime, timezone
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import types
import uuid

ROOT=Path(__file__).resolve().parents[4]
P=Path(__file__).resolve().parent
BASE=ROOT/'tests/conformance/proposals/jpeg188-requalification-v1'
PRIOR='sha256:6dbac083ea0d744880bebe0aab6a259f9530bad0635540faeba156edad23c1fa'
V10='tests/conformance/candidates/stage4-frames-text-assets-v10'
REP='sha256:a6f7f0d2c2fd7f1a5142c24edfc4696e54d5bc1811271b12bd0f195db0276f15'
CASE='representative.stage4-frames-text-assets.v10'
LEDGER='tests/conformance/coverage/ownership.json'
REGISTRY='tests/conformance/candidates/registry.json'
APP='tests/conformance/coverage/jpeg188-application-v2'
TEST_COMMAND=['cargo','test','--locked','-p','xtask','--bin','xtask','--test','coverage','--test','representative_candidate','--no-fail-fast']
PREFIX=['cargo','run','--release','--locked','-p','xtask','--','conformance']
COMMANDS=[('affected-tests',TEST_COMMAND),('candidate',PREFIX+['candidate-verify',V10]),
          ('registry',PREFIX+['candidate-executions']),('owner',PREFIX+['verify-representative-owner',CASE]),
          ('inventory',PREFIX+['verify-inventory']),('ownership',PREFIX+['verify-ownership'])]
ENV_SETTINGS={'CARGO_INCREMENTAL':'0','CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0','CARGO_PROFILE_RELEASE_DEBUG':'0','RUSTFLAGS':'','PYTHONDONTWRITEBYTECODE':'1'}
TOOLCHAIN='/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin'
def require(value,message):
    if not value:raise ValueError(message)
def unique(pairs):
    result={}
    for key,value in pairs:
        require(key not in result,'duplicate JSON field '+key);result[key]=value
    return result
def parse(data):return json.loads(data,object_pairs_hook=unique)
def read(path):
    try:return parse(path.read_bytes())
    except (OSError,ValueError) as error:raise ValueError(f'missing/invalid {path.name}: {error}') from error
def sha(data):return hashlib.sha256(data).hexdigest()
def canonical(v):return json.dumps(v,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()
def pretty(v):return json.dumps(v,indent=2,ensure_ascii=False).encode()+b'\n'
def desc(path):
    data=path.read_bytes();return {'path':str(path.relative_to(ROOT)),'byte_length':len(data),'sha256':sha(data)}
def check(d):
    require(desc(ROOT/d['path'])==d,'stale descriptor '+d['path']);return (ROOT/d['path']).read_bytes()
def put(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as out:out.write(data);out.flush();os.fsync(out.fileno())
def environment():
    env=dict(os.environ,**ENV_SETTINGS);env['PATH']=TOOLCHAIN+':'+os.environ['PATH']
    for key in ['CARGO_ENCODED_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTC','CARGO_BUILD_RUSTC','CARGO_TARGET_DIR','CARGO_PROFILE_TEST_OPT_LEVEL']:
        env.pop(key,None)
    return env
def packet(applied):
    m=read(P/'manifest.json');bundle=read(P/'approval-subject.json')
    require(m['approval'] is None and m['prior_production_subject_sha256']==PRIOR and m['representative_subject_sha256']==REP,'packet identity changed')
    require(bundle=={'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in m.items() if k!='approval'},'files':m['files']},'packet envelope mismatch')
    for d in m['files']+m['preserved_files']:check(d)
    if not applied:check(m['pending_v10_manifest'])
    require('sha256:'+sha(canonical(read(BASE/'approval-subject.json')))==PRIOR,'v1 subject changed')
    return 'sha256:'+sha(canonical(bundle))
def load_original(name):
    path=BASE/name;relative=str(path.relative_to(ROOT))
    d=next(d for d in read(BASE/'approval-subject.json')['files'] if d['path']==relative);check(d)
    module=types.ModuleType('authenticated_'+name.replace('.','_'));module.__file__=str(path)
    exec(compile(path.read_bytes(),module.__file__,'exec',optimize=0),module.__dict__)
    return module
def evidence(applied):
    q=load_original('workflow.py');suite=load_original('suite-verification.py')
    with contextlib.redirect_stdout(io.StringIO()):
        suite.verify_suite(applied)
        if not applied:q.verify(False)
        else:
            m=q.read(BASE/'manifest.json');bundle=q.read(BASE/'approval-subject.json')
            require(m['approval'] is None and bundle=={'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in m.items() if k!='approval'},'files':m['files']},'original envelope mismatch')
            for d in m['files']:check(d)
            q.check_freeze(True);q.qualification();q.verify_migration(True)
            c=q.collector();c.verify_snapshots(BASE/'old/snapshots');c.verify_snapshots(BASE/'final-current/snapshots')
            require(sha(canonical(parse(q.task('ownership-catalog'))))==read(BASE/'migration-plan.json')['catalog_sha256'],'catalog changed')
            require(q.task('candidate-approval-subject',V10).decode().strip()==REP,'V10 subject changed')

def authenticate_decision(subject,approval,comment):
    q=load_original('workflow.py')
    q.authenticate(approval,comment,subject,REP,read(ROOT/V10/'manifest.json')['approval'])

def current_binding(subject):
    # Complete post-application source set plus actual approval-bearing metadata.
    # Original freeze includes all tracked tests/adapters/Cargo/source/evidence.
    frozen=read(BASE/'final-freeze.json')['producer_files']
    for path,digest in frozen.items():
        if path not in [LEDGER,REGISTRY]:require(sha((ROOT/path).read_bytes())==digest,'frozen source changed '+path)
    return {'production_subject_sha256':subject,'representative_subject_sha256':REP,
            'frozen_source_record':desc(BASE/'final-freeze.json'),
            'post_application_files':[desc(ROOT/path) for path in [V10+'/manifest.json',REGISTRY,LEDGER,
                'tests/conformance/coverage/inventory.json','tests/conformance/coverage/ownership-policy.json',
                'xtask/src/main.rs','xtask/src/coverage/ownership.rs','xtask/tests/coverage.rs','xtask/tests/representative_candidate.rs',
                'Cargo.toml','Cargo.lock']],
            'v2_verifier':desc(P/'verify.py'),'test_inventory':desc(P/'target-identities.json')}

def test_inventory():
    inventory=read(P/'target-identities.json')
    raw=check(inventory['source_log']).decode()
    selectors=[('xtask-bin','Running unittests src/main.rs (target/debug/deps/xtask-'),
               ('coverage','Running tests/coverage.rs'),('representative_candidate','Running tests/representative_candidate.rs')]
    expected=[]
    for (name,marker),count in zip(selectors,[103,5,1]):
        marker_at=raw.index(marker)
        starts=list(re.finditer(r'^running '+str(count)+r' tests?$',raw,re.M))
        # Cargo stderr may be interleaved after initial libtest stdout lines.
        start=min(starts,key=lambda m:abs(m.start()-marker_at)).start()
        require(abs(start-marker_at)<1200,'test target/count association too distant')
        end=raw.index('test result:',start)
        identities=re.findall(r'^test (.+) \.\.\. (?:ok|FAILED)$',raw[start:end],re.M)
        require(len(identities)==len(set(identities))==count,'bound original test inventory incomplete')
        expected.append({'target':name,'count':count,'identities':sorted(identities)})
    require(inventory['targets']==expected,'target identities differ from bound raw log')
    return expected

def validate_test_output(stdout,stderr):
    text=stdout.decode();err=stderr.decode();inventory=test_inventory()
    matches=list(re.finditer(r'^running (\d+) tests?$',text,re.M))
    require(len(matches)==3,'exact three test result blocks required')
    summaries=[]
    for i,(match,expected) in enumerate(zip(matches,inventory)):
        segment=text[match.end():matches[i+1].start() if i+1<len(matches) else len(text)]
        require(int(match[1])==expected['count'],'wrong test count')
        passed=re.findall(r'^test (.+) \.\.\. ok$',segment,re.M)
        require(len(passed)==len(set(passed))==expected['count'] and sorted(passed)==expected['identities'],'missing/duplicate/substituted successful test identities')
        summaries_found=re.findall(r'^test result: ok\. (\d+) passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [0-9.]+s$',segment,re.M)
        require(summaries_found==[str(expected['count'])],'exact successful target summary required')
        for line in segment.splitlines():
            if not line:continue
            if line.startswith('test result:'):continue
            if re.fullmatch(r'test .+ \.\.\. ok',line):continue
            if re.fullmatch(r'test .+ has been running for over \d+ seconds',line):continue
            raise ValueError('unexpected test output: '+line)
        summaries.append({'target':expected['target'],'passed':expected['count'],'identities_sha256':sha(canonical(expected['identities']))})
    require(not text[:matches[0].start()].strip(),'unexpected test stdout prefix')
    require(not re.search(r'(^error:|FAILED|panicked at|^failures:)',err,re.M),'stderr indicates failed test/compilation')
    running=re.findall(r'^\s*Running (.+)$',err,re.M)
    patterns=[r'unittests src/main\.rs \(target/debug/deps/xtask-[0-9a-f]+\)',
              r'tests/coverage\.rs \(target/debug/deps/coverage-[0-9a-f]+\)',
              r'tests/representative_candidate\.rs \(target/debug/deps/representative_candidate-[0-9a-f]+\)']
    require(len(running)==3 and all(re.fullmatch(pattern,row) for pattern,row in zip(patterns,running)),'wrong/missing executed test binaries')
    return {'targets':summaries,'passed':109,'failed':0,'ignored':0}

def expected_consumer():
    m=read(ROOT/V10/'manifest.json');native=read(ROOT/V10/'native-capture.json')
    judgment=sha(canonical(read(ROOT/V10/'judgments.json')));semantic=sha(canonical(read(ROOT/V10/'semantics.json')))
    rows=[]
    require(m['approval'] is not None,'approved consumer requires real manifest approval')
    for row,fmt in zip(native['rows'],['svg','png','pdf']):
        require(row['format']==fmt and row['fresh_process_runs']==2,'native record is incomplete')
        rows.append({'approval_subject_sha256':REP.removeprefix('sha256:'),'execution_key':CASE+':default:'+fmt,
                     'judgments_sha256':judgment,'semantics_sha256':semantic,
                     'native':{'format':fmt,'fresh_process_runs':2,'artifact_sha256':row['artifact']['sha256'],'report_sha256':row['report']['sha256']}})
    require(len(native['rows'])==3,'wrong native format set')
    return {'schema':'excalidraw-render/representative-executions/v1','approval_required':True,
            'approval_subject_sha256':REP.removeprefix('sha256:'),'case_id':CASE,'executions':rows}

def outcome(name,stdout,stderr):
    if name=='affected-tests':return validate_test_output(stdout,stderr)
    require(not re.search(rb'(^error:|panicked at|FAILED)',stderr,re.M),'consumer error in stderr')
    if name=='candidate':
        value=parse(stdout);require(value==expected_consumer(),'candidate receipts differ from current artifacts/subject');return value
    if name=='registry':
        value=parse(stdout);require(value=={'schema':'excalidraw-render/representative-corpus-results/v1','cases':[expected_consumer()]},'registry receipt set differs');return value
    expected={'owner':f'verified approved representative owner {CASE}\n',
              'inventory':'verified draft inventory: 320 coverage rows, 168 executions, 8 unresolved milestones; release closure is pending\n',
              'ownership':'verified 1992 ownership assignments: 1915 assigned/excluded, 77 explicit gaps; ledger approval pending\n'}
    require(stdout.decode()==expected[name],'wrong '+name+' verification output')
    return {'verified_output':stdout.decode()}

def validate_receipts(receipts,subject,directory):
    require(set(receipts)=={'schema','production_subject_sha256','representative_subject_sha256','binding','runs'},'unexpected receipt fields')
    require(receipts['schema']=='excalidraw-render/jpeg188-post-approval-receipts/v1','wrong receipt schema')
    binding=current_binding(subject)
    require(receipts['production_subject_sha256']==subject and receipts['representative_subject_sha256']==REP,'receipt subject mismatch')
    require(receipts['binding']==binding,'receipt post-application source/metadata binding mismatch')
    require(len(receipts['runs'])==len(COMMANDS),'missing or extra required command receipt')
    results=[]
    for row,(name,command) in zip(receipts['runs'],COMMANDS):
        require(set(row)=={'name','command','cwd','environment','exit_code','stdout','stderr','before','after','checked_outcome'},'unexpected run fields')
        require(row['name']==name and row['command']==command and row['cwd']=='.','wrong command/flags/order/cwd')
        require(row['environment']=={'toolchain':TOOLCHAIN,'settings':ENV_SETTINGS},'unqualified receipt environment')
        require(type(row['exit_code']) is int and row['exit_code']==0,'required command did not exit successfully')
        require(row['before']==binding and row['after']==binding,'source/metadata changed during required command')
        for key in ['stdout','stderr']:
            require(row[key]['path']==str((directory/'logs'/(name+'.'+key)).relative_to(ROOT)),'redirected command log')
        result=outcome(name,check(row['stdout']),check(row['stderr']))
        require(row['checked_outcome']==result,'forged command summary')
        results.append(result)
    return results

def process(command):
    return subprocess.run(command,cwd=ROOT,env=environment(),capture_output=True)

def run_required(subject,directory=None):
    binding=current_binding(subject);runs=[];results=[]
    for name,command in COMMANDS:
        before=current_binding(subject)
        result=process(command)
        if directory:
            put(directory/'logs'/(name+'.stdout'),result.stdout);put(directory/'logs'/(name+'.stderr'),result.stderr)
        require(result.returncode==0,f'{name} command failed: '+result.stderr.decode()+result.stdout.decode())
        checked=outcome(name,result.stdout,result.stderr)
        after=current_binding(subject);require(before==binding==after,'source/metadata changed while rerunning '+name)
        results.append(checked)
        if directory:runs.append({'name':name,'command':command,'cwd':'.','environment':{'toolchain':TOOLCHAIN,'settings':ENV_SETTINGS},'exit_code':result.returncode,
                                  'stdout':desc(directory/'logs'/(name+'.stdout')),'stderr':desc(directory/'logs'/(name+'.stderr')),
                                  'before':before,'after':after,'checked_outcome':checked})
    if directory:
        receipts={'schema':'excalidraw-render/jpeg188-post-approval-receipts/v1','production_subject_sha256':subject,'representative_subject_sha256':REP,'binding':binding,'runs':runs}
        put(directory/'receipts.json',pretty(receipts));validate_receipts(receipts,subject,directory)
    return results

def authorize(subject):
    app=ROOT/APP;state=read(app/'status.json')
    require(state.get('state')=='applied' and state.get('production_subject_sha256')==subject and state.get('representative_subject_sha256')==REP,'not successfully applied for these subjects')
    approval=read(app/'approval.json');comment=read(app/'comment.json');q=load_original('workflow.py')
    # Retain strict sagikazarmark + uniquely fenced affirmative jpeg188-icc policy.
    q.authenticate(approval,comment,subject,REP,read(ROOT/V10/'manifest.json')['approval'])

def verify(applied=False):
    subject=packet(applied)
    if applied:authorize(subject)
    evidence(applied)
    if applied:
        saved=validate_receipts(read(ROOT/APP/'receipts.json'),subject,ROOT/APP)
        fresh=run_required(subject)
        require(fresh==saved,'saved receipts disagree with independently rerun actual checks')
    print(subject+(' approved decision, saved receipts and independently rerun 109 tests/consumers verified' if applied else ' pending; no post-approval results claimed'))
    print(REP+' unchanged')
    return subject

def write_status(directory,state):
    tmp=directory/('status.tmp-'+uuid.uuid4().hex);put(tmp,pretty(state));tmp.replace(directory/'status.json')

def transaction(approval):
    candidate=read(ROOT/V10/'manifest.json');require(candidate['approval'] is None,'V10 already approved')
    candidate['approval']=dict(approval,approval_subject_sha256=REP)
    candidate_bytes=canonical(candidate)+b'\n'
    registry=(BASE/'pending/registry.json').read_bytes()
    ledger=read(BASE/'pending/ownership.template.json')
    owner=next(r for r in ledger['assignments'] if r['id']=='interaction:frames-text-assets')
    owner['disposition']['owner']['binding']['manifest']['sha256']=sha(candidate_bytes)
    return {V10+'/manifest.json':candidate_bytes,REGISTRY:registry,LEDGER:pretty(ledger)}

def apply(approval_path):
    app=ROOT/APP
    require(not app.exists(),'existing attempt: preserve and inspect; automatic overwrite/retry is forbidden')
    subject=verify(False)
    approval=read(Path(approval_path));q=load_original('workflow.py')
    require(isinstance(approval,dict) and set(approval)=={'approved_by','approved_at','approval_url','approval_subject_sha256'},'exact four-field approval required')
    comment=q.fetch_comment(approval['approval_url'])
    q.authenticate(approval,comment,subject,REP,dict(approval,approval_subject_sha256=REP))
    q.verify_migration(False)
    files=transaction(approval);before={p:(ROOT/p).read_bytes() for p in files}
    app.mkdir(parents=True,exist_ok=False)
    state={'schema':'excalidraw-render/jpeg188-receipt-transaction/v1','state':'preparing','attempt_id':uuid.uuid4().hex,
           'started_at':datetime.now(timezone.utc).isoformat(),'production_subject_sha256':subject,'representative_subject_sha256':REP,
           'before_sha256':{p:sha(b) for p,b in before.items()}}
    try:
        write_status(app,state)
        for p,b in before.items():put(app/'before'/p,b)
        put(app/'approval.json',pretty(approval));put(app/'comment.json',pretty(comment))
        for p,b in before.items():require((ROOT/p).read_bytes()==b,'live preimage changed before write')
        state['state']='checking';write_status(app,state)
        for p,b in files.items():(ROOT/p).write_bytes(b)
        evidence(True)
        run_required(subject,app)
        # Receipt parsing plus real runs above must finish before applied status.
        q.authenticate(approval,read(app/'comment.json'),subject,REP,read(ROOT/V10/'manifest.json')['approval'])
        evidence(True)
        validate_receipts(read(app/'receipts.json'),subject,app)
        state['state']='applied';write_status(app,state)
    except BaseException as error:
        failures=[]
        for p,b in before.items():
            try:
                (ROOT/p).write_bytes(b);require((ROOT/p).read_bytes()==b,'rollback bytes differ')
            except BaseException as rollback:failures.append(f'{p}: {rollback}')
        state.update(state='rollback_failed' if failures else 'failed_rolled_back',error=f'{type(error).__name__}: {error}',rollback_errors=failures)
        write_status(app,state)
        if failures:raise RuntimeError('rollback incomplete; manual recovery required') from error
        raise
    print(subject+' applied only after actual 109-test rerun and all required approved checks; receipts retained.')

if __name__=='__main__':
    args=sys.argv[1:]
    if args==['verify']:verify(False)
    elif args==['verify-applied']:verify(True)
    elif len(args)==2 and args[0]=='apply':apply(args[1])
    else:raise SystemExit('usage: verify.py verify | verify-applied | apply ACTUAL_APPROVAL_JSON')
