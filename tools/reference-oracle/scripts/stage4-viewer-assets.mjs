// Materialize only resources actually embedded by each SVG, never a full font
// catalog. The Rust adapter decodes these exact WOFF2 bytes and records names.
import {readFileSync,writeFileSync,existsSync} from "node:fs";
import assert from "node:assert/strict";
import {createHash} from "node:crypto";
import {resolve} from "node:path";
const root=resolve(import.meta.dirname,"../../.."),dir=process.argv[2]??"tests/conformance/candidates/stage4-frames-text-assets";
const mode=process.argv[3]??"collect";assert.ok(["collect","verify"].includes(mode));
const hash=b=>createHash("sha256").update(b).digest("hex");
const write=(path,bytes)=>{const p=resolve(root,path);if(mode==="verify"||existsSync(p))assert.deepEqual(readFileSync(p),Buffer.from(bytes),`SVG-derived resource differs: ${path}`);else writeFileSync(p,bytes,{flag:"wx"});};
const plans=[];
for(const kind of ["official","native"]){
 const svg=readFileSync(resolve(root,dir,`${kind}.svg`),"utf8"),fonts=[],images=[];
 {
  // CSS Fonts defines format() as an optional capability hint. The pinned
  // viewer requires the hint; add only that hint for the exporter's single
  // embedded WOFF2 URL. All geometry, font bytes, names and text stay exact.
  const view=svg.replace(/(@font-face\s*\{[^}]*?src\s*:\s*url\(data:font\/woff2;base64,[A-Za-z0-9+/=]+\))(?=\s*[;}])/gu,"$1 format('woff2')");
  write(`${dir}/${kind}-view.svg`,view);
 }
 for(const [i,m]of [...svg.matchAll(/@font-face\s*\{([^}]*)\}/gu)].entries()){
  const css=m[1],field=n=>css.match(new RegExp(`${n}\\s*:\\s*([^;]+)`,"iu"))?.[1].trim();
  const url=css.match(/url\(([^)]+)\)/u)?.[1].replace(/^["']|["']$/gu,"");if(!url?.startsWith("data:font/woff2;base64,"))throw Error("undeclared font encoding");
  const bytes=Buffer.from(url.split(",")[1],"base64"),path=`${dir}/assets/${kind}-font-${i}.woff2`;write(path,bytes);
  const weight=field("font-weight")??"400";
  fonts.push({woff2:path,source:{media_type:"font/woff2",sha256:hash(bytes)},css:{family:field("font-family").replace(/^["']|["']$/gu,""),style:field("font-style")??"normal",weight:weight==="normal"?400:weight==="bold"?700:Number(weight),stretch:field("font-stretch")??"normal",unicode_range:(field("unicode-range")??"U+0-10FFFF").split(",").map(v=>v.trim())}});
 }
 for(const [i,m]of [...svg.matchAll(/<image\b[^>]*\b(?:href|xlink:href)="([^"]+)"[^>]*>/gu)].entries()){
  const [meta,...parts]=m[1].split(","),type=meta.slice(5).split(";")[0],bytes=meta.endsWith(";base64")?Buffer.from(parts.join(","),"base64"):Buffer.from(decodeURIComponent(parts.join(",")));
  const path=`${dir}/assets/${kind}-image-${i}.${type==="image/svg+xml"?"svg":"png"}`;write(path,bytes);images.push({source:{media_type:type,sha256:hash(bytes)},canonical:{path,media_type:type,sha256:hash(bytes)}});
 }
 plans.push({kind,fonts,images});
}
write(`${dir}/viewer-assets-plan.json`,JSON.stringify(plans,null,2)+"\n");
