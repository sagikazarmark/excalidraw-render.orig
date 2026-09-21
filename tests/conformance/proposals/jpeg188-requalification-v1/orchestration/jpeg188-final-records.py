import json
from pathlib import Path
import subprocess
import types

ROOT=Path('/home/laborant/excalidraw-render.orig')
P=ROOT/'tests/conformance/proposals/jpeg188-requalification-v1'
q=types.ModuleType('frozen_icc_workflow');q.__file__=str(P/'workflow.py')
exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec'),q.__dict__)
q.check_freeze()
checks=[]
for name,args in [
    ('jpeg-icc-final',['cargo','test','--locked','-p','excalidraw-render','--test','fast','jpeg_icc_contract::','--','--nocapture']),
    ('jpeg-color-final',['cargo','test','--locked','-p','excalidraw-render','--test','fast','jpeg_color_contract::','--','--nocapture']),
    ('jpeg-inputs-final',['cargo','test','--locked','-p','excalidraw-render','--test','jpeg_matrix_inputs']),
]:
    result=subprocess.run(args,cwd=ROOT,env=q.ENV,capture_output=True)
    q.put(P/'checks'/f'{name}.stdout',result.stdout);q.put(P/'checks'/f'{name}.stderr',result.stderr)
    q.require(result.returncode==0,result.stdout.decode()+result.stderr.decode())
    checks.append({'command':args,'passed':True,'log':q.desc(P/'checks'/f'{name}.stdout'),'stderr':q.desc(P/'checks'/f'{name}.stderr')})
    print(name+': passed',flush=True)
old=q.read(P/'old/snapshots/snapshots.json')['runs'][0]['results']
new=q.read(P/'final-current/snapshots/snapshots.json')['runs'][0]['results']
q.require(len(old)==len(new)==168,'capture count')
changes=[]
for a,b in zip(old,new):
    q.require(a['key']==b['key'],'full key drift')
    if a!=b:changes.append({'key':a['key'],'before':a['samples'][0],'after':b['samples'][0]})
q.require(not changes,'STOP: unexpected output change')
q.write(P/'checks/inventory-comparison.json',{'approval':None,'keys':168,'unchanged':168,'changes':changes})
q.put(P/'checks/parent-freeze-authorization.json',Path('/tmp/opencode/jpeg188-parent-freeze.json').read_bytes())
q.write(P/'checks/parent-freeze-attestation.json',{'approval':None,'origin':'actual parent session message; not invented command output',
    'authorization':q.read(Path('/tmp/opencode/jpeg188-parent-freeze.json')),
    'reported_checks':['10 final ICC tests pass','workspace formatting passes','workspace Clippy passes'],
    'current_full_workspace_status':'parent running in parallel; expected current stale owner/representative failures pending actual approval; no waiver',
    'test_source':q.desc(ROOT/q.SOURCE)})
review={'approval':None,'scope':'jpeg188-icc','test_source':q.desc(ROOT/q.SOURCE),
    'producer_sources':q.read(ROOT/q.V10/'native-capture.json')['producer_sources'],
    'icc_reference_sha256':q.REFERENCE_HASH,'icc_vectors':160,'icc_gates_passed':True,
    'icc_canonical_color_vectors':104,'icc_strict_recovery_vectors':56,
    'jpeg187_vectors':76,'jpeg187_canonical_pass_vectors':64,'jpeg187_known_failure_vectors':12,'jpeg187_exact_failure_pins':24,
    'checks':checks,'diagnosed_inventory_changes':changes,
    'auxiliary_controls':{'gray_plus_minus_one':'independently analytical scratch/extent boundary check only; not a changed reference comparison budget',
                          'gray_trc_on_ycbcr':'metamorphic tagged/untagged equality only; not a new browser authority claim'},
    'parent_full_workspace':'awaiting source-bound parent report; not a final production approval packet'}
q.write(P/'checks/parent-qualification.json',review)
q.qualification();q.verify_migration(False)
prefix=['cargo','run','--release','--locked','-p','xtask','--','conformance']
for name,args,expected in [('v10-pending',['candidate-verify',q.V10],'approval must be an object'),
                          ('ledger-stale',['verify-ownership'],'stale'),
                          ('v9-stale',['candidate-approval-subject','tests/conformance/candidates/stage4-frames-text-assets-v9'],'stale')]:
    output=subprocess.run(prefix+args,cwd=ROOT,env=q.ENV,capture_output=True)
    q.put(P/'checks'/f'{name}.stdout',output.stdout);q.put(P/'checks'/f'{name}.stderr',output.stderr)
    q.require(output.returncode!=0 and expected in output.stderr.decode() and not output.stdout,'expected strict blocker missing')
    print(name+': correctly blocked',flush=True)
q.check_freeze()
q.put(P/'orchestration/jpeg188-final-records.py',Path(__file__).read_bytes())
print('Qualification records bound; awaiting parent suite results before final production seal.')
