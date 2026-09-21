#!/usr/bin/env python3
"""Bound exact migration, authenticated decision and fresh successful receipts."""
import sys
if not __debug__ or sys.flags.optimize:
    raise SystemExit('Optimized Python is forbidden')
import json
from pathlib import Path
import re
import subprocess
import types

ROOT = Path(__file__).resolve().parents[4]
P = Path(__file__).resolve().parent
CASE = 'representative.stage4-frames-text-assets.v14'
V14 = 'tests/conformance/candidates/stage4-frames-text-assets-v14'
BRIDGE = 'tests/conformance/candidates/stage4-pdf-colorspace-bridge-v8'
LEDGER = 'tests/conformance/coverage/ownership.json'
REGISTRY = 'tests/conformance/candidates/registry.json'
FOCUSED = 'tests/conformance/cases/focused/rectangle-outline/pdf-renderer-expectations.json'
HELPERS = {'receipt': 'tests/conformance/proposals/jpeg188-requalification-v2/verify.py',
           'auth': 'tests/conformance/proposals/jpeg188-requalification-v1/workflow.py',
           'snapshot': 'tests/conformance/proposals/jpeg187-requalification-v1/collect.py'}


def load(path, name):
    module = types.ModuleType(name)
    module.__file__ = str(path)
    exec(compile(path.read_bytes(), str(path), 'exec'), module.__dict__)
    return module


c = load(P/'collect.py', 'collection')
m = load(P/'migration.py', 'migration')
REP = (P/'logs/final-subject.stdout').read_text().strip()


def require(ok, message):
    if not ok:
        raise ValueError(message)


def read(path):
    return json.loads(path.read_bytes())


def descriptor(path):
    data = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'byte_length':len(data), 'sha256':c.sha(data)}


def check(d):
    require(descriptor(ROOT/d['path']) == d, 'stale descriptor '+d['path'])


def canonical(v):
    return json.dumps(v, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()


def authenticated(name):
    path = ROOT/HELPERS[name]
    archived = P/'helpers'/name
    expected = read(P/'helper-bindings.json')[name]
    require(c.sha(archived.read_bytes()) == expected, 'archived helper changed')
    require(path.read_bytes() == archived.read_bytes(), 'original helper changed')
    return load(archived, 'authenticated_'+name)


def initialize():
    bindings = {}
    for name, path in HELPERS.items():
        data = subprocess.check_output(['git', 'show', '0cbdad3:'+path], cwd=ROOT)
        require(data == (ROOT/path).read_bytes(), 'helper differs from committed source')
        c.put(P/'helpers'/name, data)
        bindings[name] = c.sha(data)
    c.put(P/'helper-bindings.json', m.pretty(bindings))
    original = ROOT/'tests/conformance/proposals/jpeg188-requalification-v2/target-identities.json'
    c.put(P/'target-identities.json', original.read_bytes())


def freeze_check(applied):
    for path, digest in read(P/'final-freeze.json')['files'].items():
        if applied and path in [LEDGER, REGISTRY]:
            continue
        if path == FOCUSED:
            focused_check(applied)
            continue
        require(c.sha((ROOT/path).read_bytes()) == digest, 'source freeze changed '+path)


def focused_check(required):
    live = (ROOT/FOCUSED).read_bytes()
    if live == (P/'before'/FOCUSED).read_bytes():
        require(not required, 'focused #44 approval/application is required first')
        return
    value = json.loads(live)
    approval = value.pop('approval')
    wanted = read(P/'pending/focused-pdf-expectations.json')
    wanted.pop('approval')
    require(value == wanted, 'focused change exceeds exact profile rebinding')
    subject = 'sha256:'+c.sha(canonical(value)+b'\n')
    require(subject == read(P/'focused-plan.json')['subject_sha256'], 'focused subject mismatch')
    require(set(approval)=={'approved_by','approved_at','approval_url','approval_subject_sha256'},'focused approval fields')
    require(approval['approved_by']=='sagikazarmark' and approval['approval_subject_sha256']==subject,'focused approval identity')
    match = re.fullmatch(r'https://github.com/sagikazarmark/excalidraw-render.orig/issues/44#issuecomment-(\d+)',approval['approval_url'])
    require(match is not None,'focused approval must be issue44')
    comment = json.loads(subprocess.check_output(['gh','api','repos/sagikazarmark/excalidraw-render.orig/issues/comments/'+match[1]],cwd=ROOT))
    require(comment['user']['login']=='sagikazarmark' and comment['html_url']==approval['approval_url'] and comment['created_at'][:10]==approval['approved_at'],'focused remote identity mismatch')
    decision = re.fullmatch(r'\s*```json\r?\n([^`]+)\r?\n```\s*',comment['body'])
    require(decision is not None,'focused standalone decision required')
    def unique(pairs):
        out={}
        for key,val in pairs:
            require(key not in out,'duplicate focused decision key');out[key]=val
        return out
    require(json.loads(decision[1],object_pairs_hook=unique)=={'decision':'approve','scope':'pdfium192-focused-rectangle','approval_subject_sha256':subject},'focused exact affirmative decision required')


def verify_migration(applied):
    ledger, registry, changes = m.expected(applied)
    require(len(changes)==36, 'source refresh scope')
    require((P/'pending/registry.json').read_bytes()==m.pretty(registry), 'registry template changed')
    if applied:
        require((ROOT/LEDGER).read_bytes()==m.pretty(ledger), 'applied ledger differs')
        require((ROOT/REGISTRY).read_bytes()==m.pretty(registry), 'applied registry differs')
    else:
        m.verify()
    for path in ['tests/conformance/coverage/ownership-policy.json','tests/conformance/coverage/inventory.json']:
        require((ROOT/path).read_bytes()==(P/'before'/path).read_bytes(),'policy/inventory changed')


def packet(applied):
    manifest = read(P/'manifest-v2.json')
    require(manifest['approval'] is None and manifest['scope']=='pdfium192-successor', 'packet scope')
    require(manifest['representative_subject_sha256']==REP, 'representative subject changed')
    for d in manifest['files']:
        check(d)
    if not applied:
        check(manifest['pending_representative_manifest'])
    return 'sha256:'+c.sha(canonical(manifest))


def evidence(applied):
    freeze_check(applied)
    verify_migration(applied)
    collector = authenticated('snapshot')
    collector.ROOT = ROOT
    collector.verify_snapshots(P/'final-retry/snapshots')
    comparison = read(P/'inventory-comparison.json')
    require(comparison['keys']==comparison['unchanged']==168 and comparison['changes']==[], 'unreviewed output changes')
    check(comparison['prior']); check(comparison['current'])
    old = read(ROOT/comparison['prior']['path'])['runs'][0]['results']
    new = read(ROOT/comparison['current']['path'])['runs'][0]['results']
    require(old==new, 'packaged output mismatch')
    proof = read(P/'protected-build-binding.json')
    native = read(ROOT/V14/'native-capture.json')
    require(proof['build']==native['build'],'protected build differs from V14')
    for d in proof['files']: check(d)
    if applied:
        result=subprocess.run(['cargo','test','--locked','-p','excalidraw-render','--test','fast','pdf_renderer_expectations::focused_pdf_expectations_stay_bound_to_their_case_and_dependencies','--','--exact'],cwd=ROOT,env=c.ENV,capture_output=True)
        require(result.returncode==0 and b'1 passed; 0 failed;' in result.stdout,'focused expectation verification failed: '+result.stderr.decode()+result.stdout.decode())
    result = subprocess.run(['cargo','run','--release','--locked','-p','xtask','--','conformance','candidate-approval-subject',V14], cwd=ROOT, env=c.ENV, capture_output=True)
    require(result.returncode==0 and result.stdout.decode().strip()==REP, result.stderr.decode())


def affirmative(body, subject, rep):
    match = re.fullmatch(r'\s*```json\r?\n([^`]+)\r?\n```\s*', body)
    require(match is not None, 'single standalone JSON decision required')
    def unique(pairs):
        result = {}
        for k,v in pairs:
            require(k not in result, 'duplicate decision field')
            result[k]=v
        return result
    require(json.loads(match[1], object_pairs_hook=unique)=={
        'decision':'approve','scope':'pdfium192-successor',
        'production_subject_sha256':subject,'representative_subject_sha256':rep}, 'exact affirmative decision required')


def auth():
    helper = authenticated('auth')
    helper.ROOT=ROOT
    helper.affirmative=affirmative
    return helper


def binding(subject):
    freeze_check(True)
    return {'production_subject_sha256':subject,'representative_subject_sha256':REP,
            'frozen_sources':descriptor(P/'final-freeze.json'),
            'metadata':[descriptor(ROOT/p) for p in [V14+'/manifest.json',LEDGER,REGISTRY]],
            'focused':descriptor(ROOT/FOCUSED),
            'verifier':descriptor(P/'verify.py'),'migration':descriptor(P/'migration.py'),
            'test_inventory':descriptor(P/'target-identities.json')}


def gate():
    g = authenticated('receipt')
    # Parameterize the existing reviewed receipt engine; retain its strict 109
    # identity parser, actual command receipts, fresh reruns and rollback.
    g.ROOT=ROOT; g.P=P; g.BASE=P; g.V10=V14; g.REP=REP; g.CASE=CASE
    g.LEDGER=LEDGER; g.REGISTRY=REGISTRY; g.APP='tests/conformance/coverage/pdfium192-application-v1'
    g.COMMANDS=[('affected-tests',g.TEST_COMMAND),('candidate',g.PREFIX+['candidate-verify',V14]),
        ('registry',g.PREFIX+['candidate-executions']),('owner',g.PREFIX+['verify-representative-owner',CASE]),
        ('inventory',g.PREFIX+['verify-inventory']),('ownership',g.PREFIX+['verify-ownership'])]
    original = g.outcome
    def outcome(name, stdout, stderr):
        if name != 'ownership':
            return original(name,stdout,stderr)
        require(not re.search(rb'(^error:|panicked at|FAILED)',stderr,re.M),'ownership error')
        require(stdout==b'verified 2418 ownership assignments: 2321 assigned/excluded, 97 explicit gaps; ledger approval pending\n','ownership totals differ')
        return {'verified_output':stdout.decode()}
    g.outcome=outcome
    g.packet=packet; g.evidence=evidence; g.current_binding=binding
    def api(name):
        require(name=='workflow.py','unexpected helper request')
        h=auth()
        return types.SimpleNamespace(authenticate=h.authenticate,fetch_comment=h.fetch_comment,verify_migration=verify_migration)
    g.load_original=api
    return g


def seal():
    evidence(False)
    gate().test_inventory()
    files=[]
    for directory in [P, ROOT/V14, ROOT/BRIDGE]:
        for path in sorted(directory.rglob('*')):
            if path.is_file() and '__pycache__' not in path.parts and path.name!='README.md' and path!=ROOT/V14/'manifest.json':
                files.append(descriptor(path))
    value={'schema':'pdfium192-successor-requalification/v1','approval':None,'scope':'pdfium192-successor',
           'representative_subject_sha256':REP,'files':files,
           'pending_representative_manifest':descriptor(ROOT/V14/'manifest.json'),
           'new_owners':0,'source_refreshes':36,'gaps':97}
    c.put(P/'manifest-v2.json',m.pretty(value))
    print('sha256:'+c.sha(canonical(value)))


if __name__=='__main__':
    args=sys.argv[1:]
    if args==['initialize']: initialize()
    elif args==['seal']: seal()
    elif args==['verify']: gate().verify(False)
    elif args==['verify-applied']: gate().verify(True)
    elif len(args)==2 and args[0]=='apply': gate().apply(args[1])
    else: raise SystemExit('initialize|seal|verify|verify-applied|apply APPROVAL_JSON')
