// Frozen producers are provenance; complete frame/fixture bytes are expectations.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fixtures, frameRequests, scene, sha, OCI } from "./webp-sample-fixtures.mjs";
import { encodeFrames } from "./webp-encode-inputs.mjs";

export const INPUT_SHA = "21ce05bb34249f370de8dab10026972e58dd59d40a6dfbff4d7d7df1e3b857c4";
export const REFERENCE_SHA = "b325e8a2d8ad0a1fbd72463a9e2272ef43a1151c2ca040aa5fc46150335f49b6";

export function frozenEvidence() {
  const load = (name, digest) => {
    const bytes = readFileSync(new URL(`../../../tests/oracle/webp-sample/${name}.json`, import.meta.url));
    assert.equal(sha(bytes), digest, `immutable ${name} evidence changed`);
    return JSON.parse(bytes);
  };
  const inputs = load("inputs", INPUT_SHA), reference = load("reference", REFERENCE_SHA);
  assert.equal(inputs.oci, OCI); assert.equal(reference.collection.oci, OCI);
  assert.equal(reference.inputs_sha256, INPUT_SHA);
  assert.deepEqual(reference.encoder, inputs.encoder);
  for (const source of inputs.author_sources) assert.ok(reference.sources.some(s => s.path === source.path && s.sha256 === source.sha256));
  return { inputs, reference };
}

export function assertFrameParity(actual, inputs, reference) {
  // Metadata/options are exact too. Only historical producer identity differs.
  for (const key of ["implementation", "version", "distribution_package", "lossless", "lossy", "native_diagnostic", "independence"])
    assert.deepEqual(actual.encoder[key], inputs.encoder[key], `encoder ${key}`);
  assert.deepEqual(actual.frames, inputs.frames, "encoded and decoded frame byte parity");
  const { cases, controls } = fixtures(actual);
  assert.equal(cases.length, reference.cases.length);
  for (const [i, row] of cases.entries()) {
    const { reference: observation, comparison, ...expected } = reference.cases[i];
    assert.deepEqual({ ...row, scene: scene(row) }, expected, `${row.id}: fixture reconstruction`);
  }
  assert.deepEqual(controls, reference.controls, "malformed control reconstruction");
}

export function verifyFrameParity() {
  const { inputs, reference } = frozenEvidence();
  const actual = encodeFrames(frameRequests());
  assertFrameParity(actual, inputs, reference);
  return actual;
}
