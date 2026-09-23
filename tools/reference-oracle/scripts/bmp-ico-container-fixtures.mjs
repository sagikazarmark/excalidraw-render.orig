// BMP and ICO header, palette, mask, alpha, RLE, color-space, directory and
// entry variants (#85 BMP and ICO facets). Every row is built here from one
// 4x3 base picture, and the ICC profiles are the frozen PNG color vectors
// (tests/oracle/png-color/reference.json). Each row declares its expectation:
//   match     - the renderer must equal the reference exactly (or within an
//               approved exact-ceiling exception, see bmp-ico-container);
//   partial   - the reference loads the file but paints only the pixels it
//               could decode, possibly none; the renderer rejects (the
//               declared divergence, #85);
//   rejected  - the reference rejects the file;
//   divergent - the reference paints a PNG icon entry that needs a color
//               transform fully transparent (a reference defect); the
//               renderer color-manages it as the same PNG standalone.
import assert from "node:assert/strict";
import { deflateSync, inflateSync, crc32 } from "node:zlib";

const W = 4, H = 3;
const COL = [[[239, 31, 73], [19, 211, 97], [43, 67, 229], [251, 193, 17]],
  [[10, 20, 30], [200, 100, 50], [90, 180, 250], [255, 255, 255]],
  [[0, 0, 0], [128, 128, 128], [60, 0, 120], [250, 5, 5]]];
const RED = [[250, 5, 5]], BLUE = [[5, 5, 250]];

const bytes = (...parts) => Buffer.concat(parts.map(p => Buffer.isBuffer(p) ? p : Buffer.from(p)));
const u16 = v => { const b = Buffer.alloc(2); b.writeUInt16LE(v); return b; };
const u32 = v => { const b = Buffer.alloc(4); b.writeUInt32LE(v >>> 0); return b; };
const i32 = v => { const b = Buffer.alloc(4); b.writeInt32LE(v); return b; };
const be32 = v => { const b = Buffer.alloc(4); b.writeUInt32BE(v >>> 0); return b; };
const pad = b => Buffer.concat([b, Buffer.alloc((4 - (b.length % 4)) % 4)]);
const bottomUp = (topDown = false) => (topDown ? [0, 1, 2] : [2, 1, 0]).map(y => COL[y]);

function fileHeader(total, offset, sizeField = total, reserved = 0) {
  return bytes("BM", u32(sizeField), u32(reserved), u32(offset));
}
function info({ hsize = 40, w = W, h = H, bpp = 24, comp = 0, clr = 0, masks = null, cstype = 0x73524742, endpoints = null, gammas = null, profile = [0, 0] } = {}) {
  let b = bytes(u32(hsize), i32(w), i32(h), u16(1), u16(bpp), u32(comp), u32(0), i32(2835), i32(2835), u32(clr), u32(0));
  const m = masks ?? [0, 0, 0, 0];
  if (hsize >= 52) b = bytes(b, u32(m[0]), u32(m[1]), u32(m[2]));
  if (hsize >= 56) b = bytes(b, u32(m[3] ?? 0));
  if (hsize >= 108) b = bytes(b, u32(cstype), ...(endpoints ?? Array(9).fill(0)).map(i32), ...(gammas ?? [0, 0, 0]).map(u32));
  if (hsize >= 124) b = bytes(b, u32(4), u32(profile[0]), u32(profile[1]), u32(0));
  return b.length >= hsize ? b.subarray(0, hsize) : bytes(b, Buffer.alloc(hsize - b.length));
}
const px24 = (topDown = false) => Buffer.concat(bottomUp(topDown).map(r => pad(Buffer.from(r.flatMap(c => [c[2], c[1], c[0]])))));
function bmp(header, { palette = Buffer.alloc(0), data = Buffer.alloc(0), sizeField, reserved = 0 } = {}) {
  const offset = 14 + header.length + palette.length;
  return bytes(fileHeader(offset + data.length, offset, sizeField ?? offset + data.length, reserved), header, palette, data);
}
const PAL = Buffer.from(COL.flat().flatMap(c => [c[2], c[1], c[0], 0]));
const IDX = [0, 1, 2].map(y => [0, 1, 2, 3].map(x => y * 4 + x));
const px8 = rows => Buffer.concat([...rows].reverse().map(r => pad(Buffer.from(r))));
const px32 = alpha => Buffer.from(bottomUp().flatMap((r, y) => r.flatMap((c, x) => [c[2], c[1], c[0], alpha(x, y)])));
const px16 = f => Buffer.concat(bottomUp().map(r => pad(Buffer.concat(r.map(c => u16(f(c)))))));
const BF = [0x00ff0000, 0x0000ff00, 0x000000ff, 0xff000000];
const fx = v => Math.round(v * 2 ** 30);
const SRGB_ENDPOINTS = [0.4361, 0.2225, 0.0139, 0.3851, 0.7169, 0.0971, 0.1431, 0.0606, 0.7141].map(fx);
const G22 = Math.trunc(2.2 * 65536);

function chunk(type, data) {
  const body = bytes(type, data);
  return bytes(be32(data.length), body, be32(crc32(body)));
}
function png(w, h, rgba, extra = Buffer.alloc(0)) {
  const raw = Buffer.concat(Array.from({ length: h }, (_, y) => bytes([0], rgba.subarray(y * w * 4, (y + 1) * w * 4))));
  const ihdr = bytes(be32(w), be32(h), [8, 6, 0, 0, 0]);
  return bytes([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a], chunk("IHDR", ihdr), extra, chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0)));
}
const PNG_RGBA = Buffer.from(COL.flat().flatMap(c => [...c, 255]));

/** An icon directory; entries are [width, height, colors, reserved, planes,
 *  depth, payload, declared size]. */
function ico(entries, type = 1) {
  let offset = 6 + 16 * entries.length;
  const directory = [u16(0), u16(type), u16(entries.length)], bodies = [];
  for (const [w, h, colors, reserved, planes, depth, payload, size] of entries) {
    directory.push(Buffer.from([w & 255, h & 255, colors, reserved]), u16(planes), u16(depth), u32(size ?? payload.length), u32(offset));
    bodies.push(payload); offset += payload.length;
  }
  return bytes(...directory, ...bodies);
}
/** A DIB icon entry of `colors` repeated diagonally, with its AND mask. */
function dib(w, h, colors, { bpp = 32, alpha = () => 255, andByte = () => 0, heightField = 2 * h, palette = null } = {}) {
  const rows = Array.from({ length: h }, (_, y) => Array.from({ length: w }, (_, x) => colors ? colors[(x + y) % colors.length] : null));
  const header = info({ w, h: heightField, bpp, clr: palette ? palette.length / 4 : 0 });
  let pixels;
  if (bpp === 32) pixels = Buffer.from([...rows].reverse().flatMap((r, yy) => r.flatMap((c, x) => [c[2], c[1], c[0], alpha(x, h - 1 - yy)])));
  else if (bpp === 24) pixels = Buffer.concat([...rows].reverse().map(r => pad(Buffer.from(r.flatMap(c => [c[2], c[1], c[0]])))));
  else pixels = Buffer.concat([...rows].reverse().map((r, yy) => pad(Buffer.from(packIndices(r.map((_, x) => (x + h - 1 - yy) % (palette.length / 4)), bpp)))));
  const mask = Buffer.concat(Array.from({ length: h }, (_, y) => pad(Buffer.alloc(Math.ceil(w / 8), andByte(h - 1 - y)))));
  return bytes(header, palette ?? Buffer.alloc(0), pixels, mask);
}
function packIndices(indices, bpp) {
  const out = []; let acc = 0, bits = 0;
  for (const i of indices) { acc = (acc << bpp) | i; bits += bpp; if (bits === 8) { out.push(acc); acc = 0; bits = 0; } }
  if (bits) out.push(acc << (8 - bits));
  return out;
}

function iccFromPng(pngBytes) {
  for (let at = 8; at < pngBytes.length;) {
    const size = pngBytes.readUInt32BE(at);
    if (pngBytes.toString("latin1", at + 4, at + 8) === "iCCP") { const d = pngBytes.subarray(at + 8, at + 8 + size); return inflateSync(d.subarray(d.indexOf(0) + 2)); }
    at += 12 + size;
  }
  throw Error("no iCCP");
}

export function bmpIcoContainerFixtures(color) {
  const icc = signal => iccFromPng(Buffer.from(color.cases.find(c => c.signal === signal).png, "base64"));
  const p3 = icc("icc-v2-p3"), linear = icc("icc-v2-gamma1"), srgbV4 = icc("icc-v4-srgb");
  const rows = [];
  const add = (id, group, expectation, data, size = [W, H]) =>
    rows.push({ id, group, expectation, mime: id.startsWith("ico-") ? "image/x-icon" : "image/bmp", payload: data, size });
  const base = bmp(info(), { data: px24() });

  // BMP bases and depths.
  add("bmp-base", "base", "match", base);
  add("bmp-top-down", "orientation", "match", bmp(info({ h: -H }), { data: px24(true) }));
  add("bmp-8bpp", "base", "match", bmp(info({ bpp: 8, clr: 12 }), { palette: PAL, data: px8(IDX) }));
  add("bmp-4bpp", "base", "match", bmp(info({ bpp: 4, clr: 12 }), { palette: PAL, data: Buffer.concat([...IDX].reverse().map(r => pad(Buffer.from(packIndices(r, 4))))) }));
  add("bmp-2bpp", "base", "match", bmp(info({ bpp: 2, clr: 4 }), { palette: PAL.subarray(0, 16), data: Buffer.concat(Array(H).fill(pad(Buffer.from([0b00011011])))) }));
  add("bmp-1bpp", "base", "match", bmp(info({ bpp: 1, clr: 2 }), { palette: PAL.subarray(0, 8), data: Buffer.concat(Array(H).fill(pad(Buffer.from([0b10100000])))) }));
  add("bmp-rgb16-555", "base", "match", bmp(info({ bpp: 16 }), { data: px16(c => ((c[0] >> 3) << 10) | ((c[1] >> 3) << 5) | (c[2] >> 3)) }));
  add("bmp-rgb32", "base", "match", bmp(info({ bpp: 32 }), { data: px32(() => 0) }));
  // Headers.
  add("bmp-size-mismatch", "container", "match", bmp(info(), { data: px24(), sizeField: 12345 }));
  add("bmp-reserved-nonzero", "container", "match", bmp(info(), { data: px24(), reserved: 0x01020304 }));
  add("bmp-trailing", "container", "match", bytes(base, Buffer.alloc(16)));
  add("bmp-core-24", "container", "match", bmp(bytes(u32(12), u16(W), u16(H), u16(1), u16(24)), { data: px24() }));
  add("bmp-os2v2-16", "container", "match", bmp(bytes(u32(16), i32(W), i32(H), u16(1), u16(24)), { data: px24() }));
  add("bmp-os2v2-64", "container", "match", bmp(info({ hsize: 64 }), { data: px24() }));
  add("bmp-v3-52", "container", "match", bmp(info({ hsize: 52 }), { data: px24() }));
  add("bmp-v5", "container", "match", bmp(info({ hsize: 124 }), { data: px24() }));
  const inPalette = bmp(info({ bpp: 8, clr: 12 }), { palette: PAL, data: px8(IDX) });
  const offset = 14 + 40 + 16;
  add("bmp-offset-in-palette", "container", "match", bytes(inPalette.subarray(0, 10), u32(offset), inPalette.subarray(14, offset), px8(IDX)));
  add("bmp-ba-array", "container", "rejected", bytes("BA", u32(14 + base.length), u32(0), u32(0), base));
  add("bmp-w65536", "container", "rejected", bmp(info({ w: 65536, h: 1 }), { data: pad(Buffer.alloc(3 * 65536)) }));
  add("bmp-core-32", "container", "rejected", bmp(bytes(u32(12), u16(W), u16(H), u16(1), u16(32)), { data: px32(() => 0) }));
  add("bmp-bad-depth", "container", "rejected", bmp(info({ bpp: 7 }), { data: px24() }));
  add("bmp-truncated-header", "container", "rejected", base.subarray(0, 30));
  // Palettes.
  add("bmp-idx-oob", "palette", "match", bmp(info({ bpp: 8, clr: 2 }), { palette: PAL.subarray(0, 8), data: px8(IDX) }));
  add("bmp-clrused-over", "palette", "match", bmp(info({ bpp: 1, clr: 300 }), { palette: PAL.subarray(0, 8), data: Buffer.concat(Array(H).fill(pad(Buffer.from([0b10100000])))) }));
  add("bmp-clrused-zero-8bpp", "palette", "match", bmp(info({ bpp: 8 }), { palette: Buffer.concat([PAL, Buffer.alloc(1024 - PAL.length)]), data: px8(IDX) }));
  // Masks and alpha.
  add("bmp-rgb32-alpha-ignored", "alpha", "match", bmp(info({ bpp: 32 }), { data: px32(() => 64) }));
  add("bmp-bf32-allzero-alpha", "alpha", "match", bmp(info({ hsize: 108, bpp: 32, comp: 3, masks: BF }), { data: px32(() => 0) }));
  add("bmp-bf32-mixed-alpha", "alpha", "match", bmp(info({ hsize: 108, bpp: 32, comp: 3, masks: BF }), { data: px32((x, y) => (y === 0 ? 0 : 128)) }));
  add("bmp-bf32-partial-alpha", "alpha", "match", bmp(info({ hsize: 108, bpp: 32, comp: 3, masks: BF }), { data: px32((x, y) => [0, 1, 128, 255][x]) }));
  add("bmp-v4-rgb32-alpha", "alpha", "match", bmp(info({ hsize: 108, bpp: 32, masks: [0x000000ff, 0x0000ff00, 0x00ff0000, 0xff000000] }), { data: px32(() => 128) }));
  add("bmp-v4-rgb32-zero-rgbmask", "alpha", "match", bmp(info({ hsize: 108, bpp: 32, masks: [0, 0, 0, 0xff000000] }), { data: px32(() => 128) }));
  add("bmp-v3-rgb16-alpha8000", "alpha", "match", bmp(info({ hsize: 56, bpp: 16, masks: [0, 0, 0, 0x8000] }), { data: px16(c => 0x8000 | ((c[0] >> 3) << 10) | ((c[1] >> 3) << 5) | (c[2] >> 3)) }));
  add("bmp-abf-52", "alpha", "match", bmp(info({ hsize: 52, bpp: 32, comp: 6, masks: BF }), { data: px32(() => 128) }));
  add("bmp-abf-56", "alpha", "match", bmp(info({ hsize: 56, bpp: 32, comp: 6, masks: BF }), { data: px32(() => 128) }));
  add("bmp-bf32-10bit", "masks", "match", bmp(info({ hsize: 108, bpp: 32, comp: 3, masks: [0x3ff00000, 0x000ffc00, 0x000003ff, 0] }),
    { data: Buffer.concat(bottomUp().flatMap(r => r.map(c => u32(((c[0] * 4 + 3) << 20) | ((c[1] * 4 + 1) << 10) | (c[2] * 4 + 2))))) }));
  add("bmp-bf16-565", "masks", "match", bmp(info({ hsize: 108, bpp: 16, comp: 3, masks: [0xf800, 0x07e0, 0x001f, 0] }), { data: px16(c => ((c[0] >> 3) << 11) | ((c[1] >> 2) << 5) | (c[2] >> 3)) }));
  add("bmp-bf16-v3-565", "masks", "match", bytes(bmp(bytes(info({ bpp: 16, comp: 3 })), { palette: bytes(u32(0xf800), u32(0x07e0), u32(0x001f)), data: px16(c => ((c[0] >> 3) << 11) | ((c[1] >> 2) << 5) | (c[2] >> 3)) })));
  add("bmp-bf16-zero-green", "masks", "match", bmp(info({ hsize: 108, bpp: 16, comp: 3, masks: [0xf800, 0, 0x001f, 0] }), { data: px16(c => ((c[0] >> 3) << 11) | (c[2] >> 3)) }));
  add("bmp-bf16-mask-out-of-word", "masks", "match", bmp(info({ hsize: 108, bpp: 16, comp: 3, masks: [0xf800f800, 0x07e0, 0x001f, 0] }), { data: px16(c => ((c[0] >> 3) << 11) | ((c[1] >> 2) << 5) | (c[2] >> 3)) }));
  add("bmp-bf32-overlapping", "masks", "partial", bmp(info({ hsize: 108, bpp: 32, comp: 3, masks: [0x00ff0000, 0x00ffff00, 0x000000ff, 0] }), { data: px32(() => 0) }));
  add("bmp-bf32-noncontiguous", "masks", "partial", bmp(info({ hsize: 108, bpp: 32, comp: 3, masks: [0x00f0f000, 0x0000000f, 0x0f000000, 0] }), { data: px32(() => 0) }));
  // Run-length encoding.
  const full = []; for (let y = 0; y < H; y++) full.push(0, 4, ...IDX[H - 1 - y], 0, 0);
  const rle8 = (ops, clr = 12, h = H) => bmp(info({ bpp: 8, comp: 1, clr, h }), { palette: PAL.subarray(0, 4 * clr), data: Buffer.from(ops) });
  add("bmp-rle8", "rle", "match", rle8([...full, 0, 1]));
  add("bmp-rle8-delta-hole", "rle", "match", rle8([2, 1, 0, 2, 2, 1, 0, 0, 0, 4, ...IDX[1], 0, 0, 0, 4, ...IDX[0], 0, 0, 0, 1]));
  add("bmp-rle8-early-eob", "rle", "match", rle8([0, 4, ...IDX[2], 0, 0, 0, 1]));
  add("bmp-rle8-no-eob", "rle", "match", rle8(full));
  add("bmp-rle8-run-overrun", "rle", "match", rle8([6, 1, 0, 0, 0, 4, ...IDX[1], 0, 0, 0, 4, ...IDX[0], 0, 0, 0, 1]));
  add("bmp-rle8-topdown", "orientation", "match", rle8([...full, 0, 1], 12, -H));
  add("bmp-rle8-idx-oob", "rle", "match", rle8([4, 5, 0, 0, 4, 1, 0, 0, 4, 0, 0, 0, 0, 1], 2));
  add("bmp-rle4", "rle", "match", bmp(info({ bpp: 4, comp: 2, clr: 12 }), { palette: PAL, data: Buffer.from([4, 0x89, 0, 0, 0, 4, 0x45, 0x67, 0, 0, 4, 0x01, 0, 0, 0, 1]) }));
  add("bmp-rle24", "rle", "match", bmp(info({ bpp: 24, comp: 4 }), { data: Buffer.from([4, 5, 6, 7, 0, 0, 4, 50, 60, 70, 0, 0, 4, 90, 100, 110, 0, 1]) }));
  // The declared divergence: pixel data that ends before its last row.
  add("bmp-trunc-last-row", "partial", "partial", base.subarray(0, -6));
  add("bmp-trunc-padding", "partial", "partial", base.subarray(0, -1));
  add("bmp-trunc-8bpp", "partial", "partial", bmp(info({ bpp: 8, clr: 12 }), { palette: PAL, data: px8(IDX).subarray(0, 6) }));
  // Color spaces.
  add("bmp-v4-srgb", "color-profile", "match", bmp(info({ hsize: 108 }), { data: px24() }));
  add("bmp-v4-win", "color-profile", "match", bmp(info({ hsize: 108, cstype: 0x57696e20 }), { data: px24() }));
  add("bmp-v4-unknown-type", "color-profile", "match", bmp(info({ hsize: 108, cstype: 0x41424344 }), { data: px24() }));
  add("bmp-v4-calibrated", "color-profile", "match", bmp(info({ hsize: 108, cstype: 0, endpoints: [0.6, 0.3, 0.02, 0.3, 0.6, 0.1, 0.1, 0.05, 0.7].map(fx), gammas: [G22, G22, G22] }), { data: px24() }));
  add("bmp-v4-calibrated-srgbish", "color-profile", "match", bmp(info({ hsize: 108, cstype: 0, endpoints: SRGB_ENDPOINTS, gammas: [G22, G22, G22] }), { data: px24() }));
  add("bmp-v4-calibrated-zero", "color-profile", "match", bmp(info({ hsize: 108, cstype: 0, gammas: [G22, G22, G22] }), { data: px24() }));
  add("bmp-v4-calibrated-gamma0", "color-profile", "match", bmp(info({ hsize: 108, cstype: 0, endpoints: SRGB_ENDPOINTS }), { data: px24() }));
  const mbed = (profile, at) => {
    const data = px24();
    return bytes(bmp(info({ hsize: 124, cstype: 0x4d424544, profile: [at ?? 124 + data.length, profile.length] }), { data }), profile);
  };
  add("bmp-v5-mbed-p3", "color-profile", "match", mbed(p3));
  add("bmp-v5-mbed-linear", "color-profile", "match", mbed(linear));
  add("bmp-v5-mbed-srgb-v4", "color-profile", "match", mbed(srgbV4));
  add("bmp-v5-mbed-invalid", "color-profile", "rejected", mbed(Buffer.from("not an icc profile at all")));
  add("bmp-v5-mbed-oob", "color-profile", "rejected", mbed(p3, 100000));
  add("bmp-v5-link", "color-profile", "match", bytes(bmp(info({ hsize: 124, cstype: 0x4c494e4b, profile: [124 + px24().length, 10] }), { data: px24() }), Buffer.from("C:\\x.icc\0")));

  // ICO directories and selection.
  const entry = (payload, over = {}) => [over.w ?? 4, over.h ?? 3, 0, over.reserved ?? 0, over.planes ?? 1, over.depth ?? 32, payload, over.size];
  const icoBase = ico([entry(dib(4, 3, COL[0]))]);
  add("ico-base", "base", "match", icoBase);
  add("ico-largest", "default-frame", "match", ico([entry(dib(2, 2, BLUE), { w: 2, h: 2 }), entry(dib(4, 3, RED))]));
  add("ico-tie-order", "default-frame", "match", ico([entry(dib(4, 3, RED)), entry(dib(4, 3, BLUE))]));
  add("ico-tie-depth", "default-frame", "match", ico([entry(dib(4, 3, RED, { bpp: 24 }), { depth: 24 }), entry(dib(4, 3, BLUE))]));
  // Equal areas: the deeper entry wins over the earlier one.
  add("ico-area-tie-depth", "default-frame", "match", ico([entry(dib(4, 3, RED), { depth: 8 }), entry(dib(3, 4, BLUE), { w: 3, h: 4 })]), [3, 4]);
  add("ico-cur", "default-frame", "match", ico([entry(dib(4, 3, RED)), entry(dib(2, 2, BLUE), { w: 2, h: 2 })], 2));
  add("ico-reserved-planes", "container", "match", ico([entry(dib(4, 3, COL[0]), { reserved: 7, planes: 9 })]));
  add("ico-bytesize-short", "container", "match", ico([entry(dib(4, 3, COL[0]), { size: 10 })]));
  add("ico-bytesize-long", "container", "match", ico([entry(dib(4, 3, COL[0]), { size: 5000 })]));
  add("ico-unselected-garbage", "container", "match", ico([entry(Buffer.from("garbage payload here"), { w: 2, h: 2 }), entry(dib(4, 3, RED))]));
  add("ico-depth-mismatch", "container", "match", ico([entry(dib(4, 3, RED), { depth: 24 })]));
  add("ico-trailing", "container", "match", bytes(icoBase, Buffer.alloc(16)));
  add("ico-no-entries", "container", "rejected", bytes(u16(0), u16(1), u16(0)));
  add("ico-type-3", "container", "rejected", bytes(icoBase.subarray(0, 2), u16(3), icoBase.subarray(4)));
  add("ico-offset-outside", "container", "partial", bytes(icoBase.subarray(0, 18), u32(100000), icoBase.subarray(22)));
  // DIB entries.
  const pal4 = PAL.subarray(0, 16);
  add("ico-dib24-and", "alpha", "match", ico([entry(dib(4, 3, RED, { bpp: 24, andByte: () => 0xa0 }), { depth: 24 })]));
  add("ico-dib8-and", "alpha", "match", ico([entry(dib(4, 3, null, { bpp: 8, palette: pal4, andByte: y => (y ? 0x50 : 0) }), { depth: 8 })]));
  add("ico-dib4", "base", "match", ico([entry(dib(4, 3, null, { bpp: 4, palette: pal4 }), { depth: 4 })]));
  add("ico-dib1-and", "alpha", "match", ico([entry(dib(4, 3, null, { bpp: 1, palette: PAL.subarray(0, 8), andByte: () => 0x30 }), { depth: 1 })]));
  add("ico-dib32-alpha-and-bit", "alpha", "match", ico([entry(dib(4, 3, RED, { alpha: () => 200, andByte: () => 0xff }))]));
  add("ico-dib32-zero-alpha-and", "alpha", "match", ico([entry(dib(4, 3, RED, { alpha: () => 0, andByte: () => 0xf0 }))]));
  add("ico-dib32-partial-alpha", "alpha", "match", ico([entry(dib(4, 3, COL[1], { alpha: x => [0, 64, 128, 255][x] }))]));
  add("ico-dib-topdown", "orientation", "match", ico([entry(dib(4, 3, RED, { heightField: -6 }))]));
  add("ico-dib-odd-height", "container", "match", ico([entry(dib(4, 3, RED, { heightField: 7 }))]));
  add("ico-dib-truncated", "partial", "partial", ico([entry(dib(4, 3, RED).subarray(0, 60))]));
  // PNG entries.
  const pngEntry = extra => ico([entry(png(4, 3, PNG_RGBA, extra))]);
  add("ico-png", "base", "match", pngEntry());
  add("ico-png-text", "orientation", "match", pngEntry(chunk("tEXt", Buffer.from("k\0v"))));
  add("ico-png-exif-orient6", "orientation", "match", pngEntry(chunk("eXIf", Buffer.from([0x49, 0x49, 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0]))));
  add("ico-png-size-mismatch", "container", "partial", ico([entry(png(2, 2, PNG_RGBA))]));
  add("ico-png-srgb", "color-profile", "match", pngEntry(chunk("sRGB", Buffer.from([0]))));
  add("ico-png-gama45455", "color-profile", "match", pngEntry(chunk("gAMA", be32(45455))));
  add("ico-png-chrm", "color-profile", "match", pngEntry(chunk("cHRM", Buffer.concat([31270, 32900, 64000, 33000, 30000, 60000, 15000, 6000].map(be32)))));
  add("ico-png-cicp-srgb", "color-profile", "match", pngEntry(chunk("cICP", Buffer.from([1, 13, 0, 1]))));
  add("ico-png-iccp-srgb-v4", "color-profile", "match", pngEntry(chunk("iCCP", bytes("x\0\0", deflateSync(srgbV4)))));
  add("ico-png-gama100000", "color-profile", "divergent", pngEntry(chunk("gAMA", be32(100000))));
  add("ico-png-cicp-linear", "color-profile", "divergent", pngEntry(chunk("cICP", Buffer.from([1, 8, 0, 1]))));
  add("ico-png-iccp-p3", "color-profile", "divergent", pngEntry(chunk("iCCP", bytes("x\0\0", deflateSync(p3)))));
  add("ico-png-iccp-linear", "color-profile", "divergent", pngEntry(chunk("iCCP", bytes("x\0\0", deflateSync(linear)))));

  assert.equal(new Set(rows.map(r => r.id)).size, rows.length);
  return rows.map(row => ({ ...row, payload: row.payload.toString("base64") }));
}

export function sceneFor(row) {
  const [width, height] = row.size;
  return { type: "excalidraw", version: 2, source: "bmp-ico-container", appState: { viewBackgroundColor: "#ffffff" },
    elements: [{ id: "image", type: "image", x: 0, y: 0, width, height, angle: 0, strokeColor: "transparent", backgroundColor: "transparent",
      fillStyle: "solid", strokeWidth: 1, strokeStyle: "solid", roughness: 0, opacity: 100, groupIds: [], frameId: null, roundness: null, seed: 1,
      version: 1, versionNonce: 1, isDeleted: false, boundElements: null, updated: 1, link: null, locked: false, status: "saved", fileId: "asset",
      scale: [1, 1], crop: null, index: "a0" }],
    files: { asset: { id: "asset", mimeType: row.mime, dataURL: `data:${row.mime};base64,${row.payload}`, created: 1 } } };
}
