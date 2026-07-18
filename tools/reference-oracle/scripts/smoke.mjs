import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

const ORACLE_ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const REPO_ROOT = resolve(ORACLE_ROOT, "../..");
const ORIGIN = "http://reference.invalid";
const BROWSER_ARGUMENTS = [
  "--disable-gpu",
  "--font-render-hinting=none",
  "--force-color-profile=srgb",
];
const CONTEXT_OPTIONS = {
  colorScheme: "light",
  deviceScaleFactor: 1,
  forcedColors: "none",
  locale: "en-US",
  reducedMotion: "reduce",
  serviceWorkers: "block",
  timezoneId: "UTC",
  viewport: { height: 720, width: 1280 },
};
const COMMON_EXPORT_OPTIONS = {
  exportBackground: true,
  exportEmbedScene: false,
  exportPadding: 10,
  exportWithDarkMode: false,
  frameRendering: {
    clip: true,
    enabled: true,
    name: true,
    outline: true,
  },
  viewBackgroundColor: "#ffffff",
};
const EXPORT_CASES = {
  explicitScale: {
    ...COMMON_EXPORT_OPTIONS,
    exportScale: 1,
    maxWidthOrHeight: null,
  },
  maximumDimension: {
    ...COMMON_EXPORT_OPTIONS,
    exportScale: null,
    maxWidthOrHeight: 100,
  },
};

const assetManifest = JSON.parse(
  readFileSync(resolve(REPO_ROOT, "tests/oracle/reference-profiles/revision-1-assets.json"), "utf8"),
);
const allowedAssets = new Set(assetManifest.officialPackage.assets.map((asset) => `/${asset.path}`));
const scene = JSON.parse(readFileSync(resolve(ORACLE_ROOT, "fixtures/profile-smoke.excalidraw"), "utf8"));

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function safePath(root, relativePath) {
  const path = resolve(root, relativePath);
  if (path !== root && !path.startsWith(`${root}${sep}`)) {
    throw new Error(`request escaped local root: ${relativePath}`);
  }
  return path;
}

function contentType(path) {
  if (path.endsWith(".html")) return "text/html; charset=utf-8";
  if (path.endsWith(".js")) return "text/javascript; charset=utf-8";
  if (path.endsWith(".css")) return "text/css; charset=utf-8";
  if (path.endsWith(".woff2")) return "font/woff2";
  return "application/octet-stream";
}

async function renderOnce(options) {
  const browser = await chromium.launch({
    args: BROWSER_ARGUMENTS,
    chromiumSandbox: false,
    headless: true,
  });
  if (browser.version() !== "149.0.7827.55") {
    await browser.close();
    throw new Error(`expected Chromium 149.0.7827.55, received ${browser.version()}`);
  }

  const context = await browser.newContext(CONTEXT_OPTIONS);
  const page = await context.newPage();
  const errors = [];
  const requests = [];

  page.on("console", (message) => {
    if (message.type() === "error" || message.text().includes("falling back")) {
      errors.push(`console ${message.type()}: ${message.text()}`);
    }
  });
  page.on("pageerror", (error) => errors.push(`page error: ${error.message}`));
  page.on("requestfailed", (request) => {
    errors.push(`request failed: ${request.url()} (${request.failure()?.errorText ?? "unknown"})`);
  });

  await page.route("**/*", async (route) => {
    const url = new URL(route.request().url());
    requests.push(url.href);
    if (url.origin !== ORIGIN) {
      errors.push(`external request: ${url.href}`);
      await route.abort("blockedbyclient");
      return;
    }

    let path;
    if (url.pathname === "/") {
      path = resolve(ORACLE_ROOT, "dist/index.html");
    } else if (url.pathname.startsWith("/bundle/")) {
      path = safePath(resolve(ORACLE_ROOT, "dist"), url.pathname.slice("/bundle/".length));
    } else if (url.pathname.startsWith("/assets/")) {
      const assetPath = url.pathname.slice("/assets".length);
      if (!allowedAssets.has(assetPath)) {
        errors.push(`unmanifested asset request: ${url.href}`);
        await route.abort("blockedbyclient");
        return;
      }
      path = safePath(
        resolve(ORACLE_ROOT, "node_modules/@excalidraw/excalidraw/dist/prod"),
        assetPath.slice(1),
      );
    } else {
      errors.push(`unknown local request: ${url.href}`);
      await route.abort("blockedbyclient");
      return;
    }

    await route.fulfill({
      body: readFileSync(path),
      contentType: contentType(path),
      status: 200,
    });
  });

  await page.goto(`${ORIGIN}/`, { waitUntil: "load" });
  await page.waitForFunction(() => typeof window.referenceOracle?.render === "function");
  const output = await page.evaluate(
    async ({ input, options }) => window.referenceOracle.render(input, options),
    { input: scene, options },
  );
  await browser.close();

  if (errors.length > 0) {
    throw new Error(errors.join("\n"));
  }
  return {
    png: Buffer.from(output.pngBase64, "base64"),
    requests,
    svg: Buffer.from(output.svg, "utf8"),
  };
}

if (process.env.REFERENCE_ORACLE_CANONICAL !== "1") {
  throw new Error("run this command through tools/reference-oracle/run-container.sh");
}

const result = {};
for (const [name, options] of Object.entries(EXPORT_CASES)) {
  const first = await renderOnce(options);
  const second = await renderOnce(options);
  result[name] = {
    png: { bytes: first.png.length, sha256: sha256(first.png) },
    requests: [...new Set(first.requests)].sort(),
    svg: { bytes: first.svg.length, sha256: sha256(first.svg) },
  };

  if (result[name].png.sha256 !== sha256(second.png) || result[name].svg.sha256 !== sha256(second.svg)) {
    throw new Error(`${name} produced different hashes in fresh Chromium processes`);
  }
}

console.log(JSON.stringify(result, null, 2));
