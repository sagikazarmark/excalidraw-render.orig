// Exact sample evidence, collected through the immutable official export wrapper.
// The source samples and PDF straight-color expectations are authored here;
// the PNG expectations come only from pinned Chromium's official direct export.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { deflateSync } from "node:zlib";
import { chromium } from "playwright";

const ORACLE = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const ROOT = resolve(ORACLE, "../..");
const INPUT = "tests/oracle/png-precision/inputs.json";
const OUTPUT = resolve(ROOT, "tests/oracle/png-precision/reference.json");
const PROFILE = "tests/oracle/reference-profiles/revision-1.json";
const profile = JSON.parse(readFileSync(resolve(ROOT, PROFILE)));
const input = JSON.parse(readFileSync(resolve(ROOT, INPUT)));
assert.equal(input.schema, "excalidraw-render/png-precision-inputs/v1");
assert.equal(process.env.REFERENCE_ORACLE_CANONICAL, "1");
assert.equal(process.version, "v24.16.0");
assert.ok(["generate", "verify"].includes(process.argv[2]));

function sha256(bytes) { return createHash("sha256").update(bytes).digest("hex"); }
function descriptor(path) {
  const bytes = readFileSync(resolve(ROOT, path));
  return { path, sha256: sha256(bytes) };
}
function chunk(type, data) {
  const body = Buffer.concat([Buffer.from(type), data]);
  let crc = 0xffffffff;
  for (const byte of body) {
    crc ^= byte;
    for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
  }
  const length = Buffer.alloc(4); length.writeUInt32BE(data.length);
  const end = Buffer.alloc(4); end.writeUInt32BE((crc ^ 0xffffffff) >>> 0);
  return Buffer.concat([length, body, end]);
}
function png(vector) {
  const { width, height, depth, color_type: color, samples, transparency_key: key, profile } = vector;
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width); header.writeUInt32BE(height, 4);
  header[8] = depth; header[9] = color;
  const channels = { 0: 1, 2: 3, 4: 2, 6: 4 }[color];
  assert.equal(samples.length, width * height);
  const stride = width * channels * (depth / 8);
  const scanlines = Buffer.alloc(height * (stride + 1));
  samples.forEach((pixel, p) => {
    assert.equal(pixel.length, channels);
    const start = Math.floor(p / width) * (stride + 1) + 1 + (p % width) * channels * (depth / 8);
    pixel.forEach((v, c) => depth === 16
      ? scanlines.writeUInt16BE(v, start + c * 2)
      : scanlines.writeUInt8(v, start + c));
  });
  const chunks = [chunk("IHDR", header)];
  if (profile === "srgb") chunks.push(chunk("sRGB", Buffer.from([0])));
  if (profile === "linear") {
    const gamma = Buffer.alloc(4); gamma.writeUInt32BE(100000);
    chunks.push(chunk("gAMA", gamma));
  }
  if (key) {
    const data = Buffer.alloc(key.length * 2);
    key.forEach((v, i) => data.writeUInt16BE(v, i * 2));
    chunks.push(chunk("tRNS", data));
  }
  return Buffer.concat([Buffer.from("89504e470d0a1a0a", "hex"), ...chunks,
    chunk("IDAT", deflateSync(scanlines)), chunk("IEND", Buffer.alloc(0))]);
}
function straight(vector) {
  const reduce = value => vector.depth === 16 ? Math.floor(value / 256) : value;
  const transfer = value => vector.profile === "linear"
    ? Math.round(255 * (value / 255 <= 0.0031308 ? 12.92 * value / 255 : 1.055 * (value / 255) ** (1 / 2.4) - 0.055))
    : value;
  return Buffer.from(vector.samples.flatMap(pixel => {
    const gray = vector.color_type === 0 || vector.color_type === 4;
    const color = gray ? [pixel[0], pixel[0], pixel[0]] : pixel.slice(0, 3);
    const alpha = vector.color_type === 4 || vector.color_type === 6 ? reduce(pixel.at(-1))
      : vector.transparency_key?.every((value, c) => value === pixel[c]) ? 0 : 255;
    return [...(alpha === 0 ? [0, 0, 0] : color.map(reduce).map(transfer)), alpha];
  }));
}
const vectors = [];
function add(id, obligation, color_type, depth, samples, profile = "untagged", transparency_key = null, width = samples.length) {
  const vector = { id, obligation, color_type, depth, width, height: samples.length / width, profile, transparency_key, samples };
  vectors.push(vector);
  return vector;
}
add("gray16-all-values", "precision", 0, 16, Array.from({ length: 65536 }, (_, v) => [v]), "untagged", null, 256);
for (const profile of input.profiles) {
  for (const color of [0, 2, 4, 6]) {
    const samples = input.samples16.map(v => {
      if (color === 0) return [v];
      if (color === 2) return [v, 65535 - v, 0x1234];
      if (color === 4) return [0x5678, v];
      return [0x1234, 0x5678, 0x9abc, v];
    });
    add(`type${color}-16-${profile}`, "precision", color, 16, samples, profile);
  }
}
for (const key of [0, 0x1234, 0xff00, 65535]) {
  const gray = [Math.max(key - 1, 0), key, Math.min(key + 1, 65535), key & 0xff00, key | 255, ((key & 255) << 8) | (key >> 8)];
  add(`gray16-key-${key}`, "transparency-key", 0, 16, gray.map(v => [v]), "untagged", [key]);
  const rgbKey = [key, 0x5678, 0x9abc];
  const rgb = [rgbKey, ...[0, 1, 2].flatMap(channel => [-1, 1].map(delta => rgbKey.map((v, c) => c === channel ? Math.max(0, Math.min(65535, v + delta)) : v))), [key ^ 1, 0x5678 ^ 1, 0x9abc ^ 1]];
  add(`rgb16-key-${key}`, "transparency-key", 2, 16, rgb, "untagged", rgbKey);
}
add("rgba8-all-color-alpha", "alpha", 6, 8,
  Array.from({ length: 65536 }, (_, i) => [i % 256, 255 - i % 256, (i * 73) % 256, Math.floor(i / 256)]), "untagged", null, 256);
add("gray-alpha8-boundaries", "alpha", 4, 8, input.colors8.flat().flatMap(g => input.alpha8.map(a => [g, a])));
add("rgba8-boundaries", "alpha", 6, 8, input.colors8.flatMap(rgb => input.alpha8.map(a => [...rgb, a])));

const cases = vectors.flatMap(vector => {
  const backgrounds = vector.obligation === "alpha" ? input.backgrounds : [null];
  return backgrounds.map(background => {
    const source = png(vector);
    const dataURL = `data:image/png;base64,${source.toString("base64")}`;
    const scene = { type: "excalidraw", version: 2, elements: [{
      id: "image", type: "image", x: 0, y: 0, width: vector.width, height: vector.height,
      angle: 0, strokeColor: "#000000", backgroundColor: "transparent", fillStyle: "solid",
      strokeWidth: 1, strokeStyle: "solid", roughness: 0, opacity: 100, groupIds: [],
      frameId: null, index: "a0", seed: 1, version: 1, versionNonce: 1, isDeleted: false,
      boundElements: null, updated: 1, link: null, locked: false, fileId: "asset",
      status: "saved", scale: [1, 1], crop: null, roundness: null,
    }], appState: { viewBackgroundColor: background ?? "#ffffff" }, files: {
      asset: { id: "asset", mimeType: "image/png", dataURL, created: 1 },
    } };
    const options = {
      exportBackground: background !== null, exportEmbedScene: false, exportPadding: 0,
      exportScale: 1, exportWithDarkMode: false,
      frameRendering: { enabled: true, clip: true, name: true, outline: true },
      maxWidthOrHeight: null, viewBackgroundColor: background ?? "#ffffff",
    };
    return { id: `${vector.id}-${background ?? "transparent"}`, obligation: vector.obligation,
      width: vector.width, height: vector.height, scene, options,
      source_sha256: sha256(source), straight_rgba: straight(vector).toString("base64") };
  });
});

async function collect(order) {
  const browser = await chromium.launch({
    args: profile.browser.launch.explicitArguments,
    chromiumSandbox: false, headless: true,
  });
  try {
    assert.equal(browser.version(), "149.0.7827.55");
    const result = {};
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
        result[row.id] = await page.evaluate(async ({ scene, options }) => window.referenceOracle.render(scene, options), row);
        assert.deepEqual(errors, []);
      } finally { await context.close(); }
    }
    return result;
  } finally { await browser.close(); }
}
const first = await collect(cases);
const second = await collect([...cases].reverse());
for (const row of cases) assert.deepEqual(first[row.id], second[row.id], `${row.id}: fresh-process/order mismatch`);
const record = {
  schema: "excalidraw-render/png-precision-reference/v1",
  sources: [INPUT, PROFILE, "tools/reference-oracle/scripts/png-precision-evidence.mjs", "tools/reference-oracle/run-png-precision.sh"].map(descriptor),
  collection: "Official v0.18.1 direct PNG and SVG; pinned Chromium 149.0.7827.55; two fresh browser processes in forward/reverse order; fresh context per case; network none.",
  cases: cases.map(row => ({ ...row, png_base64: first[row.id].pngBase64, svg_sha256: sha256(Buffer.from(first[row.id].svg)) })),
};
const bytes = Buffer.from(JSON.stringify(record, null, 2) + "\n");
if (process.argv[2] === "generate") writeFileSync(OUTPUT, bytes, { flag: "wx" });
else assert.deepEqual(bytes, readFileSync(OUTPUT));
console.log(`Verified ${cases.length} PNG precision/alpha reference cases across two fresh browser processes; sha256:${sha256(bytes)}`);
