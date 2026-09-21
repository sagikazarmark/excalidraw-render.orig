// Project-authored MPF/TIFF/XMP wire-format fixtures, not copied encoder code.
// MPF fields and XMP vocabulary checked against libultrahdr f3e36226914289cfefcf0d81a6471cf6266c79fb.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
export { scene } from "./jpeg-icc-fixtures.mjs";
export const sha = bytes => createHash("sha256").update(bytes).digest("hex");
const XMP = "http://ns.adobe.com/xap/1.0/\0";
function segment(marker, payload) {
  const head = Buffer.from([255, marker, 0, 0]); head.writeUInt16BE(payload.length + 2, 2);
  return Buffer.concat([head, payload]);
}
function xmp(attributes, children = "") {
  return segment(0xe1, Buffer.from(`${XMP}<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/" ${attributes}>${children}</rdf:Description></rdf:RDF></x:xmpmeta>`));
}
function mpf(primarySize, secondarySize, secondaryOffset, big, bad) {
  const short = n => { const b = Buffer.alloc(2); big ? b.writeUInt16BE(n) : b.writeUInt16LE(n); return b; };
  const long = n => { const b = Buffer.alloc(4); big ? b.writeUInt32BE(n) : b.writeUInt32LE(n); return b; };
  const field = (tag, type, count, value) => Buffer.concat([short(tag), short(type), long(count), value]);
  return segment(0xe2, Buffer.concat([Buffer.from("MPF\0"), Buffer.from(big ? "MM" : "II"), short(42), long(8), short(3),
    field(0xb000, 7, 4, Buffer.from("0100")), field(0xb001, 4, 1, long(bad === "bad-count" ? 3 : 2)),
    field(0xb002, 7, 32, long(50)), long(0),
    long(0x030000), long(primarySize), long(0), short(0), short(0),
    long(0), long(bad === "bad-size" ? secondarySize + 123 : secondarySize),
    long(bad === "bad-offset" ? 0xfffffff0 : secondaryOffset), short(0), short(0)]));
}
export function fixtures(root) {
  const input = readFileSync(`${root}/tests/oracle/jpeg-color/inputs.json`);
  assert.equal(sha(input), "acbbe2df5460565936f027513305b061730da77cb392f27d68d19220ddc34258");
  const originals = JSON.parse(input).cases;
  const bases = originals.filter(r => r.model === "rgb");
  const rows = [];
  for (const base of bases) for (const big of [false, true]) {
    for (const signal of ["identity", "sdr-ramp", "sdr-edges", "hdr-ramp", "hdr-edges", "omit-mpf", "omit-xmp",
      "bad-offset", "bad-size", "bad-count", "missing-secondary", "truncated-secondary", "third-image"]) {
      const mapPattern = signal.endsWith("edges") ? "edges" : "ramp";
      const map = originals.find(r => r.model === "gray" && r.mode === base.mode && r.pattern === mapPattern);
      const mapBytes = Buffer.from(map.jpeg_base64, "base64"), source = Buffer.from(base.jpeg_base64, "base64");
      const hdr = signal.startsWith("hdr-"); const max = signal === "identity" ? 0 : 2;
      const secondaryXmp = xmp(`hdrgm:Version="1.0" hdrgm:GainMapMin="0" hdrgm:GainMapMax="${max}" hdrgm:Gamma="1" hdrgm:OffsetSDR="0" hdrgm:OffsetHDR="0" hdrgm:HDRCapacityMin="0" hdrgm:HDRCapacityMax="2" hdrgm:BaseRenditionIsHDR="${hdr ? "True" : "False"}"`);
      const secondary = Buffer.concat([mapBytes.subarray(0, 2), secondaryXmp, mapBytes.subarray(2)]);
      const directory = `<Container:Directory xmlns:Container="http://ns.google.com/photos/1.0/container/" xmlns:Item="http://ns.google.com/photos/1.0/container/item/"><rdf:Seq><rdf:li rdf:parseType="Resource"><Container:Item Item:Semantic="Primary" Item:Mime="image/jpeg"/></rdf:li><rdf:li rdf:parseType="Resource"><Container:Item Item:Semantic="GainMap" Item:Mime="image/jpeg" Item:Length="${secondary.length}"/></rdf:li></rdf:Seq></Container:Directory>`;
      const primaryXmp = signal === "omit-xmp" ? Buffer.alloc(0) : xmp('hdrgm:Version="1.0"', directory);
      const mpfSize = signal === "omit-mpf" ? 0 : 90;
      const primarySize = source.length + primaryXmp.length + mpfSize;
      // MPF is first after SOI; TIFF offset base is file byte 10.
      const index = signal === "omit-mpf" ? Buffer.alloc(0) : mpf(primarySize, secondary.length, primarySize - 10, big, signal);
      assert.equal(index.length, mpfSize);
      const primary = Buffer.concat([source.subarray(0, 2), index, primaryXmp, source.subarray(2)]);
      const jpeg = signal === "missing-secondary" ? primary : Buffer.concat([primary,
        signal === "truncated-secondary" ? secondary.subarray(0, secondary.length - 2) : secondary,
        ...(signal === "third-image" ? [mapBytes] : [])]);
      const cell = `${base.mode}-${big ? "big" : "little"}-${signal}`;
      rows.push({ id: `${cell}-${base.pattern}`, cell, signal, mode: base.mode, byte_order: big ? "big" : "little",
        pattern: base.pattern, width: base.width, height: base.height, base_id: base.id, map_id: map.id,
        base_hdr: hdr, gain_max_log2: max, primary_size: primary.length, secondary_size: secondary.length,
        primary_xmp_base64: primaryXmp.toString("base64"), secondary_xmp_base64: secondaryXmp.toString("base64"),
        mpf_base64: index.toString("base64"), jpeg_base64: jpeg.toString("base64"), jpeg_sha256: sha(jpeg),
        container_valid: ["identity", "sdr-ramp", "sdr-edges", "hdr-ramp", "hdr-edges"].includes(signal) });
    }
  }
  assert.equal(rows.length, 104); return rows;
}
