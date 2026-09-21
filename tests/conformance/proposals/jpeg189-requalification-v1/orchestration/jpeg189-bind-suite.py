from pathlib import Path
import subprocess
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/jpeg189-requalification-v1'
s=types.ModuleType('suite');s.__file__=str(P/'suite_gate.py')
exec(compile((P/'suite_gate.py').read_bytes(),s.__file__,'exec'),s.__dict__)
q=s.frozen_workflow();q.check_freeze()
raw=Path('/home/laborant/.local/share/opencode/tool-output/tool_0c281dfba001UIHGAkuDc6S5rs').read_bytes()
classification=s.parser().classify(raw)
q.put(P/'checks/full-workspace.raw.log',raw)
message="""Fullworkspacecomplete exit1exact5pendingstaleownership/rep ONLY. Rawlog /home/laborant/.local/share/opencode/tool-output/tool_0c281dfba001UIHGAkuDc6S5rs. Command cargo test --locked --workspace --no-fail-fast pinnedenvCARGO_INCREMENTAL=0 allprofilesdebug0 timeout1800000 completednormally. Fast601passed library404passed7ignored; overalllikely1232pass5fail19ignored countstrictactual. Capture/classify raw exact5identitiesmessagescodepaths unknownfailreject, explicitnotwaived. Needsealpendingproduction nowviaexistingstrong109receiptchecker no frozenbyteedit useauthsuccessorifparsernotes backtraceplacement differs samefailure bodyacceptableonlynonsemantic note. Additionalreadme finalaccurate. ReturnfinalprodsubjectV11same +verifycmd tests, noactualapproval/live/commit. Parentindependentreviewnext. Don'tfullsuiteagain.
"""
q.put(P/'checks/parent-full-suite-message.txt',message.encode())
command=['cargo','run','--release','--locked','-p','xtask','--','conformance','record-test-owner','surface_metadata_contract','editor_state_is_silent_preserved_and_independent_of_scene_and_frame_exports','Explicit reviewed editor-only surface contract','surface:unknown:/unknown:svg']
before=(R/q.LEDGER).read_bytes()
result=subprocess.run(command,cwd=R,env=q.ENV,capture_output=True)
q.require(result.returncode!=0 and not result.stdout and b'error: stale ownership evidence' in result.stderr,'unexpected reproduction result')
q.require((R/q.LEDGER).read_bytes()==before,'ledger changed')
q.put(P/'checks/unknown-owner-reproduction.stdout',result.stdout);q.put(P/'checks/unknown-owner-reproduction.stderr',result.stderr)
record={'schema':'excalidraw-render/jpeg189-full-suite-record/v1','approval':None,
 'command':['cargo','test','--locked','--workspace','--no-fail-fast'],'exit_code':1,'completed_normally':True,'timeout_ms':1800000,'outcome':'failed-only-at-exact-pending-binding-tests',
 'environment':{'toolchain':'1.97.1-x86_64-unknown-linux-gnu','CARGO_INCREMENTAL':'0','CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0','CARGO_PROFILE_RELEASE_DEBUG':'0'},
 'raw_log':q.desc(P/'checks/full-workspace.raw.log'),'parent_message':q.desc(P/'checks/parent-full-suite-message.txt'),'classification':classification,
 'freeze':q.desc(P/'final-freeze.json'),'test_source':q.desc(R/q.SOURCE),'producer_sources':q.read(R/q.V11/'native-capture.json')['producer_sources'],
 'unknown-owner-cli-reproduction':{'command':command,'exit_code':result.returncode,'stdout':q.desc(P/'checks/unknown-owner-reproduction.stdout'),'stderr':q.desc(P/'checks/unknown-owner-reproduction.stderr'),'ledger_before_sha256':q.sha(before),'ledger_after_sha256':q.sha((R/q.LEDGER).read_bytes())},
 'post_approval_required':['cargo test --locked -p xtask --bin xtask --test coverage --test representative_candidate --no-fail-fast']}
q.write(P/'checks/full-suite-record.json',record)
q.put(P/'orchestration/jpeg189-bind-suite.py',Path(__file__).read_bytes())
q.write(P/'checks/suite-successor.json',{'approval':None,'scope':'unbound strict full-suite classifier; only numeric totals changed from authenticated prior parser; same exact error identities/locations/messages and NOTE exclusion',
 'new_entry':q.desc(P/'suite_gate.py'),'historical_parser':q.desc(R/s.PARSER),'historical_parser_subject':s.PRIOR,
 'frozen_workflow':q.desc(P/'workflow.py'),'frozen_receipt_entry':q.desc(P/'receipt_gate.py'),
 'original_draft_unchanged':q.desc(P/'capture-draft-subject.json'),'original_focused_record_unchanged':q.desc(P/'checks/parent-qualification.json')})
s.verify_suite();q.check_freeze()
print(record['classification'])
