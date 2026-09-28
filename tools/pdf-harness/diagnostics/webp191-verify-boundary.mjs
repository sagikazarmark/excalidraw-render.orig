// Read-only checks of retained #191 observations; no viewer installation needed.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

const root = fileURLToPath(new URL("../../../", import.meta.url));
const base = resolve(root, "tests/oracle/webp-sample");
const control = resolve(base, "comparison-boundary-controls");
const current = resolve(base, "approved-profile-viewer-diagnosis");
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
const read = (directory, path) => readFileSync(resolve(directory, path));
const json = (directory, path) => JSON.parse(read(directory, path));
const decode = (text) => Buffer.from(text, "base64");

function views(raw) {
  assert.equal(raw.length, 252, "9x7 RGBA expected");
  const alpha = Buffer.alloc(252), black = Buffer.alloc(252), white = Buffer.alloc(252);
  for (let i = 0; i < raw.length; i += 4) {
    const a = raw[i + 3];
    for (let c = 0; c < 3; c++) {
      alpha[i + c] = a;
      black[i + c] = Math.floor((raw[i + c] * a + 127) / 255);
      white[i + c] = Math.floor((raw[i + c] * a + 255 * (255 - a) + 127) / 255);
    }
    alpha[i + 3] = black[i + 3] = white[i + 3] = 255;
  }
  return [alpha, black, white];
}

const inputs = json(base, "inputs.json");
function independentlyExpected(source) {
  const frame = inputs.frames.find((row) => row.id === source.first_frame_id);
  assert.ok(frame, "first frame is present");
  const decoded = decode(frame.native_decoded_rgba.base64);
  const [width, height, x, y] = source.config.shape === "subrect" ? [5, 3, 2, 2] : [9, 7, 0, 0];
  assert.equal(decoded.length, width * height * 4);
  const out = Buffer.alloc(252);
  for (let j = 0; j < height; j++) {
    for (let i = 0; i < width; i++) {
      const offset = (j * width + i) * 4;
      if (decoded[offset + 3]) decoded.copy(out, ((y + j) * 9 + x + i) * 4, offset, offset + 4);
    }
  }
  return out;
}

function gate(candidate, independent, source, viewer, measurement) {
  assert.deepEqual(candidate, independent, "candidate must equal independent PDF viewer output exactly");
  const straight = independentlyExpected(source);
  assert.equal(candidate.length, straight.length);
  for (let i = 0; i < candidate.length; i += 4) {
    assert.ok(candidate[i + 3] || candidate.subarray(i, i + 3).equals(Buffer.alloc(3)), "invisible RGB must be zero");
    assert.equal(candidate[i + 3], straight[i + 3], "alpha must equal independently authored alpha");
  }
  if (viewer === "pdfium") assert.deepEqual(candidate, straight, "PDFium must preserve external straight samples");
  const browser = decode(source.reference.samples.intrinsic.rgba_base64);
  const [a, b, w] = views(candidate);
  const [ra, rb, rw] = views(browser);
  assert.deepEqual(a, ra, "alpha view mismatch");
  assert.equal(b.equals(rb) && w.equals(rw), source.alpha !== "partial", "canonical outcome changed");
  if (source.alpha === "partial") {
    assert.deepEqual([b, w], views(straight).slice(1), "unexplained quantization mechanism");
    assert.ok(!b.equals(rb) && !w.equals(rw), "residual must remain an actual failure");
  }
  assert.equal(sha(candidate), measurement.actual_sha256, "observed viewer hash moved");
}

assert.equal(sha(read(control, "measurements.json")), "95c1d9da1075dc08206fb2c1edb25795baddea40abfc72b712307f26e08542c2", "control record changed");
assert.equal(sha(read(current, "measurements.json")), "6d47db1399e8c857b8f402d7604d21bd0ed8474f4422fa2128bcab6a40833879", "viewer record changed");
const record = json(control, "measurements.json");
for (const [directory, manifest] of [[control, record], [current, json(current, "measurements.json")]]) {
  for (const [path, digest] of Object.entries(manifest.files)) assert.equal(sha(read(directory, path)), digest, path);
}
// Only completed producers are historical. Every input/profile/output remains live.
const producers = new Set([
  "tools/pdf-harness/pdfium/render.py",
  "tools/pdf-harness/pdfium/test_alpha.py",
  "tools/pdf-harness/pdfjs/render.mjs",
  "tools/pdf-harness/run-pdfium.sh",
  "tools/pdf-harness/run-pdfjs.sh",
  "tools/pdf-harness/diagnostics/webp191-comparison-boundary.py",
]);
for (const d of record.sources) {
  if (!producers.has(d.path)) assert.equal(sha(read(root, d.path)), d.sha256, d.path);
}
for (const [path, key] of [["reference.json", "reference_sha256"], ["inputs.json", "inputs_sha256"]]) {
  assert.equal(sha(read(base, path)), record[key], path);
}
const sources = new Map(json(base, "reference.json").cases.map((row) => [row.id, row]));
assert.equal(sources.size, 72);
assert.equal(record.cases.length, 72);
assert.equal(new Set(record.cases.map((row) => row.id)).size, 72);
let rejected = 0, paired = 0, comparisons = 0, partial = 0;
for (const row of record.cases) {
  const source = sources.get(row.id);
  assert.ok(source, `unknown case ${row.id}`);
  for (const viewer of ["pdfium", "pdfjs"]) {
    const candidate = read(current, `${row.id}/${viewer}-0.rgba`);
    const independent = read(control, `${row.id}/${viewer}-0.rgba`);
    const observation = row.viewers[viewer];
    gate(candidate, independent, source, viewer, observation);
    comparisons++;
    const color = Buffer.from(candidate), alpha = Buffer.from(candidate);
    color[0] ^= 1;
    alpha[3] ^= 1;
    const bads = [color, alpha];
    if (source.alpha === "partial") {
      bads.push(decode(source.reference.samples.intrinsic.rgba_base64));
      partial++;
    }
    for (const bad of bads) {
      assert.throws(() => gate(bad, independent, source, viewer, observation), assert.AssertionError);
      rejected++;
      // Rebinding both buffers and the hash must not bypass semantic constraints.
      assert.throws(() => gate(bad, bad, source, viewer, { ...observation, actual_sha256: sha(bad) }), assert.AssertionError);
      paired++;
    }
  }
}
assert.deepEqual([comparisons, rejected, paired, partial], [144, 336, 336, 48]);
console.log(`${comparisons} independent viewer comparisons verified; ${rejected} independent-buffer and ${paired} paired semantic mutations rejected; ${partial} partial viewer judgments remain canonical failures.`);
