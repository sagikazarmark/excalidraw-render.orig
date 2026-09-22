#!/usr/bin/env python3
"""Reproduce hardening in a fresh temporary tree; live dependency graph untouched."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[2]
target=Path(sys.argv[1]).resolve()
if target.exists() or not target.is_relative_to(Path('/tmp/opencode')):
    raise SystemExit('fresh directory under /tmp/opencode required')
target.mkdir()
decoder=target/'jpeg187-zune-probe'
shutil.copytree(next((Path.home()/'.cargo/registry/src').glob('*/zune-jpeg-0.5.15')),decoder)
probe=target/'probe';probe.mkdir();shutil.copytree(HERE/'src',probe/'src')
shutil.copyfile(HERE/'Cargo.lock',probe/'Cargo.lock')
manifest=(HERE/'Cargo.toml').read_text().replace('/home/laborant/excalidraw-render.orig/crates/excalidraw-render-compare',str(ROOT/'crates/excalidraw-render-compare'))
(probe/'Cargo.toml').write_text(manifest)
env=dict(os.environ,PATH='/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:'+os.environ['PATH'],CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',JPEG187_REPOSITORY=str(ROOT))
def run(name,args,cwd=probe,extra=None):
    result=subprocess.run(args,cwd=cwd,env=dict(env,**(extra or {})),capture_output=True)
    (target/(name+'.stdout')).write_bytes(result.stdout);(target/(name+'.stderr')).write_bytes(result.stderr)
    if result.returncode:raise RuntimeError(name+': '+result.stderr.decode()+result.stdout.decode())
    print(name,'passed')
run('patch',['patch','--batch','-p1','-i',str(HERE/'decoder.patch')],decoder)
run('neutral',['cargo','run','--offline','--locked'])
run('matrix',['cargo','run','--offline','--locked','--','matrix'])
run('malformed',['cargo','run','--offline','--locked','--','malformed'])
fixtures=target/'mixed-reference'
run('mixed',['cargo','run','--offline','--locked','--','mixed'],extra={'JPEG187_MIXED_EXPORT':str(fixtures)})
shutil.copyfile(HERE/'browser.cjs',fixtures/'browser.cjs')
run('browser',['docker','run','--rm','--network','none','-v',str(fixtures)+':/probe:ro','-v',str(ROOT)+':/repo:ro',
    'mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9','node','/probe/browser.cjs'])
sha=lambda b:hashlib.sha256(b).hexdigest()
record={'scope':'external diagnostic; reverse-scan browser divergence remains',
    'sources':{str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(HERE.rglob('*')) if p.is_file() and 'results' not in p.parts},
    'logs':{p.name:sha(p.read_bytes()) for p in target.glob('*.*') if p.is_file()},
    'patched_sources':{p:sha((decoder/'src'/p).read_bytes()) for p in ['mcu.rs','bitstream.rs']},
    'fixtures':{p.name:sha(p.read_bytes()) for p in sorted(fixtures.glob('*')) if p.is_file()}}
(target/'result.json').write_text(json.dumps(record,indent=2)+'\n')
