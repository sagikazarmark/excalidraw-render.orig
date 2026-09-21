import json
import os
from pathlib import Path
import subprocess
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/jpeg187-requalification-v1'
q=types.ModuleType('frozen');q.__file__=str(P/'qualify.py')
exec(compile((P/'qualify.py').read_bytes(),q.__file__,'exec'),q.__dict__)
q.check_freeze()
result=subprocess.run(['python3',str(P/'test_split_verification.py')],cwd=R,env=dict(q.ENV,PYTHONDONTWRITEBYTECODE='1'),capture_output=True)
q.require(result.returncode==0,result.stderr.decode())
q.put(P/'final-checks/split-negative-tests.stdout',result.stdout)
q.put(P/'final-checks/split-negative-tests.stderr',result.stderr)
q.put(P/'orchestration-successors/jpeg187-seal-split.py',Path(__file__).read_bytes())
proof={'approval':None,'status':'strict-split-successor-tests-passed','tests':7,
       'checker':q.desc(P/'split-verification.py'),'test_source':q.desc(P/'test_split_verification.py'),
       'stdout':q.desc(P/'final-checks/split-negative-tests.stdout'),'stderr':q.desc(P/'final-checks/split-negative-tests.stderr'),
       'frozen_checker_unchanged':q.desc(P/'qualify.py'),'production_xtask_or_candidate_changed':False}
q.write(P/'final-checks/split-successor-test-proof.json',proof)
result=subprocess.run(['python3',str(P/'split-verification.py'),'seal'],cwd=R,env=q.ENV,capture_output=True)
print(result.stdout.decode());print(result.stderr.decode())
raise SystemExit(result.returncode)
