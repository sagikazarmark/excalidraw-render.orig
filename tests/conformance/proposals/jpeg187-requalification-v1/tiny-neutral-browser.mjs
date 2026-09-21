// Independent pinned-browser check of the minimized 9x9 sequential fixture.
import assert from "node:assert/strict";
import { createRequire } from "node:module";
const require = createRequire("/workspace/tools/reference-oracle/package.json");
const { chromium } = require("playwright");
const jpeg = "/9j/4AAQSkZJRgABAgAAAQABAAD/wAARCAAJAAkDAEIAAREBAhEB/9sAQwABAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEB/9sAQwEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEB/8QAHwAAAQUBAQEBAQEAAAAAAAAAAAECAwQFBgcICQoL/8QAtRAAAgEDAwIEAwUFBAQAAAF9AQIDAAQRBRIhMUEGE1FhByJxFDKBkaEII0KxwRVS0fAkM2JyggkKFicYGRolJicoKSo0NTY3ODk6Q0RFRkdISUpTVFVWV1hZWmNkZWZnaGlqc3R1dnd4eXqDhIWGh4iJipKTlJWWl5iZmqKjpKWmp6ipqrKztLW2t7i5usLDxMXGx8jJytLT1NXW19jZ2uHi4+Tl5ufo6erx8vP09fb3+Pn6/8QAHwEAAwEBAQEBAQEBAQAAAAAAAAECAwQFBgcICQoL/8QAtREAAgECBAQDBAcFBAQAAQJ3AAECAxEEBSExBhJBUQdhcRMiMoEIFEKRobHBCSMzUvAVYnLRChYkNOEl8RcYGRomJygpKjU2Nzg5OkNERUZHSElKU1RVVldYWVpjZGVmZ2hpanN0dXZ3eHl6goOEhYaHiImKkpOUlZaXmJmaoqOkpaanqKmqsrO0tba3uLm6wsPExcbHyMnK0tPU1dbX2Nna4uPk5ebn6Onq8vP09fb3+Pn6/9oACAEAAAA/APz/AKKKK//aAAgBAREAPwAP/9oACAECEQA/AA//2Q==";
// Correct the copied unused AC-table symbol to the encoder's actual bytes.
// Keep the originally probed literal visible; this is not a new decode result.
assert.equal(jpeg.split("FicY").length, 2);
const correctedJpeg = jpeg.replace("FicY", "FhcY");
for (let i = 0; i < 2; i++) {
  const browser = await chromium.launch({ headless: true, args: ["--force-color-profile=srgb"], chromiumSandbox: false });
  try {
    assert.equal(browser.version(), "149.0.7827.55");
    const page = await browser.newPage();
    const result = await page.evaluate(async jpeg => {
      const image = new Image(); image.src = `data:image/jpeg;base64,${jpeg}`; await image.decode();
      const canvas = document.createElement("canvas"); canvas.width = 9; canvas.height = 9;
      const ctx = canvas.getContext("2d", { colorSpace: "srgb" }); ctx.drawImage(image, 0, 0);
      const bytes = Array.from(ctx.getImageData(0, 0, 9, 9).data);
      return { width: image.naturalWidth, height: image.naturalHeight, bytes };
    }, correctedJpeg);
    assert.equal(result.width, 9); assert.equal(result.height, 9);
    assert.deepEqual(result.bytes, Array.from({length:81}, () => [96,96,96,255]).flat());
    console.log(`fresh process ${i}: all 81 pixels exactly [96,96,96,255]`);
  } finally { await browser.close(); }
}
