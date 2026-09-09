// Derives the Rough.js 4.6.4 path vectors the pinned filler tests in
// `crates/excalidraw-render/src/render/tests.rs` compare against, by running
// the pinned npm `roughjs` exactly as Excalidraw 0.18.1's `generateRoughOptions`
// and `generateElementShape` drive it, and serializing every set with
// `opsToPath(set, 2)` — the `fixedDecimalPlaceDigits` the static SVG export
// uses, which `push_rough_number` reproduces.
//
// The dot filler scatters each dot's centre with the unseeded `Math.random()`,
// so the reference's own output for `dots` differs from one export to the
// next. This derivation stubs that call with a second Rough.js `Random` seeded
// with the element seed, separate from the `randomizer` the options carry: the
// seeded stream the reference consumes for the dot ellipses is untouched and
// only the centres take a deterministic sample of the reference's jitter. The
// renderer draws the same sample (`rough::dots_fill`, decided on #137).
//
// Usage, after `tools/reference-oracle/run-container.sh install` has fetched
// the pinned dependency closure:
//
//   node tools/rough-filler-vectors/derive.mjs \
//     tools/reference-oracle/node_modules/roughjs/bundled/rough.esm.js \
//     < scene.excalidraw
//
// The scene is a scene document whose elements are rectangles, diamonds,
// ellipses or closed lines without arrowheads, the shapes the test scenes
// carry; each element's paths are printed in document order, the fill first,
// then the outline, with their SHA-256 digests and the digest of every path
// joined by `\n`, which is what the Rust tests pin.
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const roughPath = process.argv[2];
if (!roughPath) {
  console.error("usage: derive.mjs <roughjs/bundled/rough.esm.js> < scene.excalidraw");
  process.exit(2);
}
const { default: rough } = await import(pathToFileURL(roughPath).href);
const generator = rough.generator();

// Rough.js `Random` (bin/math.js), the seeded generator every option set
// carries; stands in for `Math.random()` while a dots fill is generated.
class Random {
  constructor(seed) {
    this.seed = seed;
  }
  next() {
    if (this.seed) {
      return ((2 ** 31 - 1) & (this.seed = Math.imul(48271, this.seed))) / 2 ** 31;
    }
    return Math.random();
  }
}

// scene/Shape.ts `adjustRoughness`.
function adjustRoughness(element) {
  const roughness = element.roughness;
  const maxSize = Math.max(element.width, element.height);
  const minSize = Math.min(element.width, element.height);
  const isLinear = element.type === "line" || element.type === "arrow";
  const canChangeRoundness = ["rectangle", "diamond", "line", "arrow", "image", "embeddable", "iframe"].includes(element.type);
  if (
    (minSize >= 20 && maxSize >= 50) ||
    (minSize >= 15 && !!element.roundness && canChangeRoundness) ||
    (isLinear && maxSize >= 50)
  ) {
    return roughness;
  }
  return Math.min(roughness / (maxSize < 10 ? 3 : 2), 2.5);
}

// scene/Shape.ts `generateRoughOptions`, for the shapes this script draws.
function generateRoughOptions(element, continuousPath = false) {
  const options = {
    seed: element.seed,
    disableMultiStroke: element.strokeStyle !== "solid",
    strokeWidth: element.strokeStyle !== "solid" ? element.strokeWidth + 0.5 : element.strokeWidth,
    fillWeight: element.strokeWidth / 2,
    hachureGap: element.strokeWidth * 4,
    roughness: adjustRoughness(element),
    stroke: element.strokeColor,
    preserveVertices: continuousPath || element.roughness < 2,
  };
  options.fillStyle = element.fillStyle;
  options.fill = element.backgroundColor === "transparent" ? undefined : element.backgroundColor;
  if (element.type === "ellipse") {
    options.curveFitting = 1;
  }
  return options;
}

// element/shape.ts `getCornerRadius`.
function getCornerRadius(x, element) {
  const type = element.roundness?.type;
  if (type === 1 || type === 2) {
    return x * 0.25;
  }
  if (type === 3) {
    const fixedRadiusSize = element.roundness?.value ?? 32;
    const cutoffSize = fixedRadiusSize / 0.25;
    return x <= cutoffSize ? x * 0.25 : fixedRadiusSize;
  }
  return 0;
}

// scene/Shape.ts `generateElementShape`, for the shapes this script draws.
function shape(element) {
  const w = element.width;
  const h = element.height;
  switch (element.type) {
    case "rectangle": {
      if (element.roundness) {
        const r = getCornerRadius(Math.min(w, h), element);
        return generator.path(
          `M ${r} 0 L ${w - r} 0 Q ${w} 0, ${w} ${r} L ${w} ${h - r} Q ${w} ${h}, ${w - r} ${h} L ${r} ${h} Q 0 ${h}, 0 ${h - r} L 0 ${r} Q 0 0, ${r} 0`,
          generateRoughOptions(element, true),
        );
      }
      return generator.rectangle(0, 0, w, h, generateRoughOptions(element, false));
    }
    case "diamond": {
      const topX = Math.floor(w / 2) + 1;
      const rightY = Math.floor(h / 2) + 1;
      return generator.polygon(
        [[topX, 0], [w, rightY], [topX, h], [0, rightY]],
        generateRoughOptions(element),
      );
    }
    case "ellipse":
      return generator.ellipse(w / 2, h / 2, w, h, generateRoughOptions(element));
    case "line":
      return generator.polygon(element.points, generateRoughOptions(element));
    default:
      throw new Error(`derive.mjs does not draw ${element.type}`);
  }
}

// The revision-1 defaults of the members the shapes read.
const defaults = {
  strokeColor: "#1e1e1e",
  backgroundColor: "transparent",
  fillStyle: "solid",
  strokeWidth: 2,
  strokeStyle: "solid",
  roughness: 1,
  roundness: null,
};

const scene = JSON.parse(readFileSync(0, "utf8"));
const nativeRandom = Math.random;
const output = [];
const allPaths = [];
for (const element of scene.elements.map((element) => ({ ...defaults, ...element }))) {
  const scatter = new Random(element.seed);
  Math.random = () => scatter.next();
  const drawable = shape(element);
  Math.random = nativeRandom;
  const paths = drawable.sets.map((set) => generator.opsToPath(set, 2));
  allPaths.push(...paths);
  output.push({
    id: element.id,
    operations: paths.map((path) => (path.match(/[MCL]/g) ?? []).length),
    sha256: paths.map((path) => createHash("sha256").update(path).digest("hex")),
    paths,
  });
}
console.log(JSON.stringify({
  joined_sha256: createHash("sha256").update(allPaths.join("\n")).digest("hex"),
  elements: output,
}, null, 2));
