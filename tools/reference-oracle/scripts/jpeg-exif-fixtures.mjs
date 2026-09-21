// Independently authored APP1/TIFF precedence vectors over frozen JPEG samples.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { scene as imageScene } from "./jpeg-icc-fixtures.mjs";
export const sha = bytes => createHash("sha256").update(bytes).digest("hex");
function tiff(values, big, magic = 42) {
  const short = n => { const b = Buffer.alloc(2); big ? b.writeUInt16BE(n) : b.writeUInt16LE(n); return b; };
  const long = n => { const b = Buffer.alloc(4); big ? b.writeUInt32BE(n) : b.writeUInt32LE(n); return b; };
  return Buffer.concat([Buffer.from("Exif\0\0"), Buffer.from(big ? "MM" : "II"), short(magic), long(8), short(values.length),
    ...values.map(value => Buffer.concat([short(0x112), short(3), long(1), short(value), short(0)])), long(0)]);
}
function app1(payload) {
  const header = Buffer.from([255, 225, 0, 0]); header.writeUInt16BE(payload.length + 2, 2);
  return Buffer.concat([header, payload]);
}
// Locate actual marker boundaries, respecting byte stuffing and restart markers.
function scanMarkers(bytes) {
  const markers = []; let at = 2;
  while (at < bytes.length) {
    assert.equal(bytes[at], 255); const kind = bytes[at + 1];
    markers.push({ kind, at });
    if (kind === 0xd9) { assert.equal(at + 2, bytes.length); break; }
    const length = bytes.readUInt16BE(at + 2); assert.ok(length >= 2);
    at += 2 + length;
    if (kind === 0xda) {
      while (at + 1 < bytes.length) {
        if (bytes[at] === 255 && bytes[at + 1] !== 0 && !(bytes[at + 1] >= 0xd0 && bytes[at + 1] <= 0xd7)) break;
        at += 1;
      }
    }
  }
  return markers;
}
export function fixtures(root) {
  const input = readFileSync(`${root}/tests/oracle/jpeg-color/inputs.json`);
  assert.equal(sha(input), "acbbe2df5460565936f027513305b061730da77cb392f27d68d19220ddc34258");
  const bases = JSON.parse(input).cases.filter(r => r.model === "rgb");
  const rows = [];
  for (const base of bases) for (const big of [false, true]) {
    const source = Buffer.from(base.jpeg_base64, "base64");
    const markers = scanMarkers(source), end = markers.at(-1).at;
    const orientation = value => tiff([value], big);
    const variants = [
      ...[1, 3, 6, 8].map(v => [`control-${v}`, [orientation(v)], null, null]),
      ["duplicate-6-3", [orientation(6), orientation(3)], null, null],
      ["duplicate-3-6", [orientation(3), orientation(6)], null, null],
      ["invalid-then-valid", [orientation(0), orientation(6)], null, null],
      ["valid-then-invalid", [orientation(6), orientation(0)], null, null],
      ["absent-tag-then-valid", [tiff([], big), orientation(6)], null, null],
      ["valid-then-absent-tag", [orientation(6), tiff([], big)], null, null],
      ["bad-magic-then-valid", [tiff([6], big, 0), orientation(6)], null, null],
      ["valid-then-bad-magic", [orientation(6), tiff([6], big, 0)], null, null],
      ["duplicate-tags-6-3", [tiff([6, 3], big)], null, null],
      ["duplicate-tags-3-6", [tiff([3, 6], big)], null, null],
      ["invalid-tag-then-valid", [tiff([0, 6], big)], null, null],
      ["valid-tag-then-invalid", [tiff([6, 0], big)], null, null],
      ["late-only", [], orientation(6), end],
      ["early-3-late-6", [orientation(3)], orientation(6), end],
      ["early-6-late-3", [orientation(6)], orientation(3), end],
    ];
    if (base.mode === "progressive") {
      const scans = markers.filter(m => m.kind === 0xda); assert.ok(scans.length > 1);
      variants.push(["between-scans-only", [], orientation(6), scans[1].at],
        ["early-3-between-6", [orientation(3)], orientation(6), scans[1].at],
        ["early-6-between-3", [orientation(6)], orientation(3), scans[1].at]);
    }
    for (const [signal, early, late, offset] of variants) {
      const segments = early.map(app1);
      const jpeg = Buffer.concat([source.subarray(0, 2), ...segments,
        ...(late ? [source.subarray(2, offset), app1(late), source.subarray(offset)] : [source.subarray(2)])]);
      const cell = `${base.mode}-${big ? "big" : "little"}-${signal}`;
      rows.push({ id: `${cell}-${base.pattern}`, cell, signal, mode: base.mode, byte_order: big ? "big" : "little",
        pattern: base.pattern, base_id: base.id, source_width: base.width, source_height: base.height,
        early_app1_base64: early.map(b => b.toString("base64")), late_app1_base64: late?.toString("base64") ?? null,
        late_source_offset: offset, jpeg_base64: jpeg.toString("base64"), jpeg_sha256: sha(jpeg) });
    }
  }
  assert.equal(rows.length, 164); return rows;
}
export function scene(row, width, height) { return imageScene({ ...row, width, height }); }
