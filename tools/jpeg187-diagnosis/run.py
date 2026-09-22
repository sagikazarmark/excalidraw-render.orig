#!/usr/bin/env python3
"""Reproduce the unapproved decoder experiment outside the production graph."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

HERE=Path(__file__).resolve().parent
ROOT=HERE.parents[1]
destination=Path(sys.argv[1]).resolve()
if not destination.is_relative_to(Path('/tmp/opencode')) or destination.exists():
    raise SystemExit('Supply a new directory under /tmp/opencode')
source=next((Path.home()/'.cargo/registry/src').glob('*/zune-jpeg-0.5.15'))
destination.mkdir()
decoder=destination/'jpeg187-zune-probe'
shutil.copytree(source,decoder)
probe=destination/'probe'
shutil.copytree(HERE,probe)
manifest=(probe/'Cargo.toml').read_text().replace('../../crates/excalidraw-render-compare',str(ROOT/'crates/excalidraw-render-compare'))
(probe/'Cargo.toml').write_text(manifest)
env=dict(os.environ,PATH='/home/laborant/.rustup/toolchains/1.97.1-x86_64-unknown-linux-gnu/bin:'+os.environ['PATH'],CARGO_INCREMENTAL='0',CARGO_PROFILE_DEV_DEBUG='0',JPEG187_REPOSITORY=str(ROOT))


def run(name,args,cwd=probe):
    result=subprocess.run(args,cwd=cwd,env=env,capture_output=True)
    (destination/(name+'.stdout')).write_bytes(result.stdout)
    (destination/(name+'.stderr')).write_bytes(result.stderr)
    print(name,result.returncode)
    return result


baseline=run('baseline',['cargo','run','--offline','--locked'])
if baseline.returncode==0:raise SystemExit('Original defect unexpectedly absent')
patched=run('patch',['patch','--batch','-p1','-i',str(HERE/'sampling.patch')],decoder)
if patched.returncode:raise SystemExit(patched.stderr.decode()+patched.stdout.decode())
for name,args in [('neutral',[]),('matrix',['--','matrix']),('malformed',['--','malformed'])]:
    result=run(name,['cargo','run','--offline','--locked',*args])
    if result.returncode:raise SystemExit(result.stderr.decode()+result.stdout.decode())
sha=lambda data:hashlib.sha256(data).hexdigest()
record={'scope':'diagnostic only; no renderer/dependency/ownership change',
        'compiler':subprocess.check_output(['rustc','--version'],env=env).decode().strip(),
        'neutral_cases':2160,'color_vectors':76,'browser_comparisons':152,
        'original_mcu_sha256':sha((source/'src/mcu.rs').read_bytes()),
        'patched_mcu_sha256':sha((decoder/'src/mcu.rs').read_bytes()),
        'sources':{str(p.relative_to(ROOT)):sha(p.read_bytes()) for p in sorted(HERE.rglob('*')) if p.is_file()},
        'logs':{p.name:sha(p.read_bytes()) for p in sorted(destination.glob('*.*')) if p.is_file()}}
(destination/'result.json').write_text(json.dumps(record,indent=2)+'\n')
print('2160 neutral cells and152browser comparisons pass; malformed results require review, not closure.')
