import { createHash } from "node:crypto";
import { lstatSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { basename, dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { inflateSync } from "node:zlib";

import { chromium } from "playwright";

const ORACLE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO_ROOT = resolve(ORACLE_ROOT, "../..");
const ORIGIN = "http://reference.invalid";
const INPUT_URL = `${ORIGIN}/input.svg`;
const ASSET_DECLARATION_ID = "excalidraw-render/svg-viewer-asset-declaration/v1";
const EMPTY_ASSET_DECLARATION = Object.freeze({ fonts: [], images: [] });
const FONT_STRETCHES = new Set([
  "ultra-condensed",
  "extra-condensed",
  "condensed",
  "semi-condensed",
  "normal",
  "semi-expanded",
  "expanded",
  "extra-expanded",
  "ultra-expanded",
]);
const FONT_STYLES = new Set(["normal", "italic", "oblique"]);
const IMAGE_MEDIA_TYPES = new Set([
  "image/png",
  "image/apng",
  "image/jpeg",
  "image/jfif",
  "image/gif",
  "image/webp",
  "image/bmp",
  "image/x-icon",
  "image/avif",
  "image/svg+xml",
]);
const BROWSER_VERSION = "149.0.7827.55";
const BROWSER_ARGUMENTS = [
  "--disable-gpu",
  "--font-render-hinting=none",
  "--force-color-profile=srgb",
];
const CONTEXT_OPTIONS = {
  colorScheme: "light",
  deviceScaleFactor: 1,
  forcedColors: "none",
  locale: "en-US",
  reducedMotion: "reduce",
  serviceWorkers: "block",
  timezoneId: "UTC",
  viewport: { height: 720, width: 1280 },
};

await main();

async function main() {
  const [command, inputArgument, outputArgument, declarationArgument, offsetArgument, ...rest] = process.argv.slice(2);
  if (command !== "render" || !inputArgument || !outputArgument || rest.length !== 0) {
    throw new Error("usage: node scripts/svg-viewer.mjs render <repository-relative-input.svg> <repository-relative-output.rgba> [repository-relative-assets.json] [animation-offset-microseconds]");
  }
  assertCanonicalEnvironment();
  const input = repositoryInputPath(inputArgument);
  const output = repositoryOutputPath(outputArgument);
  const assets = declarationArgument
    ? readAssetDeclaration(repositoryInputPath(declarationArgument))
    : EMPTY_ASSET_DECLARATION;
  const animationOffsetMicroseconds = parseAnimationOffset(offsetArgument);
  const svg = readFileSync(input);
  const nestedSvgs = validateSvg(svg, assets);
  for (const nested of nestedSvgs) {
    await render(nested, animationOffsetMicroseconds);
  }
  const result = await render(svg, animationOffsetMicroseconds);
  if (lstatSync(output, { throwIfNoEntry: false })?.isSymbolicLink()) {
    throw new Error(`output path must not be a symlink: ${outputArgument}`);
  }
  writeFileSync(output, result.rgba, { flag: "wx" });
  delete result.rgba;
  console.log(JSON.stringify(result));
}

function assertCanonicalEnvironment() {
  if (process.env.REFERENCE_ORACLE_CANONICAL !== "1"
      || process.platform !== "linux"
      || process.arch !== "x64"
      || process.version !== "v24.16.0") {
    throw new Error("run the Chromium SVG viewer in the canonical reference-profile container");
  }
}

function lexicalRepositoryPath(argument) {
  if (!argument || argument.startsWith("/") || argument.split(/[\\/]/).includes("..")) {
    throw new Error(`unsafe repository-relative path: ${argument}`);
  }
  const path = resolve(REPO_ROOT, argument);
  if (path !== REPO_ROOT && !path.startsWith(`${REPO_ROOT}${sep}`)) {
    throw new Error(`path escaped repository root: ${argument}`);
  }
  return path;
}

function repositoryInputPath(argument) {
  const root = realpathSync(REPO_ROOT);
  const path = realpathSync(lexicalRepositoryPath(argument));
  if (path !== root && !path.startsWith(`${root}${sep}`)) {
    throw new Error(`path resolves outside repository root: ${argument}`);
  }
  return path;
}

function repositoryOutputPath(argument) {
  const path = lexicalRepositoryPath(argument);
  const root = realpathSync(REPO_ROOT);
  const parent = realpathSync(dirname(path));
  if (parent !== root && !parent.startsWith(`${root}${sep}`)) {
    throw new Error(`output parent resolves outside repository root: ${argument}`);
  }
  return resolve(parent, basename(path));
}

function parseAnimationOffset(value) {
  if (value === undefined) return 0;
  if (!/^(?:0|[1-9][0-9]*)$/u.test(value)) {
    throw new Error(`animation offset must be a nonnegative integer count of microseconds: ${value}`);
  }
  const offset = Number(value);
  if (!Number.isSafeInteger(offset)) throw new Error("animation offset exceeds exact integer range");
  return offset;
}

function readAssetDeclaration(path) {
  const bytes = readFileSync(path);
  let text;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    throw new Error("asset declaration is not UTF-8");
  }
  let value;
  try {
    value = JSON.parse(text);
  } catch (error) {
    throw new Error(`invalid asset declaration JSON: ${error.message}`);
  }
  assertObject(value, "asset declaration", ["schema", "fonts", "images"]);
  if (value.schema !== ASSET_DECLARATION_ID) {
    throw new Error(`unsupported asset declaration schema: ${value.schema}`);
  }
  if (!Array.isArray(value.fonts) || !Array.isArray(value.images)) {
    throw new Error("asset declaration fonts and images must be arrays");
  }

  const fonts = value.fonts.map((font, index) => parseDeclaredFont(font, index));
  const images = value.images.map((image, index) => parseDeclaredImage(image, index));
  rejectDuplicate(fonts, (font) => font.source.key, "font source identity");
  rejectDuplicate(fonts, (font) => faceKey(font.css), "font CSS face");
  rejectDuplicate(images, (image) => image.source.key, "image source identity");
  return {
    fonts,
    fontBySource: new Map(fonts.map((font) => [font.source.key, font])),
    images,
    imageBySource: new Map(images.map((image) => [image.source.key, image])),
  };
}

function parseDeclaredFont(value, index) {
  const label = `fonts[${index}]`;
  assertObject(value, label, ["source", "css", "sfnt"]);
  const source = parseIdentity(value.source, `${label}.source`, new Set(["font/woff2"]));
  const css = parseFace(value.css, `${label}.css`, true);
  const sfnt = parseDescriptor(value.sfnt, `${label}.sfnt`, new Set(["font/ttf", "font/otf"]), true);
  return { source, css, sfnt };
}

function parseDeclaredImage(value, index) {
  const label = `images[${index}]`;
  assertObject(value, label, ["source", "canonical"]);
  const source = parseIdentity(value.source, `${label}.source`, IMAGE_MEDIA_TYPES);
  const canonical = parseDescriptor(value.canonical, `${label}.canonical`, new Set(["image/png", "image/svg+xml"]), false);
  const expected = source.mediaType === "image/svg+xml" ? "image/svg+xml" : "image/png";
  if (canonical.mediaType !== expected) {
    throw new Error(`${label}.canonical.media_type must be ${expected} for ${source.mediaType}`);
  }
  return { source, canonical };
}

function parseIdentity(value, label, mediaTypes) {
  assertObject(value, label, ["media_type", "sha256"]);
  if (!mediaTypes.has(value.media_type)) throw new Error(`${label}.media_type is not admitted`);
  const hash = parseSha256(value.sha256, `${label}.sha256`);
  return { mediaType: value.media_type, hash, key: identityKey(value.media_type, hash) };
}

function parseDescriptor(value, label, mediaTypes, font) {
  const keys = font
    ? ["path", "media_type", "sha256", "face_index", "family", "style", "weight", "stretch"]
    : ["path", "media_type", "sha256"];
  assertObject(value, label, keys);
  if (typeof value.path !== "string") throw new Error(`${label}.path must be a string`);
  validateRepositoryDescriptorPath(value.path, `${label}.path`);
  const path = repositoryInputPath(value.path);
  if (!mediaTypes.has(value.media_type)) throw new Error(`${label}.media_type is not admitted`);
  const hash = parseSha256(value.sha256, `${label}.sha256`);
  const bytes = readFileSync(path);
  if (sha256(bytes) !== hash) throw new Error(`${label} descriptor SHA-256 mismatch`);
  validateMedia(value.media_type, bytes);
  const descriptor = {
    path: value.path,
    mediaType: value.media_type,
    hash,
  };
  if (!font) return descriptor;
  if (!Number.isInteger(value.face_index) || value.face_index < 0 || value.face_index > 0xffffffff) {
    throw new Error(`${label}.face_index must be a u32`);
  }
  return { ...descriptor, faceIndex: value.face_index, ...parseFaceFields(value, label) };
}

function parseFace(value, label, unicodeRange) {
  const keys = unicodeRange
    ? ["family", "style", "weight", "stretch", "unicode_range"]
    : ["family", "style", "weight", "stretch"];
  assertObject(value, label, keys);
  const face = parseFaceFields(value, label);
  if (!unicodeRange) return face;
  if (!Array.isArray(value.unicode_range) || value.unicode_range.length === 0
      || value.unicode_range.some((range) => typeof range !== "string")) {
    throw new Error(`${label}.unicode_range must be a non-empty string array`);
  }
  return { ...face, unicodeRange: parseUnicodeRanges(value.unicode_range.join(","), label) };
}

function parseFaceFields(value, label) {
  if (typeof value.family !== "string" || value.family.length === 0 || /[\u0000-\u001f\u007f]/u.test(value.family)) {
    throw new Error(`${label}.family must be a non-empty printable string`);
  }
  if (!FONT_STYLES.has(value.style)) throw new Error(`${label}.style is not admitted`);
  if (!Number.isInteger(value.weight) || value.weight < 1 || value.weight > 1000) {
    throw new Error(`${label}.weight must be an integer from 1 through 1000`);
  }
  if (!FONT_STRETCHES.has(value.stretch)) throw new Error(`${label}.stretch is not admitted`);
  return { family: value.family, style: value.style, weight: value.weight, stretch: value.stretch };
}

function parseSha256(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    throw new Error(`${label} must be a lowercase SHA-256 digest`);
  }
  return value;
}

function assertObject(value, label, keys) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  if (actual.length !== expected.length || actual.some((key, index) => key !== expected[index])) {
    throw new Error(`${label} must contain exactly: ${expected.join(", ")}`);
  }
}

function rejectDuplicate(values, key, label) {
  const seen = new Set();
  for (const value of values) {
    const identity = key(value);
    if (seen.has(identity)) throw new Error(`duplicate ${label}: ${identity}`);
    seen.add(identity);
  }
}

function validateSvg(bytes, assets, depth = 0) {
  if (depth > 8) throw new Error("nested SVG image depth exceeds 8");
  let svg;
  try {
    svg = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    throw new Error("SVG is not UTF-8");
  }
  if (!/^\s*<svg(?:\s|>)/u.test(svg)) {
    throw new Error("input is not a top-level SVG document");
  }
  if (/<!DOCTYPE\b|<!ENTITY\b|<(?:[A-Za-z_][\w.-]*:)?(?:script|foreignObject)\b|\s+on[a-z]+\s*=/iu.test(svg)) {
    throw new Error("forbidden active SVG content");
  }
  if (/&#(?:x[0-9a-f]+|[0-9]+);/iu.test(svg)) {
    throw new Error("numeric XML character references are not admitted");
  }

  let declaredFontFaces = 0;
  const nestedSvgs = [];
  for (const match of svg.matchAll(/<(?:[A-Za-z_][\w.-]*:)?style\b[^>]*>([\s\S]*?)<\/(?:[A-Za-z_][\w.-]*:)?style\s*>/giu)) {
    declaredFontFaces += validateStylesheet(decodeXmlEntities(match[1]), assets);
  }
  const withoutStyles = svg.replace(/<(?:[A-Za-z_][\w.-]*:)?style\b[^>]*>[\s\S]*?<\/(?:[A-Za-z_][\w.-]*:)?style\s*>/giu, "");
  if (/@font-face\b/iu.test(withoutStyles)) {
    throw new Error("@font-face is only admitted in SVG style elements");
  }
  if (/<(?:[A-Za-z_][\w.-]*:)?text\b/iu.test(svg) && declaredFontFaces === 0) {
    throw new Error("undeclared font resource: text requires a declared embedded font face");
  }

  const admittedImageUrls = new Set();
  for (const match of svg.matchAll(/<(?:[A-Za-z_][\w.-]*:)?image\b[^>]*>/giu)) {
    const hrefs = [...match[0].matchAll(/(?:^|\s)(?:[A-Za-z_][\w.-]*:)?href\s*=\s*(["'])(.*?)\1/giu)];
    if (hrefs.length !== 1) throw new Error("each SVG image must have exactly one href");
    const dataUrl = decodeXmlEntities(hrefs[0][2].trim());
    const payload = parseDataUrl(dataUrl, "image");
    if (!IMAGE_MEDIA_TYPES.has(payload.mediaType)) {
      throw new Error(`forbidden image media type: ${payload.mediaType}`);
    }
    validateMedia(payload.mediaType, payload.bytes);
    const identity = identityKey(payload.mediaType, sha256(payload.bytes));
    if (!assets.imageBySource?.has(identity)) {
      throw new Error(`undeclared embedded image resource: ${identity}`);
    }
    if (payload.mediaType === "image/svg+xml") {
      nestedSvgs.push(payload.bytes, ...validateSvg(payload.bytes, assets, depth + 1));
    }
    admittedImageUrls.add(dataUrl);
  }

  for (const match of svg.matchAll(/\b(?:href|src)\s*=\s*(["'])(.*?)\1/giu)) {
    const value = decodeXmlEntities(match[2].trim());
    const tagStart = svg.lastIndexOf("<", match.index);
    const tag = svg.slice(tagStart, match.index);
    if (/^<a\b/iu.test(tag)) continue;
    if (/^<(?:[A-Za-z_][\w.-]*:)?image\b/iu.test(tag) && admittedImageUrls.has(value)) continue;
    if (!value.startsWith("#")) {
      throw new Error(`forbidden external SVG resource: ${value}`);
    }
  }
  for (const match of svg.matchAll(/\s[A-Za-z_:][\w:.-]*\s*=\s*(["'])(.*?)\1/gu)) {
    validateCssResources(decodeXmlEntities(match[2]));
  }
  for (const match of svg.matchAll(/<(?:[A-Za-z_][\w.-]*:)?(?:animate|set)\b[^>]*>/giu)) {
    const attributeName = /\battributeName\s*=\s*(["'])(.*?)\1/iu.exec(match[0])?.[2]?.toLowerCase();
    if (["href", "xlink:href", "src"].includes(attributeName)) {
      throw new Error(`animated resource attribute is forbidden: ${attributeName}`);
    }
  }
  return nestedSvgs;
}

function validateStylesheet(css, assets) {
  const clean = stripCssComments(css);
  if (clean.includes("\\")) throw new Error("CSS escapes are not admitted in SVG resources");
  if (/@import\b/iu.test(clean)) throw new Error("forbidden stylesheet resource");
  const blocks = extractFontFaceBlocks(clean);
  const masked = [...clean];
  for (const block of blocks) {
    validateFontFace(block.body, assets);
    masked.fill(" ", block.start, block.end);
  }
  validateCssResources(masked.join(""));
  return blocks.length;
}

function validateFontFace(body, assets) {
  const declarations = new Map();
  for (const declaration of splitCssDeclarations(body)) {
    const colon = declaration.indexOf(":");
    if (colon <= 0) throw new Error(`invalid @font-face declaration: ${declaration}`);
    const name = declaration.slice(0, colon).trim().toLowerCase();
    const value = declaration.slice(colon + 1).trim();
    if (!new Set(["font-family", "font-style", "font-weight", "font-stretch", "unicode-range", "src"]).has(name)) {
      throw new Error(`unsupported @font-face descriptor: ${name}`);
    }
    if (declarations.has(name)) throw new Error(`duplicate @font-face descriptor: ${name}`);
    declarations.set(name, value);
  }
  if (!declarations.has("font-family") || !declarations.has("src")) {
    throw new Error("@font-face requires font-family and src descriptors");
  }
  const family = parseCssFamily(declarations.get("font-family"));
  const style = (declarations.get("font-style") ?? "normal").toLowerCase();
  const weight = parseCssWeight(declarations.get("font-weight") ?? "normal");
  const stretch = (declarations.get("font-stretch") ?? "normal").toLowerCase();
  if (!FONT_STYLES.has(style)) throw new Error(`unsupported @font-face style: ${style}`);
  if (!FONT_STRETCHES.has(stretch)) throw new Error(`unsupported @font-face stretch: ${stretch}`);
  const unicodeRange = parseUnicodeRanges(declarations.get("unicode-range") ?? "U+0-10FFFF", "@font-face");
  const source = parseFontSource(declarations.get("src"));
  validateMedia(source.mediaType, source.bytes);
  const identity = identityKey(source.mediaType, sha256(source.bytes));
  const declared = assets.fontBySource?.get(identity);
  if (!declared) throw new Error(`undeclared embedded font resource: ${identity}`);
  const actual = { family, style, weight, stretch, unicodeRange };
  if (faceKey(actual) !== faceKey(declared.css)
      || JSON.stringify(actual.unicodeRange) !== JSON.stringify(declared.css.unicodeRange)) {
    throw new Error(`@font-face metadata does not match declaration for ${identity}`);
  }
}

function extractFontFaceBlocks(css) {
  const blocks = [];
  const lower = css.toLowerCase();
  let cursor = 0;
  while (true) {
    const start = lower.indexOf("@font-face", cursor);
    if (start < 0) return blocks;
    let open = start + "@font-face".length;
    while (/\s/u.test(css[open] ?? "")) open += 1;
    if (css[open] !== "{") throw new Error("invalid @font-face rule");
    let quote = null;
    let depth = 1;
    let end = open + 1;
    for (; end < css.length && depth > 0; end += 1) {
      const character = css[end];
      if (quote) {
        if (character === quote) quote = null;
      } else if (character === "'" || character === "\"") {
        quote = character;
      } else if (character === "{") {
        throw new Error("nested @font-face blocks are not admitted");
      } else if (character === "}") {
        depth -= 1;
      }
    }
    if (depth !== 0 || quote) throw new Error("unterminated @font-face rule");
    blocks.push({ start, end, body: css.slice(open + 1, end - 1) });
    cursor = end;
  }
}

function splitCssDeclarations(body) {
  const declarations = [];
  let start = 0;
  let quote = null;
  let parentheses = 0;
  for (let index = 0; index <= body.length; index += 1) {
    const character = body[index] ?? ";";
    if (quote) {
      if (character === quote) quote = null;
    } else if (character === "'" || character === "\"") {
      quote = character;
    } else if (character === "(") {
      parentheses += 1;
    } else if (character === ")") {
      parentheses -= 1;
      if (parentheses < 0) throw new Error("invalid CSS parentheses");
    } else if (character === ";" && parentheses === 0) {
      const declaration = body.slice(start, index).trim();
      if (declaration) declarations.push(declaration);
      start = index + 1;
    }
  }
  if (quote || parentheses !== 0) throw new Error("unterminated CSS declaration");
  return declarations;
}

function parseFontSource(value) {
  const { url, rest } = parseCssUrl(value);
  if (!/^format\(\s*(["']?)woff2\1\s*\)$/iu.test(rest)) {
    throw new Error("@font-face src must be one WOFF2 data URL with format(woff2)");
  }
  const payload = parseDataUrl(url, "font");
  if (payload.mediaType !== "font/woff2") throw new Error(`forbidden font media type: ${payload.mediaType}`);
  return payload;
}

function parseCssUrl(value) {
  const match = /^url\(\s*/iu.exec(value);
  if (!match) throw new Error("expected CSS url()");
  let cursor = match[0].length;
  const quote = value[cursor] === "'" || value[cursor] === "\"" ? value[cursor++] : null;
  const start = cursor;
  if (quote) {
    while (cursor < value.length && value[cursor] !== quote) cursor += 1;
    if (cursor === value.length) throw new Error("unterminated quoted CSS URL");
  } else {
    while (cursor < value.length && value[cursor] !== ")") cursor += 1;
  }
  const url = value.slice(start, cursor).trim();
  if (quote) cursor += 1;
  while (/\s/u.test(value[cursor] ?? "")) cursor += 1;
  if (value[cursor] !== ")") throw new Error("invalid CSS URL");
  return { url, rest: value.slice(cursor + 1).trim() };
}

function parseCssFamily(value) {
  const family = unquote(value.trim());
  if (!family || family.includes(",") || /[\u0000-\u001f\u007f]/u.test(family)) {
    throw new Error("@font-face family must be one printable family name");
  }
  return family;
}

function parseCssWeight(value) {
  const lower = value.toLowerCase();
  if (lower === "normal") return 400;
  if (lower === "bold") return 700;
  if (!/^[1-9][0-9]{0,2}$|^1000$/u.test(lower)) throw new Error(`unsupported @font-face weight: ${value}`);
  return Number(lower);
}

function parseUnicodeRanges(value, label) {
  const ranges = value.split(",").map((range) => {
    const token = range.trim().toUpperCase();
    const match = /^U\+([0-9A-F]{1,6})(?:-([0-9A-F]{1,6})|(\?{1,5}))?$/u.exec(token);
    if (!match) throw new Error(`${label} has invalid unicode-range: ${range.trim()}`);
    let start;
    let end;
    if (match[3]) {
      start = Number.parseInt(match[1] + "0".repeat(match[3].length), 16);
      end = Number.parseInt(match[1] + "F".repeat(match[3].length), 16);
    } else {
      start = Number.parseInt(match[1], 16);
      end = Number.parseInt(match[2] ?? match[1], 16);
    }
    if (start > end || end > 0x10ffff) throw new Error(`${label} unicode-range is outside Unicode`);
    return [start, end];
  });
  ranges.sort((left, right) => left[0] - right[0] || left[1] - right[1]);
  for (let index = 1; index < ranges.length; index += 1) {
    if (ranges[index][0] <= ranges[index - 1][1]) throw new Error(`${label} unicode-range entries overlap`);
  }
  return ranges;
}

function validateCssResources(css) {
  const clean = stripCssComments(css);
  if (clean.includes("\\")) throw new Error("CSS escapes are not admitted in SVG resources");
  if (/@import\b/iu.test(clean)) throw new Error("forbidden stylesheet resource");
  if (/@font-face\b/iu.test(clean)) throw new Error("@font-face is only admitted in SVG style elements");
  let remainder = clean;
  while (/url\(/iu.test(remainder)) {
    const index = remainder.search(/url\(/iu);
    const parsed = parseCssUrl(remainder.slice(index));
    if (!parsed.url.startsWith("#")) throw new Error(`forbidden CSS resource: ${parsed.url}`);
    remainder = parsed.rest;
  }
}

function stripCssComments(css) {
  if (/\/\*[\s\S]*$/u.test(css.replace(/\/\*[\s\S]*?\*\//gu, ""))) {
    throw new Error("unterminated CSS comment");
  }
  return css.replace(/\/\*[\s\S]*?\*\//gu, "");
}

function parseDataUrl(value, kind) {
  const match = /^data:([^;,]+);base64,([A-Za-z0-9+/]*={0,2})$/u.exec(value);
  if (!match) throw new Error(`${kind} resource must be a canonical base64 data URL`);
  const bytes = Buffer.from(match[2], "base64");
  if (bytes.toString("base64") !== match[2]) throw new Error(`${kind} resource has non-canonical base64`);
  return { mediaType: match[1].toLowerCase(), bytes };
}

function validateMedia(mediaType, bytes) {
  let valid = false;
  if (mediaType === "font/woff2") valid = bytes.subarray(0, 4).toString("ascii") === "wOF2";
  else if (mediaType === "font/ttf") valid = bytes.subarray(0, 4).equals(Buffer.from([0, 1, 0, 0])) || bytes.subarray(0, 4).toString("ascii") === "true";
  else if (mediaType === "font/otf") valid = bytes.subarray(0, 4).toString("ascii") === "OTTO";
  else if (mediaType === "image/png" || mediaType === "image/apng") valid = bytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
  else if (mediaType === "image/jpeg" || mediaType === "image/jfif") valid = bytes.length >= 3 && bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff;
  else if (mediaType === "image/gif") valid = ["GIF87a", "GIF89a"].includes(bytes.subarray(0, 6).toString("ascii"));
  else if (mediaType === "image/webp") {
    valid = bytes.subarray(0, 4).toString("ascii") === "RIFF" && bytes.subarray(8, 12).toString("ascii") === "WEBP";
  } else if (mediaType === "image/bmp") {
    valid = bytes.subarray(0, 2).toString("ascii") === "BM";
  } else if (mediaType === "image/x-icon") {
    valid = bytes.subarray(0, 4).equals(Buffer.from([0, 0, 1, 0]));
  } else if (mediaType === "image/avif") {
    valid = bytes.subarray(4, 8).toString("ascii") === "ftyp"
      && /^(?:avif|avis)$/u.test(bytes.subarray(8, 32).toString("ascii").match(/.{4}/gu)?.find((brand) => /^(?:avif|avis)$/u.test(brand)) ?? "");
  } else if (mediaType === "image/svg+xml") {
    const text = bytes.toString("utf8");
    valid = Buffer.byteLength(text, "utf8") === bytes.length && /^\s*<svg(?:\s|>)/u.test(text);
  }
  if (!valid) throw new Error(`payload bytes do not match declared media type ${mediaType}`);
}

function decodeXmlEntities(value) {
  return value.replace(/&(amp|lt|gt|quot|apos);/gu, (_, entity) => ({
    amp: "&", lt: "<", gt: ">", quot: "\"", apos: "'",
  })[entity]);
}

function unquote(value) {
  if ((value.startsWith("\"") && value.endsWith("\""))
      || (value.startsWith("'") && value.endsWith("'"))) {
    return value.slice(1, -1);
  }
  return value;
}

function identityKey(mediaType, hash) {
  return `${mediaType}:${hash}`;
}

function validateRepositoryDescriptorPath(path, label) {
  if (!path || path.startsWith("/") || path.includes("\\")
      || path.split("/").some((component) => !component || component === "." || component === "..")) {
    throw new Error(`${label} must be a normalized repository-relative path`);
  }
}

function faceKey(face) {
  return `${face.family}\u0000${face.stretch}\u0000${face.style}\u0000${face.weight}`;
}

async function render(svg, animationOffsetMicroseconds) {
  const browser = await chromium.launch({
    args: BROWSER_ARGUMENTS,
    chromiumSandbox: false,
    headless: true,
  });
  try {
    if (browser.version() !== BROWSER_VERSION) {
      throw new Error(`expected Chromium ${BROWSER_VERSION}, received ${browser.version()}`);
    }
    const browserSession = await browser.newBrowserCDPSession();
    const processInfo = await browserSession.send("SystemInfo.getProcessInfo");
    const browserProcess = processInfo.processInfo.find((process) => process.type === "browser");
    if (!browserProcess) throw new Error("Chromium did not report its browser process ID");

    const context = await browser.newContext(CONTEXT_OPTIONS);
    try {
      const page = await context.newPage();
      const session = await context.newCDPSession(page);
      const errors = [];
      const requests = [];
      page.on("console", (message) => {
        if (message.type() === "error") errors.push(`console error: ${message.text()}`);
      });
      page.on("pageerror", (error) => errors.push(`page error: ${error.message}`));
      page.on("requestfailed", (request) => {
        errors.push(`request failed: ${request.url()} (${request.failure()?.errorText ?? "unknown"})`);
      });
      await page.route("**/*", async (route) => {
        const url = route.request().url();
        requests.push(url);
        if (url !== INPUT_URL) {
          errors.push(`forbidden request: ${url}`);
          await route.abort("blockedbyclient");
          return;
        }
        await route.fulfill({
          body: svg,
          contentType: "image/svg+xml; charset=utf-8",
          status: 200,
        });
      });

      const virtualTime = await session.send("Emulation.setVirtualTimePolicy", {
        budget: 0,
        initialVirtualTime: 0,
        policy: "pauseIfNetworkFetchesPending",
      });
      await session.send("Emulation.setDefaultBackgroundColorOverride", {
        color: { a: 0, b: 0, g: 0, r: 0 },
      });
      await page.goto(INPUT_URL, { waitUntil: "load" });
      await page.evaluate(async () => {
        await document.fonts.ready;
        await Promise.all([...document.images].map((image) => {
          if (image.complete && image.naturalWidth > 0) return undefined;
          return image.decode();
        }));
      });
      const dimensions = await page.evaluate(() => {
        const root = document.documentElement;
        if (root.namespaceURI !== "http://www.w3.org/2000/svg" || root.localName !== "svg") {
          throw new Error("document root is not SVG");
        }
        const bounds = root.getBoundingClientRect();
        return { height: bounds.height, width: bounds.width };
      });
      if (!Number.isInteger(dimensions.width) || !Number.isInteger(dimensions.height)
          || dimensions.width <= 0 || dimensions.height <= 0) {
        throw new Error(`SVG dimensions must be positive integers, got ${dimensions.width}x${dimensions.height}`);
      }
      await session.send("Emulation.setVirtualTimePolicy", { policy: "pause" });
      if (animationOffsetMicroseconds > 0) {
        const expired = new Promise((resolveExpired) => {
          session.once("Emulation.virtualTimeBudgetExpired", resolveExpired);
        });
        await session.send("Emulation.setVirtualTimePolicy", {
          budget: animationOffsetMicroseconds / 1000,
          policy: "advance",
        });
        await expired;
        await session.send("Emulation.setVirtualTimePolicy", { policy: "pause" });
      }
      const clip = { height: dimensions.height, scale: 1, width: dimensions.width, x: 0, y: 0 };
      const before = await page.evaluate(() => Date.now());
      for (let update = 0; update < 2; update += 1) {
        await session.send("Page.captureScreenshot", {
          captureBeyondViewport: true,
          clip,
          format: "png",
          fromSurface: true,
        });
      }
      const platformFonts = await auditPlatformFonts(session);
      if (platformFonts.some((font) => !font.isCustomFont)) {
        throw new Error(`ambient platform font used: ${JSON.stringify(platformFonts)}`);
      }
      const capture = await session.send("Page.captureScreenshot", {
        captureBeyondViewport: true,
        clip,
        format: "png",
        fromSurface: true,
      });
      const after = await page.evaluate(() => Date.now());
      if (before !== after) throw new Error("virtual time advanced during SVG capture");
      if (errors.length > 0) throw new Error(errors.join("\n"));
      const decoded = decodePng(Buffer.from(capture.data, "base64"));
      if (decoded.width !== dimensions.width || decoded.height !== dimensions.height) {
        throw new Error("captured PNG dimensions differ from the SVG document");
      }
      for (let offset = 0; offset < decoded.rgba.length; offset += 4) {
        if (decoded.rgba[offset + 3] === 0) decoded.rgba.fill(0, offset, offset + 3);
      }
      return {
        browser_process_id: browserProcess.id,
        browser_version: browser.version(),
        height: decoded.height,
        lifecycle_updates: 2,
        animation_offset_microseconds: animationOffsetMicroseconds,
        platform_fonts: platformFonts,
        requests: [...new Set(requests)].sort(),
        rgba: decoded.rgba,
        rgba_sha256: sha256(decoded.rgba),
        virtual_time_ticks_base: virtualTime.virtualTimeTicksBase,
        width: decoded.width,
      };
    } finally {
      await context.close();
    }
  } finally {
    await browser.close();
  }
}

async function auditPlatformFonts(session) {
  await session.send("DOM.enable");
  await session.send("CSS.enable");
  const { root } = await session.send("DOM.getDocument", { depth: -1, pierce: true });
  const textNodes = [];
  visit(root);
  const usages = [];
  for (const node of textNodes) {
    const { fonts } = await session.send("CSS.getPlatformFontsForNode", { nodeId: node.nodeId });
    for (const font of fonts) {
      usages.push({
        familyName: font.familyName,
        glyphCount: font.glyphCount,
        isCustomFont: font.isCustomFont,
        postScriptName: font.postScriptName,
      });
    }
  }
  return usages;

  function visit(node) {
    if (node.localName === "text" && node.namespaceURI === "http://www.w3.org/2000/svg") {
      textNodes.push(node);
    }
    for (const child of node.children ?? []) visit(child);
    for (const shadow of node.shadowRoots ?? []) visit(shadow);
  }
}

function decodePng(png) {
  if (!png.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]))) {
    throw new Error("Chromium capture is not PNG");
  }
  let offset = 8;
  let header;
  const compressed = [];
  while (offset < png.length) {
    const length = png.readUInt32BE(offset);
    const type = png.toString("ascii", offset + 4, offset + 8);
    const data = png.subarray(offset + 8, offset + 8 + length);
    if (type === "IHDR") {
      header = {
        bitDepth: data[8],
        colorType: data[9],
        compression: data[10],
        filter: data[11],
        height: data.readUInt32BE(4),
        interlace: data[12],
        width: data.readUInt32BE(0),
      };
    } else if (type === "IDAT") {
      compressed.push(data);
    } else if (type === "IEND") {
      break;
    }
    offset += length + 12;
  }
  if (!header || header.bitDepth !== 8 || ![2, 6].includes(header.colorType)
      || header.compression !== 0 || header.filter !== 0 || header.interlace !== 0) {
    throw new Error(`unsupported Chromium PNG header: ${JSON.stringify(header)}`);
  }
  const channels = header.colorType === 6 ? 4 : 3;
  const rowBytes = header.width * channels;
  const filtered = inflateSync(Buffer.concat(compressed));
  if (filtered.length !== (rowBytes + 1) * header.height) {
    throw new Error("Chromium PNG has an unexpected decoded length");
  }
  const scanlines = Buffer.alloc(rowBytes * header.height);
  for (let y = 0; y < header.height; y += 1) {
    const source = y * (rowBytes + 1);
    const target = y * rowBytes;
    const filter = filtered[source];
    for (let x = 0; x < rowBytes; x += 1) {
      const raw = filtered[source + 1 + x];
      const left = x >= channels ? scanlines[target + x - channels] : 0;
      const up = y > 0 ? scanlines[target + x - rowBytes] : 0;
      const upperLeft = y > 0 && x >= channels ? scanlines[target + x - rowBytes - channels] : 0;
      let value;
      if (filter === 0) value = raw;
      else if (filter === 1) value = raw + left;
      else if (filter === 2) value = raw + up;
      else if (filter === 3) value = raw + Math.floor((left + up) / 2);
      else if (filter === 4) value = raw + paeth(left, up, upperLeft);
      else throw new Error(`unsupported PNG filter ${filter}`);
      scanlines[target + x] = value & 255;
    }
  }
  if (channels === 4) return { ...header, rgba: scanlines };
  const rgba = Buffer.alloc(header.width * header.height * 4);
  for (let source = 0, target = 0; source < scanlines.length; source += 3, target += 4) {
    scanlines.copy(rgba, target, source, source + 3);
    rgba[target + 3] = 255;
  }
  return { ...header, rgba };
}

function paeth(left, up, upperLeft) {
  const estimate = left + up - upperLeft;
  const leftDistance = Math.abs(estimate - left);
  const upDistance = Math.abs(estimate - up);
  const upperLeftDistance = Math.abs(estimate - upperLeft);
  if (leftDistance <= upDistance && leftDistance <= upperLeftDistance) return left;
  if (upDistance <= upperLeftDistance) return up;
  return upperLeft;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}
