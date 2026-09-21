import json
from pathlib import Path
import types

ROOT=Path('/home/laborant/excalidraw-render.orig');P=ROOT/'tests/conformance/proposals/jpeg188-requalification-v1'
q=types.ModuleType('frozen_icc');q.__file__=str(P/'workflow.py')
exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec'),q.__dict__)
frozen=q.check_freeze();q.qualification();q.verify_migration(False)
q.collector().verify_snapshots(P/'old/snapshots');q.collector().verify_snapshots(P/'final-current/snapshots')
subject=q.task('candidate-approval-subject',q.V10).decode().strip()
q.require(subject==q.read(P/'migration-plan.json')['representative_subject_sha256'],'subject mismatch')
q.put(P/'orchestration/jpeg188-draft.py',Path(__file__).read_bytes())
artifacts=[]
for fmt in ['svg','png','pdf']:
    for name in [f'native.{fmt}',f'native-{fmt}-report.json']:
        old=ROOT/'tests/conformance/candidates/stage4-frames-text-assets-v9'/name
        new=ROOT/q.V10/name
        q.require(old.read_bytes()==new.read_bytes(),'approved V9 artifact/report changed')
        artifacts.append(q.desc(new))
views=q.read(ROOT/q.V10/'viewer-evidence/collection.json')
for row in views['captures']:
    name=Path(row['raster']['path']).name
    q.require((ROOT/row['raster']['path']).read_bytes()==(ROOT/'tests/conformance/candidates/stage4-frames-text-assets-v9/viewer-evidence'/name).read_bytes(),'viewer moved')
summary={'approval':None,'status':'draft-capture-complete-awaiting-parent-suite-results',
         'representative_subject_sha256':subject,'icc_vectors':160,'canonical_color_vectors':104,'strict_recovery_vectors':56,
         'jpeg187_vectors':76,'jpeg187_canonical_vectors':64,'jpeg187_known_failure_vectors':12,'jpeg187_exact_pins':24,
         'full_inventory_keys':168,'full_inventory_records_equal_old':168,
         'protected_pdf_equality':True,'representative_artifacts_reports_and_viewers_equal_v9':True,
         'artifact_descriptors':artifacts,'native_build':q.read(ROOT/q.V10/'native-capture.json')['build'],
         'semantics':q.read(ROOT/q.V10/'semantics.json'),'judgments':q.read(ROOT/q.V10/'judgments.json')['rows'],
         'projected_counts':q.read(P/'migration-plan.json')['counts'],'frozen_files_checked':len(frozen['producer_files'])}
q.write(P/'checks/capture-summary.json',summary)
files=[]
for base in [P,ROOT/q.V10,ROOT/q.BRIDGE]:
    for path in sorted(base.rglob('*')):
        if path.is_file() and path.name not in ['README.md','file-inventory.json'] and '__pycache__' not in path.parts:files.append(q.desc(path))
manifest={'schema':'excalidraw-render/jpeg188-icc-draft-checkpoint/v1','approval':None,
          'status':'draft-awaiting-parent-suite-not-final-production-approval-request','representative_subject_sha256':subject,
          'scope':'jpeg188-icc; 16 new ICC owners, 64 source refreshes, V9-to-V10; no dependency/reference/baseline changes','files':files}
bundle={'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in manifest.items() if k!='approval'},'files':files}
q.write(P/'capture-draft-manifest.json',manifest);q.write(P/'capture-draft-subject.json',bundle)
print('V10 '+subject);print('DRAFT sha256:'+q.sha(q.canonical(bundle)))
print('Files',len(files),'frozen',len(frozen['producer_files']),'CLI',summary['native_build']['executable_sha256'])
