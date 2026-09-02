// Development-only helper: encodes the tiny image payloads used by the
// image-codecs conformance case. Chromium is the encoder for JPEG and WebP.
import { chromium } from "playwright";

const browser = await chromium.launch({ args: ["--disable-gpu"] });
const page = await browser.newPage();
const out = await page.evaluate(() => {
  const canvas = document.createElement("canvas");
  canvas.width = 32;
  canvas.height = 32;
  const context = canvas.getContext("2d");
  // A deterministic four-quadrant pattern with a diagonal, so a crop or a flip
  // is visually unambiguous.
  const quadrants = [["#d64545", 0, 0], ["#2f9e44", 16, 0], ["#1971c2", 0, 16], ["#f08c00", 16, 16]];
  for (const [colour, x, y] of quadrants) {
    context.fillStyle = colour;
    context.fillRect(x, y, 16, 16);
  }
  context.strokeStyle = "#000000";
  context.lineWidth = 2;
  context.beginPath();
  context.moveTo(0, 0);
  context.lineTo(32, 32);
  context.stroke();
  return {
    png: canvas.toDataURL("image/png"),
    jpeg: canvas.toDataURL("image/jpeg", 0.92),
    webp: canvas.toDataURL("image/webp", 0.92),
  };
});
await browser.close();
console.log(JSON.stringify(out));
