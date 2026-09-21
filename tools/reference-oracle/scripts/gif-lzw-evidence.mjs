import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { fixtures, scene, sha, SPEC } from "./gif-lzw-fixtures.mjs";

const ORACLE = resolve(dirname(fileURLToPath(import.meta.url)), ".."), ROOT = resolve(ORACLE, "../..");
const PROFILE = "tests/oracle/reference-profiles/revision-1.json";
const OUTPUT = resolve(ROOT, "tests/oracle/gif-lzw/reference.json");
const mode = process.argv[2];
assert.ok(["generate", "verify"].includes(mode), "usage: gif-lzw-evidence.mjs <generate|verify>");
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
  "tools/reference-oracle/scripts/jpeg-icc-fixtures.mjs", "tools/reference-oracle/scripts/gif-lzw-fixtures.mjs",
  "tools/reference-oracle/scripts/gif-lzw-evidence.mjs", "tools/reference-oracle/run-gif-lzw.sh"])].sort();
const sources = () => paths.map(path => ({ path, sha256: sha(readFileSync(resolve(ROOT, path))) }));
const initialSources = sources();
if (mode === "verify") assert.deepEqual(JSON.parse(readFileSync(OUTPUT)).sources, initialSources, "bound sources changed");
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
        const output = await page.evaluate(async ({ source, options }) => {
          const image = async src => { const im = new Image(); im.src = src; await im.decode(); return im; };
          const base64 = data => {
            let text = "";
            for (let i = 0; i < data.length; i += 8192) text += String.fromCharCode(...data.subarray(i, i + 8192));
            return btoa(text);
          };
          const sample = im => {
            const canvas = document.createElement("canvas");
            canvas.width = im.naturalWidth; canvas.height = im.naturalHeight;
            const ctx = canvas.getContext("2d", { colorSpace: "srgb" }); ctx.drawImage(im, 0, 0);
            return { width: canvas.width, height: canvas.height,
              rgba_base64: base64(ctx.getImageData(0, 0, canvas.width, canvas.height).data),
              png: canvas.toDataURL("image/png").split(",")[1] };
          };
          let input;
          try { input = await image(source.files.asset.dataURL); } catch { return { admitted: false }; }
          const intrinsic = sample(input);
          const out = await window.referenceOracle.render(source, options);
          const png = sample(await image(`data:image/png;base64,${out.pngBase64}`));
          const svg = sample(await image(`data:image/svg+xml;base64,${base64(new TextEncoder().encode(out.svg))}`));
          return { admitted: true, width: input.naturalWidth, height: input.naturalHeight,
            png: out.pngBase64, svg: out.svg, svg_png: svg.png, samples: { intrinsic, png, svg } };
        }, { source: scene(row), options });
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
    return [name, { dimensions_match: dimensionsMatch, changed_pixels: changed,
      rgba_sha256: sha(actual), matches_first: dimensionsMatch && actual.equals(expected) }];
  }));
  return { admitted: true, all_match_authored: Object.values(surfaces).every(s => s.matches_first), surfaces };
}

const forward = await collect(rows), reverse = await collect([...rows].reverse());
for (const row of rows) assert.deepEqual(forward[row.id], reverse[row.id], `${row.id}: forward/reverse`);
assert.deepEqual(sources(), initialSources, "source files changed during collection");
const cases = rows.map(row => ({ ...row, scene: scene(row), reference: forward[row.id], comparison: comparison(row, forward[row.id]) }));
const differences = cases.filter(row => !row.comparison.all_match_authored).map(row => ({ id: row.id, comparison: row.comparison }));
const range = field => [Math.min(...rows.map(row => row.stream[field])), Math.max(...rows.map(row => row.stream[field]))];
const record = {
  schema: "excalidraw-render/gif-lzw-reference/v1", options, sources: initialSources,
  specification: { url: SPEC, sections: ["15-16 sub-blocks", "17 version", "22 image data", "24 comment", "25 plain text", "26 application", "Appendix F LZW", "cover sheet deferred clear"],
    netscape: "Conventional NETSCAPE2.0 application payload; application framing is specified by section 26, looping semantics are not defined by GIF89a." },
  collection: { renderer: "official v0.18.1", chromium: "149.0.7827.55", profile_sha256: sha(readFileSync(resolve(ROOT, PROFILE))),
    oci: profile.canonicalPlatform.container.perPlatformDigest, network: "none",
    passes: ["fresh-process-forward", "fresh-process-reverse"], context: "fresh per vector per pass",
    equality: "exact complete observation equality, including PNG, SVG, SVG-derived PNG and RGBA",
    capture: "One opaque full-screen table image, no GCE or animation delay. Plain text precedes that image and is overwritten. NETSCAPE has only one table image; no animation timing claim." },
  matrix: { cells: 36, vectors: 72, patterns_per_cell: 2, palette_entries: [2, 16, 256], versions: ["87a", "89a"],
    programs: ["growth-to-full", "midstream-reset", "kwkwk"], subblock_sizes: [1, 255],
    logical_screen_by_palette: { "2": [257, 193], "16": [129, 97], "256": [129, 97] },
    extension_cells: 8, extension_vectors: 8, collected_vectors: 80, uncollected_controls: 12, total_vectors: 92, total_cells: 56 },
  stream_summary: { widths: [...new Set(rows.flatMap(row => row.stream.widths))].sort((a, b) => a - b),
    code_count_range: range("code_count"), dictionary_references_range: range("dictionary_references"),
    clear_count_range: range("clear_count"), kwkwk_count_range: range("kwkwk_count"),
    max_next_code_range: range("max_next_code"), full_dictionary_codes_range: range("full_dictionary_codes") },
  authored_expectation_policy: "Fixed-seed integer indices expanded directly through explicit palette; never derived from decode or adjusted to browser. Separate JS packed-byte audit is an author check; independent Rust checker belongs to parent.",
  differences, cases, controls,
};
const encoded = Buffer.from(JSON.stringify(record, null, 2) + "\n");
if (mode === "verify") assert.deepEqual(encoded, readFileSync(OUTPUT));
else writeFileSync(OUTPUT, encoded, { flag: "wx" });
console.log(`${mode}: 36 cells / 72 matrix + 8 extension vectors + 12 uncollected controls; ${differences.length} authored/reference differences; sha256:${sha(encoded)}`);
console.log(JSON.stringify(record.stream_summary));
