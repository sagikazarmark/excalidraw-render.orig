import assert from "node:assert/strict";
import {readFileSync,writeFileSync,mkdtempSync} from "node:fs";
import {execFileSync,spawnSync} from "node:child_process";
import {createHash} from "node:crypto";
import {resolve,relative} from "node:path";
const ROOT=resolve(import.meta.dirname,"../../.."),dir=process.argv[2],mode=process.argv[3]??"collect";
assert.ok(dir&&["collect","verify"].includes(mode));const sha=b=>createHash("sha256").update(b).digest("hex"),read=p=>readFileSync(resolve(ROOT,p));
const temp=relative(ROOT,mkdtempSync(resolve(ROOT,"target/stage4-font-audit-"))),svg=read(`${dir}/native.svg`).toString();
const command=(input,output)=>["tools/reference-oracle/run-svg-text-audit.sh","render",input,output,`${dir}/native-assets.json`];
const good=JSON.parse(execFileSync("bash",command(`${dir}/native.svg`,`${temp}/good.rgba`),{cwd:ROOT,maxBuffer:1024*1024}).toString());assert.ok(good.platform_fonts.length>0);assert.ok(good.platform_fonts.every(f=>f.isCustomFont));
const negatives=[
 ["ambient-font",svg.replace(/font-family="[^"]+"/gu,'font-family="Arial"'),"ambient platform font used"],
 ["invalid-format",svg.replace(/(src\s*:\s*url\([^)]*\))(?:\s*format\([^)]*\))?/gu,"$1 format('truetype')"),"@font-face src must be one WOFF2"],
];
const rejected=[];for(const [name,source,needle]of negatives){const input=`${temp}/${name}.svg`;writeFileSync(resolve(ROOT,input),source);const r=spawnSync("bash",command(input,`${temp}/${name}.rgba`),{cwd:ROOT,encoding:"utf8",maxBuffer:2*1024*1024});assert.notEqual(r.status,0);assert.ok(`${r.stderr}${r.stdout}`.includes(needle),name);rejected.push({id:name,rejected:true,reason:needle});}
const paths=["tools/reference-oracle/scripts/svg-viewer.mjs","tools/reference-oracle/scripts/svg-viewer-text-audit.mjs","tools/reference-oracle/run-svg-text-audit.sh","tools/reference-oracle/scripts/stage4-font-audit-qualification.mjs","tests/conformance/viewer-profiles/canonical-svg-chromium-v1.json","tools/reference-oracle/package-lock.json",`${dir}/native.svg`,`${dir}/native-assets.json`];
const profile={schema:"excalidraw-render/candidate-svg-text-audit-profile/v1",id:"canonical-svg.chromium.text-audit.v1",base_profile:"canonical-svg.chromium.v1",reference_profile_sha256:"1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71",changes:["Audit DOM text localName; CDP DOM.Node has no namespaceURI field","Accept absent optional format hint only for a declared WOFF2 data URL","Record container namespace with browser PID"],sources:paths.map(path=>({path,sha256:sha(read(path))})),qualification:{text_present:true,all_used_fonts_custom:true,positive_raster_sha256:sha(read(`${temp}/good.rgba`)),negative_controls:rejected},approval:null};
const output=`${dir}/chromium-text-profile.json`,bytes=Buffer.from(JSON.stringify(profile,null,2)+"\n");if(mode==="verify")assert.deepEqual(read(output),bytes);else writeFileSync(resolve(ROOT,output),bytes,{flag:"wx"});console.log(`font-audit ${mode}: ${sha(bytes)}`);
