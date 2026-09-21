#!/usr/bin/env python3
"""Fresh protected rerun explicitly bound to the qualified V14 executable."""
import importlib.util
import json
from pathlib import Path
import subprocess

spec=importlib.util.spec_from_file_location('collection',Path(__file__).with_name('collect.py'))
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
ROOT,P=c.ROOT,c.P
native=json.loads((ROOT/c.V14/'native-capture.json').read_bytes())
exe=ROOT/'target/release/excalidraw-render'
build=native['build']
assert c.sha(exe.read_bytes())==build['executable_sha256']
scene=(ROOT/'tests/conformance/pdf-backend-qualification/project-operations.excalidraw').read_bytes()
out=P/'protected-bound'
original=ROOT/'tests/conformance/candidates/stage4-pdf-backend-requalification-v2'
for i in range(2):
    assert c.sha(exe.read_bytes())==build['executable_sha256']
    r=subprocess.run([str(exe),'-','--output','-','--format','pdf','--report-format','json','--pdf-raster-fallback','forbid'],cwd=ROOT,env=c.ENV,input=scene,capture_output=True,check=True)
    assert r.stdout==(original/'candidate.pdf').read_bytes() and r.stderr==(original/'candidate-report.json').read_bytes()
    c.put(out/f'run-{i}.pdf',r.stdout);c.put(out/f'run-{i}-report.json',r.stderr)
    assert c.sha(exe.read_bytes())==build['executable_sha256']
c.run('bound-protected-structure',['bash','tools/pdf-harness/run-structure.sh','inspect',str(out/'run-0.pdf'),'tests/conformance/pdf-backend-qualification/project-operations-structure.json','pass',str(out/'structure.json')])
for viewer in ['pdfium','pdfjs']:
    for i in range(2):
        dest=out/f'{viewer}-{i}.rgba'
        c.run(f'bound-protected-{viewer}-{i}',['bash',f'tools/pdf-harness/run-{viewer}.sh','render',str(out/'run-0.pdf'),str(dest),'520','200'])
        assert dest.read_bytes()==(original/f'candidate-{viewer}.rgba').read_bytes()
files=[{'path':str(p.relative_to(ROOT)),'byte_length':len(p.read_bytes()),'sha256':c.sha(p.read_bytes())} for p in sorted(out.iterdir())]
c.put(P/'protected-build-binding.json',(json.dumps({'build':build,'executable_checked_before_and_after_each_run':True,'files':files},indent=2)+'\n').encode())
