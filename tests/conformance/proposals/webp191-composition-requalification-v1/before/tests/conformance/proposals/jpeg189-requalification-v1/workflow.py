#!/usr/bin/env python3
"""#189 EXIF evidence and exact proposal; current collection requires parent freeze.

Authenticated reuse of prior collectors/qualification helpers. Strict applied
authorization and actual resolution receipts are handled by receipt_gate.py.
"""
import sys
if not __debug__ or sys.flags.optimize:
    raise SystemExit('Optimized Python forbidden; no workflow started.')
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import types

ROOT=Path(__file__).resolve().parents[4]
P=Path(__file__).resolve().parent
HEAD='7b432b6'
OLD=Path('/tmp/opencode/jpeg189-old-7b432b6')
V11='tests/conformance/candidates/stage4-frames-text-assets-v11'
PRIOR='tests/conformance/candidates/stage4-frames-text-assets-v10'
BRIDGE='tests/conformance/candidates/stage4-pdf-colorspace-bridge-v5'
SOURCE='crates/excalidraw-render/tests/fast/jpeg_exif_contract.rs'
LEDGER='tests/conformance/coverage/ownership.json'
REGISTRY='tests/conformance/candidates/registry.json'
POLICY='tests/conformance/coverage/ownership-policy.json'
PREFIX='codec:jpeg:exif-precedence:'
REFERENCE='tests/oracle/jpeg-exif/reference.json'
REFERENCE_HASH='cd811bca81e24aaeb652c86d3e6dfd7b4aeb65d8050764da34b129530aac6ecf'
HELPERS={'capture':'tests/conformance/proposals/jpeg187-requalification-v1/collect.py',
         'qualification':'tests/conformance/proposals/jpeg188-requalification-v1/workflow.py',
         'receipts':'tests/conformance/proposals/jpeg188-requalification-v2/verify.py'}
FAILED_SIGNALS={'duplicate-6-3','duplicate-3-6','invalid-then-valid','valid-then-invalid',
                'absent-tag-then-valid','valid-then-absent-tag','bad-magic-then-valid','valid-then-bad-magic','invalid-tag-then-valid'}
ENV=dict(os.environ,PATH='/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:'+os.environ['PATH'],
         CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_PROFILE_RELEASE_DEBUG='0',PYTHONDONTWRITEBYTECODE='1')
def require(ok,message):
    if not ok:raise ValueError(message)
def sha(data):return hashlib.sha256(data).hexdigest()
def canonical(v):return json.dumps(v,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()
def pretty(v):return json.dumps(v,indent=2,ensure_ascii=False).encode()+b'\n'
def read(path):
    try:return json.loads(path.read_bytes())
    except (OSError,ValueError) as e:raise ValueError(f'missing/invalid {path}: {e}') from e
def put(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as f:f.write(data)
def write(path,value):put(path,pretty(value))
def desc(path):
    b=path.read_bytes();return {'path':str(path.relative_to(ROOT)),'byte_length':len(b),'sha256':sha(b)}
def check(d):
    require(desc(ROOT/d['path'])==d,'stale descriptor '+d['path']);return (ROOT/d['path']).read_bytes()
def run(args,cwd=ROOT):
    result=subprocess.run(list(map(str,args)),cwd=cwd,env=ENV,capture_output=True)
    require(result.returncode==0,result.stderr.decode()+result.stdout.decode());return result.stdout
def task(*args):return run(['cargo','run','--release','--locked','-p','xtask','--','conformance',*args])

def archive():
    commit=run(['git','rev-parse',HEAD]).decode().strip()
    paths=[LEDGER,REGISTRY,POLICY,'tests/conformance/coverage/inventory.json','tests/conformance/coverage/requirements.json',
           'Cargo.toml','Cargo.lock','tests/package-consumer/Cargo.lock',SOURCE,
           'crates/excalidraw-render/src/render/asset/jpeg.rs','crates/excalidraw-render/src/render/asset/orientation.rs',
           'xtask/src/conformance_pdf_bridge.rs','tests/package-consumer/examples/native_snapshots.rs',*HELPERS.values()]
    for path in paths:put(P/'before'/path,run(['git','show',f'{commit}:{path}']))
    require(sha((ROOT/REFERENCE).read_bytes())==REFERENCE_HASH,'EXIF reference changed')
    write(P/'basis.json',{'approval':None,'base_commit':commit,'reference':desc(ROOT/REFERENCE),
        'helper_sources':{name:desc(P/'before'/path) for name,path in HELPERS.items()},
        'qualification_scope':'JPEG-specific APP1/TIFF precedence only; shared orientation/Cargo/reference unchanged'})

def helper(name):
    d=read(P/'basis.json')['helper_sources'][name]
    data=check(d);require(data==(ROOT/HELPERS[name]).read_bytes(),'original approved helper bytes changed')
    m=types.ModuleType('authenticated_jpeg189_'+name);m.__file__=str(ROOT/HELPERS[name])
    exec(compile(data,m.__file__,'exec',optimize=0),m.__dict__)
    return m

def collector():
    c=helper('capture');c.PACKET=P;c.ROOT=ROOT;return c

def gap_ids(reference=None):
    record=reference if reference is not None else read(ROOT/REFERENCE)
    cells={PREFIX+r['cell'] for r in record['cases']}
    gaps={PREFIX+r['cell'] for r in record['cases'] if r['signal'] in FAILED_SIGNALS}
    require(len(record['cases'])==164 and len(cells)==82 and len(gaps)==36,'closed 82-cell EXIF matrix differs')
    return gaps

def check_graph():
    for path in ['Cargo.toml','Cargo.lock','tests/package-consumer/Cargo.lock','crates/excalidraw-render/src/render/asset/orientation.rs']:
        require((ROOT/path).read_bytes()==(P/'before'/path).read_bytes(),'out-of-scope graph/shared orientation change '+path)
    require(sha((ROOT/REFERENCE).read_bytes())==REFERENCE_HASH,'reference changed')

def validate_selectors(mapping,source):
    require(set(mapping)==gap_ids(),'exact 36 named EXIF selectors required')
    for identity,fn in mapping.items():
        require(isinstance(fn,str) and re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*',fn) and not any(s in fn for s in ['unqualified','failure','diagnostic']),'passing owner selector required')
        require(len(re.findall(r'#\[test\]\s*(?:pub\s+)?fn\s+'+re.escape(fn)+r'\s*\(',source))==1,'new selector must be unconditional test: '+identity)

def qualification_helper():
    q=helper('qualification')
    # Runtime parameters only; preserve all imported bytes. Do not call its
    # #188-specific migration, authorization or seal implementations.
    q.ROOT=ROOT;q.P=P;q.HEAD=HEAD;q.OLD=OLD;q.V10=V11;q.BRIDGE=BRIDGE;q.SOURCE=SOURCE
    q.REFERENCE=REFERENCE;q.REFERENCE_HASH=REFERENCE_HASH
    q.collector=collector;q.check_graph=check_graph;q.validate_selectors=validate_selectors;q.check_freeze=check_freeze
    return q

def check_freeze(applied=False):
    f=read(P/'final-freeze.json');require(f['final_current_capture_authorized'] is True,'parent freeze required')
    for path,digest in f['producer_files'].items():
        if applied and path in [LEDGER,REGISTRY]:continue
        require(sha((ROOT/path).read_bytes())==digest,'frozen source changed '+path)
    require(set(collector().tracked_state(ROOT))<=set(f['producer_files']),'incomplete tracked-source freeze')
    check_graph();validate_selectors(f['new_owner_selectors'],(ROOT/SOURCE).read_text())
    return f

def old_capture():
    c=collector();basis=read(P/'basis.json')
    require(run(['git','rev-parse','HEAD'],OLD).decode().strip()==basis['base_commit'],'OLD commit mismatch')
    require(not run(['git','status','--porcelain'],OLD),'OLD dirty')
    before=c.tracked_state(OLD);write(P/'old/producer-files.json',before)
    require(c.package(OLD,'old'),'OLD package failed');c.capture(OLD,'old')
    require(c.tracked_state(OLD)==before,'OLD source drift')
    prior=ROOT/'tests/conformance/proposals/jpeg188-requalification-v1/final-current/snapshots/snapshots.json'
    a=read(P/'old/snapshots/snapshots.json')['runs'][0]['results'];b=read(prior)['runs'][0]['results']
    require(len(a)==len(b)==168 and [r['key'] for r in a]==[r['key'] for r in b],'prior capture key mismatch')
    write(P/'old/previous-capture-comparison.json',{'approval':None,'fresh_base':basis['base_commit'],
        'fresh_capture':desc(P/'old/snapshots/snapshots.json'),'prior_capture':desc(prior),
        'identical_records':sum(x==y for x,y in zip(a,b)),'total':168,'note':'fresh HEAD provenance, never relabel old producer'})

def freeze(authorization):
    qualification_helper().freeze(authorization)

def representative():
    check_freeze()
    for name,args in [('bridge-v5',['collect-pdf-colorspace-bridge']),('revise-v11',['candidate-revise',V11])]:put(P/'checks'/(name+'.json'),task(*args))
    for name in ['assets','viewer-evidence']:(ROOT/V11/name).mkdir()
    prefix=['cargo','run','--release','--locked','-p','xtask','--','conformance']
    for name,args in [('native',prefix+['candidate-native-capture',V11]),('assets',['node','tools/reference-oracle/scripts/stage4-viewer-assets.mjs',V11]),
        ('derive',prefix+['candidate-prepare-assets',V11]),('fonts',['node','tools/reference-oracle/scripts/stage4-font-audit-qualification.mjs',V11,'collect']),
        ('viewers',['node','tools/reference-oracle/scripts/stage4-candidate-views.mjs','collect',V11]),('exceptions',prefix+['candidate-propose-exceptions',V11]),
        ('inspect',prefix+['candidate-inspect',V11]),('materialize',prefix+['candidate-materialize',V11]),('subject',prefix+['candidate-approval-subject',V11])]:
        put(P/'checks'/('v11-'+name+'.json'),run(args));check_freeze()

def protected():
    check_freeze();exe=ROOT/'target/release/excalidraw-render';build=read(ROOT/V11/'native-capture.json')['build']
    require(sha(exe.read_bytes())==build['executable_sha256'],'release executable differs')
    original=ROOT/'tests/conformance/candidates/stage4-pdf-backend-requalification-v2';out=P/'protected-final'
    scene=(ROOT/'tests/conformance/pdf-backend-qualification/project-operations.excalidraw').read_bytes()
    for i in range(2):
        r=subprocess.run([str(exe),'-','--output','-','--format','pdf','--report-format','json','--pdf-raster-fallback','forbid'],cwd=ROOT,env=ENV,input=scene,capture_output=True)
        require(r.returncode==0 and r.stdout==(original/'candidate.pdf').read_bytes() and r.stderr==(original/'candidate-report.json').read_bytes(),'protected PDF/report changed; STOP')
        put(out/f'run-{i}.pdf',r.stdout);put(out/f'run-{i}-report.json',r.stderr)
    run(['bash','tools/pdf-harness/run-structure.sh','inspect',out/'run-0.pdf',ROOT/'tests/conformance/pdf-backend-qualification/project-operations-structure.json','pass',out/'structure.json'])
    for viewer in ['pdfium','pdfjs']:
        for i in range(2):
            destination=out/f'{viewer}-{i}.rgba'
            put(out/f'{viewer}-{i}-runtime.json',run(['bash',f'tools/pdf-harness/run-{viewer}.sh','render',out/'run-0.pdf',destination,'520','200']))
            require(destination.read_bytes()==(original/f'candidate-{viewer}.rgba').read_bytes(),'protected viewer changed; STOP')
    for fmt in ['svg','png','pdf']:
        for name in [f'native.{fmt}',f'native-{fmt}-report.json']:require((ROOT/V11/name).read_bytes()==(ROOT/PRIOR/name).read_bytes(),'representative output changed; STOP')
    write(P/'checks/protected-equality.json',{'approval':None,'native_repeat_runs':2,'viewer_repeat_runs_each':2,'exact_protected_equality':True,'representative_equal_v10':True,'build':build})
    check_freeze()

def expected_transaction(before,registry,source_hash,selectors,candidate,subject,file_hash):
    require(before['approval'] is None and len(before['assignments'])==2074,'before ledger mismatch')
    rows=[r for r in before['assignments'] if r['id'].startswith(PREFIX)];reference=read(ROOT/REFERENCE)
    require(len(rows)==82 and {r['id'] for r in rows}=={PREFIX+r['cell'] for r in reference['cases']},'82 exact EXIF criteria required')
    gaps=gap_ids(reference)
    require({r['id'] for r in rows if r['disposition']['status']=='gap'}==gaps and set(selectors)==gaps,'36 exact #189 gaps required')
    require(sum(r['disposition']['status']=='owned' for r in rows)==46,'46 existing EXIF owners required')
    require(all(re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*',s) and not any(w in s for w in ['unqualified','failure','diagnostic']) for s in selectors.values()),'new selector not qualified')
    require(re.fullmatch(r'[0-9a-f]{64}',source_hash) and re.fullmatch(r'[0-9a-f]{64}',file_hash) and re.fullmatch(r'sha256:[0-9a-f]{64}',subject),'invalid digest')
    new=copy.deepcopy(before)
    command=lambda fn:['cargo','test','--locked','--release','-p','excalidraw-render','--test','fast','jpeg_exif_contract::'+fn,'--','--exact']
    for row in new['assignments']:
        if not row['id'].startswith(PREFIX):continue
        d=row['disposition']
        if d['status']=='owned':
            owner=d['owner'];require(owner['kind']=='test' and owner['source']['path']==SOURCE and owner['binary']=='fast' and owner['command']==command(owner['function']),'old owner selector/command mismatch')
            owner['source']['sha256']=source_hash
        else:
            require(d['status']=='gap' and d['issue']==189,'only #189 gaps may be assigned')
            fn=selectors[row['id']]
            row['disposition']={'status':'owned','owner':{'kind':'test','source':{'path':SOURCE,'sha256':source_hash},'function':fn,'binary':'fast','command':command(fn)},
                'rationale':'User-reviewed #189 bounded JPEG first-header-EXIF and first-valid-IFD orientation correction; both independent fixed patterns, exact reconstruction/source/extent/alpha/report/replay and canonical PNG/bound-PDF sample gates. See tests/conformance/proposals/jpeg189-requalification-v1/; no exhaustive TIFF/IFD-chain or whole-PDF closure implied.'}
    out=copy.deepcopy(registry);require(len(out['cases'])==1 and out['cases'][0]['id']=='representative.stage4-frames-text-assets.v10','prior representative mismatch')
    r=out['cases'][0];require(candidate['case_id']=='representative.stage4-frames-text-assets.v11','successor identity mismatch')
    exceptions=copy.deepcopy(r['exceptions'])
    for e in exceptions:e['case_id']=candidate['case_id']
    require(candidate['exception_proposal']['exceptions']==exceptions,'existing representative scopes changed')
    r.update(id=candidate['case_id'],directory=V11,status='jpeg189-exif-release-successor',exceptions=exceptions,
        execution_keys=[candidate['case_id']+':default:'+f for f in ['svg','png','pdf']],consumer=['cargo','run','--release','--locked','-p','xtask','--','conformance','candidate-verify',V11])
    owner=next(r for r in new['assignments'] if r['id']=='interaction:frames-text-assets')['disposition']['owner']
    require(owner['kind']=='representative' and owner['binding']['case']=='representative.stage4-frames-text-assets.v10','prior owner mismatch')
    owner['binding']={'case':candidate['case_id'],'registry':{'path':REGISTRY,'sha256':sha(pretty(out))},'manifest':{'path':V11+'/manifest.json','sha256':file_hash},'approval_subject_sha256':subject,'command':r['consumer']}
    require({s:sum(r['disposition']['status']==s for r in new['assignments']) for s in ['owned','gap','excluded']}=={'owned':1994,'gap':77,'excluded':3},'wrong migration totals')
    return new,out

def migration():
    f=check_freeze();before=read(P/'before'/LEDGER);registry=read(P/'before'/REGISTRY)
    require(read(ROOT/LEDGER)==before and read(ROOT/REGISTRY)==registry,'pending live metadata changed')
    candidate=read(ROOT/V11/'manifest.json');require(candidate['approval'] is None,'pending candidate approved')
    subject=task('candidate-approval-subject',V11).decode().strip()
    require(sha(canonical(json.loads(task('ownership-catalog'))))==before['catalog_sha256'],'policy/catalog changed')
    ledger,out=expected_transaction(before,registry,sha((ROOT/SOURCE).read_bytes()),f['new_owner_selectors'],candidate,subject,sha((ROOT/V11/'manifest.json').read_bytes()))
    write(P/'pending/ownership.template.json',ledger);write(P/'pending/registry.json',out)
    write(P/'migration-plan.json',{'approval':None,'source':desc(ROOT/SOURCE),'new_owner_selectors':f['new_owner_selectors'],'new_owner_ids':sorted(gap_ids()),
        'refresh_owner_ids':[r['id'] for r in before['assignments'] if r['id'].startswith(PREFIX) and r['disposition']['status']=='owned'],
        'representative_subject_sha256':subject,'catalog_sha256':before['catalog_sha256'],
        'counts':{'assignments':2074,'owned':1994,'excluded':3,'assigned_or_excluded':1997,'gaps':77}})
    verify_migration(False)

def verify_migration(applied):
    f=check_freeze(applied);plan=read(P/'migration-plan.json');before=read(P/'before'/LEDGER);registry=read(P/'before'/REGISTRY)
    require(desc(ROOT/SOURCE)==plan['source'] and plan['new_owner_selectors']==f['new_owner_selectors'],'source/selector plan differs')
    candidate=read(ROOT/V11/'manifest.json')
    expected,out=expected_transaction(before,registry,plan['source']['sha256'],plan['new_owner_selectors'],candidate,plan['representative_subject_sha256'],sha((ROOT/V11/'manifest.json').read_bytes()))
    require(plan['new_owner_ids']==sorted(gap_ids()),'new owner scope differs')
    require(plan['refresh_owner_ids']==[r['id'] for r in before['assignments'] if r['id'].startswith(PREFIX) and r['disposition']['status']=='owned'],'refresh scope differs')
    for path in [POLICY,'tests/conformance/coverage/inventory.json','tests/conformance/coverage/requirements.json']:require((ROOT/path).read_bytes()==(P/'before'/path).read_bytes(),'criteria/keys changed')
    require(read(P/'pending/registry.json')==out,'registry template differs')
    if applied:
        require(candidate['approval'] is not None and read(ROOT/LEDGER)==expected and read(ROOT/REGISTRY)==out,'applied transaction differs')
    else:
        require(candidate['approval'] is None and read(P/'pending/ownership.template.json')==expected,'pending template differs')
        require(read(ROOT/LEDGER)==before and read(ROOT/REGISTRY)==registry,'pending live metadata mutation')

def affirmative(body,subject,rep):
    require(isinstance(body,str),'decision text required')
    match=re.fullmatch(r'\s*```json\r?\n([^`]+)\r?\n```\s*',body);require(match is not None,'one standalone JSON decision required')
    def unique(pairs):
        result={}
        for k,v in pairs:require(k not in result,'duplicate decision field');result[k]=v
        return result
    decision=json.loads(match[1],object_pairs_hook=unique)
    require(decision=={'decision':'approve','scope':'jpeg189-exif','production_subject_sha256':subject,'representative_subject_sha256':rep},'exact affirmative jpeg189-exif decision required')

def authentication_helper():
    q=helper('qualification');q.ROOT=ROOT;q.affirmative=affirmative
    return q
def fetch_comment(url):return authentication_helper().fetch_comment(url)
def authenticate(approval,comment,subject,rep,rep_approval):return authentication_helper().authenticate(approval,comment,subject,rep,rep_approval)

def qualification():
    review=read(P/'checks/parent-qualification.json');native=read(ROOT/V11/'native-capture.json')
    require(review['test_source']==desc(ROOT/SOURCE) and review['producer_sources']==native['producer_sources'],'final qualification sources differ')
    for d in review['producer_sources']:check(d)
    require(review['exif_reference_sha256']==REFERENCE_HASH and review['exif_vectors']==164 and review['exif_canonical_gates_passed'] is True,'164 EXIF canonical/reconstruction gates required')
    require(review['icc_reference_sha256']=='7ffd57eaa8ddfe4a7d311c5a55c57b84a001463864ce6bd235613faab85e76d5' and review['icc_vectors']==160 and review['icc_color_vectors']==104 and review['icc_recovery_vectors']==56 and review['icc_gates_passed'] is True,'prior ICC regression qualification incomplete')
    require(review['jpeg187_vectors']==76 and review['jpeg187_canonical_vectors']==64 and review['jpeg187_known_failure_vectors']==12 and review['jpeg187_exact_pins']==24,'#187 known failures must remain explicit')
    require(review['checks'] and all(r['passed'] is True for r in review['checks']),'passing focused logs required')
    for row in review['checks']:check(row['log'])
    return review

def packet(applied=False):
    m=read(P/'manifest.json');bundle=read(P/'approval-subject.json')
    require(m['approval'] is None and m['approval_scope']=='jpeg189-exif','immutable packet identity differs')
    require(bundle=={'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in m.items() if k!='approval'},'files':m['files']},'subject envelope differs')
    for d in m['files']:check(d)
    if not applied:check(m['pending_representative_manifest'])
    return 'sha256:'+sha(canonical(bundle))

def evidence(applied=False):
    check_freeze(applied);qualification();verify_migration(applied)
    c=collector();c.verify_snapshots(P/'old/snapshots');c.verify_snapshots(P/'final-current/snapshots')
    require(sha(canonical(json.loads(task('ownership-catalog'))))==read(P/'migration-plan.json')['catalog_sha256'],'catalog mismatch')
    require(task('candidate-approval-subject',V11).decode().strip()==read(P/'migration-plan.json')['representative_subject_sha256'],'V11 subject differs')

def seal():
    require(not (P/'manifest.json').exists(),'refuse overwrite')
    check_freeze();review=qualification();verify_migration(False)
    require(read(P/'checks/protected-equality.json')['exact_protected_equality'] is True,'protected equality missing')
    gate=receipt_gate();gate.test_inventory()
    a=read(P/'old/snapshots/snapshots.json')['runs'][0]['results'];b=read(P/'final-current/snapshots/snapshots.json')['runs'][0]['results']
    require(len(a)==len(b)==168 and [r['key'] for r in a]==[r['key'] for r in b],'168 complete full keys differ')
    changes=[{'key':x['key'],'before':x['samples'][0],'after':y['samples'][0]} for x,y in zip(a,b) if x!=y]
    require(review['diagnosed_inventory_changes']==changes,'unexplained inventory output changes; STOP')
    write(P/'inventory-comparison.json',{'approval':None,'keys':168,'unchanged':168-len(changes),'changes':changes})
    files=[desc(path) for base in [P,ROOT/V11,ROOT/BRIDGE] for path in sorted(base.rglob('*')) if path.is_file() and path.name not in ['README.md','file-inventory.json'] and '__pycache__' not in path.parts and path!=ROOT/V11/'manifest.json']
    m={'schema':'excalidraw-render/jpeg189-exif-requalification/v1','approval':None,'status':'pending-explicit-user-approval','approval_scope':'jpeg189-exif',
       'representative_subject_sha256':read(P/'migration-plan.json')['representative_subject_sha256'],'new_owners':36,'source_refreshes':46,
       'baseline_refresh':False,'global_approval':False,'files':files,'pending_representative_manifest':desc(ROOT/V11/'manifest.json')}
    write(P/'manifest.json',m);write(P/'approval-subject.json',{'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in m.items() if k!='approval'},'files':files})
    receipt_gate().verify(False)

def receipt_gate():
    # Reuse receipt parsing, before/after bindings, fresh reruns and rollback
    # verbatim. Replace only the named version/count output and runtime identities.
    original=helper('receipts')
    raw=(P/'before'/HELPERS['receipts']).read_text()
    patches={"jpeg188-post-approval-receipts/v1":"jpeg189-post-approval-receipts/v1",
             "jpeg188-receipt-transaction/v1":"jpeg189-receipt-transaction/v1",
             "verified 1992 ownership assignments: 1915 assigned/excluded, 77 explicit gaps; ledger approval pending":"verified 2074 ownership assignments: 1997 assigned/excluded, 77 explicit gaps; ledger approval pending"}
    for before,after in patches.items():
        require(raw.count(before)==(2 if before=='jpeg188-post-approval-receipts/v1' else 1),'receipt adaptation seam changed')
        raw=raw.replace(before,after)
    gate=types.ModuleType('jpeg189_strict_receipt_adapter');gate.__file__=str(P/'receipt_gate.py')
    exec(compile(raw,gate.__file__,'exec',optimize=0),gate.__dict__)
    gate.ROOT=ROOT;gate.P=P;gate.BASE=P;gate.V10=V11;gate.CASE='representative.stage4-frames-text-assets.v11'
    gate.LEDGER=LEDGER;gate.REGISTRY=REGISTRY;gate.APP='tests/conformance/coverage/jpeg189-application-v1'
    gate.REP=read(P/'migration-plan.json')['representative_subject_sha256'] if (P/'migration-plan.json').exists() else 'UNMATERIALIZED-V11-NO-APPROVAL-SUBJECT'
    gate.COMMANDS=[('affected-tests',gate.TEST_COMMAND),('candidate',gate.PREFIX+['candidate-verify',V11]),('registry',gate.PREFIX+['candidate-executions']),
        ('owner',gate.PREFIX+['verify-representative-owner',gate.CASE]),('inventory',gate.PREFIX+['verify-inventory']),('ownership',gate.PREFIX+['verify-ownership'])]
    gate.packet=packet;gate.evidence=evidence
    api=types.SimpleNamespace(authenticate=authenticate,fetch_comment=fetch_comment,verify_migration=verify_migration)
    gate.load_original=lambda name:api if name=='workflow.py' else (_ for _ in ()).throw(ValueError('unexpected delegated helper'))
    gate.current_binding=current_binding
    return gate

def current_binding(subject):
    check_freeze(True)
    rep=read(P/'migration-plan.json')['representative_subject_sha256']
    return {'production_subject_sha256':subject,'representative_subject_sha256':rep,'frozen_source_record':desc(P/'final-freeze.json'),
        'post_application_files':[desc(ROOT/path) for path in [V11+'/manifest.json',REGISTRY,LEDGER,POLICY,
            'tests/conformance/coverage/inventory.json','xtask/src/main.rs','xtask/src/coverage/ownership.rs','xtask/tests/coverage.rs','xtask/tests/representative_candidate.rs',SOURCE,'Cargo.toml','Cargo.lock']],
        'workflow':desc(P/'workflow.py'),'receipt_entry':desc(P/'receipt_gate.py'),'authenticated_prior_receipt_helper':read(P/'basis.json')['helper_sources']['receipts'],
        'test_inventory':desc(P/'target-identities.json')}

def readiness():
    require(not (P/'final-freeze.json').exists(),'pre-freeze readiness only')
    inventory_path='tests/conformance/proposals/jpeg188-requalification-v2/target-identities.json'
    original=run(['git','show',read(P/'basis.json')['base_commit']+':'+inventory_path])
    require((ROOT/inventory_path).read_bytes()==original,'original 109 test inventory changed')
    put(P/'target-identities.json',original)
    inventory=receipt_gate().test_inventory();require([r['count'] for r in inventory]==[103,5,1],'unexpected bounded test inventory')
    result=subprocess.run([sys.executable,str(P/'test_workflow.py')],cwd=ROOT,env=ENV,capture_output=True)
    put(P/'checks/readiness-tests.stdout',result.stdout);put(P/'checks/readiness-tests.stderr',result.stderr)
    require(result.returncode==0,result.stderr.decode())
    check_graph()
    before=(P/'before/xtask/src/conformance_pdf_bridge.rs').read_bytes()
    require((ROOT/'xtask/src/conformance_pdf_bridge.rs').read_bytes()==before.replace(b'stage4-pdf-colorspace-bridge-v4',b'stage4-pdf-colorspace-bridge-v5'),'bridge changed beyond version')
    for path in [LEDGER,REGISTRY,POLICY,'tests/conformance/coverage/inventory.json','tests/conformance/coverage/requirements.json']:
        require((ROOT/path).read_bytes()==(P/'before'/path).read_bytes(),'live metadata changed '+path)
    write(P/'readiness.json',{'approval':None,'status':'waiting-for-parent-review-checks-and-explicit-freeze','current_capture_started':False,
        'reference':desc(ROOT/REFERENCE),'old_capture':desc(P/'old/snapshots/snapshots.json'),
        'old_records_equal_prior':read(P/'old/previous-capture-comparison.json')['identical_records'],
        'checker_tests_passed':25,'test_log':desc(P/'checks/readiness-tests.stderr'),
        'helper_adaptations':{'capture':'authenticated prior bytes; output-root parameter only',
            'qualification':'authenticated prior freeze/stage/capture; overridden #189 identities/check callbacks only',
            'receipts':'authenticated jpeg188 v2 parser/109 IDs/commands/fresh rerun/rollback; explicit schema and ownership-output count substitutions plus #189 runtime identities'},
        'new_owner_ids':sorted(gap_ids()),'new_owner_selectors':'parent must supply exact qualified selector map at freeze',
        'counts_after':{'assignments':2074,'owned':1994,'excluded':3,'assigned_or_excluded':1997,'gaps':77}})
    write(P/'file-inventory.json',{'approval':None,'files':[desc(path) for path in sorted(P.rglob('*')) if path.is_file() and path.name!='file-inventory.json' and '__pycache__' not in path.parts]})
    print('Ready: OLD 168 capture; 25 tests; bridge v5 only; current capture blocked pending parent freeze/selectors.')

if __name__=='__main__':
    args=sys.argv[1:]
    if args==['archive']:archive()
    elif args==['old']:old_capture()
    elif len(args)==2 and args[0]=='freeze':freeze(args[1])
    elif len(args)==2 and args[0]=='stage':qualification_helper().stage(args[1])
    elif len(args)==2 and args[0]=='final':qualification_helper().final_capture(args[1])
    elif args==['representative']:representative()
    elif args==['protected']:protected()
    elif args==['migration']:migration()
    elif args==['seal']:seal()
    elif args==['readiness']:readiness()
    else:raise SystemExit('archive|old|freeze AUTH|stage ROOT|final ROOT|representative|protected|migration|seal')
