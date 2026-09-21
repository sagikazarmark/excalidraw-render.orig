import json
from pathlib import Path
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/webp191-composition-requalification-v1'
q=types.ModuleType('webp');q.__file__=str(P/'workflow.py')
exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec'),q.__dict__)
record={'final_current_capture_authorized':True,
 'parent_freeze_message':'Freeze #191 WebP composition-only correction on stable HEADdd3389a dependencies now. Private shared standalone-frame helper uses singlefallibleallocation/checkedbounds and earlydrops; framezero placedclearRGBAcanvas thenexistingICC/orientation. FiveWebPsampletests eightWebPunit pass formatClippy. StandardsSpecpre-freeze nofindings. Partialalpha12cellsdeferred; no PNGfloor/PDFcomparisonpolicy changes. Proceedfinal168/V13bridgev7/protectedexact11owners25hashrefresh1rep withnosource/toolingeditsafterfreeze.',
 'new_owner_selectors':{key:'corrected_composition_cells_match_reference' for key in sorted(q.GAPS)}}
path=Path('/tmp/opencode/webp191-parent-freeze.json')
with path.open('x') as f:json.dump(record,f,indent=2);f.write('\n')
q.freeze(str(path));print('Exact composition-only freeze and eleven selectors recorded.')
