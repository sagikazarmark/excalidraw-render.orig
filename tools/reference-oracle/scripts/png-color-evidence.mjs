// This collector is separate from the digest-frozen precision collector. Its
// opaque companion measures straight color independently of alpha quantization.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { fixtures, sceneFor } from "./png-color-fixtures.mjs";

const ORACLE = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const ROOT = resolve(ORACLE, "../..");
const INPUT = "tests/oracle/png-color/inputs.json";
const PROFILE = "tests/oracle/reference-profiles/revision-1.json";
const mode = process.argv[2];
assert.ok(["probe", "generate", "verify"].includes(mode));
assert.equal(process.env.REFERENCE_ORACLE_CANONICAL, "1");
assert.equal(process.version, "v24.16.0");
const profile = JSON.parse(readFileSync(resolve(ROOT, PROFILE)));
const rows = fixtures(JSON.parse(readFileSync(resolve(ROOT, INPUT))), mode === "probe");
const options = { exportBackground: false, exportEmbedScene: false, exportPadding: 0, exportScale: 1,
  exportWithDarkMode: false, frameRendering: { enabled: true, clip: true, name: true, outline: true },
  maxWidthOrHeight: null, viewBackgroundColor: "#ffffff" };
const sha256 = data => createHash("sha256").update(data).digest("hex");
const descriptor = path => ({ path, sha256: sha256(readFileSync(resolve(ROOT, path))) });
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
        page.on("console", message => { if (message.type() === "error") errors.push(message.text()); });
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
        outputs[row.id] = await page.evaluate(async ({ source, opaque, options }) => {
          const image = new Image(); image.src = source.files.asset.dataURL;
          try { await image.decode(); } catch { return { admitted: false }; }
          const first = await window.referenceOracle.render(source, options);
          const companion = await window.referenceOracle.render(opaque, options);
          return { admitted: true, png: first.pngBase64, opaque_png: companion.pngBase64, svg: first.svg };
        }, { source: sceneFor(row), opaque: sceneFor(row, true), options });
        assert.deepEqual(errors, [], row.id);
      } finally { await context.close(); }
    }
    return outputs;
  } finally { await browser.close(); }
}
const first = await collect(rows);
if (mode !== "probe") {
  const second = await collect([...rows].reverse());
  for (const row of rows) assert.deepEqual(first[row.id], second[row.id], `${row.id}: fresh-process/order drift`);
}
const record = { schema: "excalidraw-render/png-color-reference/v1", sources: [INPUT, PROFILE,
  "tools/reference-oracle/scripts/png-color-fixtures.mjs", "tools/reference-oracle/scripts/png-color-evidence.mjs",
  "tools/reference-oracle/run-png-color.sh", "tests/conformance/comparator-profiles/canonical-visual-v1.json"].map(descriptor), options,
  visual_class: "geometry-image",
  collection: mode === "probe" ? "exploratory only; one process" : "Official v0.18.1; pinned Chromium149; two fresh processes in forward/reverse order; fresh context per case; network none",
  cases: rows.map(row => { const { svg, ...output } = first[row.id]; return { ...row, scene: sceneFor(row), reference: { ...output, ...(svg ? { svg_sha256: sha256(Buffer.from(svg)) } : {}) } }; }) };
const bytes = Buffer.from(JSON.stringify(record, null, 2) + "\n");
const output = resolve(ROOT, mode === "probe" ? "target/png-color-probe.json" : "tests/oracle/png-color/reference.json");
if (mode === "verify") assert.deepEqual(bytes, readFileSync(output));
else writeFileSync(output, bytes, { flag: "wx" });
console.log(`${mode}: ${rows.length} PNG color cases; sha256:${sha256(bytes)}`);
