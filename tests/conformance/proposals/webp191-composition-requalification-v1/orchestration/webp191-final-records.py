from pathlib import Path
import subprocess
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/webp191-composition-requalification-v1'
q=types.ModuleType('frozen_webp');q.__file__=str(P/'workflow.py')
exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec'),q.__dict__)
q.check_freeze();checks=[]
commands=[('webp-sample-final',['cargo','test','--locked','-p','excalidraw-render','--test','fast','webp_sample_contract::','--','--nocapture']),
 ('webp-unit-final',['cargo','test','--locked','-p','excalidraw-render','--lib','render::asset::tests::webp_codec::','--','--nocapture']),
 ('static-webp-final',['cargo','test','--locked','-p','excalidraw-render','--test','fast','static_webp_asset_render::','--','--nocapture']),
 ('jpeg-regression-final',['cargo','test','--locked','-p','excalidraw-render','--test','fast','jpeg_','--','--nocapture']),
 ('gif-regression-final',['cargo','test','--locked','-p','excalidraw-render','--test','fast','gif_','--','--nocapture'])]
for name,args in commands:
    r=subprocess.run(args,cwd=R,env=q.ENV,capture_output=True)
    q.put(P/'checks'/f'{name}.stdout',r.stdout);q.put(P/'checks'/f'{name}.stderr',r.stderr)
    q.require(r.returncode==0,r.stdout.decode()+r.stderr.decode())
    checks.append({'command':args,'passed':True,'log':q.desc(P/'checks'/f'{name}.stdout'),'stderr':q.desc(P/'checks'/f'{name}.stderr')})
    print(name+': passed',flush=True)
a=q.read(P/'old/snapshots/snapshots.json')['runs'][0]['results'];b=q.read(P/'final-current/snapshots/snapshots.json')['runs'][0]['results']
q.require(len(a)==len(b)==168 and [r['key'] for r in a]==[r['key'] for r in b],'full inventory key drift')
changes=[{'key':x['key'],'before':x['samples'][0],'after':y['samples'][0]} for x,y in zip(a,b) if x!=y]
q.require(not changes,'STOP unexplained output change')
q.write(P/'checks/inventory-comparison.json',{'approval':None,'keys':168,'unchanged':168,'changes':changes})
q.put(P/'checks/parent-freeze-authorization.json',Path('/tmp/opencode/webp191-parent-freeze.json').read_bytes())
q.write(P/'checks/parent-freeze-attestation.json',{'approval':None,'origin':'actual parent session message, not invented command output',
    'authorization':q.read(Path('/tmp/opencode/webp191-parent-freeze.json')),
    'reported_checks':['five WebP sample tests pass','eight WebP unit tests pass','workspace formatting and Clippy pass','Standards and Spec pre-freeze reviews: no findings'],
    'parent_full_workspace':'running in parallel; source-bound result awaited; expected stale gates not waived',
    'test_source':q.desc(R/q.SOURCE),'producer_source':q.desc(R/q.PRODUCER)})
record={'approval':None,'scope':'webp191-composition','composition_only':True,'static_or_alpha_policy_change':False,
    'test_source':q.desc(R/q.SOURCE),'producer_sources':q.read(R/q.V13/'native-capture.json')['producer_sources'],
    'reference_sha256':q.HASHES[q.REFERENCE],'old_measurements_sha256':q.HASHES[q.NATIVE_OLD],'new_measurements_sha256':q.HASHES[q.NATIVE_NEW],
    'sample_cells':36,'qualified_cells':24,'residual_partial_cells':12,'svg_vectors_pass':72,'png_vectors_pass':48,'pdf_vectors_pass':48,
    'measurement_transition':q.measurements(),'prior_codec_regressions_passed':True,'prior_known_limitations_preserved':True,
    'checks':checks,'diagnosed_inventory_changes':changes,'parent_full_workspace':'awaiting parent report; production subject not sealed',
    'resource_scope':'Single fallible standalone-RIFF buffer allocation, checked dimensions/extents/placement and early drops; no end-to-end physical allocation failure/OOM closure claim.',
    'profile_orientation_scope':'Existing source-bound WebP ICC and EXIF unit controls passed; metadata applied after frame-zero RGBA canvas composition. No new tagged-browser matrix claim.',
    'residual_scope':'24 partial-alpha vectors remain exact known failures. Synthetic PDF rounded straight projection mismatch is not a PDF viewer measurement.'}
q.write(P/'checks/parent-qualification.json',record);q.qualification();q.verify_migration(False)
prefix=['cargo','run','--release','--locked','-p','xtask','--','conformance']
for name,args,needle in [('v13-pending',['candidate-verify',q.V13],'approval must be an object'),('ledger-stale',['verify-ownership'],'stale'),('v12-stale',['candidate-approval-subject',q.PRIOR],'stale')]:
    r=subprocess.run(prefix+args,cwd=R,env=q.ENV,capture_output=True)
    q.put(P/'checks'/f'{name}.stdout',r.stdout);q.put(P/'checks'/f'{name}.stderr',r.stderr)
    q.require(r.returncode!=0 and not r.stdout and needle in r.stderr.decode(),'strict pending blocker missing')
    print(name+': correctly blocked',flush=True)
for name in ['webp191-freeze.py','webp191-final-records.py']:q.put(P/'orchestration'/name,(Path('/tmp/opencode')/name).read_bytes())
q.check_freeze();print('Source-bound composition qualification recorded; parent full-suite report awaited.')
