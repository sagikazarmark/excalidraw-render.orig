from pathlib import Path
import subprocess
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/jpeg190-mpf-requalification-v1'
s=types.ModuleType('suite');s.__file__=str(P/'suite_gate.py')
exec(compile((P/'suite_gate.py').read_bytes(),s.__file__,'exec'),s.__dict__)
q=s.frozen_workflow();q.check_freeze()
raw=Path('/home/laborant/.local/share/opencode/tool-output/tool_0c317b2e5001j4sIC9NDTQfgyV').read_bytes()
classification=s.parser().classify(raw)
q.put(P/'checks/full-workspace.raw.log',raw)
message="""FullsuiteCOMPLETEraw /home/laborant/.local/share/opencode/tool-output/tool_0c317b2e5001j4sIC9NDTQfgyV. command cargo test --locked --workspace --no-fail-fast pinnedmemoryprofiles0 env timeout1800000 completedexit1 ONLY5anticipatedstaleownership/representative failures (same2bin2coverage1consumer). fast613pass,lib404pass7ignore, overallexpected1244pass5fail19ignore verifystrictactual. Sealproductionpolicy/migrationpacket inclrawlogexactclassifiernewtotalsnormalcomplete; noapprovalwaive. Needcurrentexplicit16criteriondiff inapprovalsubjectvalidatesonlythis+16new28refresh1rep. Newstrongscopejpeg190-mpf actualdesignatedaffirmativebothsubjects receipt1096commands4fileatomicrestorepolicy included (majorreview). Source/freezeunchanged. ReturnfinalsubjectV12same verifycmdmeaningfultests. No sourceedit/recollect/noapprovalcommit.
"""
q.put(P/'checks/parent-full-suite-message.txt',message.encode())
command=['cargo','run','--release','--locked','-p','xtask','--','conformance','record-test-owner','surface_metadata_contract','editor_state_is_silent_preserved_and_independent_of_scene_and_frame_exports','Explicit reviewed editor-only surface contract','surface:unknown:/unknown:svg']
before=(R/q.LEDGER).read_bytes();result=subprocess.run(command,cwd=R,env=q.ENV,capture_output=True)
q.require(result.returncode!=0 and not result.stdout and b'error: stale ownership evidence' in result.stderr,'diagnostic differs')
q.require((R/q.LEDGER).read_bytes()==before,'diagnostic mutated ledger')
q.put(P/'checks/unknown-owner-reproduction.stdout',result.stdout);q.put(P/'checks/unknown-owner-reproduction.stderr',result.stderr)
q.write(P/'checks/full-suite-record.json',{'schema':'excalidraw-render/jpeg190-full-suite-record/v1','approval':None,
 'command':['cargo','test','--locked','--workspace','--no-fail-fast'],'exit_code':1,'completed_normally':True,'timeout_ms':1800000,'outcome':'failed-only-at-exact-pending-binding-tests',
 'environment':{'toolchain':'1.97.1-x86_64-unknown-linux-gnu','CARGO_INCREMENTAL':'0','CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0','CARGO_PROFILE_RELEASE_DEBUG':'0'},
 'raw_log':q.desc(P/'checks/full-workspace.raw.log'),'parent_message':q.desc(P/'checks/parent-full-suite-message.txt'),'classification':classification,
 'freeze':q.desc(P/'final-freeze.json'),'test_source':q.desc(R/q.SOURCE),'producer_sources':q.read(R/q.V12/'native-capture.json')['producer_sources'],
 'unknown-owner-cli-reproduction':{'command':command,'exit_code':result.returncode,'stdout':q.desc(P/'checks/unknown-owner-reproduction.stdout'),'stderr':q.desc(P/'checks/unknown-owner-reproduction.stderr'),'ledger_before_sha256':q.sha(before),'ledger_after_sha256':q.sha((R/q.LEDGER).read_bytes())},
 'post_approval_required':['cargo test --locked -p xtask --bin xtask --test coverage --test representative_candidate --no-fail-fast']})
q.put(P/'orchestration/jpeg190-bind-suite.py',Path(__file__).read_bytes())
q.write(P/'checks/suite-successor.json',{'approval':None,'scope':'strict full-suite classifier; authenticated old parser with numeric totals only; exact five identities/messages remain required',
 'new_entry':q.desc(P/'suite_gate.py'),'parser':q.desc(R/s.PARSER),'parser_subject':s.PRIOR,
 'frozen_workflow':q.desc(P/'workflow.py'),'frozen_receipt_entry':q.desc(P/'receipt_gate.py'),
 'policy_transition':q.desc(P/'policy/criterion-transition.json'),'preserved_draft':q.desc(P/'capture-draft-subject.json')})
s.verify_suite();q.check_freeze();print(classification)
