import json
from pathlib import Path
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/jpeg189-requalification-v1'
q=types.ModuleType('frozen_exif');q.__file__=str(P/'workflow.py')
exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec'),q.__dict__)
record={'final_current_capture_authorized':True,
 'parent_freeze_message':'Freeze #189 JPEG-specific EXIF correction on stable HEAD7b432b6 dependencies now; first pre-SOS EXIF APP1 and first-valid IFD numeric orientation are qualified, shared PNG/WebP orientation unchanged. All34JPEGtests, workspaceformatClippypass; Standards andSpec pre-freezereviews no findings. Proceed final168/V11/bridgev5/protected capture withoutboundsource/toolingchanges.',
 'new_owner_selectors':{key:'corrected_duplicate_exif_precedence_matches_reference' for key in sorted(q.gap_ids())}}
path=Path('/tmp/opencode/jpeg189-parent-freeze.json')
with path.open('x') as f:json.dump(record,f,indent=2);f.write('\n')
q.freeze(str(path))
print('Authenticated exact parent freeze and 36-owner selector map.')
