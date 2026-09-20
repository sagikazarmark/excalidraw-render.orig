#!/usr/bin/env python3
"""Audit pending/applied evidence; apply only an explicitly approved metadata transaction."""
import sys

# This must precede imports that inspect the workspace and all workflow dispatch.
# The immutable predecessor verifier uses asserts; optimized execution is forbidden.
if not __debug__ or sys.flags.optimize:
    sys.stderr.write("Optimized Python execution is forbidden; no workflow started.\n")
    raise SystemExit(2)

import contextlib
from datetime import date, datetime, timezone
import hashlib
import io
import json
import os
from pathlib import Path
import re
import subprocess
import types
import uuid

ROOT = Path(__file__).resolve().parents[4]
PACKET = Path(__file__).resolve().parent
BASE = ROOT / 'tests/conformance/proposals/stage4-format-requalification-v1'
BASE_SUBJECT = 'sha256:4befa87e77a521aae20223b78fa6da20bb3a38d8555800f844e9de5c4d3f4e52'
V8 = 'tests/conformance/candidates/stage4-frames-text-assets-v8'
V8_SUBJECT = 'sha256:aaa772430f342a60807ef97255bf2296041cbcf23dff4c2aa4284052deb92454'
REGISTRY = 'tests/conformance/candidates/registry.json'
LEDGER = 'tests/conformance/coverage/ownership.json'
APPLICATION = 'tests/conformance/coverage/stage4-format-application-v2'
FIELDS = {'approved_by', 'approved_at', 'approval_url', 'approval_subject_sha256'}
_legacy = None


def require(condition, message):
    if not condition:
        raise ValueError(message)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()


def pretty(value):
    return json.dumps(value, indent=2, ensure_ascii=False).encode() + b'\n'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read_json(path):
    try:
        return json.loads(path.read_bytes())
    except (OSError, ValueError) as error:
        raise ValueError(f'missing or invalid {path.name}: {error}') from error


def descriptor(path):
    data = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'byte_length': len(data), 'sha256': digest(data)}


def check_descriptor(record):
    require(descriptor(ROOT / record['path']) == record, f'stale descriptor: {record["path"]}')


def predecessor():
    global _legacy
    bundle = read_json(BASE / 'approval-subject.json')
    require('sha256:' + digest(canonical(bundle)) == BASE_SUBJECT, 'predecessor subject changed')
    source = next(d for d in bundle['files'] if d['path'] == 'requalify.py')
    data = (BASE / 'requalify.py').read_bytes()
    require(len(data) == source['byte_length'] and digest(data) == source['sha256'], 'predecessor checker changed')
    if _legacy is None:
        # Loading authenticated source this way creates no __pycache__ in v1.
        module = types.ModuleType('immutable_formatting_review_v1')
        module.__file__ = str(BASE / 'requalify.py')
        exec(compile(data, module.__file__, 'exec', dont_inherit=True, optimize=0), module.__dict__)
        _legacy = module
    return _legacy


def verify_packet():
    manifest = read_json(PACKET / 'manifest.json')
    bundle = read_json(PACKET / 'approval-subject.json')
    require(manifest['approval'] is None, 'review packet must remain immutable with null approval')
    expected = {'schema': 'excalidraw-render/conformance-approval-subject/v1',
                'manifest': {k: v for k, v in manifest.items() if k != 'approval'}, 'files': manifest['files']}
    require(bundle == expected, 'successor envelope differs from manifest')
    require(manifest['predecessor_subject_sha256'] == BASE_SUBJECT, 'wrong predecessor subject')
    require(manifest['representative_subject_sha256'] == V8_SUBJECT, 'V8 subject changed')
    for row in manifest['files'] + manifest['predecessor_files']:
        check_descriptor(row)
    predecessor()
    return 'sha256:' + digest(canonical(bundle))


def evidence_verification(applied):
    # The source/format/output/57-owner/one-representative validation is reused
    # verbatim. Authorization of this successor is checked separately below.
    with contextlib.redirect_stdout(io.StringIO()):
        predecessor().verify(applied)


def verify_pending():
    subject = verify_packet()
    evidence_verification(False)
    print(f'Verified pending formatting subject {subject}; {V8_SUBJECT}; no new approval or live migration.')
    return subject


def comment_id(url):
    require(isinstance(url, str), 'approval URL must be a string')
    match = re.fullmatch(r'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-([1-9][0-9]*)', url)
    require(match is not None, 'approval URL must name an actual issue #85 comment')
    return match[1]


def fetch_comment(url):
    number = comment_id(url)
    result = subprocess.run(['gh', 'api', f'repos/sagikazarmark/excalidraw-render.orig/issues/comments/{number}'],
                            cwd=ROOT, capture_output=True, check=True)
    return json.loads(result.stdout)


def stable_comment(comment):
    return {key: comment[key] for key in ['id', 'html_url', 'created_at', 'body']} | {'login': comment['user']['login']}


def authenticate_decision(approval, saved_comment, formatting_subject, v8_approval=None):
    require(isinstance(approval, dict) and set(approval) == FIELDS, 'formatting approval must have exactly four fields')
    require(approval['approval_subject_sha256'] == formatting_subject, 'formatting approval subject mismatch')
    login = approval['approved_by']
    require(isinstance(login, str) and re.fullmatch(r'[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?', login), 'invalid approval login')
    day = approval['approved_at']
    require(isinstance(day, str) and len(day) == 10 and date.fromisoformat(day).isoformat() == day, 'invalid approval date')
    number = comment_id(approval['approval_url'])
    require(saved_comment['id'] == int(number), 'comment ID does not match approval URL')
    require(saved_comment['html_url'] == approval['approval_url'], 'comment URL mismatch')
    require(saved_comment['user']['login'] == login, 'comment author mismatch')
    require(saved_comment['created_at'][:10] == day, 'comment date mismatch')
    body = saved_comment['body']
    require(isinstance(body, str), 'comment body must be a string')
    for subject in [formatting_subject, V8_SUBJECT]:
        require(re.search(re.escape(subject) + r'(?![0-9a-fA-F])', body) is not None, 'decision comment must name both exact subjects')
    remote = fetch_comment(approval['approval_url'])
    require(stable_comment(remote) == stable_comment(saved_comment), 'saved decision does not match genuine GitHub comment')
    if v8_approval is not None:
        require(v8_approval == dict(approval, approval_subject_sha256=V8_SUBJECT), 'V8 approval differs from formatting decision')
    return remote


def application_path():
    return ROOT / APPLICATION


def applied_audit(formatting_subject, allow_validating=False):
    directory = application_path()
    status = read_json(directory / 'status.json')
    permitted = {'applied', 'validating'} if allow_validating else {'applied'}
    require(status.get('state') in permitted, 'application is not successfully applied')
    require(status.get('formatting_subject_sha256') == formatting_subject and status.get('representative_subject_sha256') == V8_SUBJECT,
            'application status subject mismatch')
    # These records are mandatory even if v8 has a syntactically valid approval.
    approval = read_json(directory / 'approval.json')
    comment = read_json(directory / 'comment.json')
    v8 = read_json(ROOT / V8 / 'manifest.json')
    require(v8.get('approval') is not None, 'V8 approval is missing')
    authenticate_decision(approval, comment, formatting_subject, v8['approval'])
    evidence_verification(True)


def verify_applied():
    subject = verify_packet()
    applied_audit(subject)
    print(f'Authenticated applied formatting decision {subject}; exact V8/57-binding transaction verified.')


def write_new(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as output:
        output.write(data)
        output.flush()
        os.fsync(output.fileno())


def write_status(directory, status):
    temporary = directory / ('status.tmp-' + uuid.uuid4().hex)
    write_new(temporary, pretty(status))
    temporary.replace(directory / 'status.json')


def write_live(path, data):
    path.write_bytes(data)


def live_transaction(approval):
    legacy = predecessor()
    manifest = read_json(ROOT / V8 / 'manifest.json')
    require(manifest['approval'] is None, 'V8 is already approved')
    manifest['approval'] = dict(approval, approval_subject_sha256=V8_SUBJECT)
    manifest_bytes = canonical(manifest) + b'\n'
    registry_bytes = (BASE / 'pending/registry.json').read_bytes()
    ledger = read_json(BASE / 'pending/ownership.template-not-live.json')
    selected = next(r for r in ledger['assignments'] if r['id'] == 'interaction:frames-text-assets')
    selected['disposition']['owner']['binding']['manifest']['sha256'] = digest(manifest_bytes)
    legacy.validate_migration(False)
    return {V8 + '/manifest.json': manifest_bytes, REGISTRY: registry_bytes, LEDGER: pretty(ledger)}


def prior_attempt():
    directory = application_path()
    if not directory.exists():
        return None
    status = read_json(directory / 'status.json')
    require(status.get('state') == 'failed_rolled_back',
            'existing application is applied or incomplete; manual inspection/recovery required')
    return status


def archive_rolled_back_attempt(status, subject, previous):
    directory = application_path()
    require(status['formatting_subject_sha256'] == subject and status['representative_subject_sha256'] == V8_SUBJECT,
            'failed attempt belongs to another decision')
    require(re.fullmatch(r'[0-9a-f]{32}', status['attempt_id']) is not None, 'invalid failed attempt ID')
    require(set(status['before_sha256']) == set(previous), 'failed attempt file set differs')
    for path, data in previous.items():
        require(digest(data) == status['before_sha256'][path], 'retry refused: live preimage differs from failed attempt')
        require((directory / 'before' / path).read_bytes() == data, 'retry refused: archived preimage differs')
    destination = directory.with_name(directory.name + '.failed-' + status['attempt_id'])
    require(not destination.exists(), 'failed-attempt archive already exists')
    directory.rename(destination)


def apply(approval_path):
    # Only invoke after explicit user approval. The CLI has no synthetic or
    # offline-authorization option; tests inject the GitHub boundary in memory.
    old_attempt = prior_attempt()
    subject = verify_pending()
    approval = read_json(Path(approval_path))
    require(isinstance(approval, dict) and set(approval) == FIELDS, 'formatting approval must have exactly four fields')
    comment = fetch_comment(approval['approval_url'])
    authenticate_decision(approval, comment, subject)
    files = live_transaction(approval)
    previous = {path: (ROOT / path).read_bytes() for path in files}
    if old_attempt is not None:
        archive_rolled_back_attempt(old_attempt, subject, previous)
    directory = application_path()
    directory.mkdir(parents=True, exist_ok=False)
    status = {'schema': 'excalidraw-render/format-application-attempt/v1', 'attempt_id': uuid.uuid4().hex,
              'state': 'preparing', 'formatting_subject_sha256': subject, 'representative_subject_sha256': V8_SUBJECT,
              'started_at': datetime.now(timezone.utc).isoformat(),
              'before_sha256': {p: digest(b) for p, b in previous.items()}}
    try:
        write_status(directory, status)
        for path, data in previous.items():
            write_new(directory / 'before' / path, data)
        # Persist both actual decision records BEFORE any live mutation/audit.
        write_new(directory / 'approval.json', pretty(approval))
        write_new(directory / 'comment.json', pretty(comment))
        status['state'] = 'applying'
        write_status(directory, status)
        for path, data in files.items():
            write_live(ROOT / path, data)
        legacy = predecessor()
        for args in [('candidate-verify', V8), ('candidate-executions',),
                     ('verify-representative-owner', 'representative.stage4-frames-text-assets.v8'),
                     ('verify-inventory',), ('verify-ownership',)]:
            write_new(directory / (args[0] + '.json'), legacy.xtask(*args))
        status['state'] = 'validating'
        write_status(directory, status)
        applied_audit(subject, allow_validating=True)
        status['state'] = 'applied'
        write_status(directory, status)
    except BaseException as error:
        failures = []
        for path, data in previous.items():
            try:
                write_live(ROOT / path, data)
                require((ROOT / path).read_bytes() == data, 'rollback byte mismatch')
            except BaseException as rollback_error:
                failures.append(f'{path}: {rollback_error}')
        status.update(state='rollback_failed' if failures else 'failed_rolled_back',
                      error=f'{type(error).__name__}: {error}', rollback_errors=failures)
        write_status(directory, status)
        if failures:
            raise RuntimeError('rollback incomplete; manual recovery required: ' + '; '.join(failures)) from error
        raise
    print(f'Applied authenticated formatting decision {subject}; decision records and exact transaction audited.')


if __name__ == '__main__':
    arguments = sys.argv[1:]
    if arguments == ['verify']:
        verify_pending()
    elif arguments == ['verify-applied']:
        verify_applied()
    elif len(arguments) == 2 and arguments[0] == 'apply':
        apply(arguments[1])
    else:
        raise SystemExit('usage: requalify.py verify | verify-applied | apply ACTUAL_APPROVAL_JSON')
