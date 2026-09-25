import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { test } from "node:test";
import { HELPER, encodeFrames, encoderProvenance, requestBytes, responseFrame } from "./webp-encode-inputs.mjs";
import { frameRequests, packed } from "./webp-sample-fixtures.mjs";
import { assertFrameParity, frozenEvidence, verifyFrameParity } from "./webp-sample-parity.mjs";

const row = frameRequests()[0];
const invoke = (input, args = []) => spawnSync(HELPER, args, {
  input, timeout: 5000, env: { PATH: "/usr/bin:/bin", LANG: "C" },
});

test("all 28 frozen frames reproduce encoded AND decoded bytes; all 72 fixtures and 12 controls reconstruct", () => {
  const actual = verifyFrameParity();
  assert.equal(actual.frames.length, 28);
  const reverse = encodeFrames([...frameRequests()].reverse());
  assert.deepEqual(reverse.frames.reverse(), actual.frames, "fresh-process reverse replay");
  console.log(`libwebp ${actual.encoder.version}; ${JSON.stringify(actual.encoder.binaries)}`);
  console.log(`compiler: ${actual.encoder.build_provenance.compiler.split("\n")[0]}`);
});

test("JSON stdin/stdout entry point reproduces the frozen frame records", () => {
  const actual = JSON.parse(execFileSync(process.execPath, [new URL("./webp-encode-inputs.mjs", import.meta.url).pathname], {
    input: JSON.stringify(frameRequests()), maxBuffer: 8 * 1024 * 1024, timeout: 30000,
  }));
  const { inputs, reference } = frozenEvidence();
  assertFrameParity(actual, inputs, reference);
});

test("native protocol rejects malformed dimensions, lengths, framing and options before output", () => {
  encoderProvenance();
  const valid = requestBytes(row);
  const set = (offset, value) => { const b = Buffer.from(valid); b.writeUInt32LE(value, offset); return b; };
  const bad = [
    [Buffer.alloc(0), /truncated request header/],
    [valid.subarray(0, 23), /truncated request header/],
    [set(0, 0), /invalid protocol magic/],
    ...[0, 16384, 0xffffffff].flatMap(n => [[set(4, n), /invalid dimensions/], [set(8, n), /invalid dimensions/]]),
    [set(12, 2), /unsupported codec\/quality/],
    [set(16, 75), /unsupported codec\/quality/],
    [set(12, 1), /unsupported codec\/quality/],
    ...[0, 1, 0xffffffff].map(n => [set(20, n), /invalid RGBA length/]),
    [valid.subarray(0, valid.length - 1), /truncated RGBA/],
    [Buffer.concat([valid, Buffer.from([0])]), /trailing request data/],
    [Buffer.concat([valid, valid]), /trailing request data/],
  ];
  const oversized = Buffer.from(valid);
  oversized.writeUInt32LE(4096, 4); oversized.writeUInt32LE(4096, 8); oversized.writeUInt32LE(4096 * 4096 * 4, 20);
  bad.push([oversized, /invalid RGBA length/]);
  for (const [input, message] of bad) {
    const result = invoke(input);
    assert.ifError(result.error); assert.equal(result.signal, null);
    assert.equal(result.status, 1); assert.equal(result.stdout.length, 0);
    assert.match(result.stderr.toString(), message);
  }
  for (const args of [["--quality", "80"], ["--help"], ["--lossless"]]) {
    const result = invoke(valid, args);
    assert.equal(result.status, 1); assert.equal(result.stdout.length, 0);
    assert.match(result.stderr.toString(), /no command-line options/);
  }
});

test("Node rejects unsupported options, malformed base64, dimensions and duplicate IDs", () => {
  const bad = [null, [], { ...row, quality: 80 }, { ...row, codec: "vp8" },
    ...[0, -1, 1.5, "9", 16384, Number.MAX_SAFE_INTEGER].map(width => ({ ...row, width })),
    { ...row, width: 4096, height: 4096 }, { ...row, id: "" },
    { ...row, rgba_base64: row.rgba_base64.slice(1) },
    { ...row, rgba_base64: "!" + row.rgba_base64.slice(1) },
    { ...row, rgba_base64: "AAAA" }, { ...row, rgba_base64: null }];
  for (const input of bad) assert.throws(() => requestBytes(input));
  for (const input of [null, {}, [], [row, row]]) assert.throws(() => encodeFrames(input));
  assert.throws(() => encodeFrames(Array.from({ length: 5 }, (_, i) => ({ ...row, id: `large-${i}`, width: 2048, height: 2048 }))), /batch RGBA/);
  assert.throws(() => encodeFrames([...Array.from({ length: 5 }, (_, i) => ({ ...row, id: `large-${i}`, width: 2048, height: 2048 })), { ...row, width: -2048, height: 2048 }]), /invalid dimensions/);
});

test("response parsing rejects truncation, trailing bytes, version drift and alpha/RGB drift", () => {
  const result = invoke(requestBytes(row));
  assert.equal(result.status, 0);
  const valid = result.stdout;
  const set = (offset, value) => { const b = Buffer.from(valid); b.writeUInt32LE(value, offset); return b; };
  for (const bytes of [valid.subarray(0, 19), valid.subarray(0, valid.length - 1),
    Buffer.concat([valid, Buffer.from([0])]), set(0, 0), set(0, 0xb1cfd0d7), set(4, 0x010303), set(8, 0x010303), set(12, 0), set(16, 0)])
    assert.throws(() => responseFrame(row, bytes));
  const decodedStart = 20 + valid.readUInt32LE(12);
  for (const offset of [decodedStart, decodedStart + 3]) {
    const bytes = Buffer.from(valid); bytes[offset] ^= 1;
    assert.throws(() => responseFrame(row, bytes));
  }
});

test("parity gate rejects rehashed encoded/decoded drift and option drift", () => {
  const { inputs, reference } = frozenEvidence();
  for (const key of ["encoded", "native_decoded_rgba"]) {
    const wrong = structuredClone(inputs), bytes = Buffer.from(wrong.frames[0][key].base64, "base64");
    bytes[bytes.length - 1] ^= 1; wrong.frames[0][key] = packed(bytes);
    assert.throws(() => assertFrameParity(wrong, inputs, reference), /frame byte parity/);
  }
  const wrong = structuredClone(inputs); wrong.encoder.lossy = "quality=80";
  assert.throws(() => assertFrameParity(wrong, inputs, reference), /encoder lossy/);
});
