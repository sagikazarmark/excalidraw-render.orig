// Explicit project-authored default-font variant; same pinned collection, same
// image/clip/link/metadata requirements and unchanged visual budgets.
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {createHash} from "node:crypto";
const url=new URL("./stage4-representative-v2.mjs",import.meta.url),bytes=readFileSync(url);
assert.equal(createHash("sha256").update(bytes).digest("hex"),"b4b1674102f5f181e037e4dda3dfc35f36dd6c847a86c5957ca788815af47b58");
let code=bytes.toString();const replace=(a,b)=>{assert.equal(code.split(a).length,2);code=code.replace(a,b);};
replace('DIR="tests/conformance/candidates/stage4-frames-text-assets-v2"','DIR="tests/conformance/candidates/stage4-frames-text-assets-v4"');
replace('source.elements[1].height=25;','source.elements[1].height=25;source.elements[1].fontFamily=5;source.elements[1].angle=0.125;source.elements[1].y=20;');
replace('"tools/reference-oracle/run-stage4-representative-v2.sh",','"tools/reference-oracle/run-stage4-representative-v2.sh","tools/reference-oracle/scripts/stage4-representative-v4.mjs","tools/reference-oracle/run-stage4-representative-v4.sh",');
replace('import.meta.dirname',JSON.stringify(import.meta.dirname));replace('import("playwright")',`import(${JSON.stringify(import.meta.resolve("playwright"))})`);
await import(`data:text/javascript;base64,${Buffer.from(code).toString("base64")}`);
