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

const ORACLE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO_ROOT = resolve(ORACLE_ROOT, "../..");
const PROFILE_ROOT = resolve(REPO_ROOT, "tests/oracle/reference-profiles");
const REGISTRY_PATH = resolve(REPO_ROOT, "tests/conformance/cases/registry.json");
const REGISTRY_SCHEMA = "excalidraw-render/conformance-case-registry/v1";

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
  if (entry.kind !== "scene") {
    throw new Error(`case ${caseId} has kind ${entry.kind}; this generator builds scene cases`);
  }
  return {
    ...entry,
    root: resolve(REPO_ROOT, entry.path),
    staging: resolve(REPO_ROOT, entry.staging),
    schemaPath: resolve(REPO_ROOT, entry.schema.path),
    runnerPath: resolve(REPO_ROOT, entry.runner),
  };
}
const ORIGIN = "http://reference.invalid";
const PROFILE_DIGEST = "1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71";
const ASSET_DIGEST = "02685099e6a12632d77a3cac4c6d3acef291520e4fef93a675ee8be471245d9a";
const SURFACE_DIGEST = "14309697cdb5bf9ebe660dccf27c4c929c0f5edb7174443dd4f962544fcc4bea";
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
// A case that legitimately drives a compatibility or normalization path declares
// its diagnostics; the common case declares none.
function expectedReport(format) {
  // Reported key order, not canonical order: the checked reports are byte-pinned
  // with schema_version first, and a case with no diagnostics must keep emitting
  // exactly the bytes the existing approved cases carry.
  const diagnostics = (CASE.expected_diagnostics ?? {})[format] ?? [];
  return Buffer.from(
    `{"schema_version":1,"diagnostics":${JSON.stringify(diagnostics)}}`,
    "utf8",
  );
}
const RAW_RASTER_PROFILE = {
  alpha: "straight",
  color_space: "sRGB",
  decoder: "Chromium HTMLImageElement object URL",
  layout: "top-left-row-major RGBA8",
  renderer: "CanvasRenderingContext2D.drawImage plus getImageData",
  scope: "reference-evidence repeatability; not a candidate comparison viewer",
};
const RAW_RASTER_PROFILE_DIGEST = sha256(canonicalBytes(RAW_RASTER_PROFILE));

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
  const generatorPath = fileURLToPath(import.meta.url);
  const runnerPath = CASE.runnerPath;
  return {
    browser: {
      arguments: BROWSER_ARGUMENTS,
      context: CONTEXT_OPTIONS,
      version: BROWSER_VERSION,
    },
    case_id: CASE.id,
    container_image: CONTAINER_IMAGE,
    generated_at: CASE.generated_at,
    generator: {
      byte_length: readFileSync(generatorPath).length,
      path: repoPath(generatorPath),
      sha256: sha256(readFileSync(generatorPath)),
    },
    generation_invocation: `cargo run --locked -p xtask -- conformance generate ${CASE.id}`,
    node_version: process.version,
    png_independent_from_svg: true,
    raw_raster_profile: {
      definition: RAW_RASTER_PROFILE,
      digest: `sha256:${RAW_RASTER_PROFILE_DIGEST}`,
      id: "reference-evidence.browser-raster.v1",
    },
    reference_profile_digest: `sha256:${PROFILE_DIGEST}`,
    runner: {
      byte_length: readFileSync(runnerPath).length,
      path: repoPath(runnerPath),
      sha256: sha256(readFileSync(runnerPath)),
    },
    runs: [runRecord(first, 1), runRecord(second, 2)],
    schema: "excalidraw-render/reference-evidence-provenance/v1",
    reproduction_invocation: `cargo run --locked -p xtask -- conformance reproduce ${CASE.id}`,
  };
}

// One record per scene file payload: the payload kind and its decoded bytes,
// which is the semantic asset identity the corpus binds.
// The default variant's artifacts stay at evidence/official.*: the SVG viewer
// qualification, its Chromium viewer profile, and two harness tools pin those
// paths, and a canonical viewer identity must not move for a case layout change.
function variantDir(variant) {
  return variant.id === "default" ? "evidence" : `evidence/${variant.id}`;
}

function sceneAssets(scenePath) {
  const scene = JSON.parse(readFileSync(scenePath, "utf8"));
  const files = scene.files ?? {};
  return Object.keys(files).sort().map((fileId) => {
    const file = files[fileId];
    const separator = ";base64,";
    const index = file.dataURL.indexOf(separator);
    if (index < 0) {
      throw new Error(`scene file ${fileId} is not a base64 data URL`);
    }
    const bytes = Buffer.from(file.dataURL.slice(index + separator.length), "base64");
    return {
      byte_length: bytes.length,
      file_id: fileId,
      media_type: file.mimeType,
      sha256: sha256(bytes),
    };
  });
}

function buildManifest(scenePath, provenancePath, outputRoot, renders) {
  const descriptor = (name, extra) => fileDescriptor(resolve(outputRoot, name), extra, name);
  const variantEntry = (variant) => {
    const first = renders.get(variant.id).first;
    const dir = variantDir(variant);
    const forFormat = (format, mediaType) => ({
      artifact: descriptor(`${dir}/official.${format}`, {
        height: first.outputs[format].height,
        media_type: mediaType,
        width: first.outputs[format].width,
      }),
      expected_report: descriptor(`expected-report-${format}.json`, {}),
      raw_raster: descriptor(`${dir}/official-${format}.rgba`, {
        height: first.outputs[format].height,
        layout: "top-left-row-major",
        pixel_format: "straight-alpha-srgb-rgba8",
        width: first.outputs[format].width,
      }),
      visual_class: variant.visual_class,
    });
    return {
      formats: { png: forFormat("png", "image/png"), svg: forFormat("svg", "image/svg+xml") },
      id: variant.id,
      options: variant.options,
    };
  };
  return {
    approval: null,
    assets: sceneAssets(scenePath),
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
    coverage: CASE.coverage,
    crops: [],
    determinism: CASE.determinism,
    // Tolerance exceptions are a maintainer judgment recorded in the registry,
    // like coverage and semantic expectations; the generator carries them into
    // the manifest so the approval subject binds them.
    exceptions: CASE.exceptions ?? [],
    feature_tags: CASE.feature_tags,
    layer: CASE.layer,
    license: CASE.license,
    originating_issue: CASE.originating_issue,
    provenance: fileDescriptor(provenancePath, {}, "evidence/provenance.json"),
    purpose: CASE.purpose,
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
    sanitization: CASE.sanitization,
    scene: fileDescriptor(scenePath),
    schema: {
      digest: `sha256:${sha256(readFileSync(CASE.schemaPath))}`,
      id: CASE.schema.id,
      path: repoPath(CASE.schemaPath),
    },
    semantic_expectations: CASE.semantic_expectations,
    variants: CASE.variants.map(variantEntry),
  };
}

function writeEvidence(outputRoot, renders) {
  mkdirSync(resolve(outputRoot, "evidence"), { recursive: true });
  for (const format of ["png", "svg"]) {
    writeFileSync(resolve(outputRoot, `expected-report-${format}.json`), expectedReport(format));
  }
  for (const variant of CASE.variants) {
    const { first } = renders.get(variant.id);
    const dir = resolve(outputRoot, variantDir(variant));
    mkdirSync(dir, { recursive: true });
    writeFileSync(resolve(dir, "official.png"), first.outputs.png.artifact);
    writeFileSync(resolve(dir, "official.svg"), first.outputs.svg.artifact);
    writeFileSync(resolve(dir, "official-png.rgba"), first.outputs.png.rawRaster);
    writeFileSync(resolve(dir, "official-svg.rgba"), first.outputs.svg.rawRaster);
  }
  const base = renders.get(CASE.variants[0].id);
  const provenancePath = resolve(outputRoot, "evidence/provenance.json");
  writeFileSync(provenancePath, Buffer.concat([canonicalBytes(provenance(base.first, base.second)), Buffer.from("\n")]));
  return provenancePath;
}

function compareCheckedEvidence(stagedRoot, manifest) {
  const relativePaths = [
    "expected-report-png.json",
    "expected-report-svg.json",
    ...CASE.variants.flatMap((variant) => {
      const dir = variantDir(variant);
      return [
        `${dir}/official.png`,
        `${dir}/official.svg`,
        `${dir}/official-png.rgba`,
        `${dir}/official-svg.rgba`,
      ];
    }),
  ];
  for (const relativePath of relativePaths) {
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
  if ((mode !== "generate" && mode !== "reproduce") || !caseId) {
    throw new Error("usage: node scripts/conformance-case.mjs <generate|reproduce> <case-id>");
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
  const allowedAssets = new Set(assetManifest.officialPackage.assets.map((asset) => `/${asset.path}`));
  const scenePath = resolve(CASE.root, "scene.excalidraw");
  const scene = JSON.parse(readFileSync(scenePath, "utf8"));
  const renders = new Map();
  for (const variant of CASE.variants) {
    const first = await renderOnce(scene, allowedAssets, variant.options);
    const second = await renderOnce(scene, allowedAssets, variant.options);
    assertRepeatable(first, second);
    renders.set(variant.id, { first, second });
  }
  const first = renders.get(CASE.variants[0].id).first;

  const outputRoot = mode === "generate" ? CASE.root : CASE.staging;
  if (mode === "generate" && existsSync(resolve(CASE.root, "manifest.json"))) {
    const existing = JSON.parse(readFileSync(resolve(CASE.root, "manifest.json"), "utf8"));
    if (existing.approval !== null) {
      throw new Error("generation refuses to replace approved reference evidence");
    }
  }
  if (mode === "reproduce") {
    rmSync(CASE.staging, { force: true, recursive: true });
  }
  const provenancePath = writeEvidence(outputRoot, renders);
  const manifest = buildManifest(scenePath, provenancePath, outputRoot, renders);
  if (mode === "generate") {
    const manifestBytes = canonicalBytes(manifest);
    writeFileSync(resolve(CASE.root, "manifest.json"), Buffer.concat([manifestBytes, Buffer.from("\n")]));
    writeFileSync(resolve(CASE.root, "manifest.sha256"), `${sha256(manifestBytes)}\n`);
  } else {
    compareCheckedEvidence(CASE.staging, manifest);
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
