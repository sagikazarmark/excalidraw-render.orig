// Observe which platform font the pinned reference browser uses for each text
// node of an official SVG. Ambient fonts are expected here: this measures the
// substitute that an undeclared official family resolves to; it renders nothing.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve, sep } from "node:path";
import { chromium } from "../reference-oracle/node_modules/playwright/index.mjs";

const root = resolve(import.meta.dirname, "../..");
assert.equal(process.argv.length, 4, "usage: font-resolution.mjs <official.svg> <new-output.json>");
assert.equal(process.env.REFERENCE_ORACLE_CANONICAL, "1");
assert.equal(process.version, "v24.16.0");
const local = name => {
  const path = resolve(root, name);
  assert.ok(path.startsWith(root + sep), "path outside repository");
  return path;
};
const [inputName, outputName] = process.argv.slice(2);
const svg = readFileSync(local(inputName));
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
const profile = JSON.parse(readFileSync(resolve(root, "tests/oracle/reference-profiles/revision-1.json")));
const systemFonts = JSON.parse(readFileSync(resolve(root, "tests/oracle/reference-profiles/revision-1-assets.json"))).systemFonts.assets;

const browser = await chromium.launch({args: profile.browser.launch.explicitArguments, chromiumSandbox: false, headless: true});
let usages;
try {
  assert.equal(browser.version(), "149.0.7827.55");
  const context = await browser.newContext(profile.browser.context);
  const page = await context.newPage();
  await page.route("**/*", route => route.request().url() === "http://reference.invalid/input.svg"
    ? route.fulfill({body: svg, contentType: "image/svg+xml; charset=utf-8"}) : route.abort("blockedbyclient"));
  await page.goto("http://reference.invalid/input.svg", {waitUntil: "load"});
  await page.evaluate(() => document.fonts.ready);
  const session = await context.newCDPSession(page);
  await session.send("DOM.enable");
  await session.send("CSS.enable");
  const {root: documentRoot} = await session.send("DOM.getDocument", {depth: -1, pierce: true});
  const texts = [];
  const visit = node => {
    if (node.localName === "text") texts.push(node);
    for (const child of node.children ?? []) visit(child);
  };
  visit(documentRoot);
  usages = [];
  for (const node of texts) {
    const family = node.attributes[node.attributes.indexOf("font-family") + 1];
    const text = (node.children ?? []).filter(child => child.nodeType === 3).map(child => child.nodeValue).join("");
    const {fonts} = await session.send("CSS.getPlatformFontsForNode", {nodeId: node.nodeId});
    usages.push({font_family: family, text, fonts: fonts.map(({familyName, postScriptName, isCustomFont, glyphCount}) =>
      ({familyName, postScriptName, isCustomFont, glyphCount}))});
  }
} finally { await browser.close(); }

// Report each distinct undeclared family once, with fontconfig's own match and
// the pinned system-font file it names.
const families = new Map();
for (const usage of usages) {
  for (const font of usage.fonts.filter(font => !font.isCustomFont)) {
    const requested = usage.font_family.split(",")[0].trim();
    const key = `${requested}\u0000${font.postScriptName}`;
    if (families.has(key)) { families.get(key).text_nodes += 1; continue; }
    const file = execFileSync("fc-match", ["--format=%{file}", requested], {encoding: "utf8"});
    const postScript = execFileSync("fc-match", ["--format=%{postscriptname}", requested], {encoding: "utf8"});
    const pinned = systemFonts.find(asset => asset.path === file);
    assert.ok(pinned, `fontconfig match is not a pinned system font: ${file}`);
    assert.equal(sha(readFileSync(file)), pinned.sha256, "system font differs from the reference profile");
    families.set(key, {requested_family: requested, chromium: font, fontconfig: {file, postscript_name: postScript},
      agrees: postScript === font.postScriptName, system_font: pinned, text_nodes: 1});
  }
}
const record = {schema: "excalidraw-render/representative-font-resolution/v1", status: "observed-unapproved", approval: null,
  input: {path: inputName, sha256: sha(svg)}, browser: "149.0.7827.55", fontconfig: profile.canonicalPlatform.runtime.fontconfig,
  undeclared: [...families.values()], usages};
writeFileSync(local(outputName), JSON.stringify(record, null, 2) + "\n", {flag: "wx"});
console.log(JSON.stringify(record.undeclared, null, 2));
