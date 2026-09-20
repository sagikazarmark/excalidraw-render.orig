// Inspect the pinned PDF.js image object before any NAPI canvas drawing.
import {readFile} from "node:fs/promises";
import {getDocument,OPS,VerbosityLevel} from "../pdfjs/node_modules/pdfjs-dist/legacy/build/pdf.mjs";
const path=process.argv[2];
const loading=getDocument({data:new Uint8Array(await readFile(path)),cMapUrl:undefined,disableAutoFetch:true,disableFontFace:true,disableRange:true,disableStream:true,enableHWA:false,enableXfa:false,iccUrl:undefined,isImageDecoderSupported:false,isOffscreenCanvasSupported:false,standardFontDataUrl:undefined,stopAtErrors:true,useSystemFonts:false,useWasm:false,verbosity:VerbosityLevel.ERRORS,wasmUrl:undefined});
const document=await loading.promise;
try{
 const page=await document.getPage(1),ops=await page.getOperatorList();
 const images=[];
 for(let i=0;i<ops.fnArray.length;i++)if(ops.fnArray[i]===OPS.paintImageXObject){
  const id=ops.argsArray[i][0];
  const image=await new Promise(resolve=>page.objs.get(id,resolve));
  images.push({id,width:image.width,height:image.height,kind:image.kind,data:[...image.data]});
 }
 console.log(JSON.stringify({input:path,images}));
}finally{await loading.destroy();}
