// Supplemental fixtures; the original color fixture producer stays frozen.
// ISO 21496-1 layout follows SkGainmapInfo::serialize, and PNG nesting follows
// SkPngEncoderImpl::setHdrMetadata: version-only outer gmAP, full inner gmAP.
import assert from "node:assert/strict";
import { deflateSync } from "node:zlib";
import { fixtures } from "./png-color-fixtures.mjs";

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
    if (kind === "IDAT") result.push(...tags.filter(t => !t.after).map(t => chunk(t.kind, Buffer.from(t.data))));
    if (kind === "IEND") result.push(...tags.filter(t => t.after).map(t => chunk(t.kind, Buffer.from(t.data))));
    result.push(png.subarray(offset, offset + size)); offset += size;
  }
  return Buffer.concat(result);
}
const tag = (kind, data, after = false) => ({ kind, data: [...data], after });
function metadata(v) {
  const perChannel = [0, 4, 1, 0, 0]; // gain 1..16; gamma=1; zero offsets
  const values = [v.hdr ? 4 : 0, v.hdr ? 0 : 4,
    ...Array.from({length: v.multichannel ? 3 : 1}, (_, c) => perChannel.map((n,i) => i === 1 ? n + c : n)).flat()];
  let result = Buffer.concat([Buffer.from([0,0,0,0,(v.multichannel ? 0x80 : 0) | (v.alternate_space ? 0 : 0x40)]), ...values.flatMap(n => [u32(n),u32(1)])]);
  if (v.mode === "truncated-inner-metadata") result = result.subarray(0, 23);
  if (v.mode === "zero-denominator") result.writeUInt32BE(0, 9);
  if (v.mode === "zero-gamma") result.writeUInt32BE(0, 37);
  return result;
}
function gainPng(v, p3) {
  const header = Buffer.alloc(13); header.writeUInt32BE(v.mode === "huge-inner-dimensions" ? 0x7fffffff : 15); header.writeUInt32BE(15, 4); header[8] = 8; header[9] = 2;
  const samples = Buffer.alloc(15 * (1 + 15*3));
  for (let y=0;y<15;y++) for(let x=0;x<15;x++) for(let c=0;c<3;c++) {
    samples[y*46+1+x*3+c] = v.mode === "black" ? 0 : v.mode === "white" ? 255 : ((x+y+c)%2)*255;
  }
  let png = Buffer.concat([Buffer.from("89504e470d0a1a0a","hex"),chunk("IHDR",header),chunk("IDAT",deflateSync(samples)),chunk("IEND",Buffer.alloc(0))]);
  const tags = v.mode === "missing-inner-metadata" ? [] : [tag("gmAP", metadata(v))];
  if (v.alternate_space) tags.push(...p3);
  png = insert(png, tags);
  if (v.mode === "recursive-payload") {
    // Ten bounded nested containers: an implementation must not recurse into
    // ignored gdAT. The outer admitted byte count still includes every byte.
    for(let i=0;i<10;i++) png = insert(png,[tag("gdAT",png)]);
  }
  if (v.mode === "bad-inner-crc") png[png.length-1] ^= 1;
  return png;
}
export function gainFixtures(input, baseInput) {
  assert.equal(input.schema, "excalidraw-render/png-gain-map-inputs/v1");
  const bases = fixtures(baseInput);
  const lookup = (signal, depth) => bases.find(r => r.id === `${signal}-c6-d${depth}`);
  const controls = [...new Set(input.variants.map(v=>v.base))].flatMap(base => input.depths.map(depth => ({...lookup(base,depth),group:"gain-map-controls"})));
  const result = input.variants.flatMap(v => input.depths.map(depth => {
    const base = lookup(v.base,depth); assert.ok(base,v.base);
    let payload = gainPng(v,lookup("icc-v2-p3",depth).tags);
    if(v.mode === "empty-payload") payload = Buffer.alloc(0);
    if(v.mode === "non-png-payload") payload = Buffer.from("not a PNG");
    if(v.mode === "truncated-payload") payload = payload.subarray(0,30);
    let version = Buffer.from([0,0,0,0]);
    if(v.mode === "empty-metadata") version = Buffer.alloc(0);
    if(v.mode === "short-metadata") version = Buffer.from([0]);
    if(v.mode === "future-version") version = Buffer.from([0,1,0,1]);
    const tags = [];
    if(v.cicp) tags.push(tag("cICP",v.cicp));
    if(v.competing) tags.push(...lookup(v.competing,depth).tags);
    if(!["absent","payload-only"].includes(v.mode)) tags.push(tag("gmAP",version));
    if(v.mode === "duplicate-metadata") tags.push(tag("gmAP",[0,1,0,0]));
    if(!["absent","metadata-only"].includes(v.mode)) {
      if(v.mode === "split-payload") {
        tags.push(tag("gdAT",payload.subarray(0,30)),tag("gdAT",payload.subarray(30)));
      } else tags.push(tag("gdAT",payload,v.mode === "late-payload"));
    }
    if(v.mode === "duplicate-payload") tags.push(tag("gdAT",gainPng({...v,mode:"black"},[])));
    const withTags = bytes => {
      const png = insert(Buffer.from(bytes,"base64"),tags);
      if(v.mode === "bad-outer-crc") {
        const at = png.indexOf(Buffer.from("gdAT"));
        png[at+4] ^= 1;
      }
      return png.toString("base64");
    };
    return {...base,id:`gain-${v.id}-c6-d${depth}`,signal:`gain-${v.id}`,group:"gain-map",control_id:base.id,
      expected_outcome:v.outcome ?? "accepted",tags:[...base.tags,...tags],
      png:withTags(base.png),opaque_png:withTags(base.opaque_png)};
  }));
  return [...controls,...result];
}
