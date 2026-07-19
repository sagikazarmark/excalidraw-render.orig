import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ORACLE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO_ROOT = resolve(ORACLE_ROOT, "../..");
const VIEWER = resolve(ORACLE_ROOT, "scripts/svg-viewer.mjs");
const OUTPUT_ROOT = resolve(REPO_ROOT, "target/svg-viewer-chromium-test");
const OFFICIAL = "tests/conformance/cases/focused/rectangle-outline/evidence/official.svg";
const OFFICIAL_PNG = "tests/conformance/cases/focused/rectangle-outline/evidence/official.png";
const EMPTY_ASSETS = "tests/conformance/svg-viewer-qualification/rectangle-outline/assets.none.json";
const DECLARATION_ID = "excalidraw-render/svg-viewer-asset-declaration/v1";

rmSync(OUTPUT_ROOT, { force: true, recursive: true });
mkdirSync(OUTPUT_ROOT, { recursive: true });

const first = runViewer(OFFICIAL, "target/svg-viewer-chromium-test/first.rgba", EMPTY_ASSETS);
const second = runViewer(OFFICIAL, "target/svg-viewer-chromium-test/second.rgba");
const firstBytes = readFileSync(resolve(REPO_ROOT, "target/svg-viewer-chromium-test/first.rgba"));
const secondBytes = readFileSync(resolve(REPO_ROOT, "target/svg-viewer-chromium-test/second.rgba"));

assert(first.width === 200 && first.height === 120, "official SVG dimensions changed");
assert(firstBytes.length === 200 * 120 * 4, "viewer did not write tightly packed RGBA8");
assert(firstBytes.equals(secondBytes), "fresh Chromium processes produced different buffers");
assert(first.browser_process_id !== second.browser_process_id, "viewer reused a browser process");
assert(first.requests.length === 1 && first.requests[0] === "http://reference.invalid/input.svg", "request allowlist drifted");
assert(first.platform_fonts.length === 0, "font audit unexpectedly found text fonts");
assert(first.lifecycle_updates === 2, "viewer did not complete two lifecycle updates");

const largeInput = "target/svg-viewer-chromium-test/large.svg";
writeFileSync(resolve(REPO_ROOT, largeInput), '<svg xmlns="http://www.w3.org/2000/svg" width="1400" height="800"><rect width="1400" height="800" fill="#fff"/></svg>');
const large = runViewer(largeInput, "target/svg-viewer-chromium-test/large.rgba", EMPTY_ASSETS);
assert(large.width === 1400 && large.height === 800, "capture was clipped to the identity viewport");
assert(readFileSync(resolve(REPO_ROOT, "target/svg-viewer-chromium-test/large.rgba")).length === 1400 * 800 * 4, "large capture is not tightly packed RGBA8");

const animationInput = "target/svg-viewer-chromium-test/animation.svg";
writeFileSync(resolve(REPO_ROOT, animationInput), '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="2"><rect x="0" width="10" height="2" fill="#000"><animate attributeName="x" from="0" to="10" dur="1s" repeatCount="indefinite"/></rect></svg>');
const initial = runViewer(animationInput, "target/svg-viewer-chromium-test/animation-initial.rgba", EMPTY_ASSETS, 0);
const midpoint = runViewer(animationInput, "target/svg-viewer-chromium-test/animation-midpoint.rgba", EMPTY_ASSETS, 500000);
assert(initial.animation_offset_microseconds === 0 && midpoint.animation_offset_microseconds === 500000, "animation offsets were not recorded exactly");
assert(!readFileSync(resolve(REPO_ROOT, "target/svg-viewer-chromium-test/animation-initial.rgba")).equals(readFileSync(resolve(REPO_ROOT, "target/svg-viewer-chromium-test/animation-midpoint.rgba"))), "live-animation midpoint did not change the canonical buffer");

for (const [name, svg, message] of [
  ["network", '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><image href="https://example.invalid/a.png"/></svg>', "canonical base64 data URL"],
  ["local", '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><image href="file:///etc/passwd"/></svg>', "canonical base64 data URL"],
  ["data", '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><image href="data:image/png;base64,iVBORw0KGgo="/></svg>', "undeclared embedded image resource"],
  ["font", '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><text x="0" y="12">ambient</text></svg>', "undeclared font resource"],
  ["paint-url", '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1" fill="url(https://example.invalid/paint.svg#p)"/></svg>', "forbidden CSS resource"],
  ["style-import", '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><style>@import url("https://example.invalid/a.css");</style></svg>', "forbidden stylesheet resource"],
  ["prefixed-style-import", '<svg xmlns="http://www.w3.org/2000/svg" xmlns:s="http://www.w3.org/2000/svg" width="1" height="1"><s:style>@import url("https://example.invalid/a.css");</s:style></svg>', "forbidden stylesheet resource"],
  ["external-use", '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><use href="https://example.invalid/s.svg#x"/></svg>', "forbidden external SVG resource"],
  ["animated-image", '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect width="1" height="1"><set attributeName="href" to="data:image/png;base64,iVBORw0KGgo="/></rect></svg>', "animated resource attribute is forbidden"],
]) {
  const result = spawnViewer(name, svg);
  assert(result.status !== 0, `viewer admitted ${name} resource input`);
  assert(result.stderr.includes(message), `unexpected ${name} rejection: ${result.stderr}`);
}

const imageBytes = readFileSync(resolve(REPO_ROOT, OFFICIAL_PNG));
const imageHash = sha256(imageBytes);
const imageUrl = `data:image/png;base64,${imageBytes.toString("base64")}`;
const imageAssets = writeDeclaration("declared-image", {
  fonts: [],
  images: [{
    source: { media_type: "image/png", sha256: imageHash },
    canonical: { path: OFFICIAL_PNG, media_type: "image/png", sha256: imageHash },
  }],
  schema: DECLARATION_ID,
});
const imageInput = "target/svg-viewer-chromium-test/declared-image.svg";
writeFileSync(resolve(REPO_ROOT, imageInput), `<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><image href="${imageUrl}" width="1" height="1"/></svg>`);
const imageResult = runViewer(imageInput, "target/svg-viewer-chromium-test/declared-image.rgba", imageAssets);
assert(imageResult.width === 1 && imageResult.height === 1, "declared image dimensions changed");
assert(readFileSync(resolve(REPO_ROOT, "target/svg-viewer-chromium-test/declared-image.rgba")).length === 4, "declared image was not rendered");

const symlinkOutput = resolve(REPO_ROOT, "target/svg-viewer-chromium-test/output-symlink.rgba");
symlinkSync("/tmp/excalidraw-render-svg-viewer-escape", symlinkOutput);
const symlinkResult = spawnSync(process.execPath, [VIEWER, "render", OFFICIAL, "target/svg-viewer-chromium-test/output-symlink.rgba", EMPTY_ASSETS], {
  cwd: ORACLE_ROOT,
  encoding: "utf8",
  env: process.env,
});
assert(symlinkResult.status !== 0 && symlinkResult.stderr.includes("output path must not be a symlink"), `viewer followed output symlink: ${symlinkResult.stderr}`);

const unknownField = writeDeclaration("unknown-field", {
  fonts: [], images: [], schema: DECLARATION_ID, unexpected: true,
});
assertRejected("unknown-field", simpleSvg(), "asset declaration must contain exactly", unknownField);

const reviewedFont = Buffer.from([0, 1, 0, 0]);
writeFileSync(resolve(REPO_ROOT, "target/svg-viewer-chromium-test/reviewed.ttf"), reviewedFont);
const reviewedFontHash = sha256(reviewedFont);
const duplicateFace = writeDeclaration("duplicate-face", {
  fonts: [fontDeclaration("0".repeat(64), reviewedFontHash), fontDeclaration("1".repeat(64), reviewedFontHash)],
  images: [],
  schema: DECLARATION_ID,
});
assertRejected("duplicate-face", simpleSvg(), "duplicate font CSS face", duplicateFace);

const fakeWoff2 = Buffer.from("wOF2metadata-only");
const fontAssets = writeDeclaration("font-metadata", {
  fonts: [fontDeclaration(sha256(fakeWoff2), reviewedFontHash)],
  images: [],
  schema: DECLARATION_ID,
});
const mismatchedFont = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><style>@font-face{font-family:'Other';src:url(data:font/woff2;base64,${fakeWoff2.toString("base64")}) format('woff2');font-style:normal;font-weight:400;font-stretch:normal;unicode-range:U+0-10FFFF}</style><text x="0" y="12" font-family="Other">x</text></svg>`;
assertRejected("font-metadata", mismatchedFont, "@font-face metadata does not match declaration", fontAssets);

console.log(JSON.stringify({ result: "pass", rgba_sha256: first.rgba_sha256 }, null, 2));

function runViewer(input, output, declaration, offset) {
  const args = [VIEWER, "render", input, output];
  if (declaration) args.push(declaration);
  if (offset !== undefined) args.push(String(offset));
  return JSON.parse(execFileSync(process.execPath, args, {
    cwd: ORACLE_ROOT,
    encoding: "utf8",
    env: process.env,
  }));
}

function spawnViewer(name, svg, declaration) {
  const input = `target/svg-viewer-chromium-test/${name}.svg`;
  const output = `target/svg-viewer-chromium-test/${name}.rgba`;
  writeFileSync(resolve(REPO_ROOT, input), svg);
  const args = [VIEWER, "render", input, output];
  if (declaration) args.push(declaration);
  return spawnSync(process.execPath, args, {
    cwd: ORACLE_ROOT,
    encoding: "utf8",
    env: process.env,
  });
}

function assertRejected(name, svg, message, declaration) {
  const result = spawnViewer(name, svg, declaration);
  assert(result.status !== 0, `viewer admitted ${name}`);
  assert(result.stderr.includes(message), `unexpected ${name} rejection: ${result.stderr}`);
}

function writeDeclaration(name, declaration) {
  const path = `target/svg-viewer-chromium-test/${name}.assets.json`;
  writeFileSync(resolve(REPO_ROOT, path), `${JSON.stringify(declaration)}\n`);
  return path;
}

function fontDeclaration(hash, sfntHash) {
  return {
    source: { media_type: "font/woff2", sha256: hash },
    css: {
      family: "Declared",
      style: "normal",
      weight: 400,
      stretch: "normal",
      unicode_range: ["U+0-10FFFF"],
    },
    sfnt: {
      path: "target/svg-viewer-chromium-test/reviewed.ttf",
      media_type: "font/ttf",
      sha256: sfntHash,
      face_index: 0,
      family: "Reviewed",
      style: "normal",
      weight: 400,
      stretch: "normal",
    },
  };
}

function simpleSvg() {
  return '<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"/>';
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
