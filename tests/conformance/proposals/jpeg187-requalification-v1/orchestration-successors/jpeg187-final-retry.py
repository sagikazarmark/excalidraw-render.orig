import json
from pathlib import Path
import subprocess
import types

ROOT=Path('/home/laborant/excalidraw-render.orig')
P=ROOT/'tests/conformance/proposals/jpeg187-requalification-v1'
q=types.ModuleType('frozen_qualification');q.__file__=str(P/'qualify.py')
exec(compile((P/'qualify.py').read_bytes(),q.__file__,'exec'),q.__dict__)
c=q.collector()
stage=Path('/tmp/opencode/jpeg187-color-final-stage')
q.check_freeze()
destination=P/'final-current/snapshots'
c.require(not destination.exists(),'refuse overwrite')
c.command(['cargo','run','--release','--locked','--example','native_snapshots','--','collect',stage,destination],stage/'tests/package-consumer','final-current-snapshots-retry-1')
c.command(['cargo','run','--locked','-p','xtask','--','conformance','verify-native-snapshots',destination],stage,'final-current-snapshots-verify')
metadata=json.loads(c.command(['cargo','metadata','--locked','--format-version','1'],stage/'tests/package-consumer').stdout)
c.json_new(P/'final-current/consumer-metadata.json',metadata)
c.json_new(P/'final-current/collection-identity.json',{
    'schema':'excalidraw-render/jpeg187-public-facade-comparison/v1','approval':None,'label':'final-current',
    'commit':c.command(['git','rev-parse','HEAD'],stage).stdout.decode().strip(),
    'source_status':c.command(['git','status','--porcelain','--untracked-files=all'],stage).stdout.decode(),
    'rustc':c.command(['rustc','-vV'],stage).stdout.decode(),
    'consumer_lock':c.desc(stage,'tests/package-consumer/Cargo.lock'),
    'workspace_lock':c.desc(stage,'Cargo.lock'),
    'collector':c.desc(stage,'tests/package-consumer/examples/native_snapshots.rs'),
    'retry_reason':'initial output parent missing; no snapshot directory created; created parent and retried exact frozen collector; failed log preserved',
    'comparison_only_not_baseline_refresh':True})
c.verify_snapshots(destination)
q.check_freeze()
