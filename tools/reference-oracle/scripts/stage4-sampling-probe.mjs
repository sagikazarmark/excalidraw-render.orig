import assert from 'node:assert/strict';
import {chromium} from '../node_modules/playwright/index.mjs';
import {readFileSync} from 'node:fs';
const profile=JSON.parse(readFileSync(new URL('../../../tests/oracle/reference-profiles/revision-1.json',import.meta.url)));
async function collect(){
 const browser=await chromium.launch({args:profile.browser.launch.explicitArguments,chromiumSandbox:false,headless:true});
 try {
  assert.equal(browser.version(),'149.0.7827.55');
  const context=await browser.newContext(profile.browser.context),page=await context.newPage();
  return await page.evaluate(async()=>{
   const rows=[];
   for(const alpha of [false,true]) for(const size of [6,8,40]) {
    const src=document.createElement('canvas');src.width=2;src.height=2;
    const c=src.getContext('2d'),p=alpha?[255,0,0,255,255,0,0,128,255,0,0,128,255,0,0,255]:[0,0,0,255,255,255,255,255,255,255,255,255,0,0,0,255];
    c.putImageData(new ImageData(new Uint8ClampedArray(p),2,2),0,0);
    const image=new Image();image.src=src.toDataURL();await image.decode();
    const out=document.createElement('canvas');out.width=size;out.height=size;
    const ctx=out.getContext('2d');ctx.drawImage(image,0,0,size,size);
    const pixels=ctx.getImageData(0,0,size,1).data;
    rows.push({alpha,size,row:Array.from({length:size},(_,i)=>pixels[i*4+(alpha?3:0)])});
   }
   const source=document.createElement('canvas');source.width=3;source.height=2;
   source.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray([255,0,0,255,0,255,0,255,0,0,255,255,0,255,255,255,255,0,255,255,255,255,0,255]),3,2),0,0);
   const image=new Image();image.src=source.toDataURL();await image.decode();
   const dest=document.createElement('canvas');dest.width=40;dest.height=40;
   const ctx=dest.getContext('2d');ctx.translate(30.5,30.5);ctx.scale(-1,-1);ctx.beginPath();ctx.rect(0,0,20.5,20.5);ctx.clip();ctx.drawImage(image,-10.25,0,30.75,20.5);
   rows.push({crop:'right two columns, both axes flipped, 20.5 square',pixels:[[14,14],[25,14],[14,25],[25,25]].map(([x,y])=>({x,y,rgba:[...ctx.getImageData(x,y,1,1).data]}))});
   return rows;
  });
 }finally{await browser.close();}
}
const first=await collect();assert.deepEqual(first,await collect());console.log(JSON.stringify({browser:'149.0.7827.55',fresh_processes:2,rows:first},null,2));
