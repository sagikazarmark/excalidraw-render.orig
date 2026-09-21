// Explicit ADR-0011 extension and read-only preservation audit.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
const mode = process.argv[2]; assert.ok(["prepare", "archive-prior", "record", "verify"].includes(mode));
const base = "b150028", prefix = "codec:jpeg:mpf-xmp:";
const oldBytes = path => execFileSync("git", ["show", `${base}:${path}`], { maxBuffer: 32 * 1024 * 1024 });
const read = path => JSON.parse(readFileSync(path));
const policyPath = "tests/conformance/coverage/ownership-policy.json", ledgerPath = "tests/conformance/coverage/ownership.json";
const oldPolicy = JSON.parse(oldBytes(policyPath)), oldLedger = JSON.parse(oldBytes(ledgerPath));
const source = "crates/excalidraw-render/tests/fast/jpeg_gain_map_contract.rs";
const owner = signal => ["identity", "sdr-ramp", "sdr-edges"].includes(signal) ? "valid_sdr_gain_maps_match_reference_base_projection"
  : ["omit-mpf", "omit-xmp", "truncated-secondary", "third-image"].includes(signal) ? "invalid_container_framing_has_exact_recovery" : null;
const cells = new Map();
for (const row of read("tests/oracle/jpeg-gain-map/reference.json").cases) {
  if (!cells.has(row.cell)) cells.set(row.cell, []); cells.get(row.cell).push(row);
}
assert.equal(cells.size, 52);
const additions = [...cells].map(([cell, rows]) => {
  assert.deepEqual(rows.map(r => r.pattern).sort(), ["edges", "ramp"]);
  const recovery = owner(rows[0].signal) === "invalid_container_framing_has_exact_recovery";
  return { id: prefix + cell, category: "codec-profile",
    criterion: `JPEG MPF/Adobe-XMP ${cell}: independently reconstruct both frozen base patterns and verify MP index/TIFF image ranges, XMP namespaces/gain parameters and metadata placement; ${recovery ? "exact existing decode-failed reports, recovery artifact equivalence and extents in all formats, characterizing browser fallback without claiming visual parity for rejected input" : "canonical direct-PNG and official-SVG-authoritative bound PDF sample interpretation with discriminating gain controls, validated source-octet SVG preservation, exact extents/opaque alpha/reports and replay; malformed-index acceptance alone cannot establish validity"}. Bounded Adobe XMP/MPF evidence; no ISO21496-1, Apple or whole-PDF viewer claim.`,
    spec_anchor: "### 8.2 Raster and Nested-SVG Assets" };
});
if (mode === "prepare") {
  assert.deepEqual(read(policyPath), oldPolicy);
  writeFileSync(policyPath, JSON.stringify({ ...oldPolicy, rows: [...oldPolicy.rows, ...additions] }, null, 2) + "\n");
} else if (mode === "archive-prior") {
  const catalog = JSON.parse(execFileSync("cargo", ["run", "--locked", "-p", "xtask", "--", "conformance", "ownership-catalog"], { maxBuffer: 32 * 1024 * 1024 }));
  catalog.obligations = catalog.obligations.filter(r => !r.id.startsWith(prefix));
  catalog.sources.find(r => r.path === policyPath).sha256 = createHash("sha256").update(oldBytes(policyPath)).digest("hex");
  const canonical = v => Array.isArray(v) ? v.map(canonical) : v !== null && typeof v === "object"
    ? Object.fromEntries(Object.keys(v).sort().map(k => [k, canonical(v[k])])) : v;
  assert.equal(createHash("sha256").update(JSON.stringify(canonical(catalog))).digest("hex"), oldLedger.catalog_sha256);
  writeFileSync("tests/oracle/jpeg-gain-map/prior-catalog.json", JSON.stringify(catalog, null, 2) + "\n", { flag: "wx" });
} else if (mode === "record") {
  const ledger = read(ledgerPath);
  assert.deepEqual(ledger.assignments.filter(r => !r.id.startsWith(prefix)), oldLedger.assignments);
  for (const [cell] of cells) assert.equal(ledger.assignments.find(r => r.id === prefix + cell).disposition.status, "gap");
  for (const fn of ["valid_sdr_gain_maps_match_reference_base_projection", "invalid_container_framing_has_exact_recovery"]) {
    const ids = [...cells].filter(([, rows]) => owner(rows[0].signal) === fn).map(([cell]) => prefix + cell);
    execFileSync("cargo", ["run", "--locked", "-p", "xtask", "--", "conformance", "record-test-owner", "jpeg_gain_map_contract", fn,
      "Independent MP Index/XMP reconstruction, two authored patterns, repeated pinned-browser canvas/SVG authorities, exact native reports/extents/source preservation/alpha/replay and strict recovery where declared. Bounded SDR/recovery cells only; HDR-base and malformed-index scope remains #190. See docs/research/jpeg-gain-map-qualification.md.", ...ids], { stdio: "inherit" });
  }
} else {
  const policy = read(policyPath), ledger = read(ledgerPath);
  assert.deepEqual(policy.rows.filter(r => !r.id.startsWith(prefix)), oldPolicy.rows);
  assert.deepEqual(policy.rows.filter(r => r.id.startsWith(prefix)), additions);
  assert.deepEqual(ledger.assignments.filter(r => !r.id.startsWith(prefix)), oldLedger.assignments);
  let owned = 0, gaps = 0;
  for (const [cell, rows] of cells) {
    const d = ledger.assignments.find(r => r.id === prefix + cell).disposition, fn = owner(rows[0].signal);
    if (fn) {
      owned++; assert.equal(d.status, "owned"); assert.equal(d.owner.function, fn); assert.equal(d.owner.source.path, source);
      assert.equal(d.owner.source.sha256, createHash("sha256").update(readFileSync(source)).digest("hex"));
    } else { gaps++; assert.equal(d.status, "gap"); assert.equal(d.issue, 190); }
  }
  assert.deepEqual([owned, gaps], [28, 24]); assert.equal(ledger.assignments.length, 2126); assert.equal(ledger.approval, null);
  for (const path of ["Cargo.toml", "Cargo.lock", "tests/conformance/coverage/inventory.json", "tests/conformance/coverage/requirements.json",
    "tests/conformance/candidates/registry.json", "tests/conformance/candidates/stage4-frames-text-assets-v11/manifest.json"]) {
    assert.deepEqual(readFileSync(path), oldBytes(path), path);
  }
  console.log("Preserved 2074 prior dispositions/criteria, exclusions, inventory and V11. Added 52 cells: 28 owners / 24 #190 gaps. Final 2126 obligations / 2025 assigned-excluded / 101 gaps.");
}
