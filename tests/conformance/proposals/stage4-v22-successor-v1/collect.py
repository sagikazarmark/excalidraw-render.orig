#!/usr/bin/env python3
"""Create-only successor collection. No approval or live metadata migration."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
P = Path(__file__).resolve().parent
V22 = 'tests/conformance/candidates/stage4-frames-text-assets-v22'
V21 = 'tests/conformance/candidates/stage4-frames-text-assets-v21'
ENV = dict(os.environ, PATH='/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:'+os.environ['PATH'], CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_PROFILE_RELEASE_DEBUG='0', PYTHONDONTWRITEBYTECODE='1')


def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as output:
        output.write(data)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def run(name, args):
    result = subprocess.run(args, cwd=ROOT, env=ENV, capture_output=True)
    put(P/'logs'/f'{name}.stdout', result.stdout)
    put(P/'logs'/f'{name}.stderr', result.stderr)
    if result.returncode:
        raise RuntimeError(f'{name}: {result.stderr.decode()} {result.stdout.decode()}')
    return result.stdout


def freeze():
    paths = subprocess.check_output(['git', 'ls-files', '-z'], cwd=ROOT).decode().split('\0')
    files = {p: sha((ROOT/p).read_bytes()) for p in paths if p}
    files[str(Path(__file__).relative_to(ROOT))] = sha(Path(__file__).read_bytes())
    for path in ['tests/conformance/coverage/ownership.json', 'tests/conformance/candidates/registry.json',
                 'tests/conformance/coverage/ownership-policy.json', 'tests/conformance/coverage/inventory.json']:
        put(P/'before'/path, (ROOT/path).read_bytes())
    put(P/'freeze.json', (json.dumps({'base_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT).decode().strip(), 'files': files}, indent=2)+'\n').encode())


def check():
    for path, digest in json.loads((P/'freeze.json').read_bytes())['files'].items():
        if sha((ROOT/path).read_bytes()) != digest:
            raise ValueError(f'frozen bytes changed: {path}')


def representative():
    check()
    prefix = ['cargo', 'run', '--release', '--locked', '-p', 'xtask', '--', 'conformance']
    for name, args in [('bridge', ['collect-pdf-colorspace-bridge']), ('revise', ['candidate-revise', V22])]:
        run(name, prefix+args)
    for name in ['assets', 'viewer-evidence']:
        (ROOT/V22/name).mkdir()
    steps = [('native', prefix+['candidate-native-capture', V22]),
             ('assets', ['node', 'tools/reference-oracle/scripts/stage4-viewer-assets.mjs', V22]),
             ('derive', prefix+['candidate-prepare-assets', V22]),
             ('fonts', ['node', 'tools/reference-oracle/scripts/stage4-font-audit-qualification.mjs', V22, 'collect']),
             ('viewers', ['node', 'tools/reference-oracle/scripts/stage4-candidate-views.mjs', 'collect', V22]),
             ('exceptions', prefix+['candidate-propose-exceptions', V22]),
             ('inspect', prefix+['candidate-inspect', V22]),
             ('materialize', prefix+['candidate-materialize', V22]),
             ('subject', prefix+['candidate-approval-subject', V22])]
    for name, args in steps:
        run(name, args)
        check()
    equal = []
    for fmt in ['svg', 'png', 'pdf']:
        for name in [f'native.{fmt}', f'native-{fmt}-report.json']:
            assert (ROOT/V22/name).read_bytes() == (ROOT/V21/name).read_bytes(), name
            equal.append(name)
    put(P/'representative-equality.json', (json.dumps({'prior': V21, 'successor': V22, 'identical_files': equal}, indent=2)+'\n').encode())


if __name__ == '__main__':
    {'freeze': freeze, 'representative': representative, 'check': check}[sys.argv[1]]()
