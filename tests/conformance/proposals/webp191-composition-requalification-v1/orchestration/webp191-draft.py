from pathlib import Path
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/webp191-composition-requalification-v1'
q=types.ModuleType('frozen');q.__file__=str(P/'workflow.py')
exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec'),q.__dict__)
f=q.check_freeze();q.qualification();q.verify_migration(False)
c=q.collector();c.verify_snapshots(P/'old/snapshots');c.verify_snapshots(P/'final-current/snapshots')
subject=q.task('candidate-approval-subject',q.V13).decode().strip()
q.require(subject==q.read(P/'migration-plan.json')['representative_subject_sha256'],'subject moved')
native=q.read(R/q.V13/'native-capture.json');views=q.read(R/q.V13/'viewer-evidence/collection.json')
for row in views['captures']:
    name=Path(row['raster']['path']).name
    q.require((R/row['raster']['path']).read_bytes()==(R/q.PRIOR/'viewer-evidence'/name).read_bytes(),'viewer changed')
artifacts=[]
for fmt in ['svg','png','pdf']:
    for name in [f'native.{fmt}',f'native-{fmt}-report.json']:
        q.require((R/q.V13/name).read_bytes()==(R/q.PRIOR/name).read_bytes(),'native artifact/report changed')
        artifacts.append(q.desc(R/q.V13/name))
q.put(P/'orchestration/webp191-draft.py',Path(__file__).read_bytes())
q.write(P/'checks/capture-summary.json',{'approval':None,'status':'draft-awaiting-parent-full-suite','representative_subject_sha256':subject,
 'native_build':native['build'],'full_inventory_keys':168,'full_inventory_records_equal_old':168,
 'sample_cells':36,'qualified_cells':24,'residual_partial_cells':12,'pass_vectors':{'svg':72,'png':48,'pdf':48},
 'measurement_transition':q.measurements(),'protected_pdf_equality':True,'representative_native_reports_viewers_equal_v12':True,
 'artifacts':artifacts,'semantics':q.read(R/q.V13/'semantics.json'),'judgments':q.read(R/q.V13/'judgments.json')['rows'],
 'counts_after':q.read(P/'migration-plan.json')['counts'],'frozen_files_checked':len(f['producer_files']),
 'resource_scope':'reviewed fallible allocation/checked bounds/early drops, not physical OOM closure',
 'pdf_partial_scope':'synthetic rounded projection mismatch, not measured PDF viewer defect'})
files=[q.desc(p) for base in [P,R/q.V13,R/q.BRIDGE] for p in sorted(base.rglob('*')) if p.is_file() and p.name not in ['README.md','file-inventory.json'] and '__pycache__' not in p.parts]
files.append(q.desc(R/q.NATIVE_NEW))
m={'schema':'excalidraw-render/webp191-composition-capture-checkpoint/v1','approval':None,'status':'draft-awaiting-parent-full-suite-not-final-production-approval-request',
   'representative_subject_sha256':subject,'scope':'webp191-composition; 11 new owners/25 source refreshes/V12-to-V13; static and partial-alpha/PDF policies unchanged','files':files}
b={'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in m.items() if k!='approval'},'files':files}
q.write(P/'capture-draft-manifest.json',m);q.write(P/'capture-draft-subject.json',b);q.check_freeze()
print('V13 '+subject);print('DRAFT sha256:'+q.sha(q.canonical(b)));print('Files',len(files),'frozen',len(f['producer_files']),'CLI',native['build']['executable_sha256'])
