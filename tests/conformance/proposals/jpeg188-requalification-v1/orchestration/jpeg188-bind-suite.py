import json
from pathlib import Path
import subprocess
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/jpeg188-requalification-v1'
q=types.ModuleType('frozen');q.__file__=str(P/'workflow.py')
exec(compile((P/'workflow.py').read_bytes(),q.__file__,'exec'),q.__dict__)
q.check_freeze()
s=types.ModuleType('suite');s.__file__=str(P/'suite-verification.py')
exec(compile((P/'suite-verification.py').read_bytes(),s.__file__,'exec'),s.__dict__)
raw=Path('/home/laborant/.local/share/opencode/tool-output/tool_0c1f93690001WBgc0K8u82mEbm').read_bytes()
classification=s.classify(raw)
q.put(P/'checks/full-workspace.raw.log',raw)
message="""ParentFULLSUITE completed exitfailONLY5expectedstalebindingtests across3targets (xtaskbin2,coverage2,rep1) asprior. RAWLOG /home/laborant/.local/share/opencode/tool-output/tool_0c1f93690001WBgc0K8u82mEbm. EntirecommandCARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_RELEASE_DEBUG=0 PATH=pinned cargo test --locked --workspace --no-fail-fast timeout1800000 completednormally. 595fastpass,404libpass7ignored, allremainingrenderer/CLI/assets/comparetests pass; aggregate likely1226passed5failed19ignored(total1231) countactualrawlogs. CaptureRAWlognewpacket checks andstrictparsecheck expectedfailureidentity/messages ONLY count no blanketwaiver. No sourcechanges; fixedproduction/test/xtaskfreeze. Seal finalproductionapprovalsubject now, testsvalidatependingfailure classification unknownerrorreject andauthscope. No fakeapproval. ReturnsubjectV10unchanged +reprocheckercmds+packetboundlist andfutureapplyprotocol. Parentwillindependentreviewbeforeaskapproval. No needre-runfullsuite.
"""
q.put(P/'checks/parent-full-suite-message.txt',message.encode())
command=['cargo','run','--release','--locked','-p','xtask','--','conformance','record-test-owner','surface_metadata_contract','editor_state_is_silent_preserved_and_independent_of_scene_and_frame_exports','Explicit reviewed editor-only surface contract','surface:unknown:/unknown:svg']
before=(R/q.LEDGER).read_bytes()
result=subprocess.run(command,cwd=R,env=q.ENV,capture_output=True)
q.require(result.returncode!=0 and not result.stdout and b'error: stale ownership evidence' in result.stderr,'unexpected diagnostic cause')
q.require((R/q.LEDGER).read_bytes()==before,'diagnostic mutated ledger')
q.put(P/'checks/unknown-owner-reproduction.stdout',result.stdout);q.put(P/'checks/unknown-owner-reproduction.stderr',result.stderr)
record={'schema':'excalidraw-render/jpeg188-full-suite-record/v1','approval':None,
 'command':['cargo','test','--locked','--workspace','--no-fail-fast'],'completed_normally':True,'timeout_ms':1800000,
 'outcome':'failed-only-at-exact-pending-binding-tests','environment':{'toolchain':'1.97.1-x86_64-unknown-linux-gnu','CARGO_INCREMENTAL':'0','CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0','CARGO_PROFILE_RELEASE_DEBUG':'0'},
 'raw_log':q.desc(P/'checks/full-workspace.raw.log'),'parent_message':q.desc(P/'checks/parent-full-suite-message.txt'),
 'classification':classification,'freeze':q.desc(P/'final-freeze.json'),'test_source':q.desc(R/q.SOURCE),
 'producer_sources':q.read(R/q.V10/'native-capture.json')['producer_sources'],
 'unknown-owner-cli-reproduction':{'command':command,'exit_code':result.returncode,'stdout':q.desc(P/'checks/unknown-owner-reproduction.stdout'),'stderr':q.desc(P/'checks/unknown-owner-reproduction.stderr'),'ledger_before_sha256':q.sha(before),'ledger_after_sha256':q.sha((R/q.LEDGER).read_bytes())},
 'post_approval_required':['cargo test --locked -p xtask --bin xtask --test coverage --test representative_candidate --no-fail-fast']}
q.write(P/'checks/full-suite-record.json',record)
q.put(P/'orchestration/jpeg188-bind-suite.py',Path(__file__).read_bytes())
q.write(P/'checks/suite-checker-successor.json',{'approval':None,'purpose':'Strict exact raw-log pending-failure classification before invoking unchanged frozen seal/verify; no test failure relabeled passing.',
 'frozen_checker':q.desc(P/'workflow.py'),'new_unbound_checker':q.desc(P/'suite-verification.py'),
 'preserved_original_qualification':q.desc(P/'checks/parent-qualification.json'),'preserved_draft':q.desc(P/'capture-draft-subject.json')})
s.verify_suite();q.check_freeze()
print(json.dumps(classification,indent=2))
