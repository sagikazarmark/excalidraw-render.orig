#!/usr/bin/env python3
"""Package measured diagnosis and a narrow, unapproved pin-maintenance proposal."""
import hashlib
import json
import os
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / 'tests/conformance/candidates/stage4-image-colour-diagnosis-v1'
PROTECTED = ROOT / 'tests/conformance/candidates/stage4-pdf-backend-requalification-v1'


def descriptor(path):
    data = path.read_bytes()
    return {'path': str(path.relative_to(ROOT)), 'byte_length': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


def put(path, value):
    data = (json.dumps(value, indent=2) + '\n').encode()
    with path.open('xb') as stream:
        stream.write(data)


def load(path):
    return json.loads(path.read_bytes())


def main():
    OUT.mkdir()
    stages = ['stage4-image-colour-repro', 'stage4-image-colour-variants',
              'stage4-image-colour-icc-proof', 'stage4-image-colour-large-control']
    all_rows = []
    for name in stages:
        source = ROOT / 'target' / name
        destination = OUT / name
        destination.mkdir()
        for file in sorted(source.iterdir()):
            if file.is_file():
                with (destination / file.name).open('xb') as stream:
                    stream.write(file.read_bytes())
        all_rows += load(source / 'observations.json')['observations']
    for row in all_rows:
        expected_pass = row['viewer'] == 'pdfium' or row['name'] in {
            'device-rgb', 'device-rgb-no-mask', 'prime-rgb',
            'direct-icc', 'direct-icc-no-mask', 'old-direct-icc'}
        assert row['source_colour_passed'] == expected_pass, row
        assert row['fresh_runs'] == 2
    large = load(ROOT / 'target/stage4-image-colour-large-control/observations.json')['large_control']
    assert large['independent_alpha_samples'] == 1600
    qpdf = ROOT / 'target/pdf-harness/qpdf/bin/qpdf'
    v6 = json.loads(subprocess.check_output([str(qpdf), '--json=2', '--json-stream-data=inline', '--decode-level=generalized',
                                           str(ROOT/'tests/conformance/candidates/stage4-frames-text-assets-v6/native.pdf')]))
    objects = v6['qpdf'][1]
    page = next(o['value'] for o in objects.values() if o.get('value', {}).get('/Type') == '/Page')
    import base64
    content = base64.b64decode(objects['obj:' + page['/Contents']]['stream']['data'])
    assert 0 <= content.find(b'/CS1 ') < content.find(b'/Im0 Do')
    large['colour_selection_precedes_image'] = True
    pre_canvas = {}
    for kind, path in [
        ('alias', 'tests/conformance/candidates/stage4-pdf-backend-requalification-v1/orientation-candidate-diagnostic.pdf'),
        ('direct_icc', 'target/stage4-image-colour-icc-proof/direct-icc.pdf'),
    ]:
        decoded = json.loads(subprocess.check_output([
            'docker', 'run', '--rm', '--init', '--network', 'none', '--platform', 'linux/amd64',
            '--user', f'{os.getuid()}:{os.getgid()}', '--env', 'HOME=/tmp',
            '--mount', f'type=bind,src={ROOT},dst=/workspace,readonly', '--workdir', '/workspace',
            'mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9',
            'node', 'tools/pdf-harness/diagnostics/stage4-image-decoded.mjs', path]))
        assert len(decoded['images']) == 1
        pre_canvas[kind] = decoded
    rgb = bytes.fromhex(load(ROOT/'target/stage4-image-colour-repro/observations.json')['inspection']['current']['rgb_hex'])
    predicted_gray = sum([[sample, sample, sample, 255] for sample in rgb[:4]], [])
    assert pre_canvas['alias']['images'][0]['data'] == predicted_gray
    assert pre_canvas['direct_icc']['images'][0]['data'] == [255,0,0,255,0,255,0,255,0,0,255,255,255,255,0,255]
    bindings = [descriptor(ROOT / p) for p in [
        'tools/pdf-harness/diagnostics/stage4-image-colour.py',
        'tools/pdf-harness/diagnostics/stage4-image-decoded.mjs',
        'tools/pdf-harness/diagnostics/stage4-image-colour-package.py',
        'tools/pdf-harness/pdfjs/node_modules/pdfjs-dist/legacy/build/pdf.worker.mjs',
        'crates/excalidraw-render/src/render/pdf.rs',
        'tools/pdf-harness/pdfjs/package-lock.json', 'tools/pdf-harness/pdfjs/render.mjs',
        'tools/pdf-harness/pdfium/render.py', 'tools/pdf-harness/pdfium.lock.json',
        'tests/conformance/viewer-profiles/canonical-pdf-pdfjs-v1.json',
        'tests/conformance/viewer-profiles/canonical-pdf-pdfium-v1.json',
        'tests/conformance/candidates/stage4-frames-text-assets-v6/native.pdf',
        'tests/conformance/candidates/stage4-pdf-backend-requalification-v1/blocked-proposal.json']]
    record = {'schema': 'excalidraw-render/pdf-image-colour-cause/v1',
              'classification': 'pre-existing producer defect: named resource alias outside a content stream',
              'cause': 'Image XObjects serialize /ColorSpace /CS1. Unprimed PDF.js non-inline image parsing has resources=null and falls back to DeviceGray; priming /CS1 hides it in the cache. The RGB bytes, SMask and ICC profile are correct.',
              'standard': {'url': 'https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf',
                           'sha256': '9de0ca9e8570d6209e8bd48a355be8eb6ec376acfc3fc3ae97cd8730351417ff',
                           'section': '7.8.3', 'printed_page': 82, 'physical_page': 90},
              'observations': all_rows, 'pre_canvas': pre_canvas, 'larger_control': large,
              'bindings': bindings, 'approval': None}
    put(OUT / 'evidence.json', record)
    blocked = load(PROTECTED / 'blocked-proposal.json')
    assert blocked['old_structure_passed'] is True
    assert blocked['current_against_old_passed'] is False
    assert blocked['current_against_proposed_passed'] is True
    assert len(blocked['structural_differences']) == 6
    for viewer in blocked['protected_scene_viewers']:
        assert viewer['protected_scene_byte_identical'] is True
        assert viewer['source_roi']['passed'] is True
    proposal = {'schema': 'excalidraw-render/narrow-pdf-pin-maintenance-proposal/v1',
                'status': 'parent-review-required-known-preexisting-producer-defect',
                'scope': 'Only the protected project-operation artifact and its operation/inventory hash pins for the image-Y/interpolation correction; not general PDF conformance or image colour-space approval.',
                'approval': None,
                'old_pdf_sha256': blocked['approved_pdf']['sha256'],
                'new_pdf_sha256': blocked['candidate_pdf']['sha256'],
                'protected_evidence': descriptor(PROTECTED / 'blocked-proposal.json'),
                'proposed_structure': descriptor(PROTECTED / 'proposed-structure.json'),
                'diagnosis': descriptor(OUT / 'evidence.json'),
                'remaining_project_gates': blocked['protected_scene_viewers'],
                'original_2x2_colour_gate_passed': False,
                'required_followup': 'Fix external image ColorSpace to refer directly to the ICCBased array/profile, and add image-only no-priming regressions in both pinned viewers. This remains a renderer defect, not a tolerance exception.',
                'decision': 'The protected delta is nonregressing and supports narrowly scoped pin maintenance after explicit parent review. It does not close the producer defect or authorize a full PDF qualification claim. Fixing the producer before final capture avoids another artifact/proof rebind.'}
    put(OUT / 'narrow-pin-proposal.json', proposal)
    print(json.dumps({'diagnosis': descriptor(OUT / 'evidence.json'),
                      'narrow_proposal': descriptor(OUT / 'narrow-pin-proposal.json'),
                      'original_2x2_colour_gate_passed': False, 'approval': None}, indent=2))


if __name__ == '__main__':
    main()
