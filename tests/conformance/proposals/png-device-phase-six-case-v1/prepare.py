#!/usr/bin/env python3
"""Prepare in an isolated copy; never apply protected files or grant approval."""
import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
STAGE = Path('/tmp/opencode/png-six-case-stage-v1')
PROBE = Path('/tmp/opencode/six-phase-probe')
CASES = ['embeddable-links', 'emoji-fallback', 'bidi-text', 'bidi-alignment', 'text-line-endings', 'text-alignment-vertical']
SUBJECT = 'c314cff42bdaac7fae932c4e7e4c8a9dc278b69a7077d9e4de37bc5cddf2fa05'
ENV = dict(os.environ, PATH='/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:' + os.environ['PATH'], CARGO_INCREMENTAL='0', CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_PROFILE_RELEASE_DEBUG='0')
def sha(data): return hashlib.sha256(data).hexdigest()
def read(path): return json.loads(path.read_bytes())
def canonical(value): return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as output: output.write(data)
def write_json(path, value): put(path, json.dumps(value, indent=2, ensure_ascii=False).encode() + b'\n')
def desc(root, path):
    data = (root / path).read_bytes()
    return {'path': str(path), 'byte_length': len(data), 'sha256': sha(data)}
def run(args, cwd=ROOT):
    result = subprocess.run(list(map(str, args)), cwd=cwd, env=ENV, capture_output=True)
    if result.returncode: raise RuntimeError(f'{args}: {result.stderr.decode()}')
    return result.stdout
def xtask(command, stage=False):
    root = STAGE if stage else ROOT
    args = ['cargo', 'run', '--locked', '-p', 'xtask', '--', 'conformance', *command]
    return run(args, root)

RULE = 'Under the user-approved spec §8.5 per-glyph device-phase rule, these axis-aligned glyphs retain the whole-device-Y baseline and use quarter-device-pixel X origins; this replaces the former whole-line-only baseline placement. '
REASONS = {
 'embeddable-links': 'Sub-pixel glyph coverage and the existing #105 Helvetica-metadata/Liberation Sans substitution. ' + RULE + 'SVG path geometry, scene extent and document parity remain correct. The residual is interior label-glyph coverage between the rasterizers, including the pinned reference system Liberation Sans light autohinting/x-height behavior. The new exact limits are independently remeasured; they do not attribute the residual to obsolete whole-line placement.',
 'emoji-fallback': 'Existing #121 monochrome emoji compatibility substitution: bundled Noto Emoji 3.003 replaces the reference Noto Color Emoji 2.047. Their glyph shapes and advances differ and move following text. ' + RULE + 'The temporary placement-only control reproduces the former measurement exactly, and current native output reproduces the new measurement. The fallback/advance difference remains the non-defect cause; per-glyph ranges and fallback-run partitions are intact.',
 'bidi-text': 'Existing #152/#124/#125 deterministic missing-glyph compatibility substitution: unsupported Hebrew/Arabic and paragraph-separator scalars use the basic-Latin question mark from Liberation Sans instead of the reference fallback glyphs. Different advances displace adjacent text; bidi run order and paragraph behavior remain as specified. ' + RULE + 'A placement-only temporary control reproduces old/new measurements with unchanged SVG output and verified glyph/fallback/bidi outline partitions.',
 'bidi-alignment': 'Existing #152/#124/#125 deterministic basic-Latin question-mark fallback and its different advances for unsupported RTL scalars. Physical line anchoring and bidi order remain as specified. ' + RULE + 'The separate #168 SVG harness direction/paragraph limitation and its measured SVG pin remain unchanged. The direct-PNG limits are remeasured only for the approved per-glyph placement, with unchanged SVG output and verified outline/run partitions.',
}

def initialize():
    assert not STAGE.exists(), 'use a fresh staging directory'
    paths = run(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard']).decode().split('\0')
    paths = sorted({p for p in paths if p and not p.startswith(str(OUT.relative_to(ROOT)) + '/')})
    frozen = {}
    for p in paths:
        source = ROOT / p
        if source.is_file():
            frozen[p] = sha(source.read_bytes())
            target = STAGE / p
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
    write_json(OUT / 'live-before.json', frozen)
    put(OUT / 'before-catalog.json', xtask(['ownership-catalog']))
    for name in ['measurements.json', 'source-bindings.json']:
        put(OUT / 'diagnosis' / name, (PROBE / name).read_bytes())
    for case in CASES:
        for mode in ['current', 'legacy']:
            put(OUT / 'diagnosis' / f'{case}-{mode}.rgba', (PROBE / f'{case}-{mode}.rgba').read_bytes())
    for name in ['Cargo.toml', 'Cargo.lock', 'renderer/Cargo.toml', 'renderer/examples/six_phase_probe.rs', 'renderer/src/render/png.rs']:
        put(OUT / 'diagnosis' / 'probe' / name, (PROBE / name).read_bytes())
    for p, digest in read(PROBE / 'source-bindings.json').items(): assert sha((ROOT / p).read_bytes()) == digest
    # Retain the sole production-source diagnosis delta, including assertions.
    diff = subprocess.run(['git', 'diff', '--no-index', '--', str(ROOT / 'crates/excalidraw-render/src/render/png.rs'), str(PROBE / 'renderer/src/render/png.rs')], capture_output=True)
    assert diff.returncode == 1
    put(OUT / 'diagnosis' / 'placement-only.diff', diff.stdout)
    for p in (ROOT / 'crates/excalidraw-render/src').rglob('*.rs'):
        other = PROBE / 'renderer/src' / p.relative_to(ROOT / 'crates/excalidraw-render/src')
        if p.name == 'png.rs' and p.parent.name == 'render': continue
        assert p.read_bytes() == other.read_bytes(), p
    registry_path = 'tests/conformance/cases/registry.json'
    registry = read(ROOT / registry_path)
    summaries = []
    for case in CASES:
        entry = next(e for e in registry['cases'] if e['id'] == f'focused.{case}.v1')
        directory = entry['path']
        manifest = read(ROOT / directory / 'manifest.json')
        before = copy.deepcopy(manifest)
        assert len(manifest['variants']) == 1 and manifest['variants'][0]['id'] == 'default'
        assert len(manifest['exceptions']) == 1
        old = manifest['exceptions'][0]
        assert (old['variant'], old['format'], old['region'], old['viewer_profile']) == ('default', 'png', 'whole', None)
        measurements = [m for m in read(PROBE / 'measurements.json') if m['case'] == case]
        current = next(m for m in measurements if m['mode'] == 'current')
        legacy = next(m for m in measurements if m['mode'] == 'legacy')
        assert old['max_changed_pixels'] == legacy['changed'] and old['minimum_ssim_bits'] == legacy['ssim_bits']
        if case in REASONS:
            new = dict(old, max_changed_pixels=current['changed'], minimum_ssim_bits=current['ssim_bits'], reason=REASONS[case])
            manifest['exceptions'] = [new]
        else:
            assert current['canonical'] is True
            manifest['exceptions'] = []
        entry['exceptions'] = manifest['exceptions']
        manifest['approval'] = None
        encoded = canonical(manifest)
        (STAGE / directory / 'manifest.json').write_bytes(encoded + b'\n')
        (STAGE / directory / 'manifest.sha256').write_text(sha(encoded) + '\n')
        put(OUT / 'proposed' / directory / 'manifest.json', encoded + b'\n')
        put(OUT / 'proposed' / directory / 'manifest.sha256', (sha(encoded) + '\n').encode())
        summaries.append({'case': entry['id'], 'path': directory, 'approving_issue': entry['approving_issue'], 'old_approval': before['approval'], 'old_exception': old, 'proposed_exceptions': manifest['exceptions'], 'legacy': legacy, 'current': current})
    (STAGE / registry_path).write_text(json.dumps(registry, indent=2, ensure_ascii=False) + '\n')
    put(OUT / 'proposed' / registry_path, (STAGE / registry_path).read_bytes())
    write_json(OUT / 'cases.json', summaries)
    print(f'Isolated {len(frozen)} files at {STAGE}; staged six proposals without approvals.')

def subjects():
    rows = read(OUT / 'cases.json')
    for row in rows:
        subject = xtask(['approval-subject', row['case']], True).decode().strip()
        row['prospective_approval_subject_sha256'] = subject
        print(row['case'], subject, flush=True)
    write_json(OUT / 'prospective-subjects.json', rows)

def preserve():
    before = read(OUT / 'live-before.json')
    changed = [p for p, h in before.items() if not (ROOT / p).is_file() or sha((ROOT / p).read_bytes()) != h]
    assert not changed, changed
    actual = xtask(['candidate-approval-subject', 'tests/conformance/candidates/stage4-frames-text-assets-v7']).decode().strip()
    assert actual == 'sha256:' + SUBJECT
    print(f'All {len(before)} pre-existing files unchanged; approved v7 subject unchanged.')

def capture():
    build_env = dict(ENV, CARGO_PROFILE_RELEASE_OPT_LEVEL='3', CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS='false', CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS='false', CARGO_PROFILE_RELEASE_CODEGEN_UNITS='1', CARGO_PROFILE_RELEASE_LTO='true', CARGO_PROFILE_RELEASE_PANIC='abort', RUSTFLAGS='')
    for key in ['CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTC', 'CARGO_BUILD_RUSTC']: build_env.pop(key, None)
    command = ['cargo', 'build', '--release', '--locked', '-p', 'excalidraw-render-cli', '--message-format=json-render-diagnostics']
    result = subprocess.run(command, cwd=ROOT, env=build_env, capture_output=True, check=True)
    artifact = next(v for line in result.stdout.splitlines() if (v := json.loads(line)).get('reason') == 'compiler-artifact' and v['target']['name'] == 'excalidraw-render')
    executable = Path(artifact['executable'])
    v7 = read(ROOT / 'tests/conformance/candidates/stage4-frames-text-assets-v7/native-capture.json')
    assert sha(executable.read_bytes()) == v7['build']['executable_sha256']
    assert artifact['profile'] == v7['build']['cargo_profile']
    for d in v7['producer_sources']: assert d == desc(ROOT, d['path'])
    build = dict(v7['build'], verification_build_command=command, compiler_artifact=artifact)
    write_json(OUT / 'native-build.json', build)
    write_json(OUT / 'native-producers.json', v7['producer_sources'])
    rows = []
    for row in read(OUT / 'cases.json'):
        case = row['case'].removeprefix('focused.').removesuffix('.v1')
        directory = ROOT / row['path']
        manifest = read(directory / 'manifest.json')
        scene = (directory / manifest['scene']['path']).read_bytes()
        options = manifest['variants'][0]['options']
        assert options == {'exportBackground': True, 'exportEmbedScene': False, 'exportPadding': 10, 'exportScale': 1, 'exportWithDarkMode': False, 'frameRendering': {'clip': True, 'enabled': True, 'name': True, 'outline': True}, 'maxWidthOrHeight': None, 'viewBackgroundColor': '#ffffff'}, options
        for fmt in ['png', 'svg']:
            wanted = read(directory / manifest['variants'][0]['formats'][fmt]['expected_report']['path'])
            first = None
            for repeat in range(2):
                args = ['-', '--output', '-', '--format', fmt, '--report-format', 'json', '--compatibility-revision', '1', '--padding', '10']
                result = subprocess.run([str(executable), *args], input=scene, capture_output=True, env=ENV, check=True)
                report = json.loads(result.stderr)
                expected = {'schema_version': 1, 'status': 'success', 'renderer_version': '0.1.0', 'compatibility_revision': 1, 'excalidraw_release': 'v0.18.1', 'reference_profile_digest': 'sha256:1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71', 'format': fmt, 'artifact': {'media_type': 'image/png' if fmt == 'png' else 'image/svg+xml', 'byte_length': len(result.stdout), 'sha256': 'sha256:' + sha(result.stdout)}, 'error': None, 'diagnostics': wanted['diagnostics']}
                assert report == expected, case
                if fmt == 'svg': assert sha(result.stdout) == row['current']['svg_sha256'] == row['legacy']['svg_sha256']
                pair = (result.stdout, result.stderr)
                if first is not None: assert pair == first
                first = pair
                suffix = '.repeat' if repeat else ''
                put(OUT / 'native' / case / f'native{suffix}.{fmt}', result.stdout)
                put(OUT / 'native' / case / f'{fmt}{suffix}-report.json', result.stderr)
            rows.append({'case': row['case'], 'format': fmt, 'request_args': args, 'fresh_process_runs': 2, 'artifact': desc(OUT, f'native/{case}/native.{fmt}'), 'report': desc(OUT, f'native/{case}/{fmt}-report.json')})
            print(case, fmt, 'two fresh release processes/artifacts/reports repeat', flush=True)
    write_json(OUT / 'native-capture.json', rows)

def migration():
    rows = read(OUT / 'prospective-subjects.json')
    affected = {r['path'] + '/manifest.json' for r in rows}
    registry_path = 'tests/conformance/cases/registry.json'
    reqpath = 'tests/conformance/coverage/requirements.json'
    req = read(ROOT / reqpath)
    requirement_changes = []
    for requirement in req['requirements']:
        for evidence in requirement['evidence']:
            p = evidence['source']['path']
            actual = sha((STAGE / p).read_bytes())
            if actual != evidence['source']['sha256']:
                assert p in affected | {registry_path}, p
                assert evidence['anchor'] in (STAGE / p).read_text()
                requirement_changes.append({'id': requirement['id'], 'before': copy.deepcopy(evidence['source']), 'after': {'path': p, 'sha256': actual}})
                evidence['source']['sha256'] = actual
    if requirement_changes:
        (STAGE / reqpath).write_text(json.dumps(req, indent=2, ensure_ascii=False) + '\n')
        put(OUT / 'proposed' / reqpath, (STAGE / reqpath).read_bytes())
    invpath = 'tests/conformance/coverage/inventory.json'
    inventory = json.loads(xtask(['inventory'], True))
    (STAGE / invpath).write_text(json.dumps(inventory, indent=2, ensure_ascii=False) + '\n')
    put(OUT / 'preview' / 'inventory.pending-approval.json', (STAGE / invpath).read_bytes())
    catalog = json.loads(xtask(['ownership-catalog'], True))
    write_json(OUT / 'preview' / 'catalog.pending-approval.json', catalog)
    before = read(OUT / 'before-catalog.json')
    ledger = read(ROOT / 'tests/conformance/coverage/ownership.json')
    assert sha(canonical(before)) == ledger['catalog_sha256']
    oldobs = {r['id']: r for r in before['obligations']}
    newobs = {r['id']: r for r in catalog['obligations']}
    retired = {f'exception:focused.{case}.v1:default:png:whole' for case in CASES[-2:]}
    assert oldobs.keys() - newobs.keys() == retired and not newobs.keys() - oldobs.keys()
    authorities = []
    for identity, new in newobs.items():
        old = oldobs[identity]
        normalized = dict(new, authority=old['authority'])
        assert normalized == old, identity
        if old != new:
            assert new['authority']['path'] in affected | {invpath}
            authorities.append({'id': identity, 'before': old['authority'], 'after_pending_approval': new['authority']})
    oldinv = read(ROOT / invpath)
    assert oldinv['coverage'] == inventory['coverage'] and len(inventory['executions']) == 168
    executions = []
    for old, new in zip(oldinv['executions'], inventory['executions']):
        normalized = copy.deepcopy(new)
        normalized['key']['inputs_sha256'] = old['key']['inputs_sha256']
        assert normalized == old, old['key']
        if new != old:
            assert new['key']['case'] in {r['case'] for r in rows}
            executions.append({'before': old['key'], 'after_pending_approval': new['key']})
    assert len(executions) == 18
    proposed_ledger = copy.deepcopy(ledger)
    selected, retired_rows = [], []
    assignments = []
    for assignment in proposed_ledger['assignments']:
        if assignment['id'] in retired:
            assert assignment['disposition']['status'] == 'gap'
            retired_rows.append({'obligation': oldobs[assignment['id']], 'assignment': assignment})
            continue
        owner = assignment['disposition'].get('owner', {})
        if owner.get('kind') == 'case' and owner['manifest']['path'] in affected:
            before_owner = copy.deepcopy(owner)
            owner['manifest']['sha256'] = sha((STAGE / owner['manifest']['path']).read_bytes())
            selected.append({'obligation': assignment['id'], 'before': before_owner, 'after_pending_approval': copy.deepcopy(owner)})
        assignments.append(assignment)
    proposed_ledger['assignments'] = assignments
    proposed_ledger['catalog_sha256'] = sha(canonical(catalog))
    assert len(assignments) == 1874 and sum(a['disposition']['status'] == 'gap' for a in assignments) == 71
    write_json(OUT / 'preview' / 'ownership.pending-approval.json', proposed_ledger)
    excluded = [a for a in ledger['assignments'] if a['disposition']['status'] == 'excluded']
    assert [a for a in assignments if a['disposition']['status'] == 'excluded'] == excluded
    sources = []
    for old, new in zip(before['sources'], catalog['sources']):
        assert old['path'] == new['path']
        if old != new:
            assert new['path'] in affected | {invpath}
            sources.append({'before': old, 'after_pending_approval': new})
    plan = {'schema': 'excalidraw-render/six-case-ownership-migration-plan/v1', 'status': 'review-required-not-applied', 'approval': None, 'before_ledger': desc(ROOT, 'tests/conformance/coverage/ownership.json'), 'before_inventory': desc(ROOT, invpath), 'old_catalog_sha256': ledger['catalog_sha256'], 'pending_approval_catalog_sha256': proposed_ledger['catalog_sha256'], 'retire_only_unowned_scopes': retired_rows, 'authority_changes': authorities, 'catalog_source_changes': sources, 'requirement_source_changes': requirement_changes, 'case_owner_manifest_changes': selected, 'execution_input_binding_changes': executions, 'preserved_exclusions': excluded, 'preserved_execution_identity_count': 168, 'unchanged_full_execution_records': 150, 'full_execution_keys_byte_identical': False, 'reason': 'Normal inventory keys include inputs_sha256 over the full manifest including approval. All 168 case/variant/export/format identities, options and references are preserved; exactly 18 input bindings must change for six manifests. Final hashes must be rederived after genuine case approval comments; pending previews are not final catalog authority.', 'final_application_constraints': ['No production, xtask, comparator or representative-v7 changes', 'Record real case approvals on each registered approving issue; do not reuse old subjects', 'Recompute exactly six final approved manifest descriptors and their 18 execution input hashes using unchanged normal inventory derivation', 'Retire exactly the two listed gaps; retain every other obligation identity, criterion, pointer and candidate', 'Refresh only enumerated inventory/manifest authorities and case-owner manifest bindings', 'Preserve all other owner selections, rationales, test sources and three approved exclusions', 'Verify all six approved cases, derived inventory/ownership and unchanged v7 subject before completion', 'Keep native baseline/#178 and global ledger approval pending'], 'counts_after': {'assignments': 1874, 'owned': 1800, 'excluded': 3, 'assigned_or_excluded': 1803, 'gaps': 71}}
    write_json(OUT / 'migration-plan.json', plan)
    print(f'Catalog 1876 -> 1874; retire 2 gaps; {len(authorities)} authority and {len(selected)} owner manifest bindings; 18 input digests require rebinding, 168 semantic identities preserved.')

def measure():
    command = ['cargo', 'run', '--release', '--locked', '--manifest-path', OUT / 'comparator/Cargo.toml', '--target-dir', ROOT / 'target/six-case-requalification-compare', '--', ROOT, OUT]
    result = run(command)
    put(OUT / 'verified-measurements.json', result)
    print('Verified 12 exact old/new comparisons; 6 effective current passes; current release pixels equal diagnosis pixels.')

def svg_check():
    relative = 'crates/excalidraw-render/tests/parity_registered_case_svg.rs'
    test = b'''\n#[test]
fn six_case_proposal_preserves_svg_geometry_raster_and_168_pin() {
    let selected = ["focused.embeddable-links.v1", "focused.emoji-fallback.v1", "focused.bidi-text.v1", "focused.bidi-alignment.v1", "focused.text-line-endings.v1", "focused.text-alignment-vertical.v1"];
    let variants: Vec<_> = registered_scene_variants(&repository_root()).into_iter().filter(|v| selected.contains(&v.case.as_str())).collect();
    assert_eq!(variants.len(), 6);
    for variant in variants {
        let errors = svg_divergences(&variant);
        assert!(errors.is_empty(), "{}: {:?}", variant.case, errors);
        println!("{}: geometry, SVG raster and direct-PNG dependency gates pass", variant.case);
    }
    let pin = &RASTERIZER_LIMITATIONS[0].pin;
    assert_eq!(pin.case, "focused.bidi-alignment.v1");
    assert_eq!(pin.changed_pixels, 1190);
    assert_eq!(pin.ssim_bits, 0x3fea_564b_9c97_ab4f);
}
'''
    assert (STAGE / relative).read_bytes() == (ROOT / relative).read_bytes()
    put(OUT / 'diagnosis/staged-six-svg-test.rs', test)
    (STAGE / relative).write_bytes((ROOT / relative).read_bytes() + test)
    command = ['cargo', 'test', '--locked', '-p', 'excalidraw-render', '--test', 'parity_registered_case_svg', 'six_case_proposal_preserves_svg_geometry_raster_and_168_pin', '--', '--exact', '--nocapture']
    put(OUT / 'svg-parity-check.log', run(command, STAGE))
    print('Six-case isolated SVG geometry/raster and PNG dependencies passed; #168 SVG pin retained exactly.')

def seal():
    rows = read(OUT / 'prospective-subjects.json')
    changes = []
    for row in rows:
        assert xtask(['approval-subject', row['case']], True).decode().strip() == row['prospective_approval_subject_sha256']
        for name in ['manifest.json', 'manifest.sha256']:
            path = row['path'] + '/' + name
            put(OUT / 'before' / path, (ROOT / path).read_bytes())
        live_manifest = read(ROOT / row['path'] / 'manifest.json')
        staged_manifest = read(STAGE / row['path'] / 'manifest.json')
        restored = dict(staged_manifest, approval=live_manifest['approval'], exceptions=live_manifest['exceptions'])
        assert restored == live_manifest
        changes.append({'case_id': row['case'], 'variant': 'default', 'format': 'png', 'region': 'whole', 'action': 'replace-exact-exception' if row['proposed_exceptions'] else 'withdraw-canonical-passing-exception', 'prospective_approval_subject_sha256': row['prospective_approval_subject_sha256'], 'normal_case_approving_issue': row['approving_issue'], 'before_manifest': desc(OUT, 'before/' + row['path'] + '/manifest.json'), 'proposed_manifest': desc(OUT, 'proposed/' + row['path'] + '/manifest.json'), 'proposed_exceptions': row['proposed_exceptions'], 'current_changed_pixels': row['current']['changed'], 'current_ssim_bits': row['current']['ssim_bits'], 'canonical_pass': row['current']['canonical']})
    for path in ['tests/conformance/cases/registry.json', 'tests/conformance/coverage/inventory.json', 'tests/conformance/coverage/ownership.json', 'tests/conformance/coverage/requirements.json']:
        put(OUT / 'before' / path, (ROOT / path).read_bytes())
    old_registry = read(ROOT / 'tests/conformance/cases/registry.json')
    new_registry = read(STAGE / 'tests/conformance/cases/registry.json')
    original = copy.deepcopy(new_registry)
    for old, new in zip(old_registry['cases'], original['cases']):
        assert old['id'] == new['id']
        if old['id'] in {r['case'] for r in rows}: new['exceptions'] = old['exceptions']
    assert original == old_registry
    bindings = {}
    for d in read(OUT / 'native-producers.json') + read(ROOT / 'tests/conformance/candidates/stage4-frames-text-assets-v7/manifest.json')['producer_sources']:
        assert d == desc(ROOT, d['path'])
        bindings[d['path']] = d
    for path in ['crates/excalidraw-render/tests/parity_registered_case_png.rs', 'crates/excalidraw-render/tests/parity_registered_case_svg.rs', 'crates/excalidraw-render/tests/common/registered_cases.rs', 'crates/excalidraw-render/tests/common/raster.rs', 'crates/excalidraw-render/tests/common/reports.rs', 'docs/research/stage4-png-device-phase-assessment.md']:
        bindings[path] = desc(ROOT, path)
    for p in (ROOT / 'crates/excalidraw-render-compare/src').rglob('*.rs'):
        path = str(p.relative_to(ROOT)); bindings[path] = desc(ROOT, path)
    bindings['crates/excalidraw-render-compare/Cargo.toml'] = desc(ROOT, 'crates/excalidraw-render-compare/Cargo.toml')
    for row in rows:
        for p in (ROOT / row['path']).rglob('*'):
            if p.is_file():
                path = str(p.relative_to(ROOT)); bindings[path] = desc(ROOT, path)
    files = [desc(OUT, str(p.relative_to(OUT))) for p in sorted(OUT.rglob('*')) if p.is_file() and p.name != 'README.md' and p.suffix != '.pyc' and '__pycache__' not in p.parts]
    manifest = {'schema': 'excalidraw-render/six-case-png-requalification-batch/v1', 'compatibility_revision': 1, 'originating_issue': 'https://github.com/sagikazarmark/excalidraw-render.orig/issues/85', 'status': 'ready-for-exact-batch-review-not-applied', 'approval': None, 'case_changes': changes, 'diagnosis': desc(OUT, 'DIAGNOSIS.md'), 'native_capture': desc(OUT, 'native-capture.json'), 'native_build': desc(OUT, 'native-build.json'), 'verified_measurements': desc(OUT, 'verified-measurements.json'), 'svg_dependency_verification': desc(OUT, 'svg-parity-check.log'), 'migration_plan': desc(OUT, 'migration-plan.json'), 'live_preapplication_bindings': sorted(bindings.values(), key=lambda d: d['path']), 'approved_v7_subject_sha256': 'sha256:' + SUBJECT, 'files': files}
    bundle = {'schema': 'excalidraw-render/conformance-approval-subject/v1', 'manifest': {k: v for k, v in manifest.items() if k != 'approval'}, 'files': files}
    put(OUT / 'batch-manifest.json', canonical(manifest) + b'\n')
    put(OUT / 'approval-subject.json', canonical(bundle) + b'\n')
    expected = 'sha256:' + sha(canonical(bundle))
    command = ['cargo', 'run', '--release', '--locked', '--manifest-path', OUT / 'comparator/Cargo.toml', '--target-dir', ROOT / 'target/six-case-requalification-compare', '--', 'canonical-sha256', OUT / 'approval-subject.json']
    assert run(command).decode().strip() == expected
    print(expected)
    for row in changes: print(row['case_id'], row['prospective_approval_subject_sha256'], 'normal approval issue #' + row['normal_case_approving_issue'])

def verify():
    manifest = read(OUT / 'batch-manifest.json')
    bundle = read(OUT / 'approval-subject.json')
    assert manifest['approval'] is None
    assert bundle == {'schema': 'excalidraw-render/conformance-approval-subject/v1', 'manifest': {k: v for k, v in manifest.items() if k != 'approval'}, 'files': manifest['files']}
    for d in manifest['files']: assert d == desc(OUT, d['path']), d['path']
    for d in manifest['live_preapplication_bindings']: assert d == desc(ROOT, d['path']), d['path']
    assert all(read(OUT / row['proposed_manifest']['path'])['approval'] is None for row in manifest['case_changes'])
    preserve()
    print(f'Verified {len(manifest["files"])} batch files and {len(manifest["live_preapplication_bindings"])} live bindings; sha256:{sha(canonical(bundle))}')

if __name__ == '__main__':
    {'initialize': initialize, 'subjects': subjects, 'preserve': preserve, 'capture': capture, 'migration': migration, 'measure': measure, 'svg-check': svg_check, 'seal': seal, 'verify': verify}[sys.argv[1]]()
