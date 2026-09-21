// Explicit bounded GIF requirements and authenticated preservation audit.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
const mode = process.argv[2]; assert.ok(["prepare", "archive-prior", "record", "verify"].includes(mode));
const base = "4de0863", prefix = "codec:gif:first-frame:";
const oldBytes = path => execFileSync("git", ["show", `${base}:${path}`], { maxBuffer: 32 * 1024 * 1024 });
const read = path => JSON.parse(readFileSync(path));
const policyPath = "tests/conformance/coverage/ownership-policy.json", ledgerPath = "tests/conformance/coverage/ownership.json";
const oldPolicy = JSON.parse(oldBytes(policyPath)), oldLedger = JSON.parse(oldBytes(ledgerPath));
const source = "crates/excalidraw-render/tests/fast/gif_first_frame_contract.rs";
const evidence = read("tests/oracle/gif-first-frame/reference.json");
const cells = new Map();
for (const row of [...evidence.cases, ...evidence.controls]) {
  if (!cells.has(row.cell)) cells.set(row.cell, []); cells.get(row.cell).push(row);
}
assert.equal(cells.size, 136);
const selector = row => row.eligibility === "valid-first-frame-reference" ? "valid_first_frame_matrix_matches_frozen_reference_exactly"
  : row.kind === "malformed" ? "authored_malformed_controls_have_exact_decode_recovery"
  : row.id.startsWith("huge-") ? "huge_header_controls_are_preempted_by_bounded_validation_work"
  : "byte_pixel_dimension_and_prepaid_work_boundaries_are_exact";
const added = [...cells].map(([cell, rows]) => {
  const r = rows[0];
  if (r.eligibility === "valid-first-frame-reference") assert.deepEqual(rows.map(v => v.pattern).sort(), ["alternating", "asymmetric"]);
  else assert.equal(rows.length, 1);
  const claim = r.eligibility === "valid-first-frame-reference"
    ? "both independent asymmetric GIF89a four-entry literal-LZW vectors: first-frame logical-screen/palette/offset/transparency/interlace interpretation, disposal as frame-zero metadata only, exact authored and pinned-browser pixels, SVG octets, PNG/PDF samples/alpha/extents, ordered flattening reports and replay"
    : r.kind === "malformed" ? "independently reconstructed authored malformed control: exact all-format decode-failed reports, missing-asset artifact equivalence, extents and replay; not browser admission evidence"
    : r.id.startsWith("huge-") ? "independently authored huge header is preempted by an asset-work cap before large decode/allocation, with exact resource report/recovery; no claim of decode, pixel-limit precedence or physical OOM qualification"
    : "finite authored valid control at byte, logical-pixel, dimension and prepaid validation-work equality/first crossing, exact format-specific reports/recovery and direct-SVG pixel exemption; no aggregate/resource-umbrella claim";
  return { id: prefix + cell, category: "codec-profile", criterion: `GIF ${cell}: ${claim}. Every owner executes closed matrix, source reconstruction and negative/discrimination gates.`, spec_anchor: "### 8.2 Raster and Nested-SVG Assets" };
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
  writeFileSync("tests/oracle/gif-first-frame/prior-catalog.json", JSON.stringify(catalog, null, 2) + "\n", { flag: "wx" });
} else if (mode === "record") {
  const ledger = read(ledgerPath);
  assert.deepEqual(ledger.assignments.filter(r => !r.id.startsWith(prefix)), oldLedger.assignments);
  for (const [cell] of cells) assert.equal(ledger.assignments.find(r => r.id === prefix + cell).disposition.status, "gap");
  for (const fn of new Set([...cells.values()].map(rows => selector(rows[0])))) {
    const ids = [...cells].filter(([, rows]) => selector(rows[0]) === fn).map(([cell]) => prefix + cell);
    execFileSync("cargo", ["run", "--locked", "-p", "xtask", "--", "conformance", "record-test-owner", "gif_first_frame_contract", fn,
      "Bounded independently authored GIF first-frame/recovery/resource vectors; exact owner executes reconstruction, discrimination, negative gates, reports and replay. Browser claims apply only to the 240 valid collected vectors; huge controls prove work preemption only. See docs/research/gif-first-frame-qualification.md.", ...ids], { stdio: "inherit" });
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
  assert.equal(ledger.assignments.length, 2262); assert.equal(ledger.approval, null);
  for (const path of ["Cargo.toml", "Cargo.lock", "tests/conformance/coverage/inventory.json", "tests/conformance/coverage/requirements.json",
    "tests/conformance/candidates/registry.json", "tests/conformance/candidates/stage4-frames-text-assets-v12/manifest.json"]) assert.deepEqual(readFileSync(path), oldBytes(path), path);
  console.log("Preserved 2126 prior dispositions, all criteria/exclusions/168 keys/V12. Added 136 bounded owners. Final 2262 obligations / 2177 assigned-excluded / 85 gaps.");
}
