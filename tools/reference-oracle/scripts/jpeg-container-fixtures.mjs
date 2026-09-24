// JPEG container, marker, entropy, default-image, orientation, color and
// gain-map container variants (#85 JPEG facets). Every row edits the markers
// of a frozen project-authored JPEG (tests/oracle/jpeg-color/inputs.json); the
// ICC profiles are the frozen PNG color vectors. Each row declares its
// expectation:
//   match    - the renderer must pass the canonical comparison with the
//              reference;
//   partial  - the reference loads the file but paints only what it could
//              decode (possibly nothing, or garbled rows); the renderer
//              rejects (the declared divergence, #85);
//   rejected - the reference rejects the file.
import assert from "node:assert/strict";
import { inflateSync } from "node:zlib";

const segment = (marker, payload) => {
  const head = Buffer.from([255, marker, 0, 0]); head.writeUInt16BE(payload.length + 2, 2);
  return Buffer.concat([head, payload]);
};
const app = (n, payload) => segment(0xe0 + n, payload);

/** The marker segments of a JPEG: [marker, start, end), the scan with its
 *  entropy-coded data. */
function segments(b) {
  const out = []; let o = 2;
  while (o < b.length) {
    assert.equal(b[o], 0xff); const m = b[o + 1];
    if (m === 0xd9) { out.push([m, o, o + 2]); o += 2; continue; }
    const start = o; o += 2 + b.readUInt16BE(o + 2);
    if (m === 0xda) while (!(b[o] === 0xff && b[o + 1] !== 0 && !(b[o + 1] >= 0xd0 && b[o + 1] <= 0xd7))) o += 1;
    out.push([m, start, o]);
  }
  return out;
}
const first = (b, marker) => segments(b).find(([m]) => m === marker);

/** An EXIF APP1 with IFD0 holding `entries` ([tag, type, count, value
 *  bytes]) and an optional IFD1. */
function exif(entries, { big = false, ifd1 = null, header = "Exif\0\0", next = null } = {}) {
  const u16 = v => { const x = Buffer.alloc(2); big ? x.writeUInt16BE(v) : x.writeUInt16LE(v); return x; };
  const u32 = v => { const x = Buffer.alloc(4); big ? x.writeUInt32BE(v) : x.writeUInt32LE(v); return x; };
  const ifd = (list, nextOffset) => Buffer.concat([u16(list.length), ...list.map(([tag, type, count, value]) =>
    Buffer.concat([u16(tag), u16(type), u32(count), Buffer.concat([value, Buffer.alloc(4)]).subarray(0, 4)])), u32(nextOffset)]);
  const ifd0Size = 2 + 12 * entries.length + 4;
  const tail = ifd1 ? ifd(ifd1, 0) : Buffer.alloc(0);
  const tiff = Buffer.concat([Buffer.from(big ? "MM" : "II"), u16(42), u32(8), ifd(entries, next ?? (ifd1 ? 8 + ifd0Size : 0)), tail]);
  return app(1, Buffer.concat([Buffer.from(header, "latin1"), tiff]));
}
const short = (v, big = false) => { const x = Buffer.alloc(2); big ? x.writeUInt16BE(v) : x.writeUInt16LE(v); return x; };
const orientation = (v, big = false) => [0x112, 3, 1, short(v, big)];
const xmp = attrs => app(1, Buffer.from(`http://ns.adobe.com/xap/1.0/\0<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" ${attrs}/></rdf:RDF></x:xmpmeta>`));
const icc = profile => app(2, Buffer.concat([Buffer.from("ICC_PROFILE\0"), Buffer.from([1, 1]), profile]));

function iccFromPng(png) {
  for (let at = 8; at < png.length;) {
    const size = png.readUInt32BE(at);
    if (png.toString("latin1", at + 4, at + 8) === "iCCP") { const d = png.subarray(at + 8, at + 8 + size); return inflateSync(d.subarray(d.indexOf(0) + 2)); }
    at += 12 + size;
  }
  throw Error("no iCCP");
}

export function jpegContainerFixtures(inputs, color) {
  const jpeg = id => Buffer.from(inputs.cases.find(c => c.id === id).jpeg_base64, "base64");
  const B = jpeg("ycbcr-2x2-baseline-ramp"), P = jpeg("ycbcr-2x2-progressive-ramp"), E = jpeg("ycbcr-2x2-baseline-edges");
  const G = jpeg("gray-1x1-baseline-edges"), K = jpeg("cmyk-1x1-baseline-edges"), GR = jpeg("gray-1x1-baseline-ramp");
  const profile = signal => iccFromPng(Buffer.from(color.cases.find(c => c.signal === signal).png, "base64"));
  const rows = [];
  // The natural size: the first frame's (or the given size), swapped where
  // the reference applies a transposing orientation; a rejected row draws
  // nothing.
  const add = (id, group, expectation, bytes, transposed = false) => {
    const sof = [0xc0, 0xc2].map(m => bytes.indexOf(Buffer.from([0xff, m]))).filter(i => i >= 0).sort((x, y) => x - y)[0];
    const size = expectation === "rejected" ? [33, 25] : [bytes.readUInt16BE(sof + 7), bytes.readUInt16BE(sof + 5)];
    rows.push({ id, group, expectation, jpeg: bytes, size: Array.isArray(transposed) ? transposed : transposed ? size.reverse() : size });
  };
  const after = (b, marker) => first(b, marker)[2];
  const sosB = first(B, 0xda)[1], eoiB = first(B, 0xd9)[1];
  const at = (b, offset, insert) => Buffer.concat([b.subarray(0, offset), insert, b.subarray(offset)]);
  const withSegment = (b, seg) => at(b, 2, seg);

  add("base", "base", "match", B);
  add("base-progressive", "base", "match", P);
  add("base-edges", "base", "match", E);
  // Containers the reference draws completely.
  add("trailing-zeros", "malformed", "match", Buffer.concat([B, Buffer.alloc(1024)]));
  add("trailing-garbage", "malformed", "match", Buffer.concat([B, Buffer.from("garbage after the end of image")]));
  add("missing-eoi-progressive", "malformed", "match", P.subarray(0, -2));
  add("fill-bytes-before-marker", "malformed", "match", at(B, after(B, 0xc0), Buffer.from([255, 255, 255])));
  add("junk-before-marker", "malformed", "match", at(B, after(B, 0xc0), Buffer.from([0, 0x12])));
  add("junk-before-sos", "malformed", "match", at(B, sosB, Buffer.from("junk")));
  add("app3-garbage", "malformed", "match", at(B, after(B, 0xe0), app(3, Buffer.from("\x01\x02garbage", "latin1"))));
  add("app15-empty", "malformed", "match", at(B, after(B, 0xe0), Buffer.from([0xff, 0xef, 0, 2])));
  add("com-64k", "malformed", "match", at(B, after(B, 0xe0), segment(0xfe, Buffer.alloc(65000, 0x63))));
  add("appn-over-1mib", "malformed", "match", at(B, after(B, 0xe0), Buffer.concat(Array.from({ length: 17 }, () => app(9, Buffer.alloc(65000, 0x78))))));
  add("rst-before-sos", "malformed", "match", at(B, sosB, Buffer.from([0xff, 0xd0])));
  add("tem-marker", "malformed", "match", at(B, after(B, 0xe0), Buffer.from([0xff, 0x01])));
  add("dri-zero", "malformed", "match", at(B, sosB, segment(0xdd, Buffer.from([0, 0]))));
  add("dqt-16-bit", "malformed", "match", (() => {
    const [, s, e] = first(B, 0xdb); const table = B.subarray(s + 5, e);
    const wide = Buffer.concat([Buffer.from([0x10 | (B[s + 4] & 15)]), Buffer.from([...table].flatMap(v => [0, v]))]);
    return Buffer.concat([B.subarray(0, s), segment(0xdb, wide), B.subarray(e)]);
  })());
  add("no-dht", "malformed", "match", Buffer.concat([B.subarray(0, first(B, 0xc4)[1]), B.subarray(sosB)]));
  add("flipped-entropy", "malformed", "match", (() => { const x = Buffer.from(B); x[700] ^= 0x55; return x; })());
  // The declared divergence: the reference paints what it decoded.
  add("missing-eoi", "partial", "partial", B.subarray(0, -2));
  add("truncated-scan-half", "partial", "partial", B.subarray(0, sosB + ((eoiB - sosB) >> 1)));
  add("truncated-progressive-after-dc", "partial", "partial", P.subarray(0, segments(P).filter(([m]) => m === 0xda)[1][2]));
  add("truncated-progressive-mid", "partial", "partial", P.subarray(0, segments(P).filter(([m]) => m === 0xda)[5][2]));
  add("no-dqt", "partial", "partial", Buffer.concat([B.subarray(0, first(B, 0xdb)[1]), B.subarray(first(B, 0xc4)[1])]));
  add("dri-without-rst", "partial", "partial", at(B, sosB, segment(0xdd, Buffer.from([0, 1]))));
  // Rejected by both.
  add("appn-past-eof", "malformed", "rejected", Buffer.concat([B.subarray(0, after(B, 0xe0)), Buffer.from([0xff, 0xe5, 0x7f, 0xff]), Buffer.from("abc")]));
  add("duplicate-soi", "malformed", "rejected", Buffer.concat([Buffer.from([0xff, 0xd8]), B]));
  add("eoi-before-sos", "malformed", "rejected", at(B, sosB, Buffer.from([0xff, 0xd9])));
  add("two-sof", "malformed", "rejected", at(B, after(B, 0xc0), B.subarray(first(B, 0xc0)[1], first(B, 0xc0)[2])));
  add("width-zero", "malformed", "rejected", (() => { const x = Buffer.from(B); x.writeUInt16BE(0, first(B, 0xc0)[1] + 7); return x; })());
  add("dnl-height-zero", "malformed", "rejected", (() => {
    const x = Buffer.from(B); x.writeUInt16BE(0, first(B, 0xc0)[1] + 5);
    return at(x, eoiB, segment(0xdc, Buffer.from([0, 25])));
  })());
  add("sos-unknown-component", "malformed", "rejected", (() => { const x = Buffer.from(B); x[sosB + 5] = 9; return x; })());
  // Default image: the first image in the file.
  add("second-jpeg", "default-frame", "match", Buffer.concat([B, E]));
  add("second-jpeg-no-eoi", "default-frame", "match", Buffer.concat([B, E.subarray(0, -2)]));
  add("exif-thumbnail", "default-frame", "match", withSegment(B, exif([], { ifd1: [[0x201, 4, 1, Buffer.from([0x1a, 0, 0, 0])], [0x202, 4, 1, Buffer.from([0, 0, 0, 0])]] })));
  add("jfxx-thumbnail", "default-frame", "match", at(B, after(B, 0xe0), app(0, Buffer.concat([Buffer.from("JFXX\0\x10", "latin1"), E]))), [33, 25]);
  // Orientation: EXIF IFD0 only, the first valid value.
  for (const v of [2, 4, 5, 7]) add(`orientation-${v}`, "orientation", "match", withSegment(E, exif([orientation(v)])), v >= 5);
  add("orientation-6-big-endian", "orientation", "match", withSegment(E, exif([orientation(6, true)], { big: true })), true);
  for (const v of [0, 9, 65535]) add(`orientation-${v}`, "orientation", "match", withSegment(E, exif([orientation(v)])));
  add("orientation-long-type", "orientation", "match", withSegment(E, exif([[0x112, 4, 1, Buffer.from([6, 0, 0, 0])]])));
  add("orientation-count-2", "orientation", "match", withSegment(E, exif([[0x112, 3, 2, Buffer.from([6, 0, 6, 0])]])));
  add("orientation-ifd1-only", "orientation", "match", withSegment(E, exif([], { ifd1: [orientation(6)] })));
  add("orientation-xmp-only", "orientation", "match", withSegment(E, xmp('xmlns:tiff="http://ns.adobe.com/tiff/1.0/" tiff:Orientation="6"')));
  add("orientation-xmp-6-exif-3", "orientation", "match", withSegment(withSegment(E, exif([orientation(3)])), xmp('xmlns:tiff="http://ns.adobe.com/tiff/1.0/" tiff:Orientation="6"')));
  add("orientation-after-icc", "orientation", "match", at(E, after(E, 0xe0), Buffer.concat([icc(profile("icc-v4-srgb")), exif([orientation(6)])])), true);
  add("orientation-before-app0", "orientation", "match", withSegment(E, exif([orientation(6)])), true);
  add("orientation-exif-header-variant", "orientation", "match", withSegment(E, exif([orientation(6)], { header: "Exif\0\xff" })), true);
  add("orientation-ifd-loop", "orientation", "match", withSegment(E, exif([orientation(6)], { next: 8 })), true);
  add("orientation-gray", "orientation", "match", withSegment(G, exif([orientation(6)])), true);
  add("orientation-cmyk", "orientation", "match", withSegment(K, exif([orientation(6)])), true);
  // Color: profiles on each model, malformed profiles ignored.
  add("icc-srgb-v4", "color-profile", "match", withSegment(E, icc(profile("icc-v4-srgb"))));
  add("icc-p3", "color-profile", "match", withSegment(E, icc(profile("icc-v2-p3"))));
  add("icc-linear", "color-profile", "match", withSegment(E, icc(profile("icc-v2-gamma1"))));
  add("icc-truncated", "color-profile", "match", withSegment(E, icc(profile("icc-v2-p3").subarray(0, 200))));
  add("icc-p3-on-gray", "color-profile", "match", withSegment(G, icc(profile("icc-v2-p3"))));
  add("icc-p3-on-cmyk", "color-profile", "match", withSegment(K, icc(profile("icc-v2-p3"))));
  add("adobe-transform-0-on-ycbcr", "color-profile", "match", at(E, after(E, 0xe0), app(14, Buffer.from([0x41, 0x64, 0x6f, 0x62, 0x65, 0, 100, 0, 0, 0, 0, 0]))));
  add("rgb-component-ids", "color-profile", "match", (() => {
    const x = Buffer.from(E); const s = first(E, 0xc0)[1] + 10; x[s] = 0x52; x[s + 3] = 0x47; x[s + 6] = 0x42;
    const sos = first(x, 0xda)[1] + 5; x[sos] = 0x52; x[sos + 2] = 0x47; x[sos + 4] = 0x42; return x;
  })());
  // Gain-map containers other than Adobe hdrgm: the primary image draws.
  const gain = (primaryMeta, secondaryMeta) => {
    const secondary = Buffer.concat([GR.subarray(0, 2), secondaryMeta, GR.subarray(2)]);
    return Buffer.concat([E.subarray(0, 2), primaryMeta, E.subarray(2), secondary]);
  };
  const iso = app(2, Buffer.concat([Buffer.from("urn:iso:std:iso:ts:21496:-1\0", "latin1"), Buffer.from([0, 0, 0, 0])]));
  add("gain-map-iso-21496", "hdr-gain-map", "match", gain(iso, iso));
  add("gain-map-gcontainer-only", "hdr-gain-map", "match", gain(app(1, Buffer.from('http://ns.adobe.com/xap/1.0/\0<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:Container="http://ns.google.com/photos/1.0/container/" xmlns:Item="http://ns.google.com/photos/1.0/container/item/"><Container:Directory><rdf:Seq><rdf:li rdf:parseType="Resource"><Container:Item Item:Semantic="Primary" Item:Mime="image/jpeg"/></rdf:li><rdf:li rdf:parseType="Resource"><Container:Item Item:Semantic="GainMap" Item:Mime="image/jpeg"/></rdf:li></rdf:Seq></Container:Directory></rdf:Description></rdf:RDF></x:xmpmeta>')), Buffer.alloc(0)));

  assert.equal(new Set(rows.map(r => r.id)).size, rows.length);
  return rows.map(row => ({ ...row, jpeg: row.jpeg.toString("base64") }));
}

export function sceneFor(row) {
  const [width, height] = row.size;
  return { type: "excalidraw", version: 2, source: "jpeg-container", appState: { viewBackgroundColor: "#ffffff" },
    elements: [{ id: "image", type: "image", x: 0, y: 0, width, height, angle: 0, strokeColor: "transparent", backgroundColor: "transparent",
      fillStyle: "solid", strokeWidth: 1, strokeStyle: "solid", roughness: 0, opacity: 100, groupIds: [], frameId: null, roundness: null, seed: 1,
      version: 1, versionNonce: 1, isDeleted: false, boundElements: null, updated: 1, link: null, locked: false, status: "saved", fileId: "asset",
      scale: [1, 1], crop: null, index: "a0" }],
    files: { asset: { id: "asset", mimeType: "image/jpeg", dataURL: `data:image/jpeg;base64,${row.jpeg}`, created: 1 } } };
}
