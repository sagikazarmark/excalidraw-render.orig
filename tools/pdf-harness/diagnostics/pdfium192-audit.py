#!/usr/bin/env python3
"""Create-only audit of every tracked PDF byte identity under tests/."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT/'target/pdf-harness/pdfium'))
spec = importlib.util.spec_from_file_location('adapter', ROOT/'tools/pdf-harness/pdfium/render.py')
adapter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(adapter)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def old_projection(raw):
    result = bytearray(raw)
    for i in range(0, len(result), 4):
        a = result[i+3]
        if a == 0:
            result[i:i+3] = bytes(3)
        elif a != 255:
            for c in range(3):
                result[i+c] = min(255, (result[i+c]*255+a//2)//a)
    return bytes(result)


def put(path, data):
    with path.open('xb') as output:
        output.write(data)


def main():
    destination = (ROOT/sys.argv[1]).resolve()
    destination.relative_to(ROOT)
    destination.mkdir(parents=True, exist_ok=False)
    tracked = subprocess.check_output(['git', 'ls-files', '-z', 'tests'], cwd=ROOT).decode().split('\0')
    pdfs, raster_hashes = {}, {}
    for name in filter(None, tracked):
        path = ROOT/name
        # Content-addressed snapshot blobs may contain PDFs without extensions.
        if path.suffix != '.pdf' and '/blobs/' not in name and path.suffix != '.rgba':
            continue
        data = path.read_bytes()
        if data.startswith(b'%PDF-'):
            pdfs.setdefault(sha(data), []).append(name)
        elif path.suffix == '.rgba':
            raster_hashes.setdefault(sha(data), []).append(name)
    adapter.configure_pdfium()
    rows = []
    for digest, paths in sorted(pdfs.items()):
        row = {'pdf_sha256': digest, 'paths': paths}
        try:
            with adapter.pypdfium2.PdfDocument(ROOT/paths[0]) as document:
                if len(document) != 1:
                    raise ValueError(f'{len(document)} pages outside canonical single-page scope')
                page = document[0]
                width = max(1, int(page.get_width()*adapter.SCALE + 0.5))
                height = max(1, int(page.get_height()*adapter.SCALE + 0.5))
                if width*height > 20_000_000:
                    raise ValueError('audit resource cap exceeded')
                bitmap = adapter.render_at_exact_scale(page, width, height)
                try:
                    raw = bytes(bitmap.buffer)
                    current = adapter.canonicalize(bitmap)
                finally:
                    bitmap.close()
                    page.close()
            previous = old_projection(raw)
            changed = sum(current[i:i+4] != previous[i:i+4] for i in range(0, len(raw), 4))
            row.update(width=width, height=height, changed_pixels=changed,
                       partial_alpha_pixels=sum(0 < a < 255 for a in raw[3::4]),
                       old_rgba_sha256=sha(previous), new_rgba_sha256=sha(current),
                       exact_historical_raster_matches=raster_hashes.get(sha(previous), []))
            if changed:
                put(destination/f'{digest}.rgba', current)
            row['status'] = 'rendered'
        except (adapter.pypdfium2.PdfiumError, ValueError, RuntimeError) as error:
            row.update(status='rejected', error=str(error))
        rows.append(row)
    result = {'schema': 'pdfium192-historical-pdf-audit/v1',
              'head': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT).decode().strip(),
              'adapter_sha256': sha((ROOT/'tools/pdf-harness/pdfium/render.py').read_bytes()),
              'pdfium': str(adapter.PDFIUM_INFO), 'pypdfium2': str(adapter.PYPDFIUM_INFO),
              'scope': 'All tracked tests/**/*.pdf and content-addressed blobs with PDF magic, deduplicated by bytes; nearest integral canvas at 96 DPI. Historical files untouched.',
              'old_projection': 'Exact removed canonicalize formula applied to same raw bitmap; historical raster matches reported separately, not presumed.',
              'cases': rows}
    put(destination/'audit.json', (json.dumps(result, indent=2, sort_keys=True)+'\n').encode())
    print(json.dumps({'unique_pdfs': len(rows), 'pdf_paths': sum(len(p) for p in pdfs.values()),
                      'rendered': sum(r['status']=='rendered' for r in rows),
                      'changed': sum(r.get('changed_pixels', 0)>0 for r in rows),
                      'rejected': sum(r['status']=='rejected' for r in rows)}))


if __name__ == '__main__':
    main()
