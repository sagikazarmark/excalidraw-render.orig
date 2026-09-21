import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { fixtures, scene, sha, SPEC, SIGNALS } from "./gif-control-fixtures.mjs";

const ORACLE = resolve(dirname(fileURLToPath(import.meta.url)), ".."), ROOT = resolve(ORACLE, "../..");
const PROFILE = "tests/oracle/reference-profiles/revision-1.json";
const OUTPUT = resolve(ROOT, "tests/oracle/gif-control/reference.json");
const mode = process.argv[2];
assert.ok(["generate", "verify"].includes(mode), "usage: gif-control-evidence.mjs <generate|verify>");
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
  "tools/reference-oracle/scripts/jpeg-icc-fixtures.mjs", "tools/reference-oracle/scripts/gif-control-fixtures.mjs",
  "tools/reference-oracle/scripts/gif-control-evidence.mjs", "tools/reference-oracle/run-gif-control.sh"])].sort();
assert.equal(paths.length, 18);
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
          const started = performance.now();
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
          // Record the deterministic bound result, not a nondeterministic duration.
          if (performance.now() - started >= 30000) throw new Error("capture exceeded 30s, first-frame timing proof invalid");
          return { admitted: true, width: input.naturalWidth, height: input.naturalHeight,
            capture_under_30_seconds: true, png: out.pngBase64, svg: out.svg, svg_png: svg.png,
            samples: { intrinsic, png, svg } };
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
      rgba_sha256: sha(actual), matches_first: dimensionsMatch && actual.equals(expected),
      matches_hypothesized_later_display: row.expected_later_display
        ? dimensionsMatch && actual.equals(Buffer.from(row.expected_later_display.rgba_base64, "base64")) : null }];
  }));
  return { admitted: true, all_match_authored: Object.values(surfaces).every(s => s.matches_first), surfaces };
}

// Same first-frame source bytes in each opaque trio; second-frame transparency
// changes the authored later display. All complete files must still validate.
const timerGroups = [];
for (const scope of ["global", "local"]) for (const pattern of [1, 2]) {
  const trio = ["later-no-gce", "later-transparent", "later-opaque"].map(signal =>
    rows.find(row => row.group === "two-frame" && row.palette_scope === scope && row.pattern === pattern && row.signal === signal));
  const prefixes = trio.map(row => Buffer.from(row.gif_base64, "base64").subarray(0, row.audit.frames[0].end_offset));
  for (const prefix of prefixes) assert.deepEqual(prefix, prefixes[0]);
  assert.notEqual(trio[0].expected_later_display.rgba_sha256, trio[1].expected_later_display.rgba_sha256);
  timerGroups.push({ ids: trio.map(row => row.id), identical_first_prefix_bytes: prefixes[0].length,
    identical_first_prefix_sha256: sha(prefixes[0]), first_rgba_sha256: trio[0].expected_first.rgba_sha256,
    later_display_sha256: trio.map(row => row.expected_later_display.rgba_sha256) });
}
const forward = await collect(rows), reverse = await collect([...rows].reverse());
for (const row of rows) assert.deepEqual(forward[row.id], reverse[row.id], `${row.id}: forward/reverse`);
assert.deepEqual(sources(), initialSources, "source files changed during collection");
const cases = rows.map(row => ({ ...row, scene: scene(row), reference: forward[row.id], comparison: comparison(row, forward[row.id]) }));
const differences = cases.filter(row => !row.comparison.all_match_authored).map(row => ({ id: row.id,
  signal: row.signal, scope_status: row.scope_status, comparison: row.comparison }));
const record = {
  schema: "excalidraw-render/gif-control-reference/v1", options, sources: initialSources,
  specification: { url: SPEC, title: "CompuServe GIF89a, 31 July 1990",
    sections: ["12 scope", "15-16 sub-blocks", "19-22 palettes and images", "23 GCE", "24 comment", "25 plain text", "26 application", "27 trailer", "Appendix F LZW"],
    hypotheses: { scope: "Comment/application preserve pending GCE; Plain Text and images consume it, including empty text data.",
      duplicate: "Section 23 allows at most one GCE per graphic. Complete last-GCE replacement is an explicit recovery hypothesis, not a spec requirement.",
      trailing: "Well-framed GCE with no following graphic: orphan scope probe, no retroactive effect hypothesized.",
      canvas: "Transparent pixels leave a transparent initial canvas unchanged; later disposal 1 preserves prior pixels.",
      plain_text: "Nonzero 9x7 grid, 1x1 cells, valid GCT, empty text data. No glyph raster claim; Chromium may skip text without consuming GCE. Measurements are retained." } },
  collection: { renderer: "official v0.18.1", chromium: "149.0.7827.55", profile_sha256: sha(readFileSync(resolve(ROOT, PROFILE))),
    oci: profile.canonicalPlatform.container.perPlatformDigest, network: "none",
    passes: ["fresh-process-forward", "fresh-process-reverse"], context: "fresh per vector per pass",
    equality: "exact complete observation equality, including PNG, SVG, SVG-derived PNG and RGBA",
    capture: "Static probes contain one table image; GCE delays zero except user-input=1 with 65535cs. Two-frame probes hold frame 1 for 65535cs, no user input or loop extension; each capture asserted under 30s.",
    later_frame_limit: "Browser samples establish first display only. Authored frame counts and complete-container validation are JS audit probes, not native later-frame display or production validation qualification.",
    malformed_controls: "Never submitted to Chromium; authored strict/recovery intent and distinct bytes only." },
  matrix: { static_cells: 24, static_vectors: 48, signals: SIGNALS, palette_scopes: ["global", "local"],
    palette_entries: 2, patterns_per_cell: 2, logical_screen: [9, 7], two_frame_cells: 8, two_frame_vectors: 16,
    collected_vectors: 64, spec_valid_vectors: cases.filter(row => row.scope_status === "spec-valid").length,
    nonconforming_scope_probes: cases.filter(row => row.scope_status !== "spec-valid").length,
    uncollected_malformed_controls: 20, total_vectors: 84, distinct_gif_bindings: 84 },
  authored_expectation_policy: "Explicit integer patterns and RGB tables, literal-only LZW independently packed. Expectations fixed before collection, never changed to match Chromium. Bounded separate parser traverses all frames; independent production-decoder validation remains unqualified.",
  timer_groups: timerGroups, differences, cases, controls,
};
const encoded = Buffer.from(JSON.stringify(record, null, 2) + "\n");
if (mode === "verify") assert.deepEqual(encoded, readFileSync(OUTPUT));
else writeFileSync(OUTPUT, encoded, { flag: "wx" });
console.log(`${mode}: ${cases.length} collected / ${controls.length} uncollected controls; ${differences.length} authored/reference differences; ${encoded.length} bytes; sha256:${sha(encoded)}`);
console.log(JSON.stringify({ admitted: cases.filter(row => row.reference.admitted).length,
  differences: differences.map(row => ({ id: row.id, surfaces: row.comparison.surfaces })), matrix: record.matrix }));
