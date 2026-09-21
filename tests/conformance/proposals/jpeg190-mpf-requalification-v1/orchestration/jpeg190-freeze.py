import json
from pathlib import Path
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/jpeg190-mpf-requalification-v1'
q=types.ModuleType('mpf');q.__file__=str(P/'workflow.py')
exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec'),q.__dict__)
record={'final_current_capture_authorized':True,
 'parent_freeze_message':'Freeze #190 MPF-validation-only on stable HEAD80670ae graph now. Parent review settled bounded primary two-image index with optional UID/TotalFrames and nonoverlapping relocated entries; rejects secondary Index IFDs while admitting narrow Attribute version/individual-number IFD and rejecting duplicate secondary MPF. All46JPEGfocusedtests including12gainmap and10libraryJPEGcodec tests pass, formatClippypass; Standardsnohardblock SpecP2secondaryindexfixedconfirmed. HDRprojectiondeferredreducedIDCTmissing. Proceed final168/V12bridgev6/protected and exact16criterion/owner migration without further frozen edits.',
 'new_owner_selectors':{key:'corrected_mpf_index_ranges_have_exact_recovery' for key in sorted(q.cell_sets()[1])}}
path=Path('/tmp/opencode/jpeg190-parent-freeze.json')
with path.open('x') as f:json.dump(record,f,indent=2);f.write('\n')
q.freeze(str(path));print('Exact MPF-only freeze and 16-selector mapping recorded; live policy remains old.')
