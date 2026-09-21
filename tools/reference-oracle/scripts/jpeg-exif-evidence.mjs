import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { fixtures, scene, sha } from "./jpeg-exif-fixtures.mjs";
const ORACLE = resolve(dirname(fileURLToPath(import.meta.url)), ".."), ROOT = resolve(ORACLE, "../..");
const PROFILE = "tests/oracle/reference-profiles/revision-1.json";
const mode = process.argv[2]; assert.ok(["generate", "verify"].includes(mode));
assert.equal(process.env.REFERENCE_ORACLE_CANONICAL, "1"); assert.equal(process.version, "v24.16.0");
const profile = JSON.parse(readFileSync(resolve(ROOT, PROFILE))), rows = fixtures(ROOT);
const options = { exportBackground: false, exportEmbedScene: false, exportPadding: 0, exportScale: 1,
  exportWithDarkMode: false, frameRendering: { enabled: true, clip: true, name: true, outline: true },
  maxWidthOrHeight: null, viewBackgroundColor: "#ffffff" };
async function collect(order) {
  const browser = await chromium.launch({ args: profile.browser.launch.explicitArguments, chromiumSandbox: false, headless: true });
  try {
    assert.equal(browser.version(), "149.0.7827.55"); const result = {};
    for (const row of order) {
      const context = await browser.newContext(profile.browser.context);
      try {
        const page = await context.newPage(), errors = [];
        page.on("pageerror", e => errors.push(e.message));
        page.on("console", m => { if (m.type() === "error") errors.push(m.text()); });
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
        const intrinsic = await page.evaluate(async uri => {
          const image = new Image(); image.src = uri; await image.decode();
          return { width: image.naturalWidth, height: image.naturalHeight };
        }, `data:image/jpeg;base64,${row.jpeg_base64}`);
        const source = scene(row, intrinsic.width, intrinsic.height);
        const output = await page.evaluate(async ({ source, options }) => {
          const out = await window.referenceOracle.render(source, options);
          const image = new Image(); image.src = `data:image/svg+xml;base64,${btoa(unescape(encodeURIComponent(out.svg)))}`;
          await image.decode(); const canvas = document.createElement("canvas");
          canvas.width = source.elements[0].width; canvas.height = source.elements[0].height;
          canvas.getContext("2d", { colorSpace: "srgb" }).drawImage(image, 0, 0);
          return { png: out.pngBase64, svg: out.svg, svg_png: canvas.toDataURL("image/png").split(",")[1] };
        }, { source, options });
        assert.deepEqual(errors, [], row.id); result[row.id] = { ...intrinsic, scene: source, reference: output };
      } finally { await context.close(); }
    }
    return result;
  } finally { await browser.close(); }
}
const first = await collect(rows), second = await collect([...rows].reverse());
for (const row of rows) assert.deepEqual(first[row.id], second[row.id], row.id);
const paths = [PROFILE, "tests/oracle/reference-profiles/revision-1-assets.json", "tests/oracle/jpeg-color/inputs.json",
  "tools/reference-oracle/src/browser-entry.js", "tools/reference-oracle/package.json", "tools/reference-oracle/package-lock.json",
  "tools/reference-oracle/vite.config.mjs", "tools/reference-oracle/index.html", "tools/reference-oracle/scripts/jpeg-icc-fixtures.mjs",
  "tools/reference-oracle/scripts/jpeg-exif-fixtures.mjs", "tools/reference-oracle/scripts/jpeg-exif-evidence.mjs",
  "tools/reference-oracle/run-jpeg-exif.sh", "tests/conformance/comparator-profiles/canonical-visual-v1.json"];
const record = { schema: "excalidraw-render/jpeg-exif-reference/v1", options,
  sources: paths.map(path => ({ path, sha256: sha(readFileSync(resolve(ROOT, path))) })),
  collection: "Official v0.18.1; Chromium149; pinned OCI network none; two fresh processes forward/reverse; fresh context per vector",
  cases: rows.map(row => ({ ...row, ...first[row.id] })) };
const bytes = Buffer.from(JSON.stringify(record, null, 2) + "\n"), output = resolve(ROOT, "tests/oracle/jpeg-exif/reference.json");
if (mode === "verify") assert.deepEqual(bytes, readFileSync(output)); else writeFileSync(output, bytes, { flag: "wx" });
console.log(`${mode}: ${rows.length} EXIF vectors sha256:${sha(bytes)}`);
