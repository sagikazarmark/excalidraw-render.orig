#!/usr/bin/env python3
"""Bounded #188 evidence and pending metadata proposals; never grants approval."""
import sys
if not __debug__ or sys.flags.optimize:
    raise SystemExit('Optimized Python forbidden; no workflow started.')
import copy
from datetime import date
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import types

ROOT = Path(__file__).resolve().parents[4]
P = Path(__file__).resolve().parent
HEAD = 'b01f2db'
OLD = Path('/tmp/opencode/jpeg188-old-b01f2db')
V10 = 'tests/conformance/candidates/stage4-frames-text-assets-v10'
BRIDGE = 'tests/conformance/candidates/stage4-pdf-colorspace-bridge-v4'
SOURCE = 'crates/excalidraw-render/tests/fast/jpeg_icc_contract.rs'
LEDGER = 'tests/conformance/coverage/ownership.json'
REGISTRY = 'tests/conformance/candidates/registry.json'
POLICY = 'tests/conformance/coverage/ownership-policy.json'
PREFIX = 'codec:jpeg:icc:'
GAPS = {PREFIX + f'gray-{mode}-{version}-g1-{layout}' for mode in ['baseline','progressive']
        for version in ['v2','v4'] for layout in ['single','ordered','reverse']} | {
        PREFIX + f'{model}-{mode}-opposite-model-single' for model in ['gray','rgb'] for mode in ['baseline','progressive']}
REFERENCE = 'tests/oracle/jpeg-icc/reference.json'
REFERENCE_HASH = '7ffd57eaa8ddfe4a7d311c5a55c57b84a001463864ce6bd235613faab85e76d5'
REUSED = ['tests/conformance/proposals/jpeg187-requalification-v1/collect.py',
          'tests/package-consumer/examples/native_snapshots.rs']
ENV = dict(os.environ, PATH='/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:' + os.environ['PATH'],
           CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_PROFILE_RELEASE_DEBUG='0')

def require(value, message):
    if not value: raise ValueError(message)
def sha(data): return hashlib.sha256(data).hexdigest()
def canonical(value): return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
def pretty(value): return json.dumps(value, indent=2, ensure_ascii=False).encode() + b'\n'
def read(path):
    try: return json.loads(path.read_bytes())
    except (OSError, ValueError) as error: raise ValueError(f'missing/invalid {path}: {error}') from error
def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as f: f.write(data)
def write(path, value): put(path, pretty(value))
def desc(path):
    data = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'byte_length': len(data), 'sha256': sha(data)}
def run(args, cwd=ROOT):
    result = subprocess.run(list(map(str,args)), cwd=cwd, env=ENV, capture_output=True)
    require(result.returncode == 0, result.stderr.decode()+result.stdout.decode())
    return result.stdout
def task(*args): return run(['cargo','run','--release','--locked','-p','xtask','--','conformance',*args])

def archive():
    commit = run(['git','rev-parse',HEAD]).decode().strip()
    paths = [LEDGER,REGISTRY,POLICY,'tests/conformance/coverage/inventory.json',
             'tests/conformance/coverage/requirements.json','Cargo.toml','Cargo.lock',
             'tests/package-consumer/Cargo.lock',SOURCE,'crates/excalidraw-render/src/render/asset/jpeg.rs',
             'xtask/src/conformance_pdf_bridge.rs',*REUSED]
    for path in paths: put(P/'before'/path,run(['git','show',f'{commit}:{path}']))
    require(sha((ROOT/REFERENCE).read_bytes()) == REFERENCE_HASH,'frozen ICC reference changed')
    write(P/'basis.json',{'approval':None,'base_commit':commit,'reference':desc(ROOT/REFERENCE),
          'reused_helpers':[desc(P/'before'/p) for p in REUSED],
          'old_ledger':desc(P/'before'/LEDGER),'old_registry':desc(P/'before'/REGISTRY),
          'scope':'JPEG ICC precision/model correction only; no dependency/spec/reference change'})
    print('Archived immutable b01f2db basis and authenticated existing collectors.')

def collector():
    basis = read(P/'basis.json')
    path = REUSED[0]
    old = P/'before'/path
    require(desc(old) == basis['reused_helpers'][0], 'archived helper changed')
    require((ROOT/path).read_bytes() == old.read_bytes(), 'approved original collector changed')
    m = types.ModuleType('jpeg188_authenticated_existing_collector'); m.__file__ = str(ROOT/path)
    exec(compile(old.read_bytes(),m.__file__,'exec',optimize=0),m.__dict__)
    # Parameterize output directory only; do not rewrite original helper bytes.
    m.PACKET = P
    return m

def check_graph():
    for path in ['Cargo.toml','Cargo.lock','tests/package-consumer/Cargo.lock']:
        require((ROOT/path).read_bytes() == (P/'before'/path).read_bytes(),'dependency change out of scope: '+path)
    require(sha((ROOT/REFERENCE).read_bytes()) == REFERENCE_HASH, 'reference changed')

def old_capture():
    basis = read(P/'basis.json'); c = collector()
    require(run(['git','rev-parse','HEAD'],OLD).decode().strip() == basis['base_commit'],'wrong OLD commit')
    require(not run(['git','status','--porcelain'],OLD),'OLD tracked/untracked source not clean')
    sources = c.tracked_state(OLD); write(P/'old/producer-files.json',sources)
    require(c.package(OLD,'old'),'OLD packaging failed')
    (P/'old').mkdir(exist_ok=True)
    c.capture(OLD,'old')
    require(c.tracked_state(OLD) == sources,'OLD sources changed')
    old = read(P/'old/snapshots/snapshots.json')['runs'][0]['results']
    prior_root = ROOT/'tests/conformance/proposals/jpeg187-requalification-v1/final-current/snapshots'
    previous = read(prior_root/'snapshots.json')['runs'][0]['results']
    require([r['key'] for r in old] == [r['key'] for r in previous], 'prior key identity changed')
    write(P/'old/prior-capture-comparison.json',{'approval':None,'fresh_old_source_commit':basis['base_commit'],
          'previous_capture':desc(prior_root/'snapshots.json'),'fresh_capture':desc(P/'old/snapshots/snapshots.json'),
          'identical_records':sum(a==b for a,b in zip(old,previous)),'total':168,
          'note':'Fresh OLD producer captured; prior bytes not relabeled as b01f2db evidence.'})

def validate_selectors(mapping, source):
    require(set(mapping) == GAPS, 'exact 16 named ICC additions required')
    for identity, selector in mapping.items():
        require(isinstance(selector,str) and re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*',selector),'invalid selector')
        require(not any(word in selector for word in ['unqualified','diagnostic','failure']), 'new owners need a qualified production test selector')
        require(len(re.findall(r'#\[test\]\s*(?:pub\s+)?fn\s+'+re.escape(selector)+r'\s*\(',source)) == 1,'selector not an exact unconditional test: '+identity)

def freeze(authorization):
    a = read(Path(authorization))
    require(a.get('final_current_capture_authorized') is True and bool(a.get('parent_freeze_message')),'explicit parent freeze required')
    check_graph(); validate_selectors(a['new_owner_selectors'],(ROOT/SOURCE).read_text())
    c = collector(); files = c.tracked_state(ROOT)
    for p in P.rglob('*'):
        if p.is_file() and p.name not in ['README.md','file-inventory.json'] and '__pycache__' not in p.parts:
            files[str(p.relative_to(ROOT))] = sha(p.read_bytes())
    write(P/'final-freeze.json',{'approval':None,'final_current_capture_authorized':True,
          'parent_freeze_message':a['parent_freeze_message'],'producer_files':files,'new_owner_selectors':a['new_owner_selectors']})
    check_freeze()

def check_freeze(applied=False):
    frozen = read(P/'final-freeze.json')
    require(frozen['final_current_capture_authorized'] is True,'current capture not authorized')
    for path,expected in frozen['producer_files'].items():
        if applied and path in [LEDGER,REGISTRY]: continue
        require(sha((ROOT/path).read_bytes()) == expected,'frozen source changed: '+path)
    require(set(collector().tracked_state(ROOT)) <= set(frozen['producer_files']),'freeze omitted tracked source')
    check_graph(); validate_selectors(frozen['new_owner_selectors'],(ROOT/SOURCE).read_text())
    return frozen

def stage(destination):
    frozen = check_freeze(); root = Path(destination).resolve()
    require(root.parent == Path('/tmp/opencode') and not root.exists(),'fresh external staging root required')
    run(['git','worktree','add','--detach',root,read(P/'basis.json')['base_commit']])
    for path,expected in frozen['producer_files'].items():
        data=(ROOT/path).read_bytes(); require(sha(data)==expected,'staging race')
        target=root/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
    check_freeze()

def final_capture(destination):
    frozen = check_freeze(); root = Path(destination).resolve()
    require(root != ROOT and root != OLD and root.parent == Path('/tmp/opencode'),'isolated current stage required')
    for path,expected in frozen['producer_files'].items(): require(sha((root/path).read_bytes())==expected,'stage differs: '+path)
    c=collector();require(c.package(root,'final-current'),'current packaging failed')
    (P/'final-current').mkdir(exist_ok=False)
    c.capture(root,'final-current');check_freeze()

def representative():
    check_freeze()
    for name,args in [('bridge-v4',['collect-pdf-colorspace-bridge']),('revise-v10',['candidate-revise',V10])]:
        put(P/'checks'/(name+'.json'),task(*args))
    for directory in ['assets','viewer-evidence']:(ROOT/V10/directory).mkdir()
    prefix=['cargo','run','--release','--locked','-p','xtask','--','conformance']
    steps=[('native',prefix+['candidate-native-capture',V10]),('assets',['node','tools/reference-oracle/scripts/stage4-viewer-assets.mjs',V10]),
           ('derive',prefix+['candidate-prepare-assets',V10]),('font-audit',['node','tools/reference-oracle/scripts/stage4-font-audit-qualification.mjs',V10,'collect']),
           ('viewers',['node','tools/reference-oracle/scripts/stage4-candidate-views.mjs','collect',V10]),('exceptions',prefix+['candidate-propose-exceptions',V10]),
           ('inspect',prefix+['candidate-inspect',V10]),('materialize',prefix+['candidate-materialize',V10]),('subject',prefix+['candidate-approval-subject',V10])]
    for name,args in steps:put(P/'checks'/('v10-'+name+'.json'),run(args));check_freeze()

def expected_transaction(before,registry,source_hash,selectors,candidate,subject,manifest_hash):
    require(before['approval'] is None and len(before['assignments']) == 1992,'wrong before ledger')
    rows=[r for r in before['assignments'] if r['id'].startswith(PREFIX)]
    reference=read(ROOT/REFERENCE)
    require(len(rows)==80 and {r['id'] for r in rows} == {PREFIX+r['cell'] for r in reference['cases']},'80 exact ICC criteria required')
    require({r['id'] for r in rows if r['disposition']['status']=='gap'} == GAPS and len(selectors)==16 and set(selectors)==GAPS,'exact #188 gaps required')
    require(sum(r['disposition']['status']=='owned' for r in rows)==64,'64 existing ICC owners required')
    require(all(re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*',s) and not any(word in s for word in ['unqualified','diagnostic','failure']) for s in selectors.values()),'new owner selector is not qualified')
    require(re.fullmatch(r'[0-9a-f]{64}',source_hash) is not None and re.fullmatch(r'[0-9a-f]{64}',manifest_hash) is not None and re.fullmatch(r'sha256:[0-9a-f]{64}',subject) is not None,'invalid migration digest')
    new=copy.deepcopy(before)
    for row in new['assignments']:
        if not row['id'].startswith(PREFIX):continue
        d=row['disposition']
        if d['status']=='owned':
            o=d['owner'];require(o['kind']=='test' and o['source']['path']==SOURCE and o['binary']=='fast','unexpected old owner source/kind')
            require(o['command']==['cargo','test','--locked','--release','-p','excalidraw-render','--test','fast','jpeg_icc_contract::'+o['function'],'--','--exact'],'old command drift')
            o['source']['sha256']=source_hash
        else:
            require(d['status']=='gap' and d['issue']==188,'only #188 gaps may be assigned')
            fn=selectors[row['id']]
            row['disposition']={'status':'owned','owner':{'kind':'test','source':{'path':SOURCE,'sha256':source_hash},'binary':'fast','function':fn,
                'command':['cargo','test','--locked','--release','-p','excalidraw-render','--test','fast','jpeg_icc_contract::'+fn,'--','--exact']},
                'rationale':'User-reviewed #188 JPEG ICC precision/model correction: frozen independent 160-vector matrix, exact reconstructed APP2, report/source/extent/alpha/replay contracts and canonical qualified sample colors. See tests/conformance/proposals/jpeg188-requalification-v1/; no whole-PDF/ICC-wide or resource closure implied.'}
    out=copy.deepcopy(registry)
    require(len(out['cases'])==1 and out['cases'][0]['id']=='representative.stage4-frames-text-assets.v9','wrong prior representative')
    r=out['cases'][0];require(candidate['case_id']=='representative.stage4-frames-text-assets.v10','wrong successor')
    exceptions=copy.deepcopy(r['exceptions'])
    for e in exceptions:e['case_id']=candidate['case_id']
    require(candidate['exception_proposal']['exceptions']==exceptions,'representative scopes changed')
    r.update(id=candidate['case_id'],directory=V10,status='jpeg188-icc-release-successor',exceptions=exceptions,
             execution_keys=[candidate['case_id']+':default:'+f for f in ['svg','png','pdf']],
             consumer=['cargo','run','--release','--locked','-p','xtask','--','conformance','candidate-verify',V10])
    selected=next(r for r in new['assignments'] if r['id']=='interaction:frames-text-assets')
    require(selected['disposition']['owner']['kind']=='representative' and selected['disposition']['owner']['binding']['case']=='representative.stage4-frames-text-assets.v9','wrong prior owner')
    selected['disposition']['owner']['binding']={'case':candidate['case_id'],'registry':{'path':REGISTRY,'sha256':sha(pretty(out))},
        'manifest':{'path':V10+'/manifest.json','sha256':manifest_hash},'approval_subject_sha256':subject,'command':r['consumer']}
    require({s:sum(r['disposition']['status']==s for r in new['assignments']) for s in ['owned','gap','excluded']}=={'owned':1912,'gap':77,'excluded':3},'wrong final counts')
    return new,out

def migration():
    frozen=check_freeze();before=read(P/'before'/LEDGER);registry=read(P/'before'/REGISTRY)
    require((ROOT/LEDGER).read_bytes()==(P/'before'/LEDGER).read_bytes() and (ROOT/REGISTRY).read_bytes()==(P/'before'/REGISTRY).read_bytes(),'pending live metadata changed')
    candidate=read(ROOT/V10/'manifest.json');require(candidate['approval'] is None,'candidate already approved')
    subject=task('candidate-approval-subject',V10).decode().strip()
    catalog=json.loads(task('ownership-catalog'));require(sha(canonical(catalog))==before['catalog_sha256'],'catalog changed')
    new,out=expected_transaction(before,registry,sha((ROOT/SOURCE).read_bytes()),frozen['new_owner_selectors'],candidate,subject,sha((ROOT/V10/'manifest.json').read_bytes()))
    write(P/'pending/ownership.template.json',new);write(P/'pending/registry.json',out)
    write(P/'migration-plan.json',{'approval':None,'new_owner_ids':sorted(GAPS),
        'refresh_owner_ids':[r['id'] for r in before['assignments'] if r['id'].startswith(PREFIX) and r['disposition']['status']=='owned'],
        'new_owner_selectors':frozen['new_owner_selectors'],'source':desc(ROOT/SOURCE),'representative_subject_sha256':subject,
        'catalog_sha256':before['catalog_sha256'],'counts':{'assignments':1992,'owned':1912,'excluded':3,'assigned_or_excluded':1915,'gaps':77},
        'deferred_parameter':'Approved V10 manifest file hash only; approval/registry/ledger transaction not applied.'})
    verify_migration(False)

def verify_migration(applied):
    plan=read(P/'migration-plan.json');before=read(P/'before'/LEDGER);registry=read(P/'before'/REGISTRY)
    require(desc(ROOT/SOURCE)==plan['source'],'stale test source')
    validate_selectors(plan['new_owner_selectors'],(ROOT/SOURCE).read_text())
    require(plan['new_owner_selectors']==read(P/'final-freeze.json')['new_owner_selectors'],'selector map differs from parent freeze')
    candidate=read(ROOT/V10/'manifest.json')
    new,out=expected_transaction(before,registry,plan['source']['sha256'],plan['new_owner_selectors'],candidate,plan['representative_subject_sha256'],sha((ROOT/V10/'manifest.json').read_bytes()))
    require(plan['new_owner_ids']==sorted(GAPS),'new scope drift')
    require(plan['refresh_owner_ids']==[r['id'] for r in before['assignments'] if r['id'].startswith(PREFIX) and r['disposition']['status']=='owned'],'refresh scope drift')
    for path in [POLICY,'tests/conformance/coverage/inventory.json','tests/conformance/coverage/requirements.json']:
        require((ROOT/path).read_bytes()==(P/'before'/path).read_bytes(),'criteria/input keys changed')
    require(read(P/'pending/registry.json')==out,'registry proposal differs')
    if applied:
        require(candidate['approval'] is not None,'missing candidate approval')
        require(read(ROOT/LEDGER)==new and read(ROOT/REGISTRY)==out,'applied transaction differs')
    else:
        require(candidate['approval'] is None,'pending candidate approved')
        require(read(P/'pending/ownership.template.json')==new,'ledger template differs')
        require(read(ROOT/LEDGER)==before and read(ROOT/REGISTRY)==registry,'pending mutation')

def protected():
    check_freeze();exe=ROOT/'target/release/excalidraw-render';build=read(ROOT/V10/'native-capture.json')['build']
    require(sha(exe.read_bytes())==build['executable_sha256'],'wrong current release executable')
    old=ROOT/'tests/conformance/candidates/stage4-pdf-backend-requalification-v2';destination=P/'protected-final'
    for i in range(2):
        r=subprocess.run([str(exe),'-','--output','-','--format','pdf','--report-format','json','--pdf-raster-fallback','forbid'],
            input=(ROOT/'tests/conformance/pdf-backend-qualification/project-operations.excalidraw').read_bytes(),cwd=ROOT,env=ENV,capture_output=True)
        require(r.returncode==0 and r.stdout==(old/'candidate.pdf').read_bytes() and r.stderr==(old/'candidate-report.json').read_bytes(),'protected artifact/report changed; review required')
        put(destination/f'run-{i}.pdf',r.stdout);put(destination/f'run-{i}-report.json',r.stderr)
    run(['bash','tools/pdf-harness/run-structure.sh','inspect',destination/'run-0.pdf',ROOT/'tests/conformance/pdf-backend-qualification/project-operations-structure.json','pass',destination/'structure.json'])
    for viewer in ['pdfium','pdfjs']:
        for i in range(2):
            path=destination/f'{viewer}-{i}.rgba'
            put(destination/f'{viewer}-{i}-runtime.json',run(['bash',f'tools/pdf-harness/run-{viewer}.sh','render',destination/'run-0.pdf',path,'520','200']))
            require(path.read_bytes()==(old/f'candidate-{viewer}.rgba').read_bytes(),'protected viewer changed')
    for fmt in ['svg','png','pdf']:
        for name in [f'native.{fmt}',f'native-{fmt}-report.json']:
            require((ROOT/V10/name).read_bytes()==(ROOT/'tests/conformance/candidates/stage4-frames-text-assets-v9'/name).read_bytes(),'representative output/report changed')
    write(P/'checks/protected-equality.json',{'approval':None,'native_processes':2,'viewer_runs_each':2,'artifact_report_and_viewer_equality':True,'representative_equal_v9':True,'build':build})
    check_freeze()

def unique(pairs):
    obj={}
    for key,value in pairs:require(key not in obj,'duplicate decision field');obj[key]=value
    return obj
def affirmative(body,subject,rep):
    require(isinstance(body,str),'decision text required')
    match=re.fullmatch(r'\s*```json\r?\n([^`]+)\r?\n```\s*',body)
    require(match is not None,'one standalone JSON decision required')
    decision=json.loads(match[1],object_pairs_hook=unique)
    require(decision=={'decision':'approve','scope':'jpeg188-icc','production_subject_sha256':subject,'representative_subject_sha256':rep},'exact affirmative jpeg188-icc decision required')
def fetch_comment(url):
    match=re.fullmatch(r'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-([1-9][0-9]*)',url)
    require(match is not None,'actual issue #85 comment required')
    return json.loads(run(['gh','api','repos/sagikazarmark/excalidraw-render.orig/issues/comments/'+match[1]]))
def authenticate(approval,comment,subject,rep,rep_approval):
    require(isinstance(approval,dict) and set(approval)=={'approved_by','approved_at','approval_url','approval_subject_sha256'},'exact approval fields required')
    require(approval['approved_by']=='sagikazarmark' and comment['user']['login']=='sagikazarmark','designated maintainer required')
    require(approval['approval_subject_sha256']==subject,'production subject mismatch')
    day=approval['approved_at'];require(isinstance(day,str) and len(day)==10 and date.fromisoformat(day).isoformat()==day,'invalid date')
    match=re.fullmatch(r'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-([1-9][0-9]*)',approval['approval_url'])
    require(match is not None and comment['id']==int(match[1]) and comment['html_url']==approval['approval_url'] and comment['created_at'][:10]==day,'comment identity/date/URL mismatch')
    affirmative(comment['body'],subject,rep)
    remote=fetch_comment(approval['approval_url']);require(remote['user']['login']=='sagikazarmark','genuine comment author mismatch');affirmative(remote['body'],subject,rep)
    stable=lambda x:{k:x[k] for k in ['id','html_url','created_at','body']}|{'login':x['user']['login']}
    require(stable(remote)==stable(comment),'saved/genuine comment differs')
    require(rep_approval==dict(approval,approval_subject_sha256=rep),'representative decision differs')

def qualification():
    review=read(P/'checks/parent-qualification.json');native=read(ROOT/V10/'native-capture.json')
    require(review['test_source']==desc(ROOT/SOURCE) and review['producer_sources']==native['producer_sources'],'qualification source set differs')
    for source in review['producer_sources']:require(desc(ROOT/source['path'])==source,'stale qualification producer')
    require(review['icc_reference_sha256']==REFERENCE_HASH and review['icc_vectors']==160 and review['icc_gates_passed'] is True,'exact ICC qualification missing')
    require(review['jpeg187_vectors']==76 and review['jpeg187_canonical_pass_vectors']==64 and review['jpeg187_known_failure_vectors']==12 and review['jpeg187_exact_failure_pins']==24,'#187 limitations changed')
    require(review['checks'] and all(r['passed'] is True for r in review['checks']),'missing/failing focused checks')
    for row in review['checks']:require(desc(ROOT/row['log']['path'])==row['log'],'qualification log changed')
    return review

def readiness():
    require(not (P/'final-freeze.json').exists(),'pre-freeze readiness only')
    result=subprocess.run([sys.executable,str(P/'test_workflow.py')],cwd=ROOT,env=dict(ENV,PYTHONDONTWRITEBYTECODE='1'),capture_output=True)
    put(P/'checks/readiness-tests.stdout',result.stdout);put(P/'checks/readiness-tests.stderr',result.stderr)
    require(result.returncode==0,result.stderr.decode())
    check_graph()
    before=(P/'before/xtask/src/conformance_pdf_bridge.rs').read_bytes()
    require((ROOT/'xtask/src/conformance_pdf_bridge.rs').read_bytes()==before.replace(b'stage4-pdf-colorspace-bridge-v3',b'stage4-pdf-colorspace-bridge-v4'),'bridge changed beyond directory version')
    for path in [LEDGER,REGISTRY,POLICY,'tests/conformance/coverage/inventory.json','tests/conformance/coverage/requirements.json']:
        require((ROOT/path).read_bytes()==(P/'before'/path).read_bytes(),'live metadata changed: '+path)
    write(P/'readiness.json',{'approval':None,'status':'awaiting-parent-production-tests-and-explicit-freeze','current_capture_started':False,
        'old_capture':desc(P/'old/snapshots/snapshots.json'),'old_168_records_equal_previous':read(P/'old/prior-capture-comparison.json')['identical_records'],
        'test_result':desc(P/'checks/readiness-tests.stderr'),'tests_passed':16,'bridge_constant_only':'v3-to-v4',
        'pending_counts':{'assignments':1992,'owned':1912,'excluded':3,'assigned_or_excluded':1915,'gaps':77},
        'needed_from_parent':'final qualified selector mapping for exact 16 ICC additions, final source-bound checks and explicit freeze'})
    files=[desc(p) for p in sorted(P.rglob('*')) if p.is_file() and p.name!='file-inventory.json' and '__pycache__' not in p.parts]
    write(P/'file-inventory.json',{'approval':None,'files':files})
    print('Ready: fresh OLD 168 capture, 16 focused checker tests, no current capture or live metadata changes.')

def verify(applied=False):
    manifest=read(P/'manifest.json');bundle=read(P/'approval-subject.json')
    require(manifest['approval'] is None and bundle=={'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in manifest.items() if k!='approval'},'files':manifest['files']},'packet envelope changed')
    subject='sha256:'+sha(canonical(bundle));rep=manifest['representative_subject_sha256']
    if applied:
        authenticate_application(subject,rep)
    for d in manifest['files']:require(desc(ROOT/d['path'])==d,'stale evidence '+d['path'])
    check_freeze(applied);qualification();verify_migration(applied)
    c=collector();c.verify_snapshots(P/'old/snapshots');c.verify_snapshots(P/'final-current/snapshots')
    require(sha(canonical(json.loads(task('ownership-catalog'))))==read(P/'migration-plan.json')['catalog_sha256'],'catalog changed')
    require(task('candidate-approval-subject',V10).decode().strip()==rep,'representative subject changed')
    print(subject+(' authenticated applied state' if applied else ' pending; no live migration'))

def seal():
    check_freeze();review=qualification();verify_migration(False)
    require(read(P/'checks/protected-equality.json')['artifact_report_and_viewer_equality'] is True,'protected equality missing')
    c=collector();c.verify_snapshots(P/'old/snapshots');c.verify_snapshots(P/'final-current/snapshots')
    old=read(P/'old/snapshots/snapshots.json')['runs'][0]['results'];new=read(P/'final-current/snapshots/snapshots.json')['runs'][0]['results']
    changes=[]
    for a,b in zip(old,new):
        require(a['key']==b['key'],'full key changed')
        if a!=b:changes.append({'key':a['key'],'before':a['samples'][0],'after':b['samples'][0]})
    require(review['diagnosed_inventory_changes']==changes,'unreviewed inventory output differences')
    write(P/'inventory-comparison.json',{'approval':None,'keys':168,'unchanged':168-len(changes),'changes':changes})
    files=[desc(p) for base in [P,ROOT/V10,ROOT/BRIDGE] for p in sorted(base.rglob('*')) if p.is_file() and p.name not in ['README.md','file-inventory.json'] and '__pycache__' not in p.parts and p!=ROOT/V10/'manifest.json']
    manifest={'schema':'excalidraw-render/jpeg188-icc-requalification/v1','approval':None,'status':'pending-explicit-user-approval','originating_issue':'https://github.com/sagikazarmark/excalidraw-render.orig/issues/188',
              'representative_subject_sha256':read(P/'migration-plan.json')['representative_subject_sha256'],'files':files,
              'approval_scope':'jpeg188-icc','new_owners':16,'source_refreshes':64,'baseline_refresh':False,'global_approval':False}
    write(P/'manifest.json',manifest);write(P/'approval-subject.json',{'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in manifest.items() if k!='approval'},'files':files});verify(False)

def authenticate_application(subject,rep):
    a=ROOT/'tests/conformance/coverage/jpeg188-application-v1';status=read(a/'status.json')
    require(status.get('state')=='applied' and status.get('production_subject_sha256')==subject and status.get('representative_subject_sha256')==rep,'not successfully applied')
    authenticate(read(a/'approval.json'),read(a/'comment.json'),subject,rep,read(ROOT/V10/'manifest.json')['approval'])

if __name__=='__main__':
    args=sys.argv[1:]
    if args==['archive']:archive()
    elif args==['old']:old_capture()
    elif len(args)==2 and args[0]=='freeze':freeze(args[1])
    elif len(args)==2 and args[0]=='stage':stage(args[1])
    elif len(args)==2 and args[0]=='final':final_capture(args[1])
    elif args==['representative']:representative()
    elif args==['protected']:protected()
    elif args==['migration']:migration()
    elif args==['seal']:seal()
    elif args==['verify']:verify()
    elif args==['verify-applied']:verify(True)
    elif args==['readiness']:readiness()
    else:raise SystemExit('archive|old|freeze AUTH|stage ROOT|final ROOT|representative|protected|migration|seal|verify|verify-applied')
