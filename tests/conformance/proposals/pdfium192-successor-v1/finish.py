#!/usr/bin/env python3
"""Continue after the recorded historical-producer stop; preserve early captures."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

spec = importlib.util.spec_from_file_location('collection', Path(__file__).with_name('collect.py'))
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
ROOT, P = c.ROOT, c.P


def check():
    for path, digest in json.loads((P/'final-freeze.json').read_bytes())['files'].items():
        if c.sha((ROOT/path).read_bytes()) != digest:
            raise ValueError(f'final frozen bytes changed: {path}')


def freeze():
    before = json.loads((P/'freeze.json').read_bytes())['files']
    allowed = {'xtask/src/conformance_candidate_exceptions.rs'}
    for path, digest in before.items():
        if path not in allowed:
            assert c.sha((ROOT/path).read_bytes()) == digest, path
    # This consumer-only fix is outside native and viewer producers. Native
    # capture/collection verification independently rejects any changed producer.
    paths = subprocess.check_output(['git', 'ls-files', '-z'], cwd=ROOT).decode().split('\0')
    files = {p: c.sha((ROOT/p).read_bytes()) for p in paths if p}
    for path in P.glob('*.py'):
        files[str(path.relative_to(ROOT))] = c.sha(path.read_bytes())
    c.put(P/'final-freeze.json', (json.dumps({'files': files, 'reason': 'Historical PDFium adapter/profile bindings preserved through exact approved successor validation; no native/viewer producer changed.'}, indent=2)+'\n').encode())


def representative():
    check()
    prefix = ['cargo', 'run', '--release', '--locked', '-p', 'xtask', '--', 'conformance']
    for name, args in [('inspect', ['candidate-inspect', c.V14]),
                       ('materialize', ['candidate-materialize', c.V14]),
                       ('subject', ['candidate-approval-subject', c.V14])]:
        c.run('final-'+name, prefix+args)
    equality = []
    for fmt in ['svg', 'png', 'pdf']:
        for name in [f'native.{fmt}', f'native-{fmt}-report.json']:
            assert (ROOT/c.V14/name).read_bytes() == (ROOT/c.V13/name).read_bytes(), name
            equality.append(name)
    for path in (ROOT/c.V14/'viewer-evidence').glob('*.rgba'):
        assert path.read_bytes() == (ROOT/c.V13/'viewer-evidence'/path.name).read_bytes(), path
        equality.append('viewer-evidence/'+path.name)
    c.put(P/'representative-equality.json', (json.dumps({'prior': c.V13, 'successor': c.V14, 'identical_files': equality}, indent=2)+'\n').encode())
    check()


def protected():
    check()
    original = ROOT/'tests/conformance/candidates/stage4-pdf-backend-requalification-v2'
    out = P/'protected-final'
    scene = (ROOT/'tests/conformance/pdf-backend-qualification/project-operations.excalidraw').read_bytes()
    for i in range(2):
        result = subprocess.run([str(ROOT/'target/release/excalidraw-render'), '-', '--output', '-', '--format', 'pdf', '--report-format', 'json', '--pdf-raster-fallback', 'forbid'], cwd=ROOT, env=c.ENV, input=scene, capture_output=True, check=True)
        assert result.stdout == (original/'candidate.pdf').read_bytes()
        assert result.stderr == (original/'candidate-report.json').read_bytes()
        c.put(out/f'run-{i}.pdf', result.stdout)
        c.put(out/f'run-{i}-report.json', result.stderr)
    c.run('protected-structure', ['bash', 'tools/pdf-harness/run-structure.sh', 'inspect', str(out/'run-0.pdf'), 'tests/conformance/pdf-backend-qualification/project-operations-structure.json', 'pass', str(out/'structure.json')])
    for viewer in ['pdfium', 'pdfjs']:
        for i in range(2):
            destination = out/f'{viewer}-{i}.rgba'
            c.run(f'protected-{viewer}-{i}', ['bash', f'tools/pdf-harness/run-{viewer}.sh', 'render', str(out/'run-0.pdf'), str(destination), '520', '200'])
            assert destination.read_bytes() == (original/f'candidate-{viewer}.rgba').read_bytes()
    check()


if __name__ == '__main__':
    {'freeze': freeze, 'representative': representative, 'protected': protected, 'check': check}[sys.argv[1]]()
