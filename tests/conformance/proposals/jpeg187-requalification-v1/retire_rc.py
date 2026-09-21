#!/usr/bin/env python3
"""Archive the unapproved RC plan without changing any captured evidence."""
import sys
if not __debug__ or sys.flags.optimize:
    raise SystemExit('Optimized Python forbidden')
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
PACKET = Path(__file__).resolve().parent
def sha(data): return hashlib.sha256(data).hexdigest()
def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as f: f.write(data)

if __name__ == '__main__':
    archive = PACKET / 'retired-rc-plan'
    for name in ['qualify.py', 'collect.py', 'test_qualify.py', 'FINAL-PLAN.md', 'README.md', 'readiness.json', 'file-inventory.json', 'tiny-neutral-browser.mjs']:
        put(archive / name, (PACKET / name).read_bytes())
    rows = []
    for directory in ['old', 'exploratory-current', 'nested', 'proposed', 'logs', 'before', 'readiness-checks']:
        for path in sorted((PACKET / directory).rglob('*')):
            if path.is_file(): rows.append({'path': str(path.relative_to(PACKET)), 'sha256': sha(path.read_bytes())})
    path = PACKET / 'historical-lock-transition.json'
    rows.append({'path': path.name, 'sha256': sha(path.read_bytes())})
    put(archive / 'preserved-evidence.json', (json.dumps({'approval': None, 'files': rows}, indent=2) + '\n').encode())
    print(f'Archived prior pending RC plan and pinned {len(rows)} original evidence files; no approval or capture.')
