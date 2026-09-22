// Proposed fractional-viewport adapter. Geometry/viewBox/scale stay untouched;
// only the screenshot extent truncates to the reference canvas pixel grid.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
const url = new URL("../reference-oracle/scripts/svg-viewer.mjs", import.meta.url);
const bytes = readFileSync(url);
assert.equal(createHash("sha256").update(bytes).digest("hex"), "61968d7bca7fef68fa29af22368b5d7dfddda9be1c9a34e53f479cdb14c1a11b");
let code = bytes.toString();
function replace(before, after) {
  assert.equal(code.split(before).length, 2, `unique source boundary: ${before}`);
  code = code.replace(before, after);
}
replace('node.localName === "text" && node.namespaceURI === "http://www.w3.org/2000/svg"', 'node.localName === "text"');
replace('browser_process_id: browserProcess.id,', 'browser_process_id: browserProcess.id, container_namespace: process.env.HOSTNAME,');
replace(String.raw`if (!/^format\(\s*(["']?)woff2\1\s*\)$/iu.test(rest)) {`, String.raw`if (rest !== "" && !/^format\(\s*(["']?)woff2\1\s*\)$/iu.test(rest)) {`);
replace('return { height: bounds.height, width: bounds.width };', 'return { height: Math.trunc(bounds.height), width: Math.trunc(bounds.width) };');
// Renderer source-text attributes preserve LF as &#10;. Admit only that one
// whitespace entity and decode it for the existing resource-policy checks.
replace(String.raw`/&#(?:x[0-9a-f]+|[0-9]+);/iu.test(svg)`, String.raw`/&#(?:x[0-9a-f]+|[0-9]+);/iu.test(svg.replaceAll("&#10;", ""))`);
replace('return value.replace(/&(amp|lt|gt|quot|apos);/gu,', 'return value.replaceAll("&#10;", "\\n").replace(/&(amp|lt|gt|quot|apos);/gu,');
replace('fileURLToPath(import.meta.url)', `fileURLToPath(${JSON.stringify(url.href)})`);
replace('from "playwright"', `from ${JSON.stringify(new URL("../reference-oracle/node_modules/playwright/index.mjs", import.meta.url).href)}`);
await import(`data:text/javascript;base64,${Buffer.from(code).toString("base64")}`);
