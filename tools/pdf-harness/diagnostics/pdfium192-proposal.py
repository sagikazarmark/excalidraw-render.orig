#!/usr/bin/env python3
"""Prepare/verify an explicit PDFium profile proposal without changing live metadata."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
PACKET = ROOT/'tests/conformance/proposals/pdfium192-alpha-requalification-v1'
PROFILE = 'tests/conformance/viewer-profiles/canonical-pdf-pdfium-v1.json'
MANIFEST = 'tests/conformance/pdf-viewer-qualification/rectangle-outline/manifest.json'
TEST = 'tools/pdf-harness/pdfium/test_alpha.py'


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()


def sha(data):
    return hashlib.sha256(data).hexdigest()


def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as output:
        output.write(data)


def descriptor(path):
    return {'path': path, 'sha256': sha((ROOT/path).read_bytes())}


def refreshed(value, overlay):
    if isinstance(value, list):
        return [refreshed(v, overlay) for v in value]
    if not isinstance(value, dict):
        return value
    result = {k: refreshed(v, overlay) for k, v in value.items()}
    if 'path' in result and 'sha256' in result:
        path = result['path']
        data = overlay[path] if path in overlay else (ROOT/path).read_bytes()
        result['sha256'] = sha(data)
        if 'byte_length' in result:
            result['byte_length'] = len(data)
    return result


def prepare():
    PACKET.mkdir(exist_ok=False)
    paths = [PROFILE, PROFILE.replace('.json', '.sha256'), MANIFEST,
             MANIFEST.replace('.json', '.sha256'), 'tools/pdf-harness/pdfium/render.py',
             'tools/pdf-viewer-qualification.sh']
    for path in paths:
        # Exact committed preimages include the approved old adapter, not the
        # corrected working-tree bytes. No historical approval is transferred.
        old = subprocess.check_output(['git', 'show', f'HEAD:{path}'], cwd=ROOT)
        put(PACKET/'before'/path, old)
    profile = json.loads((ROOT/PROFILE).read_bytes())
    profile['settings']['color']['alpha'] = 'transparent initialization; PDFium BGRA bitmap is straight alpha; retain visible RGB and zero RGB at alpha zero; no unpremultiplication'
    profile['implementation']['sources'].append(descriptor(TEST))
    profile['qualification']['restrictions'].append('Independent vector-opacity and image-soft-mask controls verify straight RGBA at alpha 0,1,128,255; representation control verifies all 256 alpha values')
    profile = canonical(refreshed(profile, {}))+b'\n'
    overlay = {PROFILE: profile, PROFILE.replace('.json', '.sha256'): (sha(profile)+'\n').encode()}
    manifest = json.loads((ROOT/MANIFEST).read_bytes())
    manifest['approval'] = None
    manifest['implementation'].append(descriptor(TEST))
    manifest = refreshed(manifest, overlay)
    encoded = canonical(manifest)
    overlay[MANIFEST] = encoded+b'\n'
    overlay[MANIFEST.replace('.json', '.sha256')] = (sha(encoded)+'\n').encode()
    for path, data in overlay.items():
        put(PACKET/'pending'/path, data)
    subject_manifest = dict(manifest)
    subject_manifest.pop('approval')
    subject = 'sha256:'+sha(canonical({'manifest': subject_manifest,
                                     'schema': 'excalidraw-render/conformance-pdf-viewer-approval-subject/v1'}))
    sources = [descriptor(p) for p in ['tools/pdf-harness/pdfium/render.py', TEST,
                                     'tools/pdf-viewer-qualification.sh',
                                     'tools/pdf-harness/diagnostics/pdfium192-audit.py',
                                     'tools/pdf-harness/diagnostics/pdfium192-proposal.py',
                                     'tests/conformance/proposals/pdfium192-alpha-audit-v1/audit.json']]
    record = {'schema': 'pdfium192-profile-proposal/v1', 'approval': None,
              'profile_subject_sha256': subject, 'sources': sources,
              'pending': {p: sha(b) for p, b in overlay.items()},
              'scope': 'Correct PDFium alpha representation and bind independent controls. Same viewer binary, comparator, reference and calibration outputs. No renderer-case, representative, ownership or baseline migration.'}
    put(PACKET/'proposal.json', canonical(record)+b'\n')
    print(subject)


def verify():
    record = json.loads((PACKET/'proposal.json').read_bytes())
    for desc in record['sources']:
        assert descriptor(desc['path']) == desc, desc['path']
    overlay = {}
    for path, expected in record['pending'].items():
        overlay[path] = (PACKET/'pending'/path).read_bytes()
        assert sha(overlay[path]) == expected, path
    for path in [PROFILE, MANIFEST]:
        value = json.loads(overlay[path])
        assert canonical(refreshed(value, overlay))+b'\n' == overlay[path], path
    assert overlay[PROFILE.replace('.json', '.sha256')] == (sha(overlay[PROFILE])+'\n').encode()
    manifest = json.loads(overlay[MANIFEST])
    assert manifest['approval'] is None
    assert overlay[MANIFEST.replace('.json', '.sha256')] == (sha(canonical(manifest))+'\n').encode()
    manifest.pop('approval')
    subject = 'sha256:'+sha(canonical({'manifest': manifest,
                                     'schema': 'excalidraw-render/conformance-pdf-viewer-approval-subject/v1'}))
    assert subject == record['profile_subject_sha256']
    for path in record['pending']:
        assert (ROOT/path).read_bytes() == (PACKET/'before'/path).read_bytes(), f'live metadata moved: {path}'
    print(subject, 'pending; live metadata unchanged')


def stage(destination):
    verify()
    # Destination must already be a detached checkout at the same committed HEAD.
    destination = Path(destination).resolve(strict=True)
    assert destination.is_relative_to(Path('/tmp/opencode'))
    assert subprocess.check_output(['git', '-C', str(destination), 'rev-parse', 'HEAD']) == subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT)
    record = json.loads((PACKET/'proposal.json').read_bytes())
    for desc in record['sources']:
        path = destination/desc['path']
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes((ROOT/desc['path']).read_bytes())
    for relative in record['pending']:
        (destination/relative).write_bytes((PACKET/'pending'/relative).read_bytes())
    print('staged unapproved profile; compile and verify xtask inside', destination)


if __name__ == '__main__':
    if sys.argv[1:] == ['prepare']:
        prepare()
    elif sys.argv[1:] == ['verify']:
        verify()
    elif len(sys.argv) == 3 and sys.argv[1] == 'stage':
        stage(sys.argv[2])
    else:
        raise SystemExit('prepare | verify | stage DETACHED_CHECKOUT')
