// Read-only preservation and ownership audit for the stage based at 629930b.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { verifySixCaseApplication } from "./verify-six-case-application.mjs";
const ROOT=resolve(dirname(fileURLToPath(import.meta.url)),"../../..");
const BASE="629930b";
const application=verifySixCaseApplication(ROOT);
const actualRead=path=>readFileSync(resolve(ROOT,path));
const read=path=>application?.beforeFiles.get(path)??actualRead(path);
const json=path=>JSON.parse(read(path));
const sha=bytes=>createHash("sha256").update(bytes).digest("hex");
const canonical=value=>Array.isArray(value)?value.map(canonical):value!==null&&typeof value==="object"?Object.fromEntries(Object.keys(value).sort().map(key=>[key,canonical(value[key])])):value;
const git=(...args)=>execFileSync("git",args,{cwd:ROOT,maxBuffer:16*1024*1024});
const ledgerPath="tests/conformance/coverage/ownership.json";
const previous=JSON.parse(git("show",`${BASE}:${ledgerPath}`));
const current=json(ledgerPath);
// This audit is outside candidate producer bindings. Restore only the exact
// approved representative assignment when checking the earlier reconciliation.
const representativePath="tests/conformance/coverage/stage4-representative-owner-transition.json";
const representative=existsSync(resolve(ROOT,representativePath))?json(representativePath):null;
const priorRepresentative=structuredClone(current);
if(representative){
  assert.equal(representative.schema,"excalidraw-render/stage4-representative-owner-transition/v1");
  const expectedSubject="c314cff42bdaac7fae932c4e7e4c8a9dc278b69a7077d9e4de37bc5cddf2fa05";
  assert.equal(representative.approval_subject_sha256,`sha256:${expectedSubject}`);
  for(const key of ["after_ledger","approved_manifest","registry","comment_snapshot"]){const d=representative[key],bytes=read(d.path);assert.deepEqual(d,{path:d.path,byte_length:bytes.length,sha256:sha(bytes)});}
  const manifest=json(representative.approved_manifest.path),bare=structuredClone(manifest);delete bare.approval;
  const bundle={schema:"excalidraw-render/conformance-approval-subject/v1",manifest:bare,files:manifest.files};
  assert.equal(sha(JSON.stringify(canonical(bundle))),expectedSubject);
  assert.deepEqual(manifest.approval,representative.approval);
  assert.equal(representative.comment_snapshot.sha256,"2b1313a6f98a5728825810926e801b4634e5555a17cf3e84d3b6959c3d9db16c");
  const decision=json(representative.comment_snapshot.path);
  assert.deepEqual(manifest.approval,{approved_by:"sagikazarmark",approved_at:"2026-09-20",approval_url:"https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5749841192",approval_subject_sha256:`sha256:${expectedSubject}`});
  assert.equal(decision.user.login,manifest.approval.approved_by);assert.equal(decision.created_at.slice(0,10),manifest.approval.approved_at);assert.equal(decision.html_url,manifest.approval.approval_url);assert.ok(decision.body.includes(expectedSubject));
  const index=priorRepresentative.assignments.findIndex(r=>r.id==="interaction:frames-text-assets");
  assert.deepEqual(priorRepresentative.assignments[index],representative.after_assignment);
  assert.equal(representative.before_assignment.id,"interaction:frames-text-assets");assert.equal(representative.before_assignment.disposition.status,"gap");
  const binding=representative.after_assignment.disposition.owner.binding;
  assert.equal(representative.after_assignment.disposition.status,"owned");assert.equal(representative.after_assignment.disposition.owner.kind,"representative");
  assert.deepEqual(binding.manifest,{path:representative.approved_manifest.path,sha256:representative.approved_manifest.sha256});
  assert.deepEqual(binding.registry,{path:representative.registry.path,sha256:representative.registry.sha256});assert.equal(binding.approval_subject_sha256,`sha256:${expectedSubject}`);
  const registered=json(representative.registry.path).cases[0];assert.equal(binding.case,manifest.case_id);assert.equal(registered.id,manifest.case_id);assert.deepEqual(registered.exceptions,manifest.exception_proposal.exceptions);assert.deepEqual(binding.command,registered.consumer);
  const receipts=representative.consumer_receipts;assert.deepEqual(representative.registry_receipts.cases,[receipts]);assert.equal(receipts.approval_required,true);assert.equal(receipts.approval_subject_sha256,expectedSubject);assert.equal(receipts.executions.length,3);
  for(const [i,format] of ["svg","png","pdf"].entries()){const row=receipts.executions[i];assert.equal(row.execution_key,`${manifest.case_id}:default:${format}`);assert.equal(row.native.format,format);assert.equal(row.native.fresh_process_runs,2);assert.equal(row.approval_subject_sha256,expectedSubject);}
  priorRepresentative.assignments[index]=representative.before_assignment;
  assert.equal(sha(JSON.stringify(priorRepresentative,null,2)+"\n"),representative.before_ledger_sha256);
}
const reconciliation=json("tests/conformance/coverage/stage4-binding-reconciliation.json");
const followupPath="tests/conformance/coverage/stage4-image-icc-binding-reconciliation.json";
const followup=existsSync(resolve(ROOT,followupPath))?json(followupPath):null;
if(followup){assert.deepEqual(followup.before_files,reconciliation.after_files);assert.equal(followup.previous_reconciliation.sha256,sha(read(followup.previous_reconciliation.path)));assert.equal(followup.old_catalog_sha256,reconciliation.new_catalog_sha256);assert.equal(followup.owner_source_changes.length,236);}
assert.equal(reconciliation.old_catalog_sha256,previous.catalog_sha256);
assert.equal(current.catalog_sha256,(followup??reconciliation).new_catalog_sha256);
for(const d of [...(followup??reconciliation).after_files,reconciliation.spec,reconciliation.assessment,reconciliation.canary_qualification])assert.equal(sha(representative&&d.path===ledgerPath?JSON.stringify(priorRepresentative,null,2)+"\n":read(d.path)),d.sha256,d.path);
assert.equal(reconciliation.spec_authority_ids.length,1007);
assert.equal(reconciliation.inventory_locator_authority_ids.length,118);
assert.equal(reconciliation.owner_source_changes.length,101);
assert.equal(current.approval,null);
assert.equal(current.assignments.length,1876);
assert.equal(new Set(current.assignments.map(r=>r.id)).size,1876);
const owners=new Map();
const add=(module,fn,ids,binary="fast")=>{for(const id of ids){assert.ok(!owners.has(id));owners.set(id,{module,fn,binary});}};
const png=(fn,ids)=>add("png_stage4_contract",fn,ids);
const exif=[];for(let o=1;o<=8;o++)for(const order of ["big","little"])for(const position of ["before-idat","after-idat"])exif.push(`codec:png:exif:o${o}:${order}:${position}`);
png("all_orientation_cells_preserve_source_and_permute_pixels",[...exif,"codec:png:orientation"]);
png("invalid_orientation_is_ignored_but_duplicate_chunks_fail",["codec:png:orientation:invalid"]);
png("palettes_and_transparency_chunks_have_strict_sample_contracts",["codec:png:palette:suggested-truecolor","codec:png:malformed:palette-trns"]);
png("gray_icc_profiles_transform_expanded_samples",["codec:png:icc:gray"]);
png("lut_profiles_select_tables_and_recover_unsupported_classes",["codec:png:icc:lut"]);
png("incompatible_icc_models_preserve_svg_and_recover_projections",["codec:png:icc:unsupported-models"]);
png("malformed_iccp_has_explicit_ignore_or_recovery_contracts",["codec:png:icc:malformed"]);
png("reserved_cicp_fields_preserve_svg_and_recover_projections",["codec:png:cicp:invalid-fields"]);
png("apng_one_frame_subframes_blend_and_disposal_are_qualified",["codec:png:apng:one-frame","codec:png:apng:subframe","codec:png:apng:blend-disposal","codec:png:animation"]);
png("apng_sequence_counts_and_later_streams_fail_before_passthrough",["codec:png:apng:sequence-counts","codec:png:apng:late-data"]);
png("malformed_png_headers_chunks_and_streams_have_complete_recovery",["codec:png:malformed:ihdr","codec:png:malformed"]);
png("malformed_chunks_fail_and_unknown_ancillary_survives",["codec:png:malformed:chunks"]);
png("scanline_streams_and_adam7_extents_are_fully_validated",["codec:png:malformed:scanlines"]);
png("profile_validation_and_decode_limits_have_exact_boundaries",["codec:png:limits:validation"]);
png("sample_color_orientation_animation_axes_compose",["codec:png:default-frame","codec:png:passthrough"]);
png("complete_profile_composition_matches_independent_reference",["codec:png:composition:qualified-axes"]);
add("png_projection_contract","profile_workspace_masks_sharing_and_recovery_have_exact_boundaries",["codec:png:limits:projection"],"lib");
add("stage4_interaction_contract","nested_svg_pdf_dark_mode_retains_the_specified_154_limitation",["interaction:dark-mode-nested-svg-pdf"]);
add("stage4_interaction_contract","lazy_frames_reset_limits_replay_assets_and_keep_errors_atomic",["interaction:frame-batch-limits"]);
add("stage4_diagnostic_contract","mixed_normalization_render_resource_metadata_preserve_source_order",["interaction:diagnostic-order"],"lib");
assert.equal(owners.size,59);
const original=new Map(previous.assignments.map(r=>[r.id,r]));
let added=0;
for(const row of priorRepresentative.assignments){
  const old=original.get(row.id);assert.ok(old,row.id);
  const selected=owners.get(row.id);
  if(!selected){const normalized=structuredClone(row);for(const record of [followup,reconciliation].filter(Boolean)){const change=record.owner_source_changes.find(c=>c.obligation===row.id);if(change){assert.deepEqual(normalized.disposition.owner.source,change.after);normalized.disposition.owner.source=change.before;}}assert.deepEqual(normalized,old,`changed old disposition ${row.id}`);continue;}
  assert.equal(old.disposition.status,"gap",row.id);
  assert.equal(row.disposition.status,"owned",row.id);
  const owner=row.disposition.owner;
  assert.equal(owner.kind,"test");assert.equal(owner.function,selected.fn);assert.equal(owner.binary,selected.binary);
  const path=`crates/excalidraw-render/${selected.binary==="lib"?"src":"tests/fast"}/${selected.module}.rs`;
  assert.deepEqual(owner.source,{path,sha256:sha(read(path))});
  assert.deepEqual(owner.command,["cargo","test","--locked","--release","-p","excalidraw-render",...(selected.binary==="lib"?["--lib"]:["--test","fast"]),`${selected.module}::${selected.fn}`,"--","--exact"]);
  assert.ok(/docs\/research\/png-stage4-(ownership|review)\.md/.test(row.disposition.rationale));added++;
}
assert.equal(added,59);
const counts={};for(const r of current.assignments)counts[r.disposition.status]=(counts[r.disposition.status]??0)+1;
assert.deepEqual(counts,representative?{owned:1800,gap:73,excluded:3}:{owned:1799,gap:74,excluded:3});
if(representative){added++;assert.deepEqual(representative.counts,{assignments:1876,owned:counts.owned,excluded:counts.excluded,assigned_or_excluded:1803,gaps:counts.gap,new_stage4_owners:added});}
// Authenticate the exact approved protected-PDF transition before allowing its
// expectation file to differ from BASE. The archived GitHub response was fetched
// when recording the user's decision; this preservation audit is offline.
const pdfCase="tests/conformance/candidates/stage4-pdf-backend-requalification-v2";
const pdfExpectation="tests/conformance/pdf-backend-qualification/project-operations-structure.json";
const pdfTest="crates/excalidraw-render/tests/fast/pdf_backend_qualification.rs";
const pdfSubject="c15e6bab141ac8080bac099932e5561bfcbee4c000f4957fec71e87d5e573389";
const subject=json(`${pdfCase}/approval-subject.json`);
assert.equal(sha(JSON.stringify(canonical(subject))),pdfSubject);
const transition=json(`${pdfCase}/applied-transition.json`);
assert.equal(transition.schema,"excalidraw-render/approved-pdf-pin-transition/v1");
assert.equal(transition.approval_subject_sha256,`sha256:${pdfSubject}`);
assert.equal(transition.representative_baseline_and_global_approval_granted,false);
for(const [key,path] of Object.entries({
  before_expectation:`${pdfCase}/before-project-operations-structure.json`,
  before_test:`${pdfCase}/before-pdf_backend_qualification.rs`,
  before_verifier:`${pdfCase}/before-conformance_pdf_requalification.rs`,
  after_expectation:pdfExpectation,
  after_test:pdfTest,
  current_verifier:"xtask/src/conformance_pdf_requalification.rs",
  comment_snapshot:`${pdfCase}/user-approval-comment.json`,
})){
  const bytes=read(path);
  assert.deepEqual(transition[key],{path,byte_length:bytes.length,sha256:sha(bytes)},key);
}
assert.equal(transition.comment_snapshot.sha256,"b8d5863ffba66ef9a5ff849e9552562653523b1220602df62e7bac1e2cab2a98");
const comment=json(transition.comment_snapshot.path);
const approval={approved_by:"sagikazarmark",approved_at:"2026-09-20",approval_url:"https://github.com/sagikazarmark/excalidraw-render.orig/issues/85#issuecomment-5748855374",approval_subject_sha256:`sha256:${pdfSubject}`};
assert.deepEqual(json(`${pdfCase}/approval.json`),approval);
assert.deepEqual(transition.approval,approval);
assert.equal(comment.user.login,approval.approved_by);
assert.equal(comment.created_at.slice(0,10),approval.approved_at);
assert.equal(comment.html_url,approval.approval_url);
assert.ok(comment.body.includes(pdfSubject));
assert.ok(comment.body.includes("not an autonomous approval decision by the coding agent"));
assert.deepEqual(subject.protected_expectation,{...transition.before_expectation,path:pdfExpectation});
assert.deepEqual(subject.harness_sources.find(d=>d.path===transition.current_verifier.path),{...transition.before_verifier,path:transition.current_verifier.path});
assert.deepEqual(read(transition.before_expectation.path),git("show",`${BASE}:${pdfExpectation}`));
assert.deepEqual(read(transition.before_test.path),git("show",`${BASE}:${pdfTest}`));
const expectedStructure=json(transition.before_expectation.path);
expectedStructure.operation_sha256="ebf933b7a43e6d53730578e44562484e2aa04eb4cc0d47af7c876625e054de16";
expectedStructure.inventory_sha256="a993befecf6da05bbf204d240bc3242b045a9eefd7460348811e1dfe3e83159b";
assert.deepEqual(json(pdfExpectation),expectedStructure);
assert.deepEqual(read(pdfExpectation),read(`${pdfCase}/proposed-structure.json`));
assert.equal(transition.after_expectation.sha256,"442f0cb2580a4066d64bcb924eaa6e070252faca2fa2b37fa4f34c81ff600b1b");
assert.equal(read(pdfTest).toString(),read(transition.before_test.path).toString()
  .replaceAll("b053159dc6ea0649e6c0fa5840196ccb81db6b41b7eb69d0181392851017ee43","50180ecf1822d7d573ea78cc6375cd3f83af8ef2748422b7e9b3d52a1b7ba6dc")
  .replaceAll(transition.before_expectation.sha256,transition.after_expectation.sha256));
console.log(`Protected PDF transition verified: sha256:${pdfSubject}; ${approval.approval_url}`);
// Existing records, criteria, approvals, execution keys and all three exclusions
// survive, apart from the specifically verified binding and PDF transitions.
const priorPaths=git("ls-tree","-r","--name-only",BASE,"--","tests/oracle","tests/conformance","docs/renderer-spec-v1.md").toString().trim().split("\n").filter(p=>!application?.beforeFiles.has(p)&&![ledgerPath,"tests/conformance/coverage/OWNERSHIP.md","tests/conformance/coverage/requirements.json","tests/conformance/coverage/inventory.json","docs/renderer-spec-v1.md",pdfExpectation].includes(p));
git("diff","--exit-code",BASE,"--",...priorPaths);
const oldInventory=JSON.parse(git("show",`${BASE}:tests/conformance/coverage/inventory.json`)),inventory=json("tests/conformance/coverage/inventory.json");
assert.deepEqual(inventory.executions,oldInventory.executions);assert.deepEqual(inventory.coverage,oldInventory.coverage);assert.equal(inventory.executions.length,168);
const records=[
  ["tests/oracle/png-color/reference.json",984,"2aae93308b4289d3374ec92e960e756106af621f9bfc64e4b29314f5162857d0"],
  ["tests/oracle/png-color/gain-map-reference.json",108,"dee2a45432ce2a4e211d1e51e8e80f571bfd8f90221c7bf85153b57df88714ad"],
  ["tests/oracle/png-color/stage4-reference.json",186,"42111900327cd1e0d70cac36763dc7296e14b94c716458f5e54cdd3f68310fd1"],
  ["tests/oracle/png-color/stage4-composition-reference.json",1968,"def2563be8d963fe188792c603720b50b02855adde0b1ab957366e7fd5466445"],
  ["tests/oracle/png-color/stage4-qualified-reference.json",1968,"fb97e4b298deebca1800f8919b14ce223099e86eb41df64a3ad0148b43167bc0"],
  ["tests/oracle/png-color/stage4-oracle-reference.json",1968,"9de893992bef08f966c0bb149f0cd61f3c20d6f3c2e701ded5706b39c60953f3"],
];
for(const [path,count,digest] of records){const bytes=read(path),record=JSON.parse(bytes);assert.equal(sha(bytes),digest);assert.equal(record.cases.length,count);assert.equal(new Set(record.cases.map(r=>r.id)).size,count);for(const s of record.sources)assert.equal(sha(read(s.path)),s.sha256,s.path);console.log(`${path}: ${count} cases sha256:${digest}`);}
const helpers=["tests/common/png_icc_safety.rs","tests/common/png_stage4.rs","tests/common/png_stage4_reference.rs","tests/common/png_stage4_composition.rs","tests/common/png_stage4_review.rs","tests/common/png_color_reference.rs","tests/fast/png_profile_contract.rs","tests/common/codec_fixtures.rs","tests/common/pdf_inspect.rs","tests/common/raster.rs"];
const consumer=read("crates/excalidraw-render/tests/fast/png_stage4_contract.rs").toString();
for(const p of helpers){const path=`crates/excalidraw-render/${p}`;const hash=sha(read(path));assert.ok(consumer.includes(hash),`unbound helper ${p}`);console.log(`${path} sha256:${hash}`);}
assert.ok(read("crates/excalidraw-render/src/stage4_diagnostic_contract.rs").toString().includes(sha(read("crates/excalidraw-render/src/render/metadata/test_faults.rs"))),"unbound metadata fault helper");
const projection=read("crates/excalidraw-render/src/png_projection_contract.rs").toString();
for(const relative of ["tests/common/png_stage4.rs","tests/common/png_stage4_reference.rs","tests/common/png_color_reference.rs","tests/common/png_stage4_review.rs","tests/common/codec_fixtures.rs","src/cms_failure_contract.rs","src/render/test_support.rs"])assert.ok(projection.includes(sha(read(`crates/excalidraw-render/${relative}`))),`unbound projection helper ${relative}`);
assert.ok(!read("Cargo.toml").toString().split("\n").find(s=>s.startsWith("moxcms =")).includes('"lut"'),"infallible upstream LUT executor re-enabled");
// Retired v1/dev v6 subjects are historical observations, not eligibility.
const candidate=json("tests/conformance/candidates/registry.json").cases[0];assert.equal(candidate.approval_required,true);
assert.equal(current.assignments.find(r=>r.id==="interaction:frames-text-assets").disposition.status,representative?"owned":"gap");
console.log(representative?`Approved representative owner recorded: ${candidate.id}; three release execution receipts verified.`:`Representative remains unassigned: ${candidate.id}; release successor and real approval required.`);
for(const path of ["Cargo.toml","crates/excalidraw-render/src/lib.rs","crates/excalidraw-render/src/render/asset.rs","crates/excalidraw-render/src/render/asset/color.rs","crates/excalidraw-render/src/render/asset/icc_lut.rs","crates/excalidraw-render/src/render/asset/png.rs","crates/excalidraw-render/src/stage4_diagnostic_contract.rs","crates/excalidraw-render/src/png_projection_contract.rs","crates/excalidraw-render/tests/fast/png_stage4_contract.rs","crates/excalidraw-render/tests/fast/stage4_interaction_contract.rs",ledgerPath])console.log(`${path} sha256:${sha(actualRead(path))}`);
const finalCounts=application?.counts??{owned:counts.owned,excluded:counts.excluded,gaps:counts.gap};
console.log(`Stage4: ${added} new owners; ${finalCounts.owned} owners + ${finalCounts.excluded} exclusions; ${finalCounts.gaps} gaps. Reviewed locator/source/spec bindings reconciled. ${application?"Approved six-case migration retires two unowned exceptions and rebinds exactly 18 inputs; all 168 semantic identities and 150 other full execution keys preserved.":"All 168 execution keys preserved."} Scoped protected PDF approval applied; representative approval ${representative?"recorded":"pending"}. Baseline/global approval and the fixed-size CMS allocation/resource gap remain open.`);
const changed=new Set([...git("diff","--name-only",BASE).toString().trim().split("\n"),...git("ls-files","--others","--exclude-standard").toString().trim().split("\n")].filter(Boolean));
console.log(`Changed files (${changed.size}):\n${[...changed].sort().join("\n")}`);
