#!/usr/bin/env python3
"""#190 MPF-only preparation; exact reviewed criterion transition, never auto-approval."""
import sys
if not __debug__ or sys.flags.optimize:
    raise SystemExit('Optimized Python forbidden; no workflow started.')
import copy
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import types

ROOT=Path(__file__).resolve().parents[4]
P=Path(__file__).resolve().parent
HEAD='80670ae'
OLD=Path('/tmp/opencode/jpeg190-old-80670ae')
V12='tests/conformance/candidates/stage4-frames-text-assets-v12'
PRIOR='tests/conformance/candidates/stage4-frames-text-assets-v11'
BRIDGE='tests/conformance/candidates/stage4-pdf-colorspace-bridge-v6'
SOURCE='crates/excalidraw-render/tests/fast/jpeg_gain_map_contract.rs'
LEDGER='tests/conformance/coverage/ownership.json'
REGISTRY='tests/conformance/candidates/registry.json'
POLICY='tests/conformance/coverage/ownership-policy.json'
PREFIX='codec:jpeg:mpf-xmp:'
REFERENCE='tests/oracle/jpeg-gain-map/reference.json'
REFERENCE_HASH='af3c97e389ef230c57bbe9a4b3406dc1ef1d6a2ad37b0e8ad2e1f4fbe6568ce4'
INDEX_SIGNALS={'bad-offset','bad-size','bad-count','missing-secondary'}
HDR_SIGNALS={'hdr-ramp','hdr-edges'}
OLD_CLAUSE='canonical direct-PNG and official-SVG-authoritative bound PDF sample interpretation with discriminating gain controls, validated source-octet SVG preservation, exact extents/opaque alpha/reports and replay; malformed-index acceptance alone cannot establish validity'
NEW_CLAUSE='exact image.decode-failed machine reports in SVG/PNG/PDF, independent missing-asset recovery artifact equivalence, exact artifact/page/image extents, opacity where applicable and fresh replay for malformed MP index ranges/count or missing secondary; independently prove the index inconsistency and characterize browser base-image fallback without claiming canonical visual equivalence for rejected input or granting a tolerance exception'
HELPERS={'capture':'tests/conformance/proposals/jpeg187-requalification-v1/collect.py',
         'qualification':'tests/conformance/proposals/jpeg188-requalification-v1/workflow.py',
         'receipts':'tests/conformance/proposals/jpeg188-requalification-v2/verify.py'}
ENV=dict(os.environ,PATH='/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:'+os.environ['PATH'],
         CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',CARGO_PROFILE_TEST_DEBUG='0',CARGO_PROFILE_RELEASE_DEBUG='0',PYTHONDONTWRITEBYTECODE='1')
def require(ok,message):
    if not ok:raise ValueError(message)
def sha(data):return hashlib.sha256(data).hexdigest()
def canonical(value):return json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()
def pretty(value):return json.dumps(value,indent=2,ensure_ascii=False).encode()+b'\n'
def read(path):
    try:return json.loads(path.read_bytes())
    except (OSError,ValueError) as e:raise ValueError(f'missing/invalid {path}: {e}') from e
def put(path,data):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as f:f.write(data)
def write(path,value):put(path,pretty(value))
def desc(path):
    data=path.read_bytes();return {'path':str(path.relative_to(ROOT)),'byte_length':len(data),'sha256':sha(data)}
def check(d):
    require(desc(ROOT/d['path'])==d,'stale evidence '+d['path']);return (ROOT/d['path']).read_bytes()
def run(args,cwd=ROOT):
    result=subprocess.run(list(map(str,args)),cwd=cwd,env=ENV,capture_output=True)
    require(result.returncode==0,result.stderr.decode()+result.stdout.decode());return result.stdout
def task(*args,cwd=ROOT):return run(['cargo','run','--release','--locked','-p','xtask','--','conformance',*args],cwd)

def archive():
    commit=run(['git','rev-parse',HEAD]).decode().strip()
    paths=[LEDGER,REGISTRY,POLICY,'tests/conformance/coverage/inventory.json','tests/conformance/coverage/requirements.json',
        'Cargo.toml','Cargo.lock','tests/package-consumer/Cargo.lock',SOURCE,'crates/excalidraw-render/src/render/asset/jpeg.rs',
        'xtask/src/conformance_pdf_bridge.rs','tests/package-consumer/examples/native_snapshots.rs',*HELPERS.values()]
    for path in paths:put(P/'before'/path,run(['git','show',f'{commit}:{path}']))
    require(sha((ROOT/REFERENCE).read_bytes())==REFERENCE_HASH,'frozen gain-map reference changed')
    write(P/'basis.json',{'approval':None,'base_commit':commit,'reference':desc(ROOT/REFERENCE),
        'helper_sources':{name:desc(P/'before'/path) for name,path in HELPERS.items()},
        'scope':'bounded two-image MPF validation; HDR decode/projection deferred; exact 16 recovery-criterion transition proposed'})

def helper(name):
    data=check(read(P/'basis.json')['helper_sources'][name])
    require(data==(ROOT/HELPERS[name]).read_bytes(),'approved historical helper changed')
    m=types.ModuleType('authenticated_jpeg190_'+name);m.__file__=str(ROOT/HELPERS[name])
    exec(compile(data,m.__file__,'exec',optimize=0),m.__dict__);return m
def collector():
    c=helper('capture');c.PACKET=P;c.ROOT=ROOT;return c
def cell_sets(reference=None):
    record=reference if reference is not None else read(ROOT/REFERENCE)
    cells={PREFIX+r['cell'] for r in record['cases']}
    index={PREFIX+r['cell'] for r in record['cases'] if r['signal'] in INDEX_SIGNALS}
    hdr={PREFIX+r['cell'] for r in record['cases'] if r['signal'] in HDR_SIGNALS}
    require(len(record['cases'])==104 and len(cells)==52 and len(index)==16 and len(hdr)==8,'closed MPF matrix differs')
    return cells,index,hdr
def check_graph():
    for path in ['Cargo.toml','Cargo.lock','tests/package-consumer/Cargo.lock']:
        require((ROOT/path).read_bytes()==(P/'before'/path).read_bytes(),'dependency change out of scope '+path)
    require(sha((ROOT/REFERENCE).read_bytes())==REFERENCE_HASH,'reference changed')

def old_capture():
    basis=read(P/'basis.json');c=collector()
    require(run(['git','rev-parse','HEAD'],OLD).decode().strip()==basis['base_commit'],'wrong OLD commit')
    require(not run(['git','status','--porcelain'],OLD),'OLD tree dirty')
    sources=c.tracked_state(OLD);write(P/'old/producer-files.json',sources)
    require(c.package(OLD,'old'),'OLD packaging failed');c.capture(OLD,'old')
    require(c.tracked_state(OLD)==sources,'OLD source mutation')
    prior=ROOT/'tests/conformance/proposals/jpeg189-requalification-v1/final-current/snapshots/snapshots.json'
    a=read(P/'old/snapshots/snapshots.json')['runs'][0]['results'];b=read(prior)['runs'][0]['results']
    require(len(a)==len(b)==168 and [r['key'] for r in a]==[r['key'] for r in b],'prior keys differ')
    write(P/'old/prior-capture-comparison.json',{'approval':None,'fresh_head':basis['base_commit'],'fresh':desc(P/'old/snapshots/snapshots.json'),
        'prior':desc(prior),'identical_records':sum(x==y for x,y in zip(a,b)),'total':168,'note':'fresh HEAD provenance; prior outputs not relabeled'})

def proposed_policy(before):
    _,index,_=cell_sets();after=copy.deepcopy(before);seen=set()
    for row in after['rows']:
        if row['id'] not in index:continue
        require(row['category']=='codec-profile' and row['spec_anchor']=='### 8.2 Raster and Nested-SVG Assets','policy category/authority drift')
        require(row['criterion'].count(OLD_CLAUSE)==1,'original canonical criterion no longer matches')
        row['criterion']=row['criterion'].replace(OLD_CLAUSE,NEW_CLAUSE);seen.add(row['id'])
    require(seen==index,'missing reviewed policy rows');return after

def verify_catalog_delta(before,after,before_policy_hash,after_policy_hash):
    _,index,_=cell_sets()
    require(len(before['obligations'])==len(after['obligations'])==2126,'catalog identity count changed')
    before_map={r['id']:r for r in before['obligations']};after_map={r['id']:r for r in after['obligations']}
    require(len(before_map)==len(after_map)==2126 and before_map.keys()==after_map.keys(),'catalog identities changed')
    normalized=copy.deepcopy(after);changed=[]
    for row in normalized['obligations']:
        old=before_map[row['id']]
        if row['id'] in index:
            require(row['criterion']==old['criterion'].replace(OLD_CLAUSE,NEW_CLAUSE),'unexpected criterion replacement')
            require(old['criterion'].count(OLD_CLAUSE)==1,'wrong old criterion')
            row['criterion']=old['criterion'];changed.append(row['id'])
        require(row==old,'unrelated authority/pointer/candidate/criterion change '+row['id'])
    require(len(changed)==16,'criterion transition count')
    for d in normalized['sources']:
        if d['path']==POLICY:
            require(d['sha256']==after_policy_hash,'after policy authority not bound');d['sha256']=before_policy_hash
    require(normalized==before,'unrelated catalog source changes')
    return sorted(changed)

def policy_proposal():
    before=read(P/'before'/POLICY);after=proposed_policy(before)
    require((ROOT/POLICY).read_bytes()==(P/'before'/POLICY).read_bytes(),'live policy already changed')
    prior=json.loads(task('ownership-catalog',cwd=OLD))
    require(sha(canonical(prior))==read(P/'before'/LEDGER)['catalog_sha256'],'OLD catalog not authenticated by ledger')
    # Use unchanged xtask derivation in detached OLD staging; restore immediately.
    path=OLD/POLICY;original=path.read_bytes();require(original==(P/'before'/POLICY).read_bytes(),'staging policy drift')
    try:
        path.write_bytes(pretty(after))
        derived=json.loads(task('ownership-catalog',cwd=OLD))
    finally:path.write_bytes(original)
    changes=verify_catalog_delta(prior,derived,sha(original),sha(pretty(after)))
    write(P/'policy/before-catalog.json',prior);write(P/'pending/ownership-policy.json',after)
    write(P/'pending/catalog.json',derived)
    write(P/'policy/criterion-transition.json',{'approval':None,'schema':'excalidraw-render/jpeg190-exact-criterion-transition/v1',
        'changed_ids':changes,'old_catalog_sha256':sha(canonical(prior)),'new_catalog_sha256':sha(canonical(derived)),
        'before_policy_sha256':sha(original),'proposed_policy_sha256':sha(pretty(after)),
        'rows':[{'before':next(r for r in before['rows'] if r['id']==key),'after':next(r for r in after['rows'] if r['id']==key)} for key in changes],
        'spec_authorities_pointers_and_candidates_unchanged':True,'requires_explicit_user_criterion_approval':True})
    print('Prepared exact 16-criterion replacement; live policy untouched, all catalog authorities retained except policy-file source digest.')

def validate_selectors(mapping,source):
    _,index,_=cell_sets();require(set(mapping)==index,'exact 16 malformed-index selector map required')
    for key,fn in mapping.items():
        require(isinstance(fn,str) and re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*',fn) and not any(s in fn for s in ['known','defect','diagnostic','unqualified']),'new strict-recovery selector required')
        require(len(re.findall(r'#\[test\]\s*(?:pub\s+)?fn\s+'+re.escape(fn)+r'\s*\(',source))==1,'new selector not unconditional test '+key)

def check_freeze(applied=False):
    f=read(P/'final-freeze.json');require(f['final_current_capture_authorized'] is True,'parent freeze required')
    for path,digest in f['producer_files'].items():
        if applied and path in [LEDGER,REGISTRY,POLICY]:continue
        require(sha((ROOT/path).read_bytes())==digest,'frozen source changed '+path)
    require(set(collector().tracked_state(ROOT))<=set(f['producer_files']),'freeze omitted tracked source')
    check_graph();validate_selectors(f['new_owner_selectors'],(ROOT/SOURCE).read_text());return f

def qualification_helper():
    q=helper('qualification');q.ROOT=ROOT;q.P=P;q.HEAD=HEAD;q.OLD=OLD;q.V10=V12;q.BRIDGE=BRIDGE;q.SOURCE=SOURCE
    q.REFERENCE=REFERENCE;q.REFERENCE_HASH=REFERENCE_HASH;q.collector=collector;q.check_graph=check_graph;q.validate_selectors=validate_selectors;q.check_freeze=check_freeze
    return q

def representative():
    check_freeze()
    qualification_helper().representative()

def expected_transaction(before,registry,source_hash,selectors,candidate,subject,file_hash,catalog_digest):
    cells,index,hdr=cell_sets();require(len(before['assignments'])==2126 and before['approval'] is None,'before ledger mismatch')
    rows=[r for r in before['assignments'] if r['id'].startswith(PREFIX)]
    require(len(rows)==52 and {r['id'] for r in rows}==cells,'52 exact MPF cells required')
    require({r['id'] for r in rows if r['disposition']['status']=='gap'}==index|hdr and set(selectors)==index,'16 index additions/eight HDR gaps differ')
    require(sum(r['disposition']['status']=='owned' for r in rows)==28,'28 existing owners required')
    require(all(re.fullmatch(r'[A-Za-z_][A-Za-z0-9_]*',s) and not any(w in s for w in ['known','defect','diagnostic','unqualified']) for s in selectors.values()),'invalid recovery selector')
    for digest in [source_hash,file_hash,catalog_digest]:require(re.fullmatch(r'[0-9a-f]{64}',digest),'invalid hash')
    require(re.fullmatch(r'sha256:[0-9a-f]{64}',subject),'invalid subject')
    command=lambda fn:['cargo','test','--locked','--release','-p','excalidraw-render','--test','fast','jpeg_gain_map_contract::'+fn,'--','--exact']
    new=copy.deepcopy(before);new['catalog_sha256']=catalog_digest
    for row in new['assignments']:
        if not row['id'].startswith(PREFIX):continue
        d=row['disposition']
        if d['status']=='owned':
            o=d['owner'];require(o['kind']=='test' and o['source']['path']==SOURCE and o['binary']=='fast' and o['command']==command(o['function']),'old owner command/source drift')
            o['source']['sha256']=source_hash
        elif row['id'] in index:
            require(d['status']=='gap' and d['issue']==190,'only unowned #190 index rows')
            fn=selectors[row['id']]
            row['disposition']={'status':'owned','owner':{'kind':'test','source':{'path':SOURCE,'sha256':source_hash},'binary':'fast','function':fn,'command':command(fn)},
                'rationale':'User-reviewed #190 narrow two-image MPF index validation and explicit criterion transition: independently reconstructed invalid ranges/count/missing secondary require exact image.decode-failed recovery in all formats; browser base fallback is characterized, not claimed visually equivalent. See tests/conformance/proposals/jpeg190-mpf-requalification-v1/. HDR projection/decoder work stays deferred.'}
        else:require(row['id'] in hdr and d['status']=='gap' and d['issue']==190,'HDR gap must remain unowned')
    out=copy.deepcopy(registry);require(len(out['cases'])==1 and out['cases'][0]['id']=='representative.stage4-frames-text-assets.v11','old representative mismatch')
    r=out['cases'][0];require(candidate['case_id']=='representative.stage4-frames-text-assets.v12','new representative mismatch')
    scopes=copy.deepcopy(r['exceptions'])
    for e in scopes:e['case_id']=candidate['case_id']
    require(candidate['exception_proposal']['exceptions']==scopes,'existing representative tolerance changed')
    r.update(id=candidate['case_id'],directory=V12,status='jpeg190-mpf-release-successor',exceptions=scopes,
        execution_keys=[candidate['case_id']+':default:'+f for f in ['svg','png','pdf']],consumer=['cargo','run','--release','--locked','-p','xtask','--','conformance','candidate-verify',V12])
    o=next(r for r in new['assignments'] if r['id']=='interaction:frames-text-assets')['disposition']['owner']
    require(o['kind']=='representative' and o['binding']['case']=='representative.stage4-frames-text-assets.v11','old rep owner mismatch')
    o['binding']={'case':candidate['case_id'],'registry':{'path':REGISTRY,'sha256':sha(pretty(out))},'manifest':{'path':V12+'/manifest.json','sha256':file_hash},'approval_subject_sha256':subject,'command':r['consumer']}
    require({s:sum(r['disposition']['status']==s for r in new['assignments']) for s in ['owned','gap','excluded']}=={'owned':2038,'gap':85,'excluded':3},'wrong final counts')
    return new,out

def verify_policy(applied=False):
    before=read(P/'before'/POLICY);expected=proposed_policy(before);proposal=read(P/'pending/ownership-policy.json')
    require(proposal==expected,'policy changed beyond reviewed exact 16 criteria')
    old=read(P/'policy/before-catalog.json');new=read(P/'pending/catalog.json');plan=read(P/'policy/criterion-transition.json')
    changed=verify_catalog_delta(old,new,sha((P/'before'/POLICY).read_bytes()),sha((P/'pending/ownership-policy.json').read_bytes()))
    require(plan['changed_ids']==changed and plan['old_catalog_sha256']==sha(canonical(old)) and plan['new_catalog_sha256']==sha(canonical(new)),'catalog plan differs')
    require(plan['before_policy_sha256']==sha((P/'before'/POLICY).read_bytes()) and plan['proposed_policy_sha256']==sha((P/'pending/ownership-policy.json').read_bytes()),'policy-file descriptors differ')
    require(plan['rows']==[{'before':next(r for r in before['rows'] if r['id']==key),'after':next(r for r in expected['rows'] if r['id']==key)} for key in changed],'reviewed before/after criterion rows differ')
    require(plan['old_catalog_sha256']==read(P/'before'/LEDGER)['catalog_sha256'],'old catalog unauthenticated')
    require(read(ROOT/POLICY)==(proposal if applied else before),'live policy does not match phase')
    return plan['new_catalog_sha256']

def migration():
    f=check_freeze();catalog=verify_policy(False);before=read(P/'before'/LEDGER);registry=read(P/'before'/REGISTRY)
    require(read(ROOT/LEDGER)==before and read(ROOT/REGISTRY)==registry,'pending live metadata changed')
    candidate=read(ROOT/V12/'manifest.json');require(candidate['approval'] is None,'pending candidate approved')
    subject=task('candidate-approval-subject',V12).decode().strip()
    require(sha(canonical(json.loads(task('ownership-catalog'))))==before['catalog_sha256'],'live catalog changed before authorization')
    ledger,out=expected_transaction(before,registry,sha((ROOT/SOURCE).read_bytes()),f['new_owner_selectors'],candidate,subject,sha((ROOT/V12/'manifest.json').read_bytes()),catalog)
    write(P/'pending/ownership.template.json',ledger);write(P/'pending/registry.json',out)
    _,index,hdr=cell_sets()
    write(P/'migration-plan.json',{'approval':None,'source':desc(ROOT/SOURCE),'new_owner_selectors':f['new_owner_selectors'],'new_owner_ids':sorted(index),'preserved_hdr_gap_ids':sorted(hdr),
        'refresh_owner_ids':[r['id'] for r in before['assignments'] if r['id'].startswith(PREFIX) and r['disposition']['status']=='owned'],
        'representative_subject_sha256':subject,'old_catalog_sha256':before['catalog_sha256'],'new_catalog_sha256':catalog,
        'policy_transition':desc(P/'policy/criterion-transition.json'),'counts':{'assignments':2126,'owned':2038,'excluded':3,'assigned_or_excluded':2041,'gaps':85}})
    verify_migration(False)

def verify_migration(applied):
    f=check_freeze(applied);catalog=verify_policy(applied);plan=read(P/'migration-plan.json');before=read(P/'before'/LEDGER);registry=read(P/'before'/REGISTRY)
    require(plan['source']==desc(ROOT/SOURCE) and plan['new_owner_selectors']==f['new_owner_selectors'],'final test source/selectors changed')
    require(plan['new_catalog_sha256']==catalog,'wrong final catalog digest')
    candidate=read(ROOT/V12/'manifest.json')
    ledger,out=expected_transaction(before,registry,plan['source']['sha256'],plan['new_owner_selectors'],candidate,plan['representative_subject_sha256'],sha((ROOT/V12/'manifest.json').read_bytes()),catalog)
    _,index,hdr=cell_sets();require(plan['new_owner_ids']==sorted(index) and plan['preserved_hdr_gap_ids']==sorted(hdr),'owner/gap scope changed')
    require(plan['policy_transition']==desc(P/'policy/criterion-transition.json'),'policy transition detached')
    require(plan['refresh_owner_ids']==[r['id'] for r in before['assignments'] if r['id'].startswith(PREFIX) and r['disposition']['status']=='owned'],'source refresh scope changed')
    for path in ['tests/conformance/coverage/inventory.json','tests/conformance/coverage/requirements.json']:require((ROOT/path).read_bytes()==(P/'before'/path).read_bytes(),'168 keys or criteria locators changed')
    require(read(P/'pending/registry.json')==out,'registry template differs')
    if applied:require(candidate['approval'] is not None and read(ROOT/LEDGER)==ledger and read(ROOT/REGISTRY)==out,'applied transaction differs')
    else:
        require(candidate['approval'] is None and read(P/'pending/ownership.template.json')==ledger,'pending proposal differs')
        require(read(ROOT/LEDGER)==before and read(ROOT/REGISTRY)==registry,'pending live metadata changed')

def affirmative(body,subject,rep):
    require(isinstance(body,str),'decision text required')
    match=re.fullmatch(r'\s*```json\r?\n([^`]+)\r?\n```\s*',body);require(match is not None,'one standalone JSON decision required')
    def unique(pairs):
        out={}
        for k,v in pairs:require(k not in out,'duplicate decision field');out[k]=v
        return out
    require(json.loads(match[1],object_pairs_hook=unique)=={'decision':'approve','scope':'jpeg190-mpf','production_subject_sha256':subject,'representative_subject_sha256':rep},'exact affirmative jpeg190-mpf decision required')
def authentication_helper():
    q=helper('qualification');q.ROOT=ROOT;q.affirmative=affirmative;return q
def authenticate(a,c,s,r,ra):return authentication_helper().authenticate(a,c,s,r,ra)
def fetch_comment(url):return authentication_helper().fetch_comment(url)

def qualification():
    r=read(P/'checks/parent-qualification.json');native=read(ROOT/V12/'native-capture.json')
    require(r['test_source']==desc(ROOT/SOURCE) and r['producer_sources']==native['producer_sources'],'qualification source set differs')
    for d in r['producer_sources']:check(d)
    require(r['reference_sha256']==REFERENCE_HASH and r['vectors']==104 and r['applicable_gates_passed'] is True,'MPF reference qualification missing')
    require(r['canonical_sdr_vectors']==24 and r['strict_recovery_vectors']==64 and r['hdr_known_failure_vectors']==16 and r['hdr_exact_pins']==32,'HDR/recovery/scope classification changed')
    hdr_rows=[row for row in read(ROOT/REFERENCE)['cases'] if row['signal'] in HDR_SIGNALS]
    measured={('hdr-ramp','ramp'):(553,0.860099630813284),('hdr-edges','ramp'):(648,0.8479795899866851),
              ('hdr-ramp','edges'):(1214,0.9116739114307459),('hdr-edges','edges'):(1262,0.8737353963660468)}
    expected_pins=[]
    for row in hdr_rows:
        changed,ssim=measured[(row['signal'],row['pattern'])]
        for fmt in ['png','pdf']:expected_pins.append({'id':row['id'],'format':fmt,'canonical_pass':False,'changed_pixels':changed,'ssim_bits':struct.pack('>d',ssim).hex()})
    require(r['hdr_failure_measurements']==expected_pins,'exact unchanged HDR failure pins required; no diagnostic success substitution')
    require(r['decoder_or_hdr_production_change'] is False and r['mpf_only'] is True,'HDR/decoder implementation out of scope')
    require(r['jpeg187_vectors']==76 and r['jpeg187_canonical_vectors']==64 and r['jpeg187_known_failure_vectors']==12 and r['jpeg187_exact_pins']==24,'#187 limitation changed')
    require(r['icc_vectors']==160 and r['icc_gates_passed'] is True and r['exif_vectors']==164 and r['exif_gates_passed'] is True,'prior ICC/EXIF gates missing')
    require(r['checks'] and all(c['passed'] is True for c in r['checks']),'passing focused checks missing')
    for c in r['checks']:check(c['log'])
    return r

def protected():
    check_freeze();exe=ROOT/'target/release/excalidraw-render';build=read(ROOT/V12/'native-capture.json')['build']
    require(sha(exe.read_bytes())==build['executable_sha256'],'release producer differs')
    old=ROOT/'tests/conformance/candidates/stage4-pdf-backend-requalification-v2';out=P/'protected-final'
    for i in range(2):
        r=subprocess.run([str(exe),'-','--output','-','--format','pdf','--report-format','json','--pdf-raster-fallback','forbid'],
            input=(ROOT/'tests/conformance/pdf-backend-qualification/project-operations.excalidraw').read_bytes(),cwd=ROOT,env=ENV,capture_output=True)
        require(r.returncode==0 and r.stdout==(old/'candidate.pdf').read_bytes() and r.stderr==(old/'candidate-report.json').read_bytes(),'protected output changed; STOP')
        put(out/f'run-{i}.pdf',r.stdout);put(out/f'run-{i}-report.json',r.stderr)
    run(['bash','tools/pdf-harness/run-structure.sh','inspect',out/'run-0.pdf',ROOT/'tests/conformance/pdf-backend-qualification/project-operations-structure.json','pass',out/'structure.json'])
    for viewer in ['pdfium','pdfjs']:
        for i in range(2):
            path=out/f'{viewer}-{i}.rgba';put(out/f'{viewer}-{i}-runtime.json',run(['bash',f'tools/pdf-harness/run-{viewer}.sh','render',out/'run-0.pdf',path,'520','200']))
            require(path.read_bytes()==(old/f'candidate-{viewer}.rgba').read_bytes(),'protected viewer changed; STOP')
    for fmt in ['svg','png','pdf']:
        for name in [f'native.{fmt}',f'native-{fmt}-report.json']:require((ROOT/V12/name).read_bytes()==(ROOT/PRIOR/name).read_bytes(),'representative changed; STOP')
    write(P/'checks/protected-equality.json',{'approval':None,'exact_protected_equality':True,'representative_equal_v11':True,'native_runs':2,'viewer_runs_each':2,'build':build})

def packet(applied=False):
    m=read(P/'manifest.json');bundle=read(P/'approval-subject.json')
    require(m['approval'] is None and m['approval_scope']=='jpeg190-mpf','packet scope changed')
    require(bundle=={'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in m.items() if k!='approval'},'files':m['files']},'envelope changed')
    for d in m['files']:check(d)
    if not applied:check(m['pending_representative_manifest'])
    return 'sha256:'+sha(canonical(bundle))
def evidence(applied=False):
    check_freeze(applied);qualification();verify_migration(applied)
    c=collector();c.verify_snapshots(P/'old/snapshots');c.verify_snapshots(P/'final-current/snapshots')
    plan=read(P/'migration-plan.json')
    require(sha(canonical(json.loads(task('ownership-catalog'))))==plan['new_catalog_sha256' if applied else 'old_catalog_sha256'],'live catalog phase mismatch')
    require(task('candidate-approval-subject',V12).decode().strip()==plan['representative_subject_sha256'],'representative subject changed')

def transaction(approval):
    manifest=read(ROOT/V12/'manifest.json');require(manifest['approval'] is None,'candidate already approved')
    manifest['approval']=dict(approval,approval_subject_sha256=read(P/'migration-plan.json')['representative_subject_sha256'])
    data=canonical(manifest)+b'\n';ledger=read(P/'pending/ownership.template.json')
    next(r for r in ledger['assignments'] if r['id']=='interaction:frames-text-assets')['disposition']['owner']['binding']['manifest']['sha256']=sha(data)
    return {V12+'/manifest.json':data,REGISTRY:(P/'pending/registry.json').read_bytes(),POLICY:(P/'pending/ownership-policy.json').read_bytes(),LEDGER:pretty(ledger)}

def current_binding(subject):
    check_freeze(True)
    return {'production_subject_sha256':subject,'representative_subject_sha256':read(P/'migration-plan.json')['representative_subject_sha256'],
        'frozen_source_record':desc(P/'final-freeze.json'),'post_application_files':[desc(ROOT/p) for p in [V12+'/manifest.json',REGISTRY,LEDGER,POLICY,'tests/conformance/coverage/inventory.json',
        'xtask/src/main.rs','xtask/src/coverage/ownership.rs','xtask/tests/coverage.rs','xtask/tests/representative_candidate.rs',SOURCE,'Cargo.toml','Cargo.lock']],
        'workflow':desc(P/'workflow.py'),'receipt_entry':desc(P/'receipt_gate.py'),'original_receipt_helper':read(P/'basis.json')['helper_sources']['receipts'],
        'test_inventory':desc(P/'target-identities.json')}

def receipt_gate():
    helper('receipts');raw=(P/'before'/HELPERS['receipts']).read_text()
    patches={'jpeg188-post-approval-receipts/v1':'jpeg190-post-approval-receipts/v1','jpeg188-receipt-transaction/v1':'jpeg190-receipt-transaction/v1',
        'verified 1992 ownership assignments: 1915 assigned/excluded, 77 explicit gaps; ledger approval pending':'verified 2126 ownership assignments: 2041 assigned/excluded, 85 explicit gaps; ledger approval pending'}
    for before,after in patches.items():
        require(raw.count(before)==(2 if before=='jpeg188-post-approval-receipts/v1' else 1),'receipt adaptation seam changed');raw=raw.replace(before,after)
    g=types.ModuleType('jpeg190_strict_receipt_adapter');g.__file__=str(P/'receipt_gate.py')
    exec(compile(raw,g.__file__,'exec',optimize=0),g.__dict__)
    g.ROOT=ROOT;g.P=P;g.BASE=P;g.V10=V12;g.CASE='representative.stage4-frames-text-assets.v12';g.LEDGER=LEDGER;g.REGISTRY=REGISTRY;g.APP='tests/conformance/coverage/jpeg190-application-v1'
    g.REP=read(P/'migration-plan.json')['representative_subject_sha256'] if (P/'migration-plan.json').exists() else 'UNMATERIALIZED-V12-NO-SUBJECT'
    g.COMMANDS=[('affected-tests',g.TEST_COMMAND),('candidate',g.PREFIX+['candidate-verify',V12]),('registry',g.PREFIX+['candidate-executions']),
        ('owner',g.PREFIX+['verify-representative-owner',g.CASE]),('inventory',g.PREFIX+['verify-inventory']),('ownership',g.PREFIX+['verify-ownership'])]
    api=types.SimpleNamespace(authenticate=authenticate,fetch_comment=fetch_comment,verify_migration=verify_migration)
    g.load_original=lambda name:api if name=='workflow.py' else (_ for _ in ()).throw(ValueError('unexpected delegated helper'))
    g.packet=packet;g.evidence=evidence;g.current_binding=current_binding;g.transaction=transaction
    return g

def seal():
    require(not (P/'manifest.json').exists(),'refuse overwrite')
    check_freeze();review=qualification();verify_migration(False);receipt_gate().test_inventory()
    require(read(P/'checks/protected-equality.json')['exact_protected_equality'] is True,'protected proof missing')
    a=read(P/'old/snapshots/snapshots.json')['runs'][0]['results'];b=read(P/'final-current/snapshots/snapshots.json')['runs'][0]['results']
    require(len(a)==len(b)==168 and [r['key'] for r in a]==[r['key'] for r in b],'full inventory keys changed')
    changes=[{'key':x['key'],'before':x['samples'][0],'after':y['samples'][0]} for x,y in zip(a,b) if x!=y]
    require(review['diagnosed_inventory_changes']==changes,'unexplained output changes; STOP')
    write(P/'inventory-comparison.json',{'approval':None,'keys':168,'unchanged':168-len(changes),'changes':changes})
    files=[desc(p) for base in [P,ROOT/V12,ROOT/BRIDGE] for p in sorted(base.rglob('*')) if p.is_file() and p.name not in ['README.md','file-inventory.json'] and '__pycache__' not in p.parts and p!=ROOT/V12/'manifest.json']
    m={'schema':'excalidraw-render/jpeg190-mpf-requalification/v1','approval':None,'status':'pending-explicit-user-approval','approval_scope':'jpeg190-mpf',
        'representative_subject_sha256':read(P/'migration-plan.json')['representative_subject_sha256'],'new_owners':16,'source_refreshes':28,'criterion_changes':16,
        'hdr_decoder_or_projection_approval':False,'baseline_refresh':False,'global_approval':False,'files':files,'pending_representative_manifest':desc(ROOT/V12/'manifest.json')}
    write(P/'manifest.json',m);write(P/'approval-subject.json',{'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in m.items() if k!='approval'},'files':files})
    receipt_gate().verify(False)

def freeze(authorization):
    # The proposed policy is already fully itemized before source collection;
    # current live policy remains the old criterion set until actual approval.
    verify_policy(False)
    qualification_helper().freeze(authorization)

def readiness():
    require(not (P/'final-freeze.json').exists(),'pre-freeze only')
    inventory='tests/conformance/proposals/jpeg188-requalification-v2/target-identities.json'
    data=run(['git','show',read(P/'basis.json')['base_commit']+':'+inventory])
    require((ROOT/inventory).read_bytes()==data,'original receipt test inventory changed')
    put(P/'target-identities.json',data)
    require([r['count'] for r in receipt_gate().test_inventory()]==[103,5,1],'exact 109 receipt targets required')
    result=subprocess.run([sys.executable,str(P/'test_workflow.py')],cwd=ROOT,env=ENV,capture_output=True)
    put(P/'checks/readiness-tests.stdout',result.stdout);put(P/'checks/readiness-tests.stderr',result.stderr)
    require(result.returncode==0,result.stderr.decode())
    check_graph();verify_policy(False)
    before=(P/'before/xtask/src/conformance_pdf_bridge.rs').read_bytes()
    require((ROOT/'xtask/src/conformance_pdf_bridge.rs').read_bytes()==before.replace(b'stage4-pdf-colorspace-bridge-v5',b'stage4-pdf-colorspace-bridge-v6'),'bridge changed beyond constant')
    for path in [LEDGER,REGISTRY,POLICY,'tests/conformance/coverage/inventory.json','tests/conformance/coverage/requirements.json']:
        require((ROOT/path).read_bytes()==(P/'before'/path).read_bytes(),'live metadata changed '+path)
    write(P/'readiness.json',{'approval':None,'status':'awaiting-parent-policy-review-production-checks-and-explicit-freeze','current_capture_started':False,
        'reference':desc(ROOT/REFERENCE),'old_capture':desc(P/'old/snapshots/snapshots.json'),'old_identical_to_prior':read(P/'old/prior-capture-comparison.json')['identical_records'],
        'criterion_transition':desc(P/'policy/criterion-transition.json'),'new_owners':16,'existing_source_refreshes':28,'HDR_gaps_preserved':8,
        'counts_after':{'assignments':2126,'owned':2038,'excluded':3,'assigned_or_excluded':2041,'gaps':85},
        'checker_test_log':desc(P/'checks/readiness-tests.stderr'),'required_from_parent':'review exact 16 changed criteria, narrow MPF admission constraints and final new-owner selectors; send explicit freeze after fmt/Clippy/tests',
        'HDR_diagnosis':'external reduced-IDCT finding does not grant current production qualification or decoder permission'})
    write(P/'file-inventory.json',{'approval':None,'files':[desc(p) for p in sorted(P.rglob('*')) if p.is_file() and p.name!='file-inventory.json' and '__pycache__' not in p.parts]})
    print('Ready: OLD 168 capture; exact 16 criterion proposal; strict owner/receipt tests; current capture blocked.')

if __name__=='__main__':
    args=sys.argv[1:]
    if args==['archive']:archive()
    elif args==['old']:old_capture()
    elif args==['policy-proposal']:policy_proposal()
    elif len(args)==2 and args[0]=='freeze':freeze(args[1])
    elif len(args)==2 and args[0]=='stage':qualification_helper().stage(args[1])
    elif len(args)==2 and args[0]=='final':qualification_helper().final_capture(args[1])
    elif args==['representative']:representative()
    elif args==['protected']:protected()
    elif args==['migration']:migration()
    elif args==['seal']:seal()
    elif args==['readiness']:readiness()
    else:raise SystemExit('archive|old|policy-proposal|freeze AUTH|stage ROOT|final ROOT|representative|protected|migration|seal')
