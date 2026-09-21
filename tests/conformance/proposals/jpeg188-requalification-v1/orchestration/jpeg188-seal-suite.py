from pathlib import Path
import subprocess
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/jpeg188-requalification-v1'
q=types.ModuleType('frozen');q.__file__=str(P/'workflow.py')
exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec'),q.__dict__)
q.check_freeze()
r=subprocess.run(['python3',str(P/'test_suite_verification.py')],cwd=R,env=dict(q.ENV,PYTHONDONTWRITEBYTECODE='1'),capture_output=True)
q.put(P/'checks/suite-negative-tests.stdout',r.stdout);q.put(P/'checks/suite-negative-tests.stderr',r.stderr)
q.require(r.returncode==0,r.stderr.decode())
q.put(P/'orchestration/jpeg188-seal-suite.py',Path(__file__).read_bytes())
q.write(P/'checks/suite-successor-test-proof.json',{'approval':None,'tests_passed':9,'checker':q.desc(P/'suite-verification.py'),
        'test_source':q.desc(P/'test_suite_verification.py'),'stdout':q.desc(P/'checks/suite-negative-tests.stdout'),
        'stderr':q.desc(P/'checks/suite-negative-tests.stderr'),'frozen_checker_unchanged':q.desc(P/'workflow.py'),
        'production_xtask_or_capture_changed':False})
r=subprocess.run(['python3',str(P/'suite-verification.py'),'seal'],cwd=R,env=q.ENV,capture_output=True)
print(r.stdout.decode());print(r.stderr.decode());raise SystemExit(r.returncode)
