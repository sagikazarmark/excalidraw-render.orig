#!/usr/bin/env python3
"""Bounded formatting requalification. Never grants approval or edits live owners."""
import copy
import difflib
import hashlib
import itertools
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
PACKET = Path(__file__).resolve().parent
OLD = ROOT / 'tests/conformance/proposals/png-device-phase-six-case-v1'
V7 = 'tests/conformance/candidates/stage4-frames-text-assets-v7'
V8 = 'tests/conformance/candidates/stage4-frames-text-assets-v8'
BRIDGE = 'tests/conformance/candidates/stage4-pdf-colorspace-bridge-v2'
PDF = 'tests/conformance/candidates/stage4-pdf-backend-requalification-v2'
RUST = '/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin'
ENV = dict(os.environ, PATH=RUST + ':' + os.environ['PATH'], CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_PROFILE_RELEASE_DEBUG='0')
FILES = ['src/lib.rs', 'src/render/asset/color.rs', 'src/render/png.rs', 'tests/fast/dark_mode_frame_and_image_render.rs', 'tests/fast/png_stage4_contract.rs', 'tests/fast/stage4_interaction_contract.rs']
FILES = ['crates/excalidraw-render/' + p for p in FILES]
TOOL = 'xtask/src/conformance_pdf_bridge.rs'
EXTERNAL = 'crates/excalidraw-render/tests/stage4_typography.rs'
EXTERNAL_OLD = '93c4d040619c9f9b042caee5b7be3de5ecf7910791877b2a5549beb22cb5dc86'
def sha(data): return hashlib.sha256(data).hexdigest()
def load(path): return json.loads(path.read_bytes())
def canonical(v): return json.dumps(v, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
def pretty(v): return json.dumps(v, indent=2, ensure_ascii=False).encode() + b'\n'
def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as output: output.write(data)
def desc(root, path):
    data = (root / path).read_bytes()
    return {'path': str(path), 'byte_length': len(data), 'sha256': sha(data)}
def run(args, *, data=None):
    result = subprocess.run(list(map(str, args)), input=data, cwd=ROOT, env=ENV, capture_output=True)
    if result.returncode: raise RuntimeError(result.stderr.decode() + result.stdout.decode())
    return result.stdout
def xtask(*args): return run(['cargo', 'run', '--release', '--locked', '-p', 'xtask', '--', 'conformance', *args])
def log(name, args):
    result = subprocess.run(list(map(str, args)), cwd=ROOT, env=ENV, capture_output=True)
    put(PACKET / 'checks' / (name + '.stdout'), result.stdout)
    put(PACKET / 'checks' / (name + '.stderr'), result.stderr)
    if result.returncode: raise RuntimeError(result.stderr.decode() + result.stdout.decode())
    print(name + ': passed', flush=True)
    return result.stdout

def archive():
    frozen = {}
    paths = run(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard']).decode().split('\0')
    for p in paths:
        if p and not p.startswith(str(PACKET.relative_to(ROOT)) + '/') and (ROOT / p).is_file(): frozen[p] = sha((ROOT / p).read_bytes())
    put(PACKET / 'initial-live.json', pretty(frozen))
    expected = load(OLD / 'live-before.json')
    for p in FILES:
        before = (ROOT / p).read_bytes()
        assert sha(before) == expected[p], p
        put(PACKET / 'before' / p, before)
        # stdin/stdout formatting avoids following mod declarations or writing
        # the live source. The eventual source patch must equal this output.
        formatted = run([RUST + '/rustfmt', '--edition', '2021', '--emit', 'stdout'], data=before)
        assert formatted != before
        put(PACKET / 'rustfmt-output' / p, formatted)
    put(PACKET / 'before' / TOOL, (ROOT / TOOL).read_bytes())
    current = (ROOT / EXTERNAL).read_text()
    inverse = current.replace('    // This external test authors its input coordinates with pinned libm;\n    // the renderer\'s private portable_math module is not part of its API.\n', '')
    inverse = re.sub(r'libm::(asin|acos)\(([^()]*)\)', lambda m: (m[2] if re.fullmatch(r'\w+', m[2]) else '(' + m[2] + ')') + '.' + m[1] + '()', inverse)
    imports = 'use excalidraw_render::{\n    FrameOptions, OutputFormat, RenderRequest, Renderer, SceneDocument, Sizing,\n};'
    found = []
    for spacing, trig in itertools.product([',Sizing', ', Sizing'], ['(angle.sin(), angle.cos())', 'angle.sin_cos()']):
        candidate = inverse.replace(imports, 'use excalidraw_render::{FrameOptions, OutputFormat, RenderRequest, Renderer, SceneDocument' + spacing + '};')
        candidate = candidate.replace('(libm::sin(angle), libm::cos(angle))', trig)
        if sha(candidate.encode()) == EXTERNAL_OLD: found.append(candidate.encode())
    assert len(found) == 1, 'cannot authenticate external-test preimage'
    put(PACKET / 'before' / EXTERNAL, found[0])
    put(PACKET / 'after' / EXTERNAL, current.encode())
    put(PACKET / 'external-test.diff', ''.join(difflib.unified_diff(found[0].decode().splitlines(True), current.splitlines(True), fromfile='before/' + EXTERNAL, tofile='after/' + EXTERNAL)).encode())
    for path in ['tests/conformance/coverage/ownership.json', 'tests/conformance/coverage/inventory.json', 'tests/conformance/coverage/requirements.json', 'tests/conformance/candidates/registry.json']:
        put(PACKET / 'before' / path, (ROOT / path).read_bytes())
    print(f'Archived six exact pre-format files, tool preimage and authenticated external test {EXTERNAL_OLD}; {len(frozen)} live files frozen.')

def formatted():
    transitions = []
    for path in FILES:
        before = (PACKET / 'before' / path).read_bytes()
        after = (ROOT / path).read_bytes()
        assert after == (PACKET / 'rustfmt-output' / path).read_bytes(), path
        put(PACKET / 'after' / path, after)
        diff = ''.join(difflib.unified_diff(before.decode().splitlines(True), after.decode().splitlines(True), fromfile='before/' + path, tofile='after/' + path))
        put(PACKET / 'diffs' / (path + '.diff'), diff.encode())
        transitions.append({'path': path, 'kind': 'pinned-rustfmt-output-only', 'before': desc(PACKET, 'before/' + path), 'after': desc(PACKET, 'after/' + path)})
    before = (PACKET / 'before' / TOOL).read_bytes()
    assert (ROOT / TOOL).read_bytes() == before.replace(b'stage4-pdf-colorspace-bridge-v1', b'stage4-pdf-colorspace-bridge-v2')
    put(PACKET / 'after' / TOOL, (ROOT / TOOL).read_bytes())
    put(PACKET / 'bridge-version.diff', ''.join(difflib.unified_diff(before.decode().splitlines(True), (ROOT / TOOL).read_text().splitlines(True), fromfile='before/' + TOOL, tofile='after/' + TOOL)).encode())
    transitions.extend([{'path': TOOL, 'kind': 'versioned-immutable-bridge-directory-only', 'before': desc(PACKET, 'before/' + TOOL), 'after': desc(PACKET, 'after/' + TOOL)}, {'path': EXTERNAL, 'kind': 'parent-test-only-portable-libm-and-import-format', 'before': desc(PACKET, 'before/' + EXTERNAL), 'after': desc(PACKET, 'after/' + EXTERNAL)}])
    put(PACKET / 'source-transitions.json', pretty(transitions))
    print('Exact rustfmt outputs and sole bridge-directory version change verified.')

def quality():
    log('fmt', ['cargo', 'fmt', '--all', '--check'])
    log('clippy', ['cargo', 'clippy', '--locked', '--workspace', '--all-targets', '--', '-D', 'warnings'])
    for module in ['png_stage4_contract', 'stage4_interaction_contract', 'dark_mode_frame_and_image_render']:
        log(module, ['cargo', 'test', '--release', '--locked', '-p', 'excalidraw-render', '--test', 'fast', module + '::'])
    log('typography', ['cargo', 'test', '--release', '--locked', '-p', 'excalidraw-render', '--test', 'stage4_typography'])
    log('protected-pdf', ['cargo', 'test', '--release', '--locked', '-p', 'excalidraw-render', '--test', 'fast', 'pdf_backend_qualification::complete_project_operation_matrix_is_deterministic_and_vector_first', '--', '--exact'])

def freeze():
    before = load(PACKET / 'initial-live.json')
    current = {p: sha((ROOT / p).read_bytes()) for p in before}
    assert {p for p in before if before[p] != current[p]} == set(FILES + [TOOL])
    for row in load(PACKET / 'source-transitions.json'):
        assert (ROOT / row['path']).read_bytes() == (PACKET / row['after']['path']).read_bytes()
    current[str(Path(__file__).relative_to(ROOT))] = sha(Path(__file__).read_bytes())
    put(PACKET / 'frozen-live.json', pretty(current))
    print(f'Frozen {len(current)} source/tooling/history files before collection.')

def check_frozen(applied=False):
    allowed = {'tests/conformance/coverage/ownership.json', 'tests/conformance/candidates/registry.json'} if applied else set()
    for p, expected in load(PACKET / 'frozen-live.json').items():
        if p not in allowed: assert sha((ROOT / p).read_bytes()) == expected, p

def capture():
    check_frozen()
    commands = [
        ('bridge-v2', ['collect-pdf-colorspace-bridge']),
        ('revise-v8', ['candidate-revise', V8]),
    ]
    prefix = ['cargo', 'run', '--release', '--locked', '-p', 'xtask', '--', 'conformance']
    for name, args in commands: log(name, prefix + args)
    for name in ['assets', 'viewer-evidence']: (ROOT / V8 / name).mkdir()
    steps = [
        ('native-v8', prefix + ['candidate-native-capture', V8]),
        ('assets-v8', ['node', 'tools/reference-oracle/scripts/stage4-viewer-assets.mjs', V8]),
        ('derive-v8', prefix + ['candidate-prepare-assets', V8]),
        ('fonts-v8', ['node', 'tools/reference-oracle/scripts/stage4-font-audit-qualification.mjs', V8, 'collect']),
        ('viewers-v8', ['node', 'tools/reference-oracle/scripts/stage4-candidate-views.mjs', 'collect', V8]),
        ('exceptions-v8', prefix + ['candidate-propose-exceptions', V8]),
        ('inspect-v8', prefix + ['candidate-inspect', V8]),
        ('materialize-v8', prefix + ['candidate-materialize', V8]),
        ('subject-v8', prefix + ['candidate-approval-subject', V8]),
    ]
    for name, args in steps: log(name, args)
    check_frozen()
    print((PACKET / 'checks/subject-v8.stdout').read_text().strip())

def equality():
    check_frozen()
    build = load(ROOT / V8 / 'native-capture.json')['build']
    exe = ROOT / 'target/release/excalidraw-render'
    assert sha(exe.read_bytes()) == build['executable_sha256']
    evidence = []
    def replay(name, scene, fmt, args, old_artifact, old_report):
        first = None
        for i in range(2):
            command = [str(exe), '-', '--output', '-', '--format', fmt, '--report-format', 'json', *args]
            result = subprocess.run(command, input=scene, cwd=ROOT, env=ENV, capture_output=True, check=True)
            pair = (result.stdout, result.stderr)
            assert pair == (old_artifact.read_bytes(), old_report.read_bytes()), name
            if first is not None: assert pair == first
            first = pair
            put(PACKET / 'equality' / name / f'run-{i}.{fmt}', result.stdout)
            put(PACKET / 'equality' / name / f'run-{i}-report.json', result.stderr)
        evidence.append({'id': name, 'format': fmt, 'fresh_process_runs': 2, 'request_args': args, 'artifact_sha256': sha(first[0]), 'report_sha256': sha(first[1]), 'old_artifact': desc(ROOT, str(old_artifact.relative_to(ROOT))), 'old_report': desc(ROOT, str(old_report.relative_to(ROOT))), 'artifact_and_complete_report_byte_identical': True})
    for fmt in ['svg', 'png', 'pdf']:
        assert (ROOT / V8 / f'native.{fmt}').read_bytes() == (ROOT / V7 / f'native.{fmt}').read_bytes()
        assert (ROOT / V8 / f'native-{fmt}-report.json').read_bytes() == (ROOT / V7 / f'native-{fmt}-report.json').read_bytes()
        replay('representative-' + fmt, (ROOT / V7 / 'scene.json').read_bytes(), fmt, load(ROOT / V8 / 'manifest.json')['request_args'], ROOT / V7 / f'native.{fmt}', ROOT / V7 / f'native-{fmt}-report.json')
    replay('protected-pdf', (ROOT / 'tests/conformance/pdf-backend-qualification/project-operations.excalidraw').read_bytes(), 'pdf', ['--pdf-raster-fallback', 'forbid'], ROOT / PDF / 'candidate.pdf', ROOT / PDF / 'candidate-report.json')
    for row in load(OLD / 'cases.json'):
        name = row['case'].removeprefix('focused.').removesuffix('.v1')
        manifest = load(ROOT / row['path'] / 'manifest.json')
        scene = (ROOT / row['path'] / manifest['scene']['path']).read_bytes()
        for fmt in ['svg', 'png']:
            replay(name + '-' + fmt, scene, fmt, ['--compatibility-revision', '1', '--padding', '10'], OLD / 'native' / name / f'native.{fmt}', OLD / 'native' / name / f'{fmt}-report.json')
    for viewer in ['pdfium', 'pdfjs']:
        for i in range(2):
            output = PACKET / 'equality/protected-pdf' / f'{viewer}-{i}.rgba'
            result = run(['bash', f'tools/pdf-harness/run-{viewer}.sh', 'render', PACKET / 'equality/protected-pdf/run-0.pdf', output, '520', '200'])
            assert output.read_bytes() == (ROOT / PDF / f'candidate-{viewer}.rgba').read_bytes()
            put(output.with_suffix('.runtime.json'), result)
    run(['bash', 'tools/pdf-harness/run-structure.sh', 'inspect', PACKET / 'equality/protected-pdf/run-0.pdf', ROOT / 'tests/conformance/pdf-backend-qualification/project-operations-structure.json', 'pass', PACKET / 'equality/protected-pdf/structure.json'])
    for capture in load(ROOT / V8 / 'viewer-evidence/collection.json')['captures']:
        name = Path(capture['raster']['path']).name
        assert (ROOT / capture['raster']['path']).read_bytes() == (ROOT / V7 / 'viewer-evidence' / name).read_bytes()
    put(PACKET / 'output-equality.json', pretty({'build': build, 'comparisons': evidence, 'comparisons_count': len(evidence), 'all_old_new_bytes_and_reports_equal': True, 'protected_viewer_pairs_equal': True, 'v8_viewers_equal_v7': True}))
    check_frozen()
    print('16 output/report comparisons, 32 fresh release runs: exact equality. Protected PDF structure and both repeated viewers pass; V8 viewer bytes equal V7.')

def historical():
    # Run the unmodified preservation audit in a disposable before-source view.
    # This is explicitly historical validation; current production equality is
    # independently checked above, and no live file or approval is repaired.
    shadow = ROOT / 'target/stage4-format-historical-view'
    shadow.mkdir()
    for p, expected in load(PACKET / 'initial-live.json').items():
        data = (ROOT / p).read_bytes()
        if p in FILES + [TOOL]: data = (PACKET / 'before' / p).read_bytes()
        assert sha(data) == expected, p
        put(shadow / p, data)
    gitdir = run(['git', 'rev-parse', '--absolute-git-dir']).decode().strip()
    env = dict(ENV, GIT_DIR=gitdir, GIT_WORK_TREE=str(shadow))
    result = subprocess.run(['node', str(shadow / 'tools/reference-oracle/scripts/verify-png-stage4.mjs')], cwd=shadow, env=env, capture_output=True)
    put(PACKET / 'checks/historical-preservation.stdout', result.stdout)
    put(PACKET / 'checks/historical-preservation.stderr', result.stderr)
    assert result.returncode == 0, result.stderr.decode()
    print('Historical protected-PDF, six-case transition and ownership preservation audit passed against authenticated before-source bytes; live files untouched.')

def migration():
    check_frozen()
    old_ledger = load(ROOT / 'tests/conformance/coverage/ownership.json')
    catalog = json.loads(xtask('ownership-catalog'))
    assert sha(canonical(catalog)) == old_ledger['catalog_sha256']
    put(PACKET / 'checks/unchanged-catalog.json', pretty(catalog))
    assert (ROOT / 'tests/conformance/coverage/ownership.json').read_bytes() == (PACKET / 'before/tests/conformance/coverage/ownership.json').read_bytes()
    manifest = load(ROOT / V8 / 'manifest.json')
    assert manifest['approval'] is None
    subject = (PACKET / 'checks/subject-v8.stdout').read_text().strip()
    old_registry = load(ROOT / 'tests/conformance/candidates/registry.json')
    registry = copy.deepcopy(old_registry)
    row = registry['cases'][0]
    row.update(id=manifest['case_id'], directory=V8, status='pending-formatting-requalification-approval', execution_keys=[manifest['case_id'] + ':default:' + fmt for fmt in ['svg', 'png', 'pdf']], consumer=['cargo', 'run', '--release', '--locked', '-p', 'xtask', '--', 'conformance', 'candidate-verify', V8], exceptions=manifest['exception_proposal']['exceptions'])
    registry_bytes = pretty(registry)
    put(PACKET / 'pending/registry.json', registry_bytes)
    ledger = copy.deepcopy(old_ledger)
    changes = []
    for assignment in ledger['assignments']:
        old = copy.deepcopy(assignment)
        owner = assignment['disposition'].get('owner', {})
        if owner.get('kind') == 'test' and owner['source']['path'] in FILES:
            owner['source']['sha256'] = sha((ROOT / owner['source']['path']).read_bytes())
            changes.append({'id': assignment['id'], 'before': old, 'after_template': copy.deepcopy(assignment)})
    assert len(changes) == 57
    assert sum(r['after_template']['disposition']['owner']['source']['path'].endswith('/png_stage4_contract.rs') for r in changes) == 55
    selected = next(r for r in ledger['assignments'] if r['id'] == 'interaction:frames-text-assets')
    old_rep = copy.deepcopy(selected)
    assert selected['disposition']['owner']['kind'] == 'representative'
    assert selected['disposition']['owner']['binding']['case'] == 'representative.stage4-frames-text-assets.v7'
    selected['disposition']['owner']['binding'] = {'case': manifest['case_id'], 'registry': {'path': 'tests/conformance/candidates/registry.json', 'sha256': sha(registry_bytes)}, 'manifest': {'path': V8 + '/manifest.json', 'sha256': sha((ROOT / V8 / 'manifest.json').read_bytes())}, 'approval_subject_sha256': subject, 'command': row['consumer']}
    selected['disposition']['rationale'] = f'Formatting-only release successor {manifest["case_id"]}, subject {subject}; apply only with genuine user approval. Prior approved selection: ' + old_rep['disposition']['rationale']
    put(PACKET / 'pending/ownership.template-not-live.json', pretty(ledger))
    record = {'schema': 'excalidraw-render/formatting-owner-migration/v1', 'status': 'pending-user-approval-not-live', 'approval': None, 'before_ledger': desc(PACKET, 'before/tests/conformance/coverage/ownership.json'), 'before_registry': desc(PACKET, 'before/tests/conformance/candidates/registry.json'), 'test_source_changes': changes, 'representative_before': old_rep, 'representative_after_template': selected, 'representative_subject_sha256': subject, 'registry_template': desc(PACKET, 'pending/registry.json'), 'ledger_template': desc(PACKET, 'pending/ownership.template-not-live.json'), 'parameterized_field': 'Only the representative binding.manifest.sha256 must be derived again after recording the real V8 approval; all other template values are exact.', 'preserved_counts': {'assignments': 1874, 'owned': 1800, 'excluded': 3, 'assigned_or_excluded': 1803, 'gaps': 71}, 'catalog_sha256_unchanged': old_ledger['catalog_sha256'], 'inventory_and_168_full_keys_unchanged': True, 'six_focused_case_approvals_unchanged': True, 'global_ledger_approval': None}
    put(PACKET / 'migration-plan.json', pretty(record))
    validate_migration(False)
    print('Prospective migration verified: exactly 57 test-source bindings and one V7→V8 owner replacement; no live assignment, inventory, catalog, criteria or exclusion changes.')

def validate_migration(applied=False):
    plan = load(PACKET / 'migration-plan.json')
    old = load(PACKET / 'before/tests/conformance/coverage/ownership.json')
    template = load(PACKET / 'pending/ownership.template-not-live.json')
    registry = load(PACKET / 'pending/registry.json')
    v8 = load(ROOT / V8 / 'manifest.json')
    row = registry['cases'][0]
    assert len(registry['cases']) == 1 and row['approval_required'] is True
    assert row['id'] == v8['case_id'] and row['directory'] == V8
    assert row['coverage'] == ['interaction:frames-text-assets'] and row['formats'] == ['svg', 'png', 'pdf']
    assert row['execution_keys'] == [v8['case_id'] + ':default:' + f for f in row['formats']]
    assert row['consumer'] == ['cargo', 'run', '--release', '--locked', '-p', 'xtask', '--', 'conformance', 'candidate-verify', V8]
    assert row['exceptions'] == v8['exception_proposal']['exceptions']
    expected = copy.deepcopy(old)
    entries = {r['id']: r for r in expected['assignments']}
    assert len(plan['test_source_changes']) == 57
    for change in plan['test_source_changes']:
        assert entries[change['id']] == change['before']
        new = copy.deepcopy(change['after_template'])
        source = new['disposition']['owner']['source']
        assert source['path'] in FILES[-2:] and source['sha256'] == sha((ROOT / source['path']).read_bytes())
        new['disposition']['owner']['source'] = change['before']['disposition']['owner']['source']
        assert new == change['before']
        entries[change['id']].update(change['after_template'])
    assert entries['interaction:frames-text-assets'] == plan['representative_before']
    entries['interaction:frames-text-assets'].update(plan['representative_after_template'])
    assert expected == template
    binding = entries['interaction:frames-text-assets']['disposition']['owner']['binding']
    assert binding['registry'] == {'path': 'tests/conformance/candidates/registry.json', 'sha256': sha((PACKET / 'pending/registry.json').read_bytes())}
    assert binding['case'] == v8['case_id'] and binding['command'] == row['consumer']
    assert binding['approval_subject_sha256'] == (PACKET / 'checks/subject-v8.stdout').read_text().strip()
    assert template['catalog_sha256'] == old['catalog_sha256'] and template['approval'] is None
    assert len(template['assignments']) == 1874
    assert sum(r['disposition']['status'] == 'owned' for r in template['assignments']) == 1800
    assert sum(r['disposition']['status'] == 'gap' for r in template['assignments']) == 71
    assert sha(canonical(load(PACKET / 'checks/unchanged-catalog.json'))) == old['catalog_sha256']
    assert [r for r in template['assignments'] if r['disposition']['status'] == 'excluded'] == [r for r in old['assignments'] if r['disposition']['status'] == 'excluded']
    for p in ['inventory.json', 'requirements.json']:
        assert (ROOT / 'tests/conformance/coverage' / p).read_bytes() == (PACKET / 'before/tests/conformance/coverage' / p).read_bytes()
    if applied:
        assert v8['approval'] is not None
        binding['manifest']['sha256'] = sha((ROOT / V8 / 'manifest.json').read_bytes())
        assert load(ROOT / 'tests/conformance/coverage/ownership.json') == expected
        assert (ROOT / 'tests/conformance/candidates/registry.json').read_bytes() == (PACKET / 'pending/registry.json').read_bytes()
    else:
        assert v8['approval'] is None
        assert binding['manifest'] == {'path': V8 + '/manifest.json', 'sha256': sha((ROOT / V8 / 'manifest.json').read_bytes())}
        assert load(ROOT / 'tests/conformance/coverage/ownership.json') == old
        assert (ROOT / 'tests/conformance/candidates/registry.json').read_bytes() == (PACKET / 'before/tests/conformance/candidates/registry.json').read_bytes()

def boundary():
    prefix = ['cargo', 'run', '--release', '--locked', '-p', 'xtask', '--', 'conformance']
    for name, args, needle in [('pending-v8', ['candidate-verify', V8], 'approval must be an object'), ('historical-v7-stale', ['candidate-approval-subject', V7], 'stale'), ('historical-pdf-stale', ['pdf-requalification-subject', PDF], 'stale'), ('live-ledger-stale', ['verify-ownership'], 'stale')]:
        result = subprocess.run(prefix + args, cwd=ROOT, env=ENV, capture_output=True)
        put(PACKET / 'checks' / (name + '.stdout'), result.stdout)
        put(PACKET / 'checks' / (name + '.stderr'), result.stderr)
        assert result.returncode and needle in result.stderr.decode(), result.stderr.decode()
        assert not result.stdout
        print(name + ': correctly blocked', flush=True)
    log('candidate-negative-bindings', ['cargo', 'test', '--locked', '-p', 'xtask', '--bin', 'xtask', 'conformance_candidate', '--', '--nocapture'])
    log('inventory', prefix + ['verify-inventory'])
    for row in load(OLD / 'cases.json'):
        log(row['case'] + '-approval', prefix + ['verify', row['case']])

def seal():
    check_frozen()
    validate_migration(False)
    v8_subject = (PACKET / 'checks/subject-v8.stdout').read_text().strip()
    assert xtask('candidate-approval-subject', V8).decode().strip() == v8_subject
    dependencies = []
    for directory in [V8, BRIDGE]:
        for path in sorted((ROOT / directory).rglob('*')):
            if path.is_file() and path.name != 'README.md': dependencies.append(desc(ROOT, str(path.relative_to(ROOT))))
    put(PACKET / 'new-evidence-bindings.json', pretty(dependencies))
    files = [desc(PACKET, str(p.relative_to(PACKET))) for p in sorted(PACKET.rglob('*')) if p.is_file() and p.name != 'README.md' and '__pycache__' not in p.parts]
    manifest = {'schema': 'excalidraw-render/formatting-requalification/v1', 'status': 'pending-user-approval', 'approval': None, 'originating_issue': 'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85', 'representative_case': 'representative.stage4-frames-text-assets.v8', 'representative_subject_sha256': v8_subject, 'prior_representative_subject_sha256': 'sha256:c314cff42bdaac7fae932c4e7e4c8a9dc278b69a7077d9e4de37bc5cddf2fa05', 'prior_pdf_subject_sha256': 'sha256:c15e6bab141ac8080bac099932e5561bfcbee4c000f4957fec71e87d5e573389', 'prior_six_case_subject_sha256': 'sha256:3074ae5df1aa0c91743c68f4217f066370698f3d8970bac02dec5b1aacc00e84', 'source_transitions': load(PACKET / 'source-transitions.json'), 'output_equality': desc(PACKET, 'output-equality.json'), 'migration': desc(PACKET, 'migration-plan.json'), 'new_evidence_bindings': dependencies, 'files': files}
    bundle = {'schema': 'excalidraw-render/conformance-approval-subject/v1', 'manifest': {k: v for k, v in manifest.items() if k != 'approval'}, 'files': files}
    put(PACKET / 'manifest.json', canonical(manifest) + b'\n')
    put(PACKET / 'approval-subject.json', canonical(bundle) + b'\n')
    print('Formatting/migration subject: sha256:' + sha(canonical(bundle)))
    print('Representative subject: ' + v8_subject)

def verify(applied=False):
    manifest = load(PACKET / 'manifest.json')
    bundle = load(PACKET / 'approval-subject.json')
    assert manifest['approval'] is None
    assert bundle == {'schema': 'excalidraw-render/conformance-approval-subject/v1', 'manifest': {k: v for k, v in manifest.items() if k != 'approval'}, 'files': manifest['files']}
    for d in manifest['files']: assert desc(PACKET, d['path']) == d, d['path']
    for d in manifest['new_evidence_bindings']:
        if applied and d['path'] == V8 + '/manifest.json': continue
        assert desc(ROOT, d['path']) == d, d['path']
    check_frozen(applied)
    for row in manifest['source_transitions']:
        before = (PACKET / row['before']['path']).read_bytes()
        after = (PACKET / row['after']['path']).read_bytes()
        assert (ROOT / row['path']).read_bytes() == after
        if row['kind'] == 'pinned-rustfmt-output-only': assert run([RUST + '/rustfmt', '--edition', '2021', '--emit', 'stdout'], data=before) == after
        elif row['path'] == TOOL: assert after == before.replace(b'stage4-pdf-colorspace-bridge-v1', b'stage4-pdf-colorspace-bridge-v2')
        else: assert sha(before) == EXTERNAL_OLD
    for row in load(PACKET / 'output-equality.json')['comparisons']:
        old_artifact, old_report = row['old_artifact'], row['old_report']
        assert desc(ROOT, old_artifact['path']) == old_artifact and desc(ROOT, old_report['path']) == old_report
        for i in range(2):
            assert (PACKET / 'equality' / row['id'] / f'run-{i}.{row["format"]}').read_bytes() == (ROOT / old_artifact['path']).read_bytes()
            assert (PACKET / 'equality' / row['id'] / f'run-{i}-report.json').read_bytes() == (ROOT / old_report['path']).read_bytes()
    validate_migration(applied)
    assert xtask('candidate-approval-subject', V8).decode().strip() == manifest['representative_subject_sha256']
    print('Verified formatting subject sha256:' + sha(canonical(bundle)))
    print('Verified ' + manifest['representative_subject_sha256'] + ('; approved migration applied.' if applied else '; V8 approval null and live ledger unchanged/stale.'))

def apply():
    # Future metadata operation, only after actual user approval of BOTH subjects.
    # Input is a real four-field formatting-subject approval record, outside the
    # immutable packet. This command is never run during pending preparation.
    verify(False)
    supplied = load(Path(sys.argv[2]))
    subject = 'sha256:' + sha(canonical(load(PACKET / 'approval-subject.json')))
    assert set(supplied) == {'approved_by', 'approved_at', 'approval_url', 'approval_subject_sha256'}
    assert supplied['approval_subject_sha256'] == subject
    match = re.fullmatch(r'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-(\d+)', supplied['approval_url'])
    assert match
    comment = json.loads(run(['gh', 'api', 'repos/sagikazarmark/excalidraw-render.orig/issues/comments/' + match[1]]))
    v8_subject = load(PACKET / 'manifest.json')['representative_subject_sha256']
    assert supplied['approved_by'] == comment['user']['login'] and supplied['approved_at'] == comment['created_at'][:10]
    assert subject in comment['body'] and v8_subject in comment['body']
    assert supplied['approval_url'] == comment['html_url']
    manifest = load(ROOT / V8 / 'manifest.json')
    manifest['approval'] = dict(supplied, approval_subject_sha256=v8_subject)
    manifest_bytes = canonical(manifest) + b'\n'
    registry_bytes = (PACKET / 'pending/registry.json').read_bytes()
    ledger = load(PACKET / 'pending/ownership.template-not-live.json')
    representative = next(r for r in ledger['assignments'] if r['id'] == 'interaction:frames-text-assets')
    representative['disposition']['owner']['binding']['manifest']['sha256'] = sha(manifest_bytes)
    files = {V8 + '/manifest.json': manifest_bytes, 'tests/conformance/candidates/registry.json': registry_bytes, 'tests/conformance/coverage/ownership.json': pretty(ledger)}
    previous = {p: (ROOT / p).read_bytes() for p in files}
    application = ROOT / 'tests/conformance/coverage/stage4-format-application-v1'
    assert not application.exists()
    try:
        for p, data in files.items(): (ROOT / p).write_bytes(data)
        for args in [('candidate-verify', V8), ('candidate-executions',), ('verify-representative-owner', manifest['case_id']), ('verify-inventory',), ('verify-ownership',)]:
            output = xtask(*args)
            put(application / (args[0] + '.json'), output)
        verify(True)
        put(application / 'approval.json', pretty(supplied))
        put(application / 'comment.json', pretty(comment))
    except BaseException:
        for p, data in previous.items(): (ROOT / p).write_bytes(data)
        raise
    print('Applied exactly the approved V8/57-source-owner migration; global approval remains null.')

if __name__ == '__main__':
    {'archive': archive, 'formatted': formatted, 'quality': quality, 'freeze': freeze, 'capture': capture, 'equality': equality, 'historical': historical, 'migration': migration, 'boundary': boundary, 'seal': seal, 'verify': verify, 'verify-applied': lambda: verify(True), 'apply': apply}[sys.argv[1]]()
