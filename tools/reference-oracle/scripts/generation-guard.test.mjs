// node --test scripts/generation-guard.test.mjs
import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import { carriedApproval, generationArgs } from "./generation-guard.mjs";

test("the command line admits --supersede only with generate", () => {
  assert.deepEqual(generationArgs(["reproduce", "case.v1"], "gen"), {
    mode: "reproduce",
    caseId: "case.v1",
    supersede: false,
  });
  assert.equal(generationArgs(["generate", "case.v1", "--supersede"], "gen").supersede, true);
  for (const argv of [
    ["reproduce", "case.v1", "--supersede"],
    ["generate", "case.v1", "--force"],
    ["generate"],
    ["regenerate", "case.v1"],
  ]) {
    assert.throws(() => generationArgs(argv, "gen"), /usage: gen/);
  }
});

test("a manifested case is replaced only when superseded, keeping its approval", () => {
  const directory = mkdtempSync(join(tmpdir(), "generation-guard-"));
  try {
    const manifest = join(directory, "manifest.json");
    const generate = { mode: "generate", caseId: "case.v1", supersede: false };
    assert.equal(carriedApproval(manifest, generate), null, "a new case carries nothing");

    const approval = { approved_by: "maintainer", approved_at: "2026-09-14" };
    writeFileSync(manifest, JSON.stringify({ approval }));
    assert.throws(() => carriedApproval(manifest, generate), /refuses to replace it without --supersede/);
    assert.deepEqual(carriedApproval(manifest, { ...generate, supersede: true }), approval);
    assert.equal(
      carriedApproval(manifest, { mode: "reproduce", caseId: "case.v1", supersede: false }),
      null,
      "reproduction writes no manifest",
    );
  } finally {
    rmSync(directory, { force: true, recursive: true });
  }
});
