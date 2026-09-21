// Independently authored pixels, RIFF/chunk framing, animation headers and hypotheses.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { scene as imageScene } from "./jpeg-icc-fixtures.mjs";

export const sha = bytes => createHash("sha256").update(bytes).digest("hex");
export const packed = bytes => ({ base64: bytes.toString("base64"), sha256: sha(bytes), bytes: bytes.length });
export const INITIAL_DURATION_MS = 0xffffff;
export const OCI = "mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9";
const u24 = n => { const b = Buffer.alloc(3); b.writeUIntLE(n, 0, 3); return b; };
const u32 = n => { const b = Buffer.alloc(4); b.writeUInt32LE(n); return b; };
const chunk = (kind, body) => Buffer.concat([Buffer.from(kind), u32(body.length), body, Buffer.alloc(body.length & 1)]);
const riff = chunks => { const body = Buffer.concat([Buffer.from("WEBP"), ...chunks]); return Buffer.concat([Buffer.from("RIFF"), u32(body.length), body]); };

export function chunks(bytes, start = 12, end = bytes.length) {
  const rows = [];
  for (let offset = start; offset < end;) {
    assert.ok(offset + 8 <= end);
    const kind = bytes.toString("ascii", offset, offset + 4), size = bytes.readUInt32LE(offset + 4);
    const next = offset + 8 + size + (size & 1);
    assert.ok(next <= end);
    if (size & 1) assert.equal(bytes[next - 1], 0);
    rows.push({ kind, offset, payload_offset: offset + 8, size, padded_end: next }); offset = next;
  }
  return rows;
}

function pixels(width, height, alpha, pattern, later = false) {
  const rgba = Buffer.alloc(width * height * 4);
  const ramp = [0, 1, 17, 63, 127, 128, 191, 254, 255];
  for (let y = 0; y < height; y++) for (let x = 0; x < width; x++) {
    const i = (y * width + x) * 4;
    const rgb = pattern === "ramp" ? [ramp[(x + 2 * y) % 9], ramp[(3 * x + y + 2) % 9], ramp[(x + 5 * y + 4) % 9]]
      : [(x < 3 ? 239 : 19) + (y % 3), (y < 2 ? 31 : 211) + (x % 5), (x === y ? 229 : 67)];
    const a = alpha === "opaque" ? 255 : alpha === "binary" ? ((x + 3 * y) % 3 ? 255 : 0) : ramp[(x + 3 * y) % 9];
    // Intentional nonzero RGB underneath zero alpha, retained in original source.
    rgba.set([...(a === 0 ? [173, 59, 227] : rgb.map(v => later ? 255 - v : v)), a], i);
  }
  return rgba;
}

export function frameRequests() {
  const rows = [];
  for (const codec of ["lossless", "lossy"]) for (const alpha of ["opaque", "partial", "binary"])
    for (const shape of ["full", "subrect"]) for (const pattern of ["ramp", "edges"]) {
      const width = shape === "full" ? 9 : 5, height = shape === "full" ? 7 : 3;
      rows.push({ id: `${codec}-${alpha}-${shape}-${pattern}`, codec, alpha, shape, pattern, width, height,
        rgba_base64: pixels(width, height, alpha, pattern).toString("base64") });
    }
  for (const codec of ["lossless", "lossy"]) for (const pattern of ["ramp", "edges"])
    rows.push({ id: `${codec}-later-${pattern}`, codec, alpha: "opaque", shape: "full", pattern, width: 9, height: 7,
      rgba_base64: pixels(9, 7, "opaque", pattern, true).toString("base64") });
  return rows;
}

function payload(frame) {
  const b = Buffer.from(frame.encoded.base64, "base64");
  assert.equal(sha(b), frame.encoded.sha256);
  assert.equal(b.toString("ascii", 0, 4), "RIFF"); assert.equal(b.toString("ascii", 8, 12), "WEBP");
  assert.equal(b.readUInt32LE(4) + 8, b.length);
  const rows = chunks(b).filter(r => ["ALPH", "VP8 ", "VP8L"].includes(r.kind));
  assert.deepEqual(rows.map(r => r.kind), frame.codec === "lossless" ? ["VP8L"] : frame.alpha === "opaque" ? ["VP8 "] : ["ALPH", "VP8 "]);
  return Buffer.concat(rows.map(r => b.subarray(r.offset, r.padded_end)));
}

// A static WebP has no subrectangle offset. Six explicit configurations avoid
// inventing an invalid static/subrect cross product. Both patterns repeat each.
export const CONFIGURATIONS = [
  { id: "static-full", animated: false, shape: "full", blend: false, disposal: 0, background: [0, 0, 0, 0] },
  { id: "anim-full-blend-d0-clear", animated: true, shape: "full", blend: true, disposal: 0, background: [0, 0, 0, 0] },
  { id: "anim-full-replace-d1-opaque", animated: true, shape: "full", blend: false, disposal: 1, background: [41, 83, 137, 255] },
  { id: "anim-sub-blend-d1-opaque", animated: true, shape: "subrect", blend: true, disposal: 1, background: [41, 83, 137, 255] },
  { id: "anim-sub-replace-d0-clear", animated: true, shape: "subrect", blend: false, disposal: 0, background: [0, 0, 0, 0] },
  { id: "anim-sub-replace-d1-opaque", animated: true, shape: "subrect", blend: false, disposal: 1, background: [41, 83, 137, 255] },
];

export function compose(rgba, rect, background = [0, 0, 0, 0], blend = false) {
  const out = Buffer.alloc(9 * 7 * 4);
  for (let i = 0; i < out.length; i += 4) out.set(background, i);
  for (let y = 0; y < rect.height; y++) for (let x = 0; x < rect.width; x++) {
    const src = (y * rect.width + x) * 4, dst = ((y + rect.top) * 9 + x + rect.left) * 4;
    const a = rgba[src + 3], ba = blend ? background[3] : 0, oa = a + ba * (255 - a) / 255;
    for (let c = 0; c < 3; c++) out[dst + c] = oa ? Math.round((rgba[src + c] * a + background[c] * ba * (255 - a) / 255) / oa) : 0;
    out[dst + 3] = Math.round(oa);
  }
  return out;
}

export function fixtures(inputs) {
  assert.deepEqual(inputs.frames.map(({ encoded, native_decoded_rgba, ...request }) => request), frameRequests());
  const frames = new Map(inputs.frames.map(f => [f.id, f])), cases = [];
  for (const codec of ["lossless", "lossy"]) for (const alpha of ["opaque", "partial", "binary"])
    for (const config of CONFIGURATIONS) for (const pattern of ["ramp", "edges"]) {
      const first = frames.get(`${codec}-${alpha}-${config.shape}-${pattern}`), later = frames.get(`${codec}-later-${pattern}`);
      const rect = { left: config.shape === "full" ? 0 : 2, top: config.shape === "full" ? 0 : 2, width: first.width, height: first.height };
      const firstPayload = payload(first), laterPayload = payload(later);
      const vp8x = chunk("VP8X", Buffer.concat([Buffer.from([(alpha !== "opaque" ? 0x10 : 0) | (config.animated ? 2 : 0), 0, 0, 0]), u24(8), u24(6)]));
      const anmf = (r, data, duration, flags) => chunk("ANMF", Buffer.concat([u24(r.left / 2), u24(r.top / 2), u24(r.width - 1), u24(r.height - 1), u24(duration), Buffer.from([flags]), data]));
      const bg = config.background;
      const webp = config.animated ? riff([vp8x, chunk("ANIM", Buffer.from([bg[2], bg[1], bg[0], bg[3], 0, 0])),
        anmf(rect, firstPayload, INITIAL_DURATION_MS, (config.blend ? 0 : 2) | config.disposal),
        anmf({ left: 0, top: 0, width: 9, height: 7 }, laterPayload, 100, 2)])
        : riff([...(codec === "lossy" && alpha !== "opaque" ? [vp8x] : []), firstPayload]);
      const original = Buffer.from(first.rgba_base64, "base64"), laterOriginal = Buffer.from(later.rgba_base64, "base64");
      const transparent = compose(original, rect), background = compose(original, rect, bg, config.blend);
      const laterExpected = compose(laterOriginal, { left: 0, top: 0, width: 9, height: 7 });
      assert.notEqual(sha(transparent), sha(laterExpected));
      const layout = chunks(webp).map(r => ({ ...r, ...(r.kind === "ANMF" ? { children: chunks(webp, r.payload_offset + 16, r.payload_offset + r.size) } : {}) }));
      cases.push({ id: `${codec}-${alpha}-${config.id}-${pattern}`, cell: `${codec}-${alpha}-${config.id}`, codec, alpha, pattern,
        width: 9, height: 7, config, first_rect: rect, frame_count: config.animated ? 2 : 1,
        first_duration_ms: config.animated ? INITIAL_DURATION_MS : null, first_frame_id: first.id, later_frame_id: config.animated ? later.id : null,
        frame_payload: packed(firstPayload), later_payload: config.animated ? packed(laterPayload) : null,
        original_rgba: packed(original), expected_first_transparent: packed(transparent), expected_first_anim_background: packed(background),
        expected_later: config.animated ? packed(laterExpected) : null, webp: packed(webp), layout,
        eligibility: "valid-first-frame-reference" });
    }
  assert.equal(cases.length, 72); assert.equal(new Set(cases.map(r => r.cell)).size, 36);
  assert.equal(new Set(cases.map(r => r.webp.sha256)).size, 72);
  const base = cases.find(r => r.id === "lossless-partial-anim-sub-blend-d1-opaque-ramp");
  const original = Buffer.from(base.webp.base64, "base64"), layout = base.layout;
  const first = layout.find(r => r.kind === "ANMF"), later = layout.filter(r => r.kind === "ANMF")[1];
  const controls = [];
  const add = (id, reason, mutate) => { const b = mutate(Buffer.from(original)); controls.push({ id, base_id: base.id, kind: "malformed",
    collected: false, eligibility: "uncollected-parent-control", reason, intended_strict: "reject-malformed",
    intended_recovery: "image-local-recovery; parent must bind policy", webp: packed(b) }); };
  add("riff-size-short", "RIFF declared size two bytes short", b => { b.writeUInt32LE(b.length - 10, 4); return b; });
  add("riff-size-long", "RIFF declared size two bytes beyond input", b => { b.writeUInt32LE(b.length - 6, 4); return b; });
  add("chunk-length-overflow", "First ANMF claims 0xffffffff bytes", b => { b.writeUInt32LE(0xffffffff, first.offset + 4); return b; });
  add("vp8x-reserved-flag", "Reserved VP8X flag set", b => { b[20] |= 0x80; return b; });
  add("vp8x-reserved-byte", "Reserved VP8X byte nonzero", b => { b[21] = 1; return b; });
  add("anmf-reserved-flag", "Reserved ANMF flag set", b => { b[first.payload_offset + 15] |= 4; return b; });
  add("frame-outside-canvas", "First offset x=8 plus width=5 exceeds canvas=9", b => { b.writeUIntLE(4, first.payload_offset, 3); return b; });
  add("intrinsic-frame-mismatch", "ANMF width=4 differs from VP8L intrinsic width=5", b => { b.writeUIntLE(3, first.payload_offset + 6, 3); return b; });
  add("missing-anim", "Animation flag and ANMF exist without ANIM", b => { const a = layout.find(r => r.kind === "ANIM"); const out = Buffer.concat([b.subarray(0, a.offset), b.subarray(a.padded_end)]); out.writeUInt32LE(out.length - 8, 4); return out; });
  add("first-invalid-payload", "Invalid first VP8L signature", b => { b[first.children[0].payload_offset] = 0; return b; });
  add("later-invalid-payload", "Valid first frame; invalid later VP8L signature", b => { b[later.children[0].payload_offset] = 0; return b; });
  add("truncated-later-chunk", "Later decoder payload truncated with original lengths retained", b => b.subarray(0, -7));
  assert.equal(controls.length, 12);
  return { cases, controls };
}

export function scene(row) {
  const result = imageScene(row);
  result.files.asset.mimeType = "image/webp";
  result.files.asset.dataURL = `data:image/webp;base64,${row.webp.base64}`;
  return result;
}
