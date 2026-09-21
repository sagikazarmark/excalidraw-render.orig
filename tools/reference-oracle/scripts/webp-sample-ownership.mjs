// Explicit bounded WebP requirements and authenticated preservation audit.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
const mode = process.argv[2]; assert.ok(["prepare", "archive-prior", "record", "verify"].includes(mode));
const base = "2fa700f", prefix = "codec:webp:sample:";
const oldBytes = path => execFileSync("git", ["show", `${base}:${path}`], { maxBuffer: 32 * 1024 * 1024 });
const read = path => JSON.parse(readFileSync(path));
const policyPath = "tests/conformance/coverage/ownership-policy.json", ledgerPath = "tests/conformance/coverage/ownership.json";
const oldPolicy = JSON.parse(oldBytes(policyPath)), oldLedger = JSON.parse(oldBytes(ledgerPath));
const source = "crates/excalidraw-render/tests/fast/webp_sample_contract.rs";
const qualified = new Set(["lossless-opaque-static-full", "lossless-opaque-anim-full-replace-d1-opaque",
  "lossless-binary-static-full", "lossless-binary-anim-full-replace-d1-opaque", "lossless-binary-anim-sub-replace-d0-clear", "lossless-binary-anim-sub-replace-d1-opaque",
  "lossy-opaque-static-full", "lossy-opaque-anim-full-blend-d0-clear", "lossy-opaque-anim-full-replace-d1-opaque",
  "lossy-binary-static-full", "lossy-binary-anim-full-replace-d1-opaque", "lossy-binary-anim-sub-replace-d0-clear", "lossy-binary-anim-sub-replace-d1-opaque"]);
const selector = row => row.collected === false ? row.id.startsWith("vp8x-reserved-")
  ? "reserved_vp8x_controls_are_accepted_characterization" : "malformed_10_controls_exact_recovery"
  : qualified.has(row.cell) ? "qualified_13_cells_first_display_contract" : null;
const evidence = read("tests/oracle/webp-sample/reference.json"), cells = new Map();
for (const row of [...evidence.cases, ...evidence.controls]) {
  const cell = row.cell ?? row.id;
  if (!cells.has(cell)) cells.set(cell, []); cells.get(cell).push(row);
}
assert.equal(cells.size, 48);
const added = [...cells].map(([cell, rows]) => {
  const row = rows[0];
  const claim = row.collected === false ? row.id.startsWith("vp8x-reserved-")
    ? "independently reconstructed reserved VP8X field control is accepted according to the specified ignore rule, preserving source bytes and exact baseline artifact/report behavior; no new pixel qualification for the underlying unqualified partial-alpha scene"
    : "independently reconstructed RIFF/chunk/frame fault has exact existing decode-failed recovery in SVG/PNG/PDF, missing-asset equivalence, extents and replay; reserved ANMF rejection is a strict implementation characterization, not a browser rejection claim"
    : "both independent external-libwebp encoded patterns match pinned-browser first-frame interpretation with exact RIFF/ANMF reconstruction, source-octet SVG, dimensions/alpha/reports/replay, canonical PNG and black/white PDF sample comparisons, and lossless straight-sample invariants; explicitly discriminate first/later and background hypotheses without inferring partial-alpha precision";
  return { id: prefix + cell, category: "codec-profile", criterion: `WebP ${cell}: ${claim}. Exact owner executes closed matrix, binding, reconstruction, discrimination and negative gates; no codec-umbrella or whole-PDF viewer claim.`, spec_anchor: "### 8.2 Raster and Nested-SVG Assets" };
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
  writeFileSync("tests/oracle/webp-sample/prior-catalog.json", JSON.stringify(catalog, null, 2) + "\n", { flag: "wx" });
} else if (mode === "record") {
  const ledger = read(ledgerPath);
  assert.deepEqual(ledger.assignments.filter(r => !r.id.startsWith(prefix)), oldLedger.assignments);
  for (const [cell] of cells) assert.equal(ledger.assignments.find(r => r.id === prefix + cell).disposition.status, "gap");
  for (const fn of new Set([...cells.values()].map(rows => selector(rows[0])).filter(Boolean))) {
    const ids = [...cells].filter(([, rows]) => selector(rows[0]) === fn).map(([cell]) => prefix + cell);
    execFileSync("cargo", ["run", "--locked", "-p", "xtask", "--", "conformance", "record-test-owner", "webp_sample_contract", fn,
      "Independent pinned libwebp source encoding/browser observations with exact RIFF reconstruction and source/extent/alpha/report/replay gates. Qualified sample cells pass canonical visual gates; strict recovery/reserved-ignore controls are bounded characterizations. #191 sample failures remain gaps. See docs/research/webp-sample-qualification.md.", ...ids], { stdio: "inherit" });
  }
} else {
  const policy = read(policyPath), ledger = read(ledgerPath);
  assert.deepEqual(policy.rows.filter(r => !r.id.startsWith(prefix)), oldPolicy.rows);
  assert.deepEqual(policy.rows.filter(r => r.id.startsWith(prefix)), added);
  assert.deepEqual(ledger.assignments.filter(r => !r.id.startsWith(prefix)), oldLedger.assignments);
  let owned = 0, gaps = 0;
  for (const [cell, rows] of cells) {
    const d = ledger.assignments.find(r => r.id === prefix + cell).disposition, fn = selector(rows[0]);
    if (fn) {
      owned++; assert.equal(d.status, "owned"); assert.equal(d.owner.function, fn); assert.equal(d.owner.source.path, source);
      assert.equal(d.owner.source.sha256, createHash("sha256").update(readFileSync(source)).digest("hex"));
    } else { gaps++; assert.equal(d.status, "gap"); assert.equal(d.issue, 191); }
  }
  assert.deepEqual([owned, gaps], [25, 23]); assert.equal(ledger.assignments.length, 2418); assert.equal(ledger.approval, null);
  for (const path of ["Cargo.toml", "Cargo.lock", "tests/conformance/coverage/inventory.json", "tests/conformance/coverage/requirements.json",
    "tests/conformance/candidates/registry.json", "tests/conformance/candidates/stage4-frames-text-assets-v12/manifest.json"]) assert.deepEqual(readFileSync(path), oldBytes(path), path);
  console.log("Preserved 2370 prior dispositions/criteria, exclusions/168 keys/V12. Added 48 cells:25 owners23 #191 gaps. Final2418 obligations/2310 assigned-excluded/108 gaps.");
}
