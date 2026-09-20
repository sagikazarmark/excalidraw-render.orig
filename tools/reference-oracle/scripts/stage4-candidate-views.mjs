import assert from "node:assert/strict";
import {execFileSync} from "node:child_process";
import {readFileSync,writeFileSync,mkdtempSync} from "node:fs";
import {createHash} from "node:crypto";
import {resolve,relative} from "node:path";
const ROOT=resolve(import.meta.dirname,"../../.."),DIR=process.argv[3]??"tests/conformance/candidates/stage4-frames-text-assets",mode=process.argv[2];
assert.ok(["collect","verify","probe"].includes(mode));const sha=b=>createHash("sha256").update(b).digest("hex"),read=p=>readFileSync(resolve(ROOT,p));
const descriptor=path=>({path,byte_length:read(path).length,sha256:sha(read(path))});
const temporary=mode!=="collect"?relative(ROOT,mkdtempSync(resolve(ROOT,`target/stage4-candidate-${mode}-`))):`${DIR}/viewer-evidence`;
const specs=[
 ["official-chromium","official.svg","canonical-svg-chromium-v1",["bash","tools/reference-oracle/run-svg-text-audit.sh","render",`${DIR}/official.svg`,null,`${DIR}/official-assets.json`]],
 ["native-chromium","native.svg","canonical-svg-chromium-v1",["bash","tools/reference-oracle/run-svg-text-audit.sh","render",`${DIR}/native.svg`,null,`${DIR}/native-assets.json`]],
 ["native-pdfium","native.pdf","canonical-pdf-pdfium-v1",["bash","tools/pdf-harness/run-pdfium.sh","render",`${DIR}/native.pdf`,null,"200","120"]],
 ["native-pdfjs","native.pdf","canonical-pdf-pdfjs-v1",["bash","tools/pdf-harness/run-pdfjs.sh","render",`${DIR}/native.pdf`,null,"200","120"]],
 ...["official","native"].map(k=>[`${k}-rust`,`${k}-rust.svg`,"canonical-svg-pure-rust-v1",["cargo","run","--locked","--manifest-path","tools/svg-viewer-rust/Cargo.toml","--","render",`${DIR}/${k}-rust.svg`,null,`${DIR}/${k}-assets.json`]]),
];
const sourcePaths=["tools/reference-oracle/scripts/stage4-candidate-views.mjs","tools/reference-oracle/scripts/stage4-viewer-assets.mjs","xtask/src/conformance_candidate.rs","Cargo.toml","Cargo.lock","xtask/Cargo.toml","tools/reference-oracle/package-lock.json","tools/reference-oracle/scripts/svg-viewer.mjs","tools/reference-oracle/scripts/svg-viewer-text-audit.mjs","tools/reference-oracle/run-svg-text-audit.sh","tools/svg-viewer-rust/src/main.rs","tools/svg-viewer-rust/Cargo.toml","tools/svg-viewer-rust/Cargo.lock","tools/pdf-harness/run-pdfium.sh","tools/pdf-harness/pdfium/render.py","tools/pdf-harness/pdfium.lock.json","tools/pdf-harness/run-pdfjs.sh","tools/pdf-harness/pdfjs/render.mjs","tools/pdf-harness/pdfjs/package-lock.json"];
const bindings=()=>({sources:sourcePaths.map(descriptor),profiles:[...new Set(specs.map(s=>s[2]==="canonical-svg-chromium-v1"?`${DIR}/chromium-text-profile.json`:`tests/conformance/viewer-profiles/${s[2]}.json`)),"tests/conformance/comparator-profiles/canonical-visual-v1.json"].map(descriptor),assets:[descriptor(`${DIR}/official-assets.json`),descriptor(`${DIR}/native-assets.json`)],inputs:specs.map(s=>descriptor(`${DIR}/${s[1]}`))});
const before=bindings();
const captures=[];
for(const [id,input,profile,command]of specs){const runs=[];let first;
 for(let run=0;run<2;run++){
  const output=`${temporary}/${id}${run?".repeat":""}.rgba`,args=command.map(a=>a===null?output:a);
  const stdout=execFileSync(args[0],args.slice(1),{cwd:ROOT,maxBuffer:16*1024*1024,timeout:180000,env:{...process.env,CARGO_INCREMENTAL:"0",CARGO_PROFILE_DEV_DEBUG:"0"}}).toString();const summary=JSON.parse(stdout.trim());const bytes=read(output);assert.equal(bytes.length,200*120*4);
  assert.equal(summary.width,200);assert.equal(summary.height,120);
  if(id.includes("chromium")){assert.equal(summary.browser_version,"149.0.7827.55");assert.equal(summary.lifecycle_updates,2);assert.equal(summary.animation_offset_microseconds,0);assert.ok(summary.platform_fonts.length>0);assert.ok(summary.platform_fonts.every(f=>f.isCustomFont));}
  if(run){assert.deepEqual(bytes,first);if(id.includes("chromium"))assert.notEqual(runs[0].container_namespace,summary.container_namespace);}else first=bytes;runs.push(summary);
 }
 const path=mode==="probe"?`${temporary}/${id}.rgba`:`${DIR}/viewer-evidence/${id}.rgba`;
 if(mode==="verify")assert.deepEqual(first,read(path),id);
 captures.push({id,input:{...descriptor(`${DIR}/${input}`)},viewer_profile:profile==="canonical-svg-chromium-v1"?`${DIR}/chromium-text-profile.json`:`tests/conformance/viewer-profiles/${profile}.json`,raster:mode!=="verify"?descriptor(path):{path,byte_length:first.length,sha256:sha(first)},runs});
}
assert.deepEqual(bindings(),before,"viewer producers, profiles, assets or inputs changed during collection");
const record={schema:"excalidraw-render/candidate-viewer-collection/v1",candidate_directory:DIR,profiles:before.profiles,sources:before.sources,assets:before.assets,captures};
const file=mode==="probe"?`${temporary}/collection.json`:`${DIR}/viewer-evidence/collection.json`;
if(mode!=="verify")writeFileSync(resolve(ROOT,file),JSON.stringify(record,null,2)+"\n",{flag:"wx"});
else{const old=JSON.parse(read(file));for(let i=0;i<captures.length;i++){assert.deepEqual(captures[i].raster,old.captures[i].raster);assert.deepEqual(captures[i].input,old.captures[i].input);}assert.deepEqual(record.profiles,old.profiles);assert.deepEqual(record.assets,old.assets);assert.deepEqual(record.sources,old.sources);}
console.log(`${mode}: six viewer pairs, two fresh processes each; all raster bytes repeat; ${file}`);
