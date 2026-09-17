// Authored PNG metadata and sample vectors. No renderer or color-management
// library supplies the input pixels, ICC bytes, or required case identities.
import assert from "node:assert/strict";
import { deflateSync } from "node:zlib";

function u32(value) { const b = Buffer.alloc(4); b.writeUInt32BE(value); return b; }
function fixed(value) { const b = Buffer.alloc(4); b.writeInt32BE(Math.round(value * 65536)); return b; }
function chunk(kind, data) {
  const body = Buffer.concat([Buffer.from(kind), data]);
  let crc = 0xffffffff;
  for (const byte of body) {
    crc ^= byte;
    for (let i = 0; i < 8; i++) crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
  }
  return Buffer.concat([u32(data.length), body, u32((crc ^ 0xffffffff) >>> 0)]);
}
// D50-adapted sRGB matrix, a D50 white point and three shared TRC tags. v2
// uses u8Fixed8 curve gamma; v4 uses s15Fixed16 parametric curves (types 0/4).
function icc(version, gamma, parametric = 0, gamut = "srgb") {
  const xyz = values => Buffer.concat([Buffer.from("XYZ \0\0\0\0"), ...values.map(u32)]);
  let curve;
  if (version === 2) {
    const exponent = Buffer.alloc(4); exponent.writeUInt16BE(Math.round(gamma * 256));
    curve = Buffer.concat([Buffer.from("curv\0\0\0\0"), u32(1), exponent]);
  } else {
    const kind = Buffer.alloc(4); kind.writeUInt16BE(parametric);
    const parameters = {
      0: [gamma], 1: [gamma, 1.1, -0.1], 2: [gamma, 1.1, -0.1, 0.05],
      3: [2.4, 1 / 1.055, 0.055 / 1.055, 1 / 12.92, 0.04045],
      4: [2.4, 1 / 1.055, 0.055 / 1.055, 1 / 12.92, 0.04045, 0, 0],
    }[parametric] ?? [gamma];
    curve = Buffer.concat([Buffer.from("para\0\0\0\0"), kind, ...parameters.map(fixed)]);
  }
  const bodies = [
    xyz([0x6fa3, 0x38f6, 0x0391]), xyz([0x6294, 0xb785, 0x18dc]),
    xyz([0x24a1, 0x0f85, 0xb6d4]), xyz([0xf6d6, 0x10000, 0xd32d]), curve,
  ];
  if (gamut === "p3") {
    const colorants = [[0.515102, 0.241182, -0.00104941], [0.291965, 0.692236, 0.0418818], [0.157153, 0.0665819, 0.784378]];
    colorants.forEach((values, i) => { bodies[i] = Buffer.concat([Buffer.from("XYZ \0\0\0\0"), ...values.map(fixed)]); });
  }
  const header = Buffer.alloc(128); header[8] = version; header[9] = version === 2 ? 0x10 : 0x30;
  header.write("mntr", 12); header.write("RGB ", 16); header.write("XYZ ", 20); header.write("acsp", 36);
  Buffer.concat([u32(0xf6d6), u32(0x10000), u32(0xd32d)]).copy(header, 68);
  const signatures = ["rXYZ", "gXYZ", "bXYZ", "wtpt", "rTRC", "gTRC", "bTRC"];
  const offsets = [216, 236, 256, 276, 296, 296, 296];
  const records = signatures.map((name, i) => Buffer.concat([Buffer.from(name), u32(offsets[i]), u32(i < 4 ? 20 : curve.length)]));
  const result = Buffer.concat([header, u32(7), ...records, ...bodies]);
  result.writeUInt32BE(result.length, 0);
  return result;
}
function tag(kind, data, after = false) { return { kind, data: [...data], after }; }
const gama = value => tag("gAMA", u32(value));
const srgb = value => tag("sRGB", [value]);
const cicp = (p, t, r = 1) => tag("cICP", [p, t, 0, r]);
const iccp = (version, gamma, type = 0, gamut = "srgb") => tag("iCCP", Buffer.concat([Buffer.from("authored\0\0"), deflateSync(icc(version, gamma, type, gamut))]));

export function signals(input) {
  const result = [];
  const add = (id, group, tags) => result.push({ id, group, tags });
  add("untagged", "srgb-intents", []);
  for (const intent of [0, 1, 2, 3, 4, 255]) add(`srgb-${intent}`, "srgb-intents", [srgb(intent)]);
  add("srgb-duplicate", "srgb-intents", [srgb(0), srgb(3)]);
  add("srgb-empty", "srgb-intents", [tag("sRGB", [])]);
  add("srgb-long", "srgb-intents", [tag("sRGB", [0, 1])]);
  add("gama-linear", "gama-linear", [gama(100000)]);
  for (const value of [43181, 43182, 45455, 47727, 47728, 50000, 70000, 0, 1, 0xffffffff]) add(`gama-${value}`, "gama-nonlinear", [gama(value)]);
  add("gama-short", "gama-nonlinear", [tag("gAMA", [0, 1, 2])]);
  add("gama-duplicate", "gama-nonlinear", [gama(50000), gama(100000)]);
  for (const [name, chromaticities] of [
    ["srgb", input.srgb_chromaticities], ["srgb-d50", input.d50_chromaticities], ["p3", input.p3_chromaticities],
    ["zero", Array(8).fill(0)], ["out-of-range", [120000, 32900, ...input.srgb_chromaticities.slice(2)]],
  ]) {
    for (const gamma of [null, 45455, 100000]) {
      add(`chrm-${name}-${gamma ?? "default"}`, "chrm", [tag("cHRM", Buffer.concat(chromaticities.map(u32))), ...(gamma === null ? [] : [gama(gamma)])]);
    }
  }
  for (const version of [2, 4]) {
    for (const gamma of [1, 2.2]) add(`icc-v${version}-gamma${gamma}`, `icc-v${version}`, [iccp(version, gamma)]);
    add(`icc-v${version}-p3`, `icc-v${version}`, [iccp(version, 2.2, 0, "p3")]);
  }
  for (const type of [1, 2, 3]) add(`icc-v4-param${type}`, "icc-v4", [iccp(4, 2.2, type)]);
  add("icc-v4-srgb", "icc-v4", [iccp(4, 2.4, 4)]);
  add("icc-v4-unknown-curve", "icc-v4", [iccp(4, 1, 5)]);
  add("srgb-over-gama", "precedence", [gama(100000), srgb(0)]);
  add("gama-after-srgb", "precedence", [srgb(0), gama(100000)]);
  const chrmConflict = [tag("cHRM", Buffer.concat(input.p3_chromaticities.map(u32))), gama(100000)];
  add("srgb-over-chrm", "precedence", [...chrmConflict, srgb(0)]);
  add("icc-over-chrm", "precedence", [...chrmConflict, iccp(2, 2.2)]);
  add("cicp-over-chrm", "precedence", [cicp(1, 13), ...chrmConflict]);
  add("icc-over-srgb", "precedence", [srgb(0), iccp(2, 1)]);
  add("srgb-after-icc", "precedence", [iccp(2, 1), srgb(0)]);
  add("cicp-over-all", "precedence", [cicp(1, 13), iccp(2, 1), srgb(0), gama(70000)]);
  add("cicp-linear-over-all", "precedence", [cicp(1, 8), iccp(4, 2.2), srgb(0), gama(70000)]);
  add("narrow-cicp-over-icc", "precedence", [cicp(1, 13, 0), iccp(2, 1), srgb(0)]);
  add("cicp-over-invalid-icc", "precedence", [cicp(1, 13), tag("iCCP", Buffer.concat([Buffer.from("bad\0\0"), deflateSync(Buffer.from("not an ICC profile"))]))]);
  add("duplicate-icc", "precedence", [iccp(2, 1), iccp(4, 2.2)]);
  add("duplicate-chrm", "precedence", [gama(100000), tag("cHRM", Buffer.concat(input.srgb_chromaticities.map(u32))), tag("cHRM", Buffer.concat(input.p3_chromaticities.map(u32)))]);
  add("invalid-cicp-over-icc", "precedence", [cicp(0, 13), iccp(2, 1)]);
  add("invalid-icc-over-srgb", "precedence", [tag("iCCP", Buffer.concat([Buffer.from("bad\0\0"), deflateSync(Buffer.from("not an ICC profile"))])), srgb(0)]);
  add("duplicate-cicp", "precedence", [cicp(1, 13), cicp(1, 8)]);
  add("late-gama", "precedence", [tag("gAMA", u32(100000), true)]);
  add("late-srgb", "precedence", [gama(100000), { ...srgb(0), after: true }]);
  add("late-icc", "precedence", [gama(70000), { ...iccp(2, 1), after: true }]);
  add("late-cicp", "precedence", [{ ...cicp(1, 8), after: true }]);
  add("late-chrm", "precedence", [gama(100000), tag("cHRM", Buffer.concat(input.p3_chromaticities.map(u32)), true)]);
  for (const p of input.cicp_primaries) for (const t of input.cicp_transfers) for (const r of input.cicp_ranges) {
    add(`cicp-p${p}-t${t}-r${r}`, "cicp", [cicp(p, t, r)]);
  }
  return result;
}

function png(signal, layout, samples, width, opaque) {
  const header = Buffer.alloc(13); header.writeUInt32BE(width); header.writeUInt32BE(samples.length / width, 4);
  header[8] = layout.depth; header[9] = layout.color_type;
  const channels = layout.color_type === 4 ? 2 : 4;
  const bytes = layout.depth / 8;
  const stride = width * channels * bytes;
  const scanlines = Buffer.alloc((samples.length / width) * (stride + 1));
  samples.forEach((rgba, p) => {
    const pixel = layout.color_type === 4 ? [rgba[0], rgba[3]] : [...rgba];
    if (opaque) pixel[pixel.length - 1] = 255;
    pixel.forEach((v, c) => {
      const offset = Math.floor(p / width) * (stride + 1) + 1 + (p % width) * channels * bytes + c * bytes;
      // Distinct low bytes detect whether a tagged path preserves extra precision.
      if (bytes === 2) scanlines.writeUInt16BE(v === 0 ? 0 : v === 255 ? 65535 : v * 256 + (255 - v), offset);
      else scanlines[offset] = v;
    });
  });
  const chunks = signal.tags.map(t => ({ ...t, bytes: chunk(t.kind, Buffer.from(t.data)) }));
  return Buffer.concat([Buffer.from("89504e470d0a1a0a", "hex"), chunk("IHDR", header),
    ...chunks.filter(t => !t.after).map(t => t.bytes), chunk("IDAT", deflateSync(scanlines)),
    ...chunks.filter(t => t.after).map(t => t.bytes), chunk("IEND", Buffer.alloc(0))]);
}

export function fixtures(input, probe = false) {
  assert.equal(input.schema, "excalidraw-render/png-color-inputs/v1");
  const rgb = input.ramp.flatMap(v => [[v, v, v], [v, 0, 0], [0, v, 0], [0, 0, v], [v, 255 - v, (v * 73) % 256]]);
  // The final raster is a spatially varying color/alpha field, not padding or
  // repeated easy pixels added to dilute a comparison. Small exact semantic
  // anchors are exercised separately by the consuming contract.
  const side = input.ramp.length;
  const samples = probe ? input.alpha.flatMap(alpha => rgb.map(color => [...color, alpha]))
    : Array.from({ length: side * side }, (_, i) => {
      const x = i % side; const y = Math.floor(i / side);
      return [input.ramp[x], input.ramp[(x + 5 * y) % side], input.ramp[(3 * x + 7 * y) % side], input.alpha[(x + 3 * y) % input.alpha.length]];
    });
  const width = probe ? rgb.length : side;
  return signals(input).flatMap(signal => (probe ? [{ color_type: 6, depth: 8 }] : input.layouts)
    .filter(layout => signal.group !== "cicp" || layout.color_type === 6)
    .map(layout => ({ id: `${signal.id}-c${layout.color_type}-d${layout.depth}`, group: signal.group,
      signal: signal.id, layout, width, height: samples.length / width,
      expected_outcome: input.decode_failures.includes(signal.id) ? "decode-failed" : input.unsupported_colors.includes(signal.id) ? "unsupported-format" : "accepted",
      alpha: samples.map(p => p[3]), tags: signal.tags,
      png: png(signal, layout, samples, width, false).toString("base64"),
      opaque_png: png(signal, layout, samples, width, true).toString("base64"),
    })));
}

export function sceneFor(row, opaque = false) {
  return { type: "excalidraw", version: 2, elements: [{
    id: "image", type: "image", x: 0, y: 0, width: row.width, height: row.height,
    angle: 0, strokeColor: "#000000", backgroundColor: "transparent", fillStyle: "solid",
    strokeWidth: 1, strokeStyle: "solid", roughness: 0, opacity: 100, groupIds: [],
    frameId: null, index: "a0", seed: 1, version: 1, versionNonce: 1, isDeleted: false,
    boundElements: null, updated: 1, link: null, locked: false, fileId: "asset",
    status: "saved", scale: [1, 1], crop: null, roundness: null,
  }], appState: {}, files: { asset: { id: "asset", mimeType: "image/png", created: 1,
    dataURL: `data:image/png;base64,${opaque ? row.opaque_png : row.png}` } } };
}
