#!/usr/bin/env python3
"""Fresh packaged snapshot capture using the unchanged historical collector."""
import importlib.util
import json
from pathlib import Path
import subprocess

ROOT = Path(__file__).resolve().parents[4]
P = Path(__file__).resolve().parent
path = ROOT/'tests/conformance/proposals/jpeg187-requalification-v1/collect.py'
spec = importlib.util.spec_from_file_location('collector', path)
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
c.PACKET = P
assert path.read_bytes() == subprocess.check_output(['git', 'show', '0cbdad3:'+str(path.relative_to(ROOT))], cwd=ROOT)
stage = Path('/tmp/opencode/pdfium192-successor-snapshots')
assert not stage.exists()
c.command(['git', 'worktree', 'add', '--detach', str(stage), '0cbdad3'])
assert c.package(stage, 'final-current')
c.capture(stage, 'final-current')
prior = ROOT/'tests/conformance/proposals/webp191-composition-requalification-v1/final-current/snapshots/snapshots.json'
a = json.loads(prior.read_bytes())['runs'][0]['results']
b = json.loads((P/'final-current/snapshots/snapshots.json').read_bytes())['runs'][0]['results']
assert len(a) == len(b) == 168
assert [r['key'] for r in a] == [r['key'] for r in b]
changes = [{'before': old, 'after': new} for old, new in zip(a, b) if old != new]
c.json_new(P/'inventory-comparison.json', {'prior': c.desc(ROOT, str(prior.relative_to(ROOT))),
    'current': c.desc(ROOT, str((P/'final-current/snapshots/snapshots.json').relative_to(ROOT))),
    'keys':168, 'unchanged':168-len(changes), 'changes':changes,
    'note':'Fresh source provenance at 0cbdad3; subsequent changes affect only xtask provenance checks, not production artifacts. Not baseline approval.'})
print('Packaged comparison:', 168-len(changes), 'unchanged;', len(changes), 'changed')
