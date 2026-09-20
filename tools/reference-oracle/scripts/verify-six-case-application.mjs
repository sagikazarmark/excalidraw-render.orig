// Offline audit of the exact user-approved metadata transition. This is not a
// producer, case adapter or approval writer. Return authenticated before bytes
// so earlier preservation checkpoints can still be checked without rewriting them.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

export function verifySixCaseApplication(root) {
  const directory = "tests/conformance/coverage/stage4-six-case-application";
  if (!existsSync(resolve(root, directory, "applied-transition.json"))) return null;
  const packet = "tests/conformance/proposals/png-device-phase-six-case-v1";
  const subject = "3074ae5df1aa0c91743c68f4217f066370698f3d8970bac02dec5b1aacc00e84";
  const read = path => readFileSync(resolve(root, path));
  const json = path => JSON.parse(read(path));
  const sha = bytes => createHash("sha256").update(bytes).digest("hex");
  const canonical = value => Array.isArray(value) ? value.map(canonical) : value !== null && typeof value === "object" ? Object.fromEntries(Object.keys(value).sort().map(key => [key, canonical(value[key])])) : value;
  const digest = value => sha(JSON.stringify(canonical(value)));
  const descriptor = path => ({path, byte_length: read(path).length, sha256: sha(read(path))});
  const check = d => assert.deepEqual(d, descriptor(d.path), d.path);
  const bundle = json(`${packet}/approval-subject.json`);
  assert.equal(digest(bundle), subject);
  const batch = json(`${packet}/batch-manifest.json`);
  const bare = structuredClone(batch); delete bare.approval;
  assert.deepEqual(bundle, {schema: "excalidraw-render/conformance-approval-subject/v1", manifest: bare, files: batch.files});
  for (const d of batch.files) check({...d, path: `${packet}/${d.path}`});
  const record = json(`${directory}/applied-transition.json`);
  const plan = json(`${packet}/migration-plan.json`);
  assert.equal(record.schema, "excalidraw-render/six-case-approved-metadata-transition/v1");
  assert.equal(record.approval_subject_sha256, `sha256:${subject}`);
  const approval = json(`${directory}/approvals.json`);
  assert.deepEqual(record.approval, approval.batch_approval);
  assert.deepEqual(record.case_approvals, approval.cases);
  for (const d of record.comments) check(d);
  const checkApproval = (actual, issue, expected) => {
    const comment = json(`${directory}/issue-${issue}-comment.json`);
    assert.equal(comment.user.login, "sagikazarmark");
    assert.match(comment.html_url, new RegExp(`^https://github.com/sagikazarmark/excalidraw-render.orig/issues/${issue}#issuecomment-[0-9]+$`));
    assert.ok(comment.body.includes(subject));
    assert.ok(comment.body.includes(expected));
    assert.ok(comment.body.includes("not an autonomous approval decision by the coding agent"));
    assert.deepEqual(actual, {approved_by: comment.user.login, approved_at: comment.created_at.slice(0, 10), approval_url: comment.html_url, approval_subject_sha256: `sha256:${expected}`});
  };
  checkApproval(record.approval, "85", subject);
  const beforeFiles = new Map();
  const manifestPaths = batch.case_changes.map(r => r.proposed_manifest.path.replace(/^proposed\//, ""));
  const registryPath = "tests/conformance/cases/registry.json";
  const inventoryPath = "tests/conformance/coverage/inventory.json";
  const ledgerPath = "tests/conformance/coverage/ownership.json";
  const expectedPaths = [registryPath, inventoryPath, ledgerPath, ...manifestPaths, ...manifestPaths.map(p => p.replace(/manifest.json$/, "manifest.sha256"))].sort();
  assert.deepEqual(record.metadata_files.map(r => r.path).sort(), expectedPaths);
  for (const file of record.metadata_files) {
    assert.equal(file.before.path, `${packet}/before/${file.path}`);
    assert.equal(file.after.path, file.path);
    check(file.before); check(file.after);
    beforeFiles.set(file.path, read(file.before.path));
  }
  const before = path => JSON.parse(beforeFiles.get(path));
  assert.deepEqual(read(registryPath), read(`${packet}/proposed/${registryPath}`));
  for (const row of batch.case_changes) {
    const path = row.proposed_manifest.path.replace(/^proposed\//, "");
    const manifest = json(path), proposed = json(`${packet}/${row.proposed_manifest.path}`);
    const a = approval.cases.find(r => r.case_id === row.case_id).approval;
    checkApproval(a, row.normal_case_approving_issue, row.prospective_approval_subject_sha256.replace(/^sha256:/, ""));
    assert.deepEqual(manifest, {...proposed, approval: a});
    assert.equal(read(path.replace(/manifest.json$/, "manifest.sha256")).toString(), `${digest(manifest)}\n`);
  }
  for (const d of batch.live_preapplication_bindings) {
    const bytes = beforeFiles.get(d.path) ?? read(d.path);
    assert.equal(bytes.length, d.byte_length, d.path); assert.equal(sha(bytes), d.sha256, d.path);
  }
  const oldInventory = before(inventoryPath), inventory = json(inventoryPath);
  const inventorySources = oldInventory.sources.map(old => ({path: old.path, sha256: sha(read(old.path))}));
  const inputChanges = record.execution_input_binding_changes;
  assert.equal(inputChanges.length, 18);
  assert.deepEqual(inputChanges.map(r => r.before), plan.execution_input_binding_changes.map(r => r.before));
  assert.equal(inventory.executions.length, 168);
  const normalizedExecutions = inventory.executions.map((row, index) => {
    const old = oldInventory.executions[index];
    if (row.key.inputs_sha256 !== old.key.inputs_sha256) assert.deepEqual(inputChanges.find(r => r.before.case === old.key.case && r.before.format === old.key.format), {before: old.key, after: row.key});
    return {...row, key: {...row.key, inputs_sha256: old.key.inputs_sha256}};
  });
  assert.deepEqual(normalizedExecutions, oldInventory.executions);
  assert.equal(inventory.executions.filter((row, i) => row.key.inputs_sha256 === oldInventory.executions[i].key.inputs_sha256).length, 150);
  assert.deepEqual({...inventory, sources: oldInventory.sources, executions: normalizedExecutions}, oldInventory);
  assert.deepEqual(inventory.sources, inventorySources);
  const oldCatalog = json(`${packet}/before-catalog.json`), catalog = json(record.final_catalog.path);
  check(record.final_catalog);
  assert.equal(digest(oldCatalog), record.before_catalog_sha256);
  assert.equal(digest(catalog), record.after_catalog_sha256);
  const retired = new Set(plan.retire_only_unowned_scopes.map(r => r.assignment.id));
  assert.equal(retired.size, 2);
  assert.deepEqual(record.retired_unowned_scopes, plan.retire_only_unowned_scopes);
  const changedAuthorities = new Set(plan.authority_changes.map(r => r.id));
  assert.equal(changedAuthorities.size, 122);
  const expectedCatalog = {...oldCatalog, sources: oldCatalog.sources.map(d => ({path: d.path, sha256: sha(read(d.path))})), obligations: oldCatalog.obligations.filter(r => !retired.has(r.id)).map(r => changedAuthorities.has(r.id) ? {...r, authority: {path: r.authority.path, sha256: sha(read(r.authority.path))}} : r)};
  assert.deepEqual(catalog, expectedCatalog);
  const oldLedger = before(ledgerPath), ledger = json(ledgerPath);
  const owners = new Map(plan.case_owner_manifest_changes.map(r => [r.obligation, r]));
  assert.equal(owners.size, 12);
  const expectedLedger = structuredClone(oldLedger);
  expectedLedger.catalog_sha256 = record.after_catalog_sha256;
  expectedLedger.assignments = expectedLedger.assignments.filter(row => {
    if (retired.has(row.id)) { assert.equal(row.disposition.status, "gap"); return false; }
    if (owners.has(row.id)) {
      assert.deepEqual(row.disposition.owner, owners.get(row.id).before);
      const path = row.disposition.owner.manifest.path;
      row.disposition.owner.manifest.sha256 = sha(read(path));
    }
    return true;
  });
  assert.deepEqual(ledger, expectedLedger);
  assert.deepEqual(ledger.assignments.filter(r => r.disposition.status === "excluded"), plan.preserved_exclusions);
  assert.equal(ledger.approval, null);
  const statuses = ledger.assignments.reduce((counts, r) => {counts[r.disposition.status] = (counts[r.disposition.status] ?? 0) + 1; return counts;}, {});
  assert.deepEqual(statuses, {owned: 1800, gap: 71, excluded: 3});
  assert.deepEqual(record.counts, {assignments: 1874, owned: 1800, excluded: 3, assigned_or_excluded: 1803, gaps: 71});
  console.log(`Approved six-case metadata transition verified: sha256:${subject}; ${record.approval.approval_url}`);
  return {beforeFiles, counts: record.counts};
}
