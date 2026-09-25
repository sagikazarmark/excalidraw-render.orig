// JSON/base64/provenance belongs here; the native helper only handles one frame.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { packed, sha } from "./webp-sample-fixtures.mjs";

export const HELPER = "/opt/webp-helper/webp-encode-inputs";
export const LIBRARIES = {
  "/lib/x86_64-linux-gnu/libwebp.so.7": "9664db51ec65209078abbb8bc7b699888a43ecc2f3c74ce8dd4e497175aa5bf0",
  "/lib/x86_64-linux-gnu/libsharpyuv.so.0": "b2e62799e5dbd783a0671d53be99f7c6489567a5332a1e8f471e6bb250f294dc",
};
const source = new URL("./webp-encode-inputs.c", import.meta.url);
const run = (path, args = [], options = {}) => execFileSync(path, args, {
  timeout: 30000, maxBuffer: 40 * 1024 * 1024, ...options,
  // Do not allow LD_PRELOAD/LD_LIBRARY_PATH to substitute a codec after hashing.
  env: { PATH: "/usr/bin:/bin", LANG: "C" },
});

export function requestBytes(row) {
  assert.ok(row && typeof row === "object" && !Array.isArray(row), "frame object required");
  const keys = ["id", "codec", "alpha", "shape", "pattern", "width", "height", "rgba_base64"];
  assert.deepEqual(Object.keys(row).sort(), keys.sort(), "unsupported or missing frame fields/options");
  assert.ok(typeof row.id === "string" && row.id.length > 0, "frame id required");
  for (const key of ["width", "height"]) assert.ok(Number.isInteger(row[key]) && row[key] > 0 && row[key] <= 16383, "invalid dimensions");
  assert.ok(["lossless", "lossy"].includes(row.codec), "unsupported codec");
  assert.ok(typeof row.rgba_base64 === "string", "RGBA base64 required");
  const length = row.width * row.height * 4;
  assert.ok(length <= 16 * 1024 * 1024, "RGBA exceeds protocol limit");
  assert.equal(row.rgba_base64.length, 4 * Math.ceil(length / 3), "invalid RGBA/base64 length");
  const rgba = Buffer.from(row.rgba_base64, "base64");
  assert.equal(rgba.toString("base64"), row.rgba_base64, "noncanonical base64");
  assert.equal(rgba.length, length, "invalid RGBA length");
  const header = Buffer.alloc(24); header.write("WPI1");
  [row.width, row.height, row.codec === "lossy" ? 1 : 0, row.codec === "lossy" ? 75 : 0, length]
    .forEach((word, i) => header.writeUInt32LE(word, 4 + i * 4));
  return Buffer.concat([header, rgba]);
}

export function responseFrame(row, bytes) {
  assert.ok(bytes.length >= 20, "truncated helper response");
  assert.ok(bytes.subarray(0, 4).equals(Buffer.from("WPO1")), "invalid helper response magic");
  assert.equal(bytes.readUInt32LE(4), 0x010302, "encoder version");
  assert.equal(bytes.readUInt32LE(8), 0x010302, "decoder version");
  const encodedLength = bytes.readUInt32LE(12), decodedLength = bytes.readUInt32LE(16);
  assert.ok(encodedLength > 0, "empty encoding");
  assert.equal(decodedLength, row.width * row.height * 4);
  assert.equal(bytes.length, 20 + encodedLength + decodedLength, "helper response length");
  const encoded = bytes.subarray(20, 20 + encodedLength), native = bytes.subarray(20 + encodedLength);
  const rgba = Buffer.from(row.rgba_base64, "base64");
  for (let i = 0; i < rgba.length; i += 4) {
    assert.equal(native[i + 3], rgba[i + 3], `${row.id}: alpha`);
    if (row.codec === "lossless" && rgba[i + 3]) assert.deepEqual(native.subarray(i, i + 3), rgba.subarray(i, i + 3), `${row.id}: visible RGB`);
  }
  return { ...row, encoded: packed(encoded), native_decoded_rgba: packed(native) };
}

export function encoderProvenance() {
  assert.equal(process.version, "v24.16.0");
  assert.equal(process.platform, "linux"); assert.equal(process.arch, "x64");
  for (const [path, digest] of Object.entries(LIBRARIES)) assert.equal(sha(readFileSync(path)), digest, path);
  const sourceHash = sha(readFileSync(source));
  assert.equal(readFileSync("/opt/webp-helper/source.sha256", "utf8").split(" ")[0], sourceHash, "stale helper: run build-helper");
  const dockerfileHash = sha(readFileSync(new URL("../webp-helper.Dockerfile", import.meta.url)));
  assert.equal(sha(readFileSync("/opt/webp-helper/webp-helper.Dockerfile")), dockerfileHash, "stale build recipe: run build-helper");
  const distributionPackage = run("/usr/bin/dpkg-query", ["-W", "-f=${Version}", "libwebp7:amd64"]).toString();
  assert.equal(distributionPackage, "1.3.2-0.4build3");
  return {
    implementation: "libwebp", version: "1.3.2", distribution_package: distributionPackage,
    node: process.version, binaries: { ...LIBRARIES, [HELPER]: sha(readFileSync(HELPER)), [process.execPath]: sha(readFileSync(process.execPath)) },
    invocation: "node scripts/webp-encode-inputs.mjs (JSON stdin/stdout); native WPI1/WPO1 per frame",
    lossless: "WebPEncodeLosslessRGBA(rgba,width,height,stride=width*4,out); simple API defaults; invisible RGB may be discarded",
    lossy: "WebPEncodeRGBA(rgba,width,height,stride=width*4,quality_factor=75.0f,out); simple API defaults; exact alpha",
    native_diagnostic: "WebPDecodeRGBA; same external libwebp, not an independent codec cross-check",
    independence: "External C libwebp encoder; no production Rust image-webp encoder/decoder or renderer invoked",
    build_provenance: { source_sha256: sourceHash, compiler: readFileSync("/opt/webp-helper/compiler.txt", "utf8"),
      packages: readFileSync("/opt/webp-helper/build-packages.txt", "utf8"),
      dockerfile_sha256: dockerfileHash,
      runtime: "Original digest-pinned Playwright image plus helper; original codec libraries hash-checked; offline execution" },
  };
}

export function encodeFrames(rows) {
  assert.ok(Array.isArray(rows) && rows.length > 0 && rows.length <= 1024, "nonempty bounded frame array required");
  const totalBytes = rows.reduce((total, row) => {
    for (const key of ["width", "height"]) assert.ok(Number.isInteger(row?.[key]) && row[key] > 0 && row[key] <= 16383, "invalid dimensions");
    const bytes = row.width * row.height * 4;
    assert.ok(bytes <= 16 * 1024 * 1024, "RGBA exceeds protocol limit");
    return total + bytes;
  }, 0);
  assert.ok(Number.isSafeInteger(totalBytes) && totalBytes > 0 && totalBytes <= 64 * 1024 * 1024, "batch RGBA exceeds protocol limit");
  const requests = rows.map(requestBytes);
  assert.equal(new Set(rows.map(row => row.id)).size, rows.length, "duplicate frame id");
  const encoder = encoderProvenance();
  const frames = rows.map((row, i) => responseFrame(row, run(HELPER, [], { input: requests[i] })));
  return { encoder, frames };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  assert.equal(process.argv.length, 2, "no command-line options supported");
  console.log(JSON.stringify(encodeFrames(JSON.parse(readFileSync(0, "utf8")))));
}
