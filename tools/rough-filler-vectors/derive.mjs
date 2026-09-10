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
// ellipses, lines without arrowheads (closed with a background, open, or
// rounded), arrows with any arrowheads, or freedraw loops with a background,
// the shapes the test scenes carry; each element's paths are printed in
// document order, the fill first, then the outline, then each arrowhead's
// sets in the reference's order (start head, then end head), with their
// SHA-256 digests and the digest of every path joined by `\n`, which is what
// the Rust tests pin. A freedraw loop prints only its Rough.js background
// fill: the reference draws its stroke with perfect-freehand, not Rough.js,
// and passes `stroke: "none"` to the curve. An arrow's heads are generated
// through the reference's `getArrowheadShapes` with the one options object
// the outline and both heads share, so a head that assigns to that object
// (the default branch's roughness clamp and dash) changes the head that
// follows, exactly as in the reference (#171); each head's `strokeLineDash`
// is printed beside its paths.
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const roughPath = process.argv[2];
if (!roughPath) {
  console.error("usage: derive.mjs <roughjs/bundled/rough.esm.js> < scene.excalidraw");
  process.exit(2);
}
const { default: rough } = await import(pathToFileURL(roughPath).href);
// `points-on-curve` is the dependency Excalidraw 0.18.1 simplifies a freedraw
// loop with before filling it, resolved beside the bundled Rough.js.
const { simplify } = await import(
  pathToFileURL(resolve(dirname(roughPath), "../../points-on-curve/lib/index.js")).href
);
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

// scene/Shape.ts `getDashArrayDashed` and `getDashArrayDotted`.
const getDashArrayDashed = (strokeWidth) => [8, 8 + strokeWidth];
const getDashArrayDotted = (strokeWidth) => [1.5, 6 + strokeWidth];

// scene/Shape.ts `generateRoughOptions`, for the shapes this script draws.
function generateRoughOptions(element, continuousPath = false) {
  const options = {
    seed: element.seed,
    strokeLineDash:
      element.strokeStyle === "dashed"
        ? getDashArrayDashed(element.strokeWidth)
        : element.strokeStyle === "dotted"
          ? getDashArrayDotted(element.strokeWidth)
          : undefined,
    disableMultiStroke: element.strokeStyle !== "solid",
    strokeWidth: element.strokeStyle !== "solid" ? element.strokeWidth + 0.5 : element.strokeWidth,
    fillWeight: element.strokeWidth / 2,
    hachureGap: element.strokeWidth * 4,
    roughness: adjustRoughness(element),
    stroke: element.strokeColor,
    preserveVertices: continuousPath || element.roughness < 2,
  };
  if (element.type === "arrow") {
    return options;
  }
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

// element/bounds.ts `getArrowheadSize`, `getArrowheadAngle` and
// `getArrowheadPoints`, and math `pointRotateRads`, as the 0.18.1 chunk
// spells them.
const pointRotateRads = ([x, y], [cx, cy], angle) => [
  (x - cx) * Math.cos(angle) - (y - cy) * Math.sin(angle) + cx,
  (x - cx) * Math.sin(angle) + (y - cy) * Math.cos(angle) + cy,
];
const degreesToRadians = (degrees) => (degrees * Math.PI) / 180;
const getArrowheadSize = (arrowhead) => {
  switch (arrowhead) {
    case "arrow":
      return 25;
    case "diamond":
    case "diamond_outline":
      return 12;
    case "crowfoot_many":
    case "crowfoot_one":
    case "crowfoot_one_or_many":
      return 20;
    default:
      return 15;
  }
};
const getArrowheadAngle = (arrowhead) => {
  switch (arrowhead) {
    case "bar":
      return 90;
    case "arrow":
      return 20;
    default:
      return 25;
  }
};
function getArrowheadPoints(element, shape, position, arrowhead) {
  if (shape.length < 1) {
    return null;
  }
  const ops = shape[0].sets.find((set) => set.type === "path")?.ops ?? shape[0].sets[0].ops;
  if (ops.length < 1) {
    return null;
  }
  const index = position === "start" ? 1 : ops.length - 1;
  const data = ops[index].data;
  const p3 = [data[4], data[5]];
  const p2 = [data[2], data[3]];
  const p1 = [data[0], data[1]];
  const prevOp = ops[index - 1];
  let p0 = [0, 0];
  if (prevOp.op === "move") {
    p0 = [prevOp.data[0], prevOp.data[1]];
  } else if (prevOp.op === "bcurveTo") {
    p0 = [prevOp.data[4], prevOp.data[5]];
  }
  const equation = (t, idx) =>
    Math.pow(1 - t, 3) * p3[idx] +
    3 * t * Math.pow(1 - t, 2) * p2[idx] +
    3 * Math.pow(t, 2) * (1 - t) * p1[idx] +
    p0[idx] * Math.pow(t, 3);
  const [x2, y2] = position === "start" ? p0 : p3;
  const [x1, y1] = [equation(0.3, 0), equation(0.3, 1)];
  const distance = Math.hypot(x2 - x1, y2 - y1);
  const nx = (x2 - x1) / distance;
  const ny = (y2 - y1) / distance;
  const size = getArrowheadSize(arrowhead);
  let length = 0;
  {
    const [cx, cy] = position === "end" ? element.points[element.points.length - 1] : element.points[0];
    const [px, py] =
      element.points.length > 1
        ? position === "end"
          ? element.points[element.points.length - 2]
          : element.points[1]
        : [0, 0];
    length = Math.hypot(cx - px, cy - py);
  }
  const lengthMultiplier = arrowhead === "diamond" || arrowhead === "diamond_outline" ? 0.25 : 0.5;
  const minSize = Math.min(size, length * lengthMultiplier);
  const xs = x2 - nx * minSize;
  const ys = y2 - ny * minSize;
  if (arrowhead === "dot" || arrowhead === "circle" || arrowhead === "circle_outline") {
    const diameter = Math.hypot(ys - y2, xs - x2) + element.strokeWidth - 2;
    return [x2, y2, diameter];
  }
  const angle = getArrowheadAngle(arrowhead);
  if (arrowhead === "crowfoot_many" || arrowhead === "crowfoot_one_or_many") {
    const [x3, y3] = pointRotateRads([x2, y2], [xs, ys], degreesToRadians(-angle));
    const [x4, y4] = pointRotateRads([x2, y2], [xs, ys], degreesToRadians(angle));
    return [xs, ys, x3, y3, x4, y4];
  }
  const [x3, y3] = pointRotateRads([xs, ys], [x2, y2], (-angle * Math.PI) / 180);
  const [x4, y4] = pointRotateRads([xs, ys], [x2, y2], degreesToRadians(angle));
  if (arrowhead === "diamond" || arrowhead === "diamond_outline") {
    let ox;
    let oy;
    if (position === "start") {
      const [px, py] = element.points.length > 1 ? element.points[1] : [0, 0];
      [ox, oy] = pointRotateRads([x2 + minSize * 2, y2], [x2, y2], Math.atan2(py - y2, px - x2));
    } else {
      const [px, py] = element.points.length > 1 ? element.points[element.points.length - 2] : [0, 0];
      [ox, oy] = pointRotateRads([x2 - minSize * 2, y2], [x2, y2], Math.atan2(y2 - py, x2 - px));
    }
    return [x2, y2, x3, y3, ox, oy, x4, y4];
  }
  return [x2, y2, x3, y3, x4, y4];
}

// scene/Shape.ts `getArrowheadShapes`, verbatim: `options` is the object the
// outline was generated with and both heads share. The closed kinds delete
// its `strokeLineDash` and clamp the roughness on a spread copy; `crowfoot_one`
// draws with it as it stands; the default branch assigns its `strokeLineDash`
// and `roughness`, which the head that follows then reads (#171). The
// canvas background stands in for `canvasBackgroundColor`.
function getArrowheadShapes(element, shape, position, arrowhead, options, canvasBackgroundColor) {
  const arrowheadPoints = getArrowheadPoints(element, shape, position, arrowhead);
  if (arrowheadPoints === null) {
    return [];
  }
  const generateCrowfootOne = (arrowheadPoints, options) => {
    if (arrowheadPoints === null) {
      return [];
    }
    const [, , x3, y3, x4, y4] = arrowheadPoints;
    return [generator.line(x3, y3, x4, y4, options)];
  };
  switch (arrowhead) {
    case "dot":
    case "circle":
    case "circle_outline": {
      const [x, y, diameter] = arrowheadPoints;
      delete options.strokeLineDash;
      return [
        generator.circle(x, y, diameter, {
          ...options,
          fill: arrowhead === "circle_outline" ? canvasBackgroundColor : element.strokeColor,
          fillStyle: "solid",
          stroke: element.strokeColor,
          roughness: Math.min(0.5, options.roughness || 0),
        }),
      ];
    }
    case "triangle":
    case "triangle_outline": {
      const [x, y, x2, y2, x3, y3] = arrowheadPoints;
      delete options.strokeLineDash;
      return [
        generator.polygon(
          [
            [x, y],
            [x2, y2],
            [x3, y3],
            [x, y],
          ],
          {
            ...options,
            fill: arrowhead === "triangle_outline" ? canvasBackgroundColor : element.strokeColor,
            fillStyle: "solid",
            roughness: Math.min(1, options.roughness || 0),
          },
        ),
      ];
    }
    case "diamond":
    case "diamond_outline": {
      const [x, y, x2, y2, x3, y3, x4, y4] = arrowheadPoints;
      delete options.strokeLineDash;
      return [
        generator.polygon(
          [
            [x, y],
            [x2, y2],
            [x3, y3],
            [x4, y4],
            [x, y],
          ],
          {
            ...options,
            fill: arrowhead === "diamond_outline" ? canvasBackgroundColor : element.strokeColor,
            fillStyle: "solid",
            roughness: Math.min(1, options.roughness || 0),
          },
        ),
      ];
    }
    case "crowfoot_one":
      return generateCrowfootOne(arrowheadPoints, options);
    default: {
      const [x2, y2, x3, y3, x4, y4] = arrowheadPoints;
      if (element.strokeStyle === "dotted") {
        const dash = getDashArrayDotted(element.strokeWidth - 1);
        options.strokeLineDash = [dash[0], dash[1] - 1];
      } else {
        delete options.strokeLineDash;
      }
      options.roughness = Math.min(1, options.roughness || 0);
      return [
        generator.line(x3, y3, x2, y2, options),
        generator.line(x4, y4, x2, y2, options),
        ...(arrowhead === "crowfoot_one_or_many"
          ? generateCrowfootOne(getArrowheadPoints(element, shape, position, "crowfoot_one"), options)
          : []),
      ];
    }
  }
}

// scene/Shape.ts `generateElementShape`, for the shapes this script draws.
// Returns the drawables in the reference's order: the shape itself, then
// each arrowhead's.
function shape(element) {
  const w = element.width;
  const h = element.height;
  switch (element.type) {
    case "rectangle": {
      if (element.roundness) {
        const r = getCornerRadius(Math.min(w, h), element);
        return [generator.path(
          `M ${r} 0 L ${w - r} 0 Q ${w} 0, ${w} ${r} L ${w} ${h - r} Q ${w} ${h}, ${w - r} ${h} L ${r} ${h} Q 0 ${h}, 0 ${h - r} L 0 ${r} Q 0 0, ${r} 0`,
          generateRoughOptions(element, true),
        )];
      }
      return [generator.rectangle(0, 0, w, h, generateRoughOptions(element, false))];
    }
    case "diamond": {
      // `getDiamondPoints`: the vertices sit at `floor(extent / 2) + 1`, and
      // a rounded diamond's radii come from those vertex offsets (#160).
      const [topX, topY, rightX, rightY, bottomX, bottomY, leftX, leftY] =
        [Math.floor(w / 2) + 1, 0, w, Math.floor(h / 2) + 1, Math.floor(w / 2) + 1, h, 0, Math.floor(h / 2) + 1];
      if (element.roundness) {
        const verticalRadius = getCornerRadius(Math.abs(topX - leftX), element);
        const horizontalRadius = getCornerRadius(Math.abs(rightY - topY), element);
        return [generator.path(
          `M ${topX + verticalRadius} ${topY + horizontalRadius} L ${rightX - verticalRadius} ${rightY - horizontalRadius}
            C ${rightX} ${rightY}, ${rightX} ${rightY}, ${rightX - verticalRadius} ${rightY + horizontalRadius}
            L ${bottomX + verticalRadius} ${bottomY - horizontalRadius}
            C ${bottomX} ${bottomY}, ${bottomX} ${bottomY}, ${bottomX - verticalRadius} ${bottomY - horizontalRadius}
            L ${leftX + verticalRadius} ${leftY + horizontalRadius}
            C ${leftX} ${leftY}, ${leftX} ${leftY}, ${leftX + verticalRadius} ${leftY - horizontalRadius}
            L ${topX - verticalRadius} ${topY + horizontalRadius}
            C ${topX} ${topY}, ${topX} ${topY}, ${topX + verticalRadius} ${topY + horizontalRadius}`,
          generateRoughOptions(element, true),
        )];
      }
      return [generator.polygon(
        [[topX, topY], [rightX, rightY], [bottomX, bottomY], [leftX, leftY]],
        generateRoughOptions(element),
      )];
    }
    case "ellipse":
      return [generator.ellipse(w / 2, h / 2, w, h, generateRoughOptions(element))];
    case "line": {
      // A line with a background is a polygon, an open one a linear path, and
      // a rounded one a curve, as the reference's `line` case dispatches them.
      const options = generateRoughOptions(element);
      if (element.roundness) {
        return [generator.curve(element.points, options)];
      }
      if (options.fill) {
        return [generator.polygon(element.points, options)];
      }
      return [generator.linearPath(element.points, options)];
    }
    case "arrow": {
      // The reference's `arrow` case: the outline from the shared options,
      // then the start head, then the end head, each through
      // `getArrowheadShapes` with that same object. Elbow arrows are not
      // drawn here.
      const options = generateRoughOptions(element);
      const shape = element.roundness
        ? [generator.curve(element.points, options)]
        : [generator.linearPath(element.points, options)];
      const { startArrowhead = null, endArrowhead = "arrow" } = element;
      if (startArrowhead !== null) {
        shape.push(...getArrowheadShapes(element, shape, "start", startArrowhead, options, scene.appState?.viewBackgroundColor ?? "#ffffff"));
      }
      if (endArrowhead !== null) {
        shape.push(...getArrowheadShapes(element, shape, "end", endArrowhead, options, scene.appState?.viewBackgroundColor ?? "#ffffff"));
      }
      return shape;
    }
    case "freedraw": {
      // A freedraw loop's background is a curve over the RDP-simplified points
      // with `stroke: "none"`, as the reference's `freedraw` case builds it;
      // the stroke itself is perfect-freehand and not drawn here.
      return [generator.curve(simplify(element.points, 0.75), {
        ...generateRoughOptions(element),
        stroke: "none",
      })];
    }
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
  const drawables = shape(element);
  Math.random = nativeRandom;
  const paths = drawables.flatMap((drawable) => drawable.sets.map((set) => generator.opsToPath(set, 2)));
  allPaths.push(...paths);
  output.push({
    id: element.id,
    operations: paths.map((path) => (path.match(/[MCL]/g) ?? []).length),
    sha256: paths.map((path) => createHash("sha256").update(path).digest("hex")),
    // Each drawable's `strokeLineDash` as Rough.js copied it from the options
    // at generation time, the arrowheads' after `getArrowheadShapes` (#171).
    stroke_line_dash: drawables.map((drawable) => drawable.options.strokeLineDash ?? null),
    paths,
  });
}
console.log(JSON.stringify({
  joined_sha256: createHash("sha256").update(allPaths.join("\n")).digest("hex"),
  elements: output,
}, null, 2));
