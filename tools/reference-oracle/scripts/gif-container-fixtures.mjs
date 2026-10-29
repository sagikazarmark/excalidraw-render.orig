// GIF container, metadata, palette, bounds, animation and partial-data
// variants (#85 GIF facets). Every row edits the blocks of a frozen GIF vector
// (tests/oracle/gif-first-frame/reference.json) or re-encodes a frame with the
// same palette; the 48 frozen controls of the three GIF oracles (first-frame,
// LZW, control) are included
// as they are. Each row declares its expectation:
//   match            - the renderer must equal the reference exactly;
//   partial          - the reference paints a partial first frame; the
//                      renderer rejects (the declared divergence, #85);
//   rejected         - the reference rejects the file;
//   resource-rejected - the reference rejects it for its size; the renderer's
//                      resource limits decide, and direct SVG embeds it.
import assert from "node:assert/strict";
import { inflateSync } from "node:zlib";

const u16 = v => Buffer.from([v & 255, v >> 8]);

function parse(b) {
  const lsd = b.subarray(6, 13); const p = lsd[4];
  const n = p & 0x80 ? 3 * (2 << (p & 7)) : 0;
  const gct = b.subarray(13, 13 + n); let o = 13 + n; const blocks = [];
  while (o < b.length) {
    const t = b[o], s = o;
    if (t === 0x21) { o += 2; while (b[o]) o += 1 + b[o]; o += 1; blocks.push(["ext", b.subarray(s, o)]); }
    else if (t === 0x2c) { const pk = b[o + 9]; o += 10; if (pk & 0x80) o += 3 * (2 << (pk & 7)); o += 1; while (b[o]) o += 1 + b[o]; o += 1; blocks.push(["img", b.subarray(s, o)]); }
    else break;
  }
  return { header: b.subarray(0, 6), lsd, gct, blocks };
}
const build = (h, lsd, gct, blocks, tail = Buffer.from(";")) => Buffer.concat([h, lsd, gct, ...blocks.map(([, x]) => x), tail]);
function subBlocks(data) {
  const out = [];
  for (let i = 0; i < data.length; i += 255) { const part = data.subarray(i, i + 255); out.push(Buffer.from([part.length]), part); }
  return Buffer.concat([...out, Buffer.from([0])]);
}
function lzw(indices, minCode, { extra = [], noEoi = false } = {}) {
  const clear = 1 << minCode, eoi = clear + 1; const table = new Map();
  for (let i = 0; i < Math.min(clear, 256); i++) table.set(String.fromCharCode(i), i);
  let next = eoi + 1; const codes = [clear]; let w = "";
  for (const k of indices) {
    const wk = w + String.fromCharCode(k);
    if (table.has(wk)) w = wk;
    else { codes.push(table.get(w)); if (next < 4096) table.set(wk, next++); w = String.fromCharCode(k); }
  }
  if (w) codes.push(table.get(w));
  codes.push(...extra); if (!noEoi) codes.push(eoi);
  let bits = 0, nb = 0, size = minCode + 1, avail = eoi + 1, first = true; const out = [];
  for (const c of codes) {
    bits |= c << nb; nb += size;
    while (nb >= 8) { out.push(bits & 255); bits >>>= 8; nb -= 8; }
    if (c === clear) { size = minCode + 1; avail = eoi + 1; first = true; continue; }
    if (first) { first = false; continue; }
    if (avail < 4096) avail++;
    if (avail >= (1 << size) && size < 12) size++;
  }
  if (nb) out.push(bits & 255);
  return Buffer.from(out);
}
const image = (x, y, w, h, indices, { minCode = 2, pk = 0, lct = Buffer.alloc(0), ...rest } = {}) =>
  ["img", Buffer.concat([Buffer.from([0x2c]), u16(x), u16(y), u16(w), u16(h), Buffer.from([pk]), lct, Buffer.from([minCode]), subBlocks(lzw(indices, minCode, rest))])];
const gce = (flags = 0, tindex = 0) => ["ext", Buffer.from([0x21, 0xf9, 4, flags, 0xff, 0xff, tindex, 0])];
const app = (ident, data, size = 11) => ["ext", Buffer.concat([Buffer.from([0x21, 0xff, size]), Buffer.from(ident.padEnd(size).slice(0, size), "latin1"), subBlocks(data)])];
const pix = (n, f = i => i % 4) => Array.from({ length: n }, (_, i) => f(i));
const patch = (blocks, kind, label, edit) => blocks.map(([k, x]) => {
  if (k !== kind || (label !== undefined && x[1] !== label)) return [k, x];
  const copy = Buffer.from(x); edit(copy); return [k, copy];
});

/** The natural size the renderer and the reference report: the logical screen
 *  grown to the first frame's extent (a zero-size first frame adds nothing). */
export function naturalSize(b) {
  if (b.length < 13) return [9, 7];
  let w = b.readUInt16LE(6), h = b.readUInt16LE(8);
  const p = b[10]; let o = 13 + (p & 0x80 ? 3 * (2 << (p & 7)) : 0);
  while (o < b.length) {
    if (b[o] === 0x21) { o += 2; if (b[o - 1] === 0xf9) o += 0; while (o < b.length && b[o]) o += 1 + b[o]; o += 1; }
    else if (b[o] === 0x2c && o + 9 < b.length) {
      const [x, y, fw, fh] = [b.readUInt16LE(o + 1), b.readUInt16LE(o + 3), b.readUInt16LE(o + 5), b.readUInt16LE(o + 7)];
      if (fw && fh) { w = Math.max(w, x + fw); h = Math.max(h, y + fh); }
      break;
    } else break;
  }
  return [w || 9, h || 7];
}

function iccFromPng(png) {
  for (let at = 8; at < png.length;) {
    const size = png.readUInt32BE(at);
    if (png.toString("latin1", at + 4, at + 8) === "iCCP") { const d = png.subarray(at + 8, at + 8 + size); return inflateSync(d.subarray(d.indexOf(0) + 2)); }
    at += 12 + size;
  }
  throw Error("no iCCP");
}

export function gifContainerFixtures(firstFrame, control, lzwRecord, color) {
  const g = id => Buffer.from(firstFrame.cases.find(c => c.id === id).gif_base64, "base64");
  const S = g("global-full-opaque-sequential-static-asymmetric"), T = g("global-full-transparent-sequential-static-asymmetric");
  const A = g("global-full-opaque-sequential-animated-d0-asymmetric"), I = g("global-full-opaque-interlaced-static-asymmetric");
  const rows = [];
  const add = (id, group, expectation, bytes) => rows.push({ id, group, expectation, gif: bytes });
  const s = parse(S), t = parse(T), a = parse(A);
  const aImages = a.blocks.map(([k], i) => k === "img" ? i : -1).filter(i => i >= 0);
  const secondGce = aImages[1] - 1;
  add("base-S", "base", "match", S); add("base-T", "base", "match", T); add("base-A", "base", "match", A); add("base-I", "base", "match", I);
  add("base-encoded", "base", "match", build(s.header, s.lsd, s.gct, [gce(), image(0, 0, 9, 7, pix(63))]));
  // Container and metadata tolerance: drawn exactly like the base.
  add("trailing-junk", "container", "match", Buffer.concat([S, Buffer.alloc(16)]));
  add("nul-terminator", "container", "match", Buffer.concat([S.subarray(0, -1), Buffer.from([0])]));
  add("missing-trailer", "container", "match", S.subarray(0, -1));
  add("frame-after-junk", "container", "match", build(a.header, a.lsd, a.gct, [...a.blocks.slice(0, secondGce), ["junk", Buffer.from([0])], ...a.blocks.slice(secondGce)]));
  add("unknown-label", "container", "match", build(s.header, s.lsd, s.gct, [["ext", Buffer.from([0x21, 0x7f, 3, 97, 98, 99, 0])], ...s.blocks]));
  add("app-size-10", "container", "match", build(s.header, s.lsd, s.gct, [app("ABCDEFGHIJ", Buffer.from("x"), 10), ...s.blocks]));
  add("plaintext-size-11", "container", "match", build(s.header, s.lsd, s.gct, [["ext", Buffer.concat([Buffer.from([0x21, 0x01, 11]), Buffer.alloc(11), Buffer.from([1, 65, 0])])], ...s.blocks]));
  add("gif87a-extensions", "container", "match", build(Buffer.from("GIF87a"), t.lsd, t.gct, t.blocks));
  add("gce-reserved", "container", "match", build(s.header, s.lsd, s.gct, patch(s.blocks, "ext", 0xf9, x => { x[3] |= 0xe0; })));
  add("disposal-5", "container", "match", build(s.header, s.lsd, s.gct, patch(s.blocks, "ext", 0xf9, x => { x[3] = (x[3] & 0xe3) | (5 << 2); })));
  add("descriptor-reserved", "container", "match", build(s.header, s.lsd, s.gct, patch(s.blocks, "img", undefined, x => { x[9] |= 0x38; })));
  add("aspect-ff", "container", "match", build(s.header, Buffer.concat([s.lsd.subarray(0, 6), Buffer.from([0xff])]), s.gct, s.blocks));
  add("background-out-of-range", "container", "match", build(s.header, Buffer.concat([s.lsd.subarray(0, 5), Buffer.from([200]), s.lsd.subarray(6)]), s.gct, s.blocks));
  add("zero-logical-screen", "container", "match", build(s.header, Buffer.concat([Buffer.alloc(4), s.lsd.subarray(4)]), s.gct, s.blocks));
  const p3 = iccFromPng(Buffer.from(color.cases.find(c => c.signal === "icc-v2-p3").png, "base64"));
  const xmp = Buffer.from('<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description xmlns:tiff="http://ns.adobe.com/tiff/1.0/" tiff:Orientation="6"/></rdf:RDF></x:xmpmeta>', "latin1");
  const magic = Buffer.from([1, ...Array.from({ length: 256 }, (_, i) => 255 - i), 0]);
  const xmpBlock = body => ["ext", Buffer.concat([Buffer.from([0x21, 0xff, 11]), Buffer.from("XMP DataXMP", "latin1"), body, magic])];
  add("icc-p3", "color-profile", "match", build(s.header, s.lsd, s.gct, [app("ICCRGBG1012", p3), ...s.blocks]));
  add("icc-after-image", "color-profile", "match", build(s.header, s.lsd, s.gct, [...s.blocks, app("ICCRGBG1012", p3)]));
  add("icc-unparseable", "color-profile", "match", build(s.header, s.lsd, s.gct, [app("ICCRGBG1012", Buffer.from("not a profile")), ...s.blocks]));
  add("icc-oversized", "color-profile", "match", build(s.header, s.lsd, s.gct, [app("ICCRGBG1012", Buffer.concat([p3, Buffer.alloc((1 << 20) + 4096)])), ...s.blocks]));
  add("xmp-orientation-6", "orientation", "match", build(s.header, s.lsd, s.gct, [xmpBlock(xmp), ...s.blocks]));
  add("xmp-oversized", "orientation", "match", build(s.header, s.lsd, s.gct, [xmpBlock(Buffer.concat([xmp, Buffer.alloc((1 << 20) + 4096, 0x20)])), ...s.blocks]));
  // Palette semantics.
  const oob = i => (i % 5 ? i % 4 : 6);
  add("pixel-out-of-palette", "alpha", "match", build(s.header, s.lsd, s.gct, [gce(), image(0, 0, 9, 7, pix(63, oob), { minCode: 3 })]));
  add("pixel-out-of-palette-transparent", "alpha", "match", build(s.header, s.lsd, s.gct, [gce(1, 6), image(0, 0, 9, 7, pix(63, oob), { minCode: 3 })]));
  add("transparent-index-out-of-range", "alpha", "match", build(t.header, t.lsd, t.gct, patch(t.blocks, "ext", 0xf9, x => { x[6] = 200; })));
  const noPalette = Buffer.concat([s.lsd.subarray(0, 4), Buffer.from([s.lsd[4] & 0x7f]), s.lsd.subarray(5)]);
  add("no-palette", "alpha", "match", build(s.header, noPalette, Buffer.alloc(0), s.blocks));
  add("no-palette-transparent", "alpha", "match", build(s.header, noPalette, Buffer.alloc(0), [gce(1, 1), ...s.blocks.filter(([k]) => k === "img")]));
  // Bounds and frames.
  add("frame-overhang", "default-frame", "match", build(s.header, s.lsd, s.gct, [gce(), image(6, 4, 5, 5, pix(25))]));
  add("frame-far", "default-frame", "match", build(s.header, s.lsd, s.gct, [gce(), image(20, 20, 5, 5, pix(25))]));
  add("zero-width-first-frame", "default-frame", "match", build(s.header, s.lsd, s.gct, [gce(), image(0, 0, 0, 7, [])]));
  add("no-images", "default-frame", "match", build(s.header, s.lsd, s.gct, []));
  add("excess-pixels", "default-frame", "match", build(s.header, s.lsd, s.gct, [gce(), image(0, 0, 9, 7, pix(80))]));
  add("invalid-code-after-full", "default-frame", "match", build(s.header, s.lsd, s.gct, [gce(), image(0, 0, 9, 7, pix(63), { extra: [62] })]));
  add("missing-eoi-after-full", "default-frame", "match", build(s.header, s.lsd, s.gct, [gce(), image(0, 0, 9, 7, pix(63), { noEoi: true })]));
  add("later-frame-overhang", "animation", "match", build(a.header, a.lsd, a.gct, [...a.blocks.slice(0, aImages[0] + 1), gce(), image(6, 4, 5, 5, pix(25))]));
  add("later-frame-zero-size", "animation", "match", build(a.header, a.lsd, a.gct, [...a.blocks.slice(0, aImages[0] + 1), gce(), image(0, 0, 0, 0, [])]));
  add("later-gce-size-3", "animation", "match", build(a.header, a.lsd, a.gct, a.blocks.map(([k, x], i) => i === secondGce ? ["ext", Buffer.from([0x21, 0xf9, 3, 0, 0xff, 0xff, 0])] : [k, x])));
  add("three-frames", "animation", "match", build(a.header, a.lsd, a.gct, [...a.blocks, a.blocks[secondGce], a.blocks[aImages[1]]]));
  // The declared divergence: a first frame whose data ends before its pixels.
  add("early-eoi-opaque", "partial", "partial", build(s.header, s.lsd, s.gct, [gce(), image(0, 0, 9, 7, pix(31))]));
  add("early-eoi-transparent", "partial", "partial", build(s.header, s.lsd, s.gct, [gce(1, 3), image(0, 0, 9, 7, pix(31))]));
  add("truncated-data", "partial", "partial", S.subarray(0, -20));
  add("truncated-interlaced", "partial", "partial", I.subarray(0, -25));
  add("truncated-local-palette", "partial", "partial", build(s.header, s.lsd, s.gct, [gce(), ["img", Buffer.concat([Buffer.from([0x2c]), u16(0), u16(0), u16(9), u16(7), Buffer.from([0x81]), s.gct.subarray(0, 5)])]], Buffer.alloc(0)));
  add("minimum-code-size-9", "partial", "partial", build(s.header, s.lsd, s.gct, [gce(), image(0, 0, 9, 7, pix(63), { minCode: 9 })]));
  // Rejected by both.
  add("truncated-screen", "container", "rejected", S.subarray(0, 10));
  add("first-gce-size-3", "container", "rejected", build(s.header, s.lsd, s.gct, s.blocks.map(([k, x]) => k === "ext" && x[1] === 0xf9 ? ["ext", Buffer.from([0x21, 0xf9, 3, 0, 0xff, 0xff, 0])] : [k, x])));
  // The frozen controls, now reference-collected. Complete first frames match;
  // partial ones are the declared divergence.
  const partial = new Set(["first-frame-first-stream-invalid-code", "first-frame-truncated-image-data", "control-early-undefined-lzw",
    "control-early-truncated-subblocks", "control-early-minimum-code-size-1", "control-early-early-eoi", "control-early-missing-eoi",
    "control-early-truncated-palette", "lzw-minimum-code-size-1", "lzw-minimum-code-size-9", "lzw-undefined-initial-code",
    "lzw-undefined-after-literal", "lzw-missing-eoi", "lzw-truncated-subblock", "lzw-early-eoi"]);
  const rejected = new Set(["first-frame-invalid-gce-size", "first-frame-huge-image-descriptor", "control-early-truncated-gce"]);
  // Rejected by the reference for its size: the renderer's resource limits
  // decide instead, and direct SVG, which derives no pixels, embeds it.
  const resourceRejected = new Set(["first-frame-huge-logical-screen"]);
  for (const [prefix, record] of [["first-frame", firstFrame], ["control", control], ["lzw", lzwRecord]]) {
    for (const c of record.controls) {
      const id = `${prefix}-${c.id}`;
      const expectation = partial.has(id) ? "partial" : rejected.has(id) ? "rejected" : resourceRejected.has(id) ? "resource-rejected" : "match";
      add(`control-${id}`, "controls", expectation, Buffer.from(c.gif_base64, "base64"));
    }
  }
  assert.equal(new Set(rows.map(r => r.id)).size, rows.length);
  // A rejected row draws nothing; keep its scene small.
  return rows.map(row => ({ ...row, size: row.expectation.endsWith("rejected") ? [9, 7] : naturalSize(row.gif), gif: row.gif.toString("base64") }));
}

export function sceneFor(row) {
  const [width, height] = row.size;
  return { type: "excalidraw", version: 2, source: "gif-container", appState: { viewBackgroundColor: "#ffffff" },
    elements: [{ id: "image", type: "image", x: 0, y: 0, width, height, angle: 0, strokeColor: "transparent", backgroundColor: "transparent",
      fillStyle: "solid", strokeWidth: 1, strokeStyle: "solid", roughness: 0, opacity: 100, groupIds: [], frameId: null, roundness: null, seed: 1,
      version: 1, versionNonce: 1, isDeleted: false, boundElements: null, updated: 1, link: null, locked: false, status: "saved", fileId: "asset",
      scale: [1, 1], crop: null, index: "a0" }],
    files: { asset: { id: "asset", mimeType: "image/gif", dataURL: `data:image/gif;base64,${row.gif}`, created: 1 } } };
}
