import struct
from pathlib import Path
import subprocess
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/jpeg190-mpf-requalification-v1'
q=types.ModuleType('frozen_mpf');q.__file__=str(P/'workflow.py')
exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec'),q.__dict__)
q.check_freeze();checks=[]
for name,args in [('jpeg-focused-final',['cargo','test','--locked','-p','excalidraw-render','--test','fast','jpeg_','--','--nocapture']),
                  ('jpeg-library-final',['cargo','test','--locked','-p','excalidraw-render','--lib','render::asset::tests::jpeg_codec::','--','--nocapture']),
                  ('jpeg-inputs-final',['cargo','test','--locked','-p','excalidraw-render','--test','jpeg_matrix_inputs'])]:
    r=subprocess.run(args,cwd=R,env=q.ENV,capture_output=True)
    q.put(P/'checks'/f'{name}.stdout',r.stdout);q.put(P/'checks'/f'{name}.stderr',r.stderr)
    q.require(r.returncode==0,r.stdout.decode()+r.stderr.decode())
    checks.append({'command':args,'passed':True,'log':q.desc(P/'checks'/f'{name}.stdout'),'stderr':q.desc(P/'checks'/f'{name}.stderr')})
    print(name+': passed',flush=True)
measurements={('hdr-ramp','ramp'):(553,0.860099630813284),('hdr-edges','ramp'):(648,0.8479795899866851),('hdr-ramp','edges'):(1214,0.9116739114307459),('hdr-edges','edges'):(1262,0.8737353963660468)}
pins=[]
for row in q.read(R/q.REFERENCE)['cases']:
    if row['signal'] not in q.HDR_SIGNALS:continue
    changed,score=measurements[row['signal'],row['pattern']]
    for fmt in ['png','pdf']:pins.append({'id':row['id'],'format':fmt,'canonical_pass':False,'changed_pixels':changed,'ssim_bits':struct.pack('>d',score).hex()})
q.require(len(pins)==32,'exact HDR pins')
source=(R/q.SOURCE).read_text();old=(P/'before'/q.SOURCE).read_text()
for (signal,pattern),(changed,score) in measurements.items():
    needle=f'("{signal}", "{pattern}") => ({changed}, {score})'
    q.require(needle in source and needle in old,'HDR measurement changed '+needle)
a=q.read(P/'old/snapshots/snapshots.json')['runs'][0]['results'];b=q.read(P/'final-current/snapshots/snapshots.json')['runs'][0]['results']
q.require(len(a)==len(b)==168 and [r['key'] for r in a]==[r['key'] for r in b],'capture key mismatch')
changes=[{'key':x['key'],'before':x['samples'][0],'after':y['samples'][0]} for x,y in zip(a,b) if x!=y]
q.require(not changes,'STOP unexplained capture change')
q.write(P/'checks/inventory-comparison.json',{'approval':None,'keys':168,'unchanged':168,'changes':changes})
q.put(P/'checks/parent-freeze-authorization.json',Path('/tmp/opencode/jpeg190-parent-freeze.json').read_bytes())
q.write(P/'checks/parent-freeze-attestation.json',{'approval':None,'origin':'actual parent session freeze/review message, not invented test output',
    'authorization':q.read(Path('/tmp/opencode/jpeg190-parent-freeze.json')),
    'reported_checks':['46 JPEG focused tests including 12 gain-map pass','10 library JPEG codec tests pass','formatting and Clippy pass','Standards no hard block; Spec secondary-index finding fixed and confirmed'],
    'parent_full_workspace':'running in parallel; source-bound report pending; current stale gates not waived',
    'test_source':q.desc(R/q.SOURCE),'asset_unit_fixture_source':q.desc(R/'crates/excalidraw-render/src/render/asset/tests.rs')})
review={'approval':None,'scope':'jpeg190-mpf','mpf_only':True,'decoder_or_hdr_production_change':False,
    'test_source':q.desc(R/q.SOURCE),'producer_sources':q.read(R/q.V12/'native-capture.json')['producer_sources'],
    'reference_sha256':q.REFERENCE_HASH,'vectors':104,'applicable_gates_passed':True,
    'canonical_sdr_vectors':24,'strict_recovery_vectors':64,'hdr_known_failure_vectors':16,'hdr_exact_pins':32,'hdr_failure_measurements':pins,
    'jpeg187_vectors':76,'jpeg187_canonical_vectors':64,'jpeg187_known_failure_vectors':12,'jpeg187_exact_pins':24,
    'icc_vectors':160,'icc_gates_passed':True,'exif_vectors':164,'exif_gates_passed':True,
    'checks':checks,'diagnosed_inventory_changes':changes,
    'admission_scope':'bounded primary two-image index: optional UID/TotalFrames, nonoverlapping relocated entries and representative primary flag; secondary Index IFD rejected, narrow version/individual-number Attribute IFD admitted, duplicate secondary MPF rejected',
    'additional_controls_authority':'source-authored structural/adversarial/metamorphic controls only; no new browser reference claim',
    'parent_full_workspace':'awaiting parent report; final production approval subject not sealed'}
q.write(P/'checks/parent-qualification.json',review);q.qualification();q.verify_migration(False);q.verify_policy(False)
prefix=['cargo','run','--release','--locked','-p','xtask','--','conformance']
for name,args,needle in [('v12-pending',['candidate-verify',q.V12],'approval must be an object'),
    ('ledger-stale',['verify-ownership'],'stale'),('v11-stale',['candidate-approval-subject',q.PRIOR],'stale')]:
    r=subprocess.run(prefix+args,cwd=R,env=q.ENV,capture_output=True)
    q.put(P/'checks'/f'{name}.stdout',r.stdout);q.put(P/'checks'/f'{name}.stderr',r.stderr)
    q.require(r.returncode!=0 and not r.stdout and needle in r.stderr.decode(),'expected pending rejection missing')
    print(name+': correctly blocked',flush=True)
for name in ['jpeg190-freeze.py','jpeg190-final-records.py']:
    q.put(P/'orchestration'/name,(Path('/tmp/opencode')/name).read_bytes())
q.check_freeze();print('MPF final records bound; exact HDR failures preserved; parent full-suite report awaited.')
