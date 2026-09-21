import hashlib
import json
from pathlib import Path
import types

ROOT=Path('/home/laborant/excalidraw-render.orig');P=ROOT/'tests/conformance/proposals/jpeg187-requalification-v1'
q=types.ModuleType('frozen');q.__file__=str(P/'qualify.py')
exec(compile((P/'qualify.py').read_bytes(),q.__file__,'exec'),q.__dict__)
q.check_freeze();q.check_final_findings();q.check_sampling_pins(q.read(P/'final-checks/production-qualification.json'));q.verify_migration(False)
for name in ['jpeg187-final-retry.py','jpeg187-final-records.py','jpeg187-draft-checkpoint.py']:
    q.put(P/'orchestration-successors'/name,(Path('/tmp/opencode')/name).read_bytes())
q.put(P/'final-checks/parent-freeze-authorization.json',Path('/tmp/opencode/jpeg187-parent-freeze.json').read_bytes())
build=q.read(ROOT/q.V9/'native-capture.json')['build']
judgments=q.read(ROOT/q.V9/'judgments.json')
summary=[]
for r in judgments['rows']:
    summary.append({'id':r['id'],'passed':r['passed'],'whole':r['whole'],'crops':r['crops']})
q.write(P/'final-checks/capture-summary.json',{'approval':None,'status':'capture-complete-draft-awaiting-parent-full-suite',
    'v9_subject_sha256':q.read(P/'migration-plan.json')['representative_subject_sha256'],
    'build':build,'inventory_keys':168,'inventory_unchanged':168,
    'jpeg_vectors':{'canonical_pass':64,'known_failure':12,'exact_failure_pins':24},
    'representative_comparisons':summary,'semantics':q.read(ROOT/q.V9/'semantics.json'),
    'pending_counts':q.read(P/'migration-plan.json')['counts'],
    'new_nested_defects':[],'known_nested_sampling_failures_match_old':True,
    'known_pdfium_alpha_unchanged':{'changed_pixels':24,'ssim_bits':'3febf9303648d4c8','exception':False}})
files=[]
for base in [P,ROOT/q.V9,ROOT/q.BRIDGE]:
    for p in sorted(base.rglob('*')):
        if p.is_file() and p.name not in ['README.md','file-inventory.json','readiness.json'] and '__pycache__' not in p.parts:
            files.append(q.desc(p))
manifest={'schema':'excalidraw-render/jpeg187-capture-checkpoint/v1','approval':None,
          'status':'draft-not-final-approval-subject-awaiting-parent-full-suite',
          'representative_subject_sha256':q.read(P/'migration-plan.json')['representative_subject_sha256'],
          'scope':'stable-graph-color-only; 15 new owners proposed, 17 source refreshes, 6 sampling cells remain #187 gaps',
          'files':files,'baseline_refresh':False,'live_migration_applied':False}
bundle={'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in manifest.items() if k!='approval'},'files':files}
q.write(P/'capture-draft-manifest.json',manifest);q.write(P/'capture-draft-subject.json',bundle)
print('DRAFT checkpoint sha256:'+q.sha(q.canonical(bundle)))
print('V9 '+manifest['representative_subject_sha256'])
print('Files:',len(files),'build executable:',build['executable_sha256'])
