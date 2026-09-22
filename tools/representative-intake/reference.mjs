// Isolated pinned-browser collection; exports the supplied scene without repair.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { resolve, sep } from "node:path";
import { deflateSync } from "node:zlib";
import { chromium } from "../reference-oracle/node_modules/playwright/index.mjs";

const root = resolve(import.meta.dirname, "../..");
const [bundleName, outputName] = process.argv.slice(2);
assert.equal(process.argv.length, 4, "usage: reference.mjs <intake> <new-output>");
assert.equal(process.env.REFERENCE_ORACLE_CANONICAL, "1");
assert.equal(process.version, "v24.16.0");
const local = name => {
  const path = resolve(root, name);
  assert.ok(path.startsWith(root + sep), "path outside repository");
  return path;
};
const bundle = local(bundleName), output = local(outputName);
const sceneBytes = readFileSync(resolve(bundle, "scene.excalidraw"));
const planBytes = readFileSync(resolve(bundle, "plan.json"));
const scene = JSON.parse(sceneBytes), plan = JSON.parse(planBytes);
const profile = JSON.parse(readFileSync(resolve(root, "tests/oracle/reference-profiles/revision-1.json")));
const assets = JSON.parse(readFileSync(resolve(root, "tests/oracle/reference-profiles/revision-1-assets.json"))).officialPackage.assets;
const sha = bytes => createHash("sha256").update(bytes).digest("hex");
const sourcePaths = ["tools/representative-intake/reference.mjs", "tools/reference-oracle/src/browser-entry.js",
  "tools/reference-oracle/package-lock.json", "tools/reference-oracle/vite.config.mjs",
  "tests/oracle/reference-profiles/revision-1.json", "tests/oracle/reference-profiles/revision-1-assets.json"];
const sources = () => sourcePaths.map(path => ({path, sha256: sha(readFileSync(resolve(root, path)))}));
const before = sources();
assert.equal(plan.schema, "excalidraw-render/representative-intake-plan/v1");
assert.deepEqual(plan.formats, ["svg", "png", "pdf"]);
assert.ok(plan.variants.length > 0);
const variantIds = new Set();
for (const variant of plan.variants) {
  assert.match(variant.id, /^[a-z][a-z0-9-]*$/);
  assert.ok(!variantIds.has(variant.id)); variantIds.add(variant.id);
  assert.ok(["light", "dark"].includes(variant.color_mode));
  assert.equal(typeof variant.include_background, "boolean");
  assert.equal(typeof variant.metadata, "boolean");
  assert.ok(Number.isFinite(variant.scale) && variant.scale > 0 && variant.scale <= 4);
  assert.ok(Number.isFinite(variant.padding) && variant.padding >= 0 && variant.padding <= 100);
}

async function render(variant) {
  const options = {exportBackground: variant.include_background, exportEmbedScene: variant.metadata,
    exportPadding: variant.padding, exportScale: variant.scale, exportWithDarkMode: variant.color_mode === "dark",
    frameRendering: {enabled: true, clip: true, name: true, outline: true}, maxWidthOrHeight: null,
    viewBackgroundColor: scene.appState.viewBackgroundColor ?? "#ffffff"};
  const browser = await chromium.launch({args: profile.browser.launch.explicitArguments, chromiumSandbox: false, headless: true});
  try {
    assert.equal(browser.version(), "149.0.7827.55");
    const context = await browser.newContext(profile.browser.context);
    const page = await context.newPage(), errors = [], requests = [];
    page.on("pageerror", error => errors.push(error.message));
    page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
    await page.route("**/*", async route => {
      try {
        const url = new URL(route.request().url());
        assert.equal(url.origin, "http://reference.invalid");
        let directory, name;
        if (url.pathname.startsWith("/assets/")) {
          name = url.pathname.slice(8); assert.ok(assets.some(asset => asset.path === name));
          directory = resolve(root, "tools/reference-oracle/node_modules/@excalidraw/excalidraw/dist/prod");
        } else {
          assert.ok(url.pathname === "/" || url.pathname.startsWith("/bundle/"));
          name = url.pathname === "/" ? "index.html" : url.pathname.slice(8);
          directory = resolve(root, "tools/reference-oracle/dist");
        }
        const path = resolve(directory, name); assert.ok(path.startsWith(directory + sep));
        const body = readFileSync(path);
        requests.push({url: url.href, sha256: sha(body)});
        await route.fulfill({body, contentType: path.endsWith(".html") ? "text/html" : path.endsWith(".js") ? "text/javascript" : path.endsWith(".woff2") ? "font/woff2" : "application/octet-stream"});
      } catch (error) { errors.push(error.message); await route.abort(); }
    });
    await page.goto("http://reference.invalid/");
    await page.waitForFunction(() => typeof window.referenceOracle?.render === "function");
    const value = await page.evaluate(async ({scene, options}) => {
      const result = await window.referenceOracle.render(scene, options);
      const image = new Image(); image.src = `data:image/png;base64,${result.pngBase64}`; await image.decode();
      const canvas = document.createElement("canvas"); canvas.width = image.width; canvas.height = image.height;
      const ctx = canvas.getContext("2d"); ctx.drawImage(image, 0, 0);
      const svg = new DOMParser().parseFromString(result.svg, "image/svg+xml").documentElement;
      // Base64 keeps large canvases within Node's heap; a JSON number array does not.
      const rgba = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      let binary = "";
      for (let offset = 0; offset < rgba.length; offset += 0x8000)
        binary += String.fromCharCode(...rgba.subarray(offset, offset + 0x8000));
      return {...result, width: canvas.width, height: canvas.height,
        svg_width: svg.getAttribute("width"), svg_height: svg.getAttribute("height"), view_box: svg.getAttribute("viewBox"),
        rgbaBase64: btoa(binary)};
    }, {scene, options});
    assert.deepEqual(errors, []);
    requests.sort((a, b) => a.url.localeCompare(b.url));
    return {value, options, requests};
  } finally { await browser.close(); }
}

const collected = new Map();
for (const variant of plan.variants) collected.set(variant.id, await render(variant));
for (const variant of [...plan.variants].reverse()) assert.deepEqual(await render(variant), collected.get(variant.id), `${variant.id} repeat`);
assert.deepEqual(sources(), before);
assert.deepEqual(readFileSync(resolve(bundle, "scene.excalidraw")), sceneBytes);
assert.deepEqual(readFileSync(resolve(bundle, "plan.json")), planBytes);
mkdirSync(output); // New-only; no completed evidence is overwritten.
const rows = [];
for (const variant of plan.variants) {
  const {value, options, requests} = collected.get(variant.id);
  const files = {"official.svg": Buffer.from(value.svg), "official.png": Buffer.from(value.pngBase64, "base64"), "official-png.rgba.zlib": deflateSync(Buffer.from(value.rgbaBase64, "base64"), {level:9})};
  const dir = resolve(output, variant.id); mkdirSync(dir);
  for (const [name, bytes] of Object.entries(files)) writeFileSync(resolve(dir, name), bytes, {flag: "wx"});
  rows.push({variant: variant.id, width: value.width, height: value.height, svg_width: value.svg_width, svg_height: value.svg_height,
    view_box: value.view_box, options, requests, fresh_browser_runs: 2,
    files: Object.fromEntries(Object.entries(files).map(([name, bytes]) => [name, {bytes: bytes.length, sha256: sha(bytes)}]))});
}
const record = {schema: "excalidraw-render/representative-intake-reference/v1", status: "observed-unapproved", approval: null,
  eligible_executions: [], case_id: plan.case_id, scene_sha256: sha(sceneBytes), plan_sha256: sha(planBytes),
  browser: "149.0.7827.55", network: "none", sources: before, rows};
writeFileSync(resolve(output, "reference.json"), JSON.stringify(record, null, 2) + "\n", {flag: "wx"});
console.log(JSON.stringify({status: record.status, dimensions: rows.map(({variant,width,height,svg_width,svg_height}) => ({variant,width,height,svg_width,svg_height}))}));
