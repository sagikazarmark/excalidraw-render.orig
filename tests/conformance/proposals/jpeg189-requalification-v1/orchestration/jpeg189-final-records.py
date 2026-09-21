import json
from pathlib import Path
import subprocess
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/jpeg189-requalification-v1'
q=types.ModuleType('frozen_exif');q.__file__=str(P/'workflow.py')
exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec'),q.__dict__)
q.check_freeze();checks=[]
for name,args in [('jpeg-final',['cargo','test','--locked','-p','excalidraw-render','--test','fast','jpeg_','--','--nocapture']),
                  ('jpeg-inputs-final',['cargo','test','--locked','-p','excalidraw-render','--test','jpeg_matrix_inputs'])]:
    result=subprocess.run(args,cwd=R,env=q.ENV,capture_output=True)
    q.put(P/'checks'/f'{name}.stdout',result.stdout);q.put(P/'checks'/f'{name}.stderr',result.stderr)
    q.require(result.returncode==0,result.stdout.decode()+result.stderr.decode())
    checks.append({'command':args,'passed':True,'log':q.desc(P/'checks'/f'{name}.stdout'),'stderr':q.desc(P/'checks'/f'{name}.stderr')})
    print(name+': passed',flush=True)
old=q.read(P/'old/snapshots/snapshots.json')['runs'][0]['results'];new=q.read(P/'final-current/snapshots/snapshots.json')['runs'][0]['results']
q.require(len(old)==len(new)==168 and [r['key'] for r in old]==[r['key'] for r in new],'168 full keys mismatch')
changes=[{'key':a['key'],'before':a['samples'][0],'after':b['samples'][0]} for a,b in zip(old,new) if a!=b]
q.require(not changes,'STOP: unexplained capture output movement')
q.write(P/'checks/inventory-comparison.json',{'approval':None,'keys':168,'unchanged':168,'changes':changes})
q.put(P/'checks/parent-freeze-authorization.json',Path('/tmp/opencode/jpeg189-parent-freeze.json').read_bytes())
q.write(P/'checks/parent-freeze-attestation.json',{'approval':None,'origin':'actual parent session message, not invented command output',
    'authorization':q.read(Path('/tmp/opencode/jpeg189-parent-freeze.json')),
    'reported_checks':['34 JPEG tests passed','workspace formatting passed','workspace Clippy passed','Standards and Spec pre-freeze reviews: no findings'],
    'parent_full_workspace':'running in parallel; awaiting source-bound result; no pending binding failure waived',
    'test_source':q.desc(R/q.SOURCE),'producer_source':q.desc(R/'crates/excalidraw-render/src/render/asset/jpeg.rs')})
record={'approval':None,'scope':'jpeg189-exif','test_source':q.desc(R/q.SOURCE),
    'producer_sources':q.read(R/q.V11/'native-capture.json')['producer_sources'],
    'exif_reference_sha256':q.REFERENCE_HASH,'exif_vectors':164,'exif_canonical_gates_passed':True,
    'icc_reference_sha256':'7ffd57eaa8ddfe4a7d311c5a55c57b84a001463864ce6bd235613faab85e76d5',
    'icc_vectors':160,'icc_color_vectors':104,'icc_recovery_vectors':56,'icc_gates_passed':True,
    'jpeg187_vectors':76,'jpeg187_canonical_vectors':64,'jpeg187_known_failure_vectors':12,'jpeg187_exact_pins':24,
    'checks':checks,'diagnosed_inventory_changes':changes,'parent_full_workspace':'awaiting parent report; production subject not sealed'}
q.write(P/'checks/parent-qualification.json',record);q.qualification();q.verify_migration(False)
prefix=['cargo','run','--release','--locked','-p','xtask','--','conformance']
for name,args,needle in [('v11-pending',['candidate-verify',q.V11],'approval must be an object'),
                         ('ledger-stale',['verify-ownership'],'stale'),
                         ('v10-stale',['candidate-approval-subject',q.PRIOR],'stale')]:
    result=subprocess.run(prefix+args,cwd=R,env=q.ENV,capture_output=True)
    q.put(P/'checks'/f'{name}.stdout',result.stdout);q.put(P/'checks'/f'{name}.stderr',result.stderr)
    q.require(result.returncode!=0 and not result.stdout and needle in result.stderr.decode(),'expected strict pending rejection missing')
    print(name+': correctly blocked',flush=True)
q.put(P/'orchestration/jpeg189-final-records.py',Path(__file__).read_bytes())
q.put(P/'orchestration/jpeg189-freeze.py',Path('/tmp/opencode/jpeg189-freeze.py').read_bytes())
q.check_freeze()
print('Bound final focused logs/qualification; parent full-suite result still pending.')
