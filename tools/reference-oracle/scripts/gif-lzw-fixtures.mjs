// Independent wire author and audit decoder. No production GIF/codec dependency.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { scene as imageScene } from "./jpeg-icc-fixtures.mjs";

export const sha = bytes => createHash("sha256").update(bytes).digest("hex");
export const SPEC = "https://www.w3.org/Graphics/GIF/spec-gif89a.txt";
const b = values => Buffer.from(values);
const u16 = value => { const out = Buffer.alloc(2); out.writeUInt16LE(value); return out; };
const SEEDS = [0x13579bdf, 0x2468ace1];

function blocks(data, size) {
  const out = [];
  for (let i = 0; i < data.length; i += size) {
    const part = data.subarray(i, i + size); out.push(b([part.length]), part);
  }
  return Buffer.concat([...out, b([0])]);
}

// Standard longest-prefix LZW. The dictionary used to choose codes is one
// insertion ahead of the reader. Serialize using a separate reader-lag counter;
// in particular, the final phrase may grow the width BEFORE EOI is written.
function encode(indices, minimum, resetAt) {
  const clear = 1 << minimum, end = clear + 1, codes = [];
  for (const segment of resetAt ? [indices.subarray(0, resetAt), indices.subarray(resetAt)] : [indices]) {
    codes.push(clear);
    const dictionary = new Map();
    let next = end + 1, prefix = segment[0];
    for (const literal of segment.subarray(1)) {
      const key = prefix * 256 + literal;
      if (dictionary.has(key)) prefix = dictionary.get(key);
      else {
        codes.push(prefix);
        if (next < 4096) dictionary.set(key, next++);
        prefix = literal;
      }
    }
    codes.push(prefix);
  }
  codes.push(end);
  let width = minimum + 1, next = end + 1, previous = false, bit = 0;
  const packed = Buffer.alloc(Math.ceil(codes.length * 12 / 8));
  for (const code of codes) {
    assert.ok(code < 2 ** width);
    for (let j = 0; j < width; j++, bit++) packed[bit >> 3] |= ((code >> j) & 1) << (bit & 7);
    if (code === clear) { width = minimum + 1; next = end + 1; previous = false; }
    else if (code !== end) {
      if (previous && next < 4096) { next++; if (next === 2 ** width && width < 12) width++; }
      previous = true;
    }
  }
  return packed.subarray(0, Math.ceil(bit / 8));
}

// Audit actual packed bytes with a separate string-table decoder. This is a
// pre-collection author check, not the pixel expectation authority or Rust oracle.
function audit(packed, minimum, expected) {
  const clear = 2 ** minimum, end = clear + 1;
  let table, width, bit = 0, previous = null, output = [], ended = false;
  const stats = { widths: [], code_count: 0, clear_count: 0, dictionary_references: 0,
    kwkwk_count: 0, max_next_code: 0, full_dictionary_codes: 0, events: [] };
  const reset = () => { table = Array.from({ length: end + 1 }, (_, i) => i < clear ? [i] : null); width = minimum + 1; previous = null; };
  reset();
  while (bit + width <= packed.length * 8) {
    const start = bit, usedWidth = width;
    let code = 0;
    for (let j = 0; j < usedWidth; j++, bit++) code |= ((packed[bit >> 3] >> (bit & 7)) & 1) << j;
    stats.code_count++;
    if (!stats.widths.includes(usedWidth)) stats.widths.push(usedWidth);
    if (code === clear) {
      stats.clear_count++; stats.events.push({ kind: "clear", bit: start, width: usedWidth, output_pixels: output.length }); reset(); continue;
    }
    if (code === end) { ended = true; break; }
    let entry;
    if (code < table.length && table[code]) entry = table[code];
    else {
      assert.ok(code === table.length && previous, `undefined code ${code} at bit ${start}`);
      entry = [...previous, previous[0]]; stats.kwkwk_count++;
    }
    if (code > end) stats.dictionary_references++;
    if (table.length === 4096) stats.full_dictionary_codes++;
    output.push(...entry);
    assert.ok(output.length <= expected.length, "excess decoded indices");
    if (previous && table.length < 4096) {
      table.push([...previous, entry[0]]);
      if (table.length === 2 ** width && width < 12) {
        width++; stats.events.push({ kind: "width", bit: bit, width, next_code: table.length, output_pixels: output.length });
      }
    }
    stats.max_next_code = Math.max(stats.max_next_code, table.length);
    previous = entry;
  }
  assert.ok(ended, "missing EOI");
  assert.deepEqual(b(output), expected, "packed stream differs from authored indices");
  assert.ok(packed.length * 8 - bit < 8, "bytes after EOI");
  for (; bit < packed.length * 8; bit++) assert.equal((packed[bit >> 3] >> (bit & 7)) & 1, 0);
  return stats;
}

function pattern(length, entries, program, seed) {
  const out = Buffer.alloc(length);
  let state = seed, run = 0, at = 0;
  if (program === "kwkwk") {
    // Repeated symbols force literal K followed by the not-yet-defined KK
    // code; varying runs and palette indices give distinguishable fixtures.
    while (at < length) {
      const count = 3 + ((run * 17 + (seed & 31)) % 251);
      out.fill((run * 13 + (seed >>> 16)) % entries, at, Math.min(length, at + count));
      at += count; run++;
    }
  } else for (let i = 0; i < length; i++) {
    state ^= state << 13; state ^= state >>> 17; state ^= state << 5;
    out[i] = (state >>> 16) % entries;
  }
  return out;
}

function extension(kind) {
  const comment = Buffer.from("independent GIF comment / ".repeat(12));
  if (kind.startsWith("comment")) return Buffer.concat([b([0x21, 0xfe]), blocks(comment, kind.endsWith("split") ? 1 : 255)]);
  if (kind.startsWith("unknown-app")) return Buffer.concat([b([0x21, 0xff, 11]), Buffer.from("ORACLEFX010"), blocks(b([0, 0x21, 0x2c, 0x3b, 255, 7]), 1)]);
  if (kind.startsWith("netscape")) return Buffer.concat([b([0x21, 0xff, 11]), Buffer.from("NETSCAPE2.0"), b([3, 1]), u16(kind === "netscape-finite" ? 2 : 0), b([0])]);
  assert.equal(kind, "plain-text-before");
  // A graphic-rendering extension, not a comment. No GCE and no delay. The
  // following full-screen opaque image overwrites the entire text grid.
  return Buffer.concat([b([0x21, 1, 12]), u16(0), u16(0), u16(8), u16(8), b([8, 8, 1, 0]), blocks(Buffer.from("A"), 1)]);
}

// Walk the finished container rather than trusting the encoder's offsets or
// sub-block assembly. In particular an extension size error must fail before
// collecting a browser's permissive recovery as reference evidence.
function checkContainer(gif, version, width, height, palette, minimum, packed, extensionKind) {
  assert.equal(gif.subarray(0, 6).toString(), `GIF${version}`);
  assert.equal(gif.readUInt16LE(6), width); assert.equal(gif.readUInt16LE(8), height);
  assert.equal(2 ** ((gif[10] & 7) + 1), palette.length);
  assert.deepEqual(gif.subarray(13, 13 + palette.length * 3), b(palette.flat()));
  let at = 13 + palette.length * 3, images = 0, extensions = 0;
  const take = count => { assert.ok(at + count <= gif.length); const part = gif.subarray(at, at + count); at += count; return part; };
  const subblocks = () => {
    const parts = [];
    for (;;) { const count = take(1)[0]; if (count === 0) return Buffer.concat(parts); parts.push(take(count)); }
  };
  for (;;) {
    const marker = take(1)[0];
    if (marker === 0x3b) break;
    if (marker === 0x21) {
      assert.equal(version, "89a"); extensions++;
      const label = take(1)[0];
      if (label === 0xff || label === 1) {
        const size = take(1)[0]; assert.equal(size, label === 1 ? 12 : 11); take(size);
      } else assert.equal(label, 0xfe);
      subblocks();
    } else {
      assert.equal(marker, 0x2c); images++;
      const descriptor = take(9);
      assert.equal(descriptor.readUInt32LE(0), 0);
      assert.equal(descriptor.readUInt16LE(4), width); assert.equal(descriptor.readUInt16LE(6), height);
      assert.equal(descriptor[8], 0); assert.equal(take(1)[0], minimum);
      assert.deepEqual(subblocks(), packed);
    }
  }
  assert.equal(at, gif.length); assert.equal(images, 1); assert.equal(extensions, extensionKind ? 1 : 0);
}

function author(entries, version, program, blockSize, patternIndex, extensionKind = null) {
  // Binary entropy needs more pixels to fill 4096 entries; 12513 is not enough.
  const width = entries === 2 ? 257 : 129, height = entries === 2 ? 193 : 97;
  const minimum = Math.max(2, Math.log2(entries)), seed = SEEDS[patternIndex];
  const palette = Array.from({ length: entries }, (_, i) => [(i * 73 + 19 + patternIndex * 31) & 255,
    (i * 151 + 47 + patternIndex * 17) & 255, (i * 199 + 101 + patternIndex * 43) & 255]);
  const indices = pattern(width * height, entries, program, seed);
  const resetAt = program === "midstream-reset" ? Math.floor(indices.length / 2) : null;
  const packed = encode(indices, minimum, resetAt), stats = audit(packed, minimum, indices);
  assert.ok(stats.dictionary_references > 0);
  assert.equal(stats.clear_count, resetAt ? 2 : 1);
  if (program === "growth-to-full") { assert.equal(stats.max_next_code, 4096); assert.ok(stats.full_dictionary_codes > 0); }
  if (program !== "kwkwk") assert.equal(Math.max(...stats.widths), 12);
  if (program === "kwkwk") assert.ok(stats.kwkwk_count > 0);
  const ext = extensionKind ? extension(extensionKind) : Buffer.alloc(0);
  const after = extensionKind?.includes("after");
  const header = Buffer.concat([Buffer.from(`GIF${version}`), u16(width), u16(height), b([0xf0 | (Math.log2(entries) - 1), 0, 0]), b(palette.flat())]);
  const descriptor = Buffer.concat([b([0x2c]), u16(0), u16(0), u16(width), u16(height), b([0])]);
  const image = Buffer.concat([descriptor, b([minimum]), blocks(packed, blockSize)]);
  const gif = Buffer.concat([header, after ? Buffer.alloc(0) : ext, image, after ? ext : Buffer.alloc(0), b([0x3b])]);
  checkContainer(gif, version, width, height, palette, minimum, packed, extensionKind);
  const rgba = Buffer.alloc(indices.length * 4);
  indices.forEach((index, i) => rgba.set([...palette[index], 255], i * 4));
  const cell = extensionKind ? `extension-${extensionKind}` : `p${entries}-${version}-${program}-b${blockSize}`;
  return { id: extensionKind ? cell : `${cell}-seed${patternIndex + 1}`, cell, group: extensionKind ? "extension" : "matrix",
    width, height, palette_entries: entries, version, program, subblock_size: blockSize,
    pattern: patternIndex + 1, seed, palette, minimum_code_size: minimum, reset_at_pixel: resetAt,
    extension: extensionKind, image_count: 1, gce_count: 0, interlaced: false, transparent: false,
    authored_indices_base64: indices.toString("base64"), authored_indices_sha256: sha(indices),
    expected_first: { rgba_base64: rgba.toString("base64"), rgba_sha256: sha(rgba) },
    stream: { ...stats, packed_bytes: packed.length, packed_sha256: sha(packed),
      subblocks: Math.ceil(packed.length / blockSize), audit: "separate JS table decoder equals authored indices; EOI and zero padding checked" },
    offsets: { image: header.length + (after ? 0 : ext.length), minimum: header.length + (after ? 0 : ext.length) + 10 },
    gif_base64: gif.toString("base64"), gif_sha256: sha(gif), gif_bytes: gif.length,
    eligibility: "valid-static-reference", container_valid: true };
}

export function fixtures() {
  const cases = [];
  for (const entries of [2, 16, 256]) for (const version of ["87a", "89a"])
    for (const program of ["growth-to-full", "midstream-reset", "kwkwk"])
      for (const blockSize of [1, 255]) for (const patternIndex of [0, 1])
        cases.push(author(entries, version, program, blockSize, patternIndex));
  assert.equal(cases.length, 72); assert.equal(new Set(cases.map(row => row.cell)).size, 36);
  for (const kind of ["comment-before", "comment-after", "comment-before-split", "comment-after-split",
    "unknown-app-before", "netscape-infinite", "netscape-finite", "plain-text-before"])
    cases.push(author(16, "89a", "growth-to-full", 255, 0, kind));
  assert.equal(cases.length, 80); assert.equal(new Set(cases.map(row => row.gif_sha256)).size, 80);
  const base = cases.find(row => row.id === "p2-89a-growth-to-full-b255-seed1");
  const original = Buffer.from(base.gif_base64, "base64"), start = base.offsets.minimum;
  const controls = [];
  function control(id, mutate, reason) {
    const gif = mutate(Buffer.from(original));
    controls.push({ id, cell: id, base_id: base.id, gif_base64: gif.toString("base64"), gif_sha256: sha(gif), gif_bytes: gif.length,
      kind: "malformed", eligibility: "uncollected-parent-control", collected: false, reason,
      intended_strict: "reject-malformed", intended_recovery: "image-local-recovery",
      expectation_status: "authored strict-policy intent; requires independent parent checker; not browser evidence" });
  }
  const replaceStream = data => Buffer.concat([original.subarray(0, start), data, b([0x3b])]);
  control("minimum-code-size-1", gif => { gif[start] = 1; return gif; }, "Minimum code size 1 is forbidden even for two-color images.");
  control("minimum-code-size-9", gif => { gif[start] = 9; return gif; }, "Minimum code size exceeds 8.");
  control("undefined-initial-code", () => replaceStream(b([2, 2, 0x7c, 1, 0])), "3-bit codes [clear=4, undefined=7, EOI=5].");
  control("undefined-after-literal", () => replaceStream(b([2, 2, 0xc4, 0x0b, 0])), "3-bit codes [4,0,7,5]; next=6 so 7 is not KwKwK.");
  control("missing-eoi", () => replaceStream(b([2, 3, 0x04, 0x41, 0x10, 0])), "Exactly eight 3-bit codes [4,0,4,0,4,0,4,0], no EOI or padding ambiguity.");
  control("truncated-subblock", gif => gif.subarray(0, start + 12), "First subblock declares 255 bytes but file ends inside it.");
  control("missing-subblock-terminator", gif => Buffer.concat([gif.subarray(0, -2), gif.subarray(-1)]), "Complete LZW payload; absent image-data terminator.");
  control("early-eoi", () => replaceStream(b([2, 1, 0x2c, 0])), "[clear=4,EOI=5] emits zero pixels for a nonempty image.");
  control("excess-pixels", gif => { gif.writeUInt16LE(1, base.offsets.image + 5); gif.writeUInt16LE(1, base.offsets.image + 7); return gif; }, "Valid stream emits 49601 indices for a 1x1 descriptor.");
  control("missing-trailer", gif => gif.subarray(0, -1), "Mandatory trailer absent after complete image data.");
  control("gif87a-comment", gif => { gif.write("87a", 3); return Buffer.concat([gif.subarray(0, base.offsets.image), extension("comment-before"), gif.subarray(base.offsets.image)]); }, "Comment requires 89a; strict authored version control, no browser rejection claim.");
  control("gif87a-application", gif => { gif.write("87a", 3); return Buffer.concat([gif.subarray(0, base.offsets.image), extension("netscape-infinite"), gif.subarray(base.offsets.image)]); }, "Application extension requires 89a; strict authored version control, no browser rejection claim.");
  assert.equal(controls.length, 12);
  return { cases, controls };
}

export function scene(row) {
  const result = imageScene(row);
  result.files.asset.mimeType = "image/gif";
  result.files.asset.dataURL = `data:image/gif;base64,${row.gif_base64}`;
  return result;
}
