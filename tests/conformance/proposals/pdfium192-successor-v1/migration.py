#!/usr/bin/env python3
"""Narrow pending V14/owner-source migration; never grants approval."""
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys

spec = importlib.util.spec_from_file_location('collection', Path(__file__).with_name('collect.py'))
c = importlib.util.module_from_spec(spec)
spec.loader.exec_module(c)
ROOT, P = c.ROOT, c.P
LEDGER = 'tests/conformance/coverage/ownership.json'
REGISTRY = 'tests/conformance/candidates/registry.json'
SOURCE = 'crates/excalidraw-render/tests/fast/webp_sample_contract.rs'
CASE = 'representative.stage4-frames-text-assets.v14'


def pretty(value):
    return (json.dumps(value, indent=2, ensure_ascii=False)+'\n').encode()


def expected(applied=False):
    before = json.loads((P/'before'/LEDGER).read_bytes())
    registry = json.loads((P/'before'/REGISTRY).read_bytes())
    candidate = json.loads((ROOT/c.V14/'manifest.json').read_bytes())
    assert (candidate['approval'] is not None) == applied
    assert candidate['case_id'] == CASE
    subject = (P/'logs/final-subject.stdout').read_text().strip()
    result = copy.deepcopy(before)
    changes = []
    for row in result['assignments']:
        disposition = row['disposition']
        if disposition['status'] != 'owned':
            continue
        owner = disposition['owner']
        source = owner.get('source')
        if source is not None:
            current = c.sha((ROOT/source['path']).read_bytes())
            if source['sha256'] != current:
                assert source['path'] == SOURCE, ('unexpected stale source', row['id'], source)
                assert row['id'].startswith('codec:webp:sample:')
                changes.append(row['id'])
                source['sha256'] = current
    assert len(changes) == 36
    assert len(registry['cases']) == 1
    entry = registry['cases'][0]
    assert entry['id'] == 'representative.stage4-frames-text-assets.v13'
    exceptions = copy.deepcopy(entry['exceptions'])
    profile_path = 'tests/conformance/viewer-profiles/canonical-pdf-pdfium-v1.json'
    for scope in exceptions:
        scope['case_id'] = CASE
        if scope['comparison_id'] == 'pdf-pdfium':
            scope['viewer_profile']['descriptor'] = {'path': profile_path,
                'byte_length': len((ROOT/profile_path).read_bytes()), 'sha256': c.sha((ROOT/profile_path).read_bytes())}
    assert candidate['exception_proposal']['exceptions'] == exceptions
    command = ['cargo', 'run', '--release', '--locked', '-p', 'xtask', '--', 'conformance', 'candidate-verify', c.V14]
    entry.update(id=CASE, directory=c.V14, status='pdfium192-alpha-profile-successor', exceptions=exceptions,
                 execution_keys=[CASE+':default:'+f for f in ['svg','png','pdf']], consumer=command)
    owner = next(r for r in result['assignments'] if r['id']=='interaction:frames-text-assets')['disposition']['owner']
    assert owner['kind']=='representative' and owner['binding']['case']=='representative.stage4-frames-text-assets.v13'
    owner['binding'] = {'case': CASE, 'registry': {'path': REGISTRY, 'sha256': c.sha(pretty(registry))},
                        'manifest': {'path': c.V14+'/manifest.json', 'sha256': c.sha((ROOT/c.V14/'manifest.json').read_bytes())},
                        'approval_subject_sha256': subject, 'command': command}
    assert len(result['assignments']) == 2418
    assert {s: sum(r['disposition']['status']==s for r in result['assignments']) for s in ['owned','gap','excluded']} == {'owned':2318,'gap':97,'excluded':3}
    assert sum(r['id'].startswith('codec:webp:sample:') and r['disposition']['status']=='gap' for r in result['assignments']) == 12
    return result, registry, changes


def prepare():
    ledger, registry, changes = expected()
    c.put(P/'pending/ownership.template.json', pretty(ledger))
    c.put(P/'pending/registry.json', pretty(registry))
    c.put(P/'migration-plan.json', pretty({'approval': None, 'source_refresh_ids': changes,
        'new_owners': 0, 'representative': CASE, 'representative_subject_sha256': (P/'logs/final-subject.stdout').read_text().strip(),
        'counts': {'obligations':2418,'owned':2318,'excluded':3,'gaps':97},
        'scope': '36 WebP source-hash refreshes only; V13 to V14; two PDFium exception profile descriptors rebound with identical limits. Twelve partial-alpha gaps, all criteria and exclusions preserved.'}))
    verify()


def verify():
    ledger, registry, changes = expected()
    assert (P/'pending/ownership.template.json').read_bytes() == pretty(ledger)
    assert (P/'pending/registry.json').read_bytes() == pretty(registry)
    assert json.loads((P/'migration-plan.json').read_bytes())['source_refresh_ids'] == changes
    for path in [LEDGER, REGISTRY, 'tests/conformance/coverage/ownership-policy.json', 'tests/conformance/coverage/inventory.json']:
        assert (ROOT/path).read_bytes() == (P/'before'/path).read_bytes(), path
    print('36 source refreshes; V14; zero new owners; 2418 obligations/2321 assigned-excluded/97 gaps; live unchanged')


if __name__ == '__main__':
    {'prepare':prepare, 'verify':verify}[sys.argv[1]]()
