import json
import re
from pathlib import Path
import subprocess
import types

ROOT=Path('/home/laborant/excalidraw-render.orig')
P=ROOT/'tests/conformance/proposals/jpeg187-requalification-v1'
q=types.ModuleType('frozen_checker');q.__file__=str(P/'qualify.py')
exec(compile((P/'qualify.py').read_bytes(),q.__file__,'exec'),q.__dict__)
q.check_freeze()
checks=[]
def checked(name,args):
    result=subprocess.run(args,cwd=ROOT,env=q.ENV,capture_output=True)
    q.put(P/'final-checks'/f'{name}.stdout',result.stdout)
    q.put(P/'final-checks'/f'{name}.stderr',result.stderr)
    q.require(result.returncode==0,result.stderr.decode()+result.stdout.decode())
    checks.append({'name':name,'command':args,'passed':True,'log':q.desc(P/'final-checks'/f'{name}.stdout'),'stderr':q.desc(P/'final-checks'/f'{name}.stderr')})
    print(name+': passed',flush=True)
checked('jpeg-focused-final',['cargo','test','--locked','-p','excalidraw-render','--test','fast','jpeg_color_contract::','--','--nocapture'])
checked('jpeg-inputs-final',['cargo','test','--locked','-p','excalidraw-render','--test','jpeg_matrix_inputs'])
image='mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b73062d9'
# Exact original profile OCI (not an unpinned browser install).
image='mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9'
checked('tiny-neutral-browser-corrected',['docker','run','--rm','--init','--ipc=host','--network','none','--platform','linux/amd64','--user',f'{__import__("os").getuid()}:{__import__("os").getgid()}','--env','HOME=/tmp','--mount',f'type=bind,src={ROOT},dst=/workspace','--workdir','/workspace',image,'node','tests/conformance/proposals/jpeg187-requalification-v1/tiny-neutral-browser.mjs'])
source=(ROOT/q.SOURCE).read_text()
block=source.split('fn sampling_defect(id: &str)')[1].split('\nfn check(')[0]
pins=[]
for identity,changed,bits in re.findall(r'"([^"]+)"\s*=>\s*\((\d+),\s*0x([0-9a-f]+)\)',block):
    for fmt in ['png','pdf']:pins.append({'id':identity,'format':fmt,'canonical_pass':False,'changed_pixels':int(changed),'ssim_bits':bits})
q.require(len(pins)==24,'exact 24 source pins not found')
old=(P/'before'/q.SOURCE).read_text()
for pin in pins:
    needle=f'"{pin["id"]}" => ({pin["changed_pixels"]}, 0x{pin["ssim_bits"]})'
    q.require(needle in old,'sampling pin differs from HEAD: '+needle)
neutral=[]
for sampling in ['F_4_2','F_1_4','F_2_4']:
    for width,height in [(1,1),(9,9),(17,17),(33,25)]:
        wrong={1:0,9:9,17:153 if sampling=='F_4_2' else 289,33:825}[width]
        for fmt in ['png','pdf']:neutral.append({'sampling':sampling,'width':width,'height':height,'format':fmt,'expected_wrong_pixels':wrong,'matched_by_passing_exact_assertion':True})
q.write(P/'final-checks/neutral-measurements.json',{'followup_issue':187,'status':'known-failure-not-a-tolerance','test':'jpeg_color_contract::issue_187_minimized_noninterleaved_neutral_image_still_exposes_missing_rows','source':q.desc(ROOT/q.SOURCE),'basis':'actual source match arms plus successful final test execution; nonzero cases remain known failures; 1x1 controls pass','rows':neutral,'browser_control':q.desc(P/'final-checks/tiny-neutral-browser-corrected.stdout')})
nested=[]
for row in q.read(P/'nested/qualification.final-current.json')['records']:
    if row['id'] not in ['jpeg-ycbcr-4x2-baseline-edges','jpeg-ycck-2x4-baseline-edges']:continue
    for value in row['comparisons']:nested.append({'id':row['id'],**value})
old_runs=q.read(P/'old/snapshots/snapshots.json')['runs'][0]['results']
new_runs=q.read(P/'final-current/snapshots/snapshots.json')['runs'][0]['results']
q.require(len(old_runs)==len(new_runs)==168,'inventory count')
changes=[]
for a,b in zip(old_runs,new_runs):
    q.require(a['key']==b['key'],'key changed')
    if a!=b:changes.append({'key':a['key'],'before':a['samples'][0],'after':b['samples'][0]})
q.require(not changes,'unexpected inventory changes require parent diagnosis')
q.write(P/'final-checks/parent-freeze-attestation.json',{'origin':'actual parent session message; not invented command output','authorization':q.read(Path('/tmp/opencode/jpeg187-parent-freeze.json')),'reported_checks':['all 8 JPEG focused tests passed','workspace fmt passed','workspace Clippy passed'],'full_workspace_status':'parent running in parallel; awaiting source-bound result','frozen_test_source':q.desc(ROOT/q.SOURCE)})
record={'approval':None,'scope':'stable-graph-color-only','jpeg_reference_sha256':'9b5ab60a18e83b52c582ccb38728b71633b11b592ffe540bab2c796857694638','frozen_input_reconstruction_passed':True,'reconstructed_vectors':76,'qualified_vectors':64,'known_failure_vectors':12,'known_sampling_cells':sorted(q.DEFERRED),'sampling_defect_pins':pins,'nested_sampling_defect_pins':nested,'neutral_failure_control':{'followup_issue':187,'canonical_pass':False,'test_source':q.desc(ROOT/q.SOURCE),'measurements':q.desc(P/'final-checks/neutral-measurements.json')},'test_source':q.desc(ROOT/q.SOURCE),'producer_sources':q.read(ROOT/q.V9/'native-capture.json')['producer_sources'],'checks':checks,'diagnosed_inventory_changes':changes,'parent_full_workspace':'awaiting parent report; packet not sealed'}
q.write(P/'final-checks/production-qualification.json',record)
q.check_sampling_pins(record);q.check_final_findings();q.check_freeze();q.verify_migration(False)
q.write(P/'final-checks/inventory-comparison.json',{'approval':None,'keys':168,'unchanged_artifact_failure_report_records':168,'changed':changes})
for name,args,needle in [('v9-pending',['candidate-verify',q.V9],'approval must be an object'),('ledger-stale',['verify-ownership'],'stale'),('v8-stale',['candidate-approval-subject','tests/conformance/candidates/stage4-frames-text-assets-v8'],'stale')]:
    command=['cargo','run','--release','--locked','-p','xtask','--','conformance',*args]
    result=subprocess.run(command,cwd=ROOT,env=q.ENV,capture_output=True)
    q.put(P/'final-checks'/f'{name}.stdout',result.stdout);q.put(P/'final-checks'/f'{name}.stderr',result.stderr)
    q.require(result.returncode!=0 and needle in result.stderr.decode() and not result.stdout,'expected strict rejection missing')
    print(name+': correctly blocked',flush=True)
q.check_freeze()
print('Final color-only records assembled; awaiting parent full-suite evidence before sealing.')
