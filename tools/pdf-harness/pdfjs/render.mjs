import { readFile, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import process from "node:process";
import { createCanvas } from "@napi-rs/canvas";
import {
  AnnotationMode,
  getDocument,
  VerbosityLevel,
} from "pdfjs-dist/legacy/build/pdf.mjs";

if (process.argv.length !== 6) {
  throw new Error("usage: render.mjs <input.pdf> <output.rgba> <width> <height>");
}

const [inputPath, outputPath, widthArg, heightArg] = process.argv.slice(2);
const expectedWidth = Number(widthArg);
const expectedHeight = Number(heightArg);
if (
  !Number.isSafeInteger(expectedWidth) ||
  !Number.isSafeInteger(expectedHeight) ||
  expectedWidth < 1 ||
  expectedHeight < 1
) {
  throw new Error("expected dimensions must be positive safe integers");
}
const data = new Uint8Array(await readFile(inputPath));
const loadingTask = getDocument({
  data,
  cMapUrl: undefined,
  disableAutoFetch: true,
  disableFontFace: true,
  disableRange: true,
  disableStream: true,
  enableHWA: false,
  enableXfa: false,
  iccUrl: undefined,
  isImageDecoderSupported: false,
  isOffscreenCanvasSupported: false,
  standardFontDataUrl: undefined,
  stopAtErrors: true,
  useSystemFonts: false,
  useWasm: false,
  verbosity: VerbosityLevel.ERRORS,
  wasmUrl: undefined,
});

const document = await loadingTask.promise;
try {
  if (document.numPages !== 1) {
    throw new Error("PDF.js profile accepts exactly one page");
  }
  const page = await document.getPage(1);
  if (page.rotate !== 0 || page.userUnit !== 1) {
    throw new Error("PDF.js profile rejects rotation and non-default UserUnit");
  }
  const viewport = page.getViewport({ scale: 96 / 72, rotation: 0 });
  if (!Number.isInteger(viewport.width) || !Number.isInteger(viewport.height)) {
    throw new Error("96 DPI page dimensions must be integral");
  }
  if (viewport.width !== expectedWidth || viewport.height !== expectedHeight) {
    throw new Error(
      `PDF.js produced ${viewport.width}x${viewport.height}, expected ${expectedWidth}x${expectedHeight}`,
    );
  }
  const canvas = createCanvas(viewport.width, viewport.height);
  const context = canvas.getContext("2d");
  context.clearRect(0, 0, viewport.width, viewport.height);
  await page.render({
    annotationMode: AnnotationMode.DISABLE,
    background: "rgba(0,0,0,0)",
    canvas,
    canvasContext: context,
    intent: "display",
    viewport,
  }).promise;
  const image = context.getImageData(0, 0, viewport.width, viewport.height);
  const rgba = Buffer.from(image.data.buffer, image.data.byteOffset, image.data.byteLength);
  for (let index = 0; index < rgba.length; index += 4) {
    if (rgba[index + 3] === 0) {
      rgba[index] = 0;
      rgba[index + 1] = 0;
      rgba[index + 2] = 0;
    }
  }
  await writeFile(outputPath, rgba);
  process.stdout.write(
    `${JSON.stringify({
      byte_length: rgba.length,
      height: viewport.height,
      layout: "top-left-row-major",
      pdfjs: "6.1.200",
      pixel_format: "straight-alpha-srgb-rgba8",
      sha256: createHash("sha256").update(rgba).digest("hex"),
      width: viewport.width,
    })}\n`,
  );
} finally {
  await loadingTask.destroy();
}
