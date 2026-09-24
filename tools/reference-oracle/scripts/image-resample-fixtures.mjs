// Canvas image resampling variants (#198): seeded project-authored PNG images
// drawn by the pinned reference's canvas at other than their natural size.
// Every row is a scene of one or more image elements over a transparent
// export with no padding; the renderer's PNG must equal the reference's
// exactly. The rows cover strong and mild minification, magnification,
// anisotropic scales, fractional sizes and offsets, crops, opacity, and one
// image drawn at several sizes in one scene, whose order matters because the
// reference reuses the pre-scaled image of an earlier draw.
import assert from "node:assert/strict";
import { deflateSync, crc32 } from "node:zlib";

function png(width, height, rgba) {
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type), data]);
    const size = Buffer.alloc(4); size.writeUInt32BE(data.length);
    const crc = Buffer.alloc(4); crc.writeUInt32BE(crc32(body));
    return Buffer.concat([size, body, crc]);
  };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(width, 0); ihdr.writeUInt32BE(height, 4); ihdr.set([8, 6, 0, 0, 0], 8);
  const raw = Buffer.alloc(height * (1 + width * 4));
  for (let y = 0; y < height; y++) raw.set(rgba.subarray(y * width * 4, (y + 1) * width * 4), y * (1 + width * 4) + 1);
  return Buffer.concat([Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]), chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}

/** A 32-bit xorshift stream from `seed`. */
function random(seed) {
  let s = seed >>> 0 || 1;
  return () => { s ^= s << 13; s >>>= 0; s ^= s >>> 17; s ^= s << 5; s >>>= 0; return s & 255; };
}

function image(width, height, pixel) {
  const rgba = Buffer.alloc(width * height * 4);
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) rgba.set(pixel(x, y), (y * width + x) * 4);
  return { width, height, png: png(width, height, rgba) };
}

function images() {
  const noise = (w, h, seed, alpha) => { const r = random(seed); return image(w, h, () => [r(), r(), r(), alpha ? r() : 255]); };
  return {
    // Every pixel independent: any sampling difference shows.
    noise25: noise(25, 25, 25, false),
    noise25a: noise(25, 25, 2525, true),
    noise64: noise(64, 64, 64, false),
    noise100x37: noise(100, 37, 10037, false),
    noise257x193a: noise(257, 193, 257193, true),
    // Sharp edges, impulses, odd and thin extents.
    edges257x193: image(257, 193, (x, y) => ((x >> 3) + (y >> 3)) % 2 ? [240, 30, 60, 255] : [20, 200, 90, 255]),
    impulses200x90: image(200, 90, (x, y) => (x % 7 === 0 && y % 5 === 0) ? [255, 255, 255, 255] : [0, 0, 40, 255]),
    odd37x23: image(37, 23, (x, y) => [x * 7 % 256, y * 11 % 256, (x * y) % 256, 255]),
    thin64x3: image(64, 3, (x, y) => [x * 4, y * 120, 255 - x * 4, 255]),
    deep128x5: image(128, 5, (x, y) => [x * 2, y * 60, 128, 255]),
    // Alpha: a diagonal gradient with transparent corners.
    alpha64: image(64, 64, (x, y) => [x * 4, y * 4, 200, Math.min(255, (x + y) * 2)]),
  };
}

export function imageResampleFixtures() {
  const all = images();
  const rows = [];
  const add = (id, group, key, elements) => {
    const { width, height } = all[key];
    const right = Math.max(...elements.map(e => e.x + e.width)), bottom = Math.max(...elements.map(e => e.y + e.height));
    rows.push({ id, group, image: key, natural: [width, height], elements, extent: [right, bottom] });
  };
  const at = (width, height, extra = {}) => [{ x: 0, y: 0, width, height, ...extra }];
  // Minification sweeps, down to a few pixels.
  for (const [w, h] of [[24, 24], [22, 22], [15, 15], [13, 13], [12, 12], [10, 10], [9, 7], [6, 6], [3, 3], [1, 1]]) add(`noise25-${w}x${h}`, "minify", "noise25", at(w, h));
  for (const [w, h] of [[20, 20], [15, 15], [11, 11], [9, 9], [6, 6]]) add(`noise25a-${w}x${h}`, "minify", "noise25a", at(w, h));
  for (const [w, h] of [[38, 38], [33, 33], [17, 17], [15, 15], [13, 13], [10, 10], [6, 6]]) add(`noise64-${w}x${h}`, "minify", "noise64", at(w, h));
  for (const [w, h] of [[60, 22], [50, 18], [49, 18], [25, 9]]) add(`noise100x37-${w}x${h}`, "minify", "noise100x37", at(w, h));
  for (const [w, h] of [[231, 174], [141, 106], [128, 96], [116, 87], [103, 77], [62, 46], [18, 14]]) add(`noise257x193a-${w}x${h}`, "minify", "noise257x193a", at(w, h));
  for (const [w, h] of [[242, 181], [188, 141], [157, 118], [134, 100], [103, 77], [64, 48], [33, 25], [26, 19]]) add(`edges257x193-${w}x${h}`, "minify", "edges257x193", at(w, h));
  for (const [w, h] of [[100, 45], [50, 23], [23, 10], [7, 3]]) add(`impulses200x90-${w}x${h}`, "minify", "impulses200x90", at(w, h));
  for (const [w, h] of [[36, 22], [35, 22], [34, 21], [33, 21], [32, 20], [15, 9], [9, 6], [5, 3]]) add(`odd37x23-${w}x${h}`, "minify", "odd37x23", at(w, h));
  // Magnification and natural size.
  add("noise25-25x25", "natural", "noise25", at(25, 25));
  add("noise25-50x50", "magnify", "noise25", at(50, 50));
  add("odd37x23-60x40", "magnify", "odd37x23", at(60, 40));
  // Anisotropic scales.
  add("thin64x3-16x1", "anisotropic", "thin64x3", at(16, 1));
  add("thin64x3-64x12", "anisotropic", "thin64x3", at(64, 12));
  add("deep128x5-5x5", "anisotropic", "deep128x5", at(5, 5));
  add("alpha64-5x40", "anisotropic", "alpha64", at(5, 40));
  add("alpha64-60x8", "anisotropic", "alpha64", at(60, 8));
  add("alpha64-33x16", "anisotropic", "alpha64", at(33, 16));
  // Fractional sizes and offsets.
  add("noise25-12.5x12.5", "fractional", "noise25", at(12.5, 12.5));
  add("noise25-25.4x25.4", "fractional", "noise25", at(25.4, 25.4));
  add("noise64-13.3x9.7", "fractional", "noise64", at(13.3, 9.7));
  add("noise64-offset", "fractional", "noise64", [{ x: 0.37, y: 0.61, width: 20, height: 20 }]);
  // Crops: the pre-scaled image is built from the rounded-out crop.
  add("noise64-crop", "crop", "noise64", at(12, 12, { crop: { x: 8, y: 4, width: 40, height: 40, naturalWidth: 64, naturalHeight: 64 } }));
  add("edges257x193-crop", "crop", "edges257x193", at(30, 20, { crop: { x: 13.5, y: 7.25, width: 150, height: 100, naturalWidth: 257, naturalHeight: 193 } }));
  // Opacity.
  add("noise64-opacity", "opacity", "noise64", at(15, 15, { opacity: 50 }));
  add("alpha64-opacity", "opacity", "alpha64", at(20, 20, { opacity: 30 }));
  // One image drawn at several sizes: the order of draws matters.
  add("noise64-multi-large-first", "multi", "noise64", [
    { x: 0, y: 0, width: 40, height: 40 }, { x: 40, y: 0, width: 17, height: 17 }, { x: 60, y: 0, width: 9, height: 9 }]);
  add("noise64-multi-small-first", "multi", "noise64", [
    { x: 0, y: 0, width: 9, height: 9 }, { x: 10, y: 0, width: 17, height: 17 }, { x: 30, y: 0, width: 40, height: 40 }]);
  add("noise257x193a-multi", "multi", "noise257x193a", [
    { x: 0, y: 0, width: 128, height: 96 }, { x: 130, y: 0, width: 62, height: 46 }, { x: 200, y: 0, width: 18, height: 14 }]);
  assert.equal(new Set(rows.map(r => r.id)).size, rows.length);
  return { images: Object.fromEntries(Object.entries(all).map(([k, v]) => [k, v.png.toString("base64")])), rows };
}

export function sceneFor(row, images) {
  return { type: "excalidraw", version: 2, source: "image-resample", appState: { viewBackgroundColor: "#ffffff" },
    elements: row.elements.map((e, i) => ({ id: `image${i}`, type: "image", x: e.x, y: e.y, width: e.width, height: e.height, angle: 0,
      strokeColor: "transparent", backgroundColor: "transparent", fillStyle: "solid", strokeWidth: 1, strokeStyle: "solid", roughness: 0,
      opacity: e.opacity ?? 100, groupIds: [], frameId: null, roundness: null, seed: 1, version: 1, versionNonce: 1, isDeleted: false,
      boundElements: null, updated: 1, link: null, locked: false, status: "saved", fileId: "asset", scale: [1, 1],
      crop: e.crop ?? null, index: `a${i}` })),
    files: { asset: { id: "asset", mimeType: "image/png", dataURL: `data:image/png;base64,${images[row.image]}`, created: 1 } } };
}
