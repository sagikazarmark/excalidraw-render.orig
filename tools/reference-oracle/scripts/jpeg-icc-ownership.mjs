// Explicit append-only inventory preparation and read-only preservation audit.
// Run prepare once, then normal xtask extend-ownership/record-test-owner.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
const mode = process.argv[2];
assert.ok(["prepare", "record", "verify"].includes(mode));
const base = "ff103a5";
const getOld = path => execFileSync("git", ["show", `${base}:${path}`], { maxBuffer: 32 * 1024 * 1024 });
const read = path => JSON.parse(readFileSync(path));
const policyPath = "tests/conformance/coverage/ownership-policy.json";
const ledgerPath = "tests/conformance/coverage/ownership.json";
const source = "crates/excalidraw-render/tests/fast/jpeg_icc_contract.rs";
const prefix = "codec:jpeg:icc:";
const oldPolicy = JSON.parse(getOld(policyPath)), oldLedger = JSON.parse(getOld(ledgerPath));
const record = read("tests/oracle/jpeg-icc/reference.json");
const cells = new Map();
for (const row of record.cases) {
  if (!cells.has(row.cell)) cells.set(row.cell, []);
  cells.get(row.cell).push(row);
}
assert.equal(cells.size, 80);
const selection = row => {
  if (["unsupported-format", "decode-failed"].includes(row.expected_outcome)) return "malformed_profiles_and_sequences_have_exact_recovery_and_source_preservation";
  if (row.expected_outcome === "inspect-model" || (row.source_model === "gray" && ["v2-g1", "v4-g1"].includes(row.signal))) return null;
  return "rgb_and_gamma22_gray_icc_profiles_match_both_reference_authorities";
};
const added = [...cells].map(([cell, rows]) => {
  assert.deepEqual(rows.map(r => r.pattern).sort(), ["edges", "ramp"]);
  const row = rows[0];
  const recovery = ["unsupported-format", "decode-failed"].includes(row.expected_outcome);
  return { id: prefix + cell, category: "codec-profile",
    criterion: `JPEG ICC ${cell}: both frozen ramp/edge vectors, exact extent/opaque-alpha/report/replay gates and SVG source-byte preservation where admitted; ${recovery ? `exact ${row.expected_outcome} recovery and missing-asset artifact equivalence under the existing strict contract, independently characterize browser behavior without claiming visual parity of rejected input` : "canonical direct-PNG and official-SVG-authoritative bound PDF sample comparison; demonstrate ICC precedence and multipart equivalence"}. Bounded profile/layout evidence, not whole-PDF viewer or exhaustive ICC qualification.`,
    spec_anchor: "### 8.2 Raster and Nested-SVG Assets" };
});
if (mode === "record") {
  const ledger = read(ledgerPath);
  assert.deepEqual(ledger.assignments.filter(r => !r.id.startsWith(prefix)), oldLedger.assignments);
  for (const [cell] of cells) assert.equal(ledger.assignments.find(r => r.id === prefix + cell).disposition.status, "gap");
  for (const fn of ["rgb_and_gamma22_gray_icc_profiles_match_both_reference_authorities", "malformed_profiles_and_sequences_have_exact_recovery_and_source_preservation"]) {
    const ids = [...cells].filter(([, rows]) => selection(rows[0]) === fn).map(([cell]) => prefix + cell);
    execFileSync("cargo", ["run", "--locked", "-p", "xtask", "--", "conformance", "record-test-owner", "jpeg_icc_contract", fn,
      "Independent pinned-browser ICC matrix with exact APP2/source reconstruction, reports/recovery, extents, alpha and replay; passing color cells compare both reference authorities. Strict rejection cells characterize recovery rather than granting a visual exception. See docs/research/jpeg-icc-qualification.md; #188 cells stay gaps.", ...ids], { stdio: "inherit" });
  }
} else if (mode === "prepare") {
  assert.deepEqual(read(policyPath), oldPolicy);
  writeFileSync(policyPath, JSON.stringify({ ...oldPolicy, rows: [...oldPolicy.rows, ...added] }, null, 2) + "\n");
  console.log("Added 80 requirements; use authenticated previous catalog with xtask extend-ownership before recording owners.");
} else {
  const policy = read(policyPath), ledger = read(ledgerPath);
  assert.deepEqual(policy.rows.filter(r => !r.id.startsWith(prefix)), oldPolicy.rows);
  assert.deepEqual(policy.rows.filter(r => r.id.startsWith(prefix)), added);
  assert.deepEqual(ledger.assignments.filter(r => !r.id.startsWith(prefix)), oldLedger.assignments);
  let owned = 0, gaps = 0;
  for (const [cell, rows] of cells) {
    const d = ledger.assignments.find(r => r.id === prefix + cell).disposition;
    const fn = selection(rows[0]);
    if (fn) {
      owned++; assert.equal(d.status, "owned"); assert.equal(d.owner.function, fn);
      assert.equal(d.owner.source.path, source);
      assert.equal(d.owner.source.sha256, createHash("sha256").update(readFileSync(source)).digest("hex"));
    } else { gaps++; assert.equal(d.status, "gap"); assert.equal(d.issue, 188); }
  }
  assert.deepEqual([owned, gaps], [64, 16]);
  assert.equal(ledger.assignments.length, 1992);
  assert.equal(ledger.approval, null);
  for (const path of ["Cargo.toml", "Cargo.lock", "tests/conformance/coverage/inventory.json", "tests/conformance/coverage/requirements.json",
    "tests/conformance/candidates/registry.json", "tests/conformance/candidates/stage4-frames-text-assets-v9/manifest.json"]) {
    assert.deepEqual(readFileSync(path), getOld(path), path);
  }
  console.log("Preserved all 1912 prior dispositions and V9; 80 new cells / 64 owners / 16 #188 gaps. Final 1992 obligations, 1899 assigned-excluded, 93 gaps.");
}
