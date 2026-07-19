import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

import { canonicalize } from "./canonical-json.mjs";

const ORACLE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO_ROOT = resolve(ORACLE_ROOT, "../..");
const PROFILE_ROOT = resolve(REPO_ROOT, "tests/oracle/reference-profiles");
const SURFACE_ROOT = resolve(REPO_ROOT, "tests/oracle/scene-surfaces");
const EVIDENCE_PATH = resolve(SURFACE_ROOT, "evidence/revision-1-harmless-fields.json");
const EVIDENCE_DIGEST_PATH = resolve(SURFACE_ROOT, "evidence/revision-1-harmless-fields.sha256");
const ORIGIN = "http://reference.invalid";
const PROFILE_DIGEST = "1e08a342359ac1c7eed1384b8e027a8289257b46bdccbbad3a9980e786136b71";
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
const COMMON_EXPORT_OPTIONS = {
  exportBackground: true,
  exportEmbedScene: false,
  exportPadding: 10,
  exportWithDarkMode: false,
  frameRendering: {
    clip: true,
    enabled: true,
    name: true,
    outline: true,
  },
  viewBackgroundColor: "#ffffff",
};
const EXPORT_MODES = {
  default: {
    ...COMMON_EXPORT_OPTIONS,
    exportScale: 1,
    maxWidthOrHeight: null,
  },
  dark: {
    ...COMMON_EXPORT_OPTIONS,
    exportScale: 1,
    exportWithDarkMode: true,
    maxWidthOrHeight: null,
  },
  embeddedScene: {
    ...COMMON_EXPORT_OPTIONS,
    exportEmbedScene: true,
    exportScale: 1,
    maxWidthOrHeight: null,
  },
  framesDisabled: {
    ...COMMON_EXPORT_OPTIONS,
    exportScale: 1,
    frameRendering: {
      clip: false,
      enabled: false,
      name: false,
      outline: false,
    },
    maxWidthOrHeight: null,
  },
  maximumDimension: {
    ...COMMON_EXPORT_OPTIONS,
    exportScale: null,
    maxWidthOrHeight: 100,
  },
  transparentBackground: {
    ...COMMON_EXPORT_OPTIONS,
    exportBackground: false,
    exportScale: 1,
    maxWidthOrHeight: null,
  },
};

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function canonicalBytes(value) {
  return Buffer.from(canonicalize(value), "utf8");
}

function clone(value) {
  return structuredClone(value);
}

function pathForEvidence(path) {
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

function commonElement(overrides) {
  return {
    angle: 0,
    backgroundColor: "transparent",
    boundElements: null,
    fillStyle: "solid",
    frameId: null,
    groupIds: [],
    height: 100,
    id: "evidence-element",
    index: "a2",
    isDeleted: false,
    link: null,
    locked: false,
    opacity: 100,
    roughness: 1,
    roundness: null,
    seed: 740031,
    strokeColor: "#1e1e1e",
    strokeStyle: "solid",
    strokeWidth: 2,
    type: "rectangle",
    updated: 1,
    version: 1,
    versionNonce: 740032,
    width: 180,
    x: 300,
    y: 20,
    ...overrides,
  };
}

function baseScene() {
  return JSON.parse(
    readFileSync(resolve(ORACLE_ROOT, "fixtures/profile-smoke.excalidraw"), "utf8"),
  );
}

function imageScene() {
  const scene = baseScene();
  scene.elements.push(commonElement({
    crop: null,
    fileId: "evidence-pixel",
    height: 64,
    id: "evidence-image",
    index: "a2",
    scale: [1, 1],
    status: "saved",
    type: "image",
    width: 64,
    x: 300,
    y: 20,
  }));
  scene.files["evidence-pixel"] = {
    created: 1,
    dataURL: "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M/wHwAF/gL+XwWvWQAAAABJRU5ErkJggg==",
    id: "evidence-pixel",
    mimeType: "image/png",
  };
  return scene;
}

function missingImageScene(status) {
  const scene = baseScene();
  scene.elements.push(commonElement({
    crop: null,
    fileId: "missing-image",
    height: 96,
    id: `evidence-missing-image-${status}`,
    index: "a2",
    scale: [1, 1],
    status,
    type: "image",
    width: 160,
    x: 300,
    y: 20,
  }));
  return scene;
}

function iframeScene() {
  const scene = baseScene();
  scene.elements.push(commonElement({
    backgroundColor: "#f4f4f6",
    id: "evidence-iframe",
    index: "a2",
    link: null,
    roundness: { type: 3 },
    type: "iframe",
  }));
  return scene;
}

function embeddableScene(link) {
  const scene = baseScene();
  scene.elements.push(commonElement({
    backgroundColor: "#f4f4f6",
    height: 100,
    id: link ? "evidence-linked-embeddable" : "evidence-empty-embeddable",
    index: "a2",
    link,
    roundness: { type: 3 },
    type: "embeddable",
    width: 240,
  }));
  return scene;
}

function elbowArrowScene() {
  const scene = baseScene();
  scene.elements.push(commonElement({
    backgroundColor: "transparent",
    elbowed: true,
    endArrowhead: "arrow",
    endBinding: null,
    endIsSpecial: null,
    fixedSegments: null,
    height: 80,
    id: "evidence-elbow-arrow",
    index: "a2",
    lastCommittedPoint: null,
    points: [[0, 0], [100, 0], [100, 80], [180, 80]],
    roundness: { type: 2 },
    startArrowhead: null,
    startBinding: null,
    startIsSpecial: null,
    type: "arrow",
  }));
  return scene;
}

const CASES = [
  {
    classifications: ["element.common:/index"],
    id: "fractional-index-does-not-reorder-static-export",
    makeBaseline: baseScene,
    makeCandidate() {
      const scene = baseScene();
      scene.elements[0].index = "a1";
      scene.elements[1].index = "a0";
      return scene;
    },
  },
  {
    classifications: ["element.text:/autoResize"],
    id: "persisted-auto-resize-does-not-refresh-text-dimensions",
    makeBaseline: baseScene,
    makeCandidate() {
      const scene = baseScene();
      scene.elements[1].autoResize = false;
      return scene;
    },
  },
  {
    classifications: [
      "app_state:/gridModeEnabled",
      "app_state:/gridSize",
      "app_state:/gridStep",
    ],
    id: "persisted-grid-state-is-not-painted-by-static-export",
    makeBaseline() {
      const scene = baseScene();
      Object.assign(scene.appState, {
        gridModeEnabled: false,
        gridSize: 20,
        gridStep: 5,
      });
      return scene;
    },
    makeCandidate() {
      const scene = baseScene();
      Object.assign(scene.appState, {
        gridModeEnabled: true,
        gridSize: 80,
        gridStep: 11,
      });
      return scene;
    },
  },
  {
    classifications: [
      "file_record:/created",
      "file_record:/lastRetrieved",
      "file_record:/version",
    ],
    id: "referenced-file-lifecycle-metadata-is-not-rendered",
    makeBaseline: imageScene,
    makeCandidate() {
      const scene = imageScene();
      Object.assign(scene.files["evidence-pixel"], {
        created: 1700000000000,
        lastRetrieved: 1700000001000,
        version: 99,
      });
      return scene;
    },
  },
  {
    classifications: [
      "element.text:/baseline",
      "element.text:/rawText",
    ],
    id: "legacy-text-metadata-is-removed-before-static-export",
    makeBaseline: baseScene,
    makeCandidate() {
      const scene = baseScene();
      scene.elements[1].baseline = 1234;
      scene.elements[1].rawText = "ignored legacy value";
      return scene;
    },
  },
  {
    classifications: ["element.iframe:/customData/generationData"],
    id: "iframe-generation-metadata-does-not-change-offline-placeholder",
    makeBaseline: iframeScene,
    makeCandidate() {
      const scene = iframeScene();
      scene.elements[2].customData = {
        generationData: {
          html: "<script>not rendered</script><p>future editor state</p>",
          status: "done",
        },
      };
      return scene;
    },
  },
  {
    classifications: [
      "element.arrow:/fixedSegments",
      "element.arrow:/startIsSpecial",
      "element.arrow:/endIsSpecial",
    ],
    id: "persisted-elbow-routing-metadata-does-not-replace-points",
    makeBaseline: elbowArrowScene,
    makeCandidate() {
      const scene = elbowArrowScene();
      Object.assign(scene.elements[2], {
        endIsSpecial: false,
        fixedSegments: [{
          end: [100, 80],
          index: 2,
          start: [100, 0],
        }],
        startIsSpecial: true,
      });
      return scene;
    },
  },
];

const SNAPSHOT_CASES = [
  {
    classifications: ["recipe:official.image"],
    id: "official-pending-image-placeholder",
    makeScene: () => missingImageScene("pending"),
  },
  {
    classifications: ["recipe:official.image-error"],
    id: "official-error-image-placeholder",
    makeScene: () => missingImageScene("error"),
  },
  {
    classifications: ["recipe:official.embeddable-offline"],
    id: "official-iframe-offline-placeholder",
    makeScene: iframeScene,
  },
  {
    classifications: ["recipe:official.embeddable-offline"],
    id: "official-empty-embeddable-placeholder",
    makeScene: () => embeddableScene(null),
  },
  {
    classifications: ["recipe:official.embeddable-offline"],
    id: "official-linked-embeddable-placeholder",
    makeScene: () => embeddableScene("https://example.invalid/path"),
  },
];

function pngVisualProjection(bytes) {
  const chunks = [bytes.subarray(0, 8)];
  let offset = 8;
  while (offset < bytes.length) {
    if (offset + 12 > bytes.length) throw new Error("truncated PNG evidence output");
    const length = bytes.readUInt32BE(offset);
    const end = offset + 12 + length;
    if (end > bytes.length) throw new Error("invalid PNG evidence chunk length");
    const type = bytes.toString("ascii", offset + 4, offset + 8);
    const data = bytes.subarray(offset + 8, offset + 8 + length);
    const embeddedScene = type === "tEXt" && data.toString("latin1").startsWith("application/vnd.excalidraw+json\0");
    if (!embeddedScene) chunks.push(bytes.subarray(offset, end));
    offset = end;
  }
  return Buffer.concat(chunks);
}

function svgVisualProjection(bytes) {
  return Buffer.from(
    bytes.toString("utf8").replace(/<metadata>[\s\S]*?<\/metadata>/, "<metadata></metadata>"),
    "utf8",
  );
}

async function render(scene, allowedAssets, options) {
  const browser = await chromium.launch({
    args: BROWSER_ARGUMENTS,
    chromiumSandbox: false,
    headless: true,
  });
  if (browser.version() !== "149.0.7827.55") {
    await browser.close();
    throw new Error(`expected Chromium 149.0.7827.55, received ${browser.version()}`);
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

    await route.fulfill({
      body: readFileSync(path),
      contentType: contentType(path),
      status: 200,
    });
  });

  await page.goto(`${ORIGIN}/`, { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.referenceOracle?.render === "function");
  const output = await page.evaluate(
    async ({ input, options }) => window.referenceOracle.render(input, options),
    { input: scene, options },
  );
  await browser.close();

  if (errors.length > 0) {
    throw new Error(errors.join("\n"));
  }

  const png = Buffer.from(output.pngBase64, "base64");
  const svg = Buffer.from(output.svg, "utf8");
  return {
    png: { bytes: png.length, sha256: sha256(png), visualProjectionSha256: sha256(pngVisualProjection(png)) },
    requests: [...new Set(requests)].sort(),
    svg: { bytes: svg.length, sha256: sha256(svg), visualProjectionSha256: sha256(svgVisualProjection(svg)) },
  };
}

function compareOrWrite(mode, path, expected) {
  if (mode === "update") {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, expected);
    return;
  }
  const actual = readFileSync(path);
  if (!actual.equals(expected)) {
    let detail = "";
    try {
      const difference = firstDifference(JSON.parse(actual), JSON.parse(expected));
      detail = difference ? `; first difference: ${difference}` : "";
    } catch {
      // The digest sidecar is not JSON; the owning artifact reports its detail.
    }
    throw new Error(`${pathForEvidence(path)} is not canonical or is stale${detail}; run evidence update`);
  }
}

function firstDifference(actual, expected, pointer = "$") {
  if (Object.is(actual, expected)) return null;
  if (Array.isArray(actual) || Array.isArray(expected)) {
    if (!Array.isArray(actual) || !Array.isArray(expected)) return `${pointer} type`;
    if (actual.length !== expected.length) return `${pointer} length ${actual.length} != ${expected.length}`;
    for (let index = 0; index < actual.length; index += 1) {
      const difference = firstDifference(actual[index], expected[index], `${pointer}/${index}`);
      if (difference) return difference;
    }
    return null;
  }
  if (actual && expected && typeof actual === "object" && typeof expected === "object") {
    const actualKeys = Object.keys(actual).sort();
    const expectedKeys = Object.keys(expected).sort();
    if (canonicalize(actualKeys) !== canonicalize(expectedKeys)) return `${pointer} keys`;
    for (const key of actualKeys) {
      const difference = firstDifference(actual[key], expected[key], `${pointer}/${key}`);
      if (difference) return difference;
    }
    return null;
  }
  return `${pointer} ${JSON.stringify(actual)} != ${JSON.stringify(expected)}`;
}

async function main() {
  const mode = process.argv[2];
  if (mode !== "update" && mode !== "verify") {
    throw new Error("usage: node scripts/scene-surface-evidence.mjs <update|verify>");
  }
  if (process.env.REFERENCE_ORACLE_CANONICAL !== "1") {
    throw new Error("run in the reference profile's canonical container");
  }
  if (process.version !== "v24.16.0") {
    throw new Error(`expected Node v24.16.0, received ${process.version}`);
  }

  execFileSync(process.execPath, [resolve(ORACLE_ROOT, "scripts/profile.mjs"), "verify"], {
    cwd: ORACLE_ROOT,
    stdio: "inherit",
  });

  const profileBytes = readFileSync(resolve(PROFILE_ROOT, "revision-1.json"));
  const profileDigest = readFileSync(resolve(PROFILE_ROOT, "revision-1.sha256"), "ascii").trim();
  if (sha256(profileBytes) !== profileDigest) {
    throw new Error("reference profile digest does not match its canonical bytes");
  }
  if (profileDigest !== PROFILE_DIGEST) {
    throw new Error(`unexpected reference profile digest ${profileDigest}`);
  }
  const assetManifest = JSON.parse(
    readFileSync(resolve(PROFILE_ROOT, "revision-1-assets.json"), "utf8"),
  );
  const allowedAssets = new Set(
    assetManifest.officialPackage.assets.map((asset) => `/${asset.path}`),
  );

  const cases = [];
  for (const definition of CASES) {
    const baselineScene = definition.makeBaseline();
    const candidateScene = definition.makeCandidate();
    const modes = {};
    for (const [modeName, options] of Object.entries(EXPORT_MODES)) {
      const baseline = await render(baselineScene, allowedAssets, options);
      const candidate = await render(candidateScene, allowedAssets, options);
      const comparison = {
        pngArtifactByteIdentity: modeName === "embeddedScene" ? null : baseline.png.sha256 === candidate.png.sha256,
        pngVisualProjectionIdentity: baseline.png.visualProjectionSha256 === candidate.png.visualProjectionSha256,
        requestSetIdentity: canonicalize(baseline.requests) === canonicalize(candidate.requests),
        svgArtifactByteIdentity: modeName === "embeddedScene" ? null : baseline.svg.sha256 === candidate.svg.sha256,
        svgVisualProjectionIdentity: baseline.svg.visualProjectionSha256 === candidate.svg.visualProjectionSha256,
      };
      if (!comparison.pngVisualProjectionIdentity || !comparison.svgVisualProjectionIdentity || !comparison.requestSetIdentity) {
        throw new Error(`${definition.id}/${modeName} changed reference visual output or request set`);
      }
      const evidenceOutput = (output) => modeName === "embeddedScene"
        ? {
            png: { visualProjectionSha256: output.png.visualProjectionSha256 },
            requests: output.requests,
            svg: { visualProjectionSha256: output.svg.visualProjectionSha256 },
          }
        : output;
      modes[modeName] = {
        baseline: evidenceOutput(baseline),
        candidate: evidenceOutput(candidate),
        comparison,
      };
    }
    cases.push({
      baseline: {
        scene: baselineScene,
        sceneSha256: sha256(canonicalBytes(baselineScene)),
      },
      candidate: {
        scene: candidateScene,
        sceneSha256: sha256(canonicalBytes(candidateScene)),
      },
      classifications: definition.classifications,
      id: definition.id,
      modes,
    });
  }

  const snapshots = [];
  for (const definition of SNAPSHOT_CASES) {
    const scene = definition.makeScene();
    const output = await render(scene, allowedAssets, EXPORT_MODES.default);
    snapshots.push({
      classifications: definition.classifications,
      id: definition.id,
      output,
      scene,
      sceneSha256: sha256(canonicalBytes(scene)),
    });
  }

  const evidence = {
    cases,
    exportModes: EXPORT_MODES,
    referenceProfile: {
      digest: `sha256:${PROFILE_DIGEST}`,
      path: "tests/oracle/reference-profiles/revision-1.json",
    },
    result: "pass",
    schema: "excalidraw-render/scene-surface-evidence/v1",
    snapshots,
  };
  const bytes = canonicalBytes(evidence);
  const digest = sha256(bytes);
  compareOrWrite(mode, EVIDENCE_PATH, bytes);
  compareOrWrite(mode, EVIDENCE_DIGEST_PATH, Buffer.from(`${digest}\n`, "ascii"));
  console.log(JSON.stringify({
    cases: cases.length,
    digest,
    path: pathForEvidence(EVIDENCE_PATH),
    result: "pass",
    snapshots: snapshots.length,
  }, null, 2));
}

await main();
