#!/usr/bin/env python3
"""Archive/record the exact user-approved PDF transition; never grant approval.

The GitHub comment records the user's explicit decision. This script checks the
real API identity/body and the unchanged approved subject, archives before bytes,
and records the exact applied changes. It never edits protected pins itself.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
CASE = ROOT / 'tests/conformance/candidates/stage4-pdf-backend-requalification-v2'
SUBJECT = 'c15e6bab141ac8080bac099932e5561bfcbee4c000f4957fec71e87d5e573389'
COMMENT = '5748855374'
SPEC = ROOT / 'tests/conformance/pdf-backend-qualification/project-operations-structure.json'
TEST = ROOT / 'crates/excalidraw-render/tests/fast/pdf_backend_qualification.rs'
VERIFIER = ROOT / 'xtask/src/conformance_pdf_requalification.rs'
OLD_PDF = 'b053159dc6ea0649e6c0fa5840196ccb81db6b41b7eb69d0181392851017ee43'
NEW_PDF = '50180ecf1822d7d573ea78cc6375cd3f83af8ef2748422b7e9b3d52a1b7ba6dc'
OLD_STRUCTURE = 'b6cbc6049556ae87780a83e9c252f64193594bf37c9d6a2f18a80434b31cb7b9'
NEW_STRUCTURE = '442f0cb2580a4066d64bcb924eaa6e070252faca2fa2b37fa4f34c81ff600b1b'


def sha(data):
    return hashlib.sha256(data).hexdigest()


def descriptor(path):
    data = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'byte_length': len(data), 'sha256': sha(data)}


def put(path, data):
    with path.open('xb') as f:
        f.write(data)


def main():
    assert sys.argv[1:] in [['archive'], ['record']]
    subject = json.loads((CASE / 'approval-subject.json').read_bytes())
    canonical = json.dumps(subject, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
    assert sha(canonical) == SUBJECT
    comment = json.loads(subprocess.check_output([
        'gh', 'api', f'repos/sagikazarmark/excalidraw-render.orig/issues/comments/{COMMENT}']))
    assert comment['user']['login'] == 'sagikazarmark'
    assert comment['html_url'] == f'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-{COMMENT}'
    assert SUBJECT in comment['body'] and NEW_PDF in comment['body']
    assert 'not an autonomous approval decision by the coding agent' in comment['body']
    approval = {'approved_by': comment['user']['login'], 'approved_at': comment['created_at'][:10],
                'approval_url': comment['html_url'], 'approval_subject_sha256': f'sha256:{SUBJECT}'}
    if sys.argv[1] == 'archive':
        assert sha(SPEC.read_bytes()) == OLD_STRUCTURE
        assert descriptor(SPEC) == subject['protected_expectation']
        recorded = next(d for d in subject['harness_sources'] if d['path'] == str(VERIFIER.relative_to(ROOT)))
        assert descriptor(VERIFIER) == recorded
        assert sha(TEST.read_bytes()) == 'fedf835ed08f63fd930fe01b403b85d959752dce66c6547057753efa33e8ceb5'
        put(CASE / 'before-project-operations-structure.json', SPEC.read_bytes())
        put(CASE / 'before-pdf_backend_qualification.rs', TEST.read_bytes())
        put(CASE / 'before-conformance_pdf_requalification.rs', VERIFIER.read_bytes())
        put(CASE / 'user-approval-comment.json', (json.dumps(comment, indent=2) + '\n').encode())
    else:
        assert json.loads((CASE / 'approval.json').read_bytes()) == approval
        assert SPEC.read_bytes() == (CASE / 'proposed-structure.json').read_bytes()
        before = (CASE / 'before-pdf_backend_qualification.rs').read_bytes()
        expected = before.replace(OLD_PDF.encode(), NEW_PDF.encode()).replace(OLD_STRUCTURE.encode(), NEW_STRUCTURE.encode())
        assert TEST.read_bytes() == expected
        record = {'schema': 'excalidraw-render/approved-pdf-pin-transition/v1',
                  'approval_subject_sha256': f'sha256:{SUBJECT}', 'approval': approval,
                  'decision_origin': 'user-granted approval explicitly requested in this session; not agent self-approval',
                  'before_expectation': descriptor(CASE / 'before-project-operations-structure.json'),
                  'before_test': descriptor(CASE / 'before-pdf_backend_qualification.rs'),
                  'before_verifier': descriptor(CASE / 'before-conformance_pdf_requalification.rs'),
                  'after_expectation': descriptor(SPEC), 'after_test': descriptor(TEST),
                  'current_verifier': descriptor(VERIFIER),
                  'comment_snapshot': descriptor(CASE / 'user-approval-comment.json'),
                  'protected_scope': 'native project-operation canary, operation/inventory digests and derived expectation-file binding only',
                  'representative_baseline_and_global_approval_granted': False}
        put(CASE / 'applied-transition.json', (json.dumps(record, indent=2) + '\n').encode())
    print(json.dumps(approval, indent=2))


if __name__ == '__main__':
    main()
