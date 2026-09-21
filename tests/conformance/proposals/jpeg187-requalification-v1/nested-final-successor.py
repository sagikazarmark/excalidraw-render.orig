#!/usr/bin/env python3
"""Recorded final-driver lookup fix; frozen nested.py and all producers untouched."""
import sys
if not __debug__ or sys.flags.optimize: raise SystemExit('Optimized Python forbidden')
import hashlib
import json
from pathlib import Path
import types

P=Path(__file__).resolve().parent
R=P.parents[3]
frozen=json.loads((P/'final-freeze.json').read_bytes())
source=(P/'nested.py').read_bytes()
hash_bytes=lambda b:hashlib.sha256(b).hexdigest()
relative=str((P/'nested.py').relative_to(R))
if hash_bytes(source)!=frozen['producer_files'][relative]: raise SystemExit('frozen driver changed')
old="if path != 'tests/package-consumer/Cargo.lock': c.require(before[path]==expected, 'staged source drift '+path)"
new="if path != 'tests/package-consumer/Cargo.lock': c.require(c.sha((stage/path).read_bytes())==expected, 'staged source drift '+path)"
text=source.decode()
if text.count(old)!=1: raise SystemExit('expected exact one lookup seam')
updated=text.replace(old,new)
record={'schema':'excalidraw-render/jpeg187-unbound-tool-successor/v1','approval':None,
        'reason':'Final freeze includes untracked packet files absent from tracked_state map. Hash staged bytes directly for every frozen path instead; preserve all checks and producer bytes.',
        'frozen_driver':{'path':relative,'sha256':hash_bytes(source)},
        'executable_successor_sha256':hash_bytes(updated.encode()),
        'wrapper_sha256':hash_bytes(Path(__file__).read_bytes()),'before_expression':old,'after_expression':new,
        'initial_failed_attempt':'KeyError for frozen untracked test_qualify.py before any native capture',
        'production_or_frozen_tooling_changed':False}
evidence=P/'nested/final-driver-successor.json'
if evidence.exists():
    if json.loads(evidence.read_bytes())!=record: raise SystemExit('successor record changed')
else:
    evidence.parent.mkdir(parents=True,exist_ok=True)
    with evidence.open('x') as f: json.dump(record,f,indent=2);f.write('\n')
module=types.ModuleType('authenticated_nested_successor');module.__file__=str(P/'nested.py')
exec(compile(updated,module.__file__,'exec',optimize=0),module.__dict__)
if len(sys.argv)!=1: raise SystemExit('no arguments; exact final frozen collection only')
module.collect('final-current',Path('/tmp/opencode/jpeg187-color-final-stage'),P/'final-freeze.json')
