// Explicit append-only preparation/recording and read-only preservation audit.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
const mode = process.argv[2]; assert.ok(["prepare", "archive-prior", "record", "verify"].includes(mode));
const base = "b0b6004", prefix = "codec:jpeg:exif-precedence:";
const oldBytes = path => execFileSync("git", ["show", `${base}:${path}`], { maxBuffer: 32 * 1024 * 1024 });
const read = path => JSON.parse(readFileSync(path));
const policyPath = "tests/conformance/coverage/ownership-policy.json", ledgerPath = "tests/conformance/coverage/ownership.json";
const oldPolicy = JSON.parse(oldBytes(policyPath)), oldLedger = JSON.parse(oldBytes(ledgerPath));
const source = "crates/excalidraw-render/tests/fast/jpeg_exif_contract.rs";
const fn = "qualified_exif_precedence_matches_reference";
const failed = new Set(["duplicate-6-3", "duplicate-3-6", "invalid-then-valid", "valid-then-invalid",
  "absent-tag-then-valid", "valid-then-absent-tag", "bad-magic-then-valid", "valid-then-bad-magic", "invalid-tag-then-valid"]);
const cells = new Map();
for (const row of read("tests/oracle/jpeg-exif/reference.json").cases) {
  if (!cells.has(row.cell)) cells.set(row.cell, []); cells.get(row.cell).push(row);
}
assert.equal(cells.size, 82);
const additions = [...cells].map(([cell, rows]) => {
  assert.deepEqual(rows.map(r => r.pattern).sort(), ["edges", "ramp"]);
  return { id: prefix + cell, category: "codec-profile",
    criterion: `JPEG EXIF ${cell}: both fixed asymmetric ramp/edge vectors follow independently collected first-APP1/IFD orientation and late-marker precedence, with exact source reconstruction, SVG octets, output/page/image extents, opaque alpha, reports and replay. PNG and bound PDF image samples pass their respective canonical reference authorities; independent permutations distinguish identity/180/90/270 degrees. No full-PDF viewer or exhaustive TIFF-chain claim.`,
    spec_anchor: "applies the format-specific orientation" };
});
if (mode === "archive-prior") {
  // Reconstruct only the append-only predecessor, then authenticate it against
  // the untouched ledger's canonical catalog digest before writing the archive.
  const catalog = JSON.parse(execFileSync("cargo", ["run", "--locked", "-p", "xtask", "--", "conformance", "ownership-catalog"], { maxBuffer: 32 * 1024 * 1024 }));
  catalog.obligations = catalog.obligations.filter(r => !r.id.startsWith(prefix));
  catalog.sources.find(r => r.path === policyPath).sha256 = createHash("sha256").update(oldBytes(policyPath)).digest("hex");
  const canonical = v => Array.isArray(v) ? v.map(canonical) : v !== null && typeof v === "object"
    ? Object.fromEntries(Object.keys(v).sort().map(k => [k, canonical(v[k])])) : v;
  assert.equal(createHash("sha256").update(JSON.stringify(canonical(catalog))).digest("hex"), oldLedger.catalog_sha256);
  writeFileSync("tests/oracle/jpeg-exif/prior-catalog.json", JSON.stringify(catalog, null, 2) + "\n", { flag: "wx" });
  console.log("Archived authenticated 1992-obligation predecessor catalog.");
} else if (mode === "prepare") {
  assert.deepEqual(read(policyPath), oldPolicy);
  writeFileSync(policyPath, JSON.stringify({ ...oldPolicy, rows: [...oldPolicy.rows, ...additions] }, null, 2) + "\n");
  console.log("Added 82 requirements. Extend against prior catalog before recording passing owners.");
} else if (mode === "record") {
  const ledger = read(ledgerPath);
  assert.deepEqual(ledger.assignments.filter(r => !r.id.startsWith(prefix)), oldLedger.assignments);
  for (const [cell] of cells) assert.equal(ledger.assignments.find(r => r.id === prefix + cell).disposition.status, "gap");
  const ids = [...cells].filter(([, rows]) => !failed.has(rows[0].signal)).map(([cell]) => prefix + cell);
  assert.equal(ids.length, 46);
  execFileSync("cargo", ["run", "--locked", "-p", "xtask", "--", "conformance", "record-test-owner", "jpeg_exif_contract", fn,
    "Both independent browser patterns, exact APP1/source reconstruction, discriminating orientation permutations, canonical PNG/bound-PDF samples, SVG octets, dimensions, full reports and replay. Narrow duplicate/late EXIF cell only. See docs/research/jpeg-exif-precedence-qualification.md; 36 #189 failing cells remain gaps.", ...ids], { stdio: "inherit" });
} else {
  const policy = read(policyPath), ledger = read(ledgerPath);
  assert.deepEqual(policy.rows.filter(r => !r.id.startsWith(prefix)), oldPolicy.rows);
  assert.deepEqual(policy.rows.filter(r => r.id.startsWith(prefix)), additions);
  assert.deepEqual(ledger.assignments.filter(r => !r.id.startsWith(prefix)), oldLedger.assignments);
  let owned = 0, gaps = 0;
  for (const [cell, rows] of cells) {
    const d = ledger.assignments.find(r => r.id === prefix + cell).disposition;
    if (failed.has(rows[0].signal)) { gaps++; assert.equal(d.status, "gap"); assert.equal(d.issue, 189); }
    else {
      owned++; assert.equal(d.status, "owned"); assert.equal(d.owner.function, fn);
      assert.equal(d.owner.source.path, source);
      assert.equal(d.owner.source.sha256, createHash("sha256").update(readFileSync(source)).digest("hex"));
    }
  }
  assert.deepEqual([owned, gaps], [46, 36]); assert.equal(ledger.assignments.length, 2074); assert.equal(ledger.approval, null);
  for (const path of ["Cargo.toml", "Cargo.lock", "tests/conformance/coverage/inventory.json", "tests/conformance/coverage/requirements.json",
    "tests/conformance/candidates/registry.json", "tests/conformance/candidates/stage4-frames-text-assets-v10/manifest.json"]) {
    assert.deepEqual(readFileSync(path), oldBytes(path), path);
  }
  console.log("Preserved 1992 prior dispositions, criteria, exclusions, inventory and V10. Added 82 cells: 46 owners / 36 #189 gaps. Final 2074 / 1961 assigned-excluded / 113 gaps.");
}
