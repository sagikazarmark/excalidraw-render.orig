// Read-only ADR-0011 migration audit against the preceding committed tranche.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { createHash } from "node:crypto";
const base = "cced47d";
const read = path => JSON.parse(readFileSync(path));
const committed = path => execFileSync("git", ["show", `${base}:${path}`], { maxBuffer: 16 * 1024 * 1024 });
const before = path => JSON.parse(committed(path));
const policyPath = "tests/conformance/coverage/ownership-policy.json";
const ledgerPath = "tests/conformance/coverage/ownership.json";
const policy = read(policyPath), oldPolicy = before(policyPath);
const ledger = read(ledgerPath), oldLedger = before(ledgerPath);
const prefix = "codec:jpeg:color-sampling:";
assert.deepEqual(policy.rows.filter(r => !r.id.startsWith(prefix)), oldPolicy.rows);
assert.deepEqual(ledger.assignments.filter(r => !r.id.startsWith(prefix)), oldLedger.assignments);
const rows = read("tests/oracle/jpeg-color/inputs.json").cases;
const cells = [...new Set(rows.map(r => r.cell))];
assert.equal(cells.length, 38);
const added = policy.rows.filter(r => r.id.startsWith(prefix));
assert.deepEqual(added.map(r => r.id).sort(), cells.map(c => prefix + c).sort());
assert.equal(ledger.assignments.length, 1912);
const qualified = row => ["gray", "rgb"].includes(row.model) || (row.model === "ycbcr" &&
  (row.mode === "progressive" || !["4,2", "1,4", "2,4"].includes(String(row.sampling))));
let owned = 0, gaps = 0;
for (const cell of cells) {
  const vectors = rows.filter(r => r.cell === cell);
  assert.deepEqual(vectors.map(r => r.pattern).sort(), ["edges", "ramp"]);
  const disposition = ledger.assignments.find(r => r.id === prefix + cell).disposition;
  if (qualified(vectors[0])) {
    owned++;
    assert.equal(disposition.status, "owned");
    const owner = disposition.owner;
    assert.equal(owner.kind, "test");
    assert.equal(owner.function, "qualified_gray_rgb_and_ycbcr_cells_pass_both_reference_authorities");
    assert.equal(owner.source.path, "crates/excalidraw-render/tests/fast/jpeg_color_contract.rs");
    assert.equal(owner.source.sha256, createHash("sha256").update(readFileSync(owner.source.path)).digest("hex"));
  } else {
    gaps++;
    assert.equal(disposition.status, "gap");
    assert.equal(disposition.issue, 187);
  }
}
assert.deepEqual([owned, gaps], [17, 21]);
assert.equal(ledger.approval, null);
for (const path of ["tests/conformance/coverage/inventory.json", "tests/conformance/coverage/requirements.json",
  "tests/conformance/candidates/registry.json", "tests/conformance/candidates/stage4-frames-text-assets-v8/manifest.json"]) {
  assert.deepEqual(readFileSync(path), committed(path));
}
console.log("Preserved 1874 prior dispositions, all criteria/exclusions, inventory and V8 approvals; added 38 cells, 17 owners, 21 #187 gaps. Final 1912 / 1820 assigned-excluded / 92 gaps.");
