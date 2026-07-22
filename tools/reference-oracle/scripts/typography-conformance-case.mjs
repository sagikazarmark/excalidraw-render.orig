import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const SCRIPT_ROOT = dirname(fileURLToPath(import.meta.url));
const sourcePath = resolve(SCRIPT_ROOT, "conformance-case.mjs");
const temporaryPath = resolve(SCRIPT_ROOT, ".typography-conformance-case.generated.mjs");
let source = readFileSync(sourcePath, "utf8");

const replace = (before, after) => {
  if (!source.includes(before)) throw new Error(`typography oracle template is stale: ${before}`);
  source = source.replace(before, after);
};

source = source.replaceAll("focused.rectangle-outline.v1", "focused.excalifont-text.v1");
replace("tests/conformance/cases/focused/rectangle-outline", "tests/conformance/cases/focused/excalifont-text");
replace("target/reference-evidence/rectangle-outline", "target/reference-evidence/excalifont-text");
replace("tests/conformance/rectangle-reference-case-v1.schema.json", "tests/conformance/typography-reference-case-v1.schema.json");
replace('generated_at: "2026-07-19"', 'generated_at: "2026-07-22"');
replace(
  "const generatorPath = fileURLToPath(import.meta.url);",
  'const generatorPath = resolve(ORACLE_ROOT, "scripts/typography-conformance-case.mjs");',
);
replace(
  'const runnerPath = resolve(ORACLE_ROOT, "run-conformance.sh");',
  'const runnerPath = resolve(ORACLE_ROOT, "run-typography-conformance.sh");',
);
replace("assets: [],", `assets: [{
      byte_length: 24956,
      path: "fonts/Excalifont/Excalifont-Regular-a88b72a24fb54c9f94e3b5fdaa7481c9.woff2",
      role: "official-rendering-font",
      sha256: "e4423b318e11432aff2e6e865e300b7ca270f92321a3f56632268fede01c1b48",
    }],`);
replace(`coverage: [
      { identity: "element.rectangle", role: "primary" },
      { identity: "rough.outline", role: "primary" },`, `coverage: [
      { identity: "element.text", role: "primary" },
      { identity: "font.excalifont", role: "primary" },
      { identity: "text.wrapping", role: "primary" },`);
replace(
  'feature_tags: ["element:rectangle", "rough:outline", "format:svg", "format:png"],',
  'feature_tags: ["element:text", "font:excalifont", "text:wrapping", "format:svg", "format:png"],',
);
replace("issues/32", "issues/58");
replace(
  "Isolate one unrounded, transparent-fill Rough rectangle outline in official SVG and direct-canvas PNG.",
  "Isolate wrapped Excalifont text in official selectable SVG and direct-canvas PNG.",
);
replace("conformance-case/rectangle-reference/v1", "conformance-case/typography-reference/v1");
replace('element_type: "rectangle"', 'element_type: "text"');
replace("selectable_text: false", "selectable_text: true");

writeFileSync(temporaryPath, source);
try {
  await import(`${pathToFileURL(temporaryPath).href}?run=${Date.now()}`);
} finally {
  rmSync(temporaryPath, { force: true });
}
