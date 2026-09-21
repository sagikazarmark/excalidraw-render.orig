import json
from pathlib import Path
import types

R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/jpeg187-requalification-v1'
q=types.ModuleType('frozen');q.__file__=str(P/'qualify.py')
exec(compile((P/'qualify.py').read_bytes(),q.__file__,'exec'),q.__dict__)
q.check_freeze()
s=types.ModuleType('split');s.__file__=str(P/'split-verification.py')
exec(compile((P/'split-verification.py').read_bytes(),s.__file__,'exec'),s.__dict__)
raw=Path('/home/laborant/.local/share/opencode/tool-output/tool_0c15b1496001nt41rWaBJDo10Y').read_bytes()
q.put(P/'final-checks/workspace-timeout.raw.log',raw)
message="""Parent finalverification complete combined commands NOTsinglecleanrun. Fullworkspace cargo test --locked --workspace --no-fail-fast timedout1200000ms undercaptureload: lib404passed7ignored; fast585 total, all584reportedok exceptlong compatibility_recovery_contract::unsupported_value_contract_covers_every_current_field_family pendingatkill. Log/home/laborant/.local/share/opencode/tool-output/tool_0c15b1496001nt41rWaBJDo10Y. Completedlongtestalone samefrozenbytes cargo test --locked -p excalidraw-render --test fast compatibility_recovery_contract::unsupported_value_contract_covers_every_current_field_family -- --exact PASSED816.82s. Completedremaining renderer integrationtargets allpass PNGparity10SVG8,etc19ignoreoverall; restworkspace cargo test --locked --workspace --exclude excalidraw-render --no-fail-fast: only5anticipatedstalebindingtests fail (2xtaskbin ownership,2xtaskcoverage ownership,1representativeconsumer V8stale). Allotherspass inclxtask101. Doctestrendererpass0. Needcapturelogsforcontinuations notfiletoolbecauseoutputshort? Can get tooloutput filenames via parent? tools outputs inline full known. Record honest parentattestationcommandsresults sourcebound finalfreeze; doNOTclaimalltestsgreenbeforeapproval. You can independently run only affectedxtasktargetsafterapproval pending tobeexplicit; existingstderrstale capturesalready. Needverify initialfast listcount using rg countallowed/tool; ifnot584 don'tclaim aggregatecount. Prepare finalpending productionpacket subject now basedunchangedsource/capture, include rawtimedlogvia script filecopy (useapplypatch/generatedoperation) and parentcompletion attestation. 15sourceownedproposal correct, same24samplingpins unchanged. Importantchecker sealing may requireallchecks passed; mustrepresent expectedpendingauthorizationfailures explicitly notwaivearbitrary. No approvedsourceedit. If checkerdesignrejectstimeout but can acceptsplitcompletionmodify newauthenticatedsuccessorunboundpacketonly, need independentlyreviewfinalscript. Return finalsubjectV9unchanged plusexact approval scope andread-onlyverify command. Thenparentdelegatesindependentpacketreview. DoNOTactualapprovalorcommit.
"""
q.put(P/'final-checks/parent-split-completion-message.txt',message.encode())
attestation={'schema':'excalidraw-render/jpeg187-parent-split-verification/v1','approval':None,'verification_mode':'combined-commands-not-a-single-clean-run',
 'parent_message':q.desc(P/'final-checks/parent-split-completion-message.txt'),
 'frozen_source_record':q.desc(P/'final-freeze.json'),'producer_sources':q.read(R/q.V9/'native-capture.json')['producer_sources'],'test_source':q.desc(R/q.SOURCE),
 'initial':{'command':['cargo','test','--locked','--workspace','--no-fail-fast'],'outcome':'timed-out','timeout_ms':1200000,'raw_log':q.desc(P/'final-checks/workspace-timeout.raw.log'),'verified_counts':s.parse_workspace(raw)},
 'long_test_continuation':{'command':['cargo','test','--locked','-p','excalidraw-render','--test','fast',s.LONG,'--','--exact'],'outcome':'passed','duration_seconds':816.82,'evidence_kind':'parent-session-attestation-no-raw-log-available'},
 'renderer_integrations':{'outcome':'all-remaining-targets-passed','png_parity_passed':10,'svg_parity_passed':8,'evidence_kind':'parent-session-attestation-no-raw-log-available','command_detail':'individual remaining integration target invocations; exact argv not supplied; no invented raw logs'},
 'remaining_workspace':{'command':['cargo','test','--locked','--workspace','--exclude','excalidraw-render','--no-fail-fast'],'outcome':'failed-only-at-enumerated-pending-bindings','expected_failures':s.EXPECTED,'failure_count':5,'all_other_targets_passed':True,'xtask_bin_passed':101,'evidence_kind':'parent-session-attestation-no-raw-log-available'},
 'renderer_doctests':{'outcome':'passed','tests':0,'evidence_kind':'parent-session-attestation-no-raw-log-available'},
 'reported_ignored_total':19,'all_tests_green_before_approval':False,
 'observed_pending_gate_errors':[q.desc(P/'final-checks'/name) for name in ['ledger-stale.stderr','v8-stale.stderr','v9-pending.stderr']],
 'after_actual_approval_required':['cargo test --locked -p xtask --bin xtask coverage::ownership::','cargo test --locked -p xtask --test coverage --test representative_candidate','approved candidate-executions and verify-ownership'],
 'scope_note':'The five failures are still failures in current state, caused by unchanged live old bindings pending the proposed authorized transaction. No claim that current whole workspace is green.'}
s.validate_attestation(attestation,s.parse_workspace(raw))
q.write(P/'final-checks/parent-split-verification.json',attestation)
q.put(P/'orchestration-successors/jpeg187-bind-parent-results.py',Path(__file__).read_bytes())
q.write(P/'final-checks/split-checker-successor.json',{'approval':None,'schema':'excalidraw-render/jpeg187-split-verification-successor/v1','frozen_checker':q.desc(P/'qualify.py'),'successor':q.desc(P/'split-verification.py'),'purpose':'Validate exact split-completion/timeout/expected-pending-failure evidence before invoking unchanged frozen seal/verify. No passing flag is assigned to timeout or five failing tests.','original_focused_qualification_record_unchanged':q.desc(P/'final-checks/production-qualification.json'),'pending_draft_subject_preserved':q.desc(P/'capture-draft-subject.json')})
s.verify_split()
print(json.dumps(attestation['initial']['verified_counts'],indent=2))
