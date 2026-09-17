// Run in the revision-1 Playwright container; see the qualification record in
// docs/research/ecmascript-number-formatting.md for the exact command.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { chromium } from "../reference-oracle/node_modules/playwright/index.mjs";
import playwright from "../reference-oracle/node_modules/playwright/package.json" with { type: "json" };

const profile = JSON.parse(readFileSync("tests/oracle/reference-profiles/revision-1.json"));
const runtime = profile.canonicalPlatform.runtime;
assert.equal(process.versions.node, runtime.node);
assert.equal(playwright.version, runtime.playwright);
const browser = await chromium.launch({ headless: true, args: profile.browser.launch.explicitArguments });
try {
  assert.equal(browser.version(), runtime.chromium);
  const page = await browser.newPage();
  const vectors = await page.evaluate(() => {
    const view = new DataView(new ArrayBuffer(8));
    const bits = (value) => {
      view.setFloat64(0, value);
      return view.getBigUint64(0);
    };
    const inputs = new Map();
    const add = (bits, label) => {
      const hex = bits.toString(16).padStart(16, "0");
      if (!inputs.has(hex)) inputs.set(hex, label);
    };
    const adjacent = (value, label) => {
      for (const sign of [1, -1]) {
        const center = bits(sign * value);
        for (const offset of [-1n, 0n, 1n]) add(center + offset, label);
      }
    };
    for (const [value, label] of [
      [1e-6, "lower exponent threshold"], [1e21, "upper exponent threshold"],
      [0.005, "rounded zero boundary"], [0.125, "half-cent tie"],
      [60.125, "Rough half-cent tie"], [1.005, "inexact half cent"],
      [2.675, "inexact half cent"], [1e9, "admitted coordinate ceiling"],
      [0.1 + 0.2, "coordinate arithmetic"], [1.5, "trailing zero"],
      [Number.MIN_VALUE, "smallest subnormal"], [2 ** -1022, "normal/subnormal boundary"],
      [1000000000000000128, "shortest versus fixed digits"],
    ]) adjacent(value, label);
    for (const value of [0, -0, Number.MAX_VALUE, -Number.MAX_VALUE, NaN, Infinity, -Infinity]) {
      add(bits(value), "special value");
    }
    // Frozen xorshift64 corpus: sample across exponents plus ordinary coordinates.
    let state = 0x180ec5a5n;
    for (let i = 0; i < 256; i++) {
      state ^= BigInt.asUintN(64, state << 13n);
      state ^= state >> 7n;
      state ^= BigInt.asUintN(64, state << 17n);
      add(state, "binary64 sample");
      add(bits((Number(state >> 11n) / 2 ** 53 - 0.5) * 2e9), "admitted coordinate sample");
    }
    return Array.from(inputs, ([hex, label]) => {
      view.setBigUint64(0, BigInt(`0x${hex}`));
      const value = view.getFloat64(0);
      const fixed = Number.isFinite(value) && Math.abs(value) < 1e21 ? value.toFixed(2) : null;
      return {
        bits: hex, label,
        shortest: Number.isFinite(value) ? String(value) : null,
        fixed,
        // Renderer policy keeps exact fixed digits, trims zeros and drops -0.
        // It does not round-trip large fixed integers through Number again.
        rough: fixed === null ? null : Number(fixed) === 0 ? "0"
          : fixed.replace(/\.?0+$/, ""),
      };
    });
  });
  const output = `{\n  "oracle": ${JSON.stringify({ profile: "revision-1", ...runtime })},\n`
    + `  "vectors": [\n${vectors.map((vector) => "    " + JSON.stringify(vector)).join(",\n")}\n  ]\n}\n`;
  const path = "crates/excalidraw-render/tests/fixtures/number-format-vectors.json";
  if (process.argv[2] === "--write") writeFileSync(path, output);
  else assert.equal(readFileSync(path, "utf8"), output, "number-format oracle vectors drifted");
} finally {
  await browser.close();
}
