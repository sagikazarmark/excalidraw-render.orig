from pathlib import Path
import subprocess
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/webp191-composition-requalification-v1'
q=types.ModuleType('frozen');q.__file__=str(P/'workflow.py')
exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec'),q.__dict__)
q.check_freeze();q.verify_migration(False)
proofs=[]
for name in ['test_workflow.py','test_suite_gate.py']:
    r=subprocess.run(['python3',str(P/name)],cwd=R,env=q.ENV,capture_output=True)
    out=P/'checks'/('final-'+name+'.stdout');err=P/'checks'/('final-'+name+'.stderr')
    q.put(out,r.stdout);q.put(err,r.stderr);q.require(r.returncode==0,r.stderr.decode())
    proofs.append({'test_source':q.desc(P/name),'stdout':q.desc(out),'stderr':q.desc(err),'passed':True})
q.put(P/'orchestration/webp191-seal-suite.py',Path(__file__).read_bytes())
q.write(P/'checks/final-checker-test-proof.json',{'approval':None,'isolated_original_tests':28,'suite_successor_tests':9,'results':proofs,
    'entry':q.desc(P/'suite_gate.py'),'frozen_workflow':q.desc(P/'workflow.py'),'frozen_receipt_entry':q.desc(P/'receipt_gate.py'),
    'production_xtask_test_capture_or_measurement_changed':False,'live_metadata_changed':False})
r=subprocess.run(['python3',str(P/'suite_gate.py'),'seal'],cwd=R,env=q.ENV,capture_output=True)
print(r.stdout.decode());print(r.stderr.decode());raise SystemExit(r.returncode)
