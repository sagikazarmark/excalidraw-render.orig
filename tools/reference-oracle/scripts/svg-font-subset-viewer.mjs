import { createHash } from "node:crypto";
import { lstatSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { basename, dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { inflateSync } from "node:zlib";

import { chromium } from "playwright";

const ORACLE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO_ROOT = resolve(ORACLE_ROOT, "../..");
const INPUT_URL = "http://reference.invalid/input.svg";
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
  const [inputsArgument, outputArgument, ...rest] = process.argv.slice(2);
  if (!inputsArgument || !outputArgument || rest.length !== 0) {
    throw new Error("usage: node scripts/svg-font-subset-viewer.mjs <repository-relative-inputs.json> <repository-relative-report.json>");
  }
  assertCanonicalEnvironment();
  const inputsPath = repositoryInputPath(inputsArgument);
  const outputPath = repositoryOutputPath(outputArgument);
  const inputBytes = readFileSync(inputsPath);
  const inputs = JSON.parse(inputBytes.toString("utf8"));
  if (inputs.schema !== "excalidraw-render/svg-font-subset-viewer-inputs/v1"
      || !Array.isArray(inputs.cases) || inputs.cases.length !== 851) {
    throw new Error("invalid SVG font subset viewer input manifest");
  }
  inputs.cases.forEach(validateCase);

  const [source, sourceRepeat, subset, subsetRepeat] = await Promise.all([
    runRole(inputs.cases, "source", 0),
    runRole(inputs.cases, "source", 1),
    runRole(inputs.cases, "subset", 0),
    runRole(inputs.cases, "subset", 1),
  ]);
  const results = inputs.cases.map((entry, index) => {
    const roles = [source[index], sourceRepeat[index], subset[index], subsetRepeat[index]];
    if (roles.some((result) => result.id !== entry.semantic_subset_identity)) {
      throw new Error(`viewer result order diverged for ${entry.semantic_subset_identity}`);
    }
    const sourceDeterministic = source[index].rasterSha256 === sourceRepeat[index].rasterSha256
      && source[index].semanticSha256 === sourceRepeat[index].semanticSha256;
    const subsetDeterministic = subset[index].rasterSha256 === subsetRepeat[index].rasterSha256
      && subset[index].semanticSha256 === subsetRepeat[index].semanticSha256;
    const rasterEqual = source[index].rasterSha256 === subset[index].rasterSha256;
    const checks = semanticChecks(source[index].semantic, subset[index].semantic, rasterEqual);
    const passed = sourceDeterministic && subsetDeterministic
      && Object.values(checks).every((decision) => decision === "pass");
    return {
      checks,
      comparison: {
        changed_pixels: rasterEqual ? 0 : 1,
        height: entry.dimensions.height,
        passed: rasterEqual,
        ssim_bits: rasterEqual ? "3ff0000000000000" : "0000000000000000",
        visual_class: "typography",
        width: entry.dimensions.width,
      },
      compatibility_record: null,
      decision: passed ? "pass" : "fail",
      dimensions: entry.dimensions,
      semantic_subset_identity: entry.semantic_subset_identity,
      source_raster_sha256: source[index].rasterSha256,
      source_run_sha256: [source[index].rasterSha256, sourceRepeat[index].rasterSha256],
      source_semantic_sha256: source[index].semanticSha256,
      subset_raster_sha256: subset[index].rasterSha256,
      subset_run_sha256: [subset[index].rasterSha256, subsetRepeat[index].rasterSha256],
      subset_semantic_sha256: subset[index].semanticSha256,
    };
  });
  const report = {
    environment: {
      browser_arguments: BROWSER_ARGUMENTS.join(" "),
      chromium: BROWSER_VERSION,
      container: "mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9",
      host: "linux-amd64",
      locale: "en-US",
      node: process.version,
      playwright: "1.61.0",
      role_processes: "fresh Chromium process per execution, role, and repeat",
      timezone: "UTC",
    },
    fresh_state_per_role: true,
    inputs_sha256: sha256(inputBytes),
    results,
    samples_per_role: 2,
    schema: "excalidraw-render/svg-font-subset-viewer-report/v1",
    viewer: "chromium",
  };
  if (results.some((result) => result.decision !== "pass")) {
    throw new Error("one or more Chromium font subset qualifications failed");
  }
  if (lstatSync(outputPath, { throwIfNoEntry: false })?.isSymbolicLink()) {
    throw new Error(`output path must not be a symlink: ${outputArgument}`);
  }
  writeFileSync(outputPath, `${canonicalJson(report)}\n`, { flag: "w" });
}

async function runRole(cases, role, repeat) {
  const results = new Array(cases.length);
  let next = 0;
  await Promise.all(Array.from({ length: 3 }, async () => {
    while (next < cases.length) {
      const index = next;
      next += 1;
      const browser = await chromium.launch({
        args: BROWSER_ARGUMENTS,
        chromiumSandbox: false,
        headless: true,
      });
      try {
        if (browser.version() !== BROWSER_VERSION) {
          throw new Error(`expected Chromium ${BROWSER_VERSION}, received ${browser.version()}`);
        }
        results[index] = await renderCase(browser, cases[index], role);
      } finally {
        await browser.close();
      }
      if ((index + 1) % 50 === 0) {
        console.error(`Chromium ${role} repeat ${repeat}: ${index + 1}/${cases.length}`);
      }
    }
  }));
  return results;
}

async function renderCase(browser, entry, role) {
  const svg = readFileSync(repositoryInputPath(entry[`${role}_svg`]));
  const context = await browser.newContext(CONTEXT_OPTIONS);
  try {
    const page = await context.newPage();
    const session = await context.newCDPSession(page);
    const errors = [];
    const requests = [];
    page.on("console", (message) => {
      if (message.type() === "error" || /font|ots|decode/iu.test(message.text())) {
        errors.push(`console ${message.type()}: ${message.text()}`);
      }
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
      } else {
        await route.fulfill({ body: svg, contentType: "image/svg+xml; charset=utf-8", status: 200 });
      }
    });
    await session.send("Emulation.setVirtualTimePolicy", {
      budget: 0,
      initialVirtualTime: 0,
      policy: "pauseIfNetworkFetchesPending",
    });
    await session.send("Emulation.setDefaultBackgroundColorOverride", {
      color: { a: 0, b: 0, g: 0, r: 0 },
    });
    await page.goto(INPUT_URL, { waitUntil: "load" });
    await page.evaluate(async () => document.fonts.ready);
    await session.send("Emulation.setVirtualTimePolicy", { policy: "pause" });
    const semantic = await page.evaluate(() => {
      const root = document.documentElement;
      const text = root.querySelector("text");
      if (!(root instanceof SVGSVGElement) || !(text instanceof SVGTextElement)) {
        throw new Error("qualification document lacks one SVG text element");
      }
      const dimensions = root.getBoundingClientRect();
      const selection = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(text);
      selection.removeAllRanges();
      selection.addRange(range);
      const selected = [...selection.toString()].map((character) => character.codePointAt(0));
      selection.removeAllRanges();
      const characters = [];
      for (let index = 0; index < text.getNumberOfChars(); index += 1) {
        const start = text.getStartPositionOfChar(index);
        const end = text.getEndPositionOfChar(index);
        const extent = text.getExtentOfChar(index);
        characters.push({
          end: [end.x, end.y],
          extent: [extent.x, extent.y, extent.width, extent.height],
          rotation: text.getRotationOfChar(index),
          start: [start.x, start.y],
          substring_length: text.getSubStringLength(index, 1),
        });
      }
      const style = getComputedStyle(text);
      const fontFaces = [...document.fonts].map((face) => ({
        family: face.family,
        status: face.status,
        stretch: face.stretch,
        style: face.style,
        weight: face.weight,
      }));
      const fontCheck = document.fonts.check(
        `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${JSON.stringify(style.fontFamily)}`,
        text.textContent,
      );
      const canvas = document.createElementNS("http://www.w3.org/1999/xhtml", "canvas");
      const context = canvas.getContext("2d", { alpha: true });
      context.font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${JSON.stringify(style.fontFamily)}`;
      const lines = [...text.querySelectorAll("tspan")].map((line) => {
        const bounds = line.getBBox();
        const metrics = context.measureText(line.textContent);
        return {
          bounds: [bounds.x, bounds.y, bounds.width, bounds.height],
          computed_length: line.getComputedTextLength(),
          metrics: {
            actual_ascent: metrics.actualBoundingBoxAscent,
            actual_descent: metrics.actualBoundingBoxDescent,
            actual_left: metrics.actualBoundingBoxLeft,
            actual_right: metrics.actualBoundingBoxRight,
            font_ascent: metrics.fontBoundingBoxAscent,
            font_descent: metrics.fontBoundingBoxDescent,
            width: metrics.width,
          },
          scalars: [...line.textContent].map((character) => character.codePointAt(0)),
        };
      });
      return {
        characters,
        dimensions: [dimensions.width, dimensions.height],
        font_check: fontCheck,
        font_faces: fontFaces,
        lines,
        selected,
        text_bounds: (() => {
          const bounds = text.getBBox();
          return [bounds.x, bounds.y, bounds.width, bounds.height];
        })(),
        text_length: text.getComputedTextLength(),
      };
    });
    if (!semantic.font_check || semantic.font_faces.length !== 1
        || semantic.font_faces[0].status !== "loaded") {
      throw new Error(`declared font did not exclusively load: ${JSON.stringify(semantic.font_faces)}; ${errors.join("; ")}`);
    }
    if (semantic.dimensions[0] !== entry.dimensions.width
        || semantic.dimensions[1] !== entry.dimensions.height) {
      throw new Error(`dimension mismatch for ${entry.semantic_subset_identity}`);
    }
    const domText = semantic.lines.flatMap((line) => line.scalars);
    if (JSON.stringify(domText) !== JSON.stringify(entry.mapped_scalars)) {
      throw new Error(`DOM text mismatch for ${entry.semantic_subset_identity}`);
    }
    const clip = {
      height: entry.dimensions.height,
      scale: 1,
      width: entry.dimensions.width,
      x: 0,
      y: 0,
    };
    for (let update = 0; update < 2; update += 1) {
      await session.send("Page.captureScreenshot", {
        captureBeyondViewport: true,
        clip,
        format: "png",
        fromSurface: true,
      });
    }
    const capture = await session.send("Page.captureScreenshot", {
      captureBeyondViewport: true,
      clip,
      format: "png",
      fromSurface: true,
    });
    if (errors.length > 0 || JSON.stringify([...new Set(requests)]) !== JSON.stringify([INPUT_URL])) {
      throw new Error(errors.join("\n") || `unexpected requests: ${JSON.stringify(requests)}`);
    }
    const decoded = decodePng(Buffer.from(capture.data, "base64"));
    if (decoded.width !== entry.dimensions.width || decoded.height !== entry.dimensions.height) {
      throw new Error(`capture dimension mismatch for ${entry.semantic_subset_identity}`);
    }
    for (let offset = 0; offset < decoded.rgba.length; offset += 4) {
      if (decoded.rgba[offset + 3] === 0) decoded.rgba.fill(0, offset, offset + 3);
    }
    const platformFonts = await auditInlineSvgFont(browser, svg);
    semantic.platform_glyph_counts = platformFonts.map((font) => font.glyphCount);
    return {
      id: entry.semantic_subset_identity,
      rasterSha256: sha256(decoded.rgba),
      semantic,
      semanticSha256: sha256(Buffer.from(canonicalJson(semantic))),
    };
  } finally {
    await context.close();
  }
}

async function auditInlineSvgFont(browser, svg) {
  const svgText = svg.toString("utf8");
  const style = /<style>[\s\S]*?<\/style>/u.exec(svgText)?.[0];
  if (!style) throw new Error("qualification SVG lacks its embedded font style");
  const context = await browser.newContext(CONTEXT_OPTIONS);
  try {
    const page = await context.newPage();
    const session = await context.newCDPSession(page);
    await page.route("**/*", async (route) => {
      if (route.request().url() !== INPUT_URL) {
        await route.abort("blockedbyclient");
        return;
      }
      await route.fulfill({
        body: `<html><head>${style}</head><body>${svgText}</body></html>`,
        contentType: "text/html; charset=utf-8",
        status: 200,
      });
    });
    await page.goto(INPUT_URL, { waitUntil: "load" });
    await page.evaluate(async () => document.fonts.ready);
    await page.evaluate(() => {
      const text = document.querySelector("svg text");
      const span = document.createElement("span");
      const style = getComputedStyle(text);
      span.textContent = text.textContent;
      span.style.fontFamily = style.fontFamily;
      span.style.fontSize = style.fontSize;
      span.style.fontStretch = style.fontStretch;
      span.style.fontStyle = style.fontStyle;
      span.style.fontWeight = style.fontWeight;
      span.style.whiteSpace = "pre";
      document.body.append(span);
    });
    await session.send("Page.captureScreenshot", { format: "png", fromSurface: true });
    const fonts = await auditPlatformFonts(session);
    if (fonts.length === 0 || fonts.some((font) => !font.isCustomFont)) {
      const debug = await page.evaluate(() => {
        const span = document.querySelector("span");
        return {
          bounds: span.getBoundingClientRect().toJSON(),
          faces: [...document.fonts].map((face) => ({ family: face.family, status: face.status })),
          family: getComputedStyle(span).fontFamily,
          text: span.textContent,
        };
      });
      throw new Error(`ambient or missing platform font: ${JSON.stringify(fonts)}; ${JSON.stringify(debug)}`);
    }
    return fonts;
  } finally {
    await context.close();
  }
}

async function auditPlatformFonts(session) {
  await session.send("DOM.enable");
  await session.send("CSS.enable");
  const { root } = await session.send("DOM.getDocument", { depth: -1, pierce: true });
  const { nodeId } = await session.send("DOM.querySelector", {
    nodeId: root.nodeId,
    selector: "span",
  });
  if (!nodeId) throw new Error("DevTools DOM contained no HTML span");
  const { result } = await session.send("Runtime.evaluate", {
    expression: "document.querySelector('span').firstChild",
  });
  if (!result.objectId) throw new Error("DevTools DOM contained no HTML span text node");
  const textNode = await session.send("DOM.requestNode", { objectId: result.objectId });
  const { fonts } = await session.send("CSS.getPlatformFontsForNode", { nodeId: textNode.nodeId });
  return fonts.map(({ glyphCount, isCustomFont }) => ({ glyphCount, isCustomFont }));
}

function validateCase(entry) {
  if (!/^[0-9a-f]{64}$/u.test(entry.semantic_subset_identity)) {
    throw new Error("invalid semantic subset identity");
  }
  for (const role of ["source", "subset"]) {
    const svg = readFileSync(repositoryInputPath(entry[`${role}_svg`]));
    if (sha256(svg) !== entry[`${role}_svg_sha256`]) throw new Error(`stale ${role} SVG`);
    const match = /src:url\(data:font\/woff2;base64,([A-Za-z0-9+/]*={0,2})\) format\(woff2\)/u.exec(svg.toString("utf8"));
    if (!match) throw new Error(`missing canonical ${role} WOFF2 data URL`);
    const woff2 = Buffer.from(match[1], "base64");
    if (woff2.toString("base64") !== match[1]
        || sha256(woff2) !== entry[`${role}_woff2_sha256`]) {
      throw new Error(`stale ${role} WOFF2 payload`);
    }
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
        bitDepth: data[8], colorType: data[9], compression: data[10], filter: data[11],
        height: data.readUInt32BE(4), interlace: data[12], width: data.readUInt32BE(0),
      };
    } else if (type === "IDAT") compressed.push(data);
    else if (type === "IEND") break;
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

function assertCanonicalEnvironment() {
  if (process.env.REFERENCE_ORACLE_CANONICAL !== "1" || process.platform !== "linux"
      || process.arch !== "x64" || process.version !== "v24.16.0") {
    throw new Error("run the Chromium SVG viewer in the canonical reference-profile container");
  }
}

function repositoryInputPath(argument) {
  const root = realpathSync(REPO_ROOT);
  const path = realpathSync(lexicalRepositoryPath(argument));
  if (path !== root && !path.startsWith(`${root}${sep}`)) throw new Error(`path escaped repository: ${argument}`);
  return path;
}

function repositoryOutputPath(argument) {
  const path = lexicalRepositoryPath(argument);
  const root = realpathSync(REPO_ROOT);
  const parent = realpathSync(dirname(path));
  if (parent !== root && !parent.startsWith(`${root}${sep}`)) throw new Error(`output escaped repository: ${argument}`);
  return resolve(parent, basename(path));
}

function lexicalRepositoryPath(argument) {
  if (!argument || argument.startsWith("/") || argument.split(/[\\/]/u).includes("..")) {
    throw new Error(`unsafe repository-relative path: ${argument}`);
  }
  const path = resolve(REPO_ROOT, argument);
  if (path !== REPO_ROOT && !path.startsWith(`${REPO_ROOT}${sep}`)) throw new Error(`path escaped repository: ${argument}`);
  return path;
}

function canonicalJson(value) {
  if (value === null || typeof value !== "object") return JSON.stringify(value);
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(",")}}`;
}

function pass(value) {
  return value ? "pass" : "fail";
}

function semanticChecks(source, subset, rasterEqual) {
  const equal = (left, right) => canonicalJson(left) === canonicalJson(right);
  const sourceLines = source.lines.map(({ bounds, computed_length: computedLength }) => ({
    bounds,
    computed_length: computedLength,
  }));
  const subsetLines = subset.lines.map(({ bounds, computed_length: computedLength }) => ({
    bounds,
    computed_length: computedLength,
  }));
  const sourceMetrics = source.lines.map((line) => line.metrics);
  const subsetMetrics = subset.lines.map((line) => line.metrics);
  const sourceGeometry = source.characters.map(({ start, end, extent, rotation, substring_length: length }) => ({
    end, extent, length, rotation, start,
  }));
  const subsetGeometry = subset.characters.map(({ start, end, extent, rotation, substring_length: length }) => ({
    end, extent, length, rotation, start,
  }));
  return {
    advances_offsets: pass(equal(sourceGeometry, subsetGeometry)),
    glyph_choice: pass(equal(source.platform_glyph_counts, subset.platform_glyph_counts) && rasterEqual),
    line_layout: pass(equal(sourceLines, subsetLines)),
    metrics: pass(equal(sourceMetrics, subsetMetrics) && equal(source.text_bounds, subset.text_bounds)),
    rasterized_typography: pass(rasterEqual),
    selectable_text: pass(equal(source.selected, subset.selected)
      && equal(source.lines.map((line) => line.scalars), subset.lines.map((line) => line.scalars))),
    shaping: pass(equal(sourceGeometry, subsetGeometry)
      && source.text_length === subset.text_length
      && equal(source.platform_glyph_counts, subset.platform_glyph_counts)),
  };
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}
