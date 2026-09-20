#!/usr/bin/env python3
"""Reproduce the bound v4 typography diagnosis, never a candidate approval gate.

Usage and interpretation: docs/research/stage4-pdf-typography-diagnosis.md.
Only the new --output directory is written. No renderer build or network is
needed except optional --verify-sources. Supply the exact pinned PDFium wheel.
"""
import argparse
import base64
import ctypes
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import re
import struct
import subprocess
import sys
import urllib.request
import xml.etree.ElementTree as ET
import zipfile
import zlib

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
BASE = ROOT / 'tests/conformance/candidates/stage4-frames-text-assets-v4'
RECORD = ROOT / 'docs/research/stage4-pdf-typography-evidence.json'
IMAGE = 'mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9'
PATTERN = rb'1 0 0 -1 ([\d.]+) ([\d.]+) Tm\n<([0-9A-F]+)> Tj'


def digest(data):
    return {'byte_length': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


def checked(data, expected):
    assert digest(data) == {k: expected[k] for k in ('byte_length', 'sha256')}, expected


def stream(body):
    header, data = body.split(b'stream\n', 1)
    data = data[:int(re.search(rb'/Length (\d+)', header)[1])]
    return zlib.decompress(data) if b'/FlateDecode' in header else data


def rewrite(objects, content_id, content, root):
    # A diagnostic reserialization of the hash-bound, handwritten v4 PDF only.
    # Not a general PDF parser/writer. Preserve object bodies except page content.
    bodies = dict(objects)
    compressed = zlib.compress(content)
    bodies[content_id] = (b'<< /Filter /FlateDecode /Length '
                          + str(len(compressed)).encode() + b' >>\nstream\n'
                          + compressed + b'\nendstream')
    output = bytearray(b'%PDF-1.7\n')
    offsets = {}
    for i, body in sorted(bodies.items()):
        offsets[i] = len(output)
        output += f'{i} 0 obj\n'.encode() + body + b'\nendobj\n'
    xref = len(output)
    count = max(bodies) + 1
    output += f'xref\n0 {count}\n0000000000 65535 f \n'.encode()
    for i in range(1, count):
        output += (f'{offsets[i]:010d} 00000 n \n' if i in offsets
                   else '0000000000 00000 f \n').encode()
    output += (b'trailer\n<< /Size ' + str(count).encode() + b' /Root ' + root
               + b' >>\nstartxref\n' + str(xref).encode() + b'\n%%EOF\n')
    return bytes(output)


def font_geometry(font, glyph_map, matches):
    tables = {font[i:i+4].decode(): struct.unpack('>III', font[i+4:i+16])
              for i in range(12, 12 + 16 * int.from_bytes(font[4:6], 'big'), 16)}

    def table(name):
        _, offset, length = tables[name]
        return font[offset:offset+length]

    head, loca, glyf, hmtx = map(table, ('head', 'loca', 'glyf', 'hmtx'))
    upem = int.from_bytes(head[18:20], 'big')
    long_loca = int.from_bytes(head[50:52], 'big') == 1
    size = 4 if long_loca else 2
    assert upem == 1000 and not {'fpgm', 'prep', 'cvt '} & tables.keys()
    pen = 0.0
    rows = []
    for match in matches:
        code = int(match[3], 16)
        gid = int.from_bytes(glyph_map[code*2:code*2+2], 'big')
        offsets = [int.from_bytes(loca[g*size:(g+1)*size], 'big')
                   * (1 if long_loca else 2) for g in (gid, gid+1)]
        start, end = offsets
        contours = struct.unpack('>h', glyf[start:start+2])[0] if end > start else 0
        assert contours >= 0, 'this fixed diagnostic expects simple glyphs'
        instructions = (int.from_bytes(glyf[start+10+2*contours:start+12+2*contours], 'big')
                        if end > start else 0)
        assert instructions == 0
        advance = int.from_bytes(hmtx[gid*4:gid*4+2], 'big') * 20 / upem
        error = abs(float(match[1]) - pen)
        assert error <= 1.43e-14
        rows.append({'glyph_id': gid, 'advance': advance, 'advance_error': error,
                     'instruction_bytes': instructions})
        pen += advance
    return {'program': digest(font), 'units_per_em': upem, 'tables': list(tables), 'glyphs': rows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--wheel', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--verify-sources', action='store_true')
    args = parser.parse_args()
    assert sys.version_info[:3] == (3, 12, 3), 'pinned viewer Python is 3.12.3'
    record = json.loads(RECORD.read_bytes())
    for item in record['repository_bindings']:
        checked((ROOT / item['path']).read_bytes(), item)
    checked(args.wheel.read_bytes(), record['pdfium_wheel'])
    rustc_version = subprocess.check_output(['rustc', '--version'], text=True).strip()
    assert rustc_version.startswith('rustc 1.97.1 '), rustc_version
    out = args.output.resolve()
    assert (out.is_relative_to(ROOT / 'target') or out.is_relative_to(Path('/tmp/opencode')))
    out.mkdir(parents=True, exist_ok=False)
    if args.verify_sources:
        for source in record['upstream_sources']:
            with urllib.request.urlopen(source['url'] + '?format=TEXT', timeout=20) as response:
                checked(base64.b64decode(response.read()), source)
    with zipfile.ZipFile(args.wheel) as wheel:
        wheel.extractall(out / 'pdfium')
    sys.path.insert(0, str(out / 'pdfium'))
    import pypdfium2 as pdf
    import pypdfium2.raw as raw
    from pypdfium2.version import PDFIUM_INFO, PYPDFIUM_INFO
    assert str(PDFIUM_INFO) == '152.0.7947.0' and str(PYPDFIUM_INFO) == '5.12.1'
    spec = importlib.util.spec_from_file_location('viewer', ROOT / 'tools/pdf-harness/pdfium/render.py')
    harness = importlib.util.module_from_spec(spec)
    # Avoid creating __pycache__ beside the original harness.
    sys.dont_write_bytecode = True
    spec.loader.exec_module(harness)
    harness.configure_pdfium()
    data = (BASE / 'native.pdf').read_bytes()
    objects = {int(m[1]): m[2] for m in re.finditer(
        rb'(?m)^(\d+) 0 obj\s*\n(.*?)\nendobj', data, re.S)}
    assert len(objects) == int(re.search(rb'/Size (\d+)', data)[1]) - 1
    content = stream(objects[4])
    root = re.search(rb'/Root (\d+ 0 R)', data)[1]
    matches = list(re.finditer(PATTERN, content))
    assert len(matches) == 10
    svg_text = next(node for node in ET.fromstring((BASE / 'official.svg').read_bytes()).iter()
                    if node.tag == '{http://www.w3.org/2000/svg}text')
    assert svg_text.text == 'Stage four' and svg_text.attrib['font-size'] == '20px'
    assert b'/F2 20 Tf\n' in content
    assert all(abs(float(m[2]) - float(svg_text.attrib['y'])) < 4e-15 for m in matches)
    font = stream(objects[15])
    geometry = font_geometry(font, stream(objects[17]), matches)
    caption = next(e for e in json.loads((BASE / 'scene.json').read_bytes())['elements']
                   if e['id'] == 'caption')
    assert caption['text'] == 'Stage four' and caption['fontSize'] == 20
    a, b = math.cos(caption['angle']), math.sin(caption['angle'])
    cx, cy = caption['width'] / 2, caption['height'] / 2
    e = caption['x'] + cx - a*cx + b*cy
    f = caption['y'] + cy - b*cx - a*cy
    matrices = re.findall(rb'([-\d. ]+) cm\n', content)
    assert list(map(float, matrices[0].split())) == [.75, 0, 0, -.75, 0, 90]
    assert [a, b, -b, a, e, f] in [list(map(float, m.split())) for m in matrices]

    def render(document, name, flags=0):
        with pdf.PdfDocument(document) as doc:
            page = doc[0]
            bitmap = pdf.PdfBitmap.new_native(200, 120, format=raw.FPDFBitmap_BGRA, rev_byteorder=True)
            bitmap.fill_rect((0, 0, 0, 0), 0, 0, 200, 120)
            raw.FPDF_RenderPageBitmapWithMatrix(
                bitmap, page, raw.FS_MATRIX(96/72, 0, 0, 96/72, 0, 0),
                raw.FS_RECTF(0, 0, 200, 120), raw.FPDF_REVERSE_BYTE_ORDER | flags)
            rgba = harness.canonicalize(bitmap)
            bitmap.close()
            page.close()
        (out / (name + '.rgba')).write_bytes(rgba)
        return rgba

    def control(new_content, name):
        document = rewrite(objects, 4, new_content, root)
        (out / (name + '.pdf')).write_bytes(document)
        return render(document, name)

    baseline = render(data, 'stage4-original')
    assert baseline == (BASE / 'viewer-evidence/native-pdfium.rgba').read_bytes()
    for flag in ('FPDF_RENDER_NO_SMOOTHTEXT', 'FPDF_LCD_TEXT', 'FPDF_PRINTING', 'FPDF_RENDER_FORCEHALFTONE'):
        rgba = render(data, 'stage4-' + flag, getattr(raw, flag))
        assert (rgba == baseline) == (flag != 'FPDF_RENDER_NO_SMOOTHTEXT')

    def snap(match):
        x, y = float(match[1]), float(match[2])
        X, Y = a*x - b*y + e, b*x + a*y + f
        dx, dy = math.floor(X*4+.5)/4-X, math.floor(Y*4+.5)/4-Y
        return f'1 0 0 -1 {x+a*dx+b*dy:.12f} {y-b*dx+a*dy:.12f} Tm\n<{match[3].decode()}> Tj'.encode()

    control(re.sub(PATTERN, snap, content), 'stage4-quarter-text')
    single = re.sub(PATTERN, lambda m: m[0] if int(m[3], 16) == 1 else b'', content)
    shifts = []
    for shift in (0, .125, .25, .5):
        def offset(match):
            return f'1 0 0 -1 {float(match[1])+b*shift:.12f} {float(match[2])+a*shift:.12f} Tm\n<{match[3].decode()}> Tj'.encode()
        shifts.append(control(re.sub(PATTERN, offset, single), 'stage4-single-y' + str(shift)))
    differences = [sum(v[i:i+4] != shifts[0][i:i+4] for i in range(0, len(v), 4)) for v in shifts]
    assert differences == [0, 0, 0, 136]
    assert all(shifts[0][((y-1)*200+x)*4:((y-1)*200+x)*4+4]
               == shifts[3][(y*200+x)*4:(y*200+x)*4+4]
               for y in range(12, 53) for x in range(10, 134))
    run = b'BT\n/F2 20 Tf\n1 0 0 -1 0 17.62 Tm\n<000100020003000400050006000700080009000A> Tj\nET'
    assert control(re.sub(rb'BT\n.*?\nET', run, content, flags=re.S), 'stage4-single-run') == baseline
    control(content.replace(b'BT\n', b'BT\n7 Tr\n').replace(
        b'ET\n', b'ET\n-1000 -1000 2000 2000 re f\n'), 'stage4-text-clip')
    with pdf.PdfDocument(data) as doc:
        page = doc[0]
        text_objects = [o for o in page.get_objects() if o.type == raw.FPDF_PAGEOBJ_TEXT]
        assert len(text_objects) == len(matches)
        pdf_font = raw.FPDFTextObj_GetFont(text_objects[0])
        assert raw.FPDFFont_GetIsEmbedded(pdf_font) == 1
        origins = []
        outlines = []
        for obj, match in zip(text_objects, matches):
            x, y, code = float(match[1]), float(match[2]), int(match[3], 16)
            matrix = raw.FS_MATRIX()
            assert raw.FPDFPageObj_GetMatrix(obj, matrix)
            expected = [a*x-b*y+e, b*x+a*y+f]
            actual = [matrix.e/.75, 120-matrix.f/.75]
            assert max(abs(u-v) for u, v in zip(expected, actual)) < 6e-6
            origins.append({'expected': expected, 'parsed': actual})
            path = raw.FPDFFont_GetGlyphPath(pdf_font, ord('Stage four'[code-1]), 20)
            if not path:
                assert code == 6  # Space has no outline.
                continue
            ops, bezier = [], []
            for j in range(raw.FPDFGlyphPath_CountGlyphSegments(path)):
                segment = raw.FPDFGlyphPath_GetGlyphPathSegment(path, j)
                px, py = ctypes.c_float(), ctypes.c_float()
                assert raw.FPDFPathSegment_GetPoint(segment, px, py)
                point = f'{x+20*px.value:.9f} {y-20*py.value:.9f}'
                kind = raw.FPDFPathSegment_GetType(segment)
                if kind == raw.FPDF_SEGMENT_BEZIERTO:
                    bezier.append(point)
                    if len(bezier) == 3:
                        ops.append(' '.join(bezier) + ' c')
                        bezier = []
                else:
                    ops.append(point + (' m' if kind == raw.FPDF_SEGMENT_MOVETO else ' l'))
                if raw.FPDFPathSegment_GetClose(segment):
                    ops.append('h')
            assert not bezier
            outlines.append('\n'.join(ops) + '\nf')
        page.close()
    control(re.sub(rb'BT\n.*?\nET', '\n'.join(outlines).encode(), content, flags=re.S), 'stage4-outlines')

    for name, source in [('stage4-original-pdfjs', '/workspace/' + str((BASE / 'native.pdf').relative_to(ROOT))),
                         ('stage4-quarter-pdfjs', '/probe/stage4-quarter-text.pdf')]:
        subprocess.run(['docker', 'run', '--rm', '--init', '--network', 'none', '--platform', 'linux/amd64',
                        '--user', f'{os.getuid()}:{os.getgid()}', '--env', 'HOME=/tmp',
                        '--mount', f'type=bind,src={ROOT},dst=/workspace,readonly',
                        '--mount', f'type=bind,src={out},dst=/probe', '--workdir', '/workspace/tools/pdf-harness/pdfjs',
                        IMAGE, 'node', 'render.mjs', source, f'/probe/{name}.rgba', '200', '120'], check=True)
    assert (out / 'stage4-original-pdfjs.rgba').read_bytes() == (BASE / 'viewer-evidence/native-pdfjs.rgba').read_bytes()
    for artifact in record['controls']:
        checked((out / artifact['name']).read_bytes(), artifact)
    compiler = ['rustc', '--edition=2021', '-C', 'opt-level=2', '-C', 'debuginfo=0']
    library = out / 'libexcalidraw_render_compare.rlib'
    subprocess.run(compiler + ['--crate-type', 'lib', '--crate-name', 'excalidraw_render_compare',
                              str(ROOT / 'crates/excalidraw-render-compare/src/lib.rs'), '-o', str(library)], check=True)
    executable = out / 'compare'
    subprocess.run(compiler + ['--extern', f'excalidraw_render_compare={library}',
                              str(HERE / 'stage4-typography-compare.rs'), '-o', str(executable)], check=True)
    comparisons = []
    for expected in record['comparisons']:
        args = [str(executable), str(BASE / 'viewer-evidence/official-chromium.rgba'),
                str(out / expected['name']), expected['viewer']]
        # Exit 1 is the canonical failure; it must remain red for the original PDF.
        result = subprocess.run(args, text=True, capture_output=True)
        assert result.returncode in (0, 1), result.stderr
        rows = json.loads(result.stdout)
        assert rows == expected['regions'], (expected, rows)
        assert result.returncode == (0 if all(r['passed'] for r in rows) else 1)
        comparisons.append({'name': expected['name'], 'viewer': expected['viewer'], 'regions': rows})
    observations = {'status': 'diagnosis-reproduced-not-approval', 'rustc': rustc_version,
                    'python': sys.version, 'zlib': zlib.ZLIB_RUNTIME_VERSION,
                    'font_geometry': geometry, 'origins': origins, 'single_y_changed_pixels': differences,
                    'controls': record['controls'], 'comparisons': comparisons}
    (out / 'observations.json').write_text(json.dumps(observations, indent=2) + '\n')
    print(f'PASS: exact controls/metrics and geometry reproduced; baseline gates remain failed. {out}/observations.json')


if __name__ == '__main__':
    main()
