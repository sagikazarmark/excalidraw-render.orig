// Supplemental HDR peak-luminance fixtures (#85 codec:png:hdr-gain-map). The
// original color fixture producer stays frozen: every row inserts cLLI, mDCV or
// gain-map chunks into an unchanged base row of the main color record.
import assert from "node:assert/strict";
import { fixtures } from "./png-color-fixtures.mjs";

const u16 = v => { const b = Buffer.alloc(2); b.writeUInt16BE(v); return b; };
const u32 = v => { const b = Buffer.alloc(4); b.writeUInt32BE(v); return b; };
function chunk(kind, data) {
  const body = Buffer.concat([Buffer.from(kind), data]);
  let crc = 0xffffffff;
  for (const byte of body) {
    crc ^= byte;
    for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
  }
  return Buffer.concat([u32(data.length), body, u32((crc ^ 0xffffffff) >>> 0)]);
}
function insert(png, tags) {
  let offset = 8;
  const result = [png.subarray(0, 8)];
  while (offset < png.length) {
    const size = png.readUInt32BE(offset) + 12;
    const kind = png.toString("ascii", offset + 4, offset + 8);
    if (kind === "IDAT" && !result.inserted) { result.push(...tags.filter(t => !t.after).map(t => chunk(t.kind, Buffer.from(t.data)))); result.inserted = true; }
    if (kind === "IEND") result.push(...tags.filter(t => t.after).map(t => chunk(t.kind, Buffer.from(t.data))));
    result.push(png.subarray(offset, offset + size)); offset += size;
  }
  return Buffer.concat(result);
}
const tag = (kind, data, after = false) => ({ kind, data: [...data], after });
// cLLI: MaxCLL and MaxFALL in 0.0001 cd/m2. mDCV: BT.2020 primaries and D65 in
// 0.00002 units, then maximum and minimum luminance in 0.0001 cd/m2.
const clliData = units => Buffer.concat([u32(units), u32(Math.min(units, 4000000))]);
const mdcvData = units => Buffer.concat([
  ...[[35400, 14600], [8500, 39850], [6550, 2300], [15635, 16450]].flatMap(([x, y]) => [u16(x), u16(y)]),
  u32(units), u32(50)]);
function tags(spec) {
  return spec.map(([kind, units, shape = "valid", after = false]) => {
    let data = kind === "cLLI" ? clliData(units) : kind === "mDCV" ? mdcvData(units) : Buffer.from(units);
    if (shape === "short") data = data.subarray(0, data.length - 1);
    if (shape === "long") data = Buffer.concat([data, Buffer.from([0])]);
    return tag(kind, data, after);
  });
}
export function hdrMetadataFixtures(input, baseInput) {
  assert.equal(input.schema, "excalidraw-render/png-hdr-metadata-inputs/v1");
  const bases = fixtures(baseInput);
  const lookup = (signal, depth) => { const row = bases.find(r => r.id === `${signal}-c6-d${depth}`); assert.ok(row, signal); return row; };
  const controls = input.bases.flatMap(base => input.depths.map(depth => ({ ...lookup(base.signal, depth), group: "hdr-metadata-controls" })));
  const rows = input.bases.flatMap(base => input.variants.filter(v => base.hdr || v.sdr_control).flatMap(v => input.depths.map(depth => {
    const control = lookup(base.signal, depth);
    const inserted = tags(v.tags);
    const withTags = bytes => insert(Buffer.from(bytes, "base64"), inserted).toString("base64");
    const equivalent = v.equivalent === "control" || !base.hdr ? control.id : `hdr-${base.signal}-${v.equivalent}-c6-d${depth}`;
    return { ...control, id: `hdr-${base.signal}-${v.id}-c6-d${depth}`, signal: `hdr-${base.signal}-${v.id}`,
      group: "hdr-metadata", control_id: control.id, equivalent_id: v.equivalent ? equivalent : null,
      peak_nits: base.hdr ? v.peak_nits : null, tags: [...control.tags, ...inserted],
      png: withTags(control.png), opaque_png: withTags(control.opaque_png) };
  })));
  return [...controls, ...rows];
}
