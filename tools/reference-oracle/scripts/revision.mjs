// The compatibility revision's reference identities, as the oracle's scripts
// read them: each digest comes from the identity's `.sha256` sidecar, after the
// file is checked to still have it, so a profile update edits the sidecar and
// no script repeats the digest (xtask's `revision::Reference` is the same).
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const REVISION = 1;

const REPO_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");

const PATHS = {
  profile: `tests/oracle/reference-profiles/revision-${REVISION}.json`,
  frameProfile: `tests/oracle/reference-profiles/revision-${REVISION}-frame.json`,
  assetManifest: `tests/oracle/reference-profiles/revision-${REVISION}-assets.json`,
  sceneSurface: `tests/oracle/scene-surfaces/revision-${REVISION}.json`,
};

/** The repository-relative path of a reference identity's JSON. */
export function referencePath(identity) {
  const path = PATHS[identity];
  if (!path) throw new Error(`unknown reference identity ${identity}`);
  return path;
}

/** A reference identity's digest, lowercase hex, from its verified sidecar. */
export function referenceDigest(identity) {
  const path = referencePath(identity);
  const sidecar = path.replace(/\.json$/u, ".sha256");
  const digest = readFileSync(resolve(REPO_ROOT, sidecar), "ascii").trim();
  if (!/^[0-9a-f]{64}$/u.test(digest)) throw new Error(`${sidecar} holds no SHA-256 digest`);
  const actual = createHash("sha256").update(readFileSync(resolve(REPO_ROOT, path))).digest("hex");
  if (actual !== digest) throw new Error(`${path} does not match ${sidecar}`);
  return digest;
}
