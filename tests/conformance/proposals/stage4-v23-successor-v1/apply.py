#!/usr/bin/env python3
"""Apply an actual approval of the V23 successor and the PDF judgment record:
record both approvals, move the representative registry entry from V22 to
V23 and rebind the representative owner. Nothing else."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
P = Path(__file__).resolve().parent
V23 = 'tests/conformance/candidates/stage4-frames-text-assets-v23'
CASE = 'representative.stage4-frames-text-assets.v23'
PRIOR = 'representative.stage4-frames-text-assets.v22'
LEDGER = 'tests/conformance/coverage/ownership.json'
REGISTRY = 'tests/conformance/candidates/registry.json'
PDF_JUDGMENTS_SUBJECT = 'sha256:f0b9119f08e53d2d51873960722d4ab15619f6e2510291aa0389e6ad2f390f4a'


def pretty(value):
    return (json.dumps(value, indent=2, ensure_ascii=False) + '\n').encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def apply(approved_by, approved_at, approval_url):
    subject = (P/'logs/subject.stdout').read_text().strip()
    manifest_path = ROOT/V23/'manifest.json'
    raw = manifest_path.read_bytes()
    manifest = json.loads(raw)
    assert manifest['case_id'] == CASE and manifest['approval'] is None
    assert approval_url.startswith('https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-')
    manifest['approval'] = {'approval_subject_sha256': subject, 'approval_url': approval_url,
                            'approved_at': approved_at, 'approved_by': approved_by}
    # Candidate manifests are compact, sorted canonical JSON.
    manifest_path.write_bytes(json.dumps(manifest, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
                              + (b'\n' if raw.endswith(b'\n') else b''))

    registry = json.loads((ROOT/REGISTRY).read_bytes())
    [entry] = registry['cases']
    assert entry['id'] == PRIOR
    command = ['cargo', 'run', '--release', '--locked', '-p', 'xtask', '--', 'conformance', 'candidate-verify', V23]
    entry.update(id=CASE, directory=V23, status='stage4-v23-source-binding-successor',
                 exceptions=manifest['exception_proposal']['exceptions'],
                 execution_keys=[f'{CASE}:default:{fmt}' for fmt in ['svg', 'png', 'pdf']], consumer=command)
    (ROOT/REGISTRY).write_bytes(pretty(registry))

    ledger = json.loads((ROOT/LEDGER).read_bytes())
    [row] = [r for r in ledger['assignments'] if r['id'] == 'interaction:frames-text-assets']
    owner = row['disposition']['owner']
    assert owner['kind'] == 'representative' and owner['binding']['case'] == PRIOR
    owner['binding'] = {'case': CASE, 'registry': {'path': REGISTRY, 'sha256': sha((ROOT/REGISTRY).read_bytes())},
                        'manifest': {'path': V23 + '/manifest.json', 'sha256': sha(manifest_path.read_bytes())},
                        'approval_subject_sha256': subject, 'command': command}
    (ROOT/LEDGER).write_bytes(pretty(ledger))

    # The approved PDF judgment record.
    subprocess.run(['cargo', 'run', '-q', '--locked', '--release', '-p', 'xtask', '--', 'conformance',
                    'record-pdf-judgments-approval', approved_by, approved_at, approval_url, PDF_JUDGMENTS_SUBJECT],
                   cwd=ROOT, check=True)

if __name__ == '__main__':
    apply(*sys.argv[1:])
