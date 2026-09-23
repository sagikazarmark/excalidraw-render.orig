// WebP container/metadata reference collection (#85 WebP facets): the pinned
// official v0.18.1 export in Chromium149, two fresh processes forward/reverse,
// a fresh context per case, network none. Records admission and the PNG.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { sceneFor, webpContainerFixtures } from "./webp-container-fixtures.mjs";

const ORACLE = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const ROOT = resolve(ORACLE, "../..");
const mode = process.argv[2];
assert.ok(["generate", "verify"].includes(mode));
assert.equal(process.env.REFERENCE_ORACLE_CANONICAL, "1");
assert.equal(process.version, "v24.16.0");
const INPUTS = ["tests/oracle/webp-sample/reference.json", "tests/oracle/png-color/reference.json", "tests/oracle/png-color/stage4-reference.json"];
const PROFILE = "tests/oracle/reference-profiles/revision-1.json";
const json = path => JSON.parse(readFileSync(resolve(ROOT, path)));
const profile = json(PROFILE);
const rows = webpContainerFixtures(...INPUTS.map(json));
const options = { exportBackground: false, exportEmbedScene: false, exportPadding: 0, exportScale: 1, exportWithDarkMode: false,
  frameRendering: { enabled: true, clip: true, name: true, outline: true }, maxWidthOrHeight: null, viewBackgroundColor: "#ffffff" };
const sha256 = data => createHash("sha256").update(data).digest("hex");
async function collect(order) {
  const browser = await chromium.launch({ args: profile.browser.launch.explicitArguments, chromiumSandbox: false, headless: true });
  try {
    assert.equal(browser.version(), "149.0.7827.55");
    const outputs = {};
    for (const row of order) {
      const context = await browser.newContext(profile.browser.context);
      try {
        const page = await context.newPage();
        const errors = [];
        page.on("pageerror", error => errors.push(error.message));
        await page.route("**/*", async route => {
          const url = new URL(route.request().url());
          if (url.origin !== "http://reference.invalid" || (url.pathname !== "/" && !url.pathname.startsWith("/bundle/"))) {
            errors.push(`unadmitted request ${url.href}`); await route.abort(); return;
          }
          const path = resolve(ORACLE, "dist", url.pathname === "/" ? "index.html" : url.pathname.slice(8));
          assert.ok(path.startsWith(resolve(ORACLE, "dist") + sep));
          await route.fulfill({ body: readFileSync(path), contentType: path.endsWith(".html") ? "text/html" : path.endsWith(".js") ? "text/javascript" : "application/octet-stream" });
        });
        await page.goto("http://reference.invalid/");
        await page.waitForFunction(() => typeof window.referenceOracle?.render === "function");
        outputs[row.id] = await page.evaluate(async ({ source, options }) => {
          const image = new Image(); image.src = source.files.asset.dataURL;
          try { await image.decode(); } catch { return { admitted: false }; }
          const size = [image.naturalWidth, image.naturalHeight];
          return { admitted: true, size, png: (await window.referenceOracle.render(source, options)).pngBase64 };
        }, { source: sceneFor(row), options });
        assert.deepEqual(errors, [], row.id);
      } finally { await context.close(); }
    }
    return outputs;
  } finally { await browser.close(); }
}
const first = await collect(rows);
const second = await collect([...rows].reverse());
for (const row of rows) assert.deepEqual(first[row.id], second[row.id], `${row.id}: fresh-process/order drift`);
const sources = [...INPUTS, PROFILE, "tools/reference-oracle/scripts/webp-container-fixtures.mjs",
  "tools/reference-oracle/scripts/webp-container-evidence.mjs", "tools/reference-oracle/run-webp-container.sh",
  "tests/conformance/comparator-profiles/canonical-visual-v1.json"].map(path => ({ path, sha256: sha256(readFileSync(resolve(ROOT, path))) }));
const record = { schema: "excalidraw-render/webp-container-reference/v1", sources, options, visual_class: "geometry-image",
  collection: "Official v0.18.1; pinned Chromium149; two fresh processes in forward/reverse order; fresh context per case; network none",
  cases: rows.map(row => ({ ...row, reference: first[row.id] })) };
const bytes = Buffer.from(JSON.stringify(record, null, 1) + "\n");
const output = resolve(ROOT, "tests/oracle/webp-container/reference.json");
if (mode === "verify") assert.deepEqual(bytes, readFileSync(output));
else writeFileSync(output, bytes, { flag: "wx" });
console.log(`${mode}: ${rows.length} WebP container cases; sha256:${sha256(bytes)}`);
