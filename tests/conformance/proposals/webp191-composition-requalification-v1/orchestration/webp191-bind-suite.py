from pathlib import Path
import subprocess
import types
R=Path('/home/laborant/excalidraw-render.orig');P=R/'tests/conformance/proposals/webp191-composition-requalification-v1'
s=types.ModuleType('suite');s.__file__=str(P/'suite_gate.py')
exec(compile((P/'suite_gate.py').read_bytes(),s.__file__,'exec'),s.__dict__)
q=s.frozen_workflow();q.check_freeze()
raw=Path('/home/laborant/.local/share/opencode/tool-output/tool_0c449b1de001sz9cnHVgDTvSaK').read_bytes()
classification=s.parser().classify(raw)
q.put(P/'checks/full-workspace.raw.log',raw)
message="""Continue finalseal userdoit. Parentfullsuiteactual complete /home/laborant/.local/share/opencode/tool-output/tool_0c449b1de001sz9cnHVgDTvSaK inspectONLYthislog read or scopedrg path. Toolgrepfilepathignored blewglobal search use includeexactbasename parentdir. Counttotalsandextractverbatimlog viaallowedfiletoolapplypatch or pythonarchivingnecessaryspecialtools can't hugecopy? bashcp allowedfallbackspecialno copytool. all626fastpass expectedEXACT5stale failures 2xtaskbin inventory_extension_preserves_live_owners_and_approved_exclusions,test_owner_checks_compiled_module_exact_selector_and_source_digest;2coverage ownership_recording_validates_entire_batch_before_writing,ownership_recording_rejects_unknown_obligations_without_changing_the_ledger;1representative_registry_is_consumed_through_approval_aware_native_execution. Don'tcallfullsuitegreen. Recordactualmanifest and seal production scopewebp191-composition candidate subject520b... draft1e412.. mayupdateonlypacketmetadata nofrozenproducer/tools edits. Need finalapproveJSONexactrequiredkeys readyrequest text. Isolated28mutation rerun appropriate suitefullrecordnew nofullworkspace. Returnfinalsubjects hashesexact,commandfinalverify, reviewfiles. NOapproval/liveapplication/commit;userdoitpermissioncontinue notyetexactsubjectapproval. Parentindependentreviewsealedpacketnext.
"""
q.put(P/'checks/parent-full-suite-message.txt',message.encode())
cmd=['cargo','run','--release','--locked','-p','xtask','--','conformance','record-test-owner','surface_metadata_contract','editor_state_is_silent_preserved_and_independent_of_scene_and_frame_exports','Explicit reviewed editor-only surface contract','surface:unknown:/unknown:svg']
before=(R/q.LEDGER).read_bytes();r=subprocess.run(cmd,cwd=R,env=q.ENV,capture_output=True)
q.require(r.returncode!=0 and not r.stdout and b'error: stale ownership evidence' in r.stderr,'diagnostic does not match stale binding cause')
q.require((R/q.LEDGER).read_bytes()==before,'diagnostic mutated live ledger')
q.put(P/'checks/unknown-owner-reproduction.stdout',r.stdout);q.put(P/'checks/unknown-owner-reproduction.stderr',r.stderr)
q.write(P/'checks/full-suite-record.json',{'schema':'excalidraw-render/webp191-completed-suite-record/v1','approval':None,
 'command':['cargo','test','--locked','--workspace','--no-fail-fast'],'completed_normally':True,'outcome':'failed-only-at-exact-pending-binding-tests',
 'environment':{'toolchain':'1.97.1-x86_64-unknown-linux-gnu','CARGO_INCREMENTAL':'0','CARGO_PROFILE_DEV_DEBUG':'0','CARGO_PROFILE_TEST_DEBUG':'0','CARGO_PROFILE_RELEASE_DEBUG':'0'},
 'raw_log':q.desc(P/'checks/full-workspace.raw.log'),'parent_message':q.desc(P/'checks/parent-full-suite-message.txt'),'classification':classification,
 'freeze':q.desc(P/'final-freeze.json'),'test_source':q.desc(R/q.SOURCE),'producer_sources':q.read(R/q.V13/'native-capture.json')['producer_sources'],
 'unknown-owner-cli-reproduction':{'command':cmd,'exit_code':r.returncode,'stdout':q.desc(P/'checks/unknown-owner-reproduction.stdout'),'stderr':q.desc(P/'checks/unknown-owner-reproduction.stderr'),'ledger_before_sha256':q.sha(before),'ledger_after_sha256':q.sha((R/q.LEDGER).read_bytes())},
 'post_approval_required':['cargo test --locked -p xtask --bin xtask --test coverage --test representative_candidate --no-fail-fast']})
q.put(P/'orchestration/webp191-bind-suite.py',Path(__file__).read_bytes())
q.write(P/'checks/suite-successor.json',{'approval':None,'scope':'strict authenticated prior classifier; numeric totals only, exact error identity/location/body and standard backtrace-note rules retained',
 'entry':q.desc(P/'suite_gate.py'),'parser':q.desc(R/s.PARSER),'parser_subject':s.PRIOR,
 'frozen_workflow':q.desc(P/'workflow.py'),'frozen_receipt_entry':q.desc(P/'receipt_gate.py'),
 'preserved_draft':q.desc(P/'capture-draft-subject.json'),'preserved_focused_record':q.desc(P/'checks/parent-qualification.json')})
s.verify_suite();q.check_freeze();print(classification)
