// Prints committed official Excalidraw SVGs to PDF with Chromium's page.pdf().
// Excalidraw 0.18.1 has no PDF export, so these PDFs are "official SVG printed
// by Chromium", never an Excalidraw artifact.
//
// `cargo run --locked -p xtask -- demo build` runs it inside the pinned reference-oracle
// Playwright container with no network: each SVG is served from a fixed origin
// by request interception, and every other request fails the job.
//
// usage: node print-official-pdf.mjs <jobs.json>
//   jobs.json: [{"svg": "<path>", "pdf": "<path>", "width": "<css px>", "height": "<css px>"}]
//   Paths are absolute container paths; the PDF must not exist yet.
import { readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";

// Playwright is the oracle's pinned dependency, installed under
// tools/reference-oracle/node_modules; resolve it from there.
const require = createRequire("/workspace/tools/reference-oracle/package.json");
const { chromium } = require("playwright");

const ORIGIN = "http://demo.invalid";
// The oracle's launch arguments and browser context (tests/oracle/reference-profiles/revision-1.json).
const BROWSER_ARGUMENTS = ["--disable-gpu", "--font-render-hinting=none", "--force-color-profile=srgb"];
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

const [jobsPath, ...rest] = process.argv.slice(2);
if (!jobsPath || rest.length !== 0) {
  throw new Error("usage: node print-official-pdf.mjs <jobs.json>");
}
const jobs = JSON.parse(readFileSync(jobsPath, "utf8"));

const browser = await chromium.launch({ args: BROWSER_ARGUMENTS, chromiumSandbox: false, headless: true });
try {
  for (const job of jobs) {
    // A fresh context per SVG, so no font or image cache crosses documents.
    const context = await browser.newContext(CONTEXT_OPTIONS);
    const page = await context.newPage();
    const problems = [];
    page.on("console", (message) => problems.push(`console ${message.type()}: ${message.text()}`));
    page.on("pageerror", (error) => problems.push(`page error: ${error.message}`));
    page.on("requestfailed", (request) => problems.push(`request failed: ${request.url()}`));
    const url = `${ORIGIN}/official.svg`;
    await context.route("**/*", (route) => {
      if (route.request().url() === url) {
        return route.fulfill({ status: 200, contentType: "image/svg+xml", body: readFileSync(job.svg) });
      }
      problems.push(`unexpected request: ${route.request().url()}`);
      return route.abort();
    });
    await page.goto(url, { waitUntil: "load" });
    await page.evaluate(() => document.fonts.ready);
    const pdf = await page.pdf({
      width: `${job.width}px`,
      height: `${job.height}px`,
      margin: { top: "0", right: "0", bottom: "0", left: "0" },
      printBackground: true,
      pageRanges: "1",
    });
    if (problems.length !== 0) {
      throw new Error(`${job.svg}: ${problems.join("; ")}`);
    }
    writeFileSync(job.pdf, pinDates(pdf, job.svg), { flag: "wx" });
    await context.close();
  }
} finally {
  await browser.close();
}

// Skia stamps the wall clock into /CreationDate and /ModDate; everything else
// in its output is a function of the input. Overwrite both with a fixed date of
// the same length, so cross-reference offsets stay valid and reprints match.
function pinDates(pdf, label) {
  const text = Buffer.from(pdf).toString("latin1");
  const date = /\/(CreationDate|ModDate) \(D:\d{14}\+00'00'\)/gu;
  const found = text.match(date) ?? [];
  if (found.length !== 2) {
    throw new Error(`${label}: expected one CreationDate and one ModDate, found ${found.length} dates`);
  }
  return Buffer.from(text.replace(date, "/$1 (D:20000101000000+00'00')"), "latin1");
}
