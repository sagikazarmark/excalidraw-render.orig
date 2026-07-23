import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

const ORACLE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO_ROOT = resolve(ORACLE_ROOT, "../..");
const CASE_ROOT = resolve(
  REPO_ROOT,
  "tests/conformance/cases/focused/pdf-isolated-fallback",
);
const ORIGIN = "http://reference.invalid";
const BROWSER_VERSION = "149.0.7827.55";
const OPTIONS = {
  exportBackground: true,
  exportEmbedScene: false,
  exportPadding: 10,
  exportScale: 1,
  exportWithDarkMode: false,
  frameRendering: { clip: true, enabled: true, name: true, outline: true },
  maxWidthOrHeight: null,
  viewBackgroundColor: "#ffffff",
};

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function contentType(path) {
  if (path.endsWith(".html")) return "text/html; charset=utf-8";
  if (path.endsWith(".js")) return "text/javascript; charset=utf-8";
  if (path.endsWith(".css")) return "text/css; charset=utf-8";
  return "application/octet-stream";
}

async function renderOnce(scene) {
  const browser = await chromium.launch({
    args: [
      "--disable-gpu",
      "--font-render-hinting=none",
      "--force-color-profile=srgb",
    ],
    chromiumSandbox: false,
    headless: true,
  });
  try {
    if (browser.version() !== BROWSER_VERSION) {
      throw new Error(`expected Chromium ${BROWSER_VERSION}, received ${browser.version()}`);
    }
    const context = await browser.newContext({
      colorScheme: "light",
      deviceScaleFactor: 1,
      forcedColors: "none",
      locale: "en-US",
      reducedMotion: "reduce",
      serviceWorkers: "block",
      timezoneId: "UTC",
      viewport: { height: 720, width: 1280 },
    });
    const page = await context.newPage();
    const failures = [];
    page.on("console", (message) => {
      if (message.type() === "error") failures.push(message.text());
    });
    page.on("pageerror", (error) => failures.push(error.message));
    await page.route("**/*", async (route) => {
      const url = new URL(route.request().url());
      let path;
      if (url.origin !== ORIGIN) {
        failures.push(`external request: ${url.href}`);
        await route.abort("blockedbyclient");
        return;
      }
      if (url.pathname === "/") {
        path = resolve(ORACLE_ROOT, "dist/index.html");
      } else if (url.pathname.startsWith("/bundle/")) {
        path = resolve(ORACLE_ROOT, "dist", url.pathname.slice("/bundle/".length));
      } else {
        failures.push(`unknown local request: ${url.href}`);
        await route.abort("blockedbyclient");
        return;
      }
      await route.fulfill({ body: readFileSync(path), contentType: contentType(path) });
    });
    await page.goto(`${ORIGIN}/`, { waitUntil: "load" });
    await page.waitForFunction(() => typeof window.referenceOracle?.render === "function");
    const output = await page.evaluate(async ({ input, options }) => {
      const rendered = await window.referenceOracle.render(input, options);
      const blob = new Blob([rendered.svg], { type: "image/svg+xml" });
      const objectUrl = URL.createObjectURL(blob);
      try {
        const image = new Image();
        await new Promise((resolve, reject) => {
          image.addEventListener("load", resolve, { once: true });
          image.addEventListener("error", reject, { once: true });
          image.src = objectUrl;
        });
        const canvas = document.createElement("canvas");
        canvas.width = image.naturalWidth;
        canvas.height = image.naturalHeight;
        const context = canvas.getContext("2d", { alpha: true, willReadFrequently: true });
        context.drawImage(image, 0, 0);
        return {
          height: canvas.height,
          rgba: Array.from(context.getImageData(0, 0, canvas.width, canvas.height).data),
          svg: rendered.svg,
          width: canvas.width,
        };
      } finally {
        URL.revokeObjectURL(objectUrl);
      }
    }, { input: scene, options: OPTIONS });
    if (failures.length > 0) throw new Error(failures.join("\n"));
    return {
      height: output.height,
      rgba: Buffer.from(output.rgba),
      svg: Buffer.from(output.svg, "utf8"),
      width: output.width,
    };
  } finally {
    await browser.close();
  }
}

if (process.env.REFERENCE_ORACLE_CANONICAL !== "1" || process.version !== "v24.16.0") {
  throw new Error("run this script in the canonical reference-profile container");
}
if (process.argv[2] !== "verify") {
  throw new Error("usage: node scripts/pdf-fallback-evidence.mjs verify");
}
const scene = JSON.parse(readFileSync(resolve(CASE_ROOT, "scene.excalidraw"), "utf8"));
const first = await renderOnce(scene);
const second = await renderOnce(scene);
for (const field of ["width", "height"]) {
  if (first[field] !== second[field]) throw new Error(`${field} changed between fresh runs`);
}
if (!first.svg.equals(second.svg) || !first.rgba.equals(second.rgba)) {
  throw new Error("official SVG evidence changed between fresh browser processes");
}
if (!first.svg.equals(readFileSync(resolve(CASE_ROOT, "evidence/official.svg")))
    || !first.rgba.equals(readFileSync(resolve(CASE_ROOT, "evidence/official-svg.rgba")))) {
  throw new Error("reproduced fallback evidence differs from checked evidence");
}
console.log(JSON.stringify({
  height: first.height,
  rgba_sha256: sha256(first.rgba),
  svg_sha256: sha256(first.svg),
  width: first.width,
}));
