import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { fixtures, scene, sha, INITIAL_DELAY_CS } from "./gif-first-frame-fixtures.mjs";

const ORACLE = resolve(dirname(fileURLToPath(import.meta.url)), ".."), ROOT = resolve(ORACLE, "../..");
const PROFILE = "tests/oracle/reference-profiles/revision-1.json";
const OUTPUT = resolve(ROOT, "tests/oracle/gif-first-frame/reference.json");
const mode = process.argv[2];
assert.ok(["generate", "verify"].includes(mode), "usage: gif-first-frame-evidence.mjs <generate|verify>");
assert.equal(process.env.REFERENCE_ORACLE_CANONICAL, "1");
assert.equal(process.version, "v24.16.0");
assert.equal(process.platform, "linux"); assert.equal(process.arch, "x64");
if (mode === "generate") assert.ok(!existsSync(OUTPUT), "immutable reference already exists; use verify");
const profile = JSON.parse(readFileSync(resolve(ROOT, PROFILE)));
assert.equal(profile.referenceRenderer.sourceTag, "v0.18.1");
assert.equal(profile.canonicalPlatform.runtime.chromium, "149.0.7827.55");
const { cases: rows, controls } = fixtures();
const options = { exportBackground: false, exportEmbedScene: false, exportPadding: 0, exportScale: 1,
  exportWithDarkMode: false, frameRendering: { enabled: true, clip: true, name: true, outline: true },
  maxWidthOrHeight: null, viewBackgroundColor: "#ffffff" };
const paths = [...new Set([PROFILE, "tests/oracle/reference-profiles/revision-1.sha256",
  "tests/oracle/reference-profiles/revision-1-assets.json", "tests/oracle/reference-profiles/revision-1-assets.sha256",
  ...profile.oracleWrapper.files.map(file => file.path), "tools/reference-oracle/package-lock.json",
  "tools/reference-oracle/scripts/jpeg-icc-fixtures.mjs",
  "tools/reference-oracle/scripts/gif-first-frame-fixtures.mjs",
  "tools/reference-oracle/scripts/gif-first-frame-evidence.mjs", "tools/reference-oracle/run-gif-first-frame.sh"])].sort();
const sources = () => paths.map(path => ({ path, sha256: sha(readFileSync(resolve(ROOT, path))) }));
const initialSources = sources();
const bundle = new Map(profile.browserBundle.bundle.files.map(file => {
  const body = readFileSync(resolve(ORACLE, "dist", file.path));
  assert.equal(sha(body), file.sha256, file.path);
  return [file.path === "index.html" ? "/" : `/bundle/${file.path}`, body];
}));

async function collect(order) {
  const browser = await chromium.launch({ args: profile.browser.launch.explicitArguments,
    chromiumSandbox: profile.browser.launch.chromiumSandbox, headless: profile.browser.launch.headless });
  try {
    assert.equal(browser.version(), "149.0.7827.55");
    const result = {};
    for (const row of order) {
      const context = await browser.newContext(profile.browser.context);
      try {
        const page = await context.newPage(), errors = [];
        page.on("pageerror", e => errors.push(e.message));
        page.on("console", m => { if (m.type() === "error") errors.push(m.text()); });
        page.on("requestfailed", request => errors.push(`request failed: ${request.url()}`));
        await context.route("**/*", async route => {
          const url = new URL(route.request().url()), body = bundle.get(url.pathname);
          if (url.origin !== "http://reference.invalid" || url.search || !body) {
            errors.push(`unadmitted ${url.href}`); await route.abort(); return;
          }
          await route.fulfill({ body, contentType: url.pathname === "/" ? "text/html" : url.pathname.endsWith(".js") ? "text/javascript" : "application/octet-stream" });
        });
        await page.goto("http://reference.invalid/");
        await page.waitForFunction(() => typeof window.referenceOracle?.render === "function");
        const started = performance.now();
        const output = await page.evaluate(async ({ source, options }) => {
          // Bound the entire operation starting BEFORE the first GIF Image is created.
          const started = performance.now();
          const image = async src => { const im = new Image(); im.src = src; await im.decode(); return im; };
          const sample = im => {
            const canvas = document.createElement("canvas");
            canvas.width = im.naturalWidth; canvas.height = im.naturalHeight;
            const ctx = canvas.getContext("2d", { colorSpace: "srgb" }); ctx.drawImage(im, 0, 0);
            return { width: canvas.width, height: canvas.height,
              rgba_base64: btoa(String.fromCharCode(...ctx.getImageData(0, 0, canvas.width, canvas.height).data)),
              png: canvas.toDataURL("image/png").split(",")[1] };
          };
          let input;
          try { input = await image(source.files.asset.dataURL); } catch { return { admitted: false }; }
          const intrinsic = sample(input);
          const out = await window.referenceOracle.render(source, options);
          const png = sample(await image(`data:image/png;base64,${out.pngBase64}`));
          const svg = sample(await image(`data:image/svg+xml;base64,${btoa(unescape(encodeURIComponent(out.svg)))}`));
          if (performance.now() - started >= 30000) throw new Error("capture exceeded 30-second first-frame bound");
          return { admitted: true, width: input.naturalWidth, height: input.naturalHeight,
            png: out.pngBase64, svg: out.svg, svg_png: svg.png,
            samples: { intrinsic, png, svg }, capture_under_30_seconds: true };
        }, { source: scene(row), options });
        assert.ok(performance.now() - started < 30000, `${row.id}: host capture bound`);
        assert.deepEqual(errors, [], row.id);
        result[row.id] = output;
      } finally { await context.close(); }
    }
    return result;
  } finally { await browser.close(); }
}

function comparison(row, reference) {
  if (!reference.admitted) return { admitted: false, all_match_authored: false, surfaces: {} };
  const expected = Buffer.from(row.expected_first.rgba_base64, "base64");
  const surfaces = Object.fromEntries(Object.entries(reference.samples).map(([name, sample]) => {
    const actual = Buffer.from(sample.rgba_base64, "base64");
    const dimensionsMatch = sample.width === row.width && sample.height === row.height;
    let changed = 0;
    for (let i = 0; i < Math.max(actual.length, expected.length); i += 4) {
      if (!actual.subarray(i, i + 4).equals(expected.subarray(i, i + 4))) changed++;
    }
    const digest = sha(actual);
    return [name, { dimensions_match: dimensionsMatch, changed_pixels: changed, rgba_sha256: digest,
      matches_first: dimensionsMatch && actual.equals(expected),
      matches_later: row.expected_later ? dimensionsMatch && digest === row.expected_later.rgba_sha256 : null }];
  }));
  return { admitted: true, all_match_authored: Object.values(surfaces).every(s => s.matches_first), surfaces };
}

const forward = await collect(rows), reverse = await collect([...rows].reverse());
for (const row of rows) assert.deepEqual(forward[row.id], reverse[row.id], `${row.id}: forward/reverse`);
assert.deepEqual(sources(), initialSources, "source files changed during collection");
const cases = rows.map(row => ({ ...row, scene: scene(row), reference: forward[row.id], comparison: comparison(row, forward[row.id]) }));
const differences = cases.filter(row => !row.comparison.all_match_authored).map(row => ({ id: row.id, comparison: row.comparison }));
const record = {
  schema: "excalidraw-render/gif-first-frame-reference/v1", options, sources: initialSources,
  collection: { renderer: "official v0.18.1", chromium: "149.0.7827.55", profile_sha256: sha(readFileSync(resolve(ROOT, PROFILE))),
    oci: profile.canonicalPlatform.container.perPlatformDigest, network: "none",
    passes: ["fresh-process-forward", "fresh-process-reverse"], context: "fresh per vector per pass",
    equality: "exact complete observation equality, including PNG, SVG, SVG-derived PNG and RGBA",
    capture: { initial_delay_cs: INITIAL_DELAY_CS, maximum_capture_ms: 30000,
      bound_checked_in: ["browser monotonic clock", "host monotonic clock"],
      claim: "Only these long-delay fixtures captured within the bound; no arbitrary-offset animation timing claim.",
      later_frame: "Full-screen opaque shifted pattern; authored first/later RGBA differ for every animation." } },
  matrix: { cells: 120, vectors: 240, patterns_per_cell: 2, logical_screen: [9, 7],
    palettes: ["global", "local", "both-local-override"], rectangles: ["full 9x7 at 0,0", "5x5 at 2,1"],
    transparency: ["opaque", "index 0 transparent"], interlace: [false, true],
    animation: ["static disposal 0", "animated disposal 0", "animated disposal 1", "animated disposal 2", "animated disposal 3"],
    controls: 16, total_vectors: 256 },
  authored_expectation_policy: "Independent palette/index-coordinate expansion onto transparent logical screen; never adjusted from browser observations.",
  differences, cases, controls,
};
const encoded = Buffer.from(JSON.stringify(record, null, 2) + "\n");
if (mode === "verify") assert.deepEqual(encoded, readFileSync(OUTPUT));
else writeFileSync(OUTPUT, encoded, { flag: "wx" });
console.log(`${mode}: 120 cells / 240 collected vectors + 16 uncollected controls; ${differences.length} authored/reference differences; sha256:${sha(encoded)}`);
