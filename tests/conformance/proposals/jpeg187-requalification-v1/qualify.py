#!/usr/bin/env python3
"""Prepare/check unapproved JPEG #187 evidence and an exact metadata proposal.

No command grants approval or writes the live ledger/registry. The parent owns
the eventual transaction; applied auditing requires its actual decision records.
"""
import sys
if not __debug__ or sys.flags.optimize:
    raise SystemExit('Optimized Python is forbidden; no workflow started.')

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
PACKET = Path(__file__).resolve().parent
HEAD = '4e5b624'
V9 = 'tests/conformance/candidates/stage4-frames-text-assets-v9'
BRIDGE = 'tests/conformance/candidates/stage4-pdf-colorspace-bridge-v3'
LEDGER = 'tests/conformance/coverage/ownership.json'
REGISTRY = 'tests/conformance/candidates/registry.json'
SOURCE = 'crates/excalidraw-render/tests/fast/jpeg_color_contract.rs'
OWNER_FN = 'qualified_gray_rgb_and_ycbcr_cells_pass_both_reference_authorities'
PREFIX = 'codec:jpeg:color-sampling:'
APPROVAL_FIELDS = {'approved_by', 'approved_at', 'approval_url', 'approval_subject_sha256'}
OLD_LOCK = '27717cb869ee8b99a6a401f0c14cb4953c3e19813a69033c9121d8beca513cd2'
NEW_LOCK = OLD_LOCK
CONSUMER = '897026587dec22941b32ecef3d55d51988f1813c006ab7dd355810fc2f39b5c5'
DEFERRED = {PREFIX + f'{model}-{sampling}-baseline' for model in ['ycbcr', 'ycck'] for sampling in ['4x2', '1x4', '2x4']}
NEW_COLOR = {PREFIX + f'cmyk-1x1-{mode}' for mode in ['baseline', 'progressive']} | {
    PREFIX + f'ycck-{sampling}-{mode}' for sampling in ['1x1','2x1','1x2','2x2','4x1','4x2','1x4','2x4']
    for mode in ['baseline','progressive']} - DEFERRED
ENV = dict(os.environ, PATH='/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:' + os.environ['PATH'], CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_PROFILE_RELEASE_DEBUG='0')
def require(condition, message):
    if not condition: raise ValueError(message)
def sha(data): return hashlib.sha256(data).hexdigest()
def canonical(value): return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
def read(path):
    try: return json.loads(path.read_bytes())
    except (OSError, ValueError) as error: raise ValueError(f'missing/invalid {path}: {error}') from error
def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as f: f.write(data)
def write(path, value): put(path, json.dumps(value, indent=2, ensure_ascii=False).encode() + b'\n')
def desc(path):
    data = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'sha256': sha(data), 'byte_length': len(data)}
def run(args):
    output = subprocess.run(list(map(str, args)), cwd=ROOT, env=ENV, capture_output=True)
    require(output.returncode == 0, output.stderr.decode() + output.stdout.decode())
    return output.stdout
def task(*args): return run(['cargo', 'run', '--release', '--locked', '-p', 'xtask', '--', 'conformance', *args])
def collector():
    module = types.ModuleType('jpeg187_capture')
    module.__file__ = str(PACKET / 'collect.py')
    exec(compile((PACKET / 'collect.py').read_bytes(), module.__file__, 'exec'), module.__dict__)
    return module

def archive():
    # Archives and the former RC transition already exist and remain immutable.
    # For the stable decision this operation only authenticates the retained bytes.
    commit = run(['git', 'rev-parse', HEAD]).decode().strip()
    paths = ['Cargo.lock', 'Cargo.toml', 'tests/package-consumer/Cargo.lock', LEDGER, REGISTRY,
             'tests/conformance/coverage/inventory.json', 'tests/conformance/coverage/requirements.json',
             'tests/conformance/coverage/ownership-policy.json', SOURCE,
             'crates/excalidraw-render/src/render/asset/jpeg.rs', 'xtask/src/conformance_pdf_bridge.rs']
    for path in paths: require((PACKET/'before'/path).read_bytes()==run(['git','show',f'{commit}:{path}']), 'historical archive changed: '+path)
    require(sha((PACKET / 'before/Cargo.lock').read_bytes()) == OLD_LOCK, 'wrong historical root lock')
    require(sha((ROOT / 'Cargo.lock').read_bytes()) == NEW_LOCK, 'current root graph differs')
    require(sha((ROOT / 'tests/package-consumer/Cargo.lock').read_bytes()) == CONSUMER, 'stable consumer lock not restored')
    reference = read(ROOT / 'tests/oracle/jpeg-color/reference.json')
    require(next(d['sha256'] for d in reference['sources'] if d['path'] == 'Cargo.lock') == OLD_LOCK, 'historical reference changed')
    require((ROOT/'Cargo.toml').read_bytes()==(PACKET/'before/Cargo.toml').read_bytes(),'workspace manifest not restored')
    print('Historical include_bytes path: ../../../../tests/conformance/proposals/jpeg187-requalification-v1/before/Cargo.lock')
    print('Authenticated unchanged stable HEAD root/consumer locks; old RC transition preserved as retired evidence.')

def check_freeze():
    freeze = read(PACKET/'final-freeze.json')
    require(freeze.get('final_current_capture_authorized') is True, 'parent final freeze is required')
    paths = freeze['producer_files']
    required = set(collector().tracked_state(ROOT))
    require(required <= set(paths), 'freeze omits tracked sources')
    for path, expected in paths.items():
        require(sha((ROOT/path).read_bytes()) == expected, 'frozen source changed: '+path)
    require(sha((ROOT/'Cargo.lock').read_bytes()) == NEW_LOCK, 'root graph changed')
    require(sha((ROOT/'tests/package-consumer/Cargo.lock').read_bytes()) == CONSUMER, 'consumer graph changed')
    for path in ['Cargo.toml','Cargo.lock','tests/package-consumer/Cargo.lock']:
        require((ROOT/path).read_bytes()==(PACKET/'before'/path).read_bytes(),'stable HEAD graph differs: '+path)
    return freeze

def freeze(authorization):
    # Parent supplies this only AFTER the explicit freeze message, never inferred.
    decision = read(Path(authorization))
    require(decision.get('final_current_capture_authorized') is True and bool(decision.get('parent_freeze_message')), 'explicit parent freeze message required')
    paths = collector().tracked_state(ROOT)
    for p in PACKET.rglob('*'):
        if p.is_file() and p.name not in ['file-inventory.json','readiness.json'] and '__pycache__' not in p.parts:
            paths[str(p.relative_to(ROOT))] = sha(p.read_bytes())
    write(PACKET/'final-freeze.json', {'schema':'excalidraw-render/jpeg187-final-freeze/v1', 'final_current_capture_authorized':True,
          'parent_freeze_message':decision['parent_freeze_message'], 'producer_files':paths, 'approval':None})
    check_freeze()

def stage_final(destination):
    frozen=check_freeze(); stage=Path(destination).resolve()
    require(stage.parent==Path('/tmp/opencode') and not stage.exists(),'fresh /tmp/opencode stage required')
    run(['git','worktree','add','--detach',stage,HEAD])
    for path,expected in frozen['producer_files'].items():
        data=(ROOT/path).read_bytes(); require(sha(data)==expected,'source changed while staging')
        target=stage/path; target.parent.mkdir(parents=True,exist_ok=True)
        with target.open('wb') as output: output.write(data)
    check_freeze()
    print(str(stage)+' staged from exact final freeze; comparison evidence remains unapproved.')

def representative():
    check_freeze()
    commands = [('bridge-v3', ['collect-pdf-colorspace-bridge']), ('revise-v9',['candidate-revise',V9])]
    for name,args in commands: put(PACKET/'final-checks'/f'{name}.json',task(*args))
    for directory in ['assets','viewer-evidence']: (ROOT/V9/directory).mkdir()
    prefix=['cargo','run','--release','--locked','-p','xtask','--','conformance']
    commands=[('native',prefix+['candidate-native-capture',V9]),
              ('assets',['node','tools/reference-oracle/scripts/stage4-viewer-assets.mjs',V9]),
              ('derive',prefix+['candidate-prepare-assets',V9]),
              ('fonts',['node','tools/reference-oracle/scripts/stage4-font-audit-qualification.mjs',V9,'collect']),
              ('viewers',['node','tools/reference-oracle/scripts/stage4-candidate-views.mjs','collect',V9]),
              ('exceptions',prefix+['candidate-propose-exceptions',V9]),('inspect',prefix+['candidate-inspect',V9]),
              ('materialize',prefix+['candidate-materialize',V9]),('subject',prefix+['candidate-approval-subject',V9])]
    for name,args in commands:
        put(PACKET/'final-checks'/f'v9-{name}.json',run(args)); check_freeze()

def owner_command(): return ['cargo','test','--locked','--release','-p','excalidraw-render','--test','fast','jpeg_color_contract::'+OWNER_FN,'--','--exact']

def expected_transaction(before, old_registry, source_hash, v9, v9_subject, v9_file_hash):
    require(len(before['assignments']) == 1912 and before['approval'] is None, 'unexpected prior ledger')
    rows=[r for r in before['assignments'] if r['id'].startswith(PREFIX)]
    inputs=read(ROOT/'tests/oracle/jpeg-color/inputs.json')['cases']
    cells={PREFIX+r['cell'] for r in inputs}
    require(len(rows)==38 and {r['id'] for r in rows}==cells, 'exact 38 policy cells required')
    require(sum(r['disposition']['status']=='owned' for r in rows)==17, 'expected 17 old JPEG owners')
    require(sum(r['disposition']['status']=='gap' for r in rows)==21, 'expected 21 #187 gaps')
    require({r['id'] for r in rows if r['disposition']['status']=='gap'} == NEW_COLOR | DEFERRED, 'unexpected color/sampling gap partition')
    source={'path':SOURCE,'sha256':source_hash}
    owner={'kind':'test','source':source,'function':OWNER_FN,'binary':'fast','command':owner_command()}
    ledger=copy.deepcopy(before)
    for r in ledger['assignments']:
        if not r['id'].startswith(PREFIX): continue
        d=r['disposition']
        if d['status']=='owned':
            existing=copy.deepcopy(d['owner']); existing['source']=source
            require(existing==owner,'existing owner selector/command differs')
            d['owner']['source']=source
        else:
            require(d['status']=='gap' and d['issue']==187,'not an unowned #187 cell')
            if r['id'] in DEFERRED: continue
            require(r['id'] in NEW_COLOR, 'unreviewed owner addition')
            r['disposition']={'status':'owned','owner':copy.deepcopy(owner),
                'rationale':'User-reviewed #187 CMYK/YCCK integer color correction on the unchanged stable HEAD graph; both frozen patterns and both independent color authorities pass canonical gates. Six baseline sampling cells remain unowned #187 known failures. Exact qualification/migration: tests/conformance/proposals/jpeg187-requalification-v1/.'}
    require(v9['case_id']=='representative.stage4-frames-text-assets.v9', 'wrong successor')
    require(re.fullmatch(r'[0-9a-f]{64}',source_hash) is not None and re.fullmatch(r'[0-9a-f]{64}',v9_file_hash) is not None,'invalid source/manifest digest')
    require(re.fullmatch(r'sha256:[0-9a-f]{64}',v9_subject) is not None,'invalid representative subject')
    registry=copy.deepcopy(old_registry)
    require(len(registry['cases'])==1 and registry['cases'][0]['id']=='representative.stage4-frames-text-assets.v8','unexpected old representative')
    row=registry['cases'][0]
    scopes=copy.deepcopy(row['exceptions'])
    for scope in scopes: scope['case_id']=v9['case_id']
    require(v9['exception_proposal']['exceptions']==scopes,'representative tolerances/cause/profile changed')
    row.update(id=v9['case_id'],directory=V9,status='jpeg187-requalified-release-successor',
               execution_keys=[v9['case_id']+':default:'+f for f in ['svg','png','pdf']],
               consumer=['cargo','run','--release','--locked','-p','xtask','--','conformance','candidate-verify',V9],
               exceptions=v9['exception_proposal']['exceptions'])
    selected=next(r for r in ledger['assignments'] if r['id']=='interaction:frames-text-assets')
    require(selected['disposition']['owner']['kind']=='representative' and selected['disposition']['owner']['binding']['case']=='representative.stage4-frames-text-assets.v8','unexpected old representative owner')
    selected['disposition']['owner']['binding']={'case':v9['case_id'],
        'registry':{'path':REGISTRY,'sha256':sha(json.dumps(registry,indent=2,ensure_ascii=False).encode()+b'\n')},
        'manifest':{'path':V9+'/manifest.json','sha256':v9_file_hash},
        'approval_subject_sha256':v9_subject,'command':row['consumer']}
    # Retain the historical owner rationale; exact successor identity lives in binding.
    return ledger,registry

def migration():
    check_freeze()
    before=read(PACKET/'before'/LEDGER); old_registry=read(PACKET/'before'/REGISTRY)
    require((ROOT/LEDGER).read_bytes()==(PACKET/'before'/LEDGER).read_bytes(),'live ledger changed')
    require((ROOT/REGISTRY).read_bytes()==(PACKET/'before'/REGISTRY).read_bytes(),'live registry changed')
    v9=read(ROOT/V9/'manifest.json'); require(v9['approval'] is None,'V9 already approved')
    subject=task('candidate-approval-subject',V9).decode().strip()
    catalog=json.loads(task('ownership-catalog'))
    require(sha(canonical(catalog))==before['catalog_sha256'],'catalog/criteria changed')
    ledger,registry=expected_transaction(before,old_registry,sha((ROOT/SOURCE).read_bytes()),v9,subject,sha((ROOT/V9/'manifest.json').read_bytes()))
    write(PACKET/'pending/ownership.template.json',ledger)
    write(PACKET/'pending/registry.json',registry)
    write(PACKET/'pending/catalog.json',catalog)
    write(PACKET/'migration-plan.json',{'approval':None,'schema':'excalidraw-render/jpeg187-migration/v1',
        'source':desc(ROOT/SOURCE),'new_owner_ids':[r['id'] for r in before['assignments'] if r['id'] in NEW_COLOR],
        'deferred_sampling_ids':sorted(DEFERRED),
        'refresh_owner_ids':[r['id'] for r in before['assignments'] if r['id'].startswith(PREFIX) and r['disposition']['status']=='owned'],
        'representative_subject_sha256':subject,'catalog_sha256':before['catalog_sha256'],
        'counts':{'assignments':1912,'owned':1832,'excluded':3,'assigned_or_excluded':1835,'gaps':77},
        'parameterized_field':'Final approved V9 manifest file digest only; no synthetic approval or live assignments in this proposal.'})
    verify_migration(False)

def verify_migration(applied):
    plan=read(PACKET/'migration-plan.json')
    require(desc(ROOT/SOURCE)==plan['source'],'stale JPEG test source')
    before=read(PACKET/'before'/LEDGER); old_registry=read(PACKET/'before'/REGISTRY)
    v9=read(ROOT/V9/'manifest.json')
    expected,registry=expected_transaction(before,old_registry,plan['source']['sha256'],v9,plan['representative_subject_sha256'],sha((ROOT/V9/'manifest.json').read_bytes()))
    for path in ['tests/conformance/coverage/inventory.json','tests/conformance/coverage/requirements.json','tests/conformance/coverage/ownership-policy.json']:
        require((ROOT/path).read_bytes()==(PACKET/'before'/path).read_bytes(),'criteria/inventory changed: '+path)
    counts={s:sum(r['disposition']['status']==s for r in expected['assignments']) for s in ['owned','gap','excluded']}
    require(counts=={'owned':1832,'gap':77,'excluded':3},'migration totals differ')
    require(len(plan['new_owner_ids'])==15 and len(plan['refresh_owner_ids'])==17,'wrong migration scope')
    require(plan['new_owner_ids']==[r['id'] for r in before['assignments'] if r['id'] in NEW_COLOR],'new owner identity set changed')
    require(plan['deferred_sampling_ids']==sorted(DEFERRED),'deferred sampling scope changed')
    require(plan['refresh_owner_ids']==[r['id'] for r in before['assignments'] if r['id'].startswith(PREFIX) and r['disposition']['status']=='owned'],'refreshed owner identity set changed')
    require(read(PACKET/'pending/registry.json')==registry,'registry template differs')
    if applied:
        require(v9['approval'] is not None,'missing V9 approval')
        require(read(ROOT/LEDGER)==expected and read(ROOT/REGISTRY)==registry,'applied transaction differs')
    else:
        require(v9['approval'] is None,'pending candidate must have null approval')
        require(read(PACKET/'pending/ownership.template.json')==expected,'ledger template differs')
        require(read(ROOT/LEDGER)==before and read(ROOT/REGISTRY)==old_registry,'pending preparation changed live metadata')

def fetch_comment(url):
    match=re.fullmatch(r'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-([1-9][0-9]*)',url)
    require(match is not None,'approval requires genuine issue #85 decision URL')
    return json.loads(run(['gh','api','repos/sagikazarmark/excalidraw-render.orig/issues/comments/'+match[1]]))

def authenticate(approval,comment,subject,representative_subject,representative_approval):
    require(isinstance(approval,dict) and set(approval)==APPROVAL_FIELDS,'exact four-field production approval required')
    require(approval['approval_subject_sha256']==subject,'wrong production subject')
    require(isinstance(approval['approved_by'],str) and re.fullmatch(r'[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?',approval['approved_by']),'invalid login')
    require(isinstance(approval['approved_at'],str) and date.fromisoformat(approval['approved_at']).isoformat()==approval['approved_at'],'invalid date')
    match=re.fullmatch(r'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-([1-9][0-9]*)',approval['approval_url'])
    require(match is not None and comment['id']==int(match[1]),'comment ID mismatch')
    require(comment['html_url']==approval['approval_url'] and comment['user']['login']==approval['approved_by'] and comment['created_at'][:10]==approval['approved_at'],'decision identity/date/url mismatch')
    for s in [subject,representative_subject]: require(re.search(re.escape(s)+r'(?![0-9a-fA-F])',comment['body']) is not None,'decision must name both exact subjects')
    remote=fetch_comment(approval['approval_url'])
    stable=lambda v:{k:v[k] for k in ['id','html_url','created_at','body']}|{'login':v['user']['login']}
    require(stable(remote)==stable(comment),'saved decision differs from genuine GitHub comment')
    require(representative_approval==dict(approval,approval_subject_sha256=representative_subject),'representative approval is not same scoped decision')

def authenticate_application(subject,representative_subject):
    directory=ROOT/'tests/conformance/coverage/jpeg187-application-v1'
    status=read(directory/'status.json')
    require(status['state']=='applied' and status['production_subject_sha256']==subject and status['representative_subject_sha256']==representative_subject,'application not successfully applied')
    authenticate(read(directory/'approval.json'),read(directory/'comment.json'),subject,representative_subject,read(ROOT/V9/'manifest.json')['approval'])

def protected():
    check_freeze()
    old=ROOT/'tests/conformance/candidates/stage4-pdf-backend-requalification-v2'
    evidence=PACKET/'protected-final'
    build=read(ROOT/V9/'native-capture.json')['build']
    exe=ROOT/'target/release/excalidraw-render'
    require(sha(exe.read_bytes())==build['executable_sha256'],'release producer differs from V9')
    scene=(ROOT/'tests/conformance/pdf-backend-qualification/project-operations.excalidraw').read_bytes()
    commands=[]
    for i in range(2):
        args=[str(exe),'-','--output','-','--format','pdf','--report-format','json','--pdf-raster-fallback','forbid']
        result=subprocess.run(args,input=scene,cwd=ROOT,env=ENV,capture_output=True)
        require(result.returncode==0,'protected PDF replay failed')
        require(result.stdout==(old/'candidate.pdf').read_bytes() and result.stderr==(old/'candidate-report.json').read_bytes(),'protected PDF/report changed: stop for review, no pin update')
        put(evidence/f'run-{i}.pdf',result.stdout); put(evidence/f'run-{i}-report.json',result.stderr)
        commands.append(args[1:])
    for viewer in ['pdfium','pdfjs']:
        for i in range(2):
            output=evidence/f'{viewer}-{i}.rgba'
            result=run(['bash',f'tools/pdf-harness/run-{viewer}.sh','render',evidence/'run-0.pdf',output,'520','200'])
            require(output.read_bytes()==(old/f'candidate-{viewer}.rgba').read_bytes(),'protected viewer changed')
            put(evidence/f'{viewer}-{i}-runtime.json',result)
    run(['bash','tools/pdf-harness/run-structure.sh','inspect',evidence/'run-0.pdf',ROOT/'tests/conformance/pdf-backend-qualification/project-operations-structure.json','pass',evidence/'structure.json'])
    representative=[]
    for fmt in ['svg','png','pdf']:
        before=ROOT/'tests/conformance/candidates/stage4-frames-text-assets-v8'/f'native.{fmt}'
        after=ROOT/V9/f'native.{fmt}'
        require(before.read_bytes()==after.read_bytes(),'representative artifact changed')
        before_report=before.parent/f'native-{fmt}-report.json'
        after_report=after.parent/f'native-{fmt}-report.json'
        require(before_report.read_bytes()==after_report.read_bytes(),'representative full report changed')
        representative.append({'format':fmt,'artifact_sha256':sha(after.read_bytes()),'report_sha256':sha(after_report.read_bytes()),'equal_to_approved_v8':True})
    write(PACKET/'final-checks/protected-equality.json',{'approval':None,'current_build':build,'protected_pdf_sha256':sha((old/'candidate.pdf').read_bytes()),'fresh_native_processes':2,'fresh_viewer_runs_each':2,'native_reports_and_viewer_bytes_equal':True,'structure_passed':True,'request_args':commands,'representative':representative})
    check_freeze()

def check_sampling_pins(review):
    require(review['qualified_vectors']==64 and review['known_failure_vectors']==12, 'color-only split must be 64 canonical / 12 known-failure vectors')
    require(review['known_sampling_cells']==sorted(DEFERRED), 'six known sampling cells must remain explicit')
    pins=review['sampling_defect_pins']
    expected={(cell.removeprefix(PREFIX)+'-'+pattern, fmt) for cell in DEFERRED for pattern in ['ramp','edges'] for fmt in ['png','pdf']}
    require(len(pins)==24 and {(p['id'],p['format']) for p in pins}==expected, 'exact 24 known failure pins required')
    for pin in pins:
        require(pin['canonical_pass'] is False and isinstance(pin['changed_pixels'],int) and pin['changed_pixels']>=0 and re.fullmatch(r'[0-9a-f]{16}',pin['ssim_bits']), 'known failure cannot be relabeled pass or tolerance')
    neutral=review['neutral_failure_control']
    require(neutral['followup_issue']==187 and neutral['canonical_pass'] is False and neutral['test_source']==desc(ROOT/SOURCE), 'missing exact source-bound minimized neutral failure control')
    require(desc(ROOT/neutral['measurements']['path'])==neutral['measurements'], 'neutral failure measurements stale')

def check_final_findings():
    controls=read(PACKET/'nested/controls.json')
    result=read(PACKET/'nested/qualification.final-current.json')
    require(result['approval'] is None and result['final_freeze_recollection_required'] is False,'not final nested evidence')
    require(len(result['records'])==len(controls)==10,'nested control set incomplete')
    for r,control in zip(result['records'],controls):
        require(r['id']==control['id'] and r['label']=='final-current','nested control identity changed')
        require(r['page_form_Do_image_binding'] is True and r['output_intent_bound'] is True,'nested object semantics failed')
        if control['filter']:
            require(r['comparisons']==[],'filtered control has no invented independent blur oracle')
        else:
            require([v['target'] for v in r['comparisons']]==['bound-pdf-samples.rgba','pdfium-0.rgba','pdfjs-0.rgba'],'missing nested comparison')
            for value in r['comparisons']:
                if r['id']=='png-rgba8' and value['target']=='pdfium-0.rgba':
                    require(value['canonical_pass'] is False and value['changed_pixels']==24 and value['ssim_bits']=='3febf9303648d4c8','preexisting finding moved; investigate')
                    for repeat in range(2):
                        require((PACKET/f'nested/old/png-rgba8/pdfium-{repeat}.rgba').read_bytes()==(PACKET/f'nested/final-current/png-rgba8/pdfium-{repeat}.rgba').read_bytes(),'preexisting alpha finding changed')
                elif r['id'] in ['jpeg-ycbcr-4x2-baseline-edges','jpeg-ycck-2x4-baseline-edges']:
                    review=read(PACKET/'final-checks/production-qualification.json')
                    require(review['test_source']==desc(ROOT/SOURCE),'known failure review not bound to final test')
                    pins=review['nested_sampling_defect_pins']
                    require(len(pins)==6 and len({(p['id'],p['target']) for p in pins})==6,'exact six nested comparison pins required')
                    pin=next((p for p in pins if p['id']==r['id'] and p['target']==value['target']),None)
                    require(pin is not None and value['canonical_pass'] is False and all(value[k]==pin[k] for k in ['canonical_pass','changed_pixels','ssim_bits']), 'nested sampling failure moved or lacks reviewed exact pin')
                    if r['id']=='jpeg-ycbcr-4x2-baseline-edges':
                        old=read(PACKET/'nested/qualification.exploratory.json')
                        prior=next(p for p in old['records'] if p['label']=='old' and p['id']==r['id'])
                        require(value==next(v for v in prior['comparisons'] if v['target']==value['target']), 'YCbCr unchanged-stable sampling failure moved')
                else: require(value['canonical_pass'] is True,'new nested comparison failure')
    comparison=read(PACKET/'nested/comparison.final-current.json')
    for row in comparison['controls']:
        require(not row['failures'],'nested collector/viewer failure')
        require(row['diagnostics_before']==row['diagnostics_after'],'nested report diagnostics changed')
        if not row['id'].startswith('jpeg-'):
            require(all(f['artifact_equal'] and f['report_equal'] for f in row['formats']),'non-JPEG output changed')
            require(row['pdfium']['bytes_equal'] and row['pdfjs']['bytes_equal'],'non-JPEG viewer changed')

def verify(applied=False):
    manifest=read(PACKET/'manifest.json'); bundle=read(PACKET/'approval-subject.json')
    require(manifest['approval'] is None,'immutable review packet must retain null approval')
    require(bundle=={'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in manifest.items() if k!='approval'},'files':manifest['files']},'subject envelope differs')
    subject='sha256:'+sha(canonical(bundle))
    for d in manifest['files']:
        require(desc(ROOT/d['path'])==d,'stale packet evidence '+d['path'])
    freeze=read(PACKET/'final-freeze.json')
    for p,h in freeze['producer_files'].items():
        if applied and p in [LEDGER,REGISTRY]: continue
        require(sha((ROOT/p).read_bytes())==h,'stale frozen source '+p)
    check_final_findings()
    check_sampling_pins(read(PACKET/'final-checks/production-qualification.json'))
    collector().verify_snapshots(PACKET/'old/snapshots')
    collector().verify_snapshots(PACKET/'final-current/snapshots')
    require(sha(canonical(json.loads(task('ownership-catalog'))))==read(PACKET/'migration-plan.json')['catalog_sha256'],'catalog criteria changed')
    if applied:
        authenticate_application(subject,manifest['representative_subject_sha256'])
    verify_migration(applied)
    require(task('candidate-approval-subject',V9).decode().strip()==manifest['representative_subject_sha256'],'V9 subject changed')
    print(subject + (' authenticated applied state' if applied else ' pending; no live migration'))

def seal():
    check_freeze(); verify_migration(False)
    require((PACKET/'final-current/snapshots/snapshots.json').exists(),'final 168 capture missing')
    require((PACKET/'nested/qualification.final-current.json').exists(),'final nested qualification missing')
    require((PACKET/'final-checks/production-qualification.json').exists(),'parent final source-bound test/review results required')
    require((PACKET/'final-checks/protected-equality.json').exists(),'protected PDF/representative equality missing')
    check_final_findings()
    c=collector(); c.verify_snapshots(PACKET/'old/snapshots'); c.verify_snapshots(PACKET/'final-current/snapshots')
    old=read(PACKET/'old/snapshots/snapshots.json')['runs'][0]['results']; new=read(PACKET/'final-current/snapshots/snapshots.json')['runs'][0]['results']
    changes=[]
    for a,b in zip(old,new):
        require(a['key']==b['key'],'168 full keys changed')
        if a!=b: changes.append({'key':a['key'],'before':a['samples'][0],'after':b['samples'][0]})
    review=read(PACKET/'final-checks/production-qualification.json')
    require(review['jpeg_reference_sha256']=='9b5ab60a18e83b52c582ccb38728b71633b11b592ffe540bab2c796857694638' and review['frozen_input_reconstruction_passed'] is True and review['reconstructed_vectors']==76,'missing exact frozen JPEG input reconstruction')
    check_sampling_pins(review)
    require(review['test_source']==desc(ROOT/SOURCE),'parent test result not bound to final test source')
    native_sources=read(ROOT/V9/'native-capture.json')['producer_sources']
    require(review['producer_sources']==native_sources,'parent qualification must bind complete final native producer set')
    for source in review['producer_sources']: require(desc(ROOT/source['path'])==source,'stale parent qualification producer')
    require(len(review['checks'])>0 and all(row['passed'] is True for row in review['checks']),'parent qualification checks missing/failing')
    for row in review['checks']:
        require(desc(ROOT/row['log']['path'])==row['log'],'parent test log stale')
    require(review['diagnosed_inventory_changes']==changes,'every changed inventory output requires exact diagnosis review')
    write(PACKET/'final-inventory-comparison.json',{'approval':None,'keys':168,'unchanged':168-len(changes),'changes':changes})
    files=[]
    for base in [PACKET,ROOT/V9,ROOT/BRIDGE]:
        for p in sorted(base.rglob('*')):
            if p.is_file() and p.name not in ['README.md','file-inventory.json','readiness.json'] and '__pycache__' not in p.parts:
                if base==ROOT/V9 and p.name=='manifest.json': continue # bare subject separately verifies approval-excluded manifest
                files.append(desc(p))
    manifest={'schema':'excalidraw-render/jpeg187-production-requalification/v1','approval':None,'status':'pending-exact-user-approval',
              'originating_issue':'https://github.com/sagikazarmark/excalidraw-render.orig/issues/187',
              'representative_subject_sha256':read(PACKET/'migration-plan.json')['representative_subject_sha256'],'files':files,
              'known_preexisting_finding':{'id':'png-rgba8-pdfium','changed_pixels':24,'ssim_bits':'3febf9303648d4c8','old_new_identical_required':True,'exception_granted':False,'blocks_unrelated_jpeg187':False},
              'scope':'stable-graph-color-only','deferred_sampling_cells':sorted(DEFERRED),
              'baseline_refresh':False,'global_approval':False}
    write(PACKET/'manifest.json',manifest)
    write(PACKET/'approval-subject.json',{'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in manifest.items() if k!='approval'},'files':files})
    verify(False)

def readiness():
    require(not (PACKET/'final-freeze.json').exists(),'readiness is pre-freeze only')
    preserved=read(PACKET/'retired-rc-plan/preserved-evidence.json')
    for row in preserved['files']: require(sha((PACKET/row['path']).read_bytes())==row['sha256'],'original/RC evidence changed: '+row['path'])
    tests=[]
    for name in ['test_qualify.py','test_collect.py']:
        output=subprocess.run([sys.executable,str(PACKET/name)],cwd=ROOT,env=dict(ENV,PYTHONDONTWRITEBYTECODE='1'),capture_output=True)
        require(output.returncode==0,output.stderr.decode())
        log=PACKET/'color-only-readiness-checks'/(name+'.log')
        put(log,output.stdout+output.stderr)
        tests.append({'command':[sys.executable,str(PACKET/name)],'passed':True,'log':desc(log)})
    path=PACKET/'readiness.json'
    record={'status':'ready-for-parent-final-freeze-not-captured','approval':None,
            'final_capture_performed':False,'selected_consumer_lock_sha256':CONSUMER,
            'old_root_lock_sha256':OLD_LOCK,'current_root_lock_sha256':NEW_LOCK,
            'bridge_constant':'v3-only-no-validation-changes','tests':tests,
            'scope':'stable-graph-color-only','deferred_sampling_cells':sorted(DEFERRED),
            'expected_migration':{'new_owners':15,'source_refreshes':17,'representative_replacement':1,'assignments':1912,'assigned_or_excluded':1835,'gaps':77},
            'known_preexisting_finding':{'control':'png-rgba8','viewer':'pdfium','changed_pixels':24,'ssim_bits':'3febf9303648d4c8','exception':False,'blocks_unrelated_jpeg187':False}}
    path.write_bytes(json.dumps(record,indent=2).encode()+b'\n')
    files=[desc(p) for p in sorted(PACKET.rglob('*')) if p.is_file() and p.name!='file-inventory.json' and '__pycache__' not in p.parts]
    (PACKET/'file-inventory.json').write_bytes(json.dumps({'approval':None,'files':files},indent=2).encode()+b'\n')
    print('Color-only readiness recorded; final capture remains blocked awaiting explicit freeze.')

if __name__=='__main__':
    mode=sys.argv[1]
    if mode=='archive': archive()
    elif mode=='freeze': freeze(sys.argv[2])
    elif mode=='stage-final': stage_final(sys.argv[2])
    elif mode=='representative': representative()
    elif mode=='migration': migration()
    elif mode=='protected': protected()
    elif mode=='verify': verify()
    elif mode=='verify-applied': verify(True)
    elif mode=='seal': seal()
    elif mode=='readiness': readiness()
    else: raise SystemExit('archive|freeze AUTHORIZATION|stage-final DEST|representative|protected|migration|seal|verify|verify-applied')
