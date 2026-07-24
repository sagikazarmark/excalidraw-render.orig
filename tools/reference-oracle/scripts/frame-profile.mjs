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
const BASE_PROFILE_PATH = resolve(PROFILE_ROOT, "revision-1.json");
const BASE_DIGEST_PATH = resolve(PROFILE_ROOT, "revision-1.sha256");
const PROFILE_PATH = resolve(PROFILE_ROOT, "revision-1-frame.json");
const DIGEST_PATH = resolve(PROFILE_ROOT, "revision-1-frame.sha256");
const BUNDLE_ROOT = resolve(ORACLE_ROOT, "frame-dist");

const REFERENCE_PATHS = [
  "tools/reference-oracle/frame-index.html",
  "tools/reference-oracle/package-lock.json",
  "tools/reference-oracle/run-frame-conformance.sh",
  "tools/reference-oracle/run-frame-profile.sh",
  "tools/reference-oracle/scripts/canonical-json.mjs",
  "tools/reference-oracle/scripts/frame-conformance-case.mjs",
  "tools/reference-oracle/scripts/frame-profile.mjs",
  "tools/reference-oracle/src/frame-browser-entry.js",
  "tools/reference-oracle/vite.frame.config.mjs",
];

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function canonicalBytes(value) {
  return Buffer.from(canonicalize(value), "utf8");
}

function displayPath(path) {
  return relative(REPO_ROOT, path).split(sep).join("/");
}

function fileRecord(path, shown = displayPath(path)) {
  const bytes = readFileSync(path);
  return { bytes: bytes.length, path: shown, sha256: sha256(bytes) };
}

function listFiles(root) {
  const files = [];
  function visit(path) {
    const entries = readdirSync(path, { withFileTypes: true }).sort((left, right) =>
      left.name < right.name ? -1 : left.name > right.name ? 1 : 0,
    );
    for (const entry of entries) {
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

function compareOrWrite(mode, path, expected) {
  if (mode === "update") {
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, expected);
    return;
  }
  const actual = readFileSync(path);
  if (!actual.equals(expected)) {
    throw new Error(`${displayPath(path)} is stale; run the frame profile update command`);
  }
}

function assertCanonicalEnvironment() {
  if (process.env.REFERENCE_ORACLE_CANONICAL !== "1") {
    throw new Error("run this command through tools/reference-oracle/run-frame-profile.sh");
  }
  if (process.platform !== "linux" || process.arch !== "x64") {
    throw new Error(`expected linux/x64, received ${process.platform}/${process.arch}`);
  }
  if (process.version !== "v24.16.0") {
    throw new Error(`expected Node v24.16.0, received ${process.version}`);
  }
}

function main() {
  const mode = process.argv[2];
  if (mode !== "update" && mode !== "verify") {
    throw new Error("usage: node scripts/frame-profile.mjs <update|verify>");
  }
  assertCanonicalEnvironment();
  if (!statSync(BUNDLE_ROOT).isDirectory()) {
    throw new Error("build frame-dist before materializing the frame profile");
  }

  const baseDigest = readFileSync(BASE_DIGEST_PATH, "ascii").trim();
  const baseBytes = readFileSync(BASE_PROFILE_PATH);
  if (sha256(baseBytes) !== baseDigest) {
    throw new Error("the immutable scene reference profile does not match its sidecar");
  }
  const bundleFiles = listFiles(BUNDLE_ROOT).map((path) =>
    fileRecord(path, relative(BUNDLE_ROOT, path).split(sep).join("/")),
  );
  const referenceFiles = REFERENCE_PATHS.map((path) =>
    fileRecord(resolve(REPO_ROOT, path), path),
  );
  const profile = {
    schema: "excalidraw-render/frame-reference-profile/v1",
    compatibilityRevision: 1,
    excalidrawRelease: "v0.18.1",
    exportTarget: "frame",
    baseSceneReferenceProfile: {
      digest: `sha256:${baseDigest}`,
      path: "tests/oracle/reference-profiles/revision-1.json",
    },
    browserBundle: {
      files: bundleFiles,
      recordsSha256: sha256(canonicalBytes(bundleFiles)),
      root: "tools/reference-oracle/frame-dist",
    },
    referenceFiles: {
      files: referenceFiles,
      recordsSha256: sha256(canonicalBytes(referenceFiles)),
    },
    referenceInvocation: {
      api: { png: "exportToBlob", svg: "exportToSvg" },
      exportingFrame: "live source-order frame occurrence",
      frameRendering: { clip: true, enabled: true, name: false, outline: false },
      padding: 0,
      sizing: "exactly one of exportScale or maxWidthOrHeight is non-null",
    },
    reproducibility: {
      updateCommand: "bash tools/reference-oracle/run-frame-profile.sh update",
      verifyCommand: "bash tools/reference-oracle/run-frame-profile.sh verify",
    },
  };
  const bytes = canonicalBytes(profile);
  const digest = sha256(bytes);
  compareOrWrite(mode, PROFILE_PATH, bytes);
  compareOrWrite(mode, DIGEST_PATH, Buffer.from(`${digest}\n`, "ascii"));
  console.log(JSON.stringify({ bundleFiles: bundleFiles.length, profileDigest: digest }, null, 2));
}

main();
