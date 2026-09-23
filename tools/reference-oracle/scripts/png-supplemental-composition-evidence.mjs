// Supplemental SDR composition collection (#85 codec:png:alpha/color-profile),
// using the frozen oracle method: an authored selected-frame static reference.
import assert from "node:assert/strict";
import {createHash} from "node:crypto";
import {readFileSync,writeFileSync} from "node:fs";
import {dirname,resolve,sep} from "node:path";
import {fileURLToPath} from "node:url";
import {chromium} from "playwright";
import {supplementalCompositionFixtures,withStaticReference,sceneFor} from "./png-supplemental-composition-fixtures.mjs";
const ORACLE=resolve(dirname(fileURLToPath(import.meta.url)),".."),ROOT=resolve(ORACLE,"../..");
const mode=process.argv[2];assert.ok(["generate","verify"].includes(mode));assert.equal(process.env.REFERENCE_ORACLE_CANONICAL,"1");assert.equal(process.version,"v24.16.0");
const INPUT="tests/oracle/png-color/inputs.json",PROFILE="tests/oracle/reference-profiles/revision-1.json";
const profile=JSON.parse(readFileSync(resolve(ROOT,PROFILE))),input=JSON.parse(readFileSync(resolve(ROOT,INPUT))),rows=supplementalCompositionFixtures(input).map(row=>withStaticReference(input,row));
const options={exportBackground:false,exportEmbedScene:false,exportPadding:0,exportScale:1,exportWithDarkMode:false,frameRendering:{enabled:true,clip:true,name:true,outline:true},maxWidthOrHeight:null,viewBackgroundColor:"#ffffff"};
const sha=b=>createHash("sha256").update(b).digest("hex");
async function collect(order){const browser=await chromium.launch({args:profile.browser.launch.explicitArguments,chromiumSandbox:false,headless:true});
 try{assert.equal(browser.version(),"149.0.7827.55");const outputs={};for(const row of order){const context=await browser.newContext(profile.browser.context);
  try{const page=await context.newPage(),errors=[];page.on("pageerror",e=>errors.push(e.message));page.on("console",m=>{if(m.type()==="error")errors.push(m.text());});
   await page.route("**/*",async route=>{const url=new URL(route.request().url());if(url.origin!=="http://reference.invalid"||(url.pathname!=="/"&&!url.pathname.startsWith("/bundle/"))){errors.push(url.href);await route.abort();return;}
    const path=resolve(ORACLE,"dist",url.pathname==="/"?"index.html":url.pathname.slice(8));assert.ok(path.startsWith(resolve(ORACLE,"dist")+sep));await route.fulfill({body:readFileSync(path),contentType:path.endsWith(".html")?"text/html":path.endsWith(".js")?"text/javascript":"application/octet-stream"});});
   await page.goto("http://reference.invalid/");await page.waitForFunction(()=>typeof window.referenceOracle?.render==="function");
   const ref={...row,png:row.static_reference_png,opaque_png:row.static_reference_opaque_png};
   outputs[row.id]=await page.evaluate(async({source,opaque,options})=>{const a=await window.referenceOracle.render(source,options),b=await window.referenceOracle.render(opaque,options);return {admitted:true,png:a.pngBase64,opaque_png:b.pngBase64,svg:a.svg};},{source:sceneFor(ref),opaque:sceneFor(ref,true),options});assert.deepEqual(errors,[],row.id);
  }finally{await context.close();}}
  return outputs;
 }finally{await browser.close();}}
const first=await collect(rows),second=await collect([...rows].reverse());for(const r of rows)assert.deepEqual(first[r.id],second[r.id],r.id);
const paths=[INPUT,PROFILE,"tools/reference-oracle/scripts/png-color-fixtures.mjs","tools/reference-oracle/scripts/png-stage4-fixtures.mjs","tools/reference-oracle/scripts/png-stage4-composition-fixtures.mjs","tools/reference-oracle/scripts/png-stage4-oracle-fixtures.mjs","tools/reference-oracle/scripts/png-supplemental-composition-fixtures.mjs","tools/reference-oracle/scripts/png-supplemental-composition-evidence.mjs","tools/reference-oracle/run-png-supplemental-composition.sh","tests/conformance/comparator-profiles/canonical-visual-v1.json"];
const record={schema:"excalidraw-render/png-color-reference/v1",sources:paths.map(path=>({path,sha256:sha(readFileSync(resolve(ROOT,path)))})),options,visual_class:"geometry-image",collection:"Official v0.18.1; Chromium149; two fresh processes, forward/reverse, fresh contexts, network none; independently authored selected-frame static color oracle",cases:rows.map(row=>{const {svg,...output}=first[row.id];return {...row,scene:sceneFor(row),reference:{...output,svg_sha256:sha(Buffer.from(svg))}};})};
assert.equal(new Set(rows.map(r=>r.id)).size,rows.length);const bytes=Buffer.from(JSON.stringify(record,null,2)+"\n"),output=resolve(ROOT,"tests/oracle/png-color/supplemental-composition-reference.json");
if(mode==="verify")assert.deepEqual(bytes,readFileSync(output));else writeFileSync(output,bytes,{flag:"wx"});console.log(`${mode}: ${rows.length} supplemental selected-frame oracles; sha256:${sha(bytes)}`);
