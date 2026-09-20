// Diagnostic reference qualification for the authorized native byte-canary
// change. Reuse the frozen official collector's browser/route/font protocol.
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {createHash} from "node:crypto";
import {resolve} from "node:path";
import {pathToFileURL} from "node:url";
const root=resolve(import.meta.dirname,"../../..");
let code=readFileSync(resolve(root,"tools/reference-oracle/scripts/stage4-representative-v2.mjs"),"utf8");
assert.equal(createHash("sha256").update(code).digest("hex"),"b4b1674102f5f181e037e4dda3dfc35f36dd6c847a86c5957ca788815af47b58");
function replace(before,after){assert.ok(code.includes(before),before);code=code.replace(before,after);}
replace('ROOT=resolve(import.meta.dirname,"../../..")',`ROOT=${JSON.stringify(root)}`);
replace('DIR="tests/conformance/candidates/stage4-frames-text-assets-v2"','DIR="target/stage4-excalifont-canary"');
replace('await import("playwright")',`await import(${JSON.stringify(pathToFileURL(resolve(root,"tools/reference-oracle/node_modules/playwright/index.mjs")).href)})`);
const source={type:"excalidraw",version:2,elements:[{id:"text",type:"text",x:20,y:30,width:90,height:72,angle:0.125,strokeColor:"#1e66f5",opacity:70,fontSize:20,fontFamily:5,text:"One two three\nFour",textAlign:"center",verticalAlign:"middle",lineHeight:1.2,frameId:null,updated:1}],appState:{},files:{}};
// The official oracle accepts complete export records. These explicit defaults
// must be verified against the native canary's sparse source (identical bytes).
Object.assign(source.elements[0],{backgroundColor:"transparent",fillStyle:"hachure",strokeWidth:1,strokeStyle:"solid",roughness:1,groupIds:[],index:"a0",seed:1,version:1,versionNonce:1,isDeleted:false,boundElements:null,link:null,locked:false,originalText:source.elements[0].text});
const declaration=code.match(/^const source=.*$/mu)?.[0];assert.ok(declaration);replace(declaration,`const source=${JSON.stringify(source)};`);
replace('exportBackground:false,exportEmbedScene:true,exportPadding:0','exportBackground:true,exportEmbedScene:false,exportPadding:10');
replace('const sources=[','const sources=["tools/reference-oracle/scripts/stage4-excalifont-canary.mjs",');
replace('Object.entries(outputs)','Object.entries(outputs).filter(([name])=>!(["raster.png","nested.svg"].includes(name)))');
try { await import(`data:text/javascript;base64,${Buffer.from(code).toString("base64")}`); }
catch(error){console.error(error.message);process.exitCode=1;}
