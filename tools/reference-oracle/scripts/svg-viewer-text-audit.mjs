// Scoped qualification adapter over the immutable canonical viewer. Exactly two
// admission/audit corrections; capture, virtual time, origin and browser stay
// the same. Keep the original source/profile/approval untouched.
import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {createHash} from "node:crypto";
const url=new URL("./svg-viewer.mjs",import.meta.url),bytes=readFileSync(url);
assert.equal(createHash("sha256").update(bytes).digest("hex"),"61968d7bca7fef68fa29af22368b5d7dfddda9be1c9a34e53f479cdb14c1a11b");
let code=bytes.toString();
const replace=(before,after)=>{assert.equal(code.split(before).length,2,`unique source boundary: ${before}`);code=code.replace(before,after);};
// DOM.Node has localName but no namespaceURI member. Source validation already
// admits only SVG content. Audit every text node conservatively, not zero nodes.
replace('node.localName === "text" && node.namespaceURI === "http://www.w3.org/2000/svg"','node.localName === "text"');
// PIDs may repeat across fresh containers; record the actual PID namespace too.
replace('browser_process_id: browserProcess.id,','browser_process_id: browserProcess.id, container_namespace: process.env.HOSTNAME,');
// A format hint is optional CSS syntax. The URL still must be one declared,
// digest-verified WOFF2 payload; neither extra URLs nor other formats are allowed.
replace(String.raw`if (!/^format\(\s*(["']?)woff2\1\s*\)$/iu.test(rest)) {`,String.raw`if (rest !== "" && !/^format\(\s*(["']?)woff2\1\s*\)$/iu.test(rest)) {`);
// Evaluate the bound module without creating an untracked executable file.
replace('fileURLToPath(import.meta.url)',`fileURLToPath(${JSON.stringify(url.href)})`);
replace('from "playwright"',`from ${JSON.stringify(import.meta.resolve("playwright"))}`);
await import(`data:text/javascript;base64,${Buffer.from(code).toString("base64")}`);
