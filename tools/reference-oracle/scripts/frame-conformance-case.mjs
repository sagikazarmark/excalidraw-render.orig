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
const CASE_ID = "focused.frame-export.v1";
const CASE_ROOT = resolve(REPO_ROOT, "tests/conformance/cases/focused/frame-export");
const STAGING_ROOT = resolve(REPO_ROOT, "target/reference-evidence/frame-export");
const PROFILE_DIGEST = readFileSync(
  resolve(REPO_ROOT, "tests/oracle/reference-profiles/revision-1-frame.sha256"),
  "ascii",
).trim();
const ORIGIN = "http://reference.invalid";
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
const OPTIONS = {
  exportBackground: true,
  exportEmbedScene: false,
  exportScale: 1,
  exportWithDarkMode: false,
  maxWidthOrHeight: null,
  viewBackgroundColor: "#ffffff",
};

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function canonicalBytes(value) {
  return Buffer.from(canonicalize(value), "utf8");
}

function repoPath(path) {
  return relative(REPO_ROOT, path).split(sep).join("/");
}

function casePath(path) {
  return relative(CASE_ROOT, path).split(sep).join("/");
}

function descriptor(path, extra = {}, shown = casePath(path)) {
  const bytes = readFileSync(path);
  return { byte_length: bytes.length, path: shown, sha256: sha256(bytes), ...extra };
}

function safePath(root, requested) {
  const path = resolve(root, requested);
  if (path !== root && !path.startsWith(`${root}${sep}`)) {
    throw new Error(`request escaped local root: ${requested}`);
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

async function renderOnce(scene, allowedAssets) {
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
      let path;
      if (url.origin !== ORIGIN) {
        errors.push(`external request: ${url.href}`);
        await route.abort("blockedbyclient");
        return;
      }
      if (url.pathname === "/") {
        path = resolve(ORACLE_ROOT, "frame-dist/frame-index.html");
      } else if (url.pathname.startsWith("/frame-bundle/")) {
        path = safePath(resolve(ORACLE_ROOT, "frame-dist"), url.pathname.slice(14));
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
    await page.waitForFunction(() => typeof window.frameReferenceOracle?.render === "function");
    const output = await page.evaluate(async ({ scene, options }) => {
      const rasterize = async (blob) => {
        const url = URL.createObjectURL(blob);
        try {
          const image = new Image();
          await new Promise((resolve, reject) => {
            image.addEventListener("load", resolve, { once: true });
            image.addEventListener("error", reject, { once: true });
            image.src = url;
          });
          const canvas = document.createElement("canvas");
          canvas.width = image.naturalWidth;
          canvas.height = image.naturalHeight;
          const context = canvas.getContext("2d", { alpha: true, willReadFrequently: true });
          context.drawImage(image, 0, 0);
          const bytes = context.getImageData(0, 0, canvas.width, canvas.height).data;
          let binary = "";
          for (let offset = 0; offset < bytes.length; offset += 0x8000) {
            binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
          }
          return { base64: btoa(binary), height: canvas.height, width: canvas.width };
        } finally {
          URL.revokeObjectURL(url);
        }
      };
      const frames = scene.elements.filter(
        (element) =>
          element &&
          element.isDeleted !== true &&
          (element.type === "frame" || element.type === "magicframe"),
      );
      const targets = [];
      for (let ordinal = 1; ordinal <= frames.length; ordinal += 1) {
        const rendered = await window.frameReferenceOracle.render(scene, ordinal, options);
        const png = Uint8Array.from(atob(rendered.pngBase64), (value) => value.charCodeAt(0));
        targets.push({
          ordinal,
          png: rendered.pngBase64,
          pngRaster: await rasterize(new Blob([png], { type: "image/png" })),
          svg: rendered.svg,
          svgRaster: await rasterize(new Blob([rendered.svg], { type: "image/svg+xml" })),
        });
      }
      return targets;
    }, { scene, options: OPTIONS });
    if (errors.length > 0) throw new Error(errors.join("\n"));
    return {
      browserProcessId: browserProcess?.id,
      requests: [...new Set(requests)].sort(),
      targets: output.map((target) => ({
        ordinal: target.ordinal,
        png: {
          artifact: Buffer.from(target.png, "base64"),
          rgba: Buffer.from(target.pngRaster.base64, "base64"),
          width: target.pngRaster.width,
          height: target.pngRaster.height,
        },
        svg: {
          artifact: Buffer.from(target.svg, "utf8"),
          rgba: Buffer.from(target.svgRaster.base64, "base64"),
          width: target.svgRaster.width,
          height: target.svgRaster.height,
        },
      })),
    };
  } finally {
    await browser.close();
  }
}

function outputRecord(output) {
  return {
    artifact_sha256: sha256(output.artifact),
    height: output.height,
    raw_raster_sha256: sha256(output.rgba),
    width: output.width,
  };
}

function assertRepeatable(first, second) {
  if (first.browserProcessId === second.browserProcessId) throw new Error("browser process was reused");
  if (first.targets.length !== second.targets.length) throw new Error("target count is not repeatable");
  for (let index = 0; index < first.targets.length; index += 1) {
    if (first.targets[index].ordinal !== second.targets[index].ordinal) {
      throw new Error("target order is not repeatable");
    }
    for (const format of ["png", "svg"]) {
      if (canonicalize(outputRecord(first.targets[index][format])) !== canonicalize(outputRecord(second.targets[index][format]))) {
        throw new Error(`frame ${index + 1} ${format} differs between fresh browser processes`);
      }
    }
  }
}

function writeCase(root, scenePath, scene, first, second) {
  mkdirSync(resolve(root, "evidence"), { recursive: true });
  writeFileSync(resolve(root, "expected-report.json"), '{"schema_version":1,"diagnostics":[]}');
  for (const target of first.targets) {
    const targetRoot = resolve(root, `evidence/frame-${target.ordinal}`);
    mkdirSync(targetRoot, { recursive: true });
    writeFileSync(resolve(targetRoot, "official.png"), target.png.artifact);
    writeFileSync(resolve(targetRoot, "official.svg"), target.svg.artifact);
    writeFileSync(resolve(targetRoot, "official-png.rgba"), target.png.rgba);
    writeFileSync(resolve(targetRoot, "official-svg.rgba"), target.svg.rgba);
  }
  const generator = fileURLToPath(import.meta.url);
  const runner = resolve(ORACLE_ROOT, "run-frame-conformance.sh");
  const provenance = {
    schema: "excalidraw-render/frame-reference-evidence-provenance/v1",
    case_id: CASE_ID,
    generated_at: "2026-07-24",
    browser: { arguments: BROWSER_ARGUMENTS, context: CONTEXT_OPTIONS, version: BROWSER_VERSION },
    frame_reference_profile_digest: `sha256:${PROFILE_DIGEST}`,
    generator: descriptor(generator, {}, repoPath(generator)),
    runner: descriptor(runner, {}, repoPath(runner)),
    runs: [first, second].map((run, index) => ({
      browser_process_id: run.browserProcessId,
      ordinal: index + 1,
      targets: run.targets.map((target) => ({
        ordinal: target.ordinal,
        outputs: { png: outputRecord(target.png), svg: outputRecord(target.svg) },
      })),
      requests: run.requests,
    })),
  };
  const provenancePath = resolve(root, "evidence/provenance.json");
  writeFileSync(provenancePath, Buffer.concat([canonicalBytes(provenance), Buffer.from("\n")]));
  const formatRecord = (target, format, mediaType) => ({
    artifact: descriptor(
      resolve(root, `evidence/frame-${target.ordinal}/official.${format}`),
      { height: target[format].height, media_type: mediaType, width: target[format].width },
      `evidence/frame-${target.ordinal}/official.${format}`,
    ),
    raw_raster: descriptor(
      resolve(root, `evidence/frame-${target.ordinal}/official-${format}.rgba`),
      {
        height: target[format].height,
        layout: "top-left-row-major",
        pixel_format: "straight-alpha-srgb-rgba8",
        width: target[format].width,
      },
      `evidence/frame-${target.ordinal}/official-${format}.rgba`,
    ),
  });
  const frames = scene.elements.filter(
    (element) =>
      element &&
      element.isDeleted !== true &&
      (element.type === "frame" || element.type === "magicframe"),
  );
  const manifest = {
    approval: null,
    case_id: CASE_ID,
    compatibility_revision: 1,
    expected_report: descriptor(
      resolve(root, "expected-report.json"),
      {},
      "expected-report.json",
    ),
    originating_issue: "https://github.com/sagikazarmark/excalidraw-render.orig/issues/97",
    options: OPTIONS,
    targets: first.targets.map((target, index) => ({
      frame_kind: frames[index].type,
      frame_name: typeof frames[index].name === "string" ? frames[index].name : null,
      frame_ordinal: target.ordinal,
      outputs: {
        png: formatRecord(target, "png", "image/png"),
        svg: formatRecord(target, "svg", "image/svg+xml"),
      },
    })),
    profile: {
      digest: `sha256:${PROFILE_DIGEST}`,
      path: "tests/oracle/reference-profiles/revision-1-frame.json",
    },
    provenance: descriptor(provenancePath, {}, "evidence/provenance.json"),
    scene: descriptor(scenePath),
    schema: "excalidraw-render/frame-conformance-case/v1",
  };
  const bytes = canonicalBytes(manifest);
  writeFileSync(resolve(root, "manifest.json"), Buffer.concat([bytes, Buffer.from("\n")]));
  writeFileSync(resolve(root, "manifest.sha256"), `${sha256(bytes)}\n`);
  return manifest;
}

function normalized(value) {
  const copy = structuredClone(value);
  for (const run of copy.runs ?? []) delete run.browser_process_id;
  return copy;
}

async function main() {
  const [mode, caseId] = process.argv.slice(2);
  if (!['generate', 'reproduce'].includes(mode) || caseId !== CASE_ID) {
    throw new Error(`usage: node scripts/frame-conformance-case.mjs <generate|reproduce> ${CASE_ID}`);
  }
  if (process.env.REFERENCE_ORACLE_CANONICAL !== "1" || process.version !== "v24.16.0") {
    throw new Error("run this command in the canonical frame reference container");
  }
  execFileSync(process.execPath, [resolve(ORACLE_ROOT, "scripts/frame-profile.mjs"), "verify"], {
    cwd: ORACLE_ROOT,
    stdio: "inherit",
  });
  const assets = JSON.parse(readFileSync(resolve(REPO_ROOT, "tests/oracle/reference-profiles/revision-1-assets.json"), "utf8"));
  const allowedAssets = new Set(assets.officialPackage.assets.map((asset) => `/${asset.path}`));
  const scenePath = resolve(CASE_ROOT, "scene.excalidraw");
  const scene = JSON.parse(readFileSync(scenePath, "utf8"));
  const first = await renderOnce(scene, allowedAssets);
  const second = await renderOnce(scene, allowedAssets);
  assertRepeatable(first, second);
  const outputRoot = mode === "generate" ? CASE_ROOT : STAGING_ROOT;
  if (mode === "reproduce") rmSync(STAGING_ROOT, { recursive: true, force: true });
  const manifest = writeCase(outputRoot, scenePath, scene, first, second);
  if (mode === "reproduce") {
    const evidencePaths = ["expected-report.json"];
    for (const target of manifest.targets) {
      for (const filename of ["official.png", "official.svg", "official-png.rgba", "official-svg.rgba"]) {
        evidencePaths.push(`evidence/frame-${target.frame_ordinal}/${filename}`);
      }
    }
    for (const path of evidencePaths) {
      if (!readFileSync(resolve(CASE_ROOT, path)).equals(readFileSync(resolve(STAGING_ROOT, path)))) {
        throw new Error(`${path} differs from checked evidence`);
      }
    }
    const checked = JSON.parse(readFileSync(resolve(CASE_ROOT, "manifest.json"), "utf8"));
    manifest.approval = null;
    checked.approval = null;
    const stagedProvenance = JSON.parse(readFileSync(resolve(STAGING_ROOT, "evidence/provenance.json"), "utf8"));
    const checkedProvenance = JSON.parse(readFileSync(resolve(CASE_ROOT, "evidence/provenance.json"), "utf8"));
    if (canonicalize(normalized(stagedProvenance)) !== canonicalize(normalized(checkedProvenance))) {
      throw new Error("provenance differs beyond fresh process IDs");
    }
    manifest.provenance = checked.provenance;
    if (canonicalize(manifest) !== canonicalize(checked)) throw new Error("manifest differs");
  }
  console.log(JSON.stringify({ case_id: CASE_ID, mode, result: "pass" }, null, 2));
}

await main();
