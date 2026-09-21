import assert from "node:assert/strict";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync } from "node:child_process";
import { chromium } from "playwright";
import { fixtures, frameRequests, scene, sha, OCI, CONFIGURATIONS, INITIAL_DURATION_MS } from "./webp-sample-fixtures.mjs";

const ORACLE = resolve(dirname(fileURLToPath(import.meta.url)), ".."), ROOT = resolve(ORACLE, "../..");
const PROFILE = "tests/oracle/reference-profiles/revision-1.json";
const INPUT = "tests/oracle/webp-sample/inputs.json", OUTPUT = resolve(ROOT, "tests/oracle/webp-sample/reference.json");
const mode = process.argv[2];
assert.ok(["prepare", "generate", "verify"].includes(mode), "usage: webp-sample-evidence.mjs <prepare|generate|verify>");
assert.equal(process.env.REFERENCE_ORACLE_CANONICAL, "1");
assert.equal(process.version, "v24.16.0"); assert.equal(process.platform, "linux"); assert.equal(process.arch, "x64");
const profile = JSON.parse(readFileSync(resolve(ROOT, PROFILE)));
assert.equal(profile.referenceRenderer.sourceTag, "v0.18.1");
assert.equal(profile.canonicalPlatform.runtime.chromium, "149.0.7827.55");
const authorPaths = ["tools/reference-oracle/scripts/webp-sample-fixtures.mjs", "tools/reference-oracle/scripts/webp-encode-inputs.py",
  "tools/reference-oracle/scripts/webp-sample-evidence.mjs", "tools/reference-oracle/run-webp-sample.sh",
  "tools/reference-oracle/scripts/jpeg-icc-fixtures.mjs"].sort();
const hashes = paths => paths.map(path => ({ path, sha256: sha(readFileSync(resolve(ROOT, path))) }));
const authorSources = hashes(authorPaths);
const encode = () => JSON.parse(execFileSync("/usr/bin/python3", [resolve(ORACLE, "scripts/webp-encode-inputs.py")],
  { input: JSON.stringify(frameRequests()), maxBuffer: 8 * 1024 * 1024, timeout: 30000 }));
const inputRecord = () => ({ schema: "excalidraw-render/webp-sample-inputs/v1", oci: OCI,
  author_sources: authorSources, ...encode() });
const serialize = value => Buffer.from(JSON.stringify(value, null, 2) + "\n");
if (mode === "prepare") {
  assert.ok(!existsSync(resolve(ROOT, INPUT)), "immutable inputs exist");
  const input = inputRecord(); fixtures(input);
  assert.deepEqual(hashes(authorPaths), authorSources);
  const bytes = serialize(input); writeFileSync(resolve(ROOT, INPUT), bytes, { flag: "wx" });
  console.log(`prepare: ${input.frames.length} independent encoded frames; sha256:${sha(bytes)}`);
  process.exit(0);
}
if (mode === "generate") assert.ok(!existsSync(OUTPUT), "immutable reference exists; use verify");
const inputsBytes = readFileSync(resolve(ROOT, INPUT)), inputs = JSON.parse(inputsBytes);
assert.deepEqual(inputs.author_sources, authorSources, "frozen input source closure changed");
// Re-encoding is offline and compared before any browser observation, in both modes.
assert.deepEqual(serialize(inputRecord()), inputsBytes, "external encoder/input reproducibility");
const { cases: rows, controls } = fixtures(inputs);
const frames = new Map(inputs.frames.map(frame => [frame.id, frame]));
const options = { exportBackground: false, exportEmbedScene: false, exportPadding: 0, exportScale: 1,
  exportWithDarkMode: false, frameRendering: { enabled: true, clip: true, name: true, outline: true },
  maxWidthOrHeight: null, viewBackgroundColor: "#ffffff" };
const paths = [...new Set([...authorPaths, PROFILE, INPUT, "tests/oracle/reference-profiles/revision-1.sha256",
  "tests/oracle/reference-profiles/revision-1-assets.json", "tests/oracle/reference-profiles/revision-1-assets.sha256",
  ...profile.oracleWrapper.files.map(file => file.path), "tools/reference-oracle/package-lock.json",
  "tools/reference-oracle/scripts/profile.mjs", "tools/reference-oracle/scripts/canonical-json.mjs"])].sort();
const initialSources = hashes(paths);
const bundle = new Map(profile.browserBundle.bundle.files.map(file => {
  const body = readFileSync(resolve(ORACLE, "dist", file.path)); assert.equal(sha(body), file.sha256, file.path);
  return [file.path === "index.html" ? "/" : `/bundle/${file.path}`, body];
}));
const timings = [];

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
        page.on("requestfailed", r => errors.push(`request failed: ${r.url()}`));
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
        const output = await page.evaluate(async ({ row, source, first, later, options }) => {
          const started = performance.now();
          const image = async src => { const im = new Image(); im.src = src; await im.decode(); return im; };
          const canvas = (width, height) => { const c = document.createElement("canvas"); c.width = width; c.height = height; return c; };
          const ctx = c => c.getContext("2d", { colorSpace: "srgb" });
          const b64 = bytes => btoa(String.fromCharCode(...bytes));
          const unb64 = text => Uint8ClampedArray.from(atob(text), c => c.charCodeAt(0));
          const read = c => ({ width: c.width, height: c.height,
            rgba_base64: b64(ctx(c).getImageData(0, 0, c.width, c.height).data), png: c.toDataURL("image/png").split(",")[1] });
          const sample = im => {
            const width = im.naturalWidth || im.width, height = im.naturalHeight || im.height;
            const c = canvas(width, height); ctx(c).drawImage(im, 0, 0);
            const composites = {};
            // Draw the original source directly onto opaque targets, retaining its
            // internal precision instead of recomposing rounded getImageData bytes.
            for (const [name, color] of [["black", "#000000"], ["white", "#ffffff"]]) {
              const opaque = canvas(width, height); ctx(opaque).fillStyle = color;
              ctx(opaque).fillRect(0, 0, width, height); ctx(opaque).drawImage(im, 0, 0); composites[name] = read(opaque);
            }
            return { ...read(c), composites };
          };
          const authoredCanvas = (text, width, height) => {
            const c = canvas(width, height); ctx(c).putImageData(new ImageData(unb64(text), width, height), 0, 0); return c;
          };
          let input;
          try { input = await image(source.files.asset.dataURL); } catch { return { admitted: false }; }
          const intrinsic = sample(input);
          const out = await window.referenceOracle.render(source, options);
          const png = sample(await image(`data:image/png;base64,${out.pngBase64}`));
          const svg = sample(await image(`data:image/svg+xml;base64,${btoa(unescape(encodeURIComponent(out.svg)))}`));
          const firstImage = await image(`data:image/webp;base64,${first.encoded.base64}`);
          const composition = useBackground => {
            const c = canvas(row.width, row.height), r = row.first_rect, bg = row.config.background;
            if (useBackground && bg[3]) {
              ctx(c).fillStyle = `rgba(${bg[0]},${bg[1]},${bg[2]},${bg[3] / 255})`; ctx(c).fillRect(0, 0, c.width, c.height);
            }
            if (!row.config.blend) ctx(c).clearRect(r.left, r.top, r.width, r.height);
            ctx(c).drawImage(firstImage, r.left, r.top); return sample(c);
          };
          const diagnostic = {
            standalone_first: sample(firstImage),
            standalone_later: later ? sample(await image(`data:image/webp;base64,${later.encoded.base64}`)) : null,
            browser_first_transparent: composition(false), browser_first_anim_background: composition(true),
            authored_transparent_canvas: sample(authoredCanvas(row.expected_first_transparent.base64, row.width, row.height)),
            authored_anim_background_canvas: sample(authoredCanvas(row.expected_first_anim_background.base64, row.width, row.height)),
            native_straight_canvas: sample(authoredCanvas(first.native_decoded_rgba.base64, first.width, first.height)),
          };
          if (performance.now() - started >= 30000) throw new Error("capture exceeded 30-second first-frame bound");
          return { admitted: true, png: out.pngBase64, svg: out.svg, svg_png: svg.png,
            samples: { intrinsic, png, svg }, diagnostic, capture_under_30_seconds: true };
        }, { row, source: scene(row), first: frames.get(row.first_frame_id), later: frames.get(row.later_frame_id) ?? null, options });
        const elapsed = performance.now() - started; timings.push(elapsed);
        assert.ok(elapsed < 30000, `${row.id}: host capture bound`); assert.deepEqual(errors, [], row.id);
        result[row.id] = output;
      } finally { await context.close(); }
    }
    return result;
  } finally { await browser.close(); }
}

function compare(actual, expected, width, height) {
  const a = Buffer.from(actual.rgba_base64, "base64"), b = Buffer.from(expected, "base64");
  let changed = 0, alphaChanged = 0, visibleRgbChanged = 0, nonzeroInvisible = 0;
  for (let i = 0; i < Math.max(a.length, b.length); i += 4) {
    if (!a.subarray(i, i + 4).equals(b.subarray(i, i + 4))) changed++;
    if (a[i + 3] !== b[i + 3]) alphaChanged++;
    if (b[i + 3] && !a.subarray(i, i + 3).equals(b.subarray(i, i + 3))) visibleRgbChanged++;
    if (a[i + 3] === 0 && a.subarray(i, i + 3).some(v => v !== 0)) nonzeroInvisible++;
  }
  const dimensionsMatch = actual.width === width && actual.height === height;
  return { dimensions_match: dimensionsMatch, rgba_sha256: sha(a), changed_pixels: changed,
    alpha_changed_pixels: alphaChanged, visible_rgb_changed_pixels: visibleRgbChanged,
    nonzero_invisible_pixels: nonzeroInvisible, exact: dimensionsMatch && a.equals(b) };
}

function comparison(row, reference) {
  if (!reference.admitted) return { admitted: false, surfaces: {} };
  const d = reference.diagnostic;
  return { admitted: true, source_rgb_is_exact_codec_expectation: row.codec === "lossless",
    surfaces: Object.fromEntries(Object.entries(reference.samples).map(([name, s]) => [name, {
      authored_transparent_straight: compare(s, row.expected_first_transparent.base64, row.width, row.height),
      authored_anim_background_straight: compare(s, row.expected_first_anim_background.base64, row.width, row.height),
      authored_transparent_canvas: compare(s, d.authored_transparent_canvas.rgba_base64, row.width, row.height),
      browser_first_transparent: compare(s, d.browser_first_transparent.rgba_base64, row.width, row.height),
      browser_first_anim_background: compare(s, d.browser_first_anim_background.rgba_base64, row.width, row.height),
      matches_later: d.standalone_later ? compare(s, d.standalone_later.rgba_base64, row.width, row.height).exact : null,
      matches_intrinsic: compare(s, reference.samples.intrinsic.rgba_base64, row.width, row.height).exact,
      opaque_composites_match_intrinsic: Object.fromEntries(["black", "white"].map(bg => [bg,
        compare(s.composites[bg], reference.samples.intrinsic.composites[bg].rgba_base64, row.width, row.height).exact])),
    }])) };
}

const started = performance.now();
const forward = await collect(rows), reverse = await collect([...rows].reverse());
for (const row of rows) assert.deepEqual(forward[row.id], reverse[row.id], `${row.id}: forward/reverse`);
assert.deepEqual(hashes(paths), initialSources, "source files changed during collection");
const cases = rows.map(row => ({ ...row, scene: scene(row), reference: forward[row.id], comparison: comparison(row, forward[row.id]) }));
// Include every failed hypothesis, including lossy source RGB differences. These
// are observations, not a reason to refresh expectations or exclude a vector.
const differences = cases.flatMap(row => {
  if (!row.reference.admitted) return [{ id: row.id, admitted: false }];
  const surfaces = Object.fromEntries(Object.entries(row.comparison.surfaces).map(([name, s]) => [name,
    Object.entries(s).filter(([key, value]) => value && typeof value === "object" && "exact" in value && !value.exact).map(([key]) => key)]));
  return Object.values(surfaces).some(v => v.length) ? [{ id: row.id, surfaces }] : [];
});
const record = {
  schema: "excalidraw-render/webp-sample-reference/v1", sources: initialSources, options,
  collection: { renderer: "official v0.18.1", chromium: "149.0.7827.55", profile_sha256: sha(readFileSync(resolve(ROOT, PROFILE))),
    oci: OCI, network: "none", passes: ["fresh-process-forward", "fresh-process-reverse"], context: "fresh per vector per pass",
    equality: "exact complete observations including PNG, SVG, SVG-raster PNG, RGBA, controls and black/white composites",
    capture: { first_duration_ms: INITIAL_DURATION_MS, maximum_capture_ms: 30000,
      bound_checked_in: ["browser monotonic clock", "host monotonic clock"],
      claim: "First frame only for these long-duration fixtures within bound; no arbitrary-time or later-disposal claim" } },
  matrix: { cells: 36, vectors: 72, patterns_per_cell: 2, configurations: CONFIGURATIONS,
    codecs: ["lossless VP8L", "lossy VP8 with ALPH when nonopaque"], alpha: ["opaque", "partial", "binary"],
    static_vectors: 12, animated_vectors: 60, controls: 12, total_vectors: 84 },
  expectation_policy: "Authored source RGBA and two composition hypotheses never changed by browser; lossy RGB canonical is exact browser sample, not source RGB or a tolerance; exact alpha separately recorded",
  inputs_sha256: sha(inputsBytes), encoder: inputs.encoder, differences, cases, controls,
};
const encoded = serialize(record);
if (mode === "verify") assert.deepEqual(encoded, readFileSync(OUTPUT));
else writeFileSync(OUTPUT, encoded, { flag: "wx" });
console.log(`${mode}: 36 cells / 72 vectors + 12 uncollected controls; ${cases.filter(r => r.reference.admitted).length} admitted; ${differences.length} vectors with hypothesis differences; sha256:${sha(encoded)}`);
console.log(`timing: collection ${(performance.now() - started).toFixed(0)}ms; max capture ${Math.max(...timings).toFixed(1)}ms; ${timings.length} captures, all <30000ms`);
