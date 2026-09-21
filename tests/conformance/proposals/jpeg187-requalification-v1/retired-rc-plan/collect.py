#!/usr/bin/env python3
"""Unapproved #187 blast-radius collection; never applies owners or baselines."""
import sys
if not __debug__ or sys.flags.optimize:
    raise SystemExit('Optimized Python is forbidden; no collection started.')

import argparse
import difflib
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tomllib

ROOT = Path(__file__).resolve().parents[4]
PACKET = Path(__file__).resolve().parent
OLD = Path('/tmp/opencode/jpeg187-old-4e5b624')
CURRENT = Path('/tmp/opencode/jpeg187-current-probe')
RUST = '/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin'
ENV = dict(os.environ, PATH=RUST + ':' + os.environ['PATH'], CARGO_INCREMENTAL='0',
           CARGO_PROFILE_DEV_DEBUG='0', CARGO_PROFILE_TEST_DEBUG='0', CARGO_PROFILE_RELEASE_DEBUG='0',
           CARGO_PROFILE_RELEASE_OPT_LEVEL='3', CARGO_PROFILE_RELEASE_CODEGEN_UNITS='1',
           CARGO_PROFILE_RELEASE_LTO='true', CARGO_PROFILE_RELEASE_PANIC='abort',
           CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS='false', CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS='false', RUSTFLAGS='')
for key in ['CARGO_ENCODED_RUSTFLAGS', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER', 'RUSTC', 'CARGO_BUILD_RUSTC', 'CARGO_TARGET_DIR']:
    ENV.pop(key, None)
PACKAGES = ['excalidraw-render-assets-data-000', 'excalidraw-render-assets-data-001', 'excalidraw-render-assets-base', 'excalidraw-render']

def require(condition, message):
    if not condition: raise ValueError(message)
def sha(data): return hashlib.sha256(data).hexdigest()
def read(path): return json.loads(path.read_bytes())
def canonical(value): return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()
def put(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('xb') as output: output.write(data)
def json_new(path, value): put(path, json.dumps(value, indent=2, ensure_ascii=False).encode() + b'\n')
def desc(root, path):
    data = (root / path).read_bytes()
    return {'path': str(path), 'sha256': sha(data), 'byte_length': len(data)}
def command(args, root=ROOT, log=None, allow_failure=False):
    output = subprocess.run(list(map(str, args)), cwd=root, env=ENV, capture_output=True)
    if log:
        put(PACKET / 'logs' / (log + '.stdout'), output.stdout)
        put(PACKET / 'logs' / (log + '.stderr'), output.stderr)
    if not allow_failure: require(output.returncode == 0, f'{args}: {output.stderr.decode()} {output.stdout.decode()}')
    return output
def tracked_state(root):
    paths = command(['git', 'ls-files', '-z'], root).stdout.decode().split('\0')
    return {p: sha((root / p).read_bytes()) for p in paths if p and (root / p).is_file()}
def package(root, label):
    require((root / 'target').is_dir() or not (root / 'target').exists(), 'unsafe target')
    result = command(['cargo', 'package', '--locked', '--workspace', '--exclude', 'xtask', '--exclude', 'excalidraw-render-compare', '--no-verify', '--allow-dirty'], root, label + '-package', True)
    if result.returncode:
        print(result.stderr.decode(), flush=True)
        return False
    for name in PACKAGES:
        archive = root / 'target/package' / (name + '-0.1.0.crate')
        require(archive.is_file(), 'missing package ' + name)
        with tarfile.open(archive) as tar:
            tar.extractall(root / 'target/package', filter='data')
    return True

def old_capture():
    require(command(['git', 'rev-parse', 'HEAD'], OLD).stdout.decode().strip().startswith('4e5b624'), 'wrong baseline commit')
    require(not command(['git', 'status', '--porcelain'], OLD).stdout, 'old worktree is dirty')
    before = tracked_state(OLD)
    json_new(PACKET / 'old/producer-files.json', before)
    require(package(OLD, 'old'), 'old packaging failed; inspect retained logs')
    capture(OLD, 'old')
    require(tracked_state(OLD) == before, 'old tracked sources changed during capture')
    print('Old HEAD packaged public-facade capture complete; no baseline approval.', flush=True)

def capture(root, label):
    destination = PACKET / label / 'snapshots'
    helper = root / 'tests/package-consumer'
    command(['cargo', 'test', '--release', '--locked', '--example', 'native_snapshots'], helper, label + '-collector-tests')
    command(['cargo', 'run', '--release', '--locked', '--example', 'native_snapshots', '--', 'collect', root, destination], helper, label + '-snapshots')
    command(['cargo', 'run', '--locked', '-p', 'xtask', '--', 'conformance', 'verify-native-snapshots', destination], root, label + '-snapshots-verify')
    metadata = json.loads(command(['cargo', 'metadata', '--locked', '--format-version', '1'], helper).stdout)
    json_new(PACKET / label / 'consumer-metadata.json', metadata)
    json_new(PACKET / label / 'collection-identity.json', {
        'schema': 'excalidraw-render/jpeg187-public-facade-comparison/v1', 'approval': None,
        'label': label, 'commit': command(['git', 'rev-parse', 'HEAD'], root).stdout.decode().strip(),
        'source_status': command(['git', 'status', '--porcelain', '--untracked-files=all'], root).stdout.decode(),
        'compiler': command(['rustc', '-vV'], root).stdout.decode(),
        'consumer_lock': desc(root, 'tests/package-consumer/Cargo.lock'),
        'workspace_lock': desc(root, 'Cargo.lock'), 'collector': desc(root, 'tests/package-consumer/examples/native_snapshots.rs'),
        'comparison_only_not_baseline_refresh': True})
    verify_snapshots(destination)

def stage_current():
    require(not CURRENT.exists(), 'current diagnostic stage already exists')
    before = tracked_state(ROOT)
    command(['git', 'worktree', 'add', '--detach', CURRENT, '4e5b624'], ROOT, 'current-worktree')
    for relative in before:
        source, target = ROOT / relative, CURRENT / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
    require(before == tracked_state(ROOT), 'parent source changed while staging; discard this attempt')
    require(before == tracked_state(CURRENT), 'stage does not match captured live files')
    json_new(PACKET / 'exploratory-current/producer-files.json', before)
    print('Created source-bound exploratory current stage; not final C, no 168-key C collection authorized yet.')

def graph(root, consumer=False):
    where = root / 'tests/package-consumer' if consumer else root
    metadata = json.loads(command(['cargo', 'metadata', '--locked', '--format-version', '1'], where).stdout)
    packages = {p['id']: p for p in metadata['packages']}
    nodes = {n['id']: n for n in metadata['resolve']['nodes']}
    start = next(p['id'] for p in packages.values() if p['name'] == 'excalidraw-render')
    seen, todo = set(), [start]
    while todo:
        p = todo.pop()
        if p in seen: continue
        seen.add(p)
        for dep in nodes[p]['deps']:
            if any(k['kind'] in [None, 'build'] for k in dep['dep_kinds']): todo.append(dep['pkg'])
    return {'packages': sorted([{'name': packages[p]['name'], 'version': packages[p]['version'], 'source': packages[p]['source'], 'license': packages[p]['license'], 'features': nodes[p]['features']} for p in seen], key=lambda p: (p['name'], p['version'])), 'metadata': metadata}

def resolve_consumer():
    require(CURRENT.exists(), 'stage-current first')
    old_lock = (CURRENT / 'tests/package-consumer/Cargo.lock').read_bytes()
    # Packaging is attempted unchanged first; retain the actual Cargo failure.
    if not package(CURRENT, 'exploratory-current'):
        json_new(PACKET / 'exploratory-current/package-blocker.json', {'status': 'package-failed', 'production_not_changed': True, 'logs': ['logs/exploratory-current-package.stdout', 'logs/exploratory-current-package.stderr']})
        return
    helper = CURRENT / 'tests/package-consumer'
    result = command(['cargo', 'update', '-p', 'zune-jpeg@0.5.15', '--precise', '0.5.16-rc2'], helper, 'consumer-resolve-rc', True)
    require(result.returncode == 0, result.stderr.decode())
    # This is the user-selected transitive graph, not an unconstrained update.
    command(['cargo', 'update', '-p', 'image', '--precise', '0.25.8'], helper, 'consumer-resolve-image')
    lock = (helper / 'Cargo.lock').read_bytes()
    put(PACKET / 'proposed/package-consumer-Cargo.lock', lock)
    put(PACKET / 'proposed/package-consumer-Cargo.lock.before', old_lock)
    diff = ''.join(difflib.unified_diff(old_lock.decode().splitlines(True), lock.decode().splitlines(True), fromfile='before/tests/package-consumer/Cargo.lock', tofile='proposed/tests/package-consumer/Cargo.lock'))
    put(PACKET / 'proposed/package-consumer-lock.diff', diff.encode())
    workspace, consumer = graph(CURRENT), graph(CURRENT, True)
    json_new(PACKET / 'exploratory-current/workspace-graph.json', workspace)
    json_new(PACKET / 'exploratory-current/consumer-graph.json', consumer)
    # Workspace feature unification may add dev-driven features. Compare exact
    # resolved package versions and archive/source provenance independently.
    identity = lambda g: {(p['name'], p['version']) for p in g['packages']}
    json_new(PACKET / 'proposed/consumer-resolution.json', {'approval': None, 'workspace_packages': sorted(identity(workspace)), 'consumer_packages': sorted(identity(consumer)), 'workspace_only': sorted(identity(workspace) - identity(consumer)), 'consumer_only': sorted(identity(consumer) - identity(workspace)), 'proposed_lock_sha256': sha(lock), 'before_lock_sha256': sha(old_lock), 'workspace_lock': desc(CURRENT, 'Cargo.lock'), 'final_freeze_recheck_required': True})
    print('Proposed consumer lock resolved in isolated stage; inspect exact diff and graph before application.')

def align_consumer():
    helper = CURRENT / 'tests/package-consumer'
    before = (PACKET / 'proposed/package-consumer-Cargo.lock.before').read_bytes()
    for name, version in [('bytemuck','1.25.1'),('proc-macro2','1.0.106'),('quote','1.0.46')]:
        command(['cargo','update','-p',name,'--precise',version],helper,'consumer-align-'+name)
    workspace, consumer = graph(CURRENT), graph(CURRENT, True)
    identity = lambda g: {(p['name'],p['version']) for p in g['packages']}
    require(identity(workspace)==identity(consumer), 'consumer runtime graph still differs')
    lock = (helper / 'Cargo.lock').read_bytes()
    put(PACKET/'proposed/package-consumer-aligned.Cargo.lock',lock)
    put(PACKET/'proposed/package-consumer-aligned.diff',''.join(difflib.unified_diff(before.decode().splitlines(True),lock.decode().splitlines(True),fromfile='before/tests/package-consumer/Cargo.lock',tofile='proposed/tests/package-consumer/Cargo.lock')).encode())
    json_new(PACKET/'proposed/consumer-aligned-resolution.json',{'approval':None,'package_versions_match_workspace':True,'proposed_lock_sha256':sha(lock),'before_lock_sha256':sha(before),'workspace_lock':desc(CURRENT,'Cargo.lock'),'workspace_packages':workspace['packages'],'consumer_packages':consumer['packages'],'preexisting_consumer_drift_removed':['bytemuck 1.25.2 -> 1.25.1','proc-macro2 1.0.107 -> 1.0.106','quote 1.0.47 -> 1.0.46'],'final_freeze_recheck_required':True})
    command(['cargo','test','--release','--locked','--example','native_snapshots'],helper,'aligned-consumer-tests')
    print('Aligned consumer lock passes packaged collector tests; runtime package-version set exactly matches workspace.')

def build_proofs():
    for label,stage in [('old',OLD),('exploratory-current',CURRENT)]:
        helper=stage/'tests/package-consumer'
        args=['cargo','build','--release','--locked','--example','native_snapshots','--message-format=json-render-diagnostics']
        result=command(args,helper,label+'-collector-build-proof')
        artifacts=[v for line in result.stdout.splitlines() if (v:=json.loads(line)).get('reason')=='compiler-artifact' and v['target']['name']=='native_snapshots']
        require(len(artifacts)==1,'missing exact collector artifact')
        artifact=artifacts[0]
        require(artifact['profile']=={'opt_level':'3','debuginfo':0,'debug_assertions':False,'overflow_checks':False,'test':False},'wrong release profile')
        exe=Path(artifact['executable'])
        archives=[]
        for name in PACKAGES:
            p=stage/'target/package'/(name+'-0.1.0.crate')
            source_checks=[]
            with tarfile.open(p) as tar:
                for member in tar.getmembers():
                    if not member.isfile(): continue
                    relative=Path(member.name).relative_to(name+'-0.1.0')
                    if str(relative) in ['Cargo.toml','Cargo.lock','.cargo_vcs_info.json']: continue
                    authored='Cargo.toml' if str(relative)=='Cargo.toml.orig' else str(relative)
                    source=stage/'crates'/name/authored
                    require(source.is_file(),'archive has unknown authored file '+str(source))
                    data=tar.extractfile(member).read()
                    require(data==source.read_bytes(),'archive payload differs from staged source '+str(source))
                    source_checks.append({'path':str(source.relative_to(stage)),'sha256':sha(data)})
            archives.append({'archive':desc(stage,str(p.relative_to(stage))),'authored_payloads':source_checks})
        json_new(PACKET/label/'build-and-package-proof.json',{'approval':None,'build_command':args,'actual_cargo_profile':artifact['profile'],'executable_sha256':sha(exe.read_bytes()),'compiler':command(['rustc','-vV'],stage).stdout.decode(),'release_settings':{k:v for k,v in ENV.items() if k.startswith('CARGO_PROFILE_RELEASE') or k in ['CARGO_INCREMENTAL','RUSTFLAGS']},'package_payload_equality':archives,'consumer_lock':desc(stage,'tests/package-consumer/Cargo.lock')})
        print(label+': exact release collector/profile and all authored package payloads verified.')

def verify_snapshots(directory):
    manifest = read(directory / 'snapshots.json')
    require(manifest['approval'] is None and manifest['schema'] == 'excalidraw-render/native-snapshots/v1', 'not unapproved snapshots')
    inventory = read(ROOT / 'tests/conformance/coverage/inventory.json')
    for field, path in [('inventory','tests/conformance/coverage/inventory.json'),('tool_manifest','tests/package-consumer/tool-manifest.json'),('dagger_lock','dagger.lock'),('consumer_manifest','tests/package-consumer/Cargo.toml'),('consumer_config','tests/package-consumer/.cargo/config.toml'),('collector','tests/package-consumer/examples/native_snapshots.rs')]:
        require(manifest[field]==desc(ROOT,path), 'stale snapshot input '+path)
    require(manifest['compatibility_revision']==1 and manifest['target']=='x86_64-unknown-linux-gnu','snapshot target/revision')
    require(manifest['rustc'].startswith('rustc 1.97.1 '), 'unpinned snapshot compiler')
    package_names=set()
    for package in manifest['packages']:
        name=Path(package['path']).name
        require(name not in package_names,'duplicate package'); package_names.add(name)
        data=(directory/'packages'/name).read_bytes()
        require(sha(data)==package['sha256'] and len(data)==package['byte_length'],'package archive mismatch')
    require(package_names=={p+'-0.1.0.crate' for p in PACKAGES}, 'incomplete package archive set')
    keys = [r['key'] for r in inventory['executions']]
    require(len(keys) == 168, 'inventory count changed')
    runs = manifest['runs']
    require(len(runs) == 2 and runs[0]['process_id'] != runs[1]['process_id'], 'fresh processes required')
    referenced = set()
    for run, order in zip(runs, ['forward', 'reverse']):
        require(run['order'] == order, 'wrong order')
        require(isinstance(run['process_id'],str) and bool(run['process_id'].strip()),'missing process identity')
        require(run == read(directory / (order + '.json')), 'worker/manifest mismatch')
        require([r['key'] for r in run['results']] == (keys if order == 'forward' else keys[::-1]), 'missing/changed keys')
        for row in run['results']:
            require(len(row['samples']) == 2 and row['samples'][0] == row['samples'][1], 'session mismatch')
            sample = row['samples'][0]
            references = [sample['report'], sample['outcome']['bytes'] if sample['outcome']['kind'] == 'artifact' else sample['outcome']['descriptor']]
            for record in references:
                blob = (directory / 'blobs' / record['sha256']).read_bytes()
                require(sha(blob) == record['sha256'] and len(blob) == record['byte_length'], 'blob mismatch')
                referenced.add(record['sha256'])
            report = read(directory / 'blobs' / sample['report']['sha256'])
            require(set(report) == {'schema_version', 'diagnostics'} and report['schema_version'] == 1, 'incomplete public report')
    require(runs[0]['results'] == runs[1]['results'][::-1], 'process/order mismatch')
    require({p.name for p in (directory / 'blobs').iterdir()} == referenced, 'extra/missing blobs')
    print(str(directory) + ': verified 168 keys, 2 processes × 2 sessions, complete artifacts/reports/failures.')

def compare(old, current):
    for d in [old, current]: verify_snapshots(d)
    a, b = read(old / 'snapshots.json'), read(current / 'snapshots.json')
    changes = []
    for left, right in zip(a['runs'][0]['results'], b['runs'][0]['results']):
        require(left['key'] == right['key'], 'full inventory key changed')
        if left != right: changes.append({'key': left['key'], 'before': left['samples'][0], 'after': right['samples'][0]})
    print(json.dumps({'keys': 168, 'unchanged': 168 - len(changes), 'changes_requiring_diagnosis': changes, 'approval': None}, indent=2))

def final_capture(stage, freeze_file):
    freeze = read(freeze_file)
    require(freeze.get('final_current_capture_authorized') is True, 'explicit parent final-freeze authorization required')
    required={d['path'] for d in read(ROOT/'tests/conformance/candidates/stage4-frames-text-assets-v8/native-capture.json')['producer_sources']}
    required.update(['tests/package-consumer/examples/native_snapshots.rs','tests/conformance/coverage/inventory.json','tests/conformance/cases/registry.json'])
    require(required<=set(freeze['producer_files']), 'freeze omits required production/input/collector bindings')
    for path, expected in freeze['producer_files'].items():
        require(sha((stage / path).read_bytes()) == expected, 'stale final stage: ' + path)
        require(sha((ROOT / path).read_bytes()) == expected, 'parent live freeze changed: ' + path)
    require(stage != ROOT and stage != OLD and stage != CURRENT, 'use a fresh final staging root')
    proposed=PACKET/'proposed/package-consumer-Cargo.lock'
    require((stage/'tests/package-consumer/Cargo.lock').read_bytes()==proposed.read_bytes(),'final staged consumer lock differs from reviewed proposal')
    require(package(stage, 'final-current'), 'final package failure')
    capture(stage, 'final-current')
    for path, expected in freeze['producer_files'].items(): require(sha((ROOT / path).read_bytes()) == expected, 'parent changed during capture')

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=['old', 'stage-current', 'resolve-consumer', 'align-consumer', 'build-proofs', 'verify', 'compare', 'final'])
    parser.add_argument('paths', nargs='*')
    args = parser.parse_args()
    if args.mode == 'old': old_capture()
    elif args.mode == 'stage-current': stage_current()
    elif args.mode == 'resolve-consumer': resolve_consumer()
    elif args.mode == 'align-consumer': align_consumer()
    elif args.mode == 'build-proofs': build_proofs()
    elif args.mode == 'verify': verify_snapshots(Path(args.paths[0]))
    elif args.mode == 'compare': compare(Path(args.paths[0]), Path(args.paths[1]))
    elif args.mode == 'final': final_capture(Path(args.paths[0]), Path(args.paths[1]))
