import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { fixtures, scene } from "./jpeg-icc-fixtures.mjs";
const ORACLE = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const ROOT = resolve(ORACLE, "../..");
const PROFILE = "tests/oracle/reference-profiles/revision-1.json";
const mode = process.argv[2];
assert.ok(["generate", "verify"].includes(mode));
assert.equal(process.env.REFERENCE_ORACLE_CANONICAL, "1");
assert.equal(process.version, "v24.16.0");
const sha256 = data => createHash("sha256").update(data).digest("hex");
const profile = JSON.parse(readFileSync(resolve(ROOT, PROFILE)));
const rows = fixtures(ROOT);
assert.equal(new Set(rows.map(r => r.id)).size, rows.length);
const options = { exportBackground: false, exportEmbedScene: false, exportPadding: 0, exportScale: 1,
  exportWithDarkMode: false, frameRendering: { enabled: true, clip: true, name: true, outline: true },
  maxWidthOrHeight: null, viewBackgroundColor: "#ffffff" };
async function collect(order) {
  const browser = await chromium.launch({ args: profile.browser.launch.explicitArguments, chromiumSandbox: false, headless: true });
  try {
    assert.equal(browser.version(), "149.0.7827.55");
    const output = {};
    for (const row of order) {
      const context = await browser.newContext(profile.browser.context);
      try {
        const page = await context.newPage();
        const errors = [];
        page.on("pageerror", error => errors.push(error.message));
        page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
        await page.route("**/*", async route => {
          const url = new URL(route.request().url());
          if (url.origin !== "http://reference.invalid" || (url.pathname !== "/" && !url.pathname.startsWith("/bundle/"))) {
            errors.push(`unadmitted ${url.href}`); await route.abort(); return;
          }
          const path = resolve(ORACLE, "dist", url.pathname === "/" ? "index.html" : url.pathname.slice(8));
          assert.ok(path.startsWith(resolve(ORACLE, "dist") + sep));
          await route.fulfill({ body: readFileSync(path), contentType: path.endsWith(".html") ? "text/html" : path.endsWith(".js") ? "text/javascript" : "application/octet-stream" });
        });
        await page.goto("http://reference.invalid/");
        await page.waitForFunction(() => typeof window.referenceOracle?.render === "function");
        output[row.id] = await page.evaluate(async ({ source, options }) => {
          const image = new Image(); image.src = source.files.asset.dataURL;
          try { await image.decode(); } catch { return { admitted: false }; }
          const result = await window.referenceOracle.render(source, options);
          const svgImage = new Image();
          svgImage.src = `data:image/svg+xml;base64,${btoa(unescape(encodeURIComponent(result.svg)))}`;
          await svgImage.decode();
          const canvas = document.createElement("canvas");
          canvas.width = image.naturalWidth; canvas.height = image.naturalHeight;
          const ctx = canvas.getContext("2d", { colorSpace: "srgb" });
          ctx.drawImage(svgImage, 0, 0);
          return { admitted: true, width: image.naturalWidth, height: image.naturalHeight,
            png: result.pngBase64, svg: result.svg, svg_png: canvas.toDataURL("image/png").split(",")[1] };
        }, { source: scene(row), options });
        assert.deepEqual(errors, [], row.id);
      } finally { await context.close(); }
    }
    return output;
  } finally { await browser.close(); }
}
const first = await collect(rows), second = await collect([...rows].reverse());
for (const row of rows) assert.deepEqual(first[row.id], second[row.id], row.id);
const paths = ["tests/oracle/jpeg-color/inputs.json", PROFILE, "tests/oracle/reference-profiles/revision-1-assets.json",
  "tools/reference-oracle/src/browser-entry.js", "tools/reference-oracle/package.json", "tools/reference-oracle/package-lock.json",
  "tools/reference-oracle/vite.config.mjs", "tools/reference-oracle/index.html",
  "tools/reference-oracle/scripts/jpeg-icc-fixtures.mjs", "tools/reference-oracle/scripts/jpeg-icc-evidence.mjs",
  "tools/reference-oracle/run-jpeg-icc.sh", "tests/conformance/comparator-profiles/canonical-visual-v1.json"];
const record = { schema: "excalidraw-render/jpeg-icc-reference/v1",
  sources: paths.map(path => ({ path, sha256: sha256(readFileSync(resolve(ROOT, path))) })), options,
  collection: "Official v0.18.1; Chromium149; pinned OCI network none; two fresh processes forward/reverse; fresh context per case",
  cases: rows.map(row => ({ ...row, scene: scene(row), reference: first[row.id] })) };
const bytes = Buffer.from(JSON.stringify(record, null, 2) + "\n");
const output = resolve(ROOT, "tests/oracle/jpeg-icc/reference.json");
if (mode === "verify") assert.deepEqual(bytes, readFileSync(output));
else writeFileSync(output, bytes, { flag: "wx" });
console.log(`${mode}: ${rows.length} JPEG ICC cases sha256:${sha256(bytes)}`);
