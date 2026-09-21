#!/usr/bin/env python3
"""Diagnose straight bitmap output before the unchanged canonical PDFium adapter."""
import base64
import importlib.util
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT/'target/pdf-harness/pdfium'))
spec = importlib.util.spec_from_file_location('adapter', ROOT/'tools/pdf-harness/pdfium/render.py')
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)
spec = importlib.util.spec_from_file_location('diagnostic', Path(__file__).with_name('webp191-partial-viewers.py'))
diagnostic = importlib.util.module_from_spec(spec)
spec.loader.exec_module(diagnostic)


def main():
    folder = (ROOT/sys.argv[1]).resolve()
    folder.relative_to(ROOT)
    reference = json.loads((ROOT/'tests/oracle/webp-sample/reference.json').read_bytes())
    inputs = json.loads((ROOT/'tests/oracle/webp-sample/inputs.json').read_bytes())
    frames = {r['id']: r for r in inputs['frames']}
    adapter.configure_pdfium()
    cases = []
    for row in reference['cases']:
        with adapter.pypdfium2.PdfDocument(folder/row['id']/'image.pdf') as doc:
            page = doc[0]
            bitmap = adapter.render_at_exact_scale(page, 9, 7)
            try:
                assert bitmap.mode == 'RGBA' and bitmap.stride == 36
                raw = bytes(bitmap.buffer)
                adjusted = adapter.canonicalize(bitmap)
            finally:
                bitmap.close()
                page.close()
        assert adjusted == (folder/row['id']/'pdfium-0.rgba').read_bytes()
        diagnostic.write(folder/row['id']/'pdfium-raw.rgba', raw)
        frame = frames[row['first_frame_id']]
        straight = base64.b64decode(frame['native_decoded_rgba']['base64'])
        canvas = bytearray(252)
        sub = row['config']['shape'] == 'subrect'
        width, height, x, y = (5, 3, 2, 2) if sub else (9, 7, 0, 0)
        for j in range(height):
            for i in range(width):
                p = straight[(j*width+i)*4:(j*width+i+1)*4]
                if p[3] == 0:
                    p = b'\0\0\0\0'
                offset = ((j+y)*9+i+x)*4
                canvas[offset:offset+4] = p
        # Both codec families must match the independently decoded straight
        # source exactly; an opaque-only control cannot distinguish this bug.
        assert raw == canvas, row['id']
        views = {}
        for bg, value in [('black', 0), ('white', 255)]:
            observed = base64.b64decode(row['reference']['samples']['intrinsic']['composites'][bg]['rgba_base64'])
            views[bg] = diagnostic.diff(diagnostic.project(raw, value), observed)
        cases.append({'id': row['id'], 'raw_sha256': diagnostic.digest(raw),
                      'adapter_sha256': diagnostic.digest(adjusted),
                      'equals_external_straight': True, 'composites': views})
    result = {'kind': 'pdfium-raw-diagnosis-not-a-qualified-adapter',
              'pdfium': str(adapter.PDFIUM_INFO), 'pypdfium2': str(adapter.PYPDFIUM_INFO),
              'adapter_sha256': diagnostic.digest((ROOT/'tools/pdf-harness/pdfium/render.py').read_bytes()),
              'cases': cases}
    diagnostic.write(folder/'pdfium-raw-measurements.json', (json.dumps(result, indent=2, sort_keys=True)+'\n').encode())
    print('72 raw bitmaps equal external straight samples; adapter unchanged, correction requires requalification')


if __name__ == '__main__':
    main()
