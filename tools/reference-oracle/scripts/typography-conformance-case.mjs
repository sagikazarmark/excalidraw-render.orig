import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

import { canonicalize } from "./canonical-json.mjs";

// Standalone generator for the focused typography case.
//
// It imports nothing conformance-case.mjs owns, on purpose. Every scene case's
// provenance pins that generator's bytes, so a change made for font-bearing
// evidence must not move the scene cases' provenance; and this case's provenance
// pins this file, so the case stays reproducible through its declared generator
// alone. The one module both share, canonical-json.mjs, is bound by the
// reference profile whose digest every provenance pins. An earlier version
// templated over conformance-case.mjs with string anchors and went stale the
// first time that file was refactored (#106).

const ORACLE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO_ROOT = resolve(ORACLE_ROOT, "../..");
const PROFILE_ROOT = resolve(REPO_ROOT, "tests/oracle/reference-profiles");
const REGISTRY_PATH = resolve(REPO_ROOT, "tests/conformance/cases/registry.json");
const REGISTRY_SCHEMA = "excalidraw-render/conformance-case-registry/v1";

// This generator carries the judgment inputs of exactly one registered case.
// The registry stays the single place that names the case directory, runner,
// and generator; the case-specific manifest inputs live here, like the frame
// generator's, rather than in the registry the scene generator reads.
const CASE_ID = "focused.excalifont-text.v1";
const STAGING_ROOT = resolve(REPO_ROOT, "target/reference-evidence/excalifont-text");
const SCHEMA_ID = "excalidraw-render/conformance-case/typography-reference/v1";
const SCHEMA_PATH = resolve(REPO_ROOT, "tests/conformance/typography-reference-case-v1.schema.json");
// The date the case was first generated; a regeneration keeps it, as the scene
// cases keep theirs, and xtask pins it.
const GENERATED_AT = "2026-07-22";
const ORIGINATING_ISSUE = "https://github.com/sagikazarmark/excalidraw-render.orig/issues/58";
const PURPOSE = "Isolate wrapped Excalifont text in official selectable SVG and direct-canvas PNG.";
const COVERAGE = [
  { identity: "element.text", role: "primary" },
  { identity: "font.excalifont", role: "primary" },
  { identity: "text.wrapping", role: "primary" },
  { identity: "reference.direct-png", role: "primary" },
  { identity: "reference.svg", role: "primary" },
];
const FEATURE_TAGS = ["element:text", "font:excalifont", "text:wrapping", "format:svg", "format:png"];
const SEMANTIC_EXPECTATIONS = {
  element_count: 1,
  element_type: "text",
  embedded_scene: false,
  images: 0,
  links: 0,
  masks: 0,
  selectable_text: true,
};
const LICENSE = {
  copyright: "Copyright 2026 excalidraw-render contributors",
  expression: "MIT OR Apache-2.0",
};
const SANITIZATION = {
  performed: false,
  reason: "Project-authored synthetic scene document; no external content.",
};
const DETERMINISM = { animation_offset_microseconds: 0, fresh_process_runs: 2 };
const VISUAL_CLASS = "geometry/image";
const OPTIONS = {
  exportBackground: true,
  exportEmbedScene: false,
  exportPadding: 10,
  exportScale: 1,
  exportWithDarkMode: false,
  frameRendering: {
    clip: true,
    enabled: true,
    name: true,
    outline: true,
  },
  maxWidthOrHeight: null,
  viewBackgroundColor: "#ffffff",
};
// Reported key order, not canonical order: the checked report is byte-pinned
// with schema_version first. The case declares no diagnostics.
const EMPTY_REPORT = Buffer.from('{"schema_version":1,"diagnostics":[]}', "utf8");

const ORIGIN = "http://reference.invalid";
const PROFILE_DIGEST = "1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71";
const ASSET_DIGEST = "02685099e6a12632d77a3cac4c6d3acef291520e4fef93a675ee8be471245d9a";
const SURFACE_DIGEST = "93029f2dab63c203dee69da4497ea4c2988a72d73f2a0017e62c529a4caac202";
const CONTAINER_IMAGE = "mcr.microsoft.com/playwright@sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9";
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
const RAW_RASTER_PROFILE = {
  alpha: "straight",
  color_space: "sRGB",
  decoder: "Chromium HTMLImageElement object URL",
  layout: "top-left-row-major RGBA8",
  renderer: "CanvasRenderingContext2D.drawImage plus getImageData",
  scope: "reference-evidence repeatability; not a candidate comparison viewer",
};
const RAW_RASTER_PROFILE_DIGEST = sha256(canonicalBytes(RAW_RASTER_PROFILE));

// Populated by main() from the checked registry before any case data is read.
let CASE = null;

function loadCase(caseId) {
  const registry = JSON.parse(readFileSync(REGISTRY_PATH, "utf8"));
  if (registry.schema !== REGISTRY_SCHEMA) {
    throw new Error(`case registry schema must be ${REGISTRY_SCHEMA}`);
  }
  const entry = registry.cases.find((candidate) => candidate.id === caseId);
  if (!entry) {
    throw new Error(`unknown conformance case ${caseId}`);
  }
  if (entry.kind !== "typography") {
    throw new Error(`case ${caseId} has kind ${entry.kind}; this generator builds the typography case`);
  }
  const generatorPath = fileURLToPath(import.meta.url);
  if (resolve(REPO_ROOT, entry.generator) !== generatorPath) {
    throw new Error(`case ${caseId} registers ${entry.generator} as its generator, not this script`);
  }
  return {
    ...entry,
    generatorPath,
    root: resolve(REPO_ROOT, entry.path),
    runnerPath: resolve(REPO_ROOT, entry.runner),
  };
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function canonicalBytes(value) {
  return Buffer.from(canonicalize(value), "utf8");
}

function casePath(path) {
  return relative(CASE.root, path).split(sep).join("/");
}

function repoPath(path) {
  return relative(REPO_ROOT, path).split(sep).join("/");
}

function safePath(root, relativePath) {
  const path = resolve(root, relativePath);
  if (path !== root && !path.startsWith(`${root}${sep}`)) {
    throw new Error(`request escaped local root: ${relativePath}`);
  }
  return path;
}

function contentType(path) {
  if (path.endsWith(".html")) return "text/html; charset=utf-8";
  if (path.endsWith(".js")) return "text/javascript; charset=utf-8";
  if (path.endsWith(".css")) return "text/css; charset=utf-8";
  if (path.endsWith(".woff2")) return "font/woff2";
  return "application/octet-stream";
}

function fileDescriptor(path, extra = {}, logicalPath = casePath(path)) {
  const bytes = readFileSync(path);
  return {
    byte_length: bytes.length,
    path: logicalPath,
    sha256: sha256(bytes),
    ...extra,
  };
}

async function renderOnce(scene, allowedAssets, options) {
  const browser = await chromium.launch({
    args: BROWSER_ARGUMENTS,
    chromiumSandbox: false,
    headless: true,
  });
  try {
    if (browser.version() !== BROWSER_VERSION) {
      throw new Error(`expected Chromium ${BROWSER_VERSION}, received ${browser.version()}`);
    }
    const session = await browser.newBrowserCDPSession();
    const processInfo = await session.send("SystemInfo.getProcessInfo");
    const browserProcess = processInfo.processInfo.find((process) => process.type === "browser");
    if (!browserProcess) {
      throw new Error("Chromium did not report its browser process ID");
    }

    const context = await browser.newContext(CONTEXT_OPTIONS);
    const page = await context.newPage();
    const errors = [];
    const requests = [];
    page.on("console", (message) => {
      if (message.type() === "error" || message.text().includes("falling back")) {
        errors.push(`console ${message.type()}: ${message.text()}`);
      }
    });
    page.on("pageerror", (error) => errors.push(`page error: ${error.message}`));
    page.on("requestfailed", (request) => {
      errors.push(`request failed: ${request.url()} (${request.failure()?.errorText ?? "unknown"})`);
    });
    await page.route("**/*", async (route) => {
      const url = new URL(route.request().url());
      requests.push(url.href);
      if (url.origin !== ORIGIN) {
        errors.push(`external request: ${url.href}`);
        await route.abort("blockedbyclient");
        return;
      }
      let path;
      if (url.pathname === "/") {
        path = resolve(ORACLE_ROOT, "dist/index.html");
      } else if (url.pathname.startsWith("/bundle/")) {
        path = safePath(resolve(ORACLE_ROOT, "dist"), url.pathname.slice("/bundle/".length));
      } else if (url.pathname.startsWith("/assets/")) {
        const assetPath = url.pathname.slice("/assets".length);
        if (!allowedAssets.has(assetPath)) {
          errors.push(`unmanifested asset request: ${url.href}`);
          await route.abort("blockedbyclient");
          return;
        }
        path = safePath(
          resolve(ORACLE_ROOT, "node_modules/@excalidraw/excalidraw/dist/prod"),
          assetPath.slice(1),
        );
      } else {
        errors.push(`unknown local request: ${url.href}`);
        await route.abort("blockedbyclient");
        return;
      }
      await route.fulfill({ body: readFileSync(path), contentType: contentType(path), status: 200 });
    });

    await page.goto(`${ORIGIN}/`, { waitUntil: "load" });
    await page.waitForFunction(() => typeof window.referenceOracle?.render === "function");
    const output = await page.evaluate(async ({ input, options }) => {
      const rendered = await window.referenceOracle.render(input, options);
      const bytesFromBase64 = (encoded) => {
        const binary = atob(encoded);
        return Uint8Array.from(binary, (character) => character.charCodeAt(0));
      };
      const base64FromBytes = (bytes) => {
        let binary = "";
        for (let offset = 0; offset < bytes.length; offset += 0x8000) {
          binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
        }
        return btoa(binary);
      };
      const rasterize = async (blob) => {
        const objectUrl = URL.createObjectURL(blob);
        try {
          const image = new Image();
          await new Promise((resolve, reject) => {
            image.addEventListener("load", resolve, { once: true });
            image.addEventListener("error", () => reject(new Error("evidence image could not be decoded")), { once: true });
            image.src = objectUrl;
          });
          const canvas = document.createElement("canvas");
          canvas.width = image.naturalWidth;
          canvas.height = image.naturalHeight;
          const context = canvas.getContext("2d", { alpha: true, willReadFrequently: true });
          context.clearRect(0, 0, canvas.width, canvas.height);
          context.drawImage(image, 0, 0);
          const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
          return {
            height: canvas.height,
            rgbaBase64: base64FromBytes(pixels),
            width: canvas.width,
          };
        } finally {
          URL.revokeObjectURL(objectUrl);
        }
      };
      const pngBytes = bytesFromBase64(rendered.pngBase64);
      return {
        pngBase64: rendered.pngBase64,
        pngRaster: await rasterize(new Blob([pngBytes], { type: "image/png" })),
        svg: rendered.svg,
        svgRaster: await rasterize(new Blob([rendered.svg], { type: "image/svg+xml" })),
      };
    }, { input: scene, options });
    if (errors.length > 0) {
      throw new Error(errors.join("\n"));
    }
    const png = Buffer.from(output.pngBase64, "base64");
    const svg = Buffer.from(output.svg, "utf8");
    const pngRgba = Buffer.from(output.pngRaster.rgbaBase64, "base64");
    const svgRgba = Buffer.from(output.svgRaster.rgbaBase64, "base64");
    return {
      browserProcessId: browserProcess.id,
      outputs: {
        png: {
          artifact: png,
          artifactSha256: sha256(png),
          height: output.pngRaster.height,
          rawRaster: pngRgba,
          rawRasterSha256: sha256(pngRgba),
          width: output.pngRaster.width,
        },
        svg: {
          artifact: svg,
          artifactSha256: sha256(svg),
          height: output.svgRaster.height,
          rawRaster: svgRgba,
          rawRasterSha256: sha256(svgRgba),
          width: output.svgRaster.width,
        },
      },
      requests: [...new Set(requests)].sort(),
    };
  } finally {
    await browser.close();
  }
}

function assertRepeatable(first, second) {
  if (first.browserProcessId === second.browserProcessId) {
    throw new Error("fresh Chromium launches reused a browser process ID");
  }
  if (canonicalize(first.requests) !== canonicalize(second.requests)) {
    throw new Error("fresh Chromium processes observed different request sets");
  }
  for (const format of ["png", "svg"]) {
    for (const field of ["artifactSha256", "rawRasterSha256", "width", "height"]) {
      if (first.outputs[format][field] !== second.outputs[format][field]) {
        throw new Error(`${format} ${field} differs in fresh Chromium processes`);
      }
    }
  }
}

// The official rendering fonts are the manifested WOFF2 assets the reference
// renderer fetched while producing the evidence, so the record is derived from
// the observed requests rather than declared by hand. The typography case must
// bind at least one: text evidence rendered without any official font would be
// evidence of a fallback face.
function officialRenderingFonts(run, assetsByPath) {
  const fonts = run.requests
    .map((href) => new URL(href))
    .filter((url) => url.pathname.startsWith("/assets/"))
    .map((url) => {
      const asset = assetsByPath.get(url.pathname.slice("/assets".length));
      if (!asset) {
        throw new Error(`evidence run fetched an unmanifested asset: ${url.href}`);
      }
      if (!asset.path.endsWith(".woff2")) {
        throw new Error(`evidence run fetched a manifested asset that is not a font: ${asset.path}`);
      }
      return {
        byte_length: asset.bytes,
        path: asset.path,
        role: "official-rendering-font",
        sha256: asset.sha256,
      };
    })
    .sort((left, right) => (left.path < right.path ? -1 : left.path > right.path ? 1 : 0));
  if (fonts.length === 0) {
    throw new Error("evidence run fetched no official rendering font");
  }
  return fonts;
}

function provenance(first, second) {
  const runRecord = (run, ordinal) => ({
    browser_process_id: run.browserProcessId,
    fresh_browser_process: true,
    ordinal,
    outputs: Object.fromEntries(["png", "svg"].map((format) => [format, {
      artifact_sha256: run.outputs[format].artifactSha256,
      height: run.outputs[format].height,
      raw_raster_sha256: run.outputs[format].rawRasterSha256,
      width: run.outputs[format].width,
    }])),
    requests: run.requests,
  });
  return {
    browser: {
      arguments: BROWSER_ARGUMENTS,
      context: CONTEXT_OPTIONS,
      version: BROWSER_VERSION,
    },
    case_id: CASE.id,
    container_image: CONTAINER_IMAGE,
    generated_at: GENERATED_AT,
    generator: fileDescriptor(CASE.generatorPath, {}, repoPath(CASE.generatorPath)),
    generation_invocation: `cargo run --locked -p xtask -- conformance generate ${CASE.id}`,
    node_version: process.version,
    png_independent_from_svg: true,
    raw_raster_profile: {
      definition: RAW_RASTER_PROFILE,
      digest: `sha256:${RAW_RASTER_PROFILE_DIGEST}`,
      id: "reference-evidence.browser-raster.v1",
    },
    reference_profile_digest: `sha256:${PROFILE_DIGEST}`,
    runner: fileDescriptor(CASE.runnerPath, {}, repoPath(CASE.runnerPath)),
    runs: [runRecord(first, 1), runRecord(second, 2)],
    schema: "excalidraw-render/reference-evidence-provenance/v1",
    reproduction_invocation: `cargo run --locked -p xtask -- conformance reproduce ${CASE.id}`,
  };
}

function buildManifest(scenePath, provenancePath, outputRoot, first, assetsByPath) {
  const descriptor = (name, extra) => fileDescriptor(resolve(outputRoot, name), extra, name);
  const forFormat = (format, mediaType) => ({
    artifact: descriptor(`evidence/official.${format}`, {
      height: first.outputs[format].height,
      media_type: mediaType,
      width: first.outputs[format].width,
    }),
    raw_raster: descriptor(`evidence/official-${format}.rgba`, {
      height: first.outputs[format].height,
      layout: "top-left-row-major",
      pixel_format: "straight-alpha-srgb-rgba8",
      width: first.outputs[format].width,
    }),
    visual_class: VISUAL_CLASS,
  });
  return {
    approval: null,
    assets: officialRenderingFonts(first, assetsByPath),
    case_id: CASE.id,
    comparison: {
      comparator_profiles: [],
      viewer_profiles: [{
        digest: `sha256:${RAW_RASTER_PROFILE_DIGEST}`,
        id: "reference-evidence.browser-raster.v1",
        scope: "reference evidence repeatability only",
      }],
    },
    compatibility_revision: 1,
    coverage: COVERAGE,
    crops: [],
    determinism: DETERMINISM,
    exceptions: [],
    // One report covers both formats: the typography manifest predates the
    // per-format reports scene cases carry, and its schema and xtask pin it.
    expected_report: descriptor("expected-report.json", {}),
    feature_tags: FEATURE_TAGS,
    layer: "focused",
    license: LICENSE,
    originating_issue: ORIGINATING_ISSUE,
    provenance: fileDescriptor(provenancePath, {}, "evidence/provenance.json"),
    purpose: PURPOSE,
    reference: {
      asset_manifest: {
        digest: `sha256:${ASSET_DIGEST}`,
        path: "tests/oracle/reference-profiles/revision-1-assets.json",
      },
      profile: {
        digest: `sha256:${PROFILE_DIGEST}`,
        path: "tests/oracle/reference-profiles/revision-1.json",
      },
      scene_surface: {
        digest: `sha256:${SURFACE_DIGEST}`,
        path: "tests/oracle/scene-surfaces/revision-1.json",
      },
    },
    sanitization: SANITIZATION,
    scene: fileDescriptor(scenePath),
    schema: {
      digest: `sha256:${sha256(readFileSync(SCHEMA_PATH))}`,
      id: SCHEMA_ID,
      path: repoPath(SCHEMA_PATH),
    },
    semantic_expectations: SEMANTIC_EXPECTATIONS,
    variants: [{
      formats: { png: forFormat("png", "image/png"), svg: forFormat("svg", "image/svg+xml") },
      id: "default",
      options: OPTIONS,
    }],
  };
}

const EVIDENCE_FILES = [
  "expected-report.json",
  "evidence/official.png",
  "evidence/official.svg",
  "evidence/official-png.rgba",
  "evidence/official-svg.rgba",
];

function writeEvidence(outputRoot, first, second) {
  mkdirSync(resolve(outputRoot, "evidence"), { recursive: true });
  writeFileSync(resolve(outputRoot, "expected-report.json"), EMPTY_REPORT);
  writeFileSync(resolve(outputRoot, "evidence/official.png"), first.outputs.png.artifact);
  writeFileSync(resolve(outputRoot, "evidence/official.svg"), first.outputs.svg.artifact);
  writeFileSync(resolve(outputRoot, "evidence/official-png.rgba"), first.outputs.png.rawRaster);
  writeFileSync(resolve(outputRoot, "evidence/official-svg.rgba"), first.outputs.svg.rawRaster);
  const provenancePath = resolve(outputRoot, "evidence/provenance.json");
  writeFileSync(provenancePath, Buffer.concat([canonicalBytes(provenance(first, second)), Buffer.from("\n")]));
  return provenancePath;
}

function compareCheckedEvidence(stagedRoot, manifest) {
  for (const relativePath of EVIDENCE_FILES) {
    const staged = readFileSync(resolve(stagedRoot, relativePath));
    const checked = readFileSync(resolve(CASE.root, relativePath));
    if (!staged.equals(checked)) {
      throw new Error(`${relativePath} differs from approved reference evidence`);
    }
  }
  const checked = JSON.parse(readFileSync(resolve(CASE.root, "manifest.json"), "utf8"));
  const stagedProvenance = JSON.parse(readFileSync(resolve(stagedRoot, "evidence/provenance.json"), "utf8"));
  const checkedProvenance = JSON.parse(readFileSync(resolve(CASE.root, "evidence/provenance.json"), "utf8"));
  const normalizeProvenance = (value) => {
    const normalized = structuredClone(value);
    for (const run of normalized.runs) delete run.browser_process_id;
    // Producer code is historical provenance (ADR-0019, ADR-0024): compare
    // where it lives, not the bytes it had when the evidence was captured.
    for (const producer of [normalized.generator, normalized.runner]) {
      if (producer) { delete producer.sha256; delete producer.byte_length; }
    }
    return normalized;
  };
  if (canonicalize(normalizeProvenance(stagedProvenance))
      !== canonicalize(normalizeProvenance(checkedProvenance))) {
    throw new Error("reproduced provenance differs from approved provenance beyond fresh process IDs");
  }
  const withoutApproval = structuredClone(checked);
  withoutApproval.approval = null;
  manifest.provenance = withoutApproval.provenance;
  if (canonicalize(withoutApproval) !== canonicalize(manifest)) {
    throw new Error("reproduced manifest fields differ from approved manifest");
  }
}

async function main() {
  const [mode, caseId] = process.argv.slice(2);
  if ((mode !== "generate" && mode !== "reproduce") || caseId !== CASE_ID) {
    throw new Error(`usage: node scripts/typography-conformance-case.mjs <generate|reproduce> ${CASE_ID}`);
  }
  CASE = loadCase(caseId);
  if (process.env.REFERENCE_ORACLE_CANONICAL !== "1" || process.version !== "v24.16.0") {
    throw new Error("run this command in the canonical reference-profile container");
  }
  execFileSync(process.execPath, [resolve(ORACLE_ROOT, "scripts/profile.mjs"), "verify"], {
    cwd: ORACLE_ROOT,
    stdio: "inherit",
  });
  if (readFileSync(resolve(PROFILE_ROOT, "revision-1.sha256"), "ascii").trim() !== PROFILE_DIGEST
      || readFileSync(resolve(PROFILE_ROOT, "revision-1-assets.sha256"), "ascii").trim() !== ASSET_DIGEST) {
    throw new Error("reference profile identities do not match compatibility revision 1");
  }
  if (readFileSync(resolve(REPO_ROOT, "tests/oracle/scene-surfaces/revision-1.sha256"), "ascii").trim() !== SURFACE_DIGEST) {
    throw new Error("scene-surface identity does not match compatibility revision 1");
  }
  const assetManifest = JSON.parse(readFileSync(resolve(PROFILE_ROOT, "revision-1-assets.json"), "utf8"));
  const assetsByPath = new Map(assetManifest.officialPackage.assets.map((asset) => [`/${asset.path}`, asset]));
  const allowedAssets = new Set(assetsByPath.keys());
  const scenePath = resolve(CASE.root, "scene.excalidraw");
  const scene = JSON.parse(readFileSync(scenePath, "utf8"));
  const first = await renderOnce(scene, allowedAssets, OPTIONS);
  const second = await renderOnce(scene, allowedAssets, OPTIONS);
  assertRepeatable(first, second);

  const outputRoot = mode === "generate" ? CASE.root : STAGING_ROOT;
  if (mode === "reproduce") {
    rmSync(STAGING_ROOT, { force: true, recursive: true });
  }
  const provenancePath = writeEvidence(outputRoot, first, second);
  const manifest = buildManifest(scenePath, provenancePath, outputRoot, first, assetsByPath);
  if (mode === "generate") {
    const manifestBytes = canonicalBytes(manifest);
    writeFileSync(resolve(CASE.root, "manifest.json"), Buffer.concat([manifestBytes, Buffer.from("\n")]));
    writeFileSync(resolve(CASE.root, "manifest.sha256"), `${sha256(manifestBytes)}\n`);
  } else {
    compareCheckedEvidence(STAGING_ROOT, manifest);
  }
  console.log(JSON.stringify({
    case_id: CASE.id,
    mode,
    png: {
      artifact_sha256: first.outputs.png.artifactSha256,
      raw_raster_sha256: first.outputs.png.rawRasterSha256,
    },
    result: "pass",
    svg: {
      artifact_sha256: first.outputs.svg.artifactSha256,
      raw_raster_sha256: first.outputs.svg.rawRasterSha256,
    },
  }, null, 2));
}

await main();
