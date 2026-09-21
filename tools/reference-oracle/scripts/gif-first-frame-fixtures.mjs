// Project-authored GIF wire bytes and pixel expectations; no GIF codec imports.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { scene as jpegScene } from "./jpeg-icc-fixtures.mjs";

export const sha = bytes => createHash("sha256").update(bytes).digest("hex");
export const INITIAL_DELAY_CS = 65535;
const GLOBAL = [[239, 31, 73], [19, 211, 97], [43, 67, 229], [251, 193, 17]];
const LOCAL = [[13, 173, 241], [227, 53, 181], [113, 239, 29], [71, 11, 157]];
const u16 = n => { const b = Buffer.alloc(2); b.writeUInt16LE(n); return b; };
const bytes = values => Buffer.from(values);

// Minimum code size 2: clear=4, EOI=5. Clear before EVERY literal means
// no dictionary insertion can increase code width: every code is three bits.
function literalStream(indices) {
  const codes = indices.flatMap(index => { assert.ok(index >= 0 && index < 4); return [4, index]; });
  codes.push(5);
  const packed = Buffer.alloc(Math.ceil(codes.length * 3 / 8));
  codes.forEach((code, i) => {
    const bit = i * 3;
    packed[bit >> 3] |= code << (bit & 7);
    if ((bit & 7) > 5) packed[(bit >> 3) + 1] |= code >> (8 - (bit & 7));
  });
  const blocks = [];
  // Deliberately split codes across sub-block boundaries.
  for (let i = 0; i < packed.length; i += 17) {
    const block = packed.subarray(i, i + 17); blocks.push(bytes([block.length]), block);
  }
  return Buffer.concat([bytes([2]), ...blocks, bytes([0])]);
}

function rowOrder(height, interlaced) {
  if (!interlaced) return Array.from({ length: height }, (_, y) => y);
  const rows = [];
  for (const [start, step] of [[0, 8], [4, 8], [2, 4], [1, 2]]) {
    for (let y = start; y < height; y += step) rows.push(y);
  }
  assert.equal(new Set(rows).size, height);
  return rows;
}

function patternRows(width, height, pattern, later = false) {
  return Array.from({ length: height }, (_, y) => Array.from({ length: width }, (_, x) => {
    const index = pattern === "alternating"
      ? (x + 2 * y + (x === 1 && y === 2 ? 1 : 0)) % 4
      : (3 * x + y + Math.floor(x / 3) + (y === 1 ? 2 : 0)) % 4;
    return (index + (later ? 1 : 0)) % 4;
  }));
}

function frame(rect, indices, local, interlaced, transparent, disposal, delay) {
  const gce = bytes([0x21, 0xf9, 4, (disposal << 2) | Number(transparent), delay & 255, delay >> 8, 0, 0]);
  const descriptor = Buffer.concat([bytes([0x2c]), u16(rect.left), u16(rect.top), u16(rect.width), u16(rect.height),
    bytes([(local ? 0x81 : 0) | (interlaced ? 0x40 : 0)])]);
  const palette = local ? bytes(LOCAL.flat()) : Buffer.alloc(0);
  const stream = literalStream(rowOrder(rect.height, interlaced).flatMap(y => indices[y]));
  return { gce, descriptor, palette, stream };
}

// Expectations use display-order coordinates, never packed/interlaced bytes or a decoder.
function expectedRgba(width, height, rect, indices, palette, transparent) {
  const rgba = Buffer.alloc(width * height * 4); // transparent logical screen, NOT background-index RGB
  for (let y = 0; y < rect.height; y++) for (let x = 0; x < rect.width; x++) {
    const index = indices[y][x];
    if (transparent && index === 0) continue;
    rgba.set([...palette[index], 255], ((rect.top + y) * width + rect.left + x) * 4);
  }
  return { rgba_base64: rgba.toString("base64"), rgba_sha256: sha(rgba) };
}

function author(paletteMode, placement, transparent, interlaced, animation, pattern) {
  const width = 9, height = 7;
  const rect = placement === "full" ? { left: 0, top: 0, width, height } : { left: 2, top: 1, width: 5, height: 5 };
  const global = paletteMode !== "local", local = paletteMode !== "global";
  const palette = local ? LOCAL : GLOBAL;
  const disposal = animation === "static" ? 0 : Number(animation.slice(-1));
  const indices = patternRows(rect.width, rect.height, pattern);
  const header = Buffer.concat([Buffer.from("GIF89a"), u16(width), u16(height), bytes([global ? 0x91 : 0x10, 3, 0]),
    global ? bytes(GLOBAL.flat()) : Buffer.alloc(0)]);
  const first = frame(rect, indices, local, interlaced, transparent, disposal, INITIAL_DELAY_CS);
  const laterRect = { left: 0, top: 0, width, height };
  const laterIndices = patternRows(width, height, pattern, true);
  const later = animation === "static" ? null : frame(laterRect, laterIndices, local, interlaced, false, 1, 100);
  const pieces = [header, ...Object.values(first), ...(later ? Object.values(later) : []), bytes([0x3b])];
  const gif = Buffer.concat(pieces);
  const cell = `${paletteMode}-${placement}-${transparent ? "transparent" : "opaque"}-${interlaced ? "interlaced" : "sequential"}-${animation}`;
  const expected = expectedRgba(width, height, rect, indices, palette, transparent);
  const laterExpected = later ? expectedRgba(width, height, laterRect, laterIndices, palette, false) : null;
  if (later) assert.notEqual(expected.rgba_sha256, laterExpected.rgba_sha256);
  const firstDescriptor = header.length + first.gce.length;
  const firstStream = firstDescriptor + first.descriptor.length + first.palette.length;
  const secondGce = header.length + Object.values(first).reduce((n, b) => n + b.length, 0);
  return {
    id: `${cell}-${pattern}`, cell, width, height, palette_mode: paletteMode, placement,
    transparent, transparent_index: transparent ? 0 : null, interlaced, animation, first_disposal: disposal, pattern,
    global_palette: global ? GLOBAL : null, local_palette: local ? LOCAL : null,
    background_index: 3, background_rgb: global ? GLOBAL[3] : null,
    first_rect: rect, first_indices_rows: indices, first_wire_row_order: rowOrder(rect.height, interlaced),
    initial_delay_cs: INITIAL_DELAY_CS, frame_count: later ? 2 : 1,
    expected_first: expected, expected_later: laterExpected,
    later_rect: later ? laterRect : null, later_indices_rows: later ? laterIndices : null,
    offsets: { first_gce: header.length, first_descriptor: firstDescriptor, first_stream: firstStream,
      later_stream: later ? secondGce + later.gce.length + later.descriptor.length + later.palette.length : null },
    gif_base64: gif.toString("base64"), gif_sha256: sha(gif), gif_bytes: gif.length,
    eligibility: "valid-first-frame-reference", container_valid: true,
  };
}

export function fixtures() {
  const cases = [];
  for (const palette of ["global", "local", "both-local-override"])
    for (const placement of ["full", "subrect"])
      for (const transparent of [false, true]) for (const interlaced of [false, true])
        for (const animation of ["static", "animated-d0", "animated-d1", "animated-d2", "animated-d3"])
          for (const pattern of ["alternating", "asymmetric"]) cases.push(author(palette, placement, transparent, interlaced, animation, pattern));
  assert.equal(cases.length, 240);
  assert.equal(new Set(cases.map(row => row.cell)).size, 120);
  assert.equal(new Set(cases.map(row => row.gif_sha256)).size, 240);

  const base = cases.find(row => row.id === "global-full-opaque-sequential-animated-d0-asymmetric");
  const original = Buffer.from(base.gif_base64, "base64"), o = base.offsets;
  const controls = [];
  function control(id, mutate, reason, resource = null) {
    const gif = mutate(Buffer.from(original));
    controls.push({ id, cell: id, base_id: base.id, gif_base64: gif.toString("base64"), gif_sha256: sha(gif),
      gif_bytes: gif.length, eligibility: "uncollected-parent-control", collected: false,
      kind: resource ? "resource" : "malformed", reason, resource_control: resource,
      intended_strict: resource ? "resource-limit" : "reject-malformed",
      intended_recovery: resource ? "parent-resource-policy" : "image-local-recovery",
      expectation_status: "authored control intent; parent must bind to public policy; not a browser observation" });
  }
  control("bad-palette-index", b => { b[10] = 0x90; return Buffer.concat([b.subarray(0, 19), b.subarray(25)]); }, "Two-entry GCT, but valid literal stream uses indices 2 and 3.");
  control("later-stream-invalid-code", b => { b[o.later_stream + 2] = 0x3c; return b; }, "Second frame begins clear=4 then undefined code=7; first frame is complete.");
  control("missing-trailer", b => b.subarray(0, -1), "Missing mandatory GIF trailer after complete frames.");
  control("invalid-gce-size", b => { b[o.first_gce + 2] = 3; return b; }, "GCE block size is 3 rather than 4.");
  control("reserved-gce-bits", b => { b[o.first_gce + 3] |= 0x80; return b; }, "Nonzero reserved GCE bit.");
  control("reserved-image-bits", b => { b[o.first_descriptor + 9] |= 0x08; return b; }, "Nonzero reserved image-descriptor bit.");
  control("reserved-disposal-4", b => { b[o.first_gce + 3] = 4 << 2; return b; }, "Disposal 4 is reserved, excluded from valid matrix.");
  control("first-stream-invalid-code", b => { b[o.first_stream + 2] = 0x3c; return b; }, "First frame begins clear=4 then undefined code=7.");
  control("truncated-image-data", b => b.subarray(0, o.first_stream + 6), "First image sub-block truncated before declared length.");
  control("missing-color-table", b => { b[10] &= 0x7f; return Buffer.concat([b.subarray(0, 13), b.subarray(25)]); }, "Neither global nor local color table.");
  control("zero-logical-width", b => { b.writeUInt16LE(0, 6); return b; }, "Zero logical-screen width.");
  control("frame-outside-screen", b => { b.writeUInt16LE(8, o.first_descriptor + 1); return b; }, "First rectangle extends beyond logical screen.");
  control("input-byte-budget", b => b, "Valid bytes under a deliberately insufficient input-byte limit.", { measure: "input_bytes", required: original.length, limit: original.length - 1 });
  control("logical-pixel-budget", b => b, "Valid 9x7 logical screen under a 62-pixel limit.", { measure: "logical_screen_pixels", required: 63, limit: 62 });
  control("huge-logical-screen", b => { b.writeUInt16LE(65535, 6); b.writeUInt16LE(65535, 8); return b; }, "Tiny stream declares a 65535x65535 logical screen; reject before allocation.", { measure: "logical_screen_pixels", declared: 4294836225, limit: 63 });
  control("huge-image-descriptor", b => { b.writeUInt16LE(65535, o.first_descriptor + 5); b.writeUInt16LE(65535, o.first_descriptor + 7); return b; }, "Oversized descriptor also exceeds screen; parent checks validation/resource precedence before allocation.", { measure: "image_descriptor_pixels", declared: 4294836225, limit: 63 });
  assert.equal(controls.length, 16);
  return { cases, controls };
}

export function scene(row) {
  const result = jpegScene(row);
  result.files.asset.mimeType = "image/gif";
  result.files.asset.dataURL = `data:image/gif;base64,${row.gif_base64}`;
  return result;
}
