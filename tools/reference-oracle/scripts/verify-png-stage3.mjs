// Read-only Stage 3 preservation/ownership audit; no refresh or collection.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
const root = fileURLToPath(new URL("../../../", import.meta.url));
const base = "edbacbde66b7442b5e66a31cf7b53d749eddb263";
const read = path => readFileSync(root + path);
const previous = path => execFileSync("git", ["show",`${base}:${path}`], {cwd:root,maxBuffer:64*1024*1024});
const hash = bytes => createHash("sha256").update(bytes).digest("hex");
const colorPath = "tests/oracle/png-color/reference.json";
const beforeBytes = previous(colorPath);
assert.equal(hash(beforeBytes),"9c53ba1857d7e6406863200a685e9078e68e109569fed9c96bbcc1bcfb9c95ad");
const before = JSON.parse(beforeBytes), after = JSON.parse(read(colorPath));
assert.equal(before.cases.length,896); assert.equal(after.cases.length,984);
const byId = new Map(after.cases.map(r=>[r.id,r]));
assert.equal(byId.size,after.cases.length);
for(const row of before.cases) assert.deepEqual(byId.get(row.id),row,`${row.id}: prior evidence changed`);
for(const key of ["schema","options","visual_class","collection"]) assert.deepEqual(after[key],before[key]);
assert.deepEqual(after.sources.map(s=>s.path),before.sources.map(s=>s.path));
for(const source of before.sources) if(source.path !== "tests/oracle/png-color/inputs.json") {
  assert.deepEqual(after.sources.find(s=>s.path===source.path),source);
}
const additions = new Set(), caseIds = new Set();
for(const depth of [8,16]) for(const p of [1,4,5,6,7,8,9,10,11,12,22]) for(const t of [16,18]) for(const r of [0,1]) {
  additions.add(`codec:png:cicp:rgba${depth}:p${p}:t${t}:r${r}`);
  caseIds.add(`cicp-p${p}-t${t}-r${r}-c6-d${depth}`);
}
const oldIds = new Set(before.cases.map(r=>r.id));
assert.deepEqual(new Set(after.cases.filter(r=>!oldIds.has(r.id)).map(r=>r.id)),caseIds);
for(const facet of ["sdr-base","admission","precedence"]) additions.add(`codec:png:gain-map:${facet}`);
const ledgerPath = "tests/conformance/coverage/ownership.json";
const oldLedger = JSON.parse(previous(ledgerPath)), ledger = JSON.parse(read(ledgerPath));
for(const key of ["schema","compatibility_revision","approval"]) assert.deepEqual(ledger[key],oldLedger[key]);
// Parent-review correction: Skia's adaptation notice changes only the license
// bundle pin in the packaged consumer. Authenticate that exact one-line edit
// and the three resulting source anchors before accepting the catalog rebind.
const consumerPath = "tests/package-consumer/tests/public_facade.rs";
assert.equal(read(consumerPath).toString(),previous(consumerPath).toString().replace(
  "a3236318603e4035951ad166ad827ef9fd3a98d9eeae9996e804aa6048d20077",
  "cadd243b2228954efb8f61f62d492bcde65a23f8e7e96a2db5a6cc42ce78a9f2"));
const requirementsPath = "tests/conformance/coverage/requirements.json";
const inventoryPath = "tests/conformance/coverage/inventory.json";
for(const path of [requirementsPath,inventoryPath]) {
  const old = JSON.parse(previous(path)), current = JSON.parse(read(path));
  let anchors = 0;
  for(const row of old.requirements) for(const evidence of row.evidence) {
    if(evidence.source.path === consumerPath) { evidence.source.sha256 = hash(read(consumerPath)); anchors++; }
  }
  assert.equal(anchors,3);
  if(path === inventoryPath) {
    old.sources.find(s=>s.path===requirementsPath).sha256 = hash(read(requirementsPath));
  }
  assert.deepEqual(current,old,`${path}: changes beyond the license-consumer bindings`);
}
assert.equal(oldLedger.catalog_sha256,"fef633fa17a30830c29ee86ce2d2d200d5da63e5f4cc2c7f8ae43873f7c979e7");
assert.equal(ledger.catalog_sha256,"fb90d519e1ddb2a21b40357df48ab47f02efdce02403705f8890e9e1e7ef5001");
const module = "crates/excalidraw-render/tests/fast/png_color_contract.rs";
const currentSource = hash(read(module));
let refreshed = 0;
assert.equal(ledger.assignments.length,1876);
for(let i=0;i<ledger.assignments.length;i++) {
  const old = oldLedger.assignments[i], now = ledger.assignments[i];
  assert.equal(old.id,now.id);
  if(additions.has(old.id)) {
    assert.equal(old.disposition.status,"gap"); assert.equal(now.disposition.status,"owned");
    assert.equal(now.disposition.owner.source.path,module);
    assert.equal(now.disposition.owner.source.sha256,currentSource);
    assert.equal(now.disposition.owner.function,old.id.includes(":gain-map:") ? "gain_map_base_admission_and_precedence_match_the_reference" : "sdr_cicp_candidates_match_the_reference");
    continue;
  }
  if(old.disposition.owner?.source?.path === module) {
    old.disposition.owner.source.sha256 = currentSource; refreshed++;
  }
  assert.deepEqual(now,old,`${now.id}: prior disposition changed`);
}
assert.equal(refreshed,623);
const counts = Object.fromEntries(["owned","excluded","gap"].map(status=>[status,ledger.assignments.filter(r=>r.disposition.status===status).length]));
assert.deepEqual(counts,{owned:1740,excluded:3,gap:133});
const supplementalPath = "tests/oracle/png-color/gain-map-reference.json";
const supplemental = JSON.parse(read(supplementalPath));
const sources = new Map([...after.sources,...supplemental.sources].map(s=>[s.path,s.sha256]));
for(const [path,sha256] of sources) assert.equal(hash(read(path)),sha256,path);
for(const path of [colorPath,supplementalPath,module,"crates/excalidraw-render/tests/common/png_color_reference.rs"]) sources.set(path,hash(read(path)));
console.log(JSON.stringify({base,preserved_cases:896,added_cicp_cases:88,gain_map_cases:supplemental.cases.length,new_owners:91,source_only_refreshes:refreshed,counts,sha256:Object.fromEntries(sources)},null,2));
