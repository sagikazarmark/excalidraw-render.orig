import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { stage4Fixtures, sceneFor } from "./png-stage4-fixtures.mjs";
const ORACLE=resolve(dirname(fileURLToPath(import.meta.url)),"..");const ROOT=resolve(ORACLE,"../..");
const INPUT="tests/oracle/png-color/inputs.json", PROFILE="tests/oracle/reference-profiles/revision-1.json";
const mode=process.argv[2];assert.ok(["generate","verify","extend"].includes(mode));assert.equal(process.env.REFERENCE_ORACLE_CANONICAL,"1");assert.equal(process.version,"v24.16.0");
const profile=JSON.parse(readFileSync(resolve(ROOT,PROFILE)));const rows=stage4Fixtures(JSON.parse(readFileSync(resolve(ROOT,INPUT))));
const options={exportBackground:false,exportEmbedScene:false,exportPadding:0,exportScale:1,exportWithDarkMode:false,frameRendering:{enabled:true,clip:true,name:true,outline:true},maxWidthOrHeight:null,viewBackgroundColor:"#ffffff"};
const sha256=data=>createHash("sha256").update(data).digest("hex");
async function collect(order){const browser=await chromium.launch({args:profile.browser.launch.explicitArguments,chromiumSandbox:false,headless:true});
 try{assert.equal(browser.version(),"149.0.7827.55");const outputs={};for(const row of order){const context=await browser.newContext(profile.browser.context);
  try{const page=await context.newPage();const errors=[];page.on("pageerror",e=>errors.push(e.message));page.on("console",m=>{if(m.type()==="error")errors.push(m.text());});
   await page.route("**/*",async route=>{const url=new URL(route.request().url());if(url.origin!=="http://reference.invalid"||(url.pathname!=="/"&&!url.pathname.startsWith("/bundle/"))){errors.push(`unadmitted ${url.href}`);await route.abort();return;}
    const path=resolve(ORACLE,"dist",url.pathname==="/"?"index.html":url.pathname.slice(8));assert.ok(path.startsWith(resolve(ORACLE,"dist")+sep));await route.fulfill({body:readFileSync(path),contentType:path.endsWith(".html")?"text/html":path.endsWith(".js")?"text/javascript":"application/octet-stream"});});
   await page.goto("http://reference.invalid/");await page.waitForFunction(()=>typeof window.referenceOracle?.render==="function");
   outputs[row.id]=await page.evaluate(async({source,opaque,options})=>{const image=new Image();image.src=source.files.asset.dataURL;try{await image.decode();}catch{return {admitted:false};}
    const first=await window.referenceOracle.render(source,options);const companion=await window.referenceOracle.render(opaque,options);return {admitted:true,png:first.pngBase64,opaque_png:companion.pngBase64,svg:first.svg};}, {source:sceneFor(row),opaque:sceneFor(row,true),options});
   assert.deepEqual(errors,[],row.id);
  }finally{await context.close();}}
  return outputs;
 }finally{await browser.close();}}
const first=await collect(rows),second=await collect([...rows].reverse());for(const r of rows)assert.deepEqual(first[r.id],second[r.id],r.id);
const sources=[INPUT,PROFILE,"tools/reference-oracle/scripts/png-color-fixtures.mjs","tools/reference-oracle/scripts/png-stage4-fixtures.mjs","tools/reference-oracle/scripts/png-stage4-evidence.mjs","tools/reference-oracle/run-png-stage4.sh","tests/conformance/comparator-profiles/canonical-visual-v1.json"].map(path=>({path,sha256:sha256(readFileSync(resolve(ROOT,path)))}));
const record={schema:"excalidraw-render/png-color-reference/v1",sources,options,visual_class:"geometry-image",collection:"Official v0.18.1; pinned Chromium149; two fresh processes in forward/reverse order; fresh context per case; network none",cases:rows.map(row=>{const {svg,...output}=first[row.id];return {...row,scene:sceneFor(row),reference:{...output,...(svg?{svg_sha256:sha256(Buffer.from(svg))}:{})}};})};
assert.equal(new Set(rows.map(r=>r.id)).size,rows.length);const bytes=Buffer.from(JSON.stringify(record,null,2)+"\n");const output=resolve(ROOT,"tests/oracle/png-color/stage4-reference.json");
if(mode==="verify")assert.deepEqual(bytes,readFileSync(output));
else if(mode==="extend") {
  const previous=readFileSync(output);assert.equal(sha256(previous),process.argv[3]);
  for(const old of JSON.parse(previous).cases){
    // Correct only the authored outcome of the two unaligned mAB LUT vectors;
    // source PNGs, scenes and independently collected reference bytes stay exact.
    if(["lut-mab-matrix-m-curves-c6-d8","lut-mab-matrix-m-curves-c6-d16"].includes(old.id))old.expected_outcome="unsupported-format";
    assert.deepEqual(record.cases.find(r=>r.id===old.id),old,`changed prior case ${old.id}`);
  }
  writeFileSync(output,bytes);
}else writeFileSync(output,bytes,{flag:"wx"});console.log(`${mode}: ${rows.length} PNG stage4 cases; sha256:${sha256(bytes)}`);
