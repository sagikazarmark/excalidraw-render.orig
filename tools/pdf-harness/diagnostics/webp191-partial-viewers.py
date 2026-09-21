#!/usr/bin/env python3
"""Create-only #191 viewer diagnosis; never alters reference evidence or gates."""
import base64
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def write(path, data):
    with path.open('xb') as f:
        f.write(data)


def run(args):
    return subprocess.run(args, cwd=ROOT, check=True, capture_output=True).stdout


def render(args):
    result = subprocess.run(args, cwd=ROOT, check=True, capture_output=True)
    assert not result.stdout
    report = json.loads(result.stderr)
    assert report['status'] == 'success', report
    return result.stderr


def project(raw, bg):
    result = bytearray()
    for i in range(0, len(raw), 4):
        r, g, b, a = raw[i:i+4]
        result.extend([(c*a+127)//255 + bg*(255-a)//255 for c in (r, g, b)])
        result.append(255)
    return bytes(result)


def diff(actual, expected):
    assert len(actual) == len(expected) == 9*7*4
    return {'different_pixels': sum(actual[i:i+4] != expected[i:i+4]
                                   for i in range(0, len(actual), 4)),
            'different_alpha': sum(actual[i] != expected[i] for i in range(3, len(actual), 4)),
            'maximum_channel_delta': max(abs(a-b) for a, b in zip(actual, expected)),
            'actual_sha256': digest(actual), 'expected_sha256': digest(expected)}


def main():
    destination = (ROOT / sys.argv[1]).resolve()
    destination.relative_to(ROOT)
    destination.mkdir(parents=True, exist_ok=False)
    reference_bytes = (ROOT/'tests/oracle/webp-sample/reference.json').read_bytes()
    assert digest(reference_bytes) == 'b325e8a2d8ad0a1fbd72463a9e2272ef43a1151c2ca040aa5fc46150335f49b6'
    reference = json.loads(reference_bytes)
    cli = ROOT/'target/release/excalidraw-render'
    results = []
    for row in reference['cases']:
        case = destination/row['id']
        case.mkdir()
        scene = case/'scene.json'
        write(scene, json.dumps(row['scene']).encode())
        pdf = case/'image.pdf'
        command = [str(cli), str(scene), '--output', str(pdf), '--format', 'pdf',
                   '--padding', '0', '--no-background', '--report-format', 'json']
        report = render(command)
        write(case/'report.json', report)
        # Independent processes must reproduce the exact PDF and complete report.
        again = case/'repeat.pdf'
        repeated = render([str(cli), str(scene), '--output', str(again), '--format', 'pdf',
                        '--padding', '0', '--no-background', '--report-format', 'json'])
        assert pdf.read_bytes() == again.read_bytes()
        # CLI JSON contains no output-path identity; compare the whole record.
        write(case/'repeat-report.json', repeated)
        assert json.loads(report) == json.loads(repeated)
        samples = row['reference']['samples']['intrinsic']
        browser = base64.b64decode(samples['rgba_base64'])
        item = {'id': row['id'], 'alpha': row['alpha'], 'codec': row['codec'],
                'pdf_sha256': digest(pdf.read_bytes()), 'viewers': {}}
        for viewer in ['pdfium', 'pdfjs']:
            raw = None
            for repeat in range(2):
                out = case/f'{viewer}-{repeat}.rgba'
                metadata = run(['bash', f'tools/pdf-harness/run-{viewer}.sh',
                                'render', str(pdf), str(out), '9', '7'])
                write(case/f'{viewer}-{repeat}.json', metadata)
                pixels = out.read_bytes()
                assert len(pixels) == 252
                if raw is not None:
                    assert pixels == raw
                raw = pixels
            comparisons = {'raw': diff(raw, browser)}
            for name, bg in [('black', 0), ('white', 255)]:
                comparisons[name] = diff(project(raw, bg), base64.b64decode(
                    samples['composites'][name]['rgba_base64']))
                comparisons[f'canonical-browser-{name}'] = diff(project(raw, bg), project(browser, bg))
            item['viewers'][viewer] = comparisons
        results.append(item)
        print(row['id'], {v: item['viewers'][v]['raw']['different_pixels']
                          for v in item['viewers']}, flush=True)
    files = {str(p.relative_to(destination)): digest(p.read_bytes())
             for p in sorted(destination.rglob('*')) if p.is_file()}
    result = {'kind': 'unapproved-webp191-pdf-viewer-diagnosis-v1',
              'reference_sha256': digest(reference_bytes), 'cli_sha256': digest(cli.read_bytes()),
              'rustc': run(['rustc', '--version']).decode().strip(),
              'head': run(['git', 'rev-parse', 'HEAD']).decode().strip(),
              'script_sha256': digest(Path(__file__).read_bytes()),
              'notes': 'Actual pinned transparent viewer output, two processes each. Composite comparisons use canonical rounded projection; no thresholds or obligations changed.',
              'cases': results, 'files': files}
    write(destination/'measurements.json', (json.dumps(result, indent=2, sort_keys=True)+'\n').encode())


if __name__ == '__main__':
    main()
