// Independent browser-canvas phase controls: no scene restore, font or image.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { resolve } from "node:path";
import { chromium } from "../reference-oracle/node_modules/playwright/index.mjs";
const root = resolve(import.meta.dirname, "../..");
assert.equal(process.env.REFERENCE_ORACLE_CANONICAL, "1");
const profilePath = resolve(root, "tests/oracle/reference-profiles/revision-1.json");
const profile = JSON.parse(readFileSync(profilePath));
async function collect() {
  const browser = await chromium.launch({args:profile.browser.launch.explicitArguments,headless:true,chromiumSandbox:false});
  try {
    assert.equal(browser.version(), "149.0.7827.55");
    const page = await browser.newPage(profile.browser.context);
    await page.route("**/*", route => route.abort());
    return await page.evaluate(() => {
      const rows = [];
      for (const kind of ["horizontal-line", "vertical-line", "rounded-rectangle"]) {
        for (const phase of [0, 0.125, 0.233470223713, 0.25, 0.375, 0.5, 0.632993081447, 0.75, 0.875]) {
          const canvas = document.createElement("canvas"); canvas.width=100;canvas.height=100;
          const ctx=canvas.getContext("2d");ctx.strokeStyle="#1e1e1e";ctx.lineWidth=2;
          ctx.beginPath();
          if(kind==="horizontal-line"){ctx.moveTo(10,30+phase);ctx.lineTo(90,30+phase);}
          if(kind==="vertical-line"){ctx.moveTo(30+phase,10);ctx.lineTo(30+phase,90);}
          if(kind==="rounded-rectangle"){
            const y=30+phase;
            ctx.moveTo(20,y);ctx.lineTo(80,y);ctx.bezierCurveTo(90,y,90,y,90,y+10);
            ctx.lineTo(90,y+40);ctx.bezierCurveTo(90,y+50,90,y+50,80,y+50);
            ctx.lineTo(20,y+50);ctx.bezierCurveTo(10,y+50,10,y+50,10,y+40);
            ctx.lineTo(10,y+10);ctx.bezierCurveTo(10,y,10,y,20,y);ctx.closePath();
          }
          ctx.stroke();
          const rgba=ctx.getImageData(0,0,100,100).data;
          rows.push({kind,phase,alpha:[28,29,30,31,32].map(p=>rgba[(kind==="vertical-line"?50*100+p:p*100+50)*4+3])});
        }
      }
      return rows;
    });
  } finally {await browser.close();}
}
const a=await collect();assert.deepEqual(a,await collect());
const hash=bytes=>createHash("sha256").update(bytes).digest("hex");
const record={schema:"excalidraw-render/canvas-stroke-phase-probe/v1",browser:"149.0.7827.55",fresh_process_runs:2,
  script_sha256:hash(readFileSync(new URL(import.meta.url))),profile_sha256:hash(readFileSync(profilePath)),rows:a,approval:null};
writeFileSync(process.argv[2],JSON.stringify(record,null,2)+"\n",{flag:"wx"});
