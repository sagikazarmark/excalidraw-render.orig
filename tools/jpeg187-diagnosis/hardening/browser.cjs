const fs = require('node:fs');
const {chromium} = require('/repo/tools/reference-oracle/node_modules/playwright');
(async () => {
  let checked=0;let failures=[];
  for(let repeat=0;repeat<2;repeat++) {
    const browser=await chromium.launch({headless:true,args:['--disable-gpu','--force-color-profile=srgb']});
    for(const name of fs.readdirSync('/probe').filter(n=>n.endsWith('.jpg')).sort()) {
      const context=await browser.newContext({deviceScaleFactor:1}); const page=await context.newPage();
      const rgba=await page.evaluate(async src=>{
        const img=new Image();img.src=src;await img.decode();const c=document.createElement('canvas');c.width=img.width;c.height=img.height;
        const ctx=c.getContext('2d');ctx.drawImage(img,0,0);return Array.from(ctx.getImageData(0,0,c.width,c.height).data);
      },'data:image/jpeg;base64,'+fs.readFileSync('/probe/'+name).toString('base64'));
      const expected=fs.readFileSync('/probe/'+name.replace('.jpg','.rgb'));
      if(rgba.length!==expected.length/3*4)throw new Error('dimensions '+name);
      let wrong=0;for(let i=0;i<expected.length/3;i++)for(let c=0;c<4;c++)if(rgba[i*4+c] !== (c===3?255:expected[i*3+c]))wrong++;
      if(wrong)failures.push({repeat,name,wrong,first:rgba.slice(0,4)});
      checked++;await context.close();
    }
    console.log(browser.version());await browser.close();
  }
  console.log(JSON.stringify({checked,exact:checked-failures.length,discrepant:failures.length}));
  console.log(JSON.stringify(failures));
})();
