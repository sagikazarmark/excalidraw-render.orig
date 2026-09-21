// Authored ICC matrix/TRC and APP2 bytes, independent of renderer/CMS code.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
const sha = b => createHash("sha256").update(b).digest("hex");
const u16 = n => { const b = Buffer.alloc(2); b.writeUInt16BE(n); return b; };
const u32 = n => { const b = Buffer.alloc(4); b.writeUInt32BE(n); return b; };
const fixed = n => { const b = Buffer.alloc(4); b.writeInt32BE(Math.round(n * 65536)); return b; };
const xyz = values => Buffer.concat([Buffer.from("XYZ \0\0\0\0"), ...values.map(fixed)]);
function profile(model, version, gamma) {
  const curve = version === 2
    ? Buffer.concat([Buffer.from("curv\0\0\0\0"), u32(1), u16(Math.round(gamma * 256)), u16(0)])
    : Buffer.concat([Buffer.from("para\0\0\0\0"), u16(0), u16(0), fixed(gamma)]);
  const tags = [["wtpt", xyz([.9642, 1, .8249])]];
  if (model === "gray") tags.push(["kTRC", curve]);
  else tags.push(["rXYZ", xyz([.436035, .222488, .013916])], ["gXYZ", xyz([.385117, .716904, .097061])],
    ["bXYZ", xyz([.143051, .060608, .713913])], ["rTRC", curve], ["gTRC", curve], ["bTRC", curve]);
  const header = Buffer.alloc(128);
  header[8] = version; header[9] = 0x30;
  header.write("mntr", 12); header.write(model === "gray" ? "GRAY" : "RGB ", 16);
  header.write("XYZ ", 20); header.write("acsp", 36);
  Buffer.concat([fixed(.9642), fixed(1), fixed(.8249)]).copy(header, 68);
  let offset = 132 + 12 * tags.length;
  const records = [], bodies = [];
  for (const [name, bytes] of tags) {
    records.push(Buffer.concat([Buffer.from(name), u32(offset), u32(bytes.length)]));
    const padded = Buffer.alloc(Math.ceil(bytes.length / 4) * 4); bytes.copy(padded);
    bodies.push(padded); offset += padded.length;
  }
  const bytes = Buffer.concat([header, u32(tags.length), ...records, ...bodies]);
  bytes.writeUInt32BE(bytes.length); return bytes;
}
function part(bytes, sequence, total) {
  const data = Buffer.concat([Buffer.from("ICC_PROFILE\0"), Buffer.from([sequence, total]), bytes]);
  return Buffer.concat([Buffer.from([255, 226]), u16(data.length + 2), data]);
}
function segments(icc, layout) {
  if (layout === "single") return [part(icc, 1, 1)];
  // Split inside the header/tag table, not at a convenient profile boundary.
  const chunks = [icc.subarray(0, 73), icc.subarray(73, 145), icc.subarray(145)];
  const ordered = chunks.map((bytes, i) => part(bytes, i + 1, 3));
  if (layout === "ordered") return ordered;
  if (layout === "reverse") return ordered.reverse();
  if (layout === "missing") return ordered.slice(0, 2);
  if (layout === "duplicate") return [ordered[0], ...ordered];
  if (layout === "zero-index") return [part(icc, 0, 1)];
  if (layout === "inconsistent-count") return [ordered[0], part(chunks[1], 2, 2), ordered[2]];
  throw new Error(`unknown layout ${layout}`);
}
export function fixtures(root) {
  const inputs = readFileSync(`${root}/tests/oracle/jpeg-color/inputs.json`);
  assert.equal(sha(inputs), "acbbe2df5460565936f027513305b061730da77cb392f27d68d19220ddc34258");
  const bases = JSON.parse(inputs).cases.filter(r => ["rgb", "gray"].includes(r.model));
  const rows = [];
  function add(base, signal, icc, layout, outcome) {
    const source = Buffer.from(base.jpeg_base64, "base64");
    const jpeg = Buffer.concat([source.subarray(0, 2), ...segments(icc, layout), source.subarray(2)]);
    const cell = `${base.model}-${base.mode}-${signal}-${layout}`;
    rows.push({ id: `${cell}-${base.pattern}`, cell, signal, layout, source_model: base.model,
      mode: base.mode, pattern: base.pattern, width: base.width, height: base.height,
      expected_outcome: outcome, profile_base64: icc.toString("base64"),
      profile_sha256: sha(icc), jpeg_base64: jpeg.toString("base64"), jpeg_sha256: sha(jpeg), base_id: base.id });
  }
  for (const base of bases) {
    for (const version of [2, 4]) for (const gamma of [1, 2.2]) {
      const icc = profile(base.model, version, gamma);
      for (const layout of ["single", "ordered", "reverse"]) add(base, `v${version}-g${gamma}`, icc, layout, "accepted");
    }
    const icc = profile(base.model, 2, 1);
    add(base, "opposite-model", profile(base.model === "gray" ? "rgb" : "gray", 2, 1), "single", "inspect-model");
    for (const kind of ["bad-signature", "truncated", "tag-out-of-bounds"]) {
      let bad = Buffer.from(icc);
      if (kind === "bad-signature") bad.write("bad!", 36);
      if (kind === "truncated") bad = bad.subarray(0, 100);
      if (kind === "tag-out-of-bounds") bad.writeUInt32BE(0xfffffff0, 136);
      add(base, kind, bad, "single", "unsupported-format");
    }
    for (const layout of ["missing", "duplicate", "zero-index", "inconsistent-count"]) add(base, "v2-g1", icc, layout, "decode-failed");
  }
  assert.equal(rows.length, 160);
  return rows;
}
export function scene(row) {
  return { type: "excalidraw", version: 2, elements: [{ id: "image", type: "image", x: 0, y: 0,
    width: row.width, height: row.height, angle: 0, opacity: 100, fileId: "asset", status: "saved",
    scale: [1, 1], seed: 1, version: 1, versionNonce: 1, updated: 1, isDeleted: false,
    groupIds: [], frameId: null, boundElements: null, link: null, locked: false,
    strokeColor: "#000000", backgroundColor: "transparent", fillStyle: "solid",
    strokeWidth: 1, strokeStyle: "solid", roughness: 0, index: "a0", crop: null, roundness: null }],
    appState: {}, files: { asset: { id: "asset", mimeType: "image/jpeg", created: 1,
      dataURL: `data:image/jpeg;base64,${row.jpeg_base64}` } } };
}
