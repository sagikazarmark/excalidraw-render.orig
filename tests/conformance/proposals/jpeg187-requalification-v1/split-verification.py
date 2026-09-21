#!/usr/bin/env python3
"""Authenticate split completion before the unchanged frozen packet seal/check.

Expected pending binding failures are recorded, never treated as passing tests.
This successor changes no production, candidate, ledger or frozen checker bytes.
"""
import sys
if not __debug__ or sys.flags.optimize:
    raise SystemExit('Optimized Python forbidden; no verification started.')
import hashlib
import json
from pathlib import Path
import re
import types

P = Path(__file__).resolve().parent
ROOT = P.parents[3]
LONG = 'compatibility_recovery_contract::unsupported_value_contract_covers_every_current_field_family'
EXPECTED = [
    {'target': 'xtask --bin xtask', 'count': 2, 'reason': 'stale ownership source bindings'},
    {'target': 'xtask --test coverage', 'count': 2, 'reason': 'stale ownership source bindings'},
    {'target': 'xtask --test representative_candidate', 'count': 1, 'reason': 'approved V8 producer bindings are stale'},
]

def require(condition, message):
    if not condition:
        raise ValueError(message)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def read(path):
    return json.loads(path.read_bytes())

def check_descriptor(d):
    path = ROOT / d['path']
    data = path.read_bytes()
    require(len(data) == d['byte_length'] and sha(data) == d['sha256'], 'stale evidence '+d['path'])
    return data

def parse_workspace(raw):
    text = raw.decode()
    require(text.count('Running tests/fast/main.rs') == 1, 'expected one renderer fast target')
    lib, fast = text.split('Running tests/fast/main.rs', 1)
    passed = lambda s: re.findall(r'^test (.+) \.\.\. ok$', s, flags=re.M)
    require('running 411 tests' in lib, 'unexpected library inventory')
    require('test result: ok. 404 passed; 0 failed; 7 ignored;' in lib, 'library completion missing')
    require(len(passed(lib)) == len(set(passed(lib))) == 404, 'library pass listing differs')
    require('running 585 tests' in fast, 'unexpected fast inventory')
    require(len(passed(fast)) == len(set(passed(fast))) == 584, 'fast success listing is not 584 unique tests')
    require(LONG not in passed(fast) and 'test '+LONG+' has been running for over 60 seconds' in fast, 'wrong incomplete test')
    require('test result:' not in fast and not re.search(r'^test .* \.\.\. (FAILED|ignored)', fast, flags=re.M), 'unexpected fast result/failure')
    return {'library_passed': 404, 'library_ignored': 7, 'fast_total': 585,
            'fast_logged_passed': 584, 'fast_completed_elsewhere': LONG}

def validate_attestation(a, counts):
    require(a['schema'] == 'excalidraw-render/jpeg187-parent-split-verification/v1', 'wrong attestation schema')
    require(a['approval'] is None and a['verification_mode'] == 'combined-commands-not-a-single-clean-run', 'must describe split, unapproved verification')
    require(a['initial']['command'] == ['cargo','test','--locked','--workspace','--no-fail-fast'], 'initial command changed')
    require(a['initial']['outcome'] == 'timed-out' and a['initial']['timeout_ms'] == 1200000, 'timeout must remain explicit')
    require(a['initial']['verified_counts'] == counts, 'attested counts differ from raw output')
    continuation = a['long_test_continuation']
    require(continuation['command'] == ['cargo','test','--locked','-p','excalidraw-render','--test','fast',LONG,'--','--exact'], 'wrong long-test continuation')
    require(continuation['outcome'] == 'passed' and continuation['duration_seconds'] == 816.82, 'long-test completion absent')
    require(continuation['evidence_kind'] == 'parent-session-attestation-no-raw-log-available', 'do not invent continuation raw logs')
    integrations = a['renderer_integrations']
    require(integrations['outcome'] == 'all-remaining-targets-passed' and integrations['png_parity_passed'] == 10 and integrations['svg_parity_passed'] == 8, 'renderer integration completion missing')
    require(a['renderer_doctests'] == {'outcome':'passed','tests':0,'evidence_kind':'parent-session-attestation-no-raw-log-available'}, 'doctest report differs')
    remaining = a['remaining_workspace']
    require(remaining['command'] == ['cargo','test','--locked','--workspace','--exclude','excalidraw-render','--no-fail-fast'], 'rest-workspace command differs')
    require(remaining['outcome'] == 'failed-only-at-enumerated-pending-bindings', 'pending failures cannot be marked passing')
    require(remaining['expected_failures'] == EXPECTED and remaining['failure_count'] == 5, 'only five enumerated pending failures permitted')
    require(remaining['all_other_targets_passed'] is True and remaining['xtask_bin_passed'] == 101, 'remaining completion differs')
    require(a['reported_ignored_total'] == 19 and a['all_tests_green_before_approval'] is False, 'false green/ignore claim')

def verify_split():
    a = read(P/'final-checks/parent-split-verification.json')
    raw = check_descriptor(a['initial']['raw_log'])
    validate_attestation(a, parse_workspace(raw))
    check_descriptor(a['parent_message'])
    check_descriptor(a['frozen_source_record'])
    freeze = read(P/'final-freeze.json')
    for path, digest in freeze['producer_files'].items():
        require(sha((ROOT/path).read_bytes()) == digest, 'frozen source changed '+path)
    require(a['producer_sources'] == read(ROOT/'tests/conformance/candidates/stage4-frames-text-assets-v9/native-capture.json')['producer_sources'], 'attestation not bound to exact V9 producer set')
    for d in a['producer_sources']:
        check_descriptor(d)
    check_descriptor(a['test_source'])
    require(a['test_source']['path'] == 'crates/excalidraw-render/tests/fast/jpeg_color_contract.rs', 'wrong final JPEG test')
    for d in a['observed_pending_gate_errors']:
        text = check_descriptor(d).decode()
        if d['path'].endswith('v9-pending.stderr'):
            require('approval must be an object' in text, 'missing actual V9 approval refusal')
        else:
            require('stale' in text, 'missing actual stale-source gate refusal')
    return a

def frozen_checker():
    path = P/'qualify.py'
    data = path.read_bytes()
    expected = read(P/'final-freeze.json')['producer_files'][str(path.relative_to(ROOT))]
    require(sha(data) == expected, 'frozen checker bytes changed')
    module = types.ModuleType('authenticated_frozen_jpeg187_checker')
    module.__file__ = str(path)
    exec(compile(data, str(path), 'exec', optimize=0), module.__dict__)
    return module

if __name__ == '__main__':
    require(sys.argv[1:] in [['seal'], ['verify']], 'usage: split-verification.py seal|verify')
    verify_split()
    module = frozen_checker()
    if sys.argv[1] == 'seal': module.seal()
    else: module.verify(False)
    print('Split verification authenticated: timeout + completed continuation; five pending binding failures remain explicit, not waived.')
