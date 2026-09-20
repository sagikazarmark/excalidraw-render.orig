// The reference color oracle is a separately authored selected-frame static
// canvas. It avoids Chromium149's animated-orientation/natural-size defect.
// The original APNG is always the renderer input and is fully validated there.
import {deflateSync} from "node:zlib";
import {qualifiedFixtures,sceneFor} from "./png-stage4-qualified-fixtures.mjs";
export {sceneFor};
const word=n=>{const b=Buffer.alloc(4);b.writeUInt32BE(n);return b;};
function chunk(name,data){const body=Buffer.concat([Buffer.from(name),data]);let crc=0xffffffff;for(const b of body){crc^=b;for(let i=0;i<8;i++)crc=(crc>>>1)^((crc&1)?0xedb88320:0);}return Buffer.concat([word(data.length),body,word((crc^0xffffffff)>>>0)]);}
export function oracleFixtures(input){return qualifiedFixtures(input).map(row=>{
 const m=row.id.match(/-c(\d+)-d(\d+)-(intrinsic|key|none|palette)-(true|false)-(static|included|subframe)-o([67])$/);if(!m)throw Error(row.id);
 const color=+m[1],depth=+m[2],alpha=m[3],subframe=m[5]==="subframe",orientation=+m[6],gray=color===0||color===4;
 const w=subframe?17:15,h=subframe?16:13,outDepth=depth===16?16:8,full=outDepth===16?65535:255;
 const sourceSample=v=>depth===16?(v===0?0:v===255?65535:v*256+255-v):depth<8&&color!==3?Math.floor(v*((2**depth)-1)/255)*255/((2**depth)-1):v;
 const field=Array.from({length:195},(_,i)=>{const x=i%15,y=Math.floor(i/15);const p=[input.ramp[x],input.ramp[(x+5*y)%15],input.ramp[(3*x+7*y)%15],input.alpha[(x+3*y)%4]];if(gray)p[1]=p[2]=p[0];return p;});
 const palette=field.slice(0,Math.min(2**depth,195));
 const encode=opaque=>{
  const raw=[];for(let y=0;y<h;y++){raw.push(0);for(let x=0;x<w;x++){
   const xx=x-(subframe?1:0),yy=y-(subframe?2:0),inside=xx>=0&&xx<15&&yy>=0&&yy<13;
   let samples=gray?[0,0]:[0,0,0,0];
   if(inside){const i=yy*15+xx,p=alpha==="palette"?palette[i%palette.length]:field[i];const rgb=p.slice(0,3).map(v=>sourceSample(v));
    const a=opaque?full:alpha==="intrinsic"?sourceSample(p[3]):alpha==="palette"?p[3]:alpha==="key"&&rgb.every(v=>v===0)?0:full;
    samples=gray?[rgb[0],a]:[...rgb,a];
   }
   for(const value of samples)if(outDepth===16)raw.push(value>>8,value&255);else raw.push(value);
  }}
  const header=Buffer.concat([word(w),word(h),Buffer.from([outDepth,gray?4:6,0,0,0])]);
  const tiff=Buffer.from([73,73,42,0,8,0,0,0,1,0,18,1,3,0,1,0,0,0,orientation,0,0,0,0,0,0,0]);
  return Buffer.concat([Buffer.from("89504e470d0a1a0a","hex"),chunk("IHDR",header),...row.tags.map(t=>chunk(t.kind,Buffer.from(t.data))),chunk("eXIf",tiff),chunk("IDAT",deflateSync(Buffer.from(raw))),chunk("IEND",Buffer.alloc(0))]).toString("base64");
 };
 return {...row,reference_basis:"authored selected-frame static canvas; same profile, samples, alpha and early EXIF; original APNG orientation observations retained separately",static_reference_png:encode(false),static_reference_opaque_png:encode(true)};
});}
