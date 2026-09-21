#!/usr/bin/env python3
"""Read-only pending/applied audit; requires affirmative designated-maintainer approval.

Never records a decision or edits the live transaction. The parent applies the
reviewed metadata only after explicit user approval and authenticates it here.
"""
import sys
if not __debug__ or sys.flags.optimize:
    raise SystemExit('Optimized Python forbidden; no workflow started.')

from datetime import date
import hashlib
import json
from pathlib import Path
import re
import subprocess
import types

ROOT = Path(__file__).resolve().parents[4]
PACKET = Path(__file__).resolve().parent
BASE = ROOT / 'tests/conformance/proposals/jpeg187-requalification-v1'
BASE_SUBJECT = 'sha256:e3a794f1461248ccf2ad92b73b7d7cd2aa780386d7790ff1310e4678766105a6'
V9 = 'tests/conformance/candidates/stage4-frames-text-assets-v9'
V9_SUBJECT = 'sha256:00f193a54cf4076911ec0a679c6f82e50c09d4b75502b1a88c5de3d8e1b6858d'
MAINTAINER = 'sagikazarmark'
APPLICATION = 'tests/conformance/coverage/jpeg187-application-v2'
APPROVAL_KEYS = {'approved_by', 'approved_at', 'approval_url', 'approval_subject_sha256'}
DECISION_KEYS = {'decision', 'scope', 'production_subject_sha256', 'representative_subject_sha256'}


def require(ok, message):
    if not ok: raise ValueError(message)


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        require(key not in value, 'duplicate JSON field: ' + key)
        value[key] = item
    return value


def parse(data):
    return json.loads(data, object_pairs_hook=unique_object)


def read(path):
    try: return parse(path.read_bytes())
    except (OSError, ValueError) as error: raise ValueError(f'missing/invalid {path.name}: {error}') from error


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()


def sha(data): return hashlib.sha256(data).hexdigest()


def descriptor(path):
    data = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'byte_length': len(data), 'sha256': sha(data)}


def check(d):
    require(descriptor(ROOT / d['path']) == d, 'stale descriptor: ' + d['path'])


def packet(applied=False):
    manifest = read(PACKET / 'manifest.json')
    bundle = read(PACKET / 'approval-subject.json')
    require(manifest['approval'] is None, 'immutable packet approval must remain null')
    require(bundle == {'schema': 'excalidraw-render/conformance-approval-subject/v1',
                      'manifest': {k: v for k, v in manifest.items() if k != 'approval'}, 'files': manifest['files']}, 'v2 envelope differs')
    require(manifest['prior_production_subject_sha256'] == BASE_SUBJECT and manifest['representative_subject_sha256'] == V9_SUBJECT, 'wrong predecessor/V9 identity')
    for d in manifest['files'] + manifest['preserved_review_files']: check(d)
    if not applied: check(manifest['pending_representative_manifest'])
    base = read(BASE / 'approval-subject.json')
    require('sha256:' + sha(canonical(base)) == BASE_SUBJECT, 'original production subject changed')
    return 'sha256:' + sha(canonical(bundle))


def load_frozen(name):
    relative = str((BASE / name).relative_to(ROOT))
    bound = next(d for d in read(BASE / 'approval-subject.json')['files'] if d['path'] == relative)
    check(bound)
    module = types.ModuleType('authenticated_' + name.replace('.', '_'))
    module.__file__ = str(BASE / name)
    exec(compile((BASE / name).read_bytes(), module.__file__, 'exec', optimize=0), module.__dict__)
    return module


def affirmative_decision(body, subject):
    # The entire comment is one decision block. No prose, quotations, duplicate
    # blocks or contradictory rejection outside a block can grant authority.
    require(isinstance(body, str), 'decision body must be text')
    match = re.fullmatch(r'\s*```json\r?\n([^`]+)\r?\n```\s*', body)
    require(match is not None, 'exactly one standalone fenced JSON decision required')
    decision = parse(match[1])
    require(isinstance(decision, dict) and set(decision) == DECISION_KEYS, 'exact four decision fields required')
    require(decision == {'decision': 'approve', 'scope': 'jpeg187-color-only',
                         'production_subject_sha256': subject,
                         'representative_subject_sha256': V9_SUBJECT}, 'affirmative exact color-only decision required')
    return decision


def fetch_comment(url):
    match = re.fullmatch(r'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-([1-9][0-9]*)', url)
    require(match is not None, 'actual issue #85 comment URL required')
    result = subprocess.run(['gh', 'api', 'repos/sagikazarmark/excalidraw-render.orig/issues/comments/' + match[1]],
                            cwd=ROOT, capture_output=True, check=True)
    return parse(result.stdout)


def authenticate(approval, saved, subject, representative_approval):
    require(isinstance(approval, dict) and set(approval) == APPROVAL_KEYS, 'exact four approval fields required')
    require(approval['approved_by'] == MAINTAINER, 'designated maintainer approval required')
    require(approval['approval_subject_sha256'] == subject, 'new production subject mismatch')
    day = approval['approved_at']
    require(isinstance(day, str) and len(day) == 10 and date.fromisoformat(day).isoformat() == day, 'invalid approval date')
    url = approval['approval_url']
    require(isinstance(url, str), 'invalid approval URL')
    match = re.fullmatch(r'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-([1-9][0-9]*)', url)
    require(match is not None, 'actual issue #85 comment URL required')
    require(saved['user']['login'] == MAINTAINER, 'saved comment is not by designated maintainer')
    require(saved['id'] == int(match[1]) and saved['html_url'] == url and saved['created_at'][:10] == day, 'saved comment identity/date/URL mismatch')
    affirmative_decision(saved['body'], subject)
    remote = fetch_comment(url)
    require(remote['user']['login'] == MAINTAINER, 'genuine comment is not by designated maintainer')
    affirmative_decision(remote['body'], subject)
    stable = lambda v: {k: v[k] for k in ['id', 'html_url', 'created_at', 'body']} | {'login': v['user']['login']}
    require(stable(remote) == stable(saved), 'saved comment differs from genuine decision')
    require(representative_approval == dict(approval, approval_subject_sha256=V9_SUBJECT), 'V9 approval differs from exact same decision')


def authenticate_application(subject):
    directory = ROOT / APPLICATION
    status = read(directory / 'status.json')
    require(status.get('state') == 'applied' and status.get('production_subject_sha256') == subject
            and status.get('representative_subject_sha256') == V9_SUBJECT, 'application not successfully applied for these subjects')
    authenticate(read(directory / 'approval.json'), read(directory / 'comment.json'), subject,
                 read(ROOT / V9 / 'manifest.json')['approval'])


def evidence(applied):
    q = load_frozen('qualify.py')
    split = load_frozen('split-verification.py')
    if not applied:
        split.verify_split()
        q.verify(False)
        return
    # Full original evidence verification, retaining exactly its two designed
    # metadata exclusions. Do not invoke the obsolete permissive authorization.
    manifest = q.read(BASE / 'manifest.json'); bundle = q.read(BASE / 'approval-subject.json')
    require(manifest['approval'] is None, 'v1 packet changed')
    require(bundle == {'schema': 'excalidraw-render/conformance-approval-subject/v1',
                      'manifest': {k: v for k, v in manifest.items() if k != 'approval'}, 'files': manifest['files']}, 'v1 envelope differs')
    for d in manifest['files']: check(d)
    frozen = q.read(BASE / 'final-freeze.json')
    for path, expected in frozen['producer_files'].items():
        if path not in [q.LEDGER, q.REGISTRY]: require(sha((ROOT / path).read_bytes()) == expected, 'frozen source changed ' + path)
    a = split.read(BASE / 'final-checks/parent-split-verification.json')
    raw = split.check_descriptor(a['initial']['raw_log'])
    split.validate_attestation(a, split.parse_workspace(raw))
    for d in [a['parent_message'], a['frozen_source_record'], a['test_source'], *a['producer_sources']]: check(d)
    require(a['producer_sources'] == read(ROOT / V9 / 'native-capture.json')['producer_sources'], 'split producer set differs')
    require(a['test_source']['path'] == q.SOURCE, 'split test source differs')
    for d in a['observed_pending_gate_errors']:
        data = split.check_descriptor(d).decode()
        require(('approval must be an object' if d['path'].endswith('v9-pending.stderr') else 'stale') in data, 'historical pending-gate proof differs')
    q.check_final_findings()
    q.check_sampling_pins(q.read(BASE / 'final-checks/production-qualification.json'))
    q.collector().verify_snapshots(BASE / 'old/snapshots')
    q.collector().verify_snapshots(BASE / 'final-current/snapshots')
    require(sha(canonical(parse(q.task('ownership-catalog')))) == read(BASE / 'migration-plan.json')['catalog_sha256'], 'catalog criteria changed')
    q.verify_migration(True)
    require(q.task('candidate-approval-subject', V9).decode().strip() == V9_SUBJECT, 'V9 subject changed')


def verify(applied=False):
    subject = packet(applied)
    if applied: authenticate_application(subject)  # Strong authorization FIRST.
    evidence(applied)
    print(subject + (' affirmative maintainer decision and exact applied migration verified' if applied else ' pending; no approval or live migration'))
    print(V9_SUBJECT + ' unchanged')


if __name__ == '__main__':
    require(sys.argv[1:] in [['verify'], ['verify-applied']], 'usage: verify.py verify|verify-applied')
    verify(sys.argv[1] == 'verify-applied')
