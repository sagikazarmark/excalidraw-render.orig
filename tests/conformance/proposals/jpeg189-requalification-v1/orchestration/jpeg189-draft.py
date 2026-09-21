import json
from pathlib import Path
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/jpeg189-requalification-v1'
q=types.ModuleType('frozen');q.__file__=str(P/'workflow.py')
exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec'),q.__dict__)
freeze=q.check_freeze();q.qualification();q.verify_migration(False)
c=q.collector();c.verify_snapshots(P/'old/snapshots');c.verify_snapshots(P/'final-current/snapshots')
subject=q.task('candidate-approval-subject',q.V11).decode().strip()
q.require(subject==q.read(P/'migration-plan.json')['representative_subject_sha256'],'subject drift')
native=q.read(R/q.V11/'native-capture.json');views=q.read(R/q.V11/'viewer-evidence/collection.json')
for row in views['captures']:
    name=Path(row['raster']['path']).name
    q.require((R/row['raster']['path']).read_bytes()==(R/q.PRIOR/'viewer-evidence'/name).read_bytes(),'viewer pixels changed')
artifacts=[]
for fmt in ['svg','png','pdf']:
    for name in [f'native.{fmt}',f'native-{fmt}-report.json']:
        q.require((R/q.V11/name).read_bytes()==(R/q.PRIOR/name).read_bytes(),'native bytes/report changed')
        artifacts.append(q.desc(R/q.V11/name))
q.put(P/'orchestration/jpeg189-draft.py',Path(__file__).read_bytes())
q.write(P/'checks/capture-summary.json',{'approval':None,'status':'draft-awaiting-parent-full-suite',
    'representative_subject_sha256':subject,'native_build':native['build'],'exif_vectors_canonical':164,'icc_vectors':160,'icc_color_vectors':104,'icc_recovery_vectors':56,
    'jpeg187_vectors':76,'jpeg187_canonical_vectors':64,'jpeg187_known_failure_vectors':12,'jpeg187_exact_pins':24,
    'inventory_keys':168,'inventory_records_equal_old':168,'protected_pdf_equality':True,'representative_native_reports_viewers_equal_v10':True,
    'artifacts':artifacts,'semantics':q.read(R/q.V11/'semantics.json'),'judgments':q.read(R/q.V11/'judgments.json')['rows'],
    'counts_after':q.read(P/'migration-plan.json')['counts'],'frozen_files_checked':len(freeze['producer_files'])})
files=[q.desc(p) for base in [P,R/q.V11,R/q.BRIDGE] for p in sorted(base.rglob('*')) if p.is_file() and p.name not in ['README.md','file-inventory.json'] and '__pycache__' not in p.parts]
m={'schema':'excalidraw-render/jpeg189-exif-capture-checkpoint/v1','approval':None,'status':'draft-awaiting-parent-full-suite-not-final-production-approval-request',
   'representative_subject_sha256':subject,'scope':'jpeg189-exif; 36 new owners, 46 source refreshes, V10-to-V11; unchanged graph/shared orientation/reference/criteria/keys','files':files}
bundle={'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in m.items() if k!='approval'},'files':files}
q.write(P/'capture-draft-manifest.json',m);q.write(P/'capture-draft-subject.json',bundle)
q.check_freeze()
print('V11 '+subject);print('DRAFT sha256:'+q.sha(q.canonical(bundle)));print('Files',len(files),'frozen',len(freeze['producer_files']),'CLI',native['build']['executable_sha256'])
