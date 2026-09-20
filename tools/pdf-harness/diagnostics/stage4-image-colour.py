#!/usr/bin/env python3
"""Read-only inspection and diagnostic variants for the retained 2x2 failure.

Uses the unchanged pinned viewers/qpdf. Writes only a fresh output directory;
original PDFs, profiles and production sources are never edited.
"""
import argparse
import base64
import copy
import hashlib
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[3]
CASE = ROOT / 'tests/conformance/candidates/stage4-pdf-backend-requalification-v1'
QPDF = ROOT / 'target/pdf-harness/qpdf/bin/qpdf'
SAMPLES = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 0, 255]]
POINTS = [(5, 5), (34, 5), (5, 34), (34, 34)]


def put(path, data):
    with path.open('xb') as f:
        f.write(data)


def put_json(path, value):
    put(path, (json.dumps(value, indent=2) + '\n').encode())


def sha(data):
    return hashlib.sha256(data).hexdigest()


def model(path):
    return json.loads(subprocess.check_output([
        str(QPDF), '--json=2', '--json-stream-data=inline',
        '--decode-level=generalized', str(path)]))


def inspect(data):
    objects = data['qpdf'][1]
    images = [(key, obj['stream']) for key, obj in objects.items()
              if obj.get('stream', {}).get('dict', {}).get('/Subtype') == '/Image']
    rgb_key, rgb = next((k, v) for k, v in images if '/SMask' in v['dict'])
    mask_key = 'obj:' + rgb['dict']['/SMask']
    mask = objects[mask_key]['stream']
    page = next(obj['value'] for obj in objects.values()
                if obj.get('value', {}).get('/Type') == '/Page')
    resources = page['/Resources']
    if isinstance(resources, str):
        resources = objects['obj:' + resources]['value']
    profile_ref = resources['/ColorSpace']['/CS1'][1]
    profile = objects['obj:' + profile_ref]['stream']
    rgb_bytes, alpha = base64.b64decode(rgb['data']), base64.b64decode(mask['data'])
    assert rgb_bytes == bytes(sum([p[:3] for p in SAMPLES], [])), rgb_bytes.hex()
    assert alpha == bytes([255] * 4), alpha.hex()
    assert rgb['dict']['/Width'] == rgb['dict']['/Height'] == 2
    assert mask['dict']['/Width'] == mask['dict']['/Height'] == 2
    assert rgb['dict']['/BitsPerComponent'] == mask['dict']['/BitsPerComponent'] == 8
    assert mask['dict']['/ColorSpace'] == '/DeviceGray'
    assert profile['dict']['/N'] == 3
    icc = base64.b64decode(profile['data'])
    assert icc[16:20] == b'RGB ' and icc[36:40] == b'acsp'
    return {'rgb_object': rgb_key, 'rgb_dictionary': rgb['dict'], 'rgb_hex': rgb_bytes.hex(),
            'mask_object': mask_key, 'mask_dictionary': mask['dict'], 'mask_hex': alpha.hex(),
            'used_colour_space': resources['/ColorSpace']['/CS1'],
            'icc_dictionary': profile['dict'], 'icc_sha256': sha(icc)}, rgb_key, mask_key


def render(path, viewer, output):
    runtime = json.loads(subprocess.check_output([
        'bash', f'tools/pdf-harness/run-{viewer}.sh', 'render', str(path), str(output), '40', '40'], cwd=ROOT))
    pixels = output.read_bytes()
    assert len(pixels) == 40 * 40 * 4
    return pixels, runtime


def observe(path, name, out, expected, viewers=('pdfium', 'pdfjs')):
    rows = []
    for viewer in viewers:
        first = None
        for run in range(2):
            pixels, runtime = render(path, viewer, out / f'{name}-{viewer}-{run}.rgba')
            put_json(out / f'{name}-{viewer}-{run}.runtime.json', runtime)
            if first is None:
                first = pixels
            else:
                assert first == pixels, (name, viewer, 'repeat differs')
        corners = [list(first[(y * 40 + x) * 4:(y * 40 + x) * 4 + 4]) for x, y in POINTS]
        rows.append({'name': name, 'viewer': viewer, 'corners': corners,
                     'expected_corners': expected, 'source_colour_passed': corners == expected,
                     'rgba_sha256': sha(first), 'fresh_runs': 2})
    return rows


def variants(data, out, names=None):
    _, rgb_key, mask_key = inspect(data)
    cases = []
    for name in names or ['roundtrip', 'no-mask', 'device-rgb', 'device-rgb-no-mask',
                          'interpolate-false', 'size-4', 'size-40', 'prime-rgb']:
        changed = model(CASE / 'orientation-approved.pdf') if name == 'old-direct-icc' else copy.deepcopy(data)
        objects = changed['qpdf'][1]
        rgb, mask = objects[rgb_key]['stream'], objects[mask_key]['stream']
        if name in ['no-mask', 'device-rgb-no-mask', 'direct-icc-no-mask']:
            del rgb['dict']['/SMask']
        if name in ['device-rgb', 'device-rgb-no-mask']:
            rgb['dict']['/ColorSpace'] = '/DeviceRGB'
        if name in ['direct-icc', 'direct-icc-no-mask', 'old-direct-icc']:
            # Resolve the *same* profile into the image dictionary. No profile,
            # RGB sample, alpha sample, transform or viewer option changes.
            page = next(o['value'] for o in objects.values() if o.get('value', {}).get('/Type') == '/Page')
            rgb['dict']['/ColorSpace'] = page['/Resources']['/ColorSpace']['/CS1']
        if name == 'interpolate-false':
            rgb['dict']['/Interpolate'] = mask['dict']['/Interpolate'] = False
        if name.startswith('size-'):
            size = int(name.split('-')[1])
            samples = [SAMPLES[(y >= size // 2) * 2 + (x >= size // 2)][:3]
                       for y in range(size) for x in range(size)]
            rgb['data'] = base64.b64encode(bytes(sum(samples, []))).decode()
            mask['data'] = base64.b64encode(bytes([255] * size * size)).decode()
            for plane in [rgb, mask]:
                plane['dict']['/Width'] = plane['dict']['/Height'] = size
        if name == 'prime-rgb':
            page = next(o['value'] for o in objects.values() if o.get('value', {}).get('/Type') == '/Page')
            content = objects['obj:' + page['/Contents']]['stream']
            content['data'] = base64.b64encode(b'/CS1 cs\n0 0 0 scn\n' + base64.b64decode(content['data'])).decode()
        model_path = out / f'{name}.qpdf.json'
        put_json(model_path, changed)
        pdf = out / f'{name}.pdf'
        subprocess.run([str(QPDF), '--json-input', str(model_path), str(pdf)], check=True, capture_output=True)
        subprocess.run([str(QPDF), '--check', str(pdf)], check=True, capture_output=True)
        expected = [SAMPLES[2], SAMPLES[3], SAMPLES[0], SAMPLES[1]] if name == 'old-direct-icc' else SAMPLES
        cases += observe(pdf, name, out, expected)
    return cases


def large_control(out):
    directory = ROOT / 'tests/conformance/candidates/stage4-frames-text-assets-v6'
    data = model(directory / 'native.pdf')
    objects = data['qpdf'][1]
    rgb = next(o['stream'] for o in objects.values()
               if o.get('stream', {}).get('dict', {}).get('/Subtype') == '/Image'
               and '/SMask' in o['stream']['dict'])
    mask = objects['obj:' + rgb['dict']['/SMask']]['stream']
    colors, alpha = base64.b64decode(rgb['data']), base64.b64decode(mask['data'])
    assert rgb['dict']['/Width'] == rgb['dict']['/Height'] == 40
    assert len(colors) == 4800 and len(alpha) == 1600
    for y in range(40):
        for x in range(40):
            i = y * 40 + x
            a = 0 if (x + 2 * y) % 3 == 0 else 255 if y < 20 else 128
            assert alpha[i] == a
            assert colors[i*3:i*3+3] == (bytes([255, 0, 0]) if a else bytes(3))
    reference = (directory / 'viewer-evidence/official-chromium.rgba').read_bytes()
    rows = []
    for viewer in ['pdfium', 'pdfjs']:
        first = None
        for run in range(2):
            output = out / f'v6-{viewer}-{run}.rgba'
            runtime = json.loads(subprocess.check_output([
                'bash', f'tools/pdf-harness/run-{viewer}.sh', 'render',
                str(directory/'native.pdf'), str(output), '200', '120'], cwd=ROOT))
            put_json(out / f'v6-{viewer}-{run}.runtime.json', runtime)
            pixels = output.read_bytes()
            assert pixels == (directory / f'viewer-evidence/native-{viewer}.rgba').read_bytes()
            if first is not None:
                assert first == pixels
            first = pixels
        for y in range(38, 82):
            for x in range(178, 200):
                at = (y * 200 + x) * 4
                assert first[at:at+4] == reference[at:at+4]
        rows.append({'viewer': viewer, 'exact_reference_raster_crop': [178, 38, 22, 44],
                     'rgba_sha256': sha(first), 'fresh_runs': 2})
    return {'source_size': [40, 40], 'independent_alpha_samples': 1600,
            'observations': rows, 'role': 'additional larger control, not replacement of the failed 2x2'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--require-source-colours', action='store_true')
    parser.add_argument('--variants', action='store_true')
    parser.add_argument('--variant', action='append', choices=['direct-icc', 'direct-icc-no-mask', 'old-direct-icc'])
    parser.add_argument('--large-control', action='store_true')
    args = parser.parse_args()
    out = args.output.resolve()
    assert out.is_relative_to(ROOT)
    out.mkdir(parents=True, exist_ok=False)
    records, inspections = [], {}
    for name, file, expected in [
        ('old', 'orientation-approved.pdf', [SAMPLES[2], SAMPLES[3], SAMPLES[0], SAMPLES[1]]),
        ('current', 'orientation-candidate-diagnostic.pdf', SAMPLES),
    ]:
        path = CASE / file
        data = model(path)
        details, _, _ = inspect(data)
        inspections[name] = {'pdf_sha256': sha(path.read_bytes()), **details}
        put_json(out / f'{name}-qpdf.json', data)
        records += observe(path, name, out, expected)
    if args.variants or args.variant:
        records += variants(model(CASE / 'orientation-candidate-diagnostic.pdf'), out, args.variant)
    result = {'schema': 'excalidraw-render/stage4-image-colour-diagnosis/v1',
              'inspection': inspections, 'observations': records}
    if args.large_control:
        result['large_control'] = large_control(out)
    put_json(out / 'observations.json', result)
    print(json.dumps(result, indent=2))
    if args.require_source_colours and any(not r['source_colour_passed'] for r in records):
        raise SystemExit('RED: unchanged pinned PDF.js does not reproduce the source colours')


if __name__ == '__main__':
    main()
