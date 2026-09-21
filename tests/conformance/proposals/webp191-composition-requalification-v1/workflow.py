#!/usr/bin/env python3
"""#191 first-frame composition only. Current collection requires explicit freeze."""
import sys
if not __debug__ or sys.flags.optimize:raise SystemExit('Optimized Python forbidden; no workflow started.')
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
HEAD='dd3389a'
OLD=Path('/tmp/opencode/webp191-old-dd3389a')
V13='tests/conformance/candidates/stage4-frames-text-assets-v13'
PRIOR='tests/conformance/candidates/stage4-frames-text-assets-v12'
BRIDGE='tests/conformance/candidates/stage4-pdf-colorspace-bridge-v7'
SOURCE='crates/excalidraw-render/tests/fast/webp_sample_contract.rs'
PRODUCER='crates/excalidraw-render/src/render/asset/webp.rs'
LEDGER='tests/conformance/coverage/ownership.json'
REGISTRY='tests/conformance/candidates/registry.json'
POLICY='tests/conformance/coverage/ownership-policy.json'
PREFIX='codec:webp:sample:'
REFERENCE='tests/oracle/webp-sample/reference.json'
INPUTS='tests/oracle/webp-sample/inputs.json'
NATIVE_OLD='tests/oracle/webp-sample/native-defect-measurements.json'
NATIVE_NEW='tests/oracle/webp-sample/composition-defect-measurements.json'
HASHES={REFERENCE:'b325e8a2d8ad0a1fbd72463a9e2272ef43a1151c2ca040aa5fc46150335f49b6',
        INPUTS:'21ce05bb34249f370de8dab10026972e58dd59d40a6dfbff4d7d7df1e3b857c4',
        NATIVE_OLD:'d31850b8a23ed04039de976437a06b9922bbc963ee83f3b6269833d5cc24ba76',
        NATIVE_NEW:'6c9f48c8903604c7112085d64fd53325d30975f3552c01a3ace5e50dae5de668'}
CORRECTED={'lossless-opaque-anim-full-blend-d0-clear','lossless-opaque-anim-sub-blend-d1-opaque',
 'lossless-opaque-anim-sub-replace-d0-clear','lossless-opaque-anim-sub-replace-d1-opaque',
 'lossless-binary-anim-full-blend-d0-clear','lossless-binary-anim-sub-blend-d1-opaque',
 'lossy-opaque-anim-sub-blend-d1-opaque','lossy-opaque-anim-sub-replace-d0-clear',
 'lossy-opaque-anim-sub-replace-d1-opaque','lossy-binary-anim-full-blend-d0-clear','lossy-binary-anim-sub-blend-d1-opaque'}
RESIDUAL={f'{codec}-partial-{config}' for codec in ['lossless','lossy'] for config in ['static-full','anim-full-blend-d0-clear','anim-full-replace-d1-opaque','anim-sub-blend-d1-opaque','anim-sub-replace-d0-clear','anim-sub-replace-d1-opaque']}
GAPS={PREFIX+c for c in CORRECTED}
NEW_SELECTOR='corrected_composition_cells_match_reference'
HELPERS={'capture':'tests/conformance/proposals/jpeg187-requalification-v1/collect.py',
         'qualification':'tests/conformance/proposals/jpeg188-requalification-v1/workflow.py',
         'representative':'tests/conformance/proposals/jpeg189-requalification-v1/workflow.py',
         'receipts':'tests/conformance/proposals/jpeg188-requalification-v2/verify.py'}
ENV=dict(os.environ,PATH='/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:'+os.environ['PATH'],CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_PROFILE_RELEASE_DEBUG='0',PYTHONDONTWRITEBYTECODE='1')
def require(ok,message):
    if not ok:raise ValueError(message)
def sha(data):return hashlib.sha256(data).hexdigest()
def canonical(value):return json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()
def pretty(value):return json.dumps(value,indent=2,ensure_ascii=False).encode()+b'\n'
def read(path):
    try:return json.loads(path.read_bytes())
    except (OSError,ValueError) as error:raise ValueError(f'missing/invalid {path}: {error}') from error
def put(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as output:output.write(data)
def write(path,value):put(path,pretty(value))
def desc(path):
    data=path.read_bytes();return {'path':str(path.relative_to(ROOT)),'byte_length':len(data),'sha256':sha(data)}
def check(d):
    require(desc(ROOT/d['path'])==d,'stale descriptor '+d['path']);return (ROOT/d['path']).read_bytes()
def run(args,cwd=ROOT):
    result=subprocess.run(list(map(str,args)),cwd=cwd,env=ENV,capture_output=True)
    require(result.returncode==0,result.stderr.decode()+result.stdout.decode());return result.stdout
def task(*args):return run(['cargo','run','--release','--locked','-p','xtask','--','conformance',*args])

def archive():
    commit=run(['git','rev-parse',HEAD]).decode().strip()
    paths=[LEDGER,REGISTRY,POLICY,'tests/conformance/coverage/inventory.json','tests/conformance/coverage/requirements.json','Cargo.toml','Cargo.lock','tests/package-consumer/Cargo.lock',SOURCE,PRODUCER,
           'xtask/src/conformance_pdf_bridge.rs','tests/package-consumer/examples/native_snapshots.rs',*HELPERS.values()]
    for path in paths:put(P/'before'/path,run(['git','show',f'{commit}:{path}']))
    for path,digest in HASHES.items():require(sha((ROOT/path).read_bytes())==digest,'reference/measurement changed '+path)
    write(P/'basis.json',{'approval':None,'base_commit':commit,'helper_sources':{k:desc(P/'before'/p) for k,p in HELPERS.items()},
        'data_sources':[desc(ROOT/p) for p in HASHES],'new_owner_ids':sorted(GAPS),'residual_cell_ids':sorted(PREFIX+c for c in RESIDUAL),
        'scope':'first ANMF standalone decode and clear RGBA placement only; static/partial-alpha/PDF sample policy unchanged'})

def helper(name):
    data=check(read(P/'basis.json')['helper_sources'][name]);require(data==(ROOT/HELPERS[name]).read_bytes(),'approved helper changed')
    m=types.ModuleType('authenticated_webp191_'+name);m.__file__=str(ROOT/HELPERS[name])
    exec(compile(data,m.__file__,'exec',optimize=0),m.__dict__);return m
def collector():
    c=helper('capture');c.ROOT=ROOT;c.PACKET=P;return c
def check_graph():
    for path in ['Cargo.toml','Cargo.lock','tests/package-consumer/Cargo.lock']:
        require((ROOT/path).read_bytes()==(P/'before'/path).read_bytes(),'dependency change out of scope '+path)
    for path,digest in HASHES.items():require(sha((ROOT/path).read_bytes())==digest,'immutable reference/pin changed '+path)

def measurements(before=None,after=None,reference=None):
    old=read(ROOT/NATIVE_OLD) if before is None else before
    new=read(ROOT/NATIVE_NEW) if after is None else after
    ref=read(ROOT/REFERENCE) if reference is None else reference
    require(new['previous_measurements_sha256']==HASHES[NATIVE_OLD] and new['reference_sha256']==HASHES[REFERENCE] and new['inputs_sha256']==HASHES[INPUTS],'measurement chain differs')
    require(set(new['corrected_cells'])==CORRECTED and len(new['corrected_cells'])==11 and set(new['residual_cells'])==RESIDUAL and len(new['residual_cells'])==12,'corrected/residual cell split changed')
    require((new['qualified_sample_cells'],new['unqualified_sample_cells'],new['unqualified_vectors'])==(24,12,24),'qualification counts changed')
    original=set(new['original_qualified_cells']);require(len(original)==13 and not original&(CORRECTED|RESIDUAL),'old pass set overlaps')
    require(original|CORRECTED|RESIDUAL=={r['cell'] for r in ref['cases']},'closed 36 sample cells differ')
    a={r['id']:r for r in old['cases']};b={r['id']:r for r in new['cases']};refs={r['id']:r for r in ref['cases']}
    require(len(a)==len(b)==46 and a.keys()==b.keys(),'old/new measured vector set differs')
    require(set(new['original_qualified_cells'])=={r['cell'] for r in ref['cases']} - {r['cell'] for r in old['cases']},'original pass partition drift')
    changed=[];format_changes=[];residual_changes=[];passes=[0,0,0];failures=0
    for identity,newrow in b.items():
        oldrow=a[identity];reference_row=refs[identity]
        require(newrow['cell']==oldrow['cell']==reference_row['cell'] and newrow['source_webp_sha256']==oldrow['source_webp_sha256'],'source/cell changed')
        corrected=newrow['cell'] in CORRECTED;residual=newrow['cell'] in RESIDUAL
        require(corrected!=residual,'measured vector partition invalid')
        affected=corrected or (reference_row['alpha']=='partial' and reference_row['config']['blend'] is True)
        require((newrow!=oldrow)==affected,'only reviewed first-frame composition may change: '+identity)
        if affected:
            changed.append(identity)
            if residual:residual_changes.append(identity)
        require(len(newrow['formats'])==len(oldrow['formats'])==3,'missing format record')
        for i,(n,o,fmt) in enumerate(zip(newrow['formats'],oldrow['formats'],['svg','png','pdf'])):
            require(n['format']==o['format']==fmt,'format identity differs')
            for key in ['dimensions','diagnostic','diagnostic_sha256']:require(n[key]==o[key],'report/extent drift')
            require(n['requirement_passed'] is (i==0 or corrected),'residual cannot be relabeled passing')
            passes[i]+=int(n['requirement_passed'])
            if i==0:require(n==o,'SVG must remain exact');continue
            require((n!=o)==affected,'raster change scope mismatch')
            if n!=o:format_changes.append({'id':identity,'format':fmt})
            require(n['alpha']['expected_sha256']==o['alpha']['expected_sha256'] and n['alpha']['passed'] is True and n['alpha']['mismatched_pixels']==0 and n['alpha']['nonzero_invisible_pixels']==0,'alpha invariant changed')
            require(len(n['views'])==len(o['views']),'view inventory changed')
            for nv,ov in zip(n['views'],o['views']):
                for key in ['name','requirement','expected_rgba_sha256']:require(nv[key]==ov[key],'reference view/requirement changed')
                passed=corrected or nv['name']=='external-lossless-straight'
                require(nv['requirement_passed'] is passed and nv['canonical']['passed'] is passed,'straight/residual comparison changed')
                if not passed:
                    failures+=1
                    require(nv['canonical']['ssim']=={'kind':'small-artifact-mismatch'} and nv['canonical']['changed_pixels']==0 and nv['expected_rgba_sha256']!=nv['actual_rgba_sha256'],'residual exact small-artifact failure changed')
    require((len(changed),len(format_changes),len(residual_changes),passes,failures)==(30,60,8,[46,22,22],72),'exact transition counts differ')
    return {'changed_vectors':sorted(changed),'changed_raster_records':format_changes,'improved_but_failing_partial_vectors':sorted(residual_changes),
        'full_corpus_passes':{'svg':72,'png':48,'pdf':48},'residual_cells':12,'residual_vectors':24,'exact_failed_views':72,'tolerance_or_policy_change':False}

def old_capture():
    c=collector();basis=read(P/'basis.json')
    require(run(['git','rev-parse','HEAD'],OLD).decode().strip()==basis['base_commit'],'wrong OLD HEAD')
    require(not run(['git','status','--porcelain'],OLD),'OLD source not clean')
    before=c.tracked_state(OLD);write(P/'old/producer-files.json',before)
    require(c.package(OLD,'old'),'OLD packaging failed');c.capture(OLD,'old')
    require(c.tracked_state(OLD)==before,'OLD source changed')
    prior=ROOT/'tests/conformance/proposals/jpeg190-mpf-requalification-v1/final-current/snapshots/snapshots.json'
    a=read(P/'old/snapshots/snapshots.json')['runs'][0]['results'];b=read(prior)['runs'][0]['results']
    require(len(a)==len(b)==168 and [r['key'] for r in a]==[r['key'] for r in b],'prior inventory keys differ')
    write(P/'old/prior-capture-comparison.json',{'approval':None,'base_commit':basis['base_commit'],'fresh':desc(P/'old/snapshots/snapshots.json'),'prior':desc(prior),'identical_records':sum(x==y for x,y in zip(a,b)),'total':168,'fresh_provenance_not_relabelled':True})

def validate_selectors(mapping,source):
    require(set(mapping)==GAPS and set(mapping.values())=={NEW_SELECTOR},'exact eleven corrected-composition owners required')
    require(len(re.findall(r'#\[test\]\s*fn\s+'+NEW_SELECTOR+r'\s*\(',source))==1,'qualified selector is not unconditional test')
    for name,wanted in [('CORRECTED_CELLS',CORRECTED),('RESIDUAL_CELLS',RESIDUAL)]:
        match=re.search(r'const '+name+r':.*?=\s*\[(.*?)\];',source,re.S);require(match is not None,'source partition missing')
        values=re.findall(r'"([^"]+)"',match[1]);require(len(values)==len(wanted) and set(values)==wanted,'source cell partition differs')

def freeze(authorization):
    a=read(Path(authorization));require(a.get('final_current_capture_authorized') is True and a.get('parent_freeze_message'),'explicit parent freeze required')
    check_graph();measurements();validate_selectors(a['new_owner_selectors'],(ROOT/SOURCE).read_text())
    files=collector().tracked_state(ROOT)
    # The new composition measurement file is not yet tracked and is outside P.
    for path in HASHES:files[path]=sha((ROOT/path).read_bytes())
    for path in P.rglob('*'):
        if path.is_file() and path.name not in ['README.md','file-inventory.json'] and '__pycache__' not in path.parts:files[str(path.relative_to(ROOT))]=sha(path.read_bytes())
    write(P/'final-freeze.json',{'approval':None,'final_current_capture_authorized':True,'parent_freeze_message':a['parent_freeze_message'],'producer_files':files,'new_owner_selectors':a['new_owner_selectors']})
    check_freeze()
def check_freeze(applied=False):
    f=read(P/'final-freeze.json');require(f['final_current_capture_authorized'] is True,'parent freeze required')
    for path,digest in f['producer_files'].items():
        if applied and path in [LEDGER,REGISTRY]:continue
        require(sha((ROOT/path).read_bytes())==digest,'frozen source changed '+path)
    require(set(collector().tracked_state(ROOT))<=set(f['producer_files']) and set(HASHES)<=set(f['producer_files']),'incomplete freeze')
    check_graph();measurements();validate_selectors(f['new_owner_selectors'],(ROOT/SOURCE).read_text());return f

def qualification_helper():
    q=helper('qualification');q.ROOT=ROOT;q.P=P;q.HEAD=HEAD;q.OLD=OLD;q.V10=V13;q.BRIDGE=BRIDGE;q.SOURCE=SOURCE
    q.REFERENCE=REFERENCE;q.REFERENCE_HASH=HASHES[REFERENCE];q.collector=collector;q.check_graph=check_graph;q.validate_selectors=validate_selectors;q.check_freeze=check_freeze
    return q
def representative():
    check_freeze();qualification_helper().representative()
def protected():
    # Authenticated previous helper has a parameterized prior artifact directory
    # only in #189; use its strict protected path with #191 identities.
    q=helper('representative');q.ROOT=ROOT;q.P=P;q.V11=V13;q.PRIOR=PRIOR;q.check_freeze=check_freeze;q.ENV=ENV
    q.protected()

def expected_transaction(before,registry,source_hash,selectors,candidate,subject,file_hash):
    require(len(before['assignments'])==2418 and before['approval'] is None,'before ledger mismatch')
    rows=[r for r in before['assignments'] if r['id'].startswith(PREFIX)];ref=read(ROOT/REFERENCE)
    cells={PREFIX+r['cell'] for r in ref['cases']}|{PREFIX+r['id'] for r in ref['controls']}
    require(len(rows)==48 and {r['id'] for r in rows}==cells,'48 exact sample/control criteria required')
    require({r['id'] for r in rows if r['disposition']['status']=='gap'}==GAPS|{PREFIX+c for c in RESIDUAL} and set(selectors)==GAPS and set(selectors.values())=={NEW_SELECTOR},'eleven additions/twelve residual gaps differ')
    require(sum(r['disposition']['status']=='owned' for r in rows)==25,'25 existing owners required')
    for h in [source_hash,file_hash]:require(re.fullmatch(r'[0-9a-f]{64}',h),'invalid digest')
    require(re.fullmatch(r'sha256:[0-9a-f]{64}',subject),'invalid subject')
    command=lambda fn:['cargo','test','--locked','--release','-p','excalidraw-render','--test','fast','webp_sample_contract::'+fn,'--','--exact']
    new=copy.deepcopy(before)
    for row in new['assignments']:
        if not row['id'].startswith(PREFIX):continue
        d=row['disposition']
        if d['status']=='owned':
            owner=d['owner'];require(owner['kind']=='test' and owner['source']['path']==SOURCE and owner['binary']=='fast' and owner['command']==command(owner['function']),'old owner selector/command mismatch')
            owner['source']['sha256']=source_hash
        elif row['id'] in GAPS:
            require(d['issue']==191,'only #191 gaps may be assigned')
            row['disposition']={'status':'owned','owner':{'kind':'test','source':{'path':SOURCE,'sha256':source_hash},'binary':'fast','function':NEW_SELECTOR,'command':command(NEW_SELECTOR)},
                'rationale':'User-reviewed #191 first-ANMF standalone decode onto clear RGBA canvas. Both fixed independent patterns pass unchanged SVG/PNG/PDF sample/composite gates and exact reconstruction/alpha/report/replay invariants. See tests/conformance/proposals/webp191-composition-requalification-v1/. Partial-alpha PNG premultiplication and synthetic PDF projection-boundary decisions remain unqualified; no tolerance or whole-viewer claim.'}
        else:require(row['id'] in {PREFIX+c for c in RESIDUAL} and d['status']=='gap' and d['issue']==191,'partial-alpha gap must remain unowned')
    out=copy.deepcopy(registry);require(len(out['cases'])==1 and out['cases'][0]['id']=='representative.stage4-frames-text-assets.v12','prior representative mismatch')
    r=out['cases'][0];require(candidate['case_id']=='representative.stage4-frames-text-assets.v13','successor mismatch')
    exceptions=copy.deepcopy(r['exceptions'])
    for e in exceptions:e['case_id']=candidate['case_id']
    require(candidate['exception_proposal']['exceptions']==exceptions,'existing representative scope changed')
    r.update(id=candidate['case_id'],directory=V13,status='webp191-composition-release-successor',exceptions=exceptions,
        execution_keys=[candidate['case_id']+':default:'+f for f in ['svg','png','pdf']],consumer=['cargo','run','--release','--locked','-p','xtask','--','conformance','candidate-verify',V13])
    owner=next(r for r in new['assignments'] if r['id']=='interaction:frames-text-assets')['disposition']['owner']
    require(owner['kind']=='representative' and owner['binding']['case']=='representative.stage4-frames-text-assets.v12','prior representative owner mismatch')
    owner['binding']={'case':candidate['case_id'],'registry':{'path':REGISTRY,'sha256':sha(pretty(out))},'manifest':{'path':V13+'/manifest.json','sha256':file_hash},'approval_subject_sha256':subject,'command':r['consumer']}
    require({s:sum(r['disposition']['status']==s for r in new['assignments']) for s in ['owned','gap','excluded']}=={'owned':2318,'gap':97,'excluded':3},'final counts differ')
    return new,out

def migration():
    f=check_freeze();before=read(P/'before'/LEDGER);registry=read(P/'before'/REGISTRY)
    require(read(ROOT/LEDGER)==before and read(ROOT/REGISTRY)==registry,'live metadata changed before approval')
    candidate=read(ROOT/V13/'manifest.json');require(candidate['approval'] is None,'candidate already approved')
    subject=task('candidate-approval-subject',V13).decode().strip();require(sha(canonical(json.loads(task('ownership-catalog'))))==before['catalog_sha256'],'catalog changed')
    new,out=expected_transaction(before,registry,sha((ROOT/SOURCE).read_bytes()),f['new_owner_selectors'],candidate,subject,sha((ROOT/V13/'manifest.json').read_bytes()))
    write(P/'pending/ownership.template.json',new);write(P/'pending/registry.json',out)
    write(P/'migration-plan.json',{'approval':None,'source':desc(ROOT/SOURCE),'new_owner_ids':sorted(GAPS),'residual_gap_ids':sorted(PREFIX+c for c in RESIDUAL),'new_owner_selectors':f['new_owner_selectors'],
        'refresh_owner_ids':[r['id'] for r in before['assignments'] if r['id'].startswith(PREFIX) and r['disposition']['status']=='owned'],
        'representative_subject_sha256':subject,'catalog_sha256':before['catalog_sha256'],'measurement_transition':measurements(),
        'counts':{'assignments':2418,'owned':2318,'excluded':3,'assigned_or_excluded':2321,'gaps':97}})
    verify_migration(False)
def verify_migration(applied):
    f=check_freeze(applied);plan=read(P/'migration-plan.json');before=read(P/'before'/LEDGER);registry=read(P/'before'/REGISTRY)
    require(plan['source']==desc(ROOT/SOURCE) and plan['new_owner_selectors']==f['new_owner_selectors'],'source/selector mismatch')
    candidate=read(ROOT/V13/'manifest.json')
    expected,out=expected_transaction(before,registry,plan['source']['sha256'],plan['new_owner_selectors'],candidate,plan['representative_subject_sha256'],sha((ROOT/V13/'manifest.json').read_bytes()))
    require(plan['new_owner_ids']==sorted(GAPS) and plan['residual_gap_ids']==sorted(PREFIX+c for c in RESIDUAL),'migration scope mismatch')
    require(plan['refresh_owner_ids']==[r['id'] for r in before['assignments'] if r['id'].startswith(PREFIX) and r['disposition']['status']=='owned'],'source refresh scope changed')
    require(plan['measurement_transition']==measurements(),'measurement transition differs')
    for path in [POLICY,'tests/conformance/coverage/inventory.json','tests/conformance/coverage/requirements.json']:require((ROOT/path).read_bytes()==(P/'before'/path).read_bytes(),'criteria/full-key change out of scope')
    require(read(P/'pending/registry.json')==out,'registry template differs')
    if applied:require(candidate['approval'] is not None and read(ROOT/LEDGER)==expected and read(ROOT/REGISTRY)==out,'applied transaction differs')
    else:
        require(candidate['approval'] is None and read(P/'pending/ownership.template.json')==expected,'pending template differs')
        require(read(ROOT/LEDGER)==before and read(ROOT/REGISTRY)==registry,'pending mutation')

def affirmative(body,subject,rep):
    require(isinstance(body,str),'decision text required')
    match=re.fullmatch(r'\s*```json\r?\n([^`]+)\r?\n```\s*',body);require(match is not None,'one standalone JSON decision required')
    def unique(pairs):
        out={}
        for k,v in pairs:require(k not in out,'duplicate decision field');out[k]=v
        return out
    require(json.loads(match[1],object_pairs_hook=unique)=={'decision':'approve','scope':'webp191-composition','production_subject_sha256':subject,'representative_subject_sha256':rep},'exact affirmative webp191-composition decision required')
def authentication_helper():
    q=helper('qualification');q.ROOT=ROOT;q.affirmative=affirmative;return q
def authenticate(a,c,s,r,ra):return authentication_helper().authenticate(a,c,s,r,ra)
def fetch_comment(url):return authentication_helper().fetch_comment(url)

def qualification():
    r=read(P/'checks/parent-qualification.json');native=read(ROOT/V13/'native-capture.json')
    require(r['test_source']==desc(ROOT/SOURCE) and r['producer_sources']==native['producer_sources'],'final qualification sources differ')
    for d in r['producer_sources']:check(d)
    require(r['composition_only'] is True and r['static_or_alpha_policy_change'] is False,'production scope expanded')
    require(r['reference_sha256']==HASHES[REFERENCE] and r['old_measurements_sha256']==HASHES[NATIVE_OLD] and r['new_measurements_sha256']==HASHES[NATIVE_NEW],'measurement/reference identity differs')
    require((r['sample_cells'],r['qualified_cells'],r['residual_partial_cells'],r['svg_vectors_pass'],r['png_vectors_pass'],r['pdf_vectors_pass'])==(36,24,12,72,48,48),'qualification count changed')
    require(r['measurement_transition']==measurements(),'exact old/new observation transition missing')
    require(r['prior_codec_regressions_passed'] is True and r['prior_known_limitations_preserved'] is True,'prior regression/known-failure evidence missing')
    require(r['checks'] and all(c['passed'] is True for c in r['checks']),'actual passing focused logs required')
    for c in r['checks']:check(c['log'])
    return r

def packet(applied=False):
    m=read(P/'manifest.json');b=read(P/'approval-subject.json')
    require(m['approval'] is None and m['approval_scope']=='webp191-composition','packet identity changed')
    require(b=={'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in m.items() if k!='approval'},'files':m['files']},'subject envelope differs')
    for d in m['files']:check(d)
    if not applied:check(m['pending_representative_manifest'])
    return 'sha256:'+sha(canonical(b))
def evidence(applied=False):
    check_freeze(applied);qualification();verify_migration(applied)
    c=collector();c.verify_snapshots(P/'old/snapshots');c.verify_snapshots(P/'final-current/snapshots')
    require(sha(canonical(json.loads(task('ownership-catalog'))))==read(P/'migration-plan.json')['catalog_sha256'],'catalog mismatch')
    require(task('candidate-approval-subject',V13).decode().strip()==read(P/'migration-plan.json')['representative_subject_sha256'],'representative subject mismatch')
def current_binding(subject):
    check_freeze(True)
    return {'production_subject_sha256':subject,'representative_subject_sha256':read(P/'migration-plan.json')['representative_subject_sha256'],
        'frozen_source_record':desc(P/'final-freeze.json'),'post_application_files':[desc(ROOT/p) for p in [V13+'/manifest.json',REGISTRY,LEDGER,POLICY,'tests/conformance/coverage/inventory.json',
        'xtask/src/main.rs','xtask/src/coverage/ownership.rs','xtask/tests/coverage.rs','xtask/tests/representative_candidate.rs',SOURCE,NATIVE_NEW,'Cargo.toml','Cargo.lock']],
        'workflow':desc(P/'workflow.py'),'receipt_entry':desc(P/'receipt_gate.py'),'authenticated_receipt_helper':read(P/'basis.json')['helper_sources']['receipts'],'test_inventory':desc(P/'target-identities.json')}
def receipt_gate():
    helper('receipts');raw=(P/'before'/HELPERS['receipts']).read_text()
    patches={'jpeg188-post-approval-receipts/v1':'webp191-post-approval-receipts/v1','jpeg188-receipt-transaction/v1':'webp191-receipt-transaction/v1',
        'verified 1992 ownership assignments: 1915 assigned/excluded, 77 explicit gaps; ledger approval pending':'verified 2418 ownership assignments: 2321 assigned/excluded, 97 explicit gaps; ledger approval pending'}
    for before,after in patches.items():
        require(raw.count(before)==(2 if before=='jpeg188-post-approval-receipts/v1' else 1),'receipt adaptation seam changed');raw=raw.replace(before,after)
    g=types.ModuleType('webp191_receipt_adapter');g.__file__=str(P/'receipt_gate.py');exec(compile(raw,g.__file__,'exec',optimize=0),g.__dict__)
    g.ROOT=ROOT;g.P=P;g.BASE=P;g.V10=V13;g.CASE='representative.stage4-frames-text-assets.v13';g.LEDGER=LEDGER;g.REGISTRY=REGISTRY;g.APP='tests/conformance/coverage/webp191-application-v1'
    g.REP=read(P/'migration-plan.json')['representative_subject_sha256'] if (P/'migration-plan.json').exists() else 'UNMATERIALIZED-V13-NO-SUBJECT'
    g.COMMANDS=[('affected-tests',g.TEST_COMMAND),('candidate',g.PREFIX+['candidate-verify',V13]),('registry',g.PREFIX+['candidate-executions']),
        ('owner',g.PREFIX+['verify-representative-owner',g.CASE]),('inventory',g.PREFIX+['verify-inventory']),('ownership',g.PREFIX+['verify-ownership'])]
    api=types.SimpleNamespace(authenticate=authenticate,fetch_comment=fetch_comment,verify_migration=verify_migration)
    g.load_original=lambda name:api if name=='workflow.py' else (_ for _ in ()).throw(ValueError('unexpected delegated helper'))
    g.packet=packet;g.evidence=evidence;g.current_binding=current_binding;return g
def seal():
    require(not (P/'manifest.json').exists(),'refuse overwrite');check_freeze();review=qualification();verify_migration(False);receipt_gate().test_inventory()
    require(read(P/'checks/protected-equality.json')['exact_protected_equality'] is True,'protected proof missing')
    a=read(P/'old/snapshots/snapshots.json')['runs'][0]['results'];b=read(P/'final-current/snapshots/snapshots.json')['runs'][0]['results']
    require(len(a)==len(b)==168 and [r['key'] for r in a]==[r['key'] for r in b],'168 full keys differ')
    changes=[{'key':x['key'],'before':x['samples'][0],'after':y['samples'][0]} for x,y in zip(a,b) if x!=y]
    require(review['diagnosed_inventory_changes']==changes,'unexplained current output changes; STOP')
    write(P/'inventory-comparison.json',{'approval':None,'keys':168,'unchanged':168-len(changes),'changes':changes})
    files=[desc(p) for base in [P,ROOT/V13,ROOT/BRIDGE] for p in sorted(base.rglob('*')) if p.is_file() and p.name not in ['README.md','file-inventory.json'] and '__pycache__' not in p.parts and p!=ROOT/V13/'manifest.json']
    files += [desc(ROOT/NATIVE_NEW)]
    m={'schema':'excalidraw-render/webp191-composition-requalification/v1','approval':None,'status':'pending-explicit-user-approval','approval_scope':'webp191-composition',
        'representative_subject_sha256':read(P/'migration-plan.json')['representative_subject_sha256'],'new_owners':11,'source_refreshes':25,'residual_partial_cells':12,
        'static_or_alpha_policy_approval':False,'baseline_refresh':False,'global_approval':False,'files':files,'pending_representative_manifest':desc(ROOT/V13/'manifest.json')}
    write(P/'manifest.json',m);write(P/'approval-subject.json',{'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in m.items() if k!='approval'},'files':files});receipt_gate().verify(False)
def readiness():
    require(not (P/'final-freeze.json').exists(),'pre-freeze readiness only')
    path='tests/conformance/proposals/jpeg188-requalification-v2/target-identities.json'
    data=run(['git','show',read(P/'basis.json')['base_commit']+':'+path]);require((ROOT/path).read_bytes()==data,'historical receipt inventory changed')
    put(P/'target-identities.json',data);require([r['count'] for r in receipt_gate().test_inventory()]==[103,5,1],'109 tests required')
    r=subprocess.run([sys.executable,str(P/'test_workflow.py')],cwd=ROOT,env=ENV,capture_output=True)
    put(P/'checks/readiness-tests.stdout',r.stdout);put(P/'checks/readiness-tests.stderr',r.stderr);require(r.returncode==0,r.stderr.decode())
    check_graph();write(P/'measurement-transition.json',{'approval':None,'old':desc(ROOT/NATIVE_OLD),'new':desc(ROOT/NATIVE_NEW),'reference':desc(ROOT/REFERENCE),'scope':measurements()})
    old=(P/'before/xtask/src/conformance_pdf_bridge.rs').read_bytes();require((ROOT/'xtask/src/conformance_pdf_bridge.rs').read_bytes()==old.replace(b'stage4-pdf-colorspace-bridge-v6',b'stage4-pdf-colorspace-bridge-v7'),'bridge changed beyond directory version')
    for path in [LEDGER,REGISTRY,POLICY,'tests/conformance/coverage/inventory.json','tests/conformance/coverage/requirements.json']:require((ROOT/path).read_bytes()==(P/'before'/path).read_bytes(),'live metadata changed')
    write(P/'readiness.json',{'approval':None,'status':'awaiting-parent-production-resource-review-tests-and-explicit-freeze','current_capture_started':False,
        'old_capture':desc(P/'old/snapshots/snapshots.json'),'old_equal_prior_records':read(P/'old/prior-capture-comparison.json')['identical_records'],
        'new_owner_ids':sorted(GAPS),'residual_gap_ids':sorted(PREFIX+c for c in RESIDUAL),'measurement_transition':desc(P/'measurement-transition.json'),
        'counts_after':{'assignments':2418,'owned':2318,'excluded':3,'assigned_or_excluded':2321,'gaps':97},'tests':desc(P/'checks/readiness-tests.stderr')})
    write(P/'file-inventory.json',{'approval':None,'files':[desc(p) for p in sorted(P.rglob('*')) if p.is_file() and p.name!='file-inventory.json' and '__pycache__' not in p.parts]})
    print('Ready: fresh OLD 168 capture; exact 11/25 migration and 30/60 measurement transition; no current capture.')

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
    else:raise SystemExit('archive|old|freeze AUTH|stage ROOT|final ROOT|representative|protected|migration|seal|readiness')
