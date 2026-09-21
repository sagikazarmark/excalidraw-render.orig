#!/usr/bin/env python3
"""Apply the explicitly approved #187 three-file transaction with rollback."""
import sys
if not __debug__ or sys.flags.optimize:
    raise SystemExit('Optimized Python forbidden')
import copy
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import types
import uuid

ROOT = Path('/home/laborant/excalidraw-render.orig')
PACKET = ROOT / 'tests/conformance/proposals/jpeg187-requalification-v2'
BASE = ROOT / 'tests/conformance/proposals/jpeg187-requalification-v1'
APPLICATION = ROOT / 'tests/conformance/coverage/jpeg187-application-v2'
SUBJECT = 'sha256:0baf0e3f8b7bbf37f575f0e8d32650befa7b73dd7a0a6c0c45e383c1a1693f32'
URL = 'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5754180693'
ENV = dict(os.environ, PATH='/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:' + os.environ['PATH'],
           CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_PROFILE_RELEASE_DEBUG='0', PYTHONDONTWRITEBYTECODE='1')
v = types.ModuleType('approved_strict_verifier')
v.__file__ = str(PACKET / 'verify.py')
exec(compile((PACKET / 'verify.py').read_bytes(), v.__file__, 'exec', optimize=0), v.__dict__)
def pretty(value): return json.dumps(value, indent=2, ensure_ascii=False).encode() + b'\n'
def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as output:
        output.write(data); output.flush(); os.fsync(output.fileno())
def status_write(value):
    tmp = APPLICATION / ('status.tmp-' + uuid.uuid4().hex)
    put(tmp, pretty(value)); tmp.replace(APPLICATION / 'status.json')
def command(name, args):
    result = subprocess.run(args, cwd=ROOT, env=ENV, capture_output=True)
    put(APPLICATION / 'logs' / (name + '.stdout'), result.stdout)
    put(APPLICATION / 'logs' / (name + '.stderr'), result.stderr)
    v.require(result.returncode == 0, f'{name}: {result.stdout.decode()} {result.stderr.decode()}')
    print(name + ': passed', flush=True)
    return result.stdout
def check_receipts(data, registry=False):
    report = json.loads(data)
    if registry:
        v.require(len(report['cases']) == 1, 'wrong registry case count')
        report = report['cases'][0]
    v.require(report['approval_required'] is True and report['approval_subject_sha256'] == v.V9_SUBJECT.removeprefix('sha256:'), 'wrong approved receipt subject')
    v.require(report['case_id'] == 'representative.stage4-frames-text-assets.v9' and len(report['executions']) == 3, 'wrong receipt set')
    for row, fmt in zip(report['executions'], ['svg','png','pdf']):
        v.require(row['execution_key'] == report['case_id'] + ':default:' + fmt, 'wrong execution key')
        v.require(row['native']['format'] == fmt and row['native']['fresh_process_runs'] == 2, 'missing native repeated release execution')
    return report

v.require(not APPLICATION.exists(), 'application directory already exists; preserve and inspect before retry')
v.require(v.packet() == SUBJECT, 'approved production subject changed')
v.verify(False)
comment = v.fetch_comment(URL)
approval = {'approved_by': comment['user']['login'], 'approved_at': comment['created_at'][:10],
            'approval_url': comment['html_url'], 'approval_subject_sha256': SUBJECT}
v9_approval = dict(approval, approval_subject_sha256=v.V9_SUBJECT)
v.authenticate(approval, comment, SUBJECT, v9_approval)
q = v.load_frozen('qualify.py')
q.verify_migration(False)
manifest_path = v.V9 + '/manifest.json'
original_manifest = v.read(ROOT / manifest_path)
v.require(original_manifest['approval'] is None, 'candidate already approved')
manifest = copy.deepcopy(original_manifest); manifest['approval'] = v9_approval
manifest_bytes = v.canonical(manifest) + b'\n'
registry_bytes = (BASE / 'pending/registry.json').read_bytes()
ledger = v.read(BASE / 'pending/ownership.template.json')
row = next(r for r in ledger['assignments'] if r['id'] == 'interaction:frames-text-assets')
row['disposition']['owner']['binding']['manifest']['sha256'] = v.sha(manifest_bytes)
files = {manifest_path: manifest_bytes, q.REGISTRY: registry_bytes, q.LEDGER: pretty(ledger)}
previous = {path: (ROOT / path).read_bytes() for path in files}
paths = subprocess.check_output(['git','ls-files','-z','--cached','--others','--exclude-standard'], cwd=ROOT).decode().split('\0')
snapshot = {p: v.sha((ROOT/p).read_bytes()) for p in sorted(set(paths)) if p and (ROOT/p).is_file()}
APPLICATION.mkdir()
state = {'schema':'excalidraw-render/jpeg187-approved-application/v1','attempt_id':uuid.uuid4().hex,'state':'preparing',
         'production_subject_sha256':SUBJECT,'representative_subject_sha256':v.V9_SUBJECT,
         'started_at':datetime.now(timezone.utc).isoformat(), 'before_sha256':{p:v.sha(data) for p,data in previous.items()}}
try:
    status_write(state)
    put(APPLICATION/'transaction.py', Path(__file__).read_bytes())
    put(APPLICATION/'approval.json', pretty(approval))
    put(APPLICATION/'comment.json', pretty(comment))
    put(APPLICATION/'before-files.json', pretty(snapshot))
    for path,data in previous.items(): put(APPLICATION/'before'/path,data)
    # Recheck exact preimages immediately before any mutation.
    for path,data in previous.items(): v.require((ROOT/path).read_bytes()==data,'preimage raced: '+path)
    state['state']='applying'; status_write(state)
    for path,data in files.items(): (ROOT/path).write_bytes(data)
    prefix=['cargo','run','--release','--locked','-p','xtask','--','conformance']
    direct=check_receipts(command('candidate-verify',prefix+['candidate-verify',v.V9]))
    registry=check_receipts(command('candidate-executions',prefix+['candidate-executions']),True)
    v.require(direct==registry,'direct and registry receipt discrepancy')
    command('representative-owner',prefix+['verify-representative-owner','representative.stage4-frames-text-assets.v9'])
    command('inventory',prefix+['verify-inventory'])
    command('ownership',prefix+['verify-ownership'])
    command('affected-xtask-targets',['cargo','test','--locked','-p','xtask','--bin','xtask','--test','coverage','--test','representative_candidate','--no-fail-fast'])
    # Strong decision and exact migration audit before claiming application.
    v.authenticate(approval, v.read(APPLICATION/'comment.json'), SUBJECT, v.read(ROOT/manifest_path)['approval'])
    v.evidence(True)
    for path,expected in snapshot.items():
        if path not in files: v.require(v.sha((ROOT/path).read_bytes())==expected,'unrelated file changed: '+path)
    changes=[{'path':p,'before_sha256':v.sha(previous[p]),'after':v.descriptor(ROOT/p)} for p in files]
    put(APPLICATION/'applied-transition.json', pretty({'approval':approval,'production_subject_sha256':SUBJECT,
        'representative_subject_sha256':v.V9_SUBJECT,'files':changes,'new_owner_count':15,'refreshed_source_count':17,
        'representative_replacement_count':1,'counts':{'assignments':1912,'owned':1832,'excluded':3,'assigned_or_excluded':1835,'gaps':77},
        'inventory_full_keys_preserved':168,'policy_criteria_preserved':38,'global_approval':None,
        'direct_receipts':direct,'registry_receipts':registry,'preexisting_files_checked':len(snapshot)}))
    state['state']='applied'; status_write(state)
    command('strong-verify-applied',['python3',str(PACKET/'verify.py'),'verify-applied'])
except BaseException as error:
    rollback_errors=[]
    for path,data in previous.items():
        try:
            (ROOT/path).write_bytes(data)
            v.require((ROOT/path).read_bytes()==data,'rollback byte mismatch')
        except BaseException as failure: rollback_errors.append(f'{path}: {failure}')
    state.update(state='rollback_failed' if rollback_errors else 'failed_rolled_back', error=f'{type(error).__name__}: {error}', rollback_errors=rollback_errors)
    status_write(state)
    raise
print(json.dumps({'approval':approval,'counts':{'assignments':1912,'assigned_or_excluded':1835,'gaps':77},'live_metadata_files':list(files)},indent=2))
