// Independent 9x7 GIF89a wire author. Expectations come from indices, not decode.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { scene as imageScene } from "./jpeg-icc-fixtures.mjs";

export const SPEC = "https://www.w3.org/Graphics/GIF/spec-gif89a.txt";
export const sha = bytes => createHash("sha256").update(bytes).digest("hex");
const b = values => Buffer.from(values), cat = parts => Buffer.concat(parts);
const u16 = n => { const out = Buffer.alloc(2); out.writeUInt16LE(n); return out; };
const W = 9, H = 7, LONG = 65535;
const rgbaRecord = rgba => ({ rgba_base64: rgba.toString("base64"), rgba_sha256: sha(rgba) });
const byteRecord = gif => ({ gif_base64: gif.toString("base64"), gif_sha256: sha(gif), gif_bytes: gif.length });
const gce = (transparent = null, delay = 0, user = false) => cat([
  b([0x21, 0xf9, 4, 4 | (user ? 2 : 0) | (transparent === null ? 0 : 1)]), u16(delay), b([transparent ?? 0, 0]),
]);
const comment = () => b([0x21, 0xfe, 3, 71, 67, 69, 0]);
const application = () => cat([b([0x21, 0xff, 11]), Buffer.from("GCEPROBE001"), b([2, 0x21, 0xf9, 1, 0x2c, 0])]);
// Nonzero, in-bounds grid and cell dimensions; empty text data (no glyphs).
// A GCT is present even when table images use LCTs: Plain Text requires it.
const plainText = () => cat([b([0x21, 1, 12]), u16(0), u16(0), u16(W), u16(H), b([1, 1, 1, 0, 0])]);

export const SIGNALS = ["no-gce", "transparent", "opaque", "transparent-comment", "transparent-application",
  "transparent-plain-text", "transparent-plain-text-opaque", "duplicate-transparent-opaque",
  "duplicate-opaque-transparent", "duplicate-index0-index1", "user-input-long-delay", "trailing-gce"];
function signal(name) {
  const t = gce(0), o = gce();
  switch (name) {
    case "no-gce": return { before: [], transparent: null };
    case "transparent": return { before: [t], transparent: 0 };
    case "opaque": return { before: [o], transparent: null };
    case "transparent-comment": return { before: [t, comment()], transparent: 0 };
    case "transparent-application": return { before: [t, application()], transparent: 0 };
    case "transparent-plain-text": return { before: [t, plainText()], transparent: null };
    case "transparent-plain-text-opaque": return { before: [t, plainText(), o], transparent: null };
    case "duplicate-transparent-opaque": return { before: [t, o], transparent: null };
    case "duplicate-opaque-transparent": return { before: [o, t], transparent: 0 };
    case "duplicate-index0-index1": return { before: [t, gce(1)], transparent: 1 };
    case "user-input-long-delay": return { before: [gce(0, LONG, true)], transparent: 0 };
    case "trailing-gce": return { before: [], after: [t], transparent: null };
    default: throw new Error(name);
  }
}

// Clear before each literal: every code, including EOI, is exactly three bits.
// No dictionary encoder or production decoder participates in fixture authoring.
function pack(codes) {
  const out = Buffer.alloc(Math.ceil(codes.length * 3 / 8));
  codes.forEach((code, i) => {
    assert.ok(code >= 0 && code < 8);
    for (let j = 0; j < 3; j++) out[(i * 3 + j) >> 3] |= ((code >> j) & 1) << ((i * 3 + j) & 7);
  });
  return out;
}
const literalData = indices => pack([...indices].flatMap(index => [4, index]).concat(5));
const subblocks = data => cat([b([data.length]), data, b([0])]);
function image(indices, palette, local) {
  return cat([b([0x2c]), u16(0), u16(0), u16(W), u16(H), b([local ? 0x80 : 0]),
    local ? b(palette.flat()) : b([]), b([2]), subblocks(literalData(indices))]);
}
function pixels(indices, palette, transparent) {
  const rgba = Buffer.alloc(W * H * 4);
  indices.forEach((index, i) => { if (index !== transparent) rgba.set([...palette[index], 255], i * 4); });
  return rgba;
}

// Bounded, separately structured complete-container validator. It decodes the
// admitted literal-only dialect and traverses ALL images, never returns at frame 1.
// Duplicate/orphan GCEs are recorded as scope-policy probes, not spec-valid GIFs.
export function audit(gif) {
  let at = 0, pending = null;
  const frames = [], blocks = [], policy = [];
  const fail = code => { const error = new Error(code); error.auditCode = code; throw error; };
  const take = (n, code = "truncated-container") => {
    if (at + n > gif.length) fail(code);
    const out = gif.subarray(at, at + n); at += n; return out;
  };
  const subs = () => {
    const parts = [];
    for (;;) { const n = take(1, "truncated-subblocks")[0]; if (!n) return cat(parts); parts.push(take(n, "truncated-subblocks")); }
  };
  try {
    if (take(6).toString() !== "GIF89a") fail("header");
    const screen = take(7), width = screen.readUInt16LE(0), height = screen.readUInt16LE(2);
    const global = screen[4] & 128 ? take(3 * 2 ** ((screen[4] & 7) + 1), "truncated-palette") : b([]);
    for (;;) {
      const start = at, marker = take(1, "invalid-end")[0];
      if (marker === 0x3b) {
        if (at !== gif.length) fail("invalid-end");
        if (pending) policy.push("orphan-gce");
        return { valid: true, completed_images: frames.length, frames, blocks, policy };
      }
      if (marker === 0x21) {
        const label = take(1)[0];
        if (label === 0xf9) {
          const size = take(1, "truncated-gce")[0];
          if (size !== 4) fail("gce-size");
          const fields = take(5, "truncated-gce");
          if (fields[4] !== 0) fail("gce-terminator");
          if (pending) policy.push("duplicate-gce");
          pending = { transparent: fields[0] & 1 ? fields[3] : null, delay_cs: fields.readUInt16LE(1),
            user_input: !!(fields[0] & 2), disposal: (fields[0] >> 2) & 7 };
          blocks.push({ kind: "gce", offset: start, ...pending });
        } else {
          if (![1, 0xfe, 0xff].includes(label)) fail("extension-label");
          if (label !== 0xfe) {
            const size = take(1)[0]; if (size !== (label === 1 ? 12 : 11)) fail("extension-size");
            take(size);
          }
          const data = subs();
          blocks.push({ kind: label === 1 ? "plain-text" : label === 0xfe ? "comment" : "application",
            offset: start, payload_bytes: data.length, consumed_gce: label === 1 ? pending : null });
          if (label === 1) pending = null;
        }
        continue;
      }
      if (marker !== 0x2c) fail("invalid-end");
      const desc = take(9);
      const x = desc.readUInt16LE(0), y = desc.readUInt16LE(2), w = desc.readUInt16LE(4), h = desc.readUInt16LE(6);
      if (!w || !h || x + w > width || y + h > height) fail("out-of-bounds");
      const palette = desc[8] & 128 ? take(3 * 2 ** ((desc[8] & 7) + 1), "truncated-palette") : global;
      const minimumOffset = at;
      if (take(1)[0] !== 2) fail("minimum-code-size");
      const data = subs(), indices = [];
      let bit = 0, wantClear = true, ended = false;
      while (bit + 3 <= data.length * 8) {
        let code = 0;
        for (let j = 0; j < 3; j++, bit++) code |= ((data[bit >> 3] >> (bit & 7)) & 1) << j;
        if (code === 5) { ended = true; break; }
        if (wantClear) { if (code !== 4) fail("literal-lzw"); }
        else {
          if (code >= 4) fail("literal-lzw");
          if (code >= palette.length / 3) fail("palette-index");
          indices.push(code);
        }
        wantClear = !wantClear;
      }
      if (!ended) fail("missing-eoi");
      if (indices.length !== w * h) fail("pixel-count");
      if (data.length * 8 - bit >= 8) fail("lzw-padding");
      for (; bit < data.length * 8; bit++) if ((data[bit >> 3] >> (bit & 7)) & 1) fail("lzw-padding");
      frames.push({ offset: start, end_offset: at, minimum_offset: minimumOffset, width: w, height: h,
        palette_sha256: sha(palette), indices_base64: b(indices).toString("base64"),
        gce: pending, effective_transparent: pending?.transparent ?? null });
      blocks.push({ kind: "image", offset: start }); pending = null;
    }
  } catch (error) {
    if (!error.auditCode) throw error;
    return { valid: false, error: error.auditCode, offset: at, completed_images: frames.length, frames, blocks, policy };
  }
}

function author(paletteScope, pattern, name, later = false) {
  const palette = pattern === 1 ? [[23, 91, 167], [229, 47, 83]] : [[181, 113, 29], [41, 197, 139]];
  const global = paletteScope === "global" ? palette : [[7, 19, 31], [241, 223, 199]];
  const indices = b(Array.from({ length: W * H }, (_, i) => pattern === 1
    ? (i % W + Math.floor(i / W)) % 2 : ((i % W) * 3 + Math.floor(i / W) * 5 + Math.floor(i / 3)) % 2));
  const laterIndices = b([...indices].map(index => 1 - index));
  const local = paletteScope === "local", frames = [];
  const header = cat([Buffer.from("GIF89a"), u16(W), u16(H), b([0xf0, 0, 0]), b(global.flat())]);
  let chunks, firstTransparent, laterTransparent = null;
  if (later) {
    firstTransparent = name === "first-transparent-later-no-gce" ? 0 : null;
    laterTransparent = name === "later-transparent" ? 0 : null;
    const secondControl = name === "later-transparent" ? [gce(0)] : name === "later-opaque" ? [gce()] : [];
    chunks = [header, gce(firstTransparent, LONG), image(indices, palette, local), ...secondControl,
      image(laterIndices, palette, local), b([0x3b])];
  } else {
    const s = signal(name); firstTransparent = s.transparent;
    chunks = [header, ...s.before, image(indices, palette, local), ...(s.after ?? []), b([0x3b])];
  }
  const gif = cat(chunks), checked = audit(gif);
  assert.equal(checked.valid, true); assert.equal(checked.completed_images, later ? 2 : 1);
  for (const [i, data] of (later ? [indices, laterIndices] : [indices]).entries()) {
    const transparent = i ? laterTransparent : firstTransparent;
    assert.equal(checked.frames[i].indices_base64, data.toString("base64"));
    assert.equal(checked.frames[i].effective_transparent, transparent);
    assert.equal(checked.frames[i].palette_sha256, sha(b(palette.flat())));
    frames.push({ indices_base64: data.toString("base64"), indices_sha256: sha(data), transparent_index: transparent,
      expected_patch: rgbaRecord(pixels(data, palette, transparent)) });
  }
  const first = pixels(indices, palette, firstTransparent);
  const laterDisplay = later ? Buffer.from(first) : null;
  if (later) laterIndices.forEach((index, i) => {
    if (index !== laterTransparent) laterDisplay.set([...palette[index], 255], i * 4);
  });
  if (later) assert.notDeepEqual(first, laterDisplay, "later display must discriminate timing");
  const cell = `${later ? "two-frame" : "static"}-${paletteScope}-${name}`;
  return { id: `${cell}-pattern${pattern}`, cell, group: later ? "two-frame" : "static", signal: name,
    width: W, height: H, pattern, palette_scope: paletteScope, palette, global_palette: global,
    image_count: frames.length, first_delay_cs: later ? LONG : name === "user-input-long-delay" ? LONG : 0,
    scope_status: checked.policy.length ? "well-framed-nonconforming-scope-probe" : "spec-valid",
    scope_policy: checked.policy, eligibility: "collected-well-framed-probe",
    expectation_basis: "GIF89a scope hypothesis; duplicate GCE uses complete last-GCE replacement (non-normative); transparent initial canvas",
    frames, expected_first: rgbaRecord(first), expected_later_display: later ? rgbaRecord(laterDisplay) : null,
    audit: checked, ...byteRecord(gif) };
}

export function fixtures() {
  const cases = [];
  for (const scope of ["global", "local"]) for (const name of SIGNALS) for (const pattern of [1, 2])
    cases.push(author(scope, pattern, name));
  for (const scope of ["global", "local"]) for (const name of ["later-no-gce", "later-transparent", "later-opaque", "first-transparent-later-no-gce"])
    for (const pattern of [1, 2]) cases.push(author(scope, pattern, name, true));
  assert.equal(cases.length, 64);
  const base = cases.find(row => row.id === "two-frame-local-later-opaque-pattern1");
  const original = Buffer.from(base.gif_base64, "base64"), controls = [];
  const mutations = [
    ["bad-palette-index", "palette-index", (gif, f) => { gif[f.minimum_offset + 2] = (gif[f.minimum_offset + 2] & ~56) | (2 << 3); return gif; }],
    ["undefined-lzw", "literal-lzw", (gif, f) => { gif[f.minimum_offset + 2] = (gif[f.minimum_offset + 2] & ~56) | (7 << 3); return gif; }],
    ["truncated-gce", "truncated-gce", (gif, f) => gif.subarray(0, f.offset - 3)],
    ["out-of-bounds", "out-of-bounds", (gif, f) => { gif.writeUInt16LE(1, f.offset + 1); return gif; }],
    ["truncated-subblocks", "truncated-subblocks", (gif, f) => gif.subarray(0, f.minimum_offset + 5)],
    ["invalid-end", "invalid-end", (gif, f) => { gif[f.offset] = 0x7f; return gif; }],
    ["minimum-code-size-1", "minimum-code-size", (gif, f) => { gif[f.minimum_offset] = 1; return gif; }],
    ["early-eoi", "pixel-count", (gif, f) => cat([gif.subarray(0, f.minimum_offset), b([2]), subblocks(pack([4, 5])), gif.subarray(f.end_offset)])],
    ["missing-eoi", "missing-eoi", (gif, f) => cat([gif.subarray(0, f.minimum_offset), b([2]), subblocks(pack([4, 0, 4, 0, 4, 0, 4, 0])), gif.subarray(f.end_offset)])],
    ["truncated-palette", "truncated-palette", (gif, f) => gif.subarray(0, f.offset + 12)],
  ];
  for (const [name, error, mutate] of mutations) for (const frameIndex of [0, 1]) {
    const frame = base.audit.frames[frameIndex], gif = mutate(Buffer.from(original), frame), checked = audit(gif);
    assert.equal(checked.valid, false, name); assert.equal(checked.error, error, name);
    assert.equal(checked.completed_images, frameIndex, name);
    const changed = [...Array(Math.min(gif.length, original.length)).keys()].find(i => gif[i] !== original[i]) ?? Math.min(gif.length, original.length);
    if (frameIndex === 1) assert.deepEqual(gif.subarray(0, base.audit.frames[0].end_offset), original.subarray(0, base.audit.frames[0].end_offset));
    controls.push({ id: `${frameIndex ? "late" : "early"}-${name}`, base_id: base.id, base_sha256: base.gif_sha256,
      target_frame: frameIndex, mutation: name, first_changed_or_missing_offset: changed,
      first_frame_complete: frameIndex === 1, preserved_prefix_bytes: frameIndex ? base.audit.frames[0].end_offset : 0,
      preserved_prefix_sha256: sha(gif.subarray(0, frameIndex ? base.audit.frames[0].end_offset : 0)),
      intended_strict: "reject-entire-gif", intended_recovery: "image-local-recovery", collected: false,
      eligibility: "uncollected-malformed-validation-control", expectation_status: "authored bounded JS audit; independent decoder binding still required",
      audit: checked, ...byteRecord(gif) });
  }
  assert.equal(controls.length, 20);
  assert.equal(new Set([...cases, ...controls].map(row => row.gif_sha256)).size, 84);
  return { cases, controls };
}

export function scene(row) {
  const result = imageScene(row);
  result.files.asset.mimeType = "image/gif";
  result.files.asset.dataURL = `data:image/gif;base64,${row.gif_base64}`;
  return result;
}
