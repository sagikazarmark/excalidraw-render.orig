#!/usr/bin/env python3
"""Retry collection after creating its required parent; retain failed logs."""
import importlib.util
import json
from pathlib import Path
import subprocess

ROOT=Path(__file__).resolve().parents[4]
P=Path(__file__).resolve().parent
path=ROOT/'tests/conformance/proposals/jpeg187-requalification-v1/collect.py'
spec=importlib.util.spec_from_file_location('collector',path)
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
c.PACKET=P
stage=Path('/tmp/opencode/pdfium192-successor-snapshots')
assert subprocess.check_output(['git','-C',str(stage),'rev-parse','HEAD'])==subprocess.check_output(['git','rev-parse','0cbdad3'],cwd=ROOT)
(P/'final-retry').mkdir(exist_ok=False)
c.capture(stage,'final-retry')
prior=ROOT/'tests/conformance/proposals/webp191-composition-requalification-v1/final-current/snapshots/snapshots.json'
a=json.loads(prior.read_bytes())['runs'][0]['results']
b=json.loads((P/'final-retry/snapshots/snapshots.json').read_bytes())['runs'][0]['results']
assert len(a)==len(b)==168 and [r['key'] for r in a]==[r['key'] for r in b]
changes=[{'before':o,'after':n} for o,n in zip(a,b) if o!=n]
c.json_new(P/'inventory-comparison.json',{'prior':c.desc(ROOT,str(prior.relative_to(ROOT))),
    'current':c.desc(ROOT,str((P/'final-retry/snapshots/snapshots.json').relative_to(ROOT))),
    'keys':168,'unchanged':168-len(changes),'changes':changes,
    'note':'Fresh packaged source provenance at 0cbdad3; later changes only xtask provenance checks. Not baseline approval.'})
print('Packaged comparison:',168-len(changes),'unchanged;',len(changes),'changed')
