import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import { canonicalize } from "./canonical-json.mjs";

const ORACLE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO_ROOT = resolve(ORACLE_ROOT, "../..");
const PROFILE_ROOT = resolve(REPO_ROOT, "tests/oracle/reference-profiles");
const ASSET_MANIFEST_PATH = resolve(PROFILE_ROOT, "revision-1-assets.json");
const ASSET_DIGEST_PATH = resolve(PROFILE_ROOT, "revision-1-assets.sha256");
const PROFILE_PATH = resolve(PROFILE_ROOT, "revision-1.json");
const PROFILE_DIGEST_PATH = resolve(PROFILE_ROOT, "revision-1.sha256");

const WRAPPER_PATHS = [
  "tools/reference-oracle/fixtures/profile-smoke.excalidraw",
  "tools/reference-oracle/index.html",
  "tools/reference-oracle/package.json",
  "tools/reference-oracle/run-container.sh",
  "tools/reference-oracle/scripts/canonical-json.mjs",
  "tools/reference-oracle/scripts/profile.mjs",
  "tools/reference-oracle/scripts/smoke.mjs",
  "tools/reference-oracle/src/browser-entry.js",
  "tools/reference-oracle/vite.config.mjs",
];

const EXPECTED_VERSIONS = {
  chromium: "149.0.7827.55",
  node: "v24.16.0",
  npm: "11.13.0",
  fontconfig: "2.15.0",
};

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function canonicalBytes(value) {
  return Buffer.from(canonicalize(value), "utf8");
}

function pathForManifest(path) {
  return relative(REPO_ROOT, path).split(sep).join("/");
}

function fileRecord(path, displayPath = pathForManifest(path)) {
  const bytes = readFileSync(path);
  return {
    bytes: bytes.length,
    path: displayPath,
    sha256: sha256(bytes),
  };
}

function listFiles(root) {
  const files = [];
  function visit(path) {
    for (const entry of readdirSync(path, { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name))) {
      const child = resolve(path, entry.name);
      if (entry.isDirectory()) {
        visit(child);
      } else if (entry.isFile()) {
        files.push(child);
      }
    }
  }
  visit(root);
  return files;
}

function digestRecords(records) {
  return sha256(canonicalBytes(records));
}

function commandOutput(command, args) {
  const result = spawnSync(command, args, { encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`${command} failed: ${result.stderr || result.stdout}`);
  }
  return `${result.stdout}${result.stderr}`.trim();
}

function assertCanonicalEnvironment() {
  if (process.env.REFERENCE_ORACLE_CANONICAL !== "1") {
    throw new Error("run this command through tools/reference-oracle/run-container.sh");
  }
  if (process.platform !== "linux" || process.arch !== "x64") {
    throw new Error(`expected linux/x64, received ${process.platform}/${process.arch}`);
  }
  if (process.version !== EXPECTED_VERSIONS.node) {
    throw new Error(`expected Node ${EXPECTED_VERSIONS.node}, received ${process.version}`);
  }
  const npmVersion = execFileSync("npm", ["--version"], { encoding: "utf8" }).trim();
  if (npmVersion !== EXPECTED_VERSIONS.npm) {
    throw new Error(`expected npm ${EXPECTED_VERSIONS.npm}, received ${npmVersion}`);
  }
  const fontconfigVersion = commandOutput("fc-list", ["--version"])
    .replace(/^fontconfig version /, "");
  if (fontconfigVersion !== EXPECTED_VERSIONS.fontconfig) {
    throw new Error(`expected fontconfig ${EXPECTED_VERSIONS.fontconfig}, received ${fontconfigVersion}`);
  }
}

function readLock() {
  return JSON.parse(readFileSync(resolve(ORACLE_ROOT, "package-lock.json"), "utf8"));
}

function lockedPackage(lock, name) {
  const entry = lock.packages[`node_modules/${name}`];
  if (!entry?.version || !entry?.integrity) {
    throw new Error(`package lock has no complete entry for ${name}`);
  }
  return { integrity: entry.integrity, version: entry.version };
}

function makeAssetManifest(lock) {
  const packageRoot = resolve(ORACLE_ROOT, "node_modules/@excalidraw/excalidraw/dist/prod/fonts");
  const packageAssets = listFiles(packageRoot)
    .filter((path) => path.endsWith(".woff2") && !path.includes(`${sep}Assistant${sep}`))
    .map((path) => fileRecord(path, relative(resolve(packageRoot, ".."), path).split(sep).join("/")));

  const fontPaths = execFileSync("fc-list", ["--format=%{file}\\n"], { encoding: "utf8" })
    .trim()
    .split("\n")
    .filter(Boolean);
  const hostFonts = [...new Set(fontPaths)]
    .sort()
    .map((path) => fileRecord(path, path));

  if (packageAssets.length !== 230) {
    throw new Error(`expected 230 official rendering font files, received ${packageAssets.length}`);
  }

  return {
    schema: "excalidraw-render/reference-assets/v1",
    officialPackage: {
      assets: packageAssets,
      npm: lockedPackage(lock, "@excalidraw/excalidraw"),
      pathRoot: "node_modules/@excalidraw/excalidraw/dist/prod",
      policy: "All v0.18.1 rendering WOFF2 assets are served locally; the four Assistant UI faces are excluded.",
    },
    systemFonts: {
      assets: hostFonts,
      fontconfigVersion: EXPECTED_VERSIONS.fontconfig,
      policy: "The digest-pinned canonical container font set is available only for official local-only fallbacks.",
    },
  };
}

function makeProfile(lock, assetManifest, assetDigest) {
  const wrapperFiles = WRAPPER_PATHS.map((path) => fileRecord(resolve(REPO_ROOT, path), path));
  const bundleFiles = listFiles(resolve(ORACLE_ROOT, "dist"))
    .map((path) => fileRecord(path, relative(resolve(ORACLE_ROOT, "dist"), path).split(sep).join("/")));
  const lockRecord = fileRecord(resolve(ORACLE_ROOT, "package-lock.json"));

  const renderingDependencies = [
    "@braintree/sanitize-url",
    "fractional-indexing",
    "image-blob-reduce",
    "pako",
    "perfect-freehand",
    "pica",
    "points-on-curve",
    "roughjs",
  ];

  return {
    schema: "excalidraw-render/reference-profile/v1",
    compatibilityRevision: 1,
    canonicalSerialization: {
      digest: "SHA-256",
      encoding: "UTF-8",
      profile: "RFC 8785 JSON Canonicalization Scheme",
    },
    referenceRenderer: {
      package: lockedPackage(lock, "@excalidraw/excalidraw"),
      sourceCommit: "a2ec2889babf7d2295469c6d90ebe77fae57df84",
      sourceTag: "v0.18.1",
    },
    dependencyLock: {
      lockfileVersion: lock.lockfileVersion,
      path: lockRecord.path,
      sha256: lockRecord.sha256,
      renderingDependencies: Object.fromEntries(
        renderingDependencies.map((name) => [name, lockedPackage(lock, name)]),
      ),
      toolingDependencies: {
        playwright: lockedPackage(lock, "playwright"),
        react: lockedPackage(lock, "react"),
        "react-dom": lockedPackage(lock, "react-dom"),
        vite: lockedPackage(lock, "vite"),
      },
    },
    canonicalPlatform: {
      architecture: "linux/amd64",
      container: {
        image: "mcr.microsoft.com/playwright",
        perPlatformDigest: "sha256:111dde95859f2c659291cb60e698f9048a8fc30b35b4ddb7c90f9cb5b73062d9",
        tagIndexDigest: "sha256:57b65fdc9ceabe0ef613124c7bbe2babcf9362c4d85e382fe3b03604e84b428a",
        ubuntu: "24.04",
      },
      execution: {
        dockerArguments: ["--init", "--ipc=host", "--network=none", "--platform=linux/amd64"],
        network: "No network namespace during profile generation or evidence rendering; fixed-origin requests are fulfilled from pinned local files.",
        workers: 1,
      },
      runtime: {
        chromium: EXPECTED_VERSIONS.chromium,
        fontconfig: EXPECTED_VERSIONS.fontconfig,
        node: EXPECTED_VERSIONS.node.slice(1),
        npm: EXPECTED_VERSIONS.npm,
        playwright: lockedPackage(lock, "playwright").version,
      },
      systemFonts: {
        count: assetManifest.systemFonts.assets.length,
        recordsSha256: digestRecords(assetManifest.systemFonts.assets),
      },
    },
    browser: {
      context: {
        colorScheme: "light",
        deviceScaleFactor: 1,
        forcedColors: "none",
        locale: "en-US",
        reducedMotion: "reduce",
        serviceWorkers: "block",
        timezoneId: "UTC",
        viewport: { height: 720, width: 1280 },
      },
      launch: {
        chromiumSandbox: false,
        explicitArguments: [
          "--disable-gpu",
          "--font-render-hinting=none",
          "--force-color-profile=srgb",
        ],
        gpuPolicy: "disabled; Chromium software rendering is canonical",
        headless: true,
      },
      origin: "http://reference.invalid/",
      requestPolicy: "Only the fixed origin, built bundle files, and assets listed by the asset manifest may be fulfilled; every other request fails the run.",
    },
    browserBundle: {
      bundle: {
        files: bundleFiles,
        recordsSha256: digestRecords(bundleFiles),
      },
      bundler: {
        minifier: "Oxc through Vite 8.1.5",
        mode: "production",
        sourceMaps: false,
        target: "chrome149",
        vite: lockedPackage(lock, "vite").version,
        workerFormat: "es",
      },
      fontSubsetWorker: {
        fallbackAllowed: false,
        mode: "Excalidraw module worker emitted by the pinned Vite bundle",
      },
      react: {
        developmentBuild: false,
        mountedRoot: false,
        react: lockedPackage(lock, "react").version,
        reactDom: lockedPackage(lock, "react-dom").version,
        strictMode: false,
      },
    },
    assets: {
      manifestPath: pathForManifest(ASSET_MANIFEST_PATH),
      manifestSha256: assetDigest,
      officialPackageAssetCount: assetManifest.officialPackage.assets.length,
      systemFontCount: assetManifest.systemFonts.assets.length,
    },
    exportInvocation: {
      api: {
        png: {
          function: "exportToBlob",
          mimeType: "image/png",
          quality: "omitted",
          sizing: "Explicit scale uses getDimensions; maximum dimension uses maxWidthOrHeight.",
        },
        svg: {
          function: "exportToSvg",
          serialization: "XMLSerializer",
          sizing: "Explicit scale is passed through appState; the official SVG wrapper has no maximum-dimension transform.",
        },
      },
      fixedOptions: {
        exportingFrame: null,
        renderEmbeddables: false,
        reuseImages: true,
        skipInliningFonts: false,
      },
      requiredOptions: [
        "exportBackground",
        "exportEmbedScene",
        "exportPadding",
        "exportScale",
        "exportWithDarkMode",
        "frameRendering.enabled",
        "frameRendering.clip",
        "frameRendering.name",
        "frameRendering.outline",
        "maxWidthOrHeight",
        "viewBackgroundColor",
      ],
      semantics: {
        files: "Scene-document files are passed directly; remote URLs are rejected.",
        png: "Generated independently through the official browser canvas path, never from SVG.",
        restore: "Every element must carry stable identity, ordering, seed, version, nonce, and update fields before the public wrapper restores it.",
        sizing: "Exactly one of exportScale or maxWidthOrHeight must be non-null. A maximum dimension never upscales PNG and is not an official SVG transform.",
        svg: "Official SVG is serialized with XMLSerializer after export completes.",
      },
    },
    oracleWrapper: {
      files: wrapperFiles,
      recordsSha256: digestRecords(wrapperFiles),
    },
    reproducibility: {
      acceptance: "Two complete renders in fresh Chromium processes must have identical SVG and PNG SHA-256 hashes and no console, worker, font, asset, or network errors.",
      installCommand: "tools/reference-oracle/run-container.sh install",
      updateCommand: "tools/reference-oracle/run-container.sh update",
      verifyCommand: "tools/reference-oracle/run-container.sh verify",
    },
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
    throw new Error(`${pathForManifest(path)} is not canonical or is stale; run the update command`);
  }
}

function main() {
  const mode = process.argv[2];
  if (mode !== "update" && mode !== "verify") {
    throw new Error("usage: node scripts/profile.mjs <update|verify>");
  }
  assertCanonicalEnvironment();
  if (!statSync(resolve(ORACLE_ROOT, "dist")).isDirectory()) {
    throw new Error("build the browser bundle before materializing the profile");
  }

  const lock = readLock();
  const assetManifest = makeAssetManifest(lock);
  const assetBytes = canonicalBytes(assetManifest);
  const assetDigest = sha256(assetBytes);
  compareOrWrite(mode, ASSET_MANIFEST_PATH, assetBytes);
  compareOrWrite(mode, ASSET_DIGEST_PATH, Buffer.from(`${assetDigest}\n`, "ascii"));

  const profile = makeProfile(lock, assetManifest, assetDigest);
  const profileBytes = canonicalBytes(profile);
  const profileDigest = sha256(profileBytes);
  compareOrWrite(mode, PROFILE_PATH, profileBytes);
  compareOrWrite(mode, PROFILE_DIGEST_PATH, Buffer.from(`${profileDigest}\n`, "ascii"));

  console.log(JSON.stringify({
    assetDigest,
    profileDigest,
    officialPackageAssets: assetManifest.officialPackage.assets.length,
    systemFonts: assetManifest.systemFonts.assets.length,
  }, null, 2));
}

main();
