from pathlib import Path
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/jpeg190-mpf-requalification-v1'
q=types.ModuleType('frozen');q.__file__=str(P/'workflow.py')
exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec'),q.__dict__)
f=q.check_freeze();q.qualification();q.verify_migration(False);q.verify_policy(False)
c=q.collector();c.verify_snapshots(P/'old/snapshots');c.verify_snapshots(P/'final-current/snapshots')
subject=q.task('candidate-approval-subject',q.V12).decode().strip()
q.require(subject==q.read(P/'migration-plan.json')['representative_subject_sha256'],'subject moved')
native=q.read(R/q.V12/'native-capture.json');views=q.read(R/q.V12/'viewer-evidence/collection.json')
for row in views['captures']:
    name=Path(row['raster']['path']).name
    q.require((R/row['raster']['path']).read_bytes()==(R/q.PRIOR/'viewer-evidence'/name).read_bytes(),'viewer pixels changed')
artifacts=[]
for fmt in ['svg','png','pdf']:
    for name in [f'native.{fmt}',f'native-{fmt}-report.json']:
        q.require((R/q.V12/name).read_bytes()==(R/q.PRIOR/name).read_bytes(),'representative report/artifact changed')
        artifacts.append(q.desc(R/q.V12/name))
q.put(P/'orchestration/jpeg190-draft.py',Path(__file__).read_bytes())
q.write(P/'checks/capture-summary.json',{'approval':None,'status':'draft-awaiting-parent-full-suite','representative_subject_sha256':subject,
    'native_build':native['build'],'gain_map_vectors':104,'canonical_sdr_vectors':24,'strict_recovery_vectors':64,
    'hdr_known_failure_vectors':16,'hdr_exact_pins_unchanged':32,'decoder_or_hdr_change':False,
    'jpeg187_vectors':76,'jpeg187_canonical_vectors':64,'jpeg187_known_failure_vectors':12,'jpeg187_exact_pins':24,
    'icc_applicable_gates':160,'exif_canonical_vectors':164,'full_inventory_keys':168,'full_inventory_records_equal_old':168,
    'protected_pdf_equality':True,'representative_native_reports_viewers_equal_v11':True,
    'artifacts':artifacts,'semantics':q.read(R/q.V12/'semantics.json'),'judgments':q.read(R/q.V12/'judgments.json')['rows'],
    'proposed_criterion_changes':16,'new_owner_count':16,'existing_source_refreshes':28,'HDR_cells_still_gaps':8,
    'counts_after':q.read(P/'migration-plan.json')['counts'],'frozen_files_checked':len(f['producer_files'])})
files=[q.desc(p) for base in [P,R/q.V12,R/q.BRIDGE] for p in sorted(base.rglob('*')) if p.is_file() and p.name not in ['README.md','file-inventory.json'] and '__pycache__' not in p.parts]
m={'schema':'excalidraw-render/jpeg190-mpf-capture-checkpoint/v1','approval':None,'status':'draft-awaiting-parent-full-suite-not-final-production-approval-request',
   'representative_subject_sha256':subject,'scope':'jpeg190-mpf; 16 explicit criterion changes/16 new recovery owners/28 source refreshes/V11-to-V12; HDR projection deferred','files':files}
bundle={'schema':'excalidraw-render/conformance-approval-subject/v1','manifest':{k:v for k,v in m.items() if k!='approval'},'files':files}
q.write(P/'capture-draft-manifest.json',m);q.write(P/'capture-draft-subject.json',bundle)
q.check_freeze()
print('V12 '+subject);print('DRAFT sha256:'+q.sha(q.canonical(bundle)));print('Files',len(files),'frozen',len(f['producer_files']),'CLI',native['build']['executable_sha256'])
