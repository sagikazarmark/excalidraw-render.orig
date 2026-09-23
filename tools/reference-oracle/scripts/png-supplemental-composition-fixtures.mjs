// Supplemental encoded-layout/profile/transparency/subframe crossings (#85
// codec:png:alpha, codec:png:color-profile). The frozen composition producer
// stays unchanged; this copies its construction with an explicit signal list,
// orientation 6 and early eXIf, which the pinned browser honours.
import {deflateSync} from "node:zlib";
import {fixtures,sceneFor} from "./png-color-fixtures.mjs";
import {stage4Fixtures} from "./png-stage4-fixtures.mjs";
export {sceneFor};
const u32=n=>{const b=Buffer.alloc(4);b.writeUInt32BE(n);return b;};
function chunk(kind,data){const body=Buffer.concat([Buffer.from(kind),data]);let crc=0xffffffff;for(const b of body){crc^=b;for(let i=0;i<8;i++)crc=(crc>>>1)^((crc&1)?0xedb88320:0);}return Buffer.concat([u32(data.length),body,u32((crc^0xffffffff)>>>0)]);}
function exif(o){return Buffer.from([0x49,0x49,42,0,8,0,0,0,1,0,0x12,1,3,0,1,0,0,0,o,0,0,0,0,0,0,0]);}
function control(seq,w,h,x,y){const b=Buffer.alloc(26);[seq,w,h,x,y].forEach((v,i)=>b.writeUInt32BE(v,i*4));b.writeUInt16BE(65535,20);b.writeUInt16BE(1,22);b[24]=2;b[25]=1;return b;}
// SDR signal families the frozen composition matrix does not cross with every
// layout/transparency/interlace/container: the sRGB chunk, v2 matrix/TRC ICC,
// SDR/linear/BT.709/Display-P3 cICP and narrow-range cICP (ignored, #85). Gray
// layouts take the same chunks. HDR PQ gray completes the transfer crossings.
const configurations=[
 ...["srgb-0","icc-v2-p3","icc-v2-gamma1","cicp-p1-t13-r1","cicp-p12-t13-r1","cicp-p1-t8-r1","cicp-p1-t1-r1","cicp-p1-t13-r0"].map(s=>[s,false]),
 ...["srgb-0","gama-45455","cicp-p1-t13-r1","cicp-p1-t8-r1","cicp-p9-t16-r1"].map(s=>[s,true]),
];
export function supplementalCompositionFixtures(input){
 const sources=[...fixtures(input),...stage4Fixtures(input)];const result=[];
 for(const [signal,gray] of configurations){
  // Chunk tags do not depend on the layout they were first authored with.
  const source=sources.find(r=>r.signal===signal);if(!source)throw Error(signal);
  const layouts=[];for(const depth of [8,16])for(const alpha of ["intrinsic","key","none"])layouts.push({depth,alpha,color:gray?(alpha==="intrinsic"?4:0):(alpha==="intrinsic"?6:2)});
  if(gray){for(const depth of [1,2,4])for(const alpha of ["key","none"])layouts.push({depth,alpha,color:0});}
  else for(const depth of [1,2,4,8])layouts.push({depth,alpha:"palette",color:3});
  for(const layout of layouts)for(const adam7 of [false,true])for(const container of ["static","included","subframe"])for(const orientation of [6]){
   const fw=15,fh=13,w=container==="subframe"?17:fw,h=container==="subframe"?16:fh;
   const channels=({0:1,2:3,3:1,4:2,6:4})[layout.color];const max=(2**layout.depth)-1;
   const sample=(v,opaque=false)=>layout.depth===16?(v===0?0:v===255?65535:v*256+255-v):layout.depth<8?Math.floor(v*max/255):v;
   const field=Array.from({length:fw*fh},(_,i)=>{const x=i%fw,y=Math.floor(i/fw);const c=[input.ramp[x],input.ramp[(x+5*y)%15],input.ramp[(3*x+7*y)%15]];if(gray)c.fill(c[0]);return [...c,input.alpha[(x+3*y)%input.alpha.length]];});
   const palette=field.slice(0,Math.min(2**layout.depth,field.length));
   const alphas=field.map((p,i)=>layout.alpha==="intrinsic"?p[3]:layout.alpha==="palette"?palette[i%palette.length][3]:layout.alpha==="key"&&p.slice(0,gray?1:3).every(v=>sample(v)===0)?0:255);
   const canvasAlpha=Array(w*h).fill(container==="subframe"?0:255);for(let y=0;y<fh;y++)for(let x=0;x<fw;x++)canvasAlpha[(y+(container==="subframe"?2:0))*w+x+(container==="subframe"?1:0)]=alphas[y*fw+x];
   const orientedAlpha=Array(w*h);for(let y=0;y<h;y++)for(let x=0;x<w;x++){const dx=h-1-y,dy=orientation===6?x:w-1-x;orientedAlpha[dy*h+dx]=canvasAlpha[y*w+x];}
   const encode=(opaque)=>{
    const raw=(width,height,alternate=false)=>{
     const out=[];const passes=adam7?[[0,0,8,8],[4,0,8,8],[0,4,4,8],[2,0,4,4],[0,2,2,4],[1,0,2,2],[0,1,1,2]]:[[0,0,1,1]];
     for(const [x0,y0,dx,dy]of passes){if(x0>=width||y0>=height)continue;for(let y=y0;y<height;y+=dy){out.push(0);const values=[];for(let x=x0;x<width;x+=dx){const i=(y*fw+x)%field.length,p=alternate?[255,0,255,255]:field[i];
       if(layout.color===3)values.push(alternate?0:i%palette.length);
       else{const c=gray?[p[0]]:p.slice(0,3);if(layout.alpha==="intrinsic")c.push(opaque?255:p[3]);values.push(...c.map(v=>sample(v)));}}
      if(layout.depth<8){for(let i=0;i<values.length;i+=8/layout.depth){let b=0;for(let j=0;j<8/layout.depth&&i+j<values.length;j++)b|=values[i+j]<<(8-layout.depth*(j+1));out.push(b);}}
      else for(const v of values)if(layout.depth===16)out.push(v>>8,v&255);else out.push(v);
     }}return deflateSync(Buffer.from(out));
    };
    const header=Buffer.concat([u32(w),u32(h),Buffer.from([layout.depth,layout.color,0,0,+adam7])]);const chunks=[chunk("IHDR",header),...source.tags.map(t=>chunk(t.kind,Buffer.from(t.data)))];
    if(orientation===6)chunks.push(chunk("eXIf",exif(orientation)));
    if(layout.color===3){chunks.push(chunk("PLTE",Buffer.from(palette.flatMap(p=>p.slice(0,3)))));chunks.push(chunk("tRNS",Buffer.from(palette.map(p=>opaque?255:p[3]))));}
    if(layout.alpha==="key"&&!opaque)chunks.push(chunk("tRNS",Buffer.alloc(gray?2:6)));
    if(container==="static")chunks.push(chunk("IDAT",raw(w,h)));
    else{chunks.push(chunk("acTL",Buffer.concat([u32(2),u32(0)])));let seq=0;
      if(container==="subframe")chunks.push(chunk("IDAT",raw(w,h,true)));
      for(let f=0;f<2;f++){chunks.push(chunk("fcTL",control(seq++,fw,fh,container==="subframe"?1:0,container==="subframe"?2:0)));const data=raw(fw,fh,f!==0);
       if(container==="included"&&f===0)chunks.push(chunk("IDAT",data));else chunks.push(chunk("fdAT",Buffer.concat([u32(seq++),data])));}
    }
    if(orientation===7)chunks.push(chunk("eXIf",exif(orientation)));
    chunks.push(chunk("IEND",Buffer.alloc(0)));return Buffer.concat([Buffer.from("89504e470d0a1a0a","hex"),...chunks]).toString("base64");
   };
   const id=`supplement-cross-${signal}-${gray?"gray":"rgb"}-c${layout.color}-d${layout.depth}-${layout.alpha}-${adam7}-${container}-o${orientation}`;
   result.push({id,group:"composition",signal:id,layout:{color_type:layout.color,depth:layout.depth},width:h,height:w,alpha:orientedAlpha,tags:source.tags,expected_outcome:"accepted",animation_frames:container==="static"?1:2,png:encode(false),opaque_png:encode(true)});
  }
 }
 return result;
}

// The reference color oracle for every crossing is the separately authored
// selected-frame static canvas of png-stage4-oracle-fixtures.mjs (copied, the
// frozen producer is unchanged): it avoids Chromium149's animated-orientation/
// natural-size defect. The original APNG remains the renderer input.
const word=n=>{const b=Buffer.alloc(4);b.writeUInt32BE(n);return b;};
export function withStaticReference(input,row){

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
}
