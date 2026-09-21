// Explicit bounded GIF requirements and authenticated preservation audit.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
const mode = process.argv[2]; assert.ok(["prepare", "archive-prior", "record", "verify"].includes(mode));
const base = "fafa996", prefix = "codec:gif:lzw:";
const oldBytes = path => execFileSync("git", ["show", `${base}:${path}`], { maxBuffer: 32 * 1024 * 1024 });
const read = path => JSON.parse(readFileSync(path));
const policyPath = "tests/conformance/coverage/ownership-policy.json", ledgerPath = "tests/conformance/coverage/ownership.json";
const oldPolicy = JSON.parse(oldBytes(policyPath)), oldLedger = JSON.parse(oldBytes(ledgerPath));
const source = "crates/excalidraw-render/tests/fast/gif_lzw_contract.rs";
const evidence = read("tests/oracle/gif-lzw/reference.json");
const cells = new Map();
for (const row of [...evidence.cases, ...evidence.controls]) {
  if (!cells.has(row.cell)) cells.set(row.cell, []); cells.get(row.cell).push(row);
}
assert.equal(cells.size, 56);
const selector = row => row.kind === "malformed" ? "malformed_lzw_controls_have_exact_decode_failed_recovery"
  : "valid_lzw_matrix_and_extensions_match_frozen_reference_exactly";
const added = [...cells].map(([cell, rows]) => {
  const r = rows[0];
  if (r.group === "matrix") assert.deepEqual(rows.map(v => v.pattern).sort(), [1, 2]);
  else assert.equal(rows.length, 1);
  const claim = r.kind === "malformed"
    ? "independently reconstructed authored malformed LZW/container control with exact all-format decode-failed reports, missing-asset recovery equivalence, extents and replay; GIF87a extension rejection is strict revision policy, not browser evidence"
    : r.group === "matrix"
      ? "both independently authored static full-screen opaque patterns qualify the declared palette size, GIF version, LZW growth/reset/KwKwK program and sub-block size; independently decoded indices/code-width/dictionary events, exact authored and pinned-browser pixels, source-octet SVG, PNG/PDF samples/alpha/extents, empty reports and replay"
      : "one independently authored static opaque full-screen extension vector qualifies extension framing and final image preservation; exact source reconstruction, authored and pinned-browser pixels, SVG octets, PNG/PDF samples/alpha/extents, reports and replay; no intermediate plain-text rendering or animated loop-timing claim";
  return { id: prefix + cell, category: "codec-profile", criterion: `GIF ${cell}: ${claim}. Exact owner executes closed-matrix, bounded independent LZW, reconstruction and negative gates; no codec-umbrella claim.`, spec_anchor: "### 8.2 Raster and Nested-SVG Assets" };
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
  writeFileSync("tests/oracle/gif-lzw/prior-catalog.json", JSON.stringify(catalog, null, 2) + "\n", { flag: "wx" });
} else if (mode === "record") {
  const ledger = read(ledgerPath);
  assert.deepEqual(ledger.assignments.filter(r => !r.id.startsWith(prefix)), oldLedger.assignments);
  for (const [cell] of cells) assert.equal(ledger.assignments.find(r => r.id === prefix + cell).disposition.status, "gap");
  for (const fn of new Set([...cells.values()].map(rows => selector(rows[0])))) {
    const ids = [...cells].filter(([, rows]) => selector(rows[0]) === fn).map(([cell]) => prefix + cell);
    execFileSync("cargo", ["run", "--locked", "-p", "xtask", "--", "conformance", "record-test-owner", "gif_lzw_contract", fn,
      "Independent authored indices/palettes, packed LZW and container reconstruction, exact code-width/dictionary-event checks and frozen browser surfaces; native artifact/report/extent/alpha/replay and negative gates execute under the exact selector. Uncollected malformed controls qualify strict recovery only. See docs/research/gif-lzw-qualification.md.", ...ids], { stdio: "inherit" });
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
  assert.equal(ledger.assignments.length, 2318); assert.equal(ledger.approval, null);
  for (const path of ["Cargo.toml", "Cargo.lock", "tests/conformance/coverage/inventory.json", "tests/conformance/coverage/requirements.json",
    "tests/conformance/candidates/registry.json", "tests/conformance/candidates/stage4-frames-text-assets-v12/manifest.json"]) assert.deepEqual(readFileSync(path), oldBytes(path), path);
  console.log("Preserved 2262 prior dispositions, all criteria/exclusions/168 keys/V12. Added 56 bounded owners. Final 2318 obligations / 2233 assigned-excluded / 85 gaps.");
}
