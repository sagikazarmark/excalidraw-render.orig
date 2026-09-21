// Explicit bounded GIF requirements and authenticated preservation audit.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
const mode = process.argv[2]; assert.ok(["prepare", "archive-prior", "record", "verify"].includes(mode));
const base = "5780a00", prefix = "codec:gif:control:";
const oldBytes = path => execFileSync("git", ["show", `${base}:${path}`], { maxBuffer: 32 * 1024 * 1024 });
const read = path => JSON.parse(readFileSync(path));
const policyPath = "tests/conformance/coverage/ownership-policy.json", ledgerPath = "tests/conformance/coverage/ownership.json";
const oldPolicy = JSON.parse(oldBytes(policyPath)), oldLedger = JSON.parse(oldBytes(ledgerPath));
const source = "crates/excalidraw-render/tests/fast/gif_control_contract.rs";
const evidence = read("tests/oracle/gif-control/reference.json");
const cells = new Map();
for (const row of [...evidence.cases, ...evidence.controls]) {
  const cell = row.cell ?? row.id;
  if (!cells.has(cell)) cells.set(cell, []); cells.get(cell).push(row);
}
assert.equal(cells.size, 52);
const selector = row => row.collected === false ? "malformed_early_and_late_frames_have_exact_recovery"
  : "controls_match_frozen_reference_first_display";
const added = [...cells].map(([cell, rows]) => {
  const row = rows[0], malformed = row.collected === false;
  if (malformed) assert.equal(rows.length, 1); else assert.deepEqual(rows.map(r => r.pattern).sort(), [1, 2]);
  return { id: prefix + cell, category: "codec-profile",
    criterion: `GIF ${cell}: ${malformed ? "independently reconstructed authored fault in the specified early/later image, preserving exact positive prefix where applicable; validate whole-file rejection before source passthrough/default-frame projection, exact all-format decode-failed report/recovery, extents, replay and discriminating positive/EOI controls; no browser rejection claim" : "both frozen first-display probes match actual pinned-browser GCE/extension behavior with exact source reconstruction, SVG octets, PNG/PDF samples, alpha/extents, reports and replay; preserve distinct authored scope hypotheses, treat duplicate/orphan GCE as non-normative behavior, and prove frame-zero capture without claiming later-display or Plain Text glyph rendering"}. Each exact owner executes binding, matrix, discrimination and negative gates.`,
    spec_anchor: "### 8.2 Raster and Nested-SVG Assets" };
});
if (mode === "prepare") {
  assert.deepEqual(read(policyPath), oldPolicy);
  writeFileSync(policyPath, JSON.stringify({ ...oldPolicy, rows: [...oldPolicy.rows, ...added] }, null, 2) + "\n");
} else if (mode === "archive-prior") {
  const catalog = JSON.parse(execFileSync("cargo", ["run", "--locked", "-p", "xtask", "--", "conformance", "ownership-catalog"], { maxBuffer: 32 * 1024 * 1024 }));
  catalog.obligations = catalog.obligations.filter(r => !r.id.startsWith(prefix));
  catalog.sources.find(r => r.path === policyPath).sha256 = createHash("sha256").update(oldBytes(policyPath)).digest("hex");
  const canonical = v => Array.isArray(v) ? v.map(canonical) : v !== null && typeof v === "object"
    ? Object.fromEntries(Object.keys(v).sort().map(k => [k, canonical(v[k])])) : v;
  assert.equal(createHash("sha256").update(JSON.stringify(canonical(catalog))).digest("hex"), oldLedger.catalog_sha256);
  writeFileSync("tests/oracle/gif-control/prior-catalog.json", JSON.stringify(catalog, null, 2) + "\n", { flag: "wx" });
} else if (mode === "record") {
  const ledger = read(ledgerPath);
  assert.deepEqual(ledger.assignments.filter(r => !r.id.startsWith(prefix)), oldLedger.assignments);
  for (const [cell] of cells) assert.equal(ledger.assignments.find(r => r.id === prefix + cell).disposition.status, "gap");
  for (const fn of new Set([...cells.values()].map(rows => selector(rows[0])))) {
    const ids = [...cells].filter(([, rows]) => selector(rows[0]) === fn).map(([cell]) => prefix + cell);
    execFileSync("cargo", ["run", "--locked", "-p", "xtask", "--", "conformance", "record-test-owner", "gif_control_contract", fn,
      "Independent frozen first-display observations and exact early/later malformed recovery, source/prefix reconstruction, extent/alpha/report/replay and negative gates. Preserve Plain Text author/reference discrepancy; duplicate/orphan GCE probes are non-normative and later composites remain unqualified. See docs/research/gif-control-qualification.md.", ...ids], { stdio: "inherit" });
  }
} else {
  const policy = read(policyPath), ledger = read(ledgerPath);
  assert.deepEqual(policy.rows.filter(r => !r.id.startsWith(prefix)), oldPolicy.rows);
  assert.deepEqual(policy.rows.filter(r => r.id.startsWith(prefix)), added);
  assert.deepEqual(ledger.assignments.filter(r => !r.id.startsWith(prefix)), oldLedger.assignments);
  for (const [cell, rows] of cells) {
    const d = ledger.assignments.find(r => r.id === prefix + cell).disposition;
    assert.equal(d.status, "owned"); assert.equal(d.owner.function, selector(rows[0])); assert.equal(d.owner.source.path, source);
    assert.equal(d.owner.source.sha256, createHash("sha256").update(readFileSync(source)).digest("hex"));
  }
  assert.equal(ledger.assignments.length, 2370); assert.equal(ledger.approval, null);
  for (const path of ["Cargo.toml", "Cargo.lock", "tests/conformance/coverage/inventory.json", "tests/conformance/coverage/requirements.json",
    "tests/conformance/candidates/registry.json", "tests/conformance/candidates/stage4-frames-text-assets-v12/manifest.json"]) assert.deepEqual(readFileSync(path), oldBytes(path), path);
  console.log("Preserved 2318 prior dispositions, criteria/exclusions/168 keys/V12. Added 52 bounded owners. Final 2370 obligations / 2285 assigned-excluded / 85 gaps.");
}
