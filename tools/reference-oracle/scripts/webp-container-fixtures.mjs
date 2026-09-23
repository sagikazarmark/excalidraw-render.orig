// WebP container, metadata, color-profile, alpha and animation variants (#85
// WebP facets). Every row edits only the RIFF container of a frozen external
// libwebp vector (tests/oracle/webp-sample/reference.json); ICC profiles are
// the PNG color oracle's own authored profiles. Rows declare the base whose
// reference output they must equal when the reference ignores the variant.
import assert from "node:assert/strict";
import { inflateSync } from "node:zlib";

const le32 = v => { const b = Buffer.alloc(4); b.writeUInt32LE(v); return b; };
function chunks(webp) {
  const out = [];
  for (let at = 12; at < webp.length;) {
    const size = webp.readUInt32LE(at + 4);
    out.push([webp.toString("latin1", at, at + 4), Buffer.from(webp.subarray(at + 8, at + 8 + size))]);
    at += 8 + size + (size & 1);
  }
  return out;
}
function build(list, trailing = Buffer.alloc(0)) {
  const body = Buffer.concat([Buffer.from("WEBP"), ...list.flatMap(([kind, data]) =>
    [Buffer.from(kind, "latin1"), le32(data.length), data, Buffer.alloc(data.length & 1)])]);
  return Buffer.concat([Buffer.from("RIFF"), le32(body.length), body, trailing]);
}
const vp8x = (flags, w = 9, h = 7) => ["VP8X", Buffer.from([flags, 0, 0, 0, (w - 1) & 255, (w - 1) >> 8, 0, (h - 1) & 255, (h - 1) >> 8, 0])];
const withFlags = (list, set, clear = 0) => list.map(([k, d], i) => i === 0 && k === "VP8X" ? [k, Buffer.concat([Buffer.from([(d[0] | set) & ~clear]), d.subarray(1)])] : [k, d]);
// A VP8X header as an encoder writes it: the alpha flag states whether the
// bitstream carries alpha (an ALPH chunk, or VP8L's alpha bit).
const bitstreamAlpha = list => list.some(([k, d]) => k === "ALPH" || (k === "VP8L" && (d.readUInt32LE(1) >>> 28) & 1));
const extended = list => list[0][0] === "VP8X" ? list : [vp8x(bitstreamAlpha(list) ? 0x10 : 0), ...list];
const exif = (orientation, prefix = false) => {
  const tiff = Buffer.from([0x49, 0x49, 42, 0, 8, 0, 0, 0, 1, 0, 0x12, 1, 3, 0, 1, 0, 0, 0, orientation, 0, 0, 0, 0, 0, 0, 0]);
  return prefix ? Buffer.concat([Buffer.from("Exif\0\0", "latin1"), tiff]) : tiff;
};
const ICC = 0x20, ALPHA = 0x10, EXIF = 0x08, XMP = 0x04;

function iccFromPng(png) {
  for (let at = 8; at < png.length;) {
    const size = png.readUInt32BE(at);
    if (png.toString("latin1", at + 4, at + 8) === "iCCP") {
      const data = png.subarray(at + 8, at + 8 + size);
      return inflateSync(data.subarray(data.indexOf(0) + 2));
    }
    at += 12 + size;
  }
  throw Error("no iCCP");
}
function profiles(color, stage4) {
  const fromColor = signal => iccFromPng(Buffer.from(color.cases.find(c => c.signal === signal).png, "base64"));
  const fromStage4 = signal => {
    const tag = stage4.cases.find(c => c.signal === signal).tags.find(t => t.kind === "iCCP");
    const data = Buffer.from(tag.data);
    return inflateSync(data.subarray(data.indexOf(0) + 2));
  };
  return {
    "v2-linear": fromColor("icc-v2-gamma1"), "v2-p3": fromColor("icc-v2-p3"), "v4-param2": fromColor("icc-v4-param2"),
    "v4-srgb": fromColor("icc-v4-srgb"), "lut-mft1": fromStage4("lut-mft1-3-intent3"), "lut-lab-mab": fromStage4("lut-lab-mAB"),
    "gray-v2": fromStage4("gray-v2-g1"), "model-cmyk": fromStage4("model-CMYK"), "model-lab": fromStage4("model-Lab"),
    garbage: Buffer.from("not an ICC profile at all", "latin1"),
  };
}

export function webpContainerFixtures(samples, color, stage4) {
  const base = id => Buffer.from(samples.cases.find(c => c.id === id).webp.base64, "base64");
  const bases = {
    "lossless-opaque": base("lossless-opaque-static-full-ramp"),
    "lossy-opaque": base("lossy-opaque-static-full-ramp"),
    "lossless-partial": base("lossless-partial-static-full-ramp"),
    "lossy-partial": base("lossy-partial-static-full-ramp"),
    "anim-opaque": base("lossless-opaque-anim-full-blend-d0-clear-ramp"),
    "anim-partial": base("lossless-partial-anim-sub-blend-d1-opaque-ramp"),
    "anim-lossy-partial": base("lossy-partial-anim-full-blend-d0-clear-ramp"),
  };
  const icc = profiles(color, stage4);
  const rows = Object.entries(bases).map(([id, bytes]) => ({ id: `base-${id}`, group: "base", base_id: null, equals_base: false, webp: bytes }));
  const add = (group, id, baseId, bytes, equalsBase) => rows.push({ id: `${group}-${baseId}-${id}`, group, base_id: `base-${baseId}`, equals_base: equalsBase, webp: bytes });
  const list = id => chunks(bases[id]);

  // Orientation: the reference never applies WebP EXIF orientation.
  for (const b of ["lossless-opaque", "lossy-opaque", "lossy-partial", "anim-opaque"]) {
    for (const o of [1, 2, 3, 4, 5, 6, 7, 8, 0, 9]) add("orientation", `exif-${o}`, b, build([...withFlags(extended(list(b)), EXIF), ["EXIF", exif(o)]]), true);
    add("orientation", "exif-6-prefixed", b, build([...withFlags(extended(list(b)), EXIF), ["EXIF", exif(6, true)]]), true);
    add("orientation", "exif-6-flag-clear", b, build([...extended(list(b)), ["EXIF", exif(6)]]), true);
    add("orientation", "exif-garbage", b, build([...withFlags(extended(list(b)), EXIF), ["EXIF", Buffer.from("not a TIFF", "latin1")]]), true);
  }
  // Color profiles: applied only for a still image with the ICC flag set and
  // an RGB profile. Duplicates and a trailing ICCP follow the demuxer.
  for (const b of ["lossless-opaque", "lossy-opaque", "lossless-partial", "lossy-partial"]) {
    for (const [name, profile] of Object.entries(icc)) {
      // Ignored profiles, and v4-srgb, which Skia snaps to sRGB, draw like the base.
      const ignored = ["gray-v2", "model-cmyk", "model-lab", "garbage", "v4-srgb"].includes(name);
      const [head, ...rest] = withFlags(extended(list(b)), ICC);
      add("color-profile", `icc-${name}`, b, build([head, ["ICCP", profile], ...rest]), ignored);
    }
    const [head, ...rest] = extended(list(b));
    add("color-profile", "icc-v2-linear-flag-clear", b, build([head, ["ICCP", icc["v2-linear"]], ...rest]), true);
  }
  {
    const [head, ...rest] = withFlags(extended(list("lossless-opaque")), ICC);
    add("color-profile", "icc-duplicate-linear-then-p3", "lossless-opaque", build([head, ["ICCP", icc["v2-linear"]], ["ICCP", icc["v2-p3"]], ...rest]), false);
    add("color-profile", "icc-after-image", "lossless-opaque", build([head, ...rest, ["ICCP", icc["v2-linear"]]]), false);
    add("color-profile", "icc-flag-without-chunk", "lossless-opaque", build([head, ...rest]), true);
    add("color-profile", "icc-v2-linear-oversized", "lossless-opaque", build([head, ["ICCP", Buffer.concat([icc["v2-linear"], Buffer.alloc((1 << 20) + 4096)])], ...rest]), false);
  }
  for (const b of ["anim-opaque", "anim-partial"]) {
    const [head, ...rest] = withFlags(list(b), ICC);
    add("color-profile", "icc-v2-linear-animated", b, build([head, ["ICCP", icc["v2-linear"]], ...rest]), true);
  }
  // Container: libwebp's demuxer rules.
  for (const b of ["lossless-opaque", "lossy-partial", "anim-opaque"]) {
    const ext = extended(list(b));
    for (const bit of [0x80, 0x40, 0x01]) add("container", `vp8x-reserved-bit-${bit}`, b, build(withFlags(ext, bit)), false);
    for (const index of [1, 2, 3]) {
      const [[k, d], ...rest] = ext; const changed = Buffer.from(d); changed[index] = 0xff;
      add("container", `vp8x-reserved-byte-${index}`, b, build([[k, changed], ...rest]), true);
    }
    for (const n of [1, 4, 1024]) add("container", `trailing-${n}`, b, build(list(b), Buffer.alloc(n, 0x5a)), true);
    add("container", "exif-flag-without-chunk", b, build(withFlags(ext, EXIF)), true);
    add("container", "xmp-flag-without-chunk", b, build(withFlags(ext, XMP)), true);
    add("container", "xmp-chunk", b, build([...withFlags(ext, XMP), ["XMP ", Buffer.from("<x:xmpmeta/>", "latin1")]]), true);
    add("container", "unknown-chunk-before", b, build([ext[0], ["ZZZZ", Buffer.from("abc", "latin1")], ...ext.slice(1)]), true);
    add("container", "unknown-chunk-after", b, build([...ext, ["ZZZZ", Buffer.from("abcd", "latin1")]]), true);
    add("container", "duplicate-vp8x", b, build([ext[0], ...ext]), false);
  }
  // Metadata past the decoder workspace (1 MiB + 32 x side) stays metadata.
  const oversized = (1 << 20) + 4096;
  {
    const ext = extended(list("lossless-opaque"));
    add("container", "exif-oversized", "lossless-opaque", build([...withFlags(ext, EXIF), ["EXIF", Buffer.concat([exif(1), Buffer.alloc(oversized)])]]), true);
    add("container", "unknown-oversized", "lossless-opaque", build([...ext, ["ZZZZ", Buffer.alloc(oversized)]]), true);
  }
  add("container", "riff-size-odd-trailing", "lossless-opaque", build(list("lossless-opaque"), Buffer.alloc(3)), true);
  // Animation and default frame: extra frames, reserved bits, flags, sizes.
  for (const b of ["anim-opaque", "anim-partial", "anim-lossy-partial"]) {
    const anim = list(b);
    const frames = anim.filter(([k]) => k === "ANMF");
    add("animation", "three-frames", b, build([...anim, frames[frames.length - 1]]), true);
    add("animation", "five-frames", b, build([...anim, frames[1] ?? frames[0], frames[0], frames[1] ?? frames[0]]), true);
    add("animation", "anmf-reserved-bits", b, build(anim.map(([k, d]) => k === "ANMF" ? [k, Buffer.concat([d.subarray(0, 15), Buffer.from([d[15] | 0xfc]), d.subarray(16)])] : [k, d])), true);
    add("animation", "alpha-flag-set", b, build(withFlags(anim, ALPHA)), true);
    add("animation", "alpha-flag-clear", b, build(withFlags(anim, 0, ALPHA)), false);
    add("animation", "exif-6", b, build([...withFlags(anim, EXIF), ["EXIF", exif(6)]]), true);
    add("animation", "duplicate-anim", b, build([anim[0], anim[1], anim[1], ...anim.slice(2)]), true);
  }
  // Alpha: VP8X alpha flag against the bitstream's own alpha.
  add("alpha", "alpha-flag-clear", "lossy-partial", build(withFlags(list("lossy-partial"), 0, ALPHA)), false);
  add("alpha", "alpha-flag-set-opaque", "lossy-opaque", build(withFlags(extended(list("lossy-opaque")), ALPHA)), true);
  add("alpha", "vp8x-without-alpha-flag", "lossless-partial", build(withFlags(extended(list("lossless-partial")), 0, ALPHA)), false);
  {
    const [head, ...rest] = withFlags(extended(list("lossless-partial")), ICC, ALPHA);
    add("alpha", "icc-v2-linear-without-alpha-flag", "lossless-partial", build([head, ["ICCP", icc["v2-linear"]], ...rest]), false);
  }
  add("alpha", "vp8x-with-alpha-flag", "lossless-partial", build(withFlags(extended(list("lossless-partial")), ALPHA)), true);
  // The frozen #191 controls, now reference-collected.
  for (const control of samples.controls) rows.push({ id: `control-${control.id}`, group: "controls", base_id: `base-anim-partial`, equals_base: false, webp: Buffer.from(control.webp.base64, "base64") });
  assert.equal(new Set(rows.map(r => r.id)).size, rows.length);
  return rows.map(row => ({ ...row, webp: row.webp.toString("base64") }));
}

export function sceneFor(row) {
  return { type: "excalidraw", version: 2, source: "webp-container", appState: { viewBackgroundColor: "#ffffff" },
    elements: [{ id: "image", type: "image", x: 0, y: 0, width: 9, height: 7, angle: 0, strokeColor: "transparent", backgroundColor: "transparent",
      fillStyle: "solid", strokeWidth: 1, strokeStyle: "solid", roughness: 0, opacity: 100, groupIds: [], frameId: null, roundness: null, seed: 1,
      version: 1, versionNonce: 1, isDeleted: false, boundElements: null, updated: 1, link: null, locked: false, status: "saved", fileId: "asset",
      scale: [1, 1], crop: null, index: "a0" }],
    files: { asset: { id: "asset", mimeType: "image/webp", dataURL: `data:image/webp;base64,${row.webp}`, created: 1 } } };
}
