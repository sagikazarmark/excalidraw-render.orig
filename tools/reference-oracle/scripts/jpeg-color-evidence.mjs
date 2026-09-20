import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";

const ORACLE = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const ROOT = resolve(ORACLE, "../..");
const INPUT = "tests/oracle/jpeg-color/inputs.json";
const PROFILE = "tests/oracle/reference-profiles/revision-1.json";
const mode = process.argv[2];
assert.ok(["generate", "verify"].includes(mode));
assert.equal(process.env.REFERENCE_ORACLE_CANONICAL, "1");
assert.equal(process.version, "v24.16.0");
const sha256 = data => createHash("sha256").update(data).digest("hex");
const profile = JSON.parse(readFileSync(resolve(ROOT, PROFILE)));
const rows = JSON.parse(readFileSync(resolve(ROOT, INPUT))).cases;
assert.equal(rows.length, 76);
assert.equal(new Set(rows.map(r => r.id)).size, 76);
const options = { exportBackground: false, exportEmbedScene: false, exportPadding: 0, exportScale: 1,
  exportWithDarkMode: false, frameRendering: { enabled: true, clip: true, name: true, outline: true },
  maxWidthOrHeight: null, viewBackgroundColor: "#ffffff" };
function scene(row) {
  return { type: "excalidraw", version: 2, elements: [{ id: "image", type: "image", x: 0, y: 0,
    width: row.width, height: row.height, angle: 0, opacity: 100, fileId: "asset", status: "saved",
    scale: [1, 1], seed: 1, version: 1, versionNonce: 1, updated: 1, isDeleted: false,
    groupIds: [], frameId: null, boundElements: null, link: null, locked: false,
    strokeColor: "#000000", backgroundColor: "transparent", fillStyle: "solid",
    strokeWidth: 1, strokeStyle: "solid", roughness: 0, index: "a0", crop: null, roundness: null }],
    appState: {}, files: { asset: { id: "asset", mimeType: "image/jpeg", created: 1,
      dataURL: `data:image/jpeg;base64,${row.jpeg_base64}` } } };
}
async function collect(order) {
  const browser = await chromium.launch({ args: profile.browser.launch.explicitArguments, chromiumSandbox: false, headless: true });
  try {
    assert.equal(browser.version(), "149.0.7827.55");
    const output = {};
    for (const row of order) {
      assert.equal(sha256(Buffer.from(row.jpeg_base64, "base64")), row.jpeg_sha256);
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
          // Independently rasterize the official SVG for the PDF color authority.
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
const first = await collect(rows);
const second = await collect([...rows].reverse());
for (const row of rows) assert.deepEqual(first[row.id], second[row.id], row.id);
const paths = [INPUT, PROFILE, "tests/oracle/reference-profiles/revision-1-assets.json",
  "crates/excalidraw-render/tests/jpeg_matrix_inputs.rs", "Cargo.lock",
  "tools/reference-oracle/src/browser-entry.js", "tools/reference-oracle/package.json",
  "tools/reference-oracle/vite.config.mjs", "tools/reference-oracle/index.html",
  "tools/reference-oracle/scripts/jpeg-color-evidence.mjs", "tools/reference-oracle/run-jpeg-color.sh",
  "tools/reference-oracle/package-lock.json", "tests/conformance/comparator-profiles/canonical-visual-v1.json"];
const record = { schema: "excalidraw-render/jpeg-color-reference/v1", sources: paths.map(path => ({ path, sha256: sha256(readFileSync(resolve(ROOT, path))) })),
  options, collection: "Official v0.18.1; Chromium149; pinned OCI network none; two fresh processes forward/reverse; fresh context per case",
  cases: rows.map(row => ({ ...row, scene: scene(row), reference: first[row.id] })) };
const bytes = Buffer.from(JSON.stringify(record, null, 2) + "\n");
const output = resolve(ROOT, "tests/oracle/jpeg-color/reference.json");
if (mode === "verify") assert.deepEqual(bytes, readFileSync(output));
else writeFileSync(output, bytes, { flag: "wx" });
console.log(`${mode}: 76 JPEG cases sha256:${sha256(bytes)}`);
